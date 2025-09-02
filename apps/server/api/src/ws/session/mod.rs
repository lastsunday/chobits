use crate::config;
use crate::ws::asr::asr_cache::AsrCache;
use crate::ws::frame::{self, Frame, FrameError, FrameResult};
use crate::ws::llm::llm_cache::LlmCache;
use crate::ws::llm::{Llm, LlmQwen};
use crate::ws::session::listener::{DefaultListener, Listener};
use crate::ws::tts::tts_cache::TtsCache;
use crate::ws::tts::{Tts, TtsKokoro};
use crate::ws::util::llm::{EMOJI_MAP, analyze_emotion};
use crate::ws::vad::vad_cache::VadCache;
use anyhow::Context;
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
use tokio::sync::mpsc::{Sender, channel};
use tokio::sync::{Mutex, Notify};
use tokio::task::yield_now;
use tokio::time::{Duration, sleep};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info, instrument};

pub mod listener;

pub struct Session<L> {
    pub id: String,
    pub current_round: Option<Box<Round>>,
    output_tx: Option<Sender<Result<FrameResult, FrameError>>>,
    listener: Box<L>,
}

impl<L> Session<L>
where
    L: Listener,
{
    pub fn new(listener: Box<L>) -> Self {
        Self {
            id: gen_id(),
            current_round: None,
            output_tx: None,
            listener,
        }
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
        match frame {
            Frame::Hello(hello_message) => {
                self.new_round().await;
                self.handle_connect(hello_message).await;
            }
            Frame::Listen(listen_message) => {
                let state = listen_message.state;
                match state {
                    ListenState::Start => {
                        self.listener.clear().await;
                        self.new_round().await;
                    }
                    ListenState::Stop => {
                        if let Some(round) = &mut self.current_round {
                            let command = self.listener.get_result().await;
                            match command {
                                Some(command) => {
                                    round.accept_command(command).await;
                                }
                                None => todo!(),
                            }
                        } else {
                            panic!("current round is none");
                        }
                    }
                    ListenState::Detect => todo!(),
                    ListenState::Text => todo!(),
                }
            }
            Frame::UnknowText(utf8_bytes) => todo!(),
            Frame::Voice(bytes) => {
                self.listener.listen(&bytes).await;
            }
            Frame::Abort(abort_message) => todo!(),
            Frame::Ping(bytes) => todo!(),
            Frame::Pong(bytes) => todo!(),
            Frame::Close(close_message) => {
                if let Some(round) = &mut self.current_round {
                    round.stop().await;
                }
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
        tokio::spawn(async move {
            loop {
                let frame_result = {
                    let mut frame_result_list = frame_result_list_share_for_main_logic.lock().await;
                    frame_result_list.pop_front()
                };
                match frame_result {
                    Some(frame_result) => {
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
}

pub struct Round {
    pub parent_id: String,
    pub id: String,
    tx: Sender<Result<FrameResult, FrameError>>,
    stop: Arc<AtomicBool>,
    llm: Arc<Mutex<Box<LlmQwen>>>,
    tts: Arc<Mutex<Box<TtsKokoro>>>,
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
        }
    }

    #[instrument(skip(self), name="Round start",fields(id = %self.id,parent_id = %self.parent_id))]
    pub async fn start(&self) {
        info!("start");
    }

    // TODO: command wrapper a enum? eg,Text,Call
    pub async fn accept_command(&mut self, command: String) {
        let tx = self.tx.clone();
        let stop_me = self.stop.clone();
        let session_id = self.parent_id.clone();
        let llm = self.llm.clone();
        let tts = self.tts.clone();
        tokio::spawn(async move {
            // TODO: llm,tts logic
            if tx
                .send(Ok(FrameResult::STTResult(SttMessage::new(
                    Some(session_id.clone()),
                    Some(format!("{command}")),
                ))))
                .await
                .is_err()
            {
                info!("send stt result failure");
            }
            let llm = llm.lock().await;
            let tts = tts.lock().await;
            let system_prompt = config::get().logic().system_prompt().to_string();
            let llm_output = llm.chat(system_prompt, format!("{command}"));
            let mut tts_output = tts.output_stream(llm_output);
            while let result = tts_output.next().await {
                if let Some(result) = result {
                    match result {
                        Ok(result) => {
                            let text = result.text;
                            let emotion = analyze_emotion(&text);
                            let session_id = session_id.clone();
                            let tx = tx.clone();
                            let text = text.clone();
                            let audio_data = result.audio;
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
                                tx.send(Ok(FrameResult::TTSResult(TtsMessage::new(
                                    Some(session_id.to_string()),
                                    Some(TtsState::SentenceStart),
                                    Some(text.to_string()),
                                ))))
                                .await
                                .context("send stt result sentence start failure")?;

                                //audio
                                tx.send(Ok(FrameResult::AudioResult(AudioMessage::new(
                                    audio_data,
                                ))))
                                .await
                                .context("send audio result failure")?;

                                tx.send(Ok(FrameResult::TTSResult(TtsMessage::new(
                                    Some(session_id.to_string()),
                                    Some(TtsState::SentenceEnd),
                                    None,
                                ))))
                                .await
                                .context("send stt result sentence end failure")?;
                                tx.send(Ok(FrameResult::TTSResult(TtsMessage::new(
                                    Some(session_id.to_string()),
                                    Some(TtsState::Stop),
                                    None,
                                ))))
                                .await
                                .context("send stt result start failure")?;
                                Ok(())
                            }()
                            .await;
                            if let Err(e) = result {
                                error!("{:?}", e)
                            }
                        }
                        Err(e) => {
                            error!("{:?}", e);
                        }
                    }
                }
                // TODO: stop
                if stop_me.load(Ordering::Relaxed) {
                    // TODO: stop tx
                    drop(tx);
                    // TODO: stop llm
                    // TODO: stop tts
                    return;
                }
            }
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

    use crate::ws::{llm::llm_cache::LlmCache, tts::tts_cache::TtsCache, util::audio::pcm_decode};

    use super::*;

    use anyhow::Context;
    use axum::body::Bytes;
    use service::chobits::message::{hello::HelloMessage, listen::ListenMessage};
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
    /// listen voice and output the asr text result
    /// cargo test --features cuda --package api --lib -- ws::session::tests::test_chat_flow_all --ignored --show-output
    async fn test_chat_flow_all() {
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
                        (_) => {
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
    /// when a round running and has a break event,the output stream will stop the original output
    async fn test_chat_flow_break() {}

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
        Session::new(Box::new(DefaultListener::new(vad, asr.clone())))
    }
}
