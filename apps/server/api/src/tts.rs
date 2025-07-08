use std::{cmp, time::Duration};

use axum::{body::Bytes, extract::ws::Message};
use futures_util::{Sink, SinkExt};
use kokoro_tts::{KokoroTts, Voice};
use service::chobits::message::tts::{TtsMessage, TtsState};
use tokio::time::{Instant, sleep_until};

pub struct Tts {
    instance: KokoroTts,
}

impl Tts {
    pub async fn new() -> Self {
        Self {
            instance: KokoroTts::new("kokoro-v1.1-zh.onnx", "voices-v1.1-zh.bin")
                .await
                .unwrap(),
        }
    }

    pub async fn say<W>(&mut self, write: &mut W, text: String)
    where
        W: Sink<Message> + Unpin,
    {
        let result: String = serde_json::to_string(&TtsMessage::new(
            Some(TtsState::SentenceStart),
            Some(text.clone()),
        ))
        .unwrap();
        write.send(Message::Text(result.clone().into())).await;

        let result: String =
            serde_json::to_string(&TtsMessage::new(Some(TtsState::Start), None)).unwrap();
        write.send(Message::Text(result.clone().into())).await;

        let (audio, took) = self.instance.synth(text, Voice::Zf038(1)).await.unwrap();
        let mut encoder =
            opus::Encoder::new(24000, opus::Channels::Mono, opus::Application::LowDelay).unwrap();
        let len = audio.len();
        // 24000Hz * 1 channel * 60 ms / 1000 = 1440
        let size = 1440;
        let count = len / size;
        let mut latest_time = Instant::now();
        for n in 1..count {
            let start = (n - 1) * size;
            let end = cmp::min(n * size, len);
            tracing::info!("start = {}, end = {}", start, end);
            let packet = encoder.encode_vec_float(&audio[start..end], size).unwrap();
            let now = Instant::now();
            let offset = now - latest_time + Duration::from_millis(60);
            sleep_until(now + offset).await;
            write.send(Message::Binary(packet.into())).await;
            latest_time = Instant::now();
        }
        write.send(Message::Binary(Bytes::new())).await;

        let result: String =
            serde_json::to_string(&TtsMessage::new(Some(TtsState::SentenceEnd), None)).unwrap();
        write.send(Message::Text(result.clone().into())).await;

        let result: String =
            serde_json::to_string(&TtsMessage::new(Some(TtsState::Stop), None)).unwrap();
        write.send(Message::Text(result.clone().into())).await;
    }
}
