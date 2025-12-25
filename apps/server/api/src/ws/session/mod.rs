use super::frame::{Frame, FrameError, FrameResult};
use super::session::listener::Listener;
use super::session::round::{Command, Round};
use crate::config;
use crate::llm::Model;
use crate::llm::client::ClientBuilder;
use crate::mcp::mcp_host::UnionMcpHost;
use crate::tts::TtsFactory;
use chrono::Local;
use core::result::Result;
use futures::Stream;
use rig::message::Message;
use service::chobits::message::hello::{AudioParam, HelloMessage};
use service::chobits::message::listen::ListenState;
use service::chobits::message::mcp::McpMessage;
use service::chobits::message::{AudioFormat, Transport};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc::{Sender, channel};
use tokio::sync::{Mutex, Notify};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info, instrument};

pub mod listener;
pub mod round;

#[derive(Default)]
pub struct SessionBuilder {
    id: Option<String>,
    listener: Option<Box<dyn Listener>>,
    model: Option<Arc<Box<dyn Model>>>,
    mcp_host: Option<Arc<Mutex<UnionMcpHost>>>,
    config: Option<SessionConfig>,
}

impl SessionBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_id(mut self, id: String) -> SessionBuilder {
        self.id = Some(id);
        self
    }

    pub fn with_listener(mut self, listener: Box<dyn Listener>) -> SessionBuilder {
        self.listener = Some(listener);
        self
    }

    pub fn with_model(mut self, model: Arc<Box<dyn Model>>) -> SessionBuilder {
        self.model = Some(model);
        self
    }

    pub fn with_mcp_host(mut self, mcp_host: Arc<Mutex<UnionMcpHost>>) -> SessionBuilder {
        self.mcp_host = Some(mcp_host);
        self
    }

    pub fn with_config(mut self, config: SessionConfig) -> SessionBuilder {
        self.config = Some(config);
        self
    }

    pub fn build(self) -> Session {
        Session::new(
            self.id.expect("id is required"),
            self.listener.expect("listener is required"),
            self.model.expect("model is required"),
            self.mcp_host.expect("mcp host is required"),
            self.config.expect("config is required"),
        )
    }
}

pub struct SessionConfig {
    pub close_connection_no_voice_time: Option<i64>,
}

pub struct Session {
    pub id: String,
    pub current_round: Option<Box<Round>>,
    output_tx: Option<Sender<Result<FrameResult, FrameError>>>,
    phase: Phase,
    latest_activity_time: Arc<Mutex<Option<i64>>>,
    history: Arc<Mutex<History>>,

    config: SessionConfig,

    model: Arc<Box<dyn Model>>,
    listener: Box<dyn Listener>,
    mcp_host: Arc<Mutex<UnionMcpHost>>,
}

#[derive(Debug, Clone)]
pub enum Phase {
    Hello,
    ListenDetect,
    Listen(ListenMode),
}

#[derive(Debug, Clone)]
pub enum ListenMode {
    Auto,
    Manual,
    RealTime,
}

pub struct History {
    pub preamble: Option<String>,
    pub chat_history: Vec<Message>,
}

