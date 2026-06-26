use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use sea_orm::{ActiveValue::Set, DatabaseConnection, entity::prelude::*};
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;

use super::observer::*;
use super::wav::pcm_f32_to_wav;
use entity::frame;
use entity::round;
use entity::round_data;
use entity::session;
use framework::id::gen_id;

const MAX_FRAMES_PER_ROUND: usize = 5000;

struct FrameEntry {
    seq: u64,
    is_inbound: bool,
    detail: String,
    session_id: Option<String>,
    received_at: Instant,
}

struct RoundStep {
    step: String,
    text: String,
    pcm: Vec<f32>,
    sample_rate: Option<u32>,
    confidence: Option<f32>,
    first_event_at: Instant,
    last_event_at: Instant,
    has_text: bool,
    has_pcm: bool,
}

struct RoundBuffer {
    round_id: String,
    session_id: Option<String>,
    client_info: Option<JsonValue>,
    mode: String,
    round_start_at: Instant,
    round_creation_at: Instant,
    steps: Vec<RoundStep>,
    frames: Vec<FrameEntry>,
}

impl RoundBuffer {
    fn step_idx(&self, name: &str) -> Option<usize> {
        self.steps.iter().position(|s| s.step == name)
    }

    fn step_mut(&mut self, name: &str) -> &mut RoundStep {
        let idx = if let Some(pos) = self.step_idx(name) {
            pos
        } else {
            let now = Instant::now();
            let pos = self.steps.len();
            self.steps.push(RoundStep {
                step: name.to_string(),
                text: String::new(),
                pcm: Vec::new(),
                sample_rate: None,
                confidence: None,
                first_event_at: now,
                last_event_at: now,
                has_text: false,
                has_pcm: false,
            });
            pos
        };
        let s = &mut self.steps[idx];
        s.last_event_at = Instant::now();
        s
    }

    fn elapsed_ms(&self, name: &str) -> Option<i64> {
        self.step_idx(name).map(|idx| {
            let s = &self.steps[idx];
            let elapsed = s.first_event_at.duration_since(self.round_start_at);
            elapsed.as_millis() as i64
        })
    }

    fn process_ms(&self, name: &str) -> Option<i64> {
        self.step_idx(name).map(|idx| {
            let s = &self.steps[idx];
            let elapsed = s.first_event_at.duration_since(self.round_creation_at);
            elapsed.as_millis() as i64
        })
    }

    fn step_has(&self, name: &str, field: fn(&RoundStep) -> bool) -> bool {
        self.steps.iter().any(|s| s.step == name && field(s))
    }

    fn step_text(&self, name: &str) -> Option<String> {
        self.steps.iter().find(|s| s.step == name).and_then(|s| {
            if s.has_text && !s.text.is_empty() {
                Some(s.text.clone())
            } else {
                None
            }
        })
    }
}

struct FlushEvent {
    buffer: RoundBuffer,
    reason: RoundEndReason,
}

pub struct RecordCollector {
    conn: DatabaseConnection,
    pending: Arc<StdMutex<HashMap<String, RoundBuffer>>>,
    orphaned_frames: Arc<StdMutex<HashMap<String, Vec<FrameEntry>>>>,
    flush_tx: mpsc::UnboundedSender<FlushEvent>,
    _flush_handle: tokio::task::JoinHandle<()>,
}

impl RecordCollector {
    pub fn new(conn: DatabaseConnection) -> Self {
        let (flush_tx, mut flush_rx) = mpsc::unbounded_channel::<FlushEvent>();
        let bg_conn = conn.clone();
        let _flush_handle = tokio::spawn(async move {
            while let Some(event) = flush_rx.recv().await {
                if let Err(e) = Self::flush_to_db(&bg_conn, &event.buffer, event.reason).await {
                    tracing::error!("DB flush error: {e}");
                }
            }
        });
        Self {
            conn,
            pending: Arc::new(StdMutex::new(HashMap::new())),
            orphaned_frames: Arc::new(StdMutex::new(HashMap::new())),
            flush_tx,
            _flush_handle,
        }
    }

    pub async fn on_session_end(&self, session_id: &str) {
        let now = chrono::Local::now().fixed_offset();
        session::ActiveModel {
            id: Set(session_id.to_string()),
            end_time: Set(Some(now)),
            ..Default::default()
        }
        .insert(&self.conn)
        .await
        .ok();
    }

    fn start_round(
        &self,
        round_id: String,
        session_id: Option<String>,
        client_info: Option<JsonValue>,
        mode: &str,
    ) {
        let mut pending = self.pending.lock().expect("pending lock");
        let now = Instant::now();
        pending.insert(
            round_id.clone(),
            RoundBuffer {
                round_id,
                session_id,
                client_info,
                mode: mode.to_string(),
                round_start_at: now,
                round_creation_at: now,
                steps: Vec::new(),
                frames: Vec::new(),
            },
        );
    }

