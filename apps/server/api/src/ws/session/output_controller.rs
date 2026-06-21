use super::super::WsErrorCode;
use super::super::frame::FrameResult;
use crate::record::observer::{FrameContext, FrameDirection, SessionObserver};
use chrono::Local;
use framework::err;
use framework::error::AppError;
use service::chobits::message::tts::TtsState;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;
use tokio::sync::mpsc::{Receiver, Sender, error::SendError};
use tokio::time::{Duration, Instant, MissedTickBehavior, interval_at};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct TracedSender {
    inner: Sender<Result<FrameResult, AppError>>,
    observers: Vec<Arc<dyn SessionObserver>>,
    round_id: Option<String>,
    seq: Arc<AtomicU64>,
    cancel_token: CancellationToken,
}

impl TracedSender {
    pub fn new(
        inner: Sender<Result<FrameResult, AppError>>,
        observers: Vec<Arc<dyn SessionObserver>>,
        round_id: Option<String>,
        seq: Arc<AtomicU64>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            inner,
            observers,
            round_id,
            seq,
            cancel_token,
        }
    }

    pub async fn send(
        &self,
        item: Result<FrameResult, AppError>,
    ) -> Result<(), SendError<Result<FrameResult, AppError>>> {
        if let Some(ref round_id) = self.round_id {
            let detail = format!("{:?}", &item);
            let seq = self.seq.fetch_add(1, Ordering::Relaxed);
            for observer in &self.observers {
                observer.on_frame(&FrameContext {
                    round_id: round_id.clone(),
                    seq,
                    direction: FrameDirection::Outbound,
                    detail: detail.clone(),
                });
            }
        }
        tokio::select! {
            result = self.inner.send(item) => result,
            _ = self.cancel_token.cancelled() => {
                Err(SendError(Err(err!(WsErrorCode::InternalError))))
            }
        }
    }
}

pub struct OutputController {
    input_rx: Receiver<Result<FrameResult, AppError>>,
    output_tx: Sender<Result<FrameResult, AppError>>,
    interval: Option<tokio::time::Interval>,
    frame_duration: u64,
    latest_activity_time: Arc<Mutex<Option<i64>>>,
    cancel_token: CancellationToken,
}

impl OutputController {
    pub fn new(
        input_rx: Receiver<Result<FrameResult, AppError>>,
        output_tx: Sender<Result<FrameResult, AppError>>,
        frame_duration: u64,
        latest_activity_time: Arc<Mutex<Option<i64>>>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            input_rx,
            output_tx,
            interval: None,
            frame_duration,
            latest_activity_time,
            cancel_token,
        }
    }

    pub fn start(mut self) {
        tokio::spawn(async move {
            self.run().await;
        });
    }

    async fn run(&mut self) {
        loop {
            tokio::select! {
                item = self.input_rx.recv() => {
                    match item {
                        Some(item) => { if self.dispatch(item).await { break; } }
                        None => break,
                    }
                }
                _ = self.cancel_token.cancelled() => break,
            }
        }
    }

    /// returns true if should stop
    async fn dispatch(&mut self, item: Result<FrameResult, AppError>) -> bool {
        match &item {
            Ok(FrameResult::TTSResult(msg)) if msg.state == Some(TtsState::Start) => {
                self.interval = None;
            }
            Ok(FrameResult::AudioResult(_)) => {
                self.pace_audio().await;
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
