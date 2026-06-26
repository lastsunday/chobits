use super::frame::{Frame, FrameResult};
use crate::asr::Asr;
use crate::common::ModelError;
use crate::config::audio::AudioConfig;
use crate::config::session::SessionConfig;
use crate::llm::Model;
use crate::llm::client::{ChatRequest, Client, ClientBuilder, History};
use crate::mcp::client::device::{DeviceMcpClient, DeviceMcpPhase};
use crate::mcp::mcp_host::{McpHost, UnionMcpHost};
use crate::record::collector::RecordCollector;
use crate::record::observer::{
    AsrContext, FrameContext, FrameDirection, LlmDeltaContext, RoundEndContext, RoundEndReason,
    RoundMode, RoundStartContext, TextInputContext, TtsDeltaContext,
};
use crate::tts::Tts;
use crate::util::llm::{EMOJI_MAP, analyze_emotion};
use crate::vad::Vad;
use crate::ws::WsErrorCode;
use framework::err;
use framework::error::AppError;
use futures::{Stream, StreamExt};
use opus;
use rig::OneOrMany;
use rig::message::ToolResult;
use rig::message::{Message, Text, UserContent};
use service::chobits::message::audio::AudioMessage;
use service::chobits::message::hello::{AudioParam, HelloMessage};
use service::chobits::message::listen::{ListenMessage, ListenState};
use service::chobits::message::llm::LlmMessage;
use service::chobits::message::stt::SttMessage;
use service::chobits::message::tts::{TtsMessage, TtsState};
use service::chobits::message::{AudioFormat, Transport};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::SystemTime;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{Sender, UnboundedReceiver, UnboundedSender, channel, unbounded_channel};
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant, MissedTickBehavior, interval_at};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, Level, error, info, span};

////////////////////////////////////////////////////////////////////////////////
// OutputMessage + Command — used by all sub-components
////////////////////////////////////////////////////////////////////////////////

pub struct OutputMessage {
    pub epoch: u64,
    pub payload: Result<FrameResult, AppError>,
}

