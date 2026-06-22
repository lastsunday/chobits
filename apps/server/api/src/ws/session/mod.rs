use super::frame::{Frame, FrameResult};
use super::session::listener::Listener;
use super::session::round::{Command, Round};
use crate::config::audio::AudioConfig;
use crate::config::session::SessionConfig;
use crate::llm::Model;
use crate::llm::client::{ClientBuilder, History};
use crate::mcp::client::device::{DeviceMcpClient, DeviceMcpPhase};
use crate::mcp::mcp_host::{McpHost, UnionMcpHost};
use crate::record::observer::{
    AsrContext, FrameContext, FrameDirection, RoundEndContext, RoundMode, RoundStartContext,
    SessionObserver,
};
use crate::tts::Tts;
use chrono::Local;
use core::result::Result;
use framework::error::AppError;
use futures::Stream;
use rig::message::ToolResult;
use service::chobits::message::hello::{AudioParam, HelloMessage};
use service::chobits::message::listen::ListenState;
use service::chobits::message::{AudioFormat, Transport};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{Sender, channel};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub mod listener;
pub mod output_controller;
pub mod round;

#[derive(Default)]
pub struct SessionBuilder {
    id: Option<String>,
    listener: Option<Box<dyn Listener>>,
    model: Option<Arc<Box<dyn Model>>>,
    tts: Option<Arc<Box<dyn Tts>>>,
    mcp_host: Option<Arc<Mutex<UnionMcpHost>>>,
    config: Option<Arc<SessionConfig>>,
    audio_config: Option<Arc<AudioConfig>>,
    observers: Vec<Arc<dyn SessionObserver>>,
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

    pub fn with_tts(mut self, tts: Arc<Box<dyn Tts>>) -> SessionBuilder {
        self.tts = Some(tts);
        self
    }

    pub fn with_mcp_host(mut self, mcp_host: Arc<Mutex<UnionMcpHost>>) -> SessionBuilder {
        self.mcp_host = Some(mcp_host);
        self
    }

    pub fn with_config(mut self, config: Arc<SessionConfig>) -> SessionBuilder {
        self.config = Some(config);
        self
    }

    pub fn with_audio_config(mut self, config: Arc<AudioConfig>) -> SessionBuilder {
        self.audio_config = Some(config);
        self
    }

    pub fn add_observer(mut self, observer: Arc<dyn SessionObserver>) -> SessionBuilder {
        self.observers.push(observer);
        self
    }

    pub fn build(self) -> Session {
        let config = self.config.expect("config is required");
        let audio_config = self.audio_config.expect("audio is required");
        let system_prompt = config
            .system_prompt
            .as_ref()
            .expect("logic system prompt is empty");
        Session {
            id: self.id.expect("id is required"),
            current_round: None,
            output_tx: None,
            seq: Arc::new(AtomicU64::new(1)),
            observers: self.observers,
            phase: Phase::Hello,
            current_mode: RoundMode::Auto,
            latest_activity_time: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(History {
                preamble: Some(system_prompt.to_string()),
                chat_history: vec![],
            })),
            output_cancel_token: CancellationToken::new(),
            config,
            audio_config,
            listener: self.listener.expect("listener is required"),
            model: self.model.expect("model is required"),
            tts: self.tts.expect("tts is required"),
            mcp_host: self.mcp_host.expect("mcp host is required"),
            device_mcp_phase: DeviceMcpPhase::Initialize,
            device_mcp_call_tool_result_tx: None,
        }
    }
}

type OutputTx = Option<Sender<Result<FrameResult, AppError>>>;

pub struct Session {
    pub id: String,
    pub current_round: Option<Box<Round>>,
    output_tx: OutputTx,
    seq: Arc<AtomicU64>,
    pub observers: Vec<Arc<dyn SessionObserver>>,
    phase: Phase,
    current_mode: RoundMode,
    latest_activity_time: Arc<Mutex<Option<i64>>>,
    history: Arc<Mutex<History>>,
    output_cancel_token: CancellationToken,

    config: Arc<SessionConfig>,
    audio_config: Arc<AudioConfig>,

    model: Arc<Box<dyn Model>>,
    tts: Arc<Box<dyn Tts>>,
    listener: Box<dyn Listener>,
    mcp_host: Arc<Mutex<UnionMcpHost>>,
    device_mcp_phase: DeviceMcpPhase,
    device_mcp_call_tool_result_tx: Option<Sender<anyhow::Result<ToolResult>>>,
}

#[derive(Debug, Clone)]
pub enum Phase {
    Hello,
    ListenDetect,
    Listen(ListenMode),
}

#[derive(Debug, Clone)]
pub enum ListenMode {
    // voice call
    Auto,
    // on button send voice
    Manual,
    // esp32
    RealTime,
}

impl Session {
    pub async fn start(&mut self) -> anyhow::Result<()> {
        info!(target:"session","start" );
        Ok(())
    }

    pub async fn stop(&mut self) {
        info!(target:"session", "stop");
        self.stop_round().await;
        self.output_cancel_token.cancel();
        let tx = self.output_tx.clone().expect("output tx not exists");
        let result = tx.send(Ok(FrameResult::CloseResult)).await;
        if result.is_err() {
            info!("tx send frame result close result failure");
        }
    }

