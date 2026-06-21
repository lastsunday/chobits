use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use sea_orm::{ActiveValue::Set, DatabaseConnection, entity::prelude::*};
use serde_json::Value as JsonValue;

use super::wav::pcm_f32_to_wav;
use entity::frame;
use entity::round;
use entity::round_data;
use framework::id::gen_id;

use crate::ws::session::trace::{Direction, TraceEntry};

const MAX_FRAMES_PER_ROUND: usize = 5000;

pub struct RoundBuffer {
    pub round_id: String,
    pub user_id: Option<String>,
    pub client_info: Option<JsonValue>,

    pub input_audio_wav: Option<Vec<u8>>,
    pub asr_text: Option<String>,
    pub asr_confidence: Option<f32>,

    pub llm_text: String,
    pub tts_text: String,
    pub tts_raw_pcm: Option<(Vec<f32>, u32)>,

    pub frames: Vec<TraceEntry>,
    pub status: String,
}

#[derive(Clone)]
pub struct RecordCollector {
    conn: DatabaseConnection,
    pending: Arc<StdMutex<HashMap<String, RoundBuffer>>>,
}

impl RecordCollector {
    pub fn new(conn: DatabaseConnection) -> Self {
        Self {
            conn,
            pending: Arc::new(StdMutex::new(HashMap::new())),
        }
    }

    pub fn conn(&self) -> &DatabaseConnection {
        &self.conn
    }

    pub fn start_round(
        &self,
        round_id: String,
        user_id: Option<String>,
        client_info: Option<JsonValue>,
    ) {
        let mut pending = self.pending.lock().expect("pending lock");
        pending.insert(
            round_id.clone(),
            RoundBuffer {
                round_id,
                user_id,
                client_info,
                input_audio_wav: None,
                asr_text: None,
                asr_confidence: None,
                llm_text: String::new(),
                tts_text: String::new(),
                tts_raw_pcm: None,
                frames: Vec::new(),
                status: "processing".to_string(),
            },
        );
    }

    pub fn collect_asr(
        &self,
        round_id: &str,
        voice_pcm: Vec<f32>,
        sample_rate: u32,
        text: String,
        confidence: f32,
    ) {
        let wav = pcm_f32_to_wav(&voice_pcm, sample_rate);
        let mut pending = self.pending.lock().expect("pending lock");
        if let Some(buf) = pending.get_mut(round_id) {
            buf.input_audio_wav = Some(wav);
            buf.asr_text = Some(text);
            buf.asr_confidence = Some(confidence);
        }
    }

    pub fn collect_llm_text(&self, round_id: &str, text: &str) {
        let mut pending = self.pending.lock().expect("pending lock");
        if let Some(buf) = pending.get_mut(round_id) {
            buf.llm_text.push_str(text);
        }
    }

    pub fn collect_tts(&self, round_id: &str, text: &str, raw_pcm: Option<(Vec<f32>, u32)>) {
        let mut pending = self.pending.lock().expect("pending lock");
        if let Some(buf) = pending.get_mut(round_id) {
            buf.tts_text.push_str(text);
            if raw_pcm.is_some() {
                buf.tts_raw_pcm = raw_pcm;
            }
        }
    }

    pub fn record_frame(&self, round_id: &str, seq: u64, dir: Direction, detail: &str) {
        if dir == Direction::Internal {
            return;
        }
        let mut pending = self.pending.lock().expect("pending lock");
        if let Some(buf) = pending.get_mut(round_id) {
            if buf.frames.len() >= MAX_FRAMES_PER_ROUND {
                return;
            }
            buf.frames.push(TraceEntry {
                ts: String::new(),
                seq,
                dir,
                kind: crate::ws::session::trace::TraceKind::InboundFrame,
                detail: detail.to_string(),
            });
        }
    }

    pub async fn finish_round(&self, round_id: &str) -> Result<(), anyhow::Error> {
        let buffer = {
            let mut pending = self.pending.lock().expect("pending lock");
            pending.remove(round_id)
        };
        let Some(buffer) = buffer else {
            return Ok(());
        };

        Self::flush_to_db(&self.conn, &buffer).await
    }

    async fn flush_to_db(
        conn: &DatabaseConnection,
        buffer: &RoundBuffer,
    ) -> Result<(), anyhow::Error> {
        let now = chrono::Local::now().fixed_offset();

        let llm_text = (!buffer.llm_text.is_empty()).then_some(buffer.llm_text.as_str());
        let tts_text = (!buffer.tts_text.is_empty()).then_some(buffer.tts_text.as_str());

        // Insert round
        round::ActiveModel {
            id: Set(buffer.round_id.clone()),
            user_id: Set(buffer.user_id.clone()),
            client_info: Set(buffer.client_info.clone()),
            create_datetime: Set(Some(now)),
            update_datetime: Set(Some(now)),
        }
        .insert(conn)
        .await?;

        // Insert round_data: input_audio + asr
        if let Some(wav) = &buffer.input_audio_wav {
            let meta = serde_json::json!({
                "format": "wav",
            });
            round_data::ActiveModel {
                id: Set(gen_id()),
                round_id: Set(buffer.round_id.clone()),
                data_type: Set("input_audio".to_string()),
                data: Set(Some(wav.clone())),
                text: Set(buffer.asr_text.clone()),
                metadata: Set(Some(meta)),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        } else if let Some(text) = &buffer.asr_text {
            let meta = buffer
                .asr_confidence
                .map(|c| serde_json::json!({"confidence": c}));
            round_data::ActiveModel {
                id: Set(gen_id()),
                round_id: Set(buffer.round_id.clone()),
                data_type: Set("asr".to_string()),
                data: Set(None),
                text: Set(Some(text.clone())),
                metadata: Set(meta),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        }

        // Insert round_data: llm
        if let Some(text) = llm_text {
            round_data::ActiveModel {
                id: Set(gen_id()),
                round_id: Set(buffer.round_id.clone()),
                data_type: Set("llm".to_string()),
                data: Set(None),
                text: Set(Some(text.to_string())),
                metadata: Set(None),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        }

        // Insert round_data: tts
        if let Some((pcm, sample_rate)) = &buffer.tts_raw_pcm {
            let wav = pcm_f32_to_wav(pcm, *sample_rate);
            let meta = serde_json::json!({
                "format": "wav",
                "sample_rate": sample_rate,
            });
            round_data::ActiveModel {
                id: Set(gen_id()),
                round_id: Set(buffer.round_id.clone()),
                data_type: Set("tts".to_string()),
                data: Set(Some(wav)),
                text: Set(tts_text.map(String::from)),
                metadata: Set(Some(meta)),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        } else if let Some(text) = tts_text {
            round_data::ActiveModel {
                id: Set(gen_id()),
                round_id: Set(buffer.round_id.clone()),
                data_type: Set("tts".to_string()),
                data: Set(None),
                text: Set(Some(text.to_string())),
                metadata: Set(None),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        }

        // Insert frames
        for entry in &buffer.frames {
            let dir_str = match entry.dir {
                Direction::Inbound => "inbound",
                Direction::Outbound => "outbound",
                Direction::Internal => continue,
            };
            frame::ActiveModel {
                round_id: Set(buffer.round_id.clone()),
                seq: Set(entry.seq as i32),
                dir: Set(dir_str.to_string()),
                kind: Set(format!("{:?}", entry.kind)),
                detail: Set(Some(entry.detail.clone())),
                ..Default::default()
            }
            .insert(conn)
            .await?;
        }

        Ok(())
    }
}