#[derive(Debug)]
pub enum Command<'a> {
    Chat { text: &'a str },
    AsrChat { text: &'a str },
    Wake { text: &'a str },
    ListenUnclear { text: &'a str },
}

////////////////////////////////////////////////////////////////////////////////
// OutputController — pacing layer
////////////////////////////////////////////////////////////////////////////////

pub struct OutputController {
    input_rx: UnboundedReceiver<OutputMessage>,
    output_tx: Sender<OutputMessage>,
    epoch: Arc<AtomicU64>,
    latest_activity_time: Arc<AtomicI64>,
    interval: Option<tokio::time::Interval>,
    frame_duration: u64,
}

impl OutputController {
    pub fn new(
        input_rx: UnboundedReceiver<OutputMessage>,
        output_tx: Sender<OutputMessage>,
        epoch: Arc<AtomicU64>,
        latest_activity_time: Arc<AtomicI64>,
        frame_duration: u64,
    ) -> Self {
        Self {
            input_rx,
            output_tx,
            epoch,
            latest_activity_time,
            interval: None,
            frame_duration,
        }
    }

    pub fn spawn(mut self) {
        tokio::spawn(async move {
            self.run().await;
        });
    }

    async fn run(&mut self) {
        while let Some(msg) = self.input_rx.recv().await {
            let current_epoch = self.epoch.load(Ordering::Acquire);
            if msg.epoch != 0 && msg.epoch < current_epoch {
                continue;
            }
            if let Ok(FrameResult::TTSResult(ref t)) = msg.payload
                && t.state == Some(TtsState::Start)
            {
                self.interval = None;
            }
            if let Ok(FrameResult::AudioResult(_)) = msg.payload {
                self.pace_audio().await;
            }
            self.latest_activity_time
                .store(now_millis(), Ordering::Release);
            if self.output_tx.send(msg).await.is_err() {
                break;
            }
        }
    }

    async fn pace_audio(&mut self) {
        if let Some(interval) = &mut self.interval {
            interval.tick().await;
        } else {
            let start = Instant::now() + Duration::from_millis(self.frame_duration);
            let mut intv = interval_at(start, Duration::from_millis(self.frame_duration));
            intv.set_missed_tick_behavior(MissedTickBehavior::Skip);
            self.interval = Some(intv);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// DefaultListener — VAD + decode + prefix buffering
////////////////////////////////////////////////////////////////////////////////

const PREFIX_SAMPLES_MAX: usize = 4800;

#[derive(Debug, Clone)]
pub enum ListenInput {
    Text(String),
    Audio(Vec<u8>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListenerState {
    Idle,
    /// is_speech
    Listening(bool),
    End,
}

#[derive(Debug, Clone)]
pub enum ListenResult {
    Text(String),
    Audio { text: String, prob: f32 },
}

pub struct DefaultListener {
    temp_voice_data: Vec<f32>,
    voice_data: Vec<f32>,
    vad: Box<dyn Vad>,
    asr: Arc<Mutex<Box<dyn Asr>>>,
    decoder: StdMutex<opus::Decoder>,
    pub state: ListenerState,
    silence_voice_timeout: Option<i64>,
    latest_speaking_time: Option<i64>,
    audio_config: Arc<AudioConfig>,
    error_tx: Option<UnboundedSender<OutputMessage>>,
    prefix_buffer: Vec<f32>,
    prefix_flushed: bool,
    pending_text: Option<String>,
    frame_size: usize,
}

impl DefaultListener {
    pub fn new(
        vad: Box<dyn Vad>,
        asr: Arc<Mutex<Box<dyn Asr>>>,
        audio_config: Arc<AudioConfig>,
    ) -> Self {
        let sample_rate = audio_config
            .input_sample_rate
            .expect("input sample rate is empty");
        let frame_size = {
            let channel = audio_config.input_channel.expect("input channel is empty");
            let frame_duration = audio_config
                .input_frame_duration
                .expect("input frame duration is empty");
            ((sample_rate as u64 * channel as u64 * frame_duration) / 1000) as usize
        };
        Self {
            vad,
            asr,
            temp_voice_data: Vec::new(),
            voice_data: Vec::new(),
            decoder: StdMutex::new(opus::Decoder::new(sample_rate, opus::Channels::Mono).unwrap()),
            state: ListenerState::Idle,
            silence_voice_timeout: None,
            latest_speaking_time: None,
            audio_config,
            error_tx: None,
            prefix_buffer: Vec::with_capacity(PREFIX_SAMPLES_MAX),
            prefix_flushed: false,
            pending_text: None,
            frame_size,
        }
    }
}

impl DefaultListener {
    pub async fn accept(&mut self, input: ListenInput) {
        match input {
            ListenInput::Text(text) => {
                self.pending_text = Some(text);
            }
            ListenInput::Audio(data) => {
                if self.state == ListenerState::Idle {
                    self.state = ListenerState::Listening(false);
                }
                if let ListenerState::Listening(_) = self.state {
                    let frame_size = self.frame_size;
                    let mut samples = vec![0f32; frame_size];
                    let len =
                        match self
                            .decoder
                            .lock()
                            .unwrap()
                            .decode_float(&data, &mut samples, false)
                        {
                            Ok(len) => len,
                            Err(e) => {
                                tracing::error!(
                                    "Opus decode error: {e}, data_len={}, first_bytes={:02x?}",
                                    data.len(),
                                    &data[..data.len().min(8)]
                                );
                                return;
                            }
                        };
                    for s in samples[..len].iter_mut() {
                        *s = s.clamp(-1.0, 1.0);
                    }
                    self.temp_voice_data.append(&mut samples[..len].to_vec());
                    let window_size = self.vad.window_size();
                    while self.temp_voice_data.len() > window_size {
                        let window: Vec<f32> = self.temp_voice_data.drain(..window_size).collect();

                        self.prefix_buffer.extend(&window);
                        if self.prefix_buffer.len() > PREFIX_SAMPLES_MAX {
                            let excess = self.prefix_buffer.len() - PREFIX_SAMPLES_MAX;
                            self.prefix_buffer.drain(..excess);
                        }

                        if let Err(e) = self.vad.accept_waveform(&window) {
                            tracing::error!("accept_waveform error = {}", e.to_string());
                            return;
                        }

                        if self.vad.is_speech() {
                            self.state = ListenerState::Listening(true);
                            self.latest_speaking_time = Some(now_millis());
                        } else {
                            self.prefix_flushed = false;
                        }

                        if self.state == ListenerState::Listening(true) {
                            if !self.prefix_flushed {
                                self.voice_data.append(&mut self.prefix_buffer);
                                self.prefix_buffer = Vec::with_capacity(PREFIX_SAMPLES_MAX);
                                self.prefix_flushed = true;
                            } else {
                                self.voice_data.extend_from_slice(&window);
                            }
                        }
                    }
                }
                if let (Some(silence_voice_timeout), Some(latest_speaking_time)) =
                    (self.silence_voice_timeout, self.latest_speaking_time)
                {
                    let offset_time = now_millis().saturating_sub(latest_speaking_time);
                    if offset_time >= silence_voice_timeout {
                        self.state = ListenerState::End;
                    }
                }
            }
        }
    }

    pub async fn take_voice(&mut self) -> Vec<f32> {
        core::mem::take(&mut self.voice_data)
    }

    pub async fn take_result(
        &mut self,
    ) -> (Vec<f32>, core::result::Result<ListenResult, ModelError>) {
        if let Some(text) = self.pending_text.take() {
            return (Vec::new(), Ok(ListenResult::Text(text)));
        }
        let voice_data = core::mem::take(&mut self.voice_data);
        if voice_data.is_empty() {
            return (
                voice_data,
                Ok(ListenResult::Audio {
                    text: String::new(),
                    prob: 1.0,
                }),
            );
        }
        let sample_rate: u32 = self
            .audio_config
            .input_sample_rate
            .expect("input sample rate is empty");
        let mut asr = self.asr.lock().await;
        let result = asr.transcribe(sample_rate, &voice_data).await;
        match result {
            Ok(transcript) => (
                voice_data,
                Ok(ListenResult::Audio {
                    text: transcript.text,
                    prob: transcript.prob,
                }),
            ),
            Err(e) => {
                tracing::error!("{:?}", e);
                if let Some(tx) = &self.error_tx {
                    push_frame(
                        tx,
                        0,
                        Err(err!(WsErrorCode::AsrFailure).with_extra(e.to_string())),
                    );
                }
                (voice_data, Err(e))
            }
        }
    }

    pub async fn reset(&mut self, silence_voice_timeout: Option<i64>) {
        self.state = ListenerState::Idle;
        self.silence_voice_timeout = silence_voice_timeout;
        self.latest_speaking_time = None;
        self.temp_voice_data.clear();
        self.voice_data.clear();
        self.vad.clear();
        self.prefix_buffer.clear();
        self.prefix_flushed = false;
        self.pending_text = None;
    }
}

////////////////////////////////////////////////////////////////////////////////
// Round — TTS + LLM pipeline
////////////////////////////////////////////////////////////////////////////////

fn push_frame(
    tx: &UnboundedSender<OutputMessage>,
    epoch: u64,
    payload: Result<FrameResult, AppError>,
) -> bool {
    tx.send(OutputMessage { epoch, payload }).is_ok()
}

async fn send_tts_frame_and_change_state(
    tts_state: Arc<Mutex<Option<TtsState>>>,
    tx: &UnboundedSender<OutputMessage>,
    epoch: u64,
    session_id: &str,
    state: TtsState,
    text: Option<String>,
) {
    let mut tts_state = tts_state.lock().await;
    *tts_state = Some(state.clone());
    push_frame(
        tx,
        epoch,
        Ok(FrameResult::TTSResult(TtsMessage::new(
            Some(session_id.to_string()),
            Some(state),
            text,
        ))),
    );
}

struct SendCtx {
    tx: UnboundedSender<OutputMessage>,
    epoch: u64,
    session_id: String,
    tts_state: Arc<Mutex<Option<TtsState>>>,
    cancel: CancellationToken,
}

async fn send_llm_audio(
    ctx: &SendCtx,
    emotion: &str,
    text: &str,
    audio_data: Option<Vec<Vec<u8>>>,
) {
    if !push_frame(
        &ctx.tx,
        ctx.epoch,
        Ok(FrameResult::LLMResult(LlmMessage::new(
            Some(ctx.session_id.clone()),
            Some(emotion.to_string()),
            Some(EMOJI_MAP.get(emotion).map_or(r#"😶"#, |v| v).to_string()),
        ))),
    ) {
        return;
    }
    send_tts_frame_and_change_state(
        ctx.tts_state.clone(),
        &ctx.tx,
        ctx.epoch,
        &ctx.session_id,
        TtsState::SentenceStart,
        Some(text.to_string()),
    )
    .await;
    for packet in audio_data.unwrap_or_default() {
        if ctx.cancel.is_cancelled() {
            break;
        }
        if !push_frame(
            &ctx.tx,
            ctx.epoch,
            Ok(FrameResult::AudioResult(AudioMessage::new(
                Some(ctx.session_id.clone()),
                packet,
            ))),
        ) {
            info!(target:"round","send audio failure");
            break;
        }
    }
    send_tts_frame_and_change_state(
        ctx.tts_state.clone(),
        &ctx.tx,
        ctx.epoch,
        &ctx.session_id,
        TtsState::SentenceEnd,
        None,
    )
    .await;
}

pub struct RoundConfig {
    pub tx: UnboundedSender<OutputMessage>,
    pub epoch: u64,
    pub client: Arc<Client>,
    pub tts: Arc<Box<dyn Tts>>,
    pub recorder: Option<Arc<RecordCollector>>,
    pub cancel: CancellationToken,
}

pub struct Round {
    pub parent_id: String,
    pub id: String,
    tx: UnboundedSender<OutputMessage>,
    epoch: u64,
    client: Arc<Client>,
    tts: Arc<Box<dyn Tts>>,
    pub tts_state: Arc<Mutex<Option<TtsState>>>,
    pub cancel: CancellationToken,
    pub join_handle: Option<JoinHandle<()>>,
    pub recorder: Option<Arc<RecordCollector>>,
}

impl Round {
    pub fn new(parent_id: String, id: String, config: RoundConfig) -> Self {
        Self {
            parent_id,
            id,
            tx: config.tx,
            epoch: config.epoch,
            client: config.client,
            tts: config.tts,
            tts_state: Arc::new(Mutex::new(None)),
            cancel: config.cancel,
            join_handle: None,
            recorder: config.recorder,
        }
    }

    pub async fn accept_command<'a>(&mut self, command: Command<'a>) {
        match command {
            Command::Chat { text } => {
                if let Some(ref recorder) = self.recorder {
                    recorder.on_text_input(&TextInputContext {
                        round_id: self.id.clone(),
                        text: text.to_string(),
                    });
                }
                self.llm_tts_handle(text).await
            }
            Command::AsrChat { text }
            | Command::Wake { text }
            | Command::ListenUnclear { text } => self.llm_tts_handle(text).await,
        }
    }

    async fn llm_tts_handle(&mut self, text: &str) {
        let client = self.client.clone();
        let tts = self.tts.clone();
        let text = String::from(text);
        let recorder = self.recorder.clone();
        let round_id = self.id.clone();
        let span = span!(parent:None,Level::DEBUG, "socket", id=%self.parent_id);
        let ctx = SendCtx {
            tx: self.tx.clone(),
            epoch: self.epoch,
            session_id: self.parent_id.clone(),
            tts_state: self.tts_state.clone(),
            cancel: self.cancel.clone(),
        };
        self.join_handle = Some(tokio::spawn(
            async move {
                let stt_text = text.clone();
                if !push_frame(
                    &ctx.tx,
                    ctx.epoch,
                    Ok(FrameResult::STTResult(SttMessage::new(
                        Some(ctx.session_id.clone()),
                        Some(stt_text),
                    ))),
                ) {
                    info!(target:"round","send stt result failure");
                    return;
                }
                let request = ChatRequest {
                    message: Message::User {
                        content: OneOrMany::one(UserContent::Text(Text { text: text.clone() })),
                    },
                };
                let llm_output = client.chat(request, ctx.cancel.clone());
                let mut tts_output = tts.stream(Box::pin(llm_output), ctx.cancel.clone()).await;
                send_tts_frame_and_change_state(
                    ctx.tts_state.clone(),
                    &ctx.tx,
                    ctx.epoch,
                    &ctx.session_id,
                    TtsState::Start,
                    None,
                )
                .await;
                while let Some(result) = tts_output.next().await {
                    match result {
                        Ok(result) => {
                            if ctx.cancel.is_cancelled() {
                                break;
                            }
                            let text = result.text;
                            if let Some(ref recorder) = recorder {
                                recorder.on_llm_delta(&LlmDeltaContext {
                                    round_id: round_id.clone(),
                                    text: text.clone(),
                                });
                                if let Some((pcm, sr)) = &result.raw_pcm {
                                    recorder.on_tts_delta(&TtsDeltaContext {
                                        round_id: round_id.clone(),
                                        text: text.clone(),
                                        raw_pcm: Some((pcm.clone(), *sr as u32)),
                                    });
                                }
                            }
                            let emotion = analyze_emotion(&text);
                            send_llm_audio(&ctx, emotion, &text, result.audio).await;
                        }
                        Err(e) => {
                            error!(target:"round","{:?}", e);
                            if !push_frame(
                                &ctx.tx,
                                ctx.epoch,
                                Err(err!(WsErrorCode::TtsEncode).with_extra(e.to_string())),
                            ) {
                                error!(target:"round","send error frame failure");
                            }
                            break;
                        }
                    }
                }
                send_tts_frame_and_change_state(
                    ctx.tts_state.clone(),
                    &ctx.tx,
                    ctx.epoch,
                    &ctx.session_id,
                    TtsState::Stop,
                    None,
                )
                .await;
                if let Some(ref recorder) = recorder {
                    let _ = recorder
                        .on_round_end(&RoundEndContext {
                            round_id: round_id.clone(),
                            reason: RoundEndReason::Completed,
                        })
                        .await;
                }
            }
            .instrument(span),
        ));
    }

    pub async fn stop(&self) {
        self.cancel.cancel();
    }
}

type OutputTx = Option<UnboundedSender<OutputMessage>>;

pub struct Session {
    pub id: String,
    pub current_round: Option<Box<Round>>,
    output_tx: OutputTx,
    seq: Arc<AtomicU64>,
    pub recorder: Option<Arc<RecordCollector>>,
    phase: Phase,
    current_mode: RoundMode,
    latest_activity_time: Arc<AtomicI64>,
    history: Arc<Mutex<History>>,
    output_epoch: Arc<AtomicU64>,
    session_epoch: Arc<AtomicU64>,

    config: Arc<SessionConfig>,
    audio_config: Arc<AudioConfig>,

    model: Arc<Box<dyn Model>>,
    tts: Arc<Box<dyn Tts>>,
    listener: DefaultListener,
    mcp_host: Arc<Mutex<UnionMcpHost>>,
    device_mcp_phase: DeviceMcpPhase,
    device_mcp_call_tool_result_tx: Option<Sender<anyhow::Result<ToolResult>>>,
}

#[derive(Debug, Clone)]
pub enum Phase {
    Hello,
    ListenDetect,
    Listen(ListenMode),
    Stop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ListenMode {
    // voice call
    Auto,
    // on button send voice
    Manual,
    // esp32
    RealTime,
}

pub struct SessionOptions {
    pub id: String,
    pub listener: DefaultListener,
    pub model: Arc<Box<dyn Model>>,
    pub tts: Arc<Box<dyn Tts>>,
    pub mcp_host: Arc<Mutex<UnionMcpHost>>,
    pub config: Arc<SessionConfig>,
    pub audio_config: Arc<AudioConfig>,
    pub recorder: Option<Arc<RecordCollector>>,
}

impl Session {
    pub fn new(opts: SessionOptions) -> Self {
        let system_prompt = opts
            .config
            .system_prompt
            .as_ref()
            .expect("logic system prompt is empty")
            .to_string();
        let SessionOptions {
            id,
            listener,
            model,
            tts,
            mcp_host,
            config,
            audio_config,
            recorder,
        } = opts;
        Session {
            id,
            current_round: None,
            output_tx: None,
            seq: Arc::new(AtomicU64::new(1)),
            recorder,
            phase: Phase::Hello,
            current_mode: RoundMode::Auto,
            latest_activity_time: Arc::new(AtomicI64::new(0)),
            history: Arc::new(Mutex::new(History {
                preamble: Some(system_prompt),
                chat_history: vec![],
            })),
            output_epoch: Arc::new(AtomicU64::new(1)),
            session_epoch: Arc::new(AtomicU64::new(1)),
            config,
            audio_config,
            listener,
            model,
            tts,
            mcp_host,
            device_mcp_phase: DeviceMcpPhase::Initialize,
            device_mcp_call_tool_result_tx: None,
        }
    }

    pub async fn stop(&mut self) {
        self.phase = Phase::Stop;
        self.stop_round().await;
        if let Some(ref recorder) = self.recorder {
            recorder.on_session_end(&self.id).await;
        }
        let tx = self.output_tx.clone().expect("output tx not exists");
        if !push_frame(&tx, 0, Ok(FrameResult::CloseResult)) {
            info!("tx send frame result close result failure");
        }
    }

    pub async fn new_round(&mut self, mode: RoundMode) {
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
        let epoch = self.output_epoch.load(Ordering::Acquire);
        if let Some(ref recorder) = self.recorder {
            recorder.on_round_start(&RoundStartContext {
                round_id: round_id.clone(),
                session_id: Some(self.id.clone()),
                client_info: None,
                mode,
            });
        }
        self.current_round = Some(Box::new(Round::new(
            self.id.clone(),
            round_id,
            RoundConfig {
                tx: tx.clone(),
                epoch,
                client: Arc::new(client),
                tts: self.tts.clone(),
                recorder: self.recorder.clone(),
                cancel: cancel_token,
            },
        )));
    }

    pub async fn stop_round(&mut self) {
        if let Some(round) = &mut self.current_round {
            let round_id = round.id.clone();
            round.stop().await;
            round.join_handle.take();
            if let Some(ref recorder) = self.recorder {
                let _ = recorder
                    .on_round_end(&RoundEndContext {
                        round_id: round_id.clone(),
                        reason: RoundEndReason::Interrupted,
                    })
                    .await;
            }
        }
    }

    pub async fn accept_frame<'a>(&mut self, frame: &Frame<'a>) {
        // Handle close/abort/ping/pong immediately (no recording needed)
        match frame {
            Frame::Close(_) => {
                self.session_epoch.fetch_add(1, Ordering::Release);
                self.stop().await;
                return;
            }
            Frame::Abort(_) => {
                self.session_epoch.fetch_add(1, Ordering::Release);
                self.new_round(self.current_mode).await;
                return;
            }
            Frame::Ping { .. } | Frame::Pong { .. } => return,
            _ => {}
        }

        // Handle MCP (no recording needed)
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

        // Capture round_id before dispatch (so Listen(Stop) belongs to the round it stops)
        let round_id = self.current_round.as_ref().map(|r| r.id.clone());

        // Dispatch to phase handler (may create new round via new_round)
        let phase = self.phase.clone();
        match phase {
            Phase::Hello => self.handle_phase_hello(frame).await,
            Phase::ListenDetect => self.handle_phase_listen_detect(frame).await,
            Phase::Listen(mode) => self.handle_phase_listen(&mode, frame).await,
            Phase::Stop => return,
        }

        // Determine final round_id:
        // - If we captured a round_id before dispatch (e.g., Listen(Stop)), keep it
        // - If dispatch created a new round, use that round's ID
        // - Otherwise, it's a session-level frame (round_id = None)
        let final_round_id = round_id.or_else(|| self.current_round.as_ref().map(|r| r.id.clone()));

        if self.current_mode == RoundMode::Manual {
            self.current_round = None;
        }

        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        if let Some(ref recorder) = self.recorder {
            recorder.on_frame(&FrameContext {
                round_id: final_round_id.clone(),
                session_id: Some(self.id.clone()),
                seq,
                direction: FrameDirection::Inbound,
                detail: format!("{}", frame),
            });
        }
    }

    async fn init_mcp_device(&mut self, input_tx: UnboundedSender<OutputMessage>) {
        let (device_mcp_call_tool_result_tx, device_mcp_call_tool_result_rx) =
            channel::<anyhow::Result<ToolResult>>(1);
        self.device_mcp_call_tool_result_tx = Some(device_mcp_call_tool_result_tx);
        let mcp_device_client = DeviceMcpClient::new(
            Some(self.id.clone()),
            input_tx,
            Arc::new(Mutex::new(device_mcp_call_tool_result_rx)),
        );
        let mcp_device_client = Arc::new(Mutex::new(mcp_device_client));
        let mcp_host = self.mcp_host.clone();
        let mut mcp_host = mcp_host.lock().await;
        mcp_host.set_device_client(mcp_device_client.clone()).await;
    }

    pub async fn output_frame(
        &mut self,
    ) -> (
        impl Stream<Item = OutputMessage> + Unpin + Send + 'static,
        Arc<AtomicU64>,
        Arc<AtomicI64>,
        u64,
        Arc<AtomicU64>,
    ) {
        // Unbounded input from Session (producer never blocks).
        // Bounded output to WebSocket (backpressure boundary).
        let (input_tx, input_rx) = unbounded_channel::<OutputMessage>();
        let (output_tx, output_rx) = channel::<OutputMessage>(64);

        let frame_duration = self
            .audio_config
            .output_frame_duration
            .expect("output frame duration is empty");

        self.init_mcp_device(input_tx.clone()).await;
        self.listener.error_tx = Some(input_tx.clone());
        self.output_tx = Some(input_tx.clone());

        let controller = OutputController::new(
            input_rx,
            output_tx,
            self.output_epoch.clone(),
            self.latest_activity_time.clone(),
            frame_duration,
        );
        controller.spawn();

        let epoch = self.output_epoch.clone();
        let activity_time = self.latest_activity_time.clone();
        let session_epoch = self.session_epoch.clone();
        (
            ReceiverStream::new(output_rx),
            epoch,
            activity_time,
            frame_duration,
            session_epoch,
        )
    }

    pub fn update_latest_activity_time(&self) {
        self.latest_activity_time
            .store(now_millis(), Ordering::Release);
    }

    pub fn get_latest_activity_time(&self) -> Option<i64> {
        let time = self.latest_activity_time.load(Ordering::Acquire);
        if time == 0 { None } else { Some(time) }
    }

    async fn check_activity_timeout(&mut self) {
        match self.listener.state {
            ListenerState::Listening(true) => self.update_latest_activity_time(),
            _ => {
                let Some(activity) = self.get_latest_activity_time() else {
                    return;
                };
                let Some(timeout) = self.config.close_connection_no_voice_time else {
                    return;
                };
                if now_millis().saturating_sub(activity) >= timeout {
                    info!(target:"session", "session stop: offset_time = {} >= close_connection_no_voice_time = {}", now_millis().saturating_sub(activity), timeout);
                    self.stop().await;
                }
            }
        }
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

impl Session {
    async fn handle_phase_hello<'a>(&mut self, frame: &Frame<'a>) {
        match frame {
            Frame::Hello(hello_message) => {
                let mut has_mcp = false;
                if let Some(features) = &hello_message.features
                    && let Some(mcp) = features.mcp
                {
                    has_mcp = mcp;
                }
                self.handle_connect(hello_message).await;
                self.phase = Phase::ListenDetect;
                if has_mcp {
                    let mut mcp_host = self.mcp_host.lock().await;
                    let device_mcp_client = mcp_host
                        .get_device_client()
                        .await
                        .clone()
                        .expect("device mcp not exists");
                    let mut device_mcp_client = device_mcp_client.lock().await;
                    device_mcp_client
                        .request_mcp_initialize(hello_message)
                        .await;
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

    async fn handle_listen_start(&mut self, msg: &ListenMessage<'_>) {
        let Some(mode) = &msg.mmod else {
            error!(
                "invalid frame in phase = {:?}, state = {:?}",
                self.phase,
                ListenState::Start
            );
            return;
        };
        match mode {
            service::chobits::message::listen::ListenMode::Auto => {
                self.interrupt_output().await;
                self.phase = Phase::Listen(ListenMode::Auto);
                self.current_mode = RoundMode::Auto;
            }
            service::chobits::message::listen::ListenMode::Manual => {
                self.interrupt_output().await;
                self.phase = Phase::Listen(ListenMode::Manual);
                self.current_mode = RoundMode::Manual;
                self.listener.reset(None).await;
            }
            service::chobits::message::listen::ListenMode::RealTime => {
                self.interrupt_output().await;
                self.phase = Phase::Listen(ListenMode::RealTime);
                self.current_mode = RoundMode::RealTime;
            }
        }
    }

    async fn handle_phase_listen_detect<'a>(&mut self, frame: &Frame<'a>) {
        match frame {
            Frame::Listen(msg) => match &msg.state {
                ListenState::Start => {
                    self.handle_listen_start(msg).await;
                    if self.current_mode == RoundMode::Auto
                        || self.current_mode == RoundMode::RealTime
                    {
                        self.handle_phase_listen_for_mode(
                            &match self.current_mode {
                                RoundMode::Auto => ListenMode::Auto,
                                _ => ListenMode::RealTime,
                            },
                            frame,
                        )
                        .await;
                    }
                }
                ListenState::Detect => {
                    let mode = &msg.mmod;
                    if let Some(mode) = mode {
                        match mode {
                            service::chobits::message::listen::ListenMode::Auto => {
                                self.phase = Phase::Listen(ListenMode::Auto);
                                self.current_mode = RoundMode::Auto;
                            }
                            service::chobits::message::listen::ListenMode::Manual => {
                                self.phase = Phase::Listen(ListenMode::Manual);
                                self.current_mode = RoundMode::Manual;
                            }
                            service::chobits::message::listen::ListenMode::RealTime => {
                                self.phase = Phase::Listen(ListenMode::RealTime);
                                self.current_mode = RoundMode::RealTime;
                            }
                        }
                    } else {
                        // eps32-c3 default listen mode is none
                        self.phase = Phase::Listen(ListenMode::RealTime);
                        self.current_mode = RoundMode::RealTime;
                    }
                    self.handle_phase_listen_for_mode(
                        &match self.current_mode {
                            RoundMode::Auto => ListenMode::Auto,
                            RoundMode::Manual => ListenMode::Manual,
                            _ => ListenMode::RealTime,
                        },
                        frame,
                    )
                    .await;
                }
                _ => {
                    error!(
                        "invalid frame in phase = {:?},frame = {:?}, state = {:?}",
                        self.phase, frame, msg.state
                    );
                }
            },
            Frame::Voice { data } => {
                self.listener
                    .accept(ListenInput::Audio(data.to_vec()))
                    .await;
            }
            _ => {
                error!(
                    "invalid frame in phase = {:?},frame = {:?}",
                    self.phase, frame
                );
            }
        }
    }

    async fn handle_phase_listen<'a>(&mut self, mode: &ListenMode, frame: &Frame<'a>) {
        self.handle_phase_listen_for_mode(mode, frame).await;
    }

    async fn handle_phase_listen_for_mode<'a>(&mut self, mode: &ListenMode, frame: &Frame<'a>) {
        match frame {
            Frame::Listen(msg) => match &msg.state {
                ListenState::Start => {
                    self.handle_listen_start(msg).await;
                    if *mode == ListenMode::Auto
                        && msg.mmod == Some(service::chobits::message::listen::ListenMode::Auto)
                    {
                        self.update_latest_activity_time();
                        self.new_round(RoundMode::Auto).await;
                        if let Some(round) = &mut self.current_round {
                            round.accept_command(Command::Wake { text: "Hello" }).await;
                        } else {
                            panic!("current round is none");
                        }
                        let silence_voice_timeout = self
                            .config
                            .silence_voice_timeout
                            .expect("logic silence voice timeout is empty");
                        self.listener.reset(Some(silence_voice_timeout)).await;
                    } else if *mode == ListenMode::Manual
                        && msg.mmod == Some(service::chobits::message::listen::ListenMode::Auto)
                    {
                        let silence_voice_timeout = self
                            .config
                            .silence_voice_timeout
                            .expect("logic silence voice timeout is empty");
                        self.listener.reset(Some(silence_voice_timeout)).await;
                    }
                }
                ListenState::Detect => {
                    let Some(text) = &msg.text else {
                        error!(
                            "invalid frame in phase = {:?},frame = {:?}",
                            self.phase, frame
                        );
                        return;
                    };
                    if *mode == ListenMode::Manual {
                        self.interrupt_output().await;
                        self.listener
                            .accept(ListenInput::Text(text.to_string()))
                            .await;
                        self.handle_listen_end().await;
                    } else {
                        self.update_latest_activity_time();
                        self.new_round(self.current_mode).await;
                        if let Some(round) = &mut self.current_round {
                            self.listener.state = ListenerState::End;
                            match self.listener.take_result().await.1 {
                                Ok(_) => {
                                    round.accept_command(Command::Wake { text }).await;
                                }
                                Err(e) => {
                                    error!("{:?}", e);
                                }
                            }
                            let silence_voice_timeout = self
                                .config
                                .silence_voice_timeout
                                .expect("logic silence voice timeout is empty");
                            self.listener.reset(Some(silence_voice_timeout)).await;
                        } else {
                            panic!("current round is none");
                        }
                    }
                }
                ListenState::Stop => {
                    if *mode == ListenMode::Manual && self.current_mode != RoundMode::Text {
                        self.listener.state = ListenerState::End;
                        self.handle_listen_end().await;
                    }
                }
                _ => {
                    error!(
                        "invalid frame in phase = {:?},frame = {:?}",
                        self.phase, frame
                    );
                }
            },
            Frame::Voice { data } => {
                if *mode == ListenMode::Manual {
                    let state = self.listener.state;
                    self.listener
                        .accept(ListenInput::Audio(data.to_vec()))
                        .await;
                    let new_state = self.listener.state;
                    if new_state == ListenerState::Listening(true)
                        && state != ListenerState::Listening(true)
                        && let Some(round) = &self.current_round
                    {
                        round.stop().await;
                    }
                } else {
                    let state = self.listener.state;
                    match &self.current_round {
                        Some(round) => {
                            self.listener
                                .accept(ListenInput::Audio(data.to_vec()))
                                .await;
                            let new_state = self.listener.state;
                            if new_state == ListenerState::Listening(true)
                                && state != ListenerState::Listening(true)
                            {
                                round.stop().await;
                            }
                            if state == ListenerState::End || new_state == ListenerState::End {
                                self.handle_listen_end().await;
                                let silence_voice_timeout = self
                                    .config
                                    .silence_voice_timeout
                                    .expect("logic silence voice timeout is empty");
                                self.listener.reset(Some(silence_voice_timeout)).await;
                                self.update_latest_activity_time();
                            }
                        }
                        None => {
                            if state == ListenerState::End {
                                self.handle_listen_end().await;
                                let silence_voice_timeout = self
                                    .config
                                    .silence_voice_timeout
                                    .expect("logic silence voice timeout is empty");
                                self.listener.reset(Some(silence_voice_timeout)).await;
                                self.update_latest_activity_time();
                            } else {
                                self.listener
                                    .accept(ListenInput::Audio(data.to_vec()))
                                    .await;
                            }
                        }
                    }
                    self.check_activity_timeout().await;
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

    async fn handle_connect(&mut self, _hello_message: &HelloMessage) {
        let tx = self.output_tx.clone().expect("output tx not exists");
        let audio_config = &self.audio_config;
        let data = HelloMessage {
            message: service::chobits::message::Message {
                mtype: service::chobits::message::Type::Hello,
            },
            transport: Some(Transport::Websocket),
            audio_params: Some(AudioParam {
                format: AudioFormat::Opus,
                sample_rate: audio_config
                    .output_sample_rate
                    .expect("output sample rate is empty"),
                channels: audio_config
                    .output_channel
                    .expect("output channel is empty"),
                frame_duration: audio_config
                    .output_frame_duration
                    .expect("output frame duration is empty"),
            }),
            version: None,
            features: None,
            session_id: Some(self.id.clone()),
        };
        if !push_frame(&tx, 0, Ok(FrameResult::HelloResult(data))) {
            info!(target:"session","tx send hello result failure");
        }
    }

    async fn interrupt_output(&mut self) {
        self.stop_round().await;
        self.output_epoch.fetch_add(1, Ordering::Release);
    }

    async fn finish_asr_inner(
        &mut self,
        voice_pcm: Vec<f32>,
        result: (
            Vec<f32>,
            core::result::Result<ListenResult, crate::common::ModelError>,
        ),
        sample_rate: u32,
    ) {
        let (_voice_data_from_result, result) = result;
        match result {
            Ok(ListenResult::Text(text)) => {
                self.new_round(RoundMode::Text).await;
                if let Some(round) = &mut self.current_round {
                    round.accept_command(Command::Chat { text: &text }).await;
                } else {
                    panic!("current round is none");
                }
            }
            Ok(ListenResult::Audio { text, prob }) => {
                self.new_round(self.current_mode).await;
                let round_id = self
                    .current_round
                    .as_ref()
                    .map(|r| r.id.clone())
                    .unwrap_or_default();
                if !voice_pcm.is_empty()
                    && let Some(ref recorder) = self.recorder
                {
                    recorder.on_asr(&AsrContext {
                        round_id: round_id.clone(),
                        voice_pcm: voice_pcm.clone(),
                        sample_rate,
                        text: text.clone(),
                        confidence: prob,
                    });
                    recorder.on_asr_complete(&round_id);
                }
                let is_speech_clear = self.is_speech_clear(&text, prob);
                if let Some(round) = &mut self.current_round {
                    if is_speech_clear {
                        round.accept_command(Command::AsrChat { text: &text }).await;
                    } else {
                        round
                            .accept_command(Command::ListenUnclear { text: &text })
                            .await;
                    }
                } else {
                    panic!("current round is none");
                }
            }
            Err(e) => {
                error!("{:?}", e);
                self.stop_round().await;
            }
        }
    }

    async fn handle_listen_end(&mut self) {
        let sample_rate = self
            .audio_config
            .input_sample_rate
            .expect("input sample rate is empty");

        let voice_pcm = self.listener.take_voice().await;

        if voice_pcm.is_empty() {
            let (voice_data, result) = self.listener.take_result().await;
            self.finish_asr_inner(voice_data.clone(), (voice_data, result), sample_rate)
                .await;
        } else {
            let result = {
                let mut asr = self.listener.asr.lock().await;
                asr.transcribe(sample_rate, &voice_pcm).await
            };
            match result {
                Ok(transcript) => {
                    self.finish_asr_inner(
                        voice_pcm.clone(),
                        (
                            voice_pcm,
                            Ok(ListenResult::Audio {
                                text: transcript.text,
                                prob: transcript.prob,
                            }),
                        ),
                        sample_rate,
                    )
                    .await;
                }
                Err(e) => {
                    error!("ASR transcription error: {:?}", e);
                    self.stop_round().await;
                }
            }
        }
    }

    pub fn is_speech_clear(&self, text: &str, prob: f32) -> bool {
        !text.is_empty() && prob >= 0.8
    }
}