impl Session {
    pub fn new(
        id: String,
        listener: Box<dyn Listener>,
        model: Arc<Box<dyn Model>>,
        mcp_host: Arc<Mutex<UnionMcpHost>>,
        config: SessionConfig,
    ) -> Self {
        let system_prompt = config::get().logic().system_prompt();
        Self {
            id,
            current_round: None,
            output_tx: None,
            phase: Phase::Hello,
            latest_activity_time: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(History {
                preamble: Some(system_prompt.to_string()),
                chat_history: vec![],
            })),

            config,

            listener,
            model,
            mcp_host,
        }
    }

    #[instrument(skip(self), name="Session start",fields(id = %self.id))]
    pub async fn start(&mut self) -> anyhow::Result<()> {
        info!("start");
        Ok(())
    }

    #[instrument(skip(self), name="Session stop" fields(id = %self.id))]
    pub async fn stop(&mut self) {
        self.stop_round().await;
        let tx = self.output_tx.clone().unwrap();
        let result = tx.send(Ok(FrameResult::CloseResult)).await;
        if result.is_err() {
            info!("tx send frame result close result failure");
        }
        info!("end");
    }

    #[instrument(skip(self), name="Session new round",fields(id = %self.id))]
    pub async fn new_round(&mut self) {
        info!("new round");
        self.stop_round().await;
        let tx = self
            .output_tx
            .clone()
            .expect("tx not create,maybe new round method before output frame method");
        // TODO: need consider client chat history
        let client = ClientBuilder::new()
            .with_model(self.model.clone())
            .with_mcp_host(self.mcp_host.clone())
            .build()
            .with_chat_history(Some(self.history.lock().await.chat_history.clone()));
        let tts = TtsFactory::global().default_tts.clone();
        self.current_round = Some(Box::new(Round::new(
            self.id.clone(),
            tx,
            Arc::new(client),
            tts,
        )));
        if let Some(round) = &mut self.current_round {
            round.start().await;
        } else {
            panic!("current round is none");
        }
    }

    pub async fn stop_round(&mut self) {
        if let Some(round) = &mut self.current_round {
            round.stop().await;
        }
    }

    pub async fn accept_frame<'a>(&mut self, frame: &Frame<'a>) {
        let phase = self.phase.clone();
        // info!(
        //     "current phase = {:?}, frame = {:?}",
        //     phase.clone(),
        //     frame.clone()
        // );
        if let Frame::Mcp(message) = frame {
            self.handle_mcp(message).await;
            return;
        }
        match phase {
            Phase::Hello => self.handle_phase_hello(frame).await,
            Phase::ListenDetect => self.handle_phase_listen_detect(frame).await,
            Phase::Listen(mode) => self.handle_phase_listen(&mode, frame).await,
        }
    }

    pub async fn output_frame(
        &mut self,
    ) -> impl Stream<Item = Result<FrameResult, FrameError>> + Unpin + Send + 'static {
        let (outer_tx, outer_rx) = channel::<Result<FrameResult, FrameError>>(1);
        let (inner_tx, mut inner_rx) = channel::<Result<FrameResult, FrameError>>(1);
        let frame_result_list = Arc::new(Mutex::new(VecDeque::new()));
        let frame_result_list_share_for_main_logic = frame_result_list.clone();
        let notify = Arc::new(Notify::new());
        let notify_share_for_main_logic = notify.clone();
        // frame send to core logic
        tokio::spawn(async move {
            while let Some(frame_result) = inner_rx.recv().await {
                let mut frame_result_list = frame_result_list.lock().await;
                if frame_result_list.is_empty() {
                    notify.notify_one();
                }
                frame_result_list.push_back(frame_result);
            }
        });
        // core logic handle
        let latest_activity_time = self.latest_activity_time.clone();
        tokio::spawn(async move {
            loop {
                let frame_result = {
                    let mut frame_result_list = frame_result_list_share_for_main_logic.lock().await;
                    frame_result_list.pop_front()
                };
                match frame_result {
                    Some(frame_result) => {
                        let mut time = latest_activity_time.lock().await;
                        *time = Some(Local::now().timestamp_millis());
                        let result = outer_tx.send(frame_result).await;
                        if result.is_err() {
                            info!("outer tx send frame result failure");
                            break;
                        }
                    }
                    None => {
                        notify_share_for_main_logic.notified().await;
                    }
                }
            }
        });
        self.output_tx = Some(inner_tx);
        ReceiverStream::new(outer_rx)
    }

    pub async fn update_latest_activity_time(&mut self) {
        let mut time = self.latest_activity_time.lock().await;
        *time = Some(Local::now().timestamp_millis());
    }

    pub async fn get_latest_activity_time(&mut self) -> Option<i64> {
        let time = self.latest_activity_time.lock().await;
        *time
    }
}

include!("handle/mcp.rs");
include!("handle/phase.rs");
