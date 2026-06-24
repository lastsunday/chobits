use super::super::frame::FrameResult;
use crate::llm::client::{ChatRequest, Client};
use crate::record::observer::{
    FrameContext, FrameDirection, LlmDeltaContext, RoundEndContext, SessionObserver,
    TextInputContext, TtsDeltaContext,
};
use crate::tts::Tts;
use crate::util::llm::{EMOJI_MAP, analyze_emotion};
use crate::ws::WsErrorCode;
pub struct OutputMessage {
    pub epoch: u64,
    pub payload: Result<FrameResult, AppError>,
}
use anyhow::Context;
use core::result::Result;
use framework::err;
use framework::error::AppError;
use futures::StreamExt;
use rig::OneOrMany;
use rig::message::{Message, Text, UserContent};
use service::chobits::message::audio::AudioMessage;
use service::chobits::message::llm::LlmMessage;
use service::chobits::message::stt::SttMessage;
use service::chobits::message::tts::{TtsMessage, TtsState};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::SendError;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, Level, error, info, span};

pub struct Round {
    pub parent_id: String,
    pub id: String,
    tx: TracedSender,
    stop: Arc<AtomicBool>,
    client: Arc<Client>,
    tts: Arc<Box<dyn Tts>>,
    pub tts_state: Arc<Mutex<Option<TtsState>>>,
    pub speaking: Arc<AtomicBool>,
    pub cancel: CancellationToken,
    pub join_handle: Option<JoinHandle<()>>,
    pub observers: Vec<Arc<dyn SessionObserver>>,
}

#[derive(Debug)]
pub enum Command<'a> {
    Chat { text: &'a str },
    AsrChat { text: &'a str },
    Wake { text: &'a str },
    ListenUnclear { text: &'a str },
}

#[derive(Clone)]
pub struct TracedSender {
    inner: Sender<OutputMessage>,
    observers: Vec<Arc<dyn SessionObserver>>,
    round_id: Option<String>,
    session_id: Option<String>,
    seq: Arc<AtomicU64>,
    cancel_token: CancellationToken,
    epoch: u64,
}

impl TracedSender {
    pub fn new(
        inner: Sender<OutputMessage>,
        observers: Vec<Arc<dyn SessionObserver>>,
        round_id: Option<String>,
        session_id: Option<String>,
        seq: Arc<AtomicU64>,
        cancel_token: CancellationToken,
        epoch: u64,
    ) -> Self {
        Self {
            inner,
            observers,
            round_id,
            session_id,
            seq,
            cancel_token,
            epoch,
        }
    }

    pub async fn send(
        &self,
        item: Result<FrameResult, AppError>,
    ) -> Result<(), SendError<OutputMessage>> {
        if let Some(ref round_id) = self.round_id {
            let detail = match &item {
                Ok(r) => format!("{r}"),
                Err(e) => format!("Err({e})"),
            };
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            for observer in &self.observers {
                observer
                    .on_frame(&FrameContext {
                        round_id: Some(round_id.clone()),
                        session_id: self.session_id.clone(),
                        seq,
                        direction: FrameDirection::Outbound,
                        detail: detail.clone(),
                    })
                    .await;
            }
        }
        tokio::select! {
            result = self.inner.send(OutputMessage { epoch: self.epoch, payload: item }) => result,
            _ = self.cancel_token.cancelled() => {
                Err(SendError(OutputMessage { epoch: self.epoch, payload: Err(err!(WsErrorCode::InternalError)) }))
            }
        }
    }
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
) -> Result<(), SendError<OutputMessage>> {
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
) -> Result<(), SendError<OutputMessage>> {
    change_tts_state(tts_state, state.clone()).await;
    send_tts_frame(tx, session_id, state, text).await?;
    Ok(())
}

impl Round {
    pub fn new(
        parent_id: String,
        id: String,
        tx: TracedSender,
        client: Arc<Client>,
        tts: Arc<Box<dyn Tts>>,
        observers: Vec<Arc<dyn SessionObserver>>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            parent_id,
            id,
            tx,
            stop: Arc::new(AtomicBool::new(false)),
            client,
            tts,
            tts_state: Arc::new(Mutex::new(None)),
            speaking: Arc::new(AtomicBool::new(false)),
            cancel,
            join_handle: None,
            observers,
        }
    }

    pub async fn start(&self) {
        info!(target:"round","start");
    }

    pub async fn accept_command<'a>(&mut self, command: Command<'a>) {
        match command {
            Command::Chat { text } => {
                for observer in &self.observers {
                    observer.on_text_input(&TextInputContext {
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
        let tx = self.tx.clone();
        let stop_me = self.stop.clone();
        let session_id = self.parent_id.clone();
        let client = self.client.clone();
        let tts = self.tts.clone();
        let tts_state_clone = self.tts_state.clone();
        let speaking = self.speaking.clone();
        let text = String::from(text);
        let observers = self.observers.clone();
        let round_id = self.id.clone();
        let cancel = self.cancel.clone();
        let span = span!(parent:None,Level::DEBUG, "socket", id=%session_id);
        self.join_handle = Some(tokio::spawn(
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
                let llm_output = client.chat(request, cancel.clone());
                let mut tts_output = tts.stream(Box::pin(llm_output), cancel).await;
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
                            for observer in &observers {
                                observer.on_llm_delta(&LlmDeltaContext {
                                    round_id: round_id.clone(),
                                    text: text.clone(),
                                });
                                if let Some((pcm, sr)) = &result.raw_pcm {
                                    observer.on_tts_delta(&TtsDeltaContext {
                                        round_id: round_id.clone(),
                                        text: text.clone(),
                                        raw_pcm: Some((pcm.clone(), *sr as u32)),
                                    });
                                }
                            }
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
                                speaking.store(false, Ordering::Relaxed);
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
                    stop_me.store(true, Ordering::Relaxed);
                }
                if stop_me.load(Ordering::Relaxed) {
                    let tts_state = tts_state_clone.lock().await;
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
                            }
                            if tts_state < &TtsState::SentenceStart {
                                send_tts_frame(
                                    &tx,
                                    session_id.clone(),
                                    TtsState::SentenceEnd,
                                    None,
                                )
                                .await?;
                            }
                            if tts_state < &TtsState::SentenceEnd {
                                send_tts_frame(&tx, session_id.clone(), TtsState::Stop, None)
                                    .await?;
                            }
                            Ok(())
                        }
                        .await;
                        if let Err(e) = result {
                            error!(target:"round","{:?}", e)
                        }
                    }
                }
                for observer in &observers {
                    let _ = observer
                        .on_round_end(&RoundEndContext {
                            round_id: round_id.clone(),
                        })
                        .await;
                }
                info!(target:"round","end");
            }
            .instrument(span),
        ));
    }

    pub async fn stop(&self) {
        info!(target:"round","stop");
        self.stop.store(true, Ordering::Relaxed);
        self.cancel.cancel();
    }
}