    fn collect_asr(
        &self,
        round_id: &str,
        voice_pcm: Vec<f32>,
        sample_rate: u32,
        text: String,
        confidence: f32,
    ) {
        let mut pending = self.pending.lock().expect("pending lock");
        if let Some(buf) = pending.get_mut(round_id) {
            let io = buf.step_mut("input_audio");
            io.pcm = voice_pcm;
            io.sample_rate = Some(sample_rate);
            io.has_pcm = true;
            let dur =
                Duration::from_millis((io.pcm.len() as f64 / sample_rate as f64 * 1000.0) as u64);
            io.first_event_at = io
                .first_event_at
                .checked_sub(dur)
                .unwrap_or(io.first_event_at);

            let asr = buf.step_mut("asr");
            asr.text = text;
            asr.confidence = Some(confidence);
            asr.has_text = true;
        }
    }

    fn align_round_start(&self, round_id: &str) {
        let mut pending = self.pending.lock().expect("pending lock");
        if let Some(buf) = pending.get_mut(round_id)
            && let Some(io) = buf.steps.iter().find(|s| s.step == "input_audio")
            && buf.round_start_at != io.first_event_at
        {
            buf.round_creation_at = buf.round_start_at;
            buf.round_start_at = io.first_event_at;
        }
    }

    fn collect_text(&self, round_id: &str, text: &str) {
        let mut pending = self.pending.lock().expect("pending lock");
        if let Some(buf) = pending.get_mut(round_id) {
            let t = buf.step_mut("text");
            t.text = text.to_string();
            t.has_text = true;
        }
    }

    fn collect_llm_text(&self, round_id: &str, text: &str) {
        let mut pending = self.pending.lock().expect("pending lock");
        if let Some(buf) = pending.get_mut(round_id) {
            let llm = buf.step_mut("llm");
            llm.text.push_str(text);
            llm.has_text = true;
        }
    }

    fn collect_tts(&self, round_id: &str, text: &str, raw_pcm: Option<(Vec<f32>, u32)>) {
        let mut pending = self.pending.lock().expect("pending lock");
        if let Some(buf) = pending.get_mut(round_id) {
            let tts = buf.step_mut("tts");
            tts.text.push_str(text);
            tts.has_text = true;
            if let Some((new_pcm, sr)) = raw_pcm {
                tts.pcm.extend(new_pcm);
                tts.sample_rate = Some(sr);
                tts.has_pcm = true;
            }
        }
    }

    fn record_frame(
        &self,
        round_id: Option<&str>,
        session_id: Option<&str>,
        seq: u64,
        is_inbound: bool,
        detail: &str,
    ) {
        // If there's a round_id, try the pending buffer first
        if let Some(rid) = round_id {
            let mut pending = self.pending.lock().expect("pending lock");
            if let Some(buf) = pending.get_mut(rid) {
                if buf.frames.len() >= MAX_FRAMES_PER_ROUND {
                    return;
                }
                buf.frames.push(FrameEntry {
                    seq,
                    is_inbound,
                    detail: detail.to_string(),
                    session_id: session_id.map(|s| s.to_string()),
                    received_at: Instant::now(),
                });
                return;
            }
        }

        // No round_id yet (before the first round is created) —
        // buffer as orphaned, will be flushed when a round starts
        if round_id.is_none()
            && let Some(sid) = session_id
        {
            let mut orphaned = self.orphaned_frames.lock().expect("orphaned lock");
            orphaned
                .entry(sid.to_string())
                .or_default()
                .push(FrameEntry {
                    seq,
                    is_inbound,
                    detail: detail.to_string(),
                    session_id: Some(sid.to_string()),
                    received_at: Instant::now(),
                });
            return;
        }

        // No session_id either — fallback: spawn DB insert
        let conn = self.conn.clone();
        let round_id = round_id.map(|s| s.to_string());
        let session_id = session_id.map(|s| s.to_string());
        let detail = detail.to_string();
        let dir_str = if is_inbound { "inbound" } else { "outbound" };
        tokio::spawn(async move {
            let _ = frame::ActiveModel {
                round_id: Set(round_id),
                session_id: Set(session_id),
                seq: Set(seq as i32),
                dir: Set(dir_str.to_string()),
                kind: Set("frame".to_string()),
                detail: Set(Some(detail)),
                ..Default::default()
            }
            .insert(&conn)
            .await;
        });
    }

    fn finish_round(&self, round_id: &str, reason: RoundEndReason) -> Result<(), ()> {
        let buffer = {
            let mut pending = self.pending.lock().expect("pending lock");
            pending.remove(round_id)
        };
        let Some(buffer) = buffer else {
            return Ok(());
        };

        self.flush_tx
            .send(FlushEvent { buffer, reason })
            .map_err(|_| ())
    }

