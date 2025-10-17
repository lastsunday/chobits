use crate::config;
use crate::ws::frame::{FrameError, FrameResult};
use crate::ws::llm::client::Client;
use crate::ws::mcp::McpHost;
use crate::ws::tts::{Tts, TtsKokoro};
use crate::ws::util::llm::{EMOJI_MAP, analyze_emotion};
use anyhow::Context;
use core::result::Result;
use framework::id::gen_id;
use futures::StreamExt;
use rig::OneOrMany;
use rig::completion::CompletionRequest;
use rig::message::{Message, Text, UserContent};
use service::chobits::message::audio::AudioMessage;
use service::chobits::message::llm::LlmMessage;
use service::chobits::message::stt::SttMessage;
use service::chobits::message::tts::{TtsMessage, TtsState};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;
use tokio::time::{Duration, sleep};
use tracing::{error, info, instrument};

pub struct Round {
    pub parent_id: String,
    pub id: String,
    tx: Sender<Result<FrameResult, FrameError>>,
    stop: Arc<AtomicBool>,
    llm: Arc<Client>,
    tts: Arc<Mutex<Box<TtsKokoro>>>,
    pub tts_state: Arc<Mutex<Option<TtsState>>>,
    pub speaking: Arc<AtomicBool>,
    pub end: Arc<AtomicBool>,
    mcp_host: Arc<Mutex<Option<McpHost>>>,
}

#[derive(Debug)]
pub enum Command<'a> {
    Chat { text: &'a str },
    Wake { text: &'a str },
    ListenUnclear { text: &'a str },
}

impl Round {
    pub fn new(
        parent_id: String,
        tx: Sender<Result<FrameResult, FrameError>>,
        llm: Arc<Client>,
        tts: Arc<Mutex<Box<TtsKokoro>>>,
        mcp_host: Arc<Mutex<Option<McpHost>>>,
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
            mcp_host,
        }
    }

    #[instrument(skip(self), name="Round start",fields(id = %self.id,parent_id = %self.parent_id))]
    pub async fn start(&self) {
        info!("start");
    }

    pub async fn accept_command<'a>(&mut self, command: Command<'a>) {
        match command {
            Command::Chat { text } => {
                let system_prompt = config::get().logic().system_prompt();
                // TODO: HOST MCP
                // TODO: MCP client tools list setting
                let mut all_tools = Vec::new();
                let mcp_host = self.mcp_host.clone();
                let mcp_host = mcp_host.lock().await;
                if let Some(mcp_host) = mcp_host.as_ref() {
                    all_tools = mcp_host.get_all_tools().await;
                }
                info!("{:?}", all_tools);
                // TODO: MCP client call tool
                self.llm_tts_handle(text, system_prompt).await;
            }
            Command::Wake { text } => {
                let system_prompt = config::get().logic().system_wake_prompt();
                self.llm_tts_handle(text, system_prompt).await;
            }
            Command::ListenUnclear { text } => {
                let system_prompt = config::get().logic().system_listen_unclear_prompt();
                self.llm_tts_handle(text, system_prompt).await;
            }
        }
    }

    async fn llm_tts_handle<'a>(&mut self, text: &'a str, system_prompt: &'a str) {
        let tx = self.tx.clone();
        let stop_me = self.stop.clone();
        let session_id = self.parent_id.clone();
        let llm = self.llm.clone();
        let tts = self.tts.clone();
        let tts_state_clone = self.tts_state.clone();
        let speaking = self.speaking.clone();
        let end = self.end.clone();
        let text = String::from(text);
        let system_prompt = String::from(system_prompt);
        let chat_history = OneOrMany::<Message>::one(Message::User {
            content: OneOrMany::<UserContent>::one(UserContent::Text(Text { text: text.clone() })),
        });
        let request = CompletionRequest {
            preamble: Some(system_prompt),
            chat_history,
            documents: vec![],
            tools: vec![],
            temperature: Some(0.8),
            max_tokens: Some(999),
            tool_choice: None,
            additional_params: None,
        };
        tokio::spawn(async move {
            if tx
                .send(Ok(FrameResult::STTResult(SttMessage::new(
                    Some(session_id.clone()),
                    Some(text.to_string()),
                ))))
                .await
                .is_err()
            {
                info!("send stt result failure");
            }
            let tts = tts.lock().await;
            let llm_output = llm.chat(request);
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
                        let result: Result<(), anyhow::Error> = async {
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
                            let data = audio_data.into_iter();
                            speaking.store(true, Ordering::Relaxed);
                            info!("set speaking = true");
                            info!("send audio frame start");
                            for packet in data {
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
                        }
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
                if let Some(tts_state) = tts_state.as_ref() {
                    let result: Result<(), anyhow::Error> = async {
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
                    }
                    .await;
                    if let Err(e) = result {
                        error!("{:?}", e)
                    }
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
