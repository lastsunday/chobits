use super::super::frame::{FrameError, FrameResult};
use crate::llm::client::{ChatRequest, Client};
use crate::tts::Tts;
use crate::util::llm::{EMOJI_MAP, analyze_emotion};
use anyhow::Context;
use core::result::Result;
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
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::SendError;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, instrument};

pub struct Round {
    pub parent_id: String,
    pub id: String,
    tx: Sender<Result<FrameResult, FrameError>>,
    stop: Arc<AtomicBool>,
    client: Arc<Client>,
    tts: Arc<Box<dyn Tts>>,
    pub tts_state: Arc<Mutex<Option<TtsState>>>,
    pub speaking: Arc<AtomicBool>,
    pub end: Arc<AtomicBool>,
    output_frame_duration: Option<u64>,
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
    // drop(tts_state);
}

async fn send_tts_frame(
    tx: Sender<Result<FrameResult, FrameError>>,
    session_id: String,
    state: TtsState,
    text: Option<String>,
) -> Result<(), SendError<Result<FrameResult, FrameError>>> {
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
    tx: Sender<Result<FrameResult, FrameError>>,
    session_id: String,
    state: TtsState,
    text: Option<String>,
) -> Result<(), SendError<Result<FrameResult, FrameError>>> {
    change_tts_state(tts_state, state.clone()).await;
    send_tts_frame(tx, session_id, state, text).await?;
    Ok(())
}

impl Round {
    pub fn new(
        parent_id: String,
        tx: Sender<Result<FrameResult, FrameError>>,
        client: Arc<Client>,
        tts: Arc<Box<dyn Tts>>,
        output_frame_duration: Option<u64>,
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
            output_frame_duration,
        }
    }

    #[instrument(skip(self), name="Round start",fields(round_id = %self.id))]
    pub async fn start(&self) {
        debug!("start");
    }

    #[instrument(skip(self), name="Accept command",fields(round_id = %self.id))]
    pub async fn accept_command<'a>(&mut self, command: Command<'a>) {
        debug!("accept command = {:?}", command);
        match command {
            Command::Chat { text } => {
                self.llm_tts_handle(text).await;
            }
            Command::Wake { text } => {
                // let system_prompt = config::get().logic().system_wake_prompt();
                // TODO: add wake tip to text?
                self.llm_tts_handle(text).await;
            }
            Command::ListenUnclear { text } => {
                // let system_prompt = config::get().logic().system_listen_unclear_prompt();
                // TODO: add unclear tip to text?
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
        // let history = self.history.clone();
        let text = String::from(text);
        let output_frame_duration = self.output_frame_duration;
        tokio::spawn(async move {
            if tx
                .send(Ok(FrameResult::STTResult(SttMessage::new(
                    Some(session_id.clone()),
                    Some(text.to_string()),
                ))))
                .await
                .is_err()
            {
                debug!("send stt result failure");
                return;
            }
            let request = ChatRequest {
                message: Message::User {
                    content: OneOrMany::one(UserContent::Text(Text { text: text.clone() })),
                },
            };
            let llm_output = client.chat(request);
            let mut tts_output = tts.stream(Box::pin(llm_output)).await;
            let delay = output_frame_duration.expect("output frame duration is empty");
            let mut latest_time = Instant::now() + Duration::from_millis(delay);
            // pre buffer count
            let pre_buffer_frame_count: u64 = 6;
            let mut send_frame_count: u64 = 0;
            let speaking = speaking.clone();
            let stop_me = stop_me.clone();
            if send_tts_frame_and_change_state(
                tts_state_clone.clone(),
                tx.clone(),
                session_id.clone(),
                TtsState::Start,
                None,
            )
            .await
            .is_err()
            {
                debug!("send tts state start failure");
                stop_me.store(true, Ordering::Relaxed);
            }
            while let Some(result) = tts_output.next().await {
                match result {
                    Ok(result) => {
                        if stop_me.load(Ordering::Relaxed) {
                            break;
                        }
                        let text = result.text;
                        // TODO: add llm response text to chat history
                        // TODO: consider all llm text? tts output text?(tts output in one message item?)
                        let emotion = analyze_emotion(&text);
                        let session_id = session_id.clone();
                        let tx = tx.clone();
                        let text = text.clone();
                        let audio_data = result.audio;
                        // TODO: save text and tts with session id,round id to database
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
                            send_tts_frame_and_change_state(
                                tts_state_clone.clone(),
                                tx.clone(),
                                session_id.clone(),
                                TtsState::SentenceStart,
                                Some(text.to_string()),
                            )
                            .await?;
                            //audio
                            //real time send audio
                            let audio_data = audio_data.unwrap_or_default();
                            let data = audio_data.into_iter();
                            speaking.store(true, Ordering::Relaxed);
                            debug!("set speaking = true");
                            debug!("send audio frame start");
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
                            debug!("send audio frame end");
                            speaking.store(false, Ordering::Relaxed);
                            debug!("set speaking = false");
                            send_tts_frame_and_change_state(
                                tts_state_clone.clone(),
                                tx.clone(),
                                session_id.clone(),
                                TtsState::SentenceEnd,
                                None,
                            )
                            .await?;
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
                        if let Err(e) = tx.send(Err(FrameError::Tts(e.to_string()))).await {
                            error!("{:?}", e);
                        }
                        stop_me.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
            if send_tts_frame_and_change_state(
                tts_state_clone.clone(),
                tx.clone(),
                session_id.clone(),
                TtsState::Stop,
                None,
            )
            .await
            .is_err()
            {
                debug!("send tts state stop failure");
                stop_me.store(true, Ordering::Relaxed);
            }
            if stop_me.load(Ordering::Relaxed) {
                let tts_state = tts_state_clone.lock().await;
                debug!(
                    "trigger stop me,round id = {}, current tts state = {:?}",
                    id.clone(),
                    tts_state
                );
                if let Some(tts_state) = tts_state.as_ref() {
                    let result: Result<(), anyhow::Error> = async {
                        if tts_state < &TtsState::Start {
                            send_tts_frame(
                                tx.clone(),
                                session_id.clone(),
                                TtsState::SentenceStart,
                                None,
                            )
                            .await?;
                            debug!(
                                "after trigger stop me send tts state = {:?}",
                                TtsState::SentenceStart
                            );
                        }
                        if tts_state < &TtsState::SentenceStart {
                            send_tts_frame(
                                tx.clone(),
                                session_id.clone(),
                                TtsState::SentenceEnd,
                                None,
                            )
                            .await?;
                            debug!(
                                "after trigger stop me send tts state = {:?}",
                                TtsState::SentenceEnd
                            );
                        }
                        if tts_state < &TtsState::SentenceEnd {
                            send_tts_frame(tx.clone(), session_id.clone(), TtsState::Stop, None)
                                .await?;
                            debug!(
                                "after trigger stop me send tts state = {:?}",
                                TtsState::Stop
                            );
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
            debug!("round setting end = true");
        });
    }

    #[instrument(skip(self), name="Round stop",fields(round_id = %self.id))]
    pub async fn stop(&self) {
        debug!("stop");
        self.stop.store(true, Ordering::Relaxed);
    }
}
