use crate::config;
use crate::ws::frame::{Frame, FrameError, FrameResult};
use crate::ws::llm::llm_cache::LlmCache;
use crate::ws::mcp::{McpHost, device::DeviceMcpPhase};
use crate::ws::session::listener::Listener;
use crate::ws::session::round::{Command, Round};
use crate::ws::tts::tts_cache::TtsCache;
use chrono::Local;
use core::result::Result;
use framework::id::gen_id;
use futures::Stream;
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

pub struct Session<L> {
    pub id: String,
    pub current_round: Option<Box<Round>>,
    output_tx: Option<Sender<Result<FrameResult, FrameError>>>,
    listener: Box<L>,
    phase: Phase,
    latest_activity_time: Arc<Mutex<Option<i64>>>,
    close_connection_no_voice_time: Option<i64>,
    mcp_host: Arc<Mutex<Option<McpHost>>>,
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

impl<L> Session<L>
where
    L: Listener + Send,
{
    pub fn new(listener: Box<L>, close_connection_no_voice_time: Option<i64>) -> Self {
        Self {
            id: gen_id(),
            current_round: None,
            output_tx: None,
            listener,
            phase: Phase::Hello,
            latest_activity_time: Arc::new(Mutex::new(None)),
            close_connection_no_voice_time,
            mcp_host: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn update_latest_activity_time(&mut self) {
        let mut time = self.latest_activity_time.lock().await;
        *time = Some(Local::now().timestamp_millis());
    }

    pub async fn get_latest_activity_time(&mut self) -> Option<i64> {
        let time = self.latest_activity_time.lock().await;
        *time
    }

    #[instrument(skip(self), name="Session start",fields(id = %self.id))]
    pub async fn start(&mut self) {
        info!("start");
    }

    #[instrument(skip(self), name="Session stop" fields(id = %self.id))]
    pub async fn stop(&mut self) {
        if let Some(round) = &mut self.current_round {
            round.stop().await;
        }
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
        if let Some(round) = &mut self.current_round {
            round.stop().await;
        }
        let tx = self
            .output_tx
            .clone()
            .expect("tx not create,maybe new round method before output frame method");
        let llm = LlmCache::global().instance.clone();
        let tts = TtsCache::global().instance.clone();
        self.current_round = Some(Box::new(Round::new(
            self.id.clone(),
            tx,
            Arc::new(Mutex::new(llm)),
            Arc::new(Mutex::new(tts)),
            self.mcp_host.clone(),
        )));
        if let Some(round) = &mut self.current_round {
            round.start().await;
        } else {
            panic!("current round is none");
        }
    }

    pub async fn accept_frame(&mut self, frame: Frame) {
        let phase = self.phase.clone();
        // info!(
        //     "current phase = {:?}, frame = {:?}",
        //     phase.clone(),
        //     frame.clone()
        // );
        if let Frame::Mcp(message) = frame.clone() {
            let mcp_host = self.mcp_host.clone();
            let mut mcp_host = mcp_host.lock().await;
            if let Some(mcp_host) = mcp_host.as_mut() {
                match mcp_host.get_phase().await {
                    DeviceMcpPhase::Initialize => {
                        self.handle_mcp_initialize_result(mcp_host, &message).await;
                        self.request_mcp_tools_list(mcp_host).await;
                    }
                    DeviceMcpPhase::GetToolList => {
                        let has_next = self.handle_mcp_tools_list_result(mcp_host, &message).await;
                        if has_next {
                            self.request_mcp_tools_list(mcp_host).await;
                        } else {
                            // TODO: llm tools list setting value
                            let all_tools = mcp_host.get_all_tools().await;
                            let json = serde_json::to_string_pretty(&all_tools).unwrap();
                            info!("{}", json);
                        }
                    }
                }
            } else {
                error!("mcp host is none");
            }
            return;
        }
        match phase {
            Phase::Hello => match frame.clone() {
                Frame::Hello(hello_message) => {
                    let mut has_mcp = false;
                    if let Some(features) = &hello_message.features
                        && let Some(mcp) = features.mcp
                    {
                        has_mcp = mcp;
                    }
                    if has_mcp {
                        // TODO: init MCP host
                        self.mcp_host =
                            Arc::new(Mutex::new(Some(McpHost::new(Some(self.id.clone())))));
                        // TODO: init Server MCP client
                        // TODO: init Remote Server MCP client
                    }
                    self.handle_connect(&hello_message).await;
                    self.phase = Phase::ListenDetect;
                    if has_mcp {
                        let mcp_host = self.mcp_host.clone();
                        let mut mcp_host = mcp_host.lock().await;
                        let mcp_host = mcp_host.as_mut().expect("mcp host is none");
                        //init Device MCP client
                        self.request_mcp_initialize(mcp_host, &hello_message).await;
                    }
                }
                _ => {
                    error!(
                        "invalid frame in phase = {:?},frame = {:?}",
                        self.phase, frame
                    );
                }
            },
            Phase::ListenDetect => match frame.clone() {
                Frame::Listen(listen_message) => {
                    let state = listen_message.state;
                    match state {
                        ListenState::Start => {
                            let mode = listen_message.mmod;
                            if let Some(mode) = mode {
                                match mode {
                                    service::chobits::message::listen::ListenMode::Auto => {
                                        self.phase = Phase::Listen(ListenMode::Auto);
                                    }
                                    service::chobits::message::listen::ListenMode::Manual => {
                                        self.phase = Phase::Listen(ListenMode::Manual);
                                    }
                                    service::chobits::message::listen::ListenMode::RealTime => {
                                        self.phase = Phase::Listen(ListenMode::RealTime);
                                    }
                                }
                                Box::pin(self.accept_frame(frame)).await;
                            } else {
                                error!(
                                    "invalid frame in phase = {:?},frame = {:?}, state = {:?}",
                                    self.phase, frame, state
                                );
                            }
                        }
                        ListenState::Detect => {
                            // eps32-c3 default listen mode is none
                            // set listen mode to auto
                            self.phase = Phase::Listen(ListenMode::Auto);
                            Box::pin(self.accept_frame(frame)).await;
                        }
                        _ => {
                            error!(
                                "invalid frame in phase = {:?},frame = {:?}, state = {:?}",
                                self.phase, frame, state
                            );
                        }
                    }
                }
                Frame::Voice(bytes) => {
                    self.listener.listen(&bytes).await;
                }
                _ => {
                    error!(
                        "invalid frame in phase = {:?},frame = {:?}",
                        self.phase, frame
                    );
                }
            },
            Phase::Listen(mode) => match mode {
                ListenMode::Auto => match frame.clone() {
                    Frame::Listen(listen_message) => {
                        let state = listen_message.state;
                        match state {
                            ListenState::Start => {
                                let mode = listen_message.mmod;
                                if let Some(mode) = mode {
                                    match mode {
                                        service::chobits::message::listen::ListenMode::Auto => {
                                            self.phase = Phase::Listen(ListenMode::Auto);
                                        }
                                        service::chobits::message::listen::ListenMode::Manual => {
                                            self.phase = Phase::Listen(ListenMode::Manual);
                                            self.listener.reset(None).await;
                                        }
                                        service::chobits::message::listen::ListenMode::RealTime => {
                                            self.phase = Phase::Listen(ListenMode::RealTime);
                                        }
                                    }
                                } else {
                                    error!(
                                        "invalid frame in phase = {:?},frame = {:?}, state = {:?}",
                                        self.phase, frame, state
                                    );
                                }
                            }
                            ListenState::Detect => {
                                let text = listen_message.text;
                                match text {
                                    Some(text) => {
                                        info!("detect text = {}", text.to_string());
                                        self.update_latest_activity_time().await;
                                        self.new_round().await;
                                        //if match walk word
                                        if let Some(round) = &mut self.current_round {
                                            // TODO: detech voice id
                                            self.listener.set_state(
                                                crate::ws::session::listener::ListenState::End,
                                            );
                                            let command = self.listener.get_result().await;
                                            match command {
                                                Ok(command) => {
                                                    info!("command  = {:?}", command);
                                                    let mode = listen_message.mmod;
                                                    let mut is_text_message = false;
                                                    if let Some(mode) = mode {
                                                        is_text_message = mode == service::chobits::message::listen::ListenMode::Manual;
                                                    }
                                                    if is_text_message {
                                                        // text message handle
                                                        round
                                                            .accept_command(Command::Chat(text))
                                                            .await;
                                                    } else {
                                                        // TODO: replace text to command.text
                                                        //say hello
                                                        round
                                                            .accept_command(Command::Wake(text))
                                                            .await;
                                                    }
                                                }
                                                Err(e) => {
                                                    error!("{:?}", e);
                                                }
                                            }
                                            let silence_voice_timeout =
                                                config::get().logic().silence_voice_timeout();
                                            //reset listener to option(slinent condition limit)
                                            self.listener.reset(Some(silence_voice_timeout)).await;
                                        } else {
                                            panic!("current round is none");
                                        }
                                    }
                                    None => {
                                        error!(
                                            "invalid frame in phase = {:?},frame = {:?}",
                                            self.phase, frame
                                        );
                                    }
                                }
                            }
                            _ => {
                                error!(
                                    "invalid frame in phase = {:?},frame = {:?}",
                                    self.phase, frame
                                );
                            }
                        }
                    }
                    Frame::Voice(bytes) => {
                        let state = self.listener.get_state();
                        let mut round_end = true;
                        match &self.current_round {
                            Some(round) => {
                                round_end = round.end.load(Ordering::Relaxed);
                                // info!(
                                //     "listener listen round end = {} state = {:?}",
                                //     round_end, state,
                                // );
                                if round_end {
                                    //round is end
                                    if state == crate::ws::session::listener::ListenState::End {
                                        self.handle_listen_end().await;
                                        let silence_voice_timeout =
                                            config::get().logic().silence_voice_timeout();
                                        self.listener.reset(Some(silence_voice_timeout)).await;
                                        self.update_latest_activity_time().await;
                                    } else {
                                        self.listener.listen(&bytes).await;
                                    }
                                } else {
                                    //round is running
                                }
                            }
                            None => {
                                if state == crate::ws::session::listener::ListenState::End {
                                    self.handle_listen_end().await;
                                    let silence_voice_timeout =
                                        config::get().logic().silence_voice_timeout();
                                    self.listener.reset(Some(silence_voice_timeout)).await;
                                    self.update_latest_activity_time().await;
                                } else {
                                    self.listener.listen(&bytes).await;
                                }
                            }
                        }
                        let is_speech = match self.listener.get_state() {
                            listener::ListenState::Listening(speech) => speech,
                            _ => false,
                        };
                        if !round_end || is_speech {
                            self.update_latest_activity_time().await;
                        } else {
                            let latest_activity_time = self.get_latest_activity_time().await;
                            if let (
                                Some(latest_activity_time),
                                Some(close_connection_no_voice_time),
                            ) = (latest_activity_time, self.close_connection_no_voice_time)
                            {
                                let offset_time =
                                    Local::now().timestamp_millis() - latest_activity_time;
                                if offset_time >= close_connection_no_voice_time {
                                    self.stop().await;
                                }
                            }
                        }
                        // info!("latest_activity_time = {:?}", self.latest_activity_time);
                    }
                    _ => {
                        error!(
                            "invalid frame in phase = {:?},frame = {:?}",
                            self.phase, frame
                        );
                    }
                },
                ListenMode::Manual => match frame.clone() {
                    Frame::Listen(listen_message) => {
                        let state = listen_message.state;
                        match state {
                            ListenState::Start => {
                                let mode = listen_message.mmod;
                                if let Some(mode) = mode {
                                    match mode {
                                        service::chobits::message::listen::ListenMode::Auto => {
                                            self.phase = Phase::Listen(ListenMode::Auto);
                                            let silence_voice_timeout =
                                                config::get().logic().silence_voice_timeout();
                                            //reset listener to option(slinent condition limit)
                                            self.listener.reset(Some(silence_voice_timeout)).await;
                                        }
                                        service::chobits::message::listen::ListenMode::Manual => {
                                            self.phase = Phase::Listen(ListenMode::Manual);
                                            self.listener.reset(None).await;
                                            self.new_round().await;
                                        }
                                        service::chobits::message::listen::ListenMode::RealTime => {
                                            self.phase = Phase::Listen(ListenMode::RealTime);
                                        }
                                    }
                                } else {
                                    error!(
                                        "invalid frame in phase = {:?},frame = {:?}, state = {:?}",
                                        self.phase, frame, state
                                    );
                                }
                            }
                            ListenState::Stop => {
                                self.listener
                                    .set_state(crate::ws::session::listener::ListenState::End);
                                self.handle_listen_end().await;
                            }
                            ListenState::Detect => {
                                let text = listen_message.text;
                                match text {
                                    Some(text) => {
                                        info!("detect text = {}", text.to_string());
                                        self.new_round().await;
                                        //if match walk word
                                        if let Some(round) = &mut self.current_round {
                                            // handle send text
                                            round.accept_command(Command::Chat(text)).await;
                                        } else {
                                            panic!("current round is none");
                                        }
                                    }
                                    None => {
                                        error!(
                                            "invalid frame in phase = {:?},frame = {:?}",
                                            self.phase, frame
                                        );
                                    }
                                }
                            }
                            _ => {
                                error!(
                                    "invalid frame in phase = {:?},frame = {:?}",
                                    self.phase, frame
                                );
                            }
                        }
                    }
                    Frame::Voice(bytes) => {
                        self.listener.listen(&bytes).await;
                    }
                    _ => {
                        error!(
                            "invalid frame in phase = {:?},frame = {:?}",
                            self.phase, frame
                        );
                    }
                },
                ListenMode::RealTime => match frame.clone() {
                    Frame::Listen(listen_message) => {
                        let state = listen_message.state;
                        match state {
                            ListenState::Start => {
                                let mode = listen_message.mmod;
                                if let Some(mode) = mode {
                                    match mode {
                                        service::chobits::message::listen::ListenMode::Auto => {
                                            self.phase = Phase::Listen(ListenMode::Auto);
                                        }
                                        service::chobits::message::listen::ListenMode::Manual => {
                                            self.phase = Phase::Listen(ListenMode::Manual);
                                            self.listener.reset(None).await;
                                        }
                                        service::chobits::message::listen::ListenMode::RealTime => {
                                            self.phase = Phase::Listen(ListenMode::RealTime);
                                        }
                                    }
                                } else {
                                    error!(
                                        "invalid frame in phase = {:?},frame = {:?}, state = {:?}",
                                        self.phase, frame, state
                                    );
                                }
                            }
                            ListenState::Detect => {
                                let text = listen_message.text;
                                match text {
                                    Some(text) => {
                                        info!("detect text = {}", text.to_string());
                                        self.update_latest_activity_time().await;
                                        self.new_round().await;
                                        //if match walk word
                                        if let Some(round) = &mut self.current_round {
                                            // TODO: detech voice id
                                            self.listener.set_state(
                                                crate::ws::session::listener::ListenState::End,
                                            );
                                            let command = self.listener.get_result().await;
                                            match command {
                                                Ok(command) => {
                                                    info!("command  = {:?}", command);
                                                    //say hello
                                                    round.accept_command(Command::Wake(text)).await;
                                                }
                                                Err(e) => {
                                                    error!("{:?}", e);
                                                }
                                            }
                                            let silence_voice_timeout =
                                                config::get().logic().silence_voice_timeout();
                                            //reset listener to option(slinent condition limit)
                                            self.listener.reset(Some(silence_voice_timeout)).await;
                                        } else {
                                            panic!("current round is none");
                                        }
                                    }
                                    None => {
                                        error!(
                                            "invalid frame in phase = {:?},frame = {:?}",
                                            self.phase, frame
                                        );
                                    }
                                }
                            }
                            _ => {
                                error!(
                                    "invalid frame in phase = {:?},frame = {:?}",
                                    self.phase, frame
                                );
                            }
                        }
                    }
                    Frame::Voice(bytes) => {
                        let state = self.listener.get_state();
                        match &self.current_round {
                            Some(_round) => {
                                // info!(
                                //     "listener listen round end = {} state = {:?}",
                                //     round_end, state,
                                // );
                                self.listener.listen(&bytes).await;
                                if state == crate::ws::session::listener::ListenState::End {
                                    self.handle_listen_end().await;
                                    let silence_voice_timeout =
                                        config::get().logic().silence_voice_timeout();
                                    self.listener.reset(Some(silence_voice_timeout)).await;
                                    self.update_latest_activity_time().await;
                                }
                            }
                            None => {
                                if state == crate::ws::session::listener::ListenState::End {
                                    self.handle_listen_end().await;
                                    let silence_voice_timeout =
                                        config::get().logic().silence_voice_timeout();
                                    self.listener.reset(Some(silence_voice_timeout)).await;
                                    self.update_latest_activity_time().await;
                                } else {
                                    self.listener.listen(&bytes).await;
                                }
                            }
                        }
                        let is_speech = match self.listener.get_state() {
                            listener::ListenState::Listening(speech) => speech,
                            _ => false,
                        };
                        if is_speech {
                            self.update_latest_activity_time().await;
                        } else {
                            let latest_activity_time = self.get_latest_activity_time().await;
                            if let (
                                Some(latest_activity_time),
                                Some(close_connection_no_voice_time),
                            ) = (latest_activity_time, self.close_connection_no_voice_time)
                            {
                                //connection timeout handle
                                let offset_time =
                                    Local::now().timestamp_millis() - latest_activity_time;
                                // info!("offset_time = {}", offset_time);
                                if offset_time >= close_connection_no_voice_time {
                                    self.stop().await;
                                }
                            }
                        }
                        // info!("latest_activity_time = {:?}", self.latest_activity_time);
                    }
                    _ => {
                        error!(
                            "invalid frame in phase = {:?},frame = {:?}",
                            self.phase, frame
                        );
                    }
                },
            },
        }
    }

    async fn handle_listen_end(&mut self) {
        let command = self.listener.get_result().await;
        match command {
            Ok(command) => {
                self.new_round().await;
                info!("command = {:?}", command.clone());
                let is_speech_clear = self.is_speech_clear(command.prob);
                if let Some(round) = &mut self.current_round {
                    if is_speech_clear {
                        round.accept_command(Command::Chat(command.text)).await;
                    } else {
                        round
                            .accept_command(Command::ListenUnclear(command.text))
                            .await;
                    }
                } else {
                    panic!("current round is none");
                }
            }
            Err(e) => {
                error!("{:?}", e);
            }
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

    pub async fn stop_round(&mut self) {}

    async fn request_mcp_initialize(
        &mut self,
        mcp_host: &mut McpHost,
        _hello_message: &HelloMessage,
    ) {
        let tx = self.output_tx.clone().unwrap();
        let request = mcp_host.create_initialize_request().await;
        // mcp request send
        let result = tx.send(Ok(FrameResult::McpResult(request))).await;
        if result.is_err() {
            info!("tx send mcp initialize reqeust failure");
        }
    }

    async fn handle_mcp_initialize_result(&mut self, mcp_host: &mut McpHost, message: &McpMessage) {
        mcp_host.handle_initialize_result(&message.payload).await;
    }

    async fn request_mcp_tools_list(&mut self, mcp_host: &mut McpHost) {
        let tx = self.output_tx.clone().unwrap();
        let result = tx
            .send(Ok(FrameResult::McpResult(
                mcp_host.create_tools_list_request().await,
            )))
            .await;
        if result.is_err() {
            info!("tx send mcp tools list reqeust failure");
        }
    }

    async fn handle_mcp_tools_list_result(
        &mut self,
        mcp_host: &mut McpHost,
        message: &McpMessage,
    ) -> bool {
        return mcp_host.handle_tools_list_result(&message.payload).await;
    }

    async fn handle_connect(&mut self, _hello_message: &HelloMessage) {
        let tx = self.output_tx.clone().unwrap();
        let audio_config = config::get().audio();
        let data = HelloMessage {
            message: service::chobits::message::Message {
                mtype: service::chobits::message::Type::Hello,
            },
            transport: Some(Transport::Websocket),
            audio_params: Some(AudioParam {
                format: AudioFormat::Opus,
                sample_rate: audio_config.output_sample_rate(),
                channels: audio_config.output_channel(),
                frame_duration: audio_config.output_frame_duration(),
            }),
            version: None,
            features: None,
            session_id: Some(self.id.clone()),
        };
        let result = tx.send(Ok(FrameResult::HelloResult(data))).await;
        if result.is_err() {
            info!("tx send hello result failure");
        }
    }

    pub fn is_speech_clear(&self, prob: f32) -> bool {
        prob >= 0.8
    }
}