    pub async fn new_round(&mut self, mode: RoundMode) {
        info!(target:"session", "new round");
        self.stop_round().await;
        self.current_mode = mode;
        let tx = self
            .output_tx
            .clone()
            .expect("tx not create,maybe new round method before output frame method");
        let client = ClientBuilder::new()
            .with_session_id(Some(self.id.clone()))
            .with_model(self.model.clone())
            .with_mcp_host(self.mcp_host.clone())
            .build()
            .with_history(self.history.clone())
            .with_max_prompt_len(self.config.max_prompt_len);
        let round_id = framework::id::gen_id();
        let cancel_token = CancellationToken::new();
        let traced_tx = output_controller::TracedSender::new(
            tx.clone(),
            self.observers.clone(),
            Some(round_id.clone()),
            self.seq.clone(),
            cancel_token.clone(),
        );
        for observer in &self.observers {
            observer.on_round_start(&RoundStartContext {
                round_id: round_id.clone(),
                session_id: Some(self.id.clone()),
                client_info: None,
                mode,
            });
        }
        self.current_round = Some(Box::new(Round::new(
            self.id.clone(),
            round_id,
            traced_tx,
            Arc::new(client),
            self.tts.clone(),
            self.observers.clone(),
            cancel_token,
        )));
        if let Some(round) = &mut self.current_round {
            round.start().await;
        } else {
            panic!("current round is none");
        }
    }

    pub async fn stop_round(&mut self) {
        info!(target:"session", "stop round");
        if let Some(round) = &mut self.current_round {
            let round_id = round.id.clone();
            round.stop().await;
            // Wait for task to finish (cancellation unblocks any blocked send)
            if let Some(handle) = round.join_handle.take() {
                tokio::time::timeout(Duration::from_secs(5), handle)
                    .await
                    .ok();
            }
            for observer in &self.observers {
                let _ = observer
                    .on_round_end(&RoundEndContext {
                        round_id: round_id.clone(),
                    })
                    .await;
            }
        }
    }

    pub async fn accept_frame<'a>(&mut self, frame: &Frame<'a>) {
        if let Some(round_id) = self.current_round.as_ref().map(|r| r.id.clone()) {
            let seq = self.seq.load(Ordering::Relaxed);
            for observer in &self.observers {
                observer
                    .on_frame(&FrameContext {
                        round_id: round_id.clone(),
                        seq,
                        direction: FrameDirection::Inbound,
                        detail: format!("{:?}", frame),
                    })
                    .await;
            }
        }

        match frame {
            Frame::Close(_) => {
                info!(target:"session","close");
                self.stop().await;
                return;
            }
            Frame::Abort(_) => {
                info!(target:"session","abort");
                self.new_round(self.current_mode).await;
                return;
            }
            Frame::Ping { .. } | Frame::Pong { .. } => return,
            _ => {}
        }

        if let Frame::Mcp(message) = frame {
            match self.device_mcp_phase {
                DeviceMcpPhase::ToolCall => {
                    let result = DeviceMcpClient::handle_mcp_tool_call_result(message).await;
                    let device_mcp_call_tool_result_tx = self
                        .device_mcp_call_tool_result_tx
                        .clone()
                        .expect("device mcp call tool result tx not exists");
                    if let Err(ex) = device_mcp_call_tool_result_tx.send(result).await {
                        panic!("can't send device mcp call tool result {:?}", ex);
                    }
                }
                _ => {
                    let mcp_host = self.mcp_host.clone();
                    let mut mcp_host = mcp_host.lock().await;
                    let device_mcp_client = mcp_host.get_device_client().await;
                    let device_mcp_client = device_mcp_client.clone();
                    if let Some(device_mcp_client) = device_mcp_client {
                        let mut device_mcp_client = device_mcp_client.lock().await;
                        self.device_mcp_phase = device_mcp_client.handle_mcp(message).await.clone();
                    } else {
                        error!("mcp device client not exists");
                    }
                }
            }
            return;
        }
        let phase = self.phase.clone();
        match phase {
            Phase::Hello => self.handle_phase_hello(frame).await,
            Phase::ListenDetect => self.handle_phase_listen_detect(frame).await,
            Phase::Listen(mode) => self.handle_phase_listen(&mode, frame).await,
        }
    }

    pub async fn output_frame(
        &mut self,
    ) -> impl Stream<Item = Result<FrameResult, AppError>> + Unpin + Send + 'static {
        let (controller_input_tx, controller_input_rx) =
            channel::<Result<FrameResult, AppError>>(64);
        let (controller_output_tx, controller_output_rx) =
            channel::<Result<FrameResult, AppError>>(64);

        let frame_duration = self
            .audio_config
            .output_frame_duration
            .expect("output frame duration is empty");
        let controller = self::output_controller::OutputController::new(
            controller_input_rx,
            controller_output_tx,
            frame_duration,
            self.latest_activity_time.clone(),
            self.output_cancel_token.child_token(),
        );
        controller.start();

        let (device_mcp_call_tool_result_tx, device_mcp_call_tool_result_rx) =
            channel::<anyhow::Result<ToolResult>>(1);
        self.device_mcp_call_tool_result_tx = Some(device_mcp_call_tool_result_tx);
        let mcp_device_client = DeviceMcpClient::new(
            Some(self.id.clone()),
            controller_input_tx.clone(),
            Arc::new(Mutex::new(device_mcp_call_tool_result_rx)),
        );
        let mcp_device_client = Arc::new(Mutex::new(mcp_device_client));
        let mcp_host = self.mcp_host.clone();
        let mut mcp_host = mcp_host.lock().await;
        mcp_host.set_device_client(mcp_device_client.clone()).await;
        self.listener.set_sender(controller_input_tx.clone()).await;
        self.output_tx = Some(controller_input_tx.clone());

        ReceiverStream::new(controller_output_rx)
    }

    pub async fn update_latest_activity_time(&self) {
        let mut time = self.latest_activity_time.lock().await;
        *time = Some(Local::now().timestamp_millis());
    }

    pub async fn get_latest_activity_time(&self) -> Option<i64> {
        let time = self.latest_activity_time.lock().await;
        *time
    }
}

include!("handle/phase.rs");
