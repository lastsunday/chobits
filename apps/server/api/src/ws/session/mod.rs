use crate::config;
use crate::ws::frame::{Frame, FrameError, FrameResult};
use crate::ws::llm::llm_cache::LlmCache;
use crate::ws::llm::{Llm, LlmQwen};
use crate::ws::session::listener::Listener;
use crate::ws::tts::tts_cache::TtsCache;
use crate::ws::tts::{Tts, TtsKokoro};
use crate::ws::util::llm::{EMOJI_MAP, analyze_emotion};
use anyhow::Context;
use chrono::Local;
use core::result::Result;
use framework::id::gen_id;
use futures::{Stream, StreamExt};
use service::chobits::message::audio::AudioMessage;
use service::chobits::message::hello::{AudioParam, HelloMessage};
use service::chobits::message::listen::ListenState;
use service::chobits::message::llm::LlmMessage;
use service::chobits::message::stt::SttMessage;
use service::chobits::message::tts::{TtsMessage, TtsState};
use service::chobits::message::{AudioFormat, Transport};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::mpsc::{Sender, channel};
use tokio::sync::{Mutex, Notify};
use tokio::time::{Duration, sleep};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info, instrument};

pub mod listener;

pub struct Session<L> {
    pub id: String,
    pub current_round: Option<Box<Round>>,
    output_tx: Option<Sender<Result<FrameResult, FrameError>>>,
    listener: Box<L>,
    phase: Phase,
    latest_activity_time: Arc<Mutex<Option<i64>>>,
    close_connection_no_voice_time: Option<i64>,
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
        tx.send(Ok(FrameResult::CloseResult)).await;
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
        )));
        if let Some(round) = &mut self.current_round {
            round.start().await;
        } else {
            panic!("current round is none");
        }
    }

    pub async fn accept_frame(&mut self, frame: Frame) {
        let phase = self.phase.clone();
        // info!("current phase = {:?}", phase.clone());
        match phase {
            Phase::Hello => match frame {
                Frame::Hello(hello_message) => {
                    self.handle_connect(hello_message).await;
                    self.phase = Phase::ListenDetect;
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
                                                        if mode == service::chobits::message::listen::ListenMode::Manual {
                                                            is_text_message = true;
                                                        }
                                                    }
                                                    if is_text_message {
                                                        // text message handle
                                                        round
                                                            .accept_command(Command::Chat(text))
                                                            .await;
                                                    } else {
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
                            if let Some(latest_activity_time) = latest_activity_time {
                                if let Some(close_connection_no_voice_time) =
                                    self.close_connection_no_voice_time
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
                            Some(round) => {
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
                            if let Some(latest_activity_time) = latest_activity_time {
                                if let Some(close_connection_no_voice_time) =
                                    self.close_connection_no_voice_time
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
                        outer_tx.send(frame_result).await;
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

    async fn handle_connect(&mut self, hello_message: HelloMessage) {
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
        tx.send(Ok(FrameResult::HelloResult(data))).await;
    }

    pub fn is_speech_clear(&self, prob: f32) -> bool {
        prob >= 0.8
    }
}

pub struct Round {
    pub parent_id: String,
    pub id: String,
    tx: Sender<Result<FrameResult, FrameError>>,
    stop: Arc<AtomicBool>,
    llm: Arc<Mutex<Box<LlmQwen>>>,
    tts: Arc<Mutex<Box<TtsKokoro>>>,
    pub tts_state: Arc<Mutex<Option<TtsState>>>,
    pub speaking: Arc<AtomicBool>,
    pub end: Arc<AtomicBool>,
}

#[derive(Debug)]
pub enum Command {
    Chat(String),
    Wake(String),
    ListenUnclear(String),
}

impl Round {
    pub fn new(
        parent_id: String,
        tx: Sender<Result<FrameResult, FrameError>>,
        llm: Arc<Mutex<Box<LlmQwen>>>,
        tts: Arc<Mutex<Box<TtsKokoro>>>,
    ) -> Self {
        Self {
            parent_id,
            id: gen_id(),
            tx,
            stop: Arc::new(AtomicBool::new(false)),
            llm,
            tts,
            tts_state: Arc::new(Mutex::new(None)),
            speaking: Arc::new(AtomicBool::new(false)),
            end: Arc::new(AtomicBool::new(false)),
        }
    }

    #[instrument(skip(self), name="Round start",fields(id = %self.id,parent_id = %self.parent_id))]
    pub async fn start(&self) {
        info!("start");
    }

    pub async fn accept_command(&mut self, command: Command) {
        match command {
            Command::Chat(text) => {
                let system_prompt = config::get().logic().system_prompt().to_string();
                self.llm_tts_handle(text, system_prompt).await;
            }
            Command::Wake(text) => {
                let system_prompt = config::get().logic().system_wake_prompt().to_string();
                self.llm_tts_handle(text, system_prompt).await;
            }
            Command::ListenUnclear(text) => {
                let system_prompt = config::get()
                    .logic()
                    .system_listen_unclear_prompt()
                    .to_string();
                self.llm_tts_handle(text, system_prompt).await;
            }
        }
    }

    async fn llm_tts_handle(&mut self, text: String, system_prompt: String) {
        let tx = self.tx.clone();
        let stop_me = self.stop.clone();
        let session_id = self.parent_id.clone();
        let llm = self.llm.clone();
        let tts = self.tts.clone();
        let tts_state_clone = self.tts_state.clone();
        let speaking = self.speaking.clone();
        let end = self.end.clone();
        tokio::spawn(async move {
            if tx
                .send(Ok(FrameResult::STTResult(SttMessage::new(
                    Some(session_id.clone()),
                    Some(format!("{text}")),
                ))))
                .await
                .is_err()
            {
                info!("send stt result failure");
            }
            let llm = llm.lock().await;
            let tts = tts.lock().await;
            let llm_output = llm.chat(system_prompt, format!("{text}"));
            let mut tts_output = tts.output_stream(llm_output);
            let audio_config = config::get().audio();
            let delay = audio_config.output_frame_duration();
            let mut latest_time = Instant::now() + Duration::from_millis(delay);
            // pre buffer count
            let pre_buffer_frame_count: u64 = 6;
            let mut send_frame_count: u64 = 0;
            let speaking = speaking.clone();
            let stop_me = stop_me.clone();
            while let Some(result) = tts_output.next().await {
                match result {
                    Ok(result) => {
                        if stop_me.load(Ordering::Relaxed) {
                            break;
                        }
                        let text = result.text;
                        let emotion = analyze_emotion(&text);
                        let session_id = session_id.clone();
                        let tx = tx.clone();
                        let text = text.clone();
                        let audio_data = result.audio;
                        let tts_state_clone = tts_state_clone.clone();
                        let speaking = speaking.clone();
                        let stop_me_by_tts_packet = stop_me.clone();
                        let result = async move || -> Result<(), anyhow::Error> {
                            //llm
                            tx.send(Ok(FrameResult::LLMResult(LlmMessage::new(
                                Some(session_id.to_string()),
                                Some(emotion.to_string()),
                                Some(EMOJI_MAP.get(emotion).map_or(r#"😶"#, |v| v).to_string()),
                            ))))
                            .await
                            .context("send llm result failure")?;
                            //tts
                            tx.send(Ok(FrameResult::TTSResult(TtsMessage::new(
                                Some(session_id.to_string()),
                                Some(TtsState::Start),
                                None,
                            ))))
                            .await
                            .context("send stt result start failure")?;
                            let tts_state = tts_state_clone.clone();
                            let mut tts_state = tts_state.lock().await;
                            *tts_state = Some(TtsState::Start);
                            drop(tts_state);
                            tx.send(Ok(FrameResult::TTSResult(TtsMessage::new(
                                Some(session_id.to_string()),
                                Some(TtsState::SentenceStart),
                                Some(text.to_string()),
                            ))))
                            .await
                            .context("send stt result sentence start failure")?;
                            let tts_state = tts_state_clone.clone();
                            let mut tts_state = tts_state.lock().await;
                            *tts_state = Some(TtsState::SentenceStart);
                            drop(tts_state);
                            //audio
                            //real time send audio
                            let mut data = audio_data.into_iter();
                            speaking.store(true, Ordering::Relaxed);
                            info!("set speaking = true");
                            info!("send audio frame start");
                            while let Some(packet) = data.next() {
                                if stop_me_by_tts_packet.load(Ordering::Relaxed) {
                                    break;
                                }
                                let now = Instant::now();
                                let offset = (now - latest_time).as_millis() as u64;
                                let mut actual_delay: u64 = 0;
                                if offset < delay {
                                    actual_delay = delay - offset;
                                }
                                if send_frame_count >= pre_buffer_frame_count && actual_delay > 0 {
                                    sleep(Duration::from_millis(actual_delay)).await;
                                }
                                latest_time = Instant::now();
                                tx.send(Ok(FrameResult::AudioResult(AudioMessage::new(
                                    Some(session_id.to_string()),
                                    packet,
                                ))))
                                .await
                                .context("send audio result failure")?;
                                send_frame_count += 1;
                            }
                            info!("send audio frame end");
                            speaking.store(false, Ordering::Relaxed);
                            info!("set speaking = false");
                            tx.send(Ok(FrameResult::TTSResult(TtsMessage::new(
                                Some(session_id.to_string()),
                                Some(TtsState::SentenceEnd),
                                None,
                            ))))
                            .await
                            .context("send stt result sentence end failure")?;
                            let tts_state = tts_state_clone.clone();
                            let mut tts_state = tts_state.lock().await;
                            *tts_state = Some(TtsState::SentenceEnd);
                            drop(tts_state);
                            tx.send(Ok(FrameResult::TTSResult(TtsMessage::new(
                                Some(session_id.to_string()),
                                Some(TtsState::Stop),
                                None,
                            ))))
                            .await
                            .context("send stt result start failure")?;
                            let tts_state = tts_state_clone.clone();
                            let mut tts_state = tts_state.lock().await;
                            *tts_state = Some(TtsState::Stop);
                            drop(tts_state);
                            Ok(())
                        }()
                        .await;
                        if let Err(e) = result {
                            error!("{:?}", e);
                            stop_me.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        stop_me.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
            if stop_me.load(Ordering::Relaxed) {
                let tts_state = tts_state_clone.lock().await;
                match tts_state.as_ref() {
                    Some(tts_state) => {
                        let result = async move || -> Result<(), anyhow::Error> {
                            info!("{:?}", tts_state);
                            if tts_state > &TtsState::Start {
                                tx.send(Ok(FrameResult::TTSResult(TtsMessage::new(
                                    Some(session_id.to_string()),
                                    Some(TtsState::SentenceStart),
                                    None,
                                ))))
                                .await
                                .context("send stt result sentence start failure")?;
                            }
                            if tts_state > &TtsState::SentenceStart {
                                tx.send(Ok(FrameResult::TTSResult(TtsMessage::new(
                                    Some(session_id.to_string()),
                                    Some(TtsState::SentenceEnd),
                                    None,
                                ))))
                                .await
                                .context("send stt result sentence end failure")?;
                            }
                            if tts_state > &TtsState::SentenceEnd {
                                tx.send(Ok(FrameResult::TTSResult(TtsMessage::new(
                                    Some(session_id.to_string()),
                                    Some(TtsState::Stop),
                                    None,
                                ))))
                                .await
                                .context("send stt result stop failure")?;
                            }
                            Ok(())
                        }()
                        .await;
                        if let Err(e) = result {
                            error!("{:?}", e)
                        }
                    }
                    None => {}
                }
            }
            end.store(true, Ordering::Relaxed);
            info!("round setting end = true");
        });
    }

    #[instrument(skip(self), name="Round stop",fields(id = %self.id,parent_id = %self.parent_id))]
    pub async fn stop(&self) {
        info!("stop");
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use std::cmp;

    use crate::ws::{
        asr::asr_cache::AsrCache, llm::llm_cache::LlmCache, session::listener::DefaultListener,
        tts::tts_cache::TtsCache, util::audio::pcm_decode, vad::vad_cache::VadCache,
    };

    use super::*;

    use axum::body::Bytes;
    use service::chobits::message::{
        hello::HelloMessage,
        listen::{ListenMessage, ListenMode},
    };
    use tokio_stream::StreamExt;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// hello paramter input and output the hello result
    /// cargo test --package api --lib -- ws::session::tests::test_chat_flow_hello --ignored --show-output
    async fn test_chat_flow_hello() {
        let mut session = create_session().await;
        session.start().await;
        let mut output = session.output_frame().await;
        let join_handle = tokio::spawn(async move {
            while let Some(data) = output.next().await {
                info!("{:?}", data);
                match data {
                    Ok(frame_result) => match frame_result {
                        FrameResult::HelloResult(_hello_message) => {
                            return;
                        }
                        _ => {
                            panic!("unexpected frame result");
                        }
                    },
                    Err(_) => {
                        break;
                    }
                }
            }
            panic!("receive hello message error");
        });
        let hello_frame = Frame::Hello(HelloMessage {
            ..Default::default()
        });
        session.accept_frame(hello_frame.clone()).await;
        session.stop().await;
        join_handle.await.unwrap();
    }

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// listen voice by manual mode and output the asr text result
    /// cargo test --features cuda --package api --lib -- ws::session::tests::test_chat_flow_listen_manual --ignored --show-output
    async fn test_chat_flow_listen_manual() {
        use std::path::PathBuf;

        let wav_file: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "resources",
            "test",
            "samples_jfk.wav",
        ]
        .iter()
        .collect();
        info!("{}", wav_file.display());
        let (pcm_data, sample_rate) = pcm_decode(wav_file).unwrap();
        info!(
            "pcm_data len = {},sample_rate = {}",
            pcm_data.len(),
            sample_rate
        );

        /// the follow code is output wav file to test
        // use wavers::{AudioSample, ConvertSlice, ConvertTo, Samples, read, write};
        // let fp = "./decode_pcm_data.wav";
        // let sr: i32 = 16000;
        // write(fp, &pcm_data, sr, 1);

        const ENCODE_SAMPLE_RATE: u32 = 16000;
        let mut encoder = opus::Encoder::new(
            ENCODE_SAMPLE_RATE,
            opus::Channels::Mono,
            opus::Application::Audio,
        )
        .unwrap();

        // 16000Hz * 1 channel * 60 ms / 1000 = 960
        const MONO_60MS: usize = ENCODE_SAMPLE_RATE as usize * 60 / 1000;
        let size = MONO_60MS;
        info!("size = {}", size);
        let len = pcm_data.len();
        let mut count = len / size;
        if len % size > 0 {
            count = count + 1;
        }
        info!("count = {}", count);
        let mut audio: Vec<Vec<u8>> = Vec::new();

        for n in 0..count {
            let start = n * size;
            let end = cmp::min((n + 1) * size, len);
            let packet = encoder
                .encode_vec_float(&pcm_data[start..end], size)
                .unwrap();
            audio.push(packet);
        }

        let mut session = create_session().await;
        let session_id = session.id.clone();
        session.start().await;
        let mut output = session.output_frame().await;
        let join_handle = tokio::spawn(async move {
            while let Some(data) = output.next().await {
                info!("session id = {}, data = {:?}", session_id, data);
                match data {
                    Ok(frame_result) => match frame_result {
                        FrameResult::HelloResult(_hello_message) => {}
                        FrameResult::STTResult(_stt_message) => {}
                        FrameResult::LLMResult(_llm_message) => {}
                        FrameResult::TTSResult(tts_message) => {
                            let state = tts_message.state;
                            if let Some(state) = state {
                                if TtsState::Stop == state {
                                    return;
                                }
                            }
                        }
                        FrameResult::AudioResult(audio_message) => {}
                        _ => {
                            panic!("unexpected frame result");
                        }
                    },
                    Err(_) => {
                        break;
                    }
                }
            }
            panic!("receive hello message error");
        });
        let hello_frame = Frame::Hello(HelloMessage {
            ..Default::default()
        });
        session.accept_frame(hello_frame.clone()).await;
        session
            .accept_frame(Frame::Listen(ListenMessage {
                state: ListenState::Start,
                mmod: Some(service::chobits::message::listen::ListenMode::Manual),
                ..Default::default()
            }))
            .await;
        for n in 0..audio.len() {
            session
                .accept_frame(Frame::Voice(Bytes::copy_from_slice(&audio.get(n).unwrap())))
                .await;
        }
        session
            .accept_frame(Frame::Listen(ListenMessage {
                state: ListenState::Stop,
                ..Default::default()
            }))
            .await;
        join_handle.await.unwrap();
        session.stop().await;
    }

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// listen voice by auto mode and output the asr text result
    /// cargo test --features cuda --package api --lib -- ws::session::tests::test_chat_flow_listen_auto --ignored --show-output
    async fn test_chat_flow_listen_auto() {
        use std::path::PathBuf;

        let wav_file: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "resources",
            "test",
            "samples_jfk.wav",
        ]
        .iter()
        .collect();
        info!("{}", wav_file.display());
        let (pcm_data, sample_rate) = pcm_decode(wav_file).unwrap();
        info!(
            "pcm_data len = {},sample_rate = {}",
            pcm_data.len(),
            sample_rate
        );

        /// the follow code is output wav file to test
        // use wavers::{AudioSample, ConvertSlice, ConvertTo, Samples, read, write};
        // let fp = "./decode_pcm_data.wav";
        // let sr: i32 = 16000;
        // write(fp, &pcm_data, sr, 1);

        const ENCODE_SAMPLE_RATE: u32 = 16000;
        let mut encoder = opus::Encoder::new(
            ENCODE_SAMPLE_RATE,
            opus::Channels::Mono,
            opus::Application::Audio,
        )
        .unwrap();

        // 16000Hz * 1 channel * 60 ms / 1000 = 960
        const MONO_60MS: usize = ENCODE_SAMPLE_RATE as usize * 60 / 1000;
        let size = MONO_60MS;
        info!("size = {}", size);
        let len = pcm_data.len();
        let mut count = len / size;
        if len % size > 0 {
            count = count + 1;
        }
        info!("count = {}", count);
        let mut audio: Vec<Vec<u8>> = Vec::new();

        for n in 0..count {
            let start = n * size;
            let end = cmp::min((n + 1) * size, len);
            let packet = encoder
                .encode_vec_float(&pcm_data[start..end], size)
                .unwrap();
            audio.push(packet);
        }

        let mut session = create_session().await;
        let session_id = session.id.clone();
        session.start().await;
        let mut output = session.output_frame().await;
        let next_step = Arc::new(AtomicBool::new(false));
        let next_step_for_sender = next_step.clone();
        let join_handle = tokio::spawn(async move {
            let mut count = 0;
            while let Some(data) = output.next().await {
                info!("session id = {}, data = {:?}", session_id, data);
                match data {
                    Ok(frame_result) => match frame_result {
                        FrameResult::HelloResult(_hello_message) => {}
                        FrameResult::STTResult(_stt_message) => {}
                        FrameResult::LLMResult(_llm_message) => {}
                        FrameResult::TTSResult(tts_message) => {
                            let state = tts_message.state;
                            if let Some(state) = state {
                                if TtsState::Stop == state {
                                    count = count + 1;
                                    next_step.store(true, Ordering::Relaxed);
                                    //when next round tts stop after wake tts round
                                    if count >= 2 {
                                        return;
                                    }
                                }
                            }
                        }
                        FrameResult::AudioResult(audio_message) => {}
                        _ => {
                            panic!("unexpected frame result");
                        }
                    },
                    Err(_) => {
                        break;
                    }
                }
            }
            panic!("receive hello message error");
        });
        session
            .accept_frame(Frame::Hello(HelloMessage {
                ..Default::default()
            }))
            .await;
        for n in 0..audio.len() {
            session
                .accept_frame(Frame::Voice(Bytes::copy_from_slice(&audio.get(n).unwrap())))
                .await;
        }
        session
            .accept_frame(Frame::Listen(ListenMessage {
                state: ListenState::Detect,
                mmod: None,
                text: Some(String::from("Hello")),
                ..Default::default()
            }))
            .await;
        session
            .accept_frame(Frame::Listen(ListenMessage {
                state: ListenState::Start,
                mmod: Some(ListenMode::Auto),
                ..Default::default()
            }))
            .await;
        let mut to_next_step = false;
        info!("before next step");
        while !to_next_step {
            to_next_step = next_step_for_sender.load(Ordering::Relaxed);
            sleep(Duration::from_millis(500)).await;
        }
        info!("after next step");
        for n in 0..audio.len() {
            session
                .accept_frame(Frame::Voice(Bytes::copy_from_slice(&audio.get(n).unwrap())))
                .await;
        }
        join_handle.await.unwrap();
        session.stop().await;
    }

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// listen voice by realtime mode and output the asr text result
    /// cargo test --features cuda --package api --lib -- ws::session::tests::test_chat_flow_listen_realtime --ignored --show-output
    async fn test_chat_flow_listen_realtime() {
        use std::path::PathBuf;

        let wav_file: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "resources",
            "test",
            "samples_jfk.wav",
        ]
        .iter()
        .collect();
        info!("{}", wav_file.display());
        let (pcm_data, sample_rate) = pcm_decode(wav_file).unwrap();
        info!(
            "pcm_data len = {},sample_rate = {}",
            pcm_data.len(),
            sample_rate
        );

        /// the follow code is output wav file to test
        // use wavers::{AudioSample, ConvertSlice, ConvertTo, Samples, read, write};
        // let fp = "./decode_pcm_data.wav";
        // let sr: i32 = 16000;
        // write(fp, &pcm_data, sr, 1);

        const ENCODE_SAMPLE_RATE: u32 = 16000;
        let mut encoder = opus::Encoder::new(
            ENCODE_SAMPLE_RATE,
            opus::Channels::Mono,
            opus::Application::Audio,
        )
        .unwrap();

        // 16000Hz * 1 channel * 60 ms / 1000 = 960
        const MONO_60MS: usize = ENCODE_SAMPLE_RATE as usize * 60 / 1000;
        let size = MONO_60MS;
        info!("size = {}", size);
        let len = pcm_data.len();
        let mut count = len / size;
        if len % size > 0 {
            count = count + 1;
        }
        info!("count = {}", count);
        let mut audio: Vec<Vec<u8>> = Vec::new();

        for n in 0..count {
            let start = n * size;
            let end = cmp::min((n + 1) * size, len);
            let packet = encoder
                .encode_vec_float(&pcm_data[start..end], size)
                .unwrap();
            audio.push(packet);
        }

        let mut session = create_session().await;
        let session_id = session.id.clone();
        session.start().await;
        let mut output = session.output_frame().await;
        let next_step = Arc::new(AtomicBool::new(false));
        let next_step_for_sender = next_step.clone();
        let join_handle = tokio::spawn(async move {
            let mut count = 0;
            while let Some(data) = output.next().await {
                info!("session id = {}, data = {:?}", session_id, data);
                match data {
                    Ok(frame_result) => match frame_result {
                        FrameResult::HelloResult(_hello_message) => {}
                        FrameResult::STTResult(_stt_message) => {}
                        FrameResult::LLMResult(_llm_message) => {}
                        FrameResult::TTSResult(tts_message) => {
                            let state = tts_message.state;
                            if let Some(state) = state {
                                if TtsState::Stop == state {
                                    count = count + 1;
                                    //when next round tts stop after wake tts round
                                    if count >= 2 {
                                        return;
                                    }
                                }
                            }
                        }
                        FrameResult::AudioResult(audio_message) => {}
                        _ => {
                            panic!("unexpected frame result");
                        }
                    },
                    Err(_) => {
                        break;
                    }
                }
            }
            panic!("receive hello message error");
        });
        session
            .accept_frame(Frame::Hello(HelloMessage {
                ..Default::default()
            }))
            .await;
        for n in 0..audio.len() {
            session
                .accept_frame(Frame::Voice(Bytes::copy_from_slice(&audio.get(n).unwrap())))
                .await;
        }
        session
            .accept_frame(Frame::Listen(ListenMessage {
                state: ListenState::Detect,
                mmod: None,
                text: Some(String::from("Hello")),
                ..Default::default()
            }))
            .await;
        session
            .accept_frame(Frame::Listen(ListenMessage {
                state: ListenState::Start,
                mmod: Some(ListenMode::RealTime),
                ..Default::default()
            }))
            .await;
        for n in 0..audio.len() {
            session
                .accept_frame(Frame::Voice(Bytes::copy_from_slice(&audio.get(n).unwrap())))
                .await;
        }
        join_handle.await.unwrap();
        session.stop().await;
    }

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// get text message and output the asr text result
    /// cargo test --features cuda --package api --lib -- ws::session::tests::test_chat_flow_handle_text_message --ignored --show-output
    async fn test_chat_flow_handle_text_message() {
        let mut session = create_session().await;
        let session_id = session.id.clone();
        session.start().await;
        let mut output = session.output_frame().await;
        let join_handle = tokio::spawn(async move {
            while let Some(data) = output.next().await {
                info!("session id = {}, data = {:?}", session_id, data);
                match data {
                    Ok(frame_result) => match frame_result {
                        FrameResult::HelloResult(_hello_message) => {}
                        FrameResult::STTResult(_stt_message) => {}
                        FrameResult::LLMResult(_llm_message) => {}
                        FrameResult::TTSResult(tts_message) => {
                            let state = tts_message.state;
                            if let Some(state) = state {
                                if TtsState::Stop == state {
                                    return;
                                }
                            }
                        }
                        FrameResult::AudioResult(audio_message) => {}
                        _ => {
                            panic!("unexpected frame result");
                        }
                    },
                    Err(_) => {
                        break;
                    }
                }
            }
            panic!("receive hello message error");
        });
        session
            .accept_frame(Frame::Hello(HelloMessage {
                ..Default::default()
            }))
            .await;
        session
            .accept_frame(Frame::Listen(ListenMessage {
                state: ListenState::Detect,
                mmod: Some(service::chobits::message::listen::ListenMode::Manual),
                text: Some(String::from("Hello")),
                ..Default::default()
            }))
            .await;
        join_handle.await.unwrap();
        session.stop().await;
    }

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// when a round running and has a break event,the output stream will stop the original output
    /// cargo test --features cuda --package api --lib -- ws::session::tests::test_chat_flow_break --ignored --show-output
    async fn test_chat_flow_break() {
        let mut session = create_session().await;
        let session_id = session.id.clone();
        session.start().await;
        let mut output = session.output_frame().await;
        let mut count = 0;
        let join_handle = tokio::spawn(async move {
            while let Some(data) = output.next().await {
                info!("session id = {}, data = {:?}", session_id, data);
                match data {
                    Ok(frame_result) => match frame_result {
                        FrameResult::HelloResult(_hello_message) => {}
                        FrameResult::STTResult(_stt_message) => {}
                        FrameResult::LLMResult(_llm_message) => {}
                        FrameResult::TTSResult(tts_message) => {
                            let state = tts_message.state;
                            if let Some(state) = state {
                                if TtsState::Stop == state {
                                    count = count + 1;
                                    //when next round tts stop after wake tts round
                                    if count >= 2 {
                                        return;
                                    }
                                }
                            }
                        }
                        FrameResult::AudioResult(audio_message) => {}
                        _ => {
                            panic!("unexpected frame result");
                        }
                    },
                    Err(_) => {
                        break;
                    }
                }
            }
            panic!("receive hello message error");
        });
        session
            .accept_frame(Frame::Hello(HelloMessage {
                ..Default::default()
            }))
            .await;
        session
            .accept_frame(Frame::Listen(ListenMessage {
                state: ListenState::Detect,
                mmod: Some(service::chobits::message::listen::ListenMode::Manual),
                text: Some(String::from("Hello")),
                ..Default::default()
            }))
            .await;
        session
            .accept_frame(Frame::Listen(ListenMessage {
                state: ListenState::Detect,
                mmod: Some(service::chobits::message::listen::ListenMode::Manual),
                text: Some(String::from("Hello")),
                ..Default::default()
            }))
            .await;
        join_handle.await.unwrap();
        session.stop().await;
    }

    async fn create_session() -> Session<impl Listener> {
        info!("init vad cahce");
        VadCache::init().await;
        info!("init vad cahce successfully");
        info!("init asr cahce");
        AsrCache::init().await;
        info!("init asr cahce successfully");
        tracing::info!("init llm cahce");
        LlmCache::init().await;
        tracing::info!("init llm cahce successfully");
        tracing::info!("init tts cahce");
        TtsCache::init().await;
        tracing::info!("init tts cahce successfully");
        let vad = VadCache::create_vad();
        let vad = Arc::new(Mutex::new(vad));
        let asr = AsrCache::global().instance.clone();
        let asr = Arc::new(Mutex::new(asr));
        let close_connection_no_voice_time = config::get().logic().close_connection_no_voice_time();
        Session::new(
            Box::new(DefaultListener::new(vad, asr.clone())),
            Some(close_connection_no_voice_time),
        )
    }
}