    async fn flush_to_db(
        conn: &DatabaseConnection,
        buffer: &RoundBuffer,
        reason: RoundEndReason,
    ) -> Result<(), anyhow::Error> {
        let now = chrono::Local::now().fixed_offset();

        let status = match reason {
            RoundEndReason::Completed => Some("completed".to_string()),
            RoundEndReason::Interrupted => Some("interrupted".to_string()),
        };

        round::ActiveModel {
            id: Set(buffer.round_id.clone()),
            session_id: Set(buffer.session_id.clone().unwrap_or_default()),
            client_info: Set(buffer.client_info.clone()),
            mode: Set(buffer.mode.clone()),
            status: Set(status),
            create_datetime: Set(Some(now)),
            update_datetime: Set(Some(now)),
        }
        .insert(conn)
        .await?;

        let has_input_audio = buffer.step_has("input_audio", |s| s.has_pcm);
        let has_asr_text = buffer.step_has("asr", |s| s.has_text);

        if has_input_audio {
            let io = buffer
                .steps
                .iter()
                .find(|s| s.step == "input_audio")
                .unwrap();
            let wav = pcm_f32_to_wav(&io.pcm, io.sample_rate.unwrap_or(16000));
            let mut meta = serde_json::json!({"format": "wav"});
            let sr = io.sample_rate.unwrap_or(16000) as f64;
            if !io.pcm.is_empty() {
                let dur_ms = (io.pcm.len() as f64 / sr * 1000.0) as i64;
                meta["audio_duration_ms"] = serde_json::json!(dur_ms);
                meta["duration_ms"] = serde_json::json!(dur_ms);
            }
            if let Some(ms) = buffer.elapsed_ms("input_audio") {
                meta["elapsed_ms"] = serde_json::json!(ms);
            }
            round_data::ActiveModel {
                id: Set(gen_id()),
                round_id: Set(buffer.round_id.clone()),
                data_type: Set("input_audio".to_string()),
                data: Set(Some(wav)),
                text: Set(buffer.step_text("asr")),
                metadata: Set(Some(meta)),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        }
        if has_asr_text {
            let confidence = buffer
                .steps
                .iter()
                .find(|s| s.step == "asr")
                .and_then(|s| s.confidence);
            let mut meta = confidence
                .map(|c| serde_json::json!({"confidence": c}))
                .unwrap_or(serde_json::json!({}));
            if let Some(ms) = buffer.elapsed_ms("asr") {
                meta["elapsed_ms"] = serde_json::json!(ms);
            }
            if let Some(ms) = buffer.process_ms("asr") {
                meta["duration_ms"] = serde_json::json!(ms);
            }
            round_data::ActiveModel {
                id: Set(gen_id()),
                round_id: Set(buffer.round_id.clone()),
                data_type: Set("asr".to_string()),
                data: Set(None),
                text: Set(buffer.step_text("asr")),
                metadata: Set(Some(meta)),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        }

        let has_text = buffer.step_has("text", |s| s.has_text);
        if has_text {
            let text = buffer.steps.iter().find(|s| s.step == "text").unwrap();
            let mut meta = serde_json::json!({});
            if let Some(ms) = buffer.elapsed_ms("text") {
                meta["elapsed_ms"] = serde_json::json!(ms);
            }
            if let Some(ms) = buffer.process_ms("text") {
                meta["duration_ms"] = serde_json::json!(ms);
            }
            round_data::ActiveModel {
                id: Set(gen_id()),
                round_id: Set(buffer.round_id.clone()),
                data_type: Set("text".to_string()),
                data: Set(None),
                text: Set(Some(text.text.clone())),
                metadata: Set(Some(meta)),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        }

        if let Some(text) = buffer.step_text("llm") {
            let mut meta = serde_json::json!({});
            if let Some(ms) = buffer.elapsed_ms("llm") {
                meta["elapsed_ms"] = serde_json::json!(ms);
            }
            if let Some(ms) = buffer.process_ms("llm") {
                meta["duration_ms"] = serde_json::json!(ms);
            }
            round_data::ActiveModel {
                id: Set(gen_id()),
                round_id: Set(buffer.round_id.clone()),
                data_type: Set("llm".to_string()),
                data: Set(None),
                text: Set(Some(text)),
                metadata: Set(Some(meta)),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        }

        let has_tts_pcm = buffer.step_has("tts", |s| s.has_pcm);
        let has_tts_text = buffer.step_has("tts", |s| s.has_text);

        if has_tts_pcm {
            let tts = buffer.steps.iter().find(|s| s.step == "tts").unwrap();
            let wav = pcm_f32_to_wav(&tts.pcm, tts.sample_rate.unwrap_or(24000));
            let mut meta = serde_json::json!({
                "format": "wav",
                "sample_rate": tts.sample_rate,
            });
            let sr = tts.sample_rate.unwrap_or(24000) as f64;
            if !tts.pcm.is_empty() {
                meta["audio_duration_ms"] =
                    serde_json::json!((tts.pcm.len() as f64 / sr * 1000.0) as i64);
            }
            if let Some(ms) = buffer.elapsed_ms("tts") {
                meta["elapsed_ms"] = serde_json::json!(ms);
            }
            if let Some(ms) = buffer.process_ms("tts") {
                meta["duration_ms"] = serde_json::json!(ms);
            }
            round_data::ActiveModel {
                id: Set(gen_id()),
                round_id: Set(buffer.round_id.clone()),
                data_type: Set("tts".to_string()),
                data: Set(Some(wav)),
                text: Set(buffer.step_text("tts")),
                metadata: Set(Some(meta)),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        } else if has_tts_text {
            let mut meta = serde_json::json!({});
            if let Some(ms) = buffer.elapsed_ms("tts") {
                meta["elapsed_ms"] = serde_json::json!(ms);
            }
            if let Some(ms) = buffer.process_ms("tts") {
                meta["duration_ms"] = serde_json::json!(ms);
            }
            round_data::ActiveModel {
                id: Set(gen_id()),
                round_id: Set(buffer.round_id.clone()),
                data_type: Set("tts".to_string()),
                data: Set(None),
                text: Set(buffer.step_text("tts")),
                metadata: Set(Some(meta)),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        }

        for entry in &buffer.frames {
            let dir_str = if entry.is_inbound {
                "inbound"
            } else {
                "outbound"
            };
            let elapsed_ms = entry
                .received_at
                .saturating_duration_since(buffer.round_start_at)
                .as_micros() as i64;
            frame::ActiveModel {
                round_id: Set(Some(buffer.round_id.clone())),
                session_id: Set(entry.session_id.clone()),
                seq: Set(entry.seq as i32),
                dir: Set(dir_str.to_string()),
                kind: Set("frame".to_string()),
                detail: Set(Some(entry.detail.clone())),
                elapsed_us: Set(Some(elapsed_ms)),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        }

        Ok(())
    }
}

impl RecordCollector {
    pub async fn on_session_start(&self, session_id: &str) {
        session::ActiveModel {
            id: Set(session_id.to_string()),
            ..Default::default()
        }
        .insert(&self.conn)
        .await
        .ok();
    }

    pub fn on_round_start(&self, ctx: &RoundStartContext) {
        self.start_round(
            ctx.round_id.clone(),
            ctx.session_id.clone(),
            ctx.client_info.clone(),
            ctx.mode.as_str(),
        );

        // Flush orphaned frames to the new round,
        // keeping their original received_at timestamps (monotonic clock)
        if let Some(ref session_id) = ctx.session_id {
            let mut orphaned = self.orphaned_frames.lock().expect("orphaned lock");
            if let Some(mut frames) = orphaned.remove(session_id) {
                drop(orphaned);
                if !frames.is_empty() {
                    let mut pending = self.pending.lock().expect("pending lock");
                    if let Some(buf) = pending.get_mut(&ctx.round_id) {
                        buf.frames.append(&mut frames);
                    }
                }
            }
        }
    }

    pub fn on_text_input(&self, ctx: &TextInputContext) {
        self.collect_text(&ctx.round_id, &ctx.text);
    }

    pub fn on_asr(&self, ctx: &AsrContext) {
        self.collect_asr(
            &ctx.round_id,
            ctx.voice_pcm.clone(),
            ctx.sample_rate,
            ctx.text.clone(),
            ctx.confidence,
        );
    }

    pub fn on_asr_complete(&self, round_id: &str) {
        self.align_round_start(round_id);
    }

    pub fn on_llm_delta(&self, ctx: &LlmDeltaContext) {
        self.collect_llm_text(&ctx.round_id, &ctx.text);
    }

    pub fn on_tts_delta(&self, ctx: &TtsDeltaContext) {
        self.collect_tts(&ctx.round_id, &ctx.text, ctx.raw_pcm.clone());
    }

    pub fn on_frame(&self, ctx: &FrameContext) {
        let is_inbound = matches!(ctx.direction, FrameDirection::Inbound);
        self.record_frame(
            ctx.round_id.as_deref(),
            ctx.session_id.as_deref(),
            ctx.seq,
            is_inbound,
            &ctx.detail,
        );
    }

    pub async fn on_round_end(&self, ctx: &RoundEndContext) -> Result<(), anyhow::Error> {
        self.finish_round(&ctx.round_id, ctx.reason.clone())
            .map_err(|_| anyhow::anyhow!("flush channel closed"))
    }
}
