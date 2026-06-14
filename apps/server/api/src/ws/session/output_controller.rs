use super::super::frame::FrameResult;
use super::trace::{Direction, TraceKind, TraceLog};
use chrono::Local;
use framework::error::AppError;
use service::chobits::message::tts::TtsState;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{Receiver, Sender, error::SendError};
use tokio::time::{Duration, sleep};

const PRE_BUFFER_FRAME_COUNT: u64 = 6;

fn trace_info_from_result(item: &Result<FrameResult, AppError>) -> (TraceKind, String) {
    match item {
        Ok(FrameResult::AudioResult(msg)) => (TraceKind::Audio, format!("{} bytes", msg.data.len())),
        Ok(FrameResult::TTSResult(msg)) => {
            let detail = msg
                .text
                .clone()
                .or_else(|| msg.state.as_ref().map(|s| format!("{:?}", s)))
                .unwrap_or_default();
            (TraceKind::TTS, detail)
        }
        Ok(FrameResult::STTResult(msg)) => (TraceKind::STT, msg.text.clone().unwrap_or_default()),
        Ok(FrameResult::LLMResult(msg)) => (TraceKind::LLM, msg.text.clone().unwrap_or_default()),
        Ok(FrameResult::HelloResult(_)) => (TraceKind::Hello, String::new()),
        Ok(FrameResult::McpResult(_)) => (TraceKind::MCP, String::new()),
        Ok(FrameResult::CloseResult) => (TraceKind::Close, String::new()),
        Err(_) => (TraceKind::Error, String::new()),
    }
}

#[derive(Clone)]
pub struct TracedSender {
    inner: Sender<Result<FrameResult, AppError>>,
    log: TraceLog,
    dir: Direction,
}

impl TracedSender {
    pub fn new(
        inner: Sender<Result<FrameResult, AppError>>,
        log: TraceLog,
        dir: Direction,
    ) -> Self {
        Self { inner, log, dir }
    }

    pub async fn send(
        &self,
        item: Result<FrameResult, AppError>,
    ) -> Result<(), SendError<Result<FrameResult, AppError>>> {
        let (kind, detail) = trace_info_from_result(&item);
        self.log.push(self.dir, kind, detail);
        self.inner.send(item).await
    }
}

pub struct OutputController {
    input_rx: Receiver<Result<FrameResult, AppError>>,
    output_tx: TracedSender,
    audio_send_count: u64,
    audio_last_time: Instant,
    frame_duration: u64,
    latest_activity_time: Arc<Mutex<Option<i64>>>,
}

impl OutputController {
    pub fn new(
    input_rx: Receiver<Result<FrameResult, AppError>>,
        output_tx: TracedSender,
        frame_duration: u64,
        latest_activity_time: Arc<Mutex<Option<i64>>>,
    ) -> Self {
        Self {
            input_rx,
            output_tx,
            audio_send_count: 0,
            audio_last_time: Instant::now(),
            frame_duration,
            latest_activity_time,
        }
    }

    pub fn start(mut self) {
        tokio::spawn(async move {
            self.run().await;
        });
    }

    async fn run(&mut self) {
        while let Some(item) = self.input_rx.recv().await {
            if self.dispatch(item).await {
                break;
            }
        }
    }

    /// returns true if should stop
    async fn dispatch(&mut self, item: Result<FrameResult, AppError>) -> bool {
        match &item {
            Ok(FrameResult::TTSResult(msg)) if msg.state == Some(TtsState::Start) => {
                self.audio_send_count = 0;
                self.audio_last_time = Instant::now();
            }
            Ok(FrameResult::AudioResult(_)) => {
                self.pace_audio().await;
                self.audio_last_time = Instant::now();
                self.audio_send_count += 1;
            }
            _ => {}
        }
        self.send_now(item).await
    }

    /// returns true if output channel closed
    async fn send_now(&mut self, item: Result<FrameResult, AppError>) -> bool {
        {
            let mut time = self.latest_activity_time.lock().await;
            *time = Some(Local::now().timestamp_millis());
        }
        self.output_tx.send(item).await.is_err()
    }

    async fn pace_audio(&mut self) {
        if self.audio_send_count >= PRE_BUFFER_FRAME_COUNT {
            let elapsed = self.audio_last_time.elapsed();
            let delay_ms = self
                .frame_duration
                .saturating_sub(elapsed.as_millis() as u64);
            if delay_ms > 0 {
                sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }
}
