use super::super::frame::FrameResult;
use super::output_controller::TracedSender;
use crate::llm::client::{ChatRequest, Client};
use crate::tts::Tts;
use crate::util::llm::{EMOJI_MAP, analyze_emotion};
use crate::ws::WsErrorCode;
use anyhow::Context;
use core::result::Result;
use framework::err;
use framework::error::AppError;
use framework::id::gen_id;
use futures::StreamExt;
use rig::OneOrMany;
use rig::message::{Message, Text, UserContent};
use service::chobits::message::audio::AudioMessage;
use service::chobits::message::llm::LlmMessage;
use service::chobits::message::stt::SttMessage;
use service::chobits::message::tts::{TtsMessage, TtsState};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tokio::sync::mpsc::error::SendError;
use tracing::{Instrument, Level, debug, error, info, span};

pub struct Round {
    pub parent_id: String,
    pub id: String,
    tx: TracedSender,
    stop: Arc<AtomicBool>,
    client: Arc<Client>,
    tts: Arc<Box<dyn Tts>>,
    pub tts_state: Arc<Mutex<Option<TtsState>>>,
    pub speaking: Arc<AtomicBool>,
    pub end: Arc<AtomicBool>,
}

#[derive(Debug)]
pub enum Command<'a> {
    Chat { text: &'a str },
    Wake { text: &'a str },
    ListenUnclear { text: &'a str },
}

async fn change_tts_state(tts_state: Arc<Mutex<Option<TtsState>>>, state: TtsState) {
    let mut tts_state = tts_state.lock().await;
    *tts_state = Some(state);
}

async fn send_tts_frame(
    tx: &TracedSender,
    session_id: String,
    state: TtsState,
    text: Option<String>,
) -> Result<(), SendError<Result<FrameResult, AppError>>> {
    tx.send(Ok(FrameResult::TTSResult(TtsMessage::new(
        Some(session_id),
        Some(state),
        text,
    ))))
    .await?;
    Ok(())
}

async fn send_tts_frame_and_change_state(
    tts_state: Arc<Mutex<Option<TtsState>>>,
    tx: &TracedSender,
    session_id: String,
    state: TtsState,
    text: Option<String>,
) -> Result<(), SendError<Result<FrameResult, AppError>>> {
    change_tts_state(tts_state, state.clone()).await;
    send_tts_frame(tx, session_id, state, text).await?;
    Ok(())
}

impl Round {
    pub fn new(
        parent_id: String,
        tx: TracedSender,
        client: Arc<Client>,
        tts: Arc<Box<dyn Tts>>,
    ) -> Self {
        Self {
            parent_id,
            id: gen_id(),
            tx,
            stop: Arc::new(AtomicBool::new(false)),
            client,
            tts,
            tts_state: Arc::new(Mutex::new(None)),
            speaking: Arc::new(AtomicBool::new(false)),
            end: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(&self) {
        info!(target:"round","start");
    }

    pub async fn accept_command<'a>(&mut self, command: Command<'a>) {
        match command {
            Command::Chat { text } => {
                self.llm_tts_handle(text).await;
            }
            Command::Wake { text } => {
                self.llm_tts_handle(text).await;
            }
            Command::ListenUnclear { text } => {
                self.llm_tts_handle(text).await;
            }
        }
    }

    async fn llm_tts_handle(&mut self, text: &str) {
        let tx = self.tx.clone();
        let id = self.id.clone();
        let stop_me = self.stop.clone();
        let session_id = self.parent_id.clone();
        let client = self.client.clone();
        let tts = self.tts.clone();
        let tts_state_clone = self.tts_state.clone();
        let speaking = self.speaking.clone();
        let end = self.end.clone();
        let text = String::from(text);
        let span = span!(parent:None,Level::DEBUG, "socket", id=%session_id);
        tokio::spawn(
            async move {
                if tx
                    .send(Ok(FrameResult::STTResult(SttMessage::new(
                        Some(session_id.clone()),
                        Some(text.to_string()),
                    ))))
                    .await
                    .is_err()
                {
                    info!(target:"round","send stt result failure");
                    return;
                }
                let request = ChatRequest {
                    message: Message::User {
                        content: OneOrMany::one(UserContent::Text(Text { text: text.clone() })),
                    },
                };
                let llm_output = client.chat(request);
                let mut tts_output = tts.stream(Box::pin(llm_output)).await;
                let speaking = speaking.clone();
                let stop_me = stop_me.clone();
                if send_tts_frame_and_change_state(
                    tts_state_clone.clone(),
                    &tx,
                    session_id.clone(),
                    TtsState::Start,
                    None,
                )
                .await
                .is_err()
                {
                    info!(target:"round","send tts state start failure");
                    stop_me.store(true, Ordering::Relaxed);
                }
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
                                tx.send(Ok(FrameResult::LLMResult(LlmMessage::new(
                                    Some(session_id.to_string()),
                                    Some(emotion.to_string()),
                                    Some(EMOJI_MAP.get(emotion).map_or(r#"😶"#, |v| v).to_string()),
                                ))))
                                .await
                                .context("send llm result failure")?;
                                send_tts_frame_and_change_state(
                                    tts_state_clone.clone(),
                                    &tx,
                                    session_id.clone(),
                                    TtsState::SentenceStart,
                                    Some(text.to_string()),
                                )
                                .await?;
                                let audio_data = audio_data.unwrap_or_default();
                                let data = audio_data.into_iter();
                                speaking.store(true, Ordering::Relaxed);
                                debug!(target:"round","speaking start");
                                debug!(target:"round","send audio frame start");
                                for packet in data {
                                    if stop_me_by_tts_packet.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    tx.send(Ok(FrameResult::AudioResult(AudioMessage::new(
                                        Some(session_id.to_string()),
                                        packet,
                                    ))))
                                    .await
                                    .context("send audio result failure")?;
                                }
                                debug!(target:"round","send audio frame end");
                                speaking.store(false, Ordering::Relaxed);
                                debug!(target:"round","speaking end");
                                send_tts_frame_and_change_state(
                                    tts_state_clone.clone(),
                                    &tx,
                                    session_id.clone(),
                                    TtsState::SentenceEnd,
                                    None,
                                )
                                .await?;
                                Ok(())
                            }
                            .await;
                            if let Err(e) = result {
                                error!(target:"round","{:?}", e);
                                stop_me.store(true, Ordering::Relaxed);
                                break;
                            }
                        }
                        Err(e) => {
                            error!(target:"round","{:?}", e);
                            if let Err(e) = tx
                                .send(Err(err!(WsErrorCode::TtsEncode).with_extra(e.to_string())))
                                .await
                            {
                                error!(target:"round","{:?}", e);
                            }
                            stop_me.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                }
                if send_tts_frame_and_change_state(
                    tts_state_clone.clone(),
                    &tx,
                    session_id.clone(),
                    TtsState::Stop,
                    None,
                )
                .await
                .is_err()
                {
                    debug!(target:"round","send tts state stop failure");
                    stop_me.store(true, Ordering::Relaxed);
                }
                if stop_me.load(Ordering::Relaxed) {
                    let tts_state = tts_state_clone.lock().await;
                    debug!(target:"round",
                        "trigger stop me,round id = {}, current tts state = {:?}",
                        id.clone(),
                        tts_state
                    );
                    if let Some(tts_state) = tts_state.as_ref() {
                        let result: Result<(), anyhow::Error> = async {
                            if tts_state < &TtsState::Start {
                                send_tts_frame(
                                    &tx,
                                    session_id.clone(),
                                    TtsState::SentenceStart,
                                    None,
                                )
                                .await?;
                                debug!(
                                    target:"round",
                                    "after trigger stop me send tts state = {:?}",
                                    TtsState::SentenceStart
                                );
                            }
                            if tts_state < &TtsState::SentenceStart {
                                send_tts_frame(
                                    &tx,
                                    session_id.clone(),
                                    TtsState::SentenceEnd,
                                    None,
                                )
                                .await?;
                                debug!(
                                    target:"round",
                                    "after trigger stop me send tts state = {:?}",
                                    TtsState::SentenceEnd
                                );
                            }
                            if tts_state < &TtsState::SentenceEnd {
                                send_tts_frame(&tx, session_id.clone(), TtsState::Stop, None)
                                    .await?;
                                debug!(
                                    target:"round",
                                    "after trigger stop me send tts state = {:?}",
                                    TtsState::Stop
                                );
                            }
                            Ok(())
                        }
                        .await;
                        if let Err(e) = result {
                            error!(target:"round","{:?}", e)
                        }
                    }
                }
                end.store(true, Ordering::Relaxed);
                info!(target:"round","end");
            }
            .instrument(span),
        );
    }

    pub async fn stop(&self) {
        info!(target:"round","stop");
        self.stop.store(true, Ordering::Relaxed);
    }
}
