use chrono::Local;
use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

pub(crate) const TRACE_CAPACITY: usize = 500;

pub type TraceBuf = Arc<StdMutex<VecDeque<TraceEntry>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Inbound,
    Internal,
    Outbound,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceKind {
    InboundFrame,
    Audio,
    TTS,
    STT,
    LLM,
    Hello,
    MCP,
    Close,
    Error,
}

impl fmt::Display for TraceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
pub struct TraceEntry {
    pub ts: String,
    pub seq: u64,
    pub dir: Direction,
    pub kind: TraceKind,
    pub detail: String,
}

#[derive(Clone, Default)]
pub struct TraceLog {
    pub buf: TraceBuf,
    pub seq: Arc<AtomicU64>,
}

impl TraceLog {
    pub fn new() -> Self {
        Self {
            buf: Arc::new(StdMutex::new(VecDeque::new())),
            seq: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn push(&self, dir: Direction, kind: TraceKind, detail: String) {
        let mut buf = self.buf.lock().expect("trace_buf lock");
        if buf.len() >= TRACE_CAPACITY {
            buf.pop_front();
        }
        buf.push_back(TraceEntry {
            ts: Local::now().format("%H:%M:%S.%3f").to_string(),
            seq: self.seq.fetch_add(1, Ordering::Relaxed),
            dir,
            kind,
            detail,
        });
    }

    pub fn push_input(&self, detail: &str) {
        self.push(Direction::Inbound, TraceKind::InboundFrame, detail.to_string());
    }

    pub fn entries(&self) -> Vec<TraceEntry> {
        let guard = self.buf.lock().expect("trace_buf lock");
        guard.iter().cloned().collect()
    }
}
