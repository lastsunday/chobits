use std::{cmp, sync::Arc};

use futures::Stream;
use kokoro_tts::{KokoroTts, Voice};
use tokio::sync::{Mutex, mpsc::channel};
use tokio_stream::wrappers::ReceiverStream;

pub trait Tts {
    fn output(&self, text: String) -> impl Stream<Item = Vec<u8>> + Unpin + Send;
}

#[derive(Clone)]
pub struct TtsKokoro {
    instance: Arc<Mutex<KokoroTts>>,
}

impl TtsKokoro {
    pub fn new(instance: Arc<Mutex<KokoroTts>>) -> Self {
        Self { instance }
    }
}

impl Tts for TtsKokoro {
    fn output(&self, text: String) -> impl Stream<Item = Vec<u8>> + Unpin + Send {
        let text = text.clone();
        let (tx, rx) = channel(1);
        let instance = self.instance.clone();
        tokio::spawn(async move {
            let instance = instance.lock().await;
            let (audio, took) = instance.synth(text, Voice::Zf039(1)).await.unwrap();
            let mut encoder =
                opus::Encoder::new(24000, opus::Channels::Mono, opus::Application::LowDelay)
                    .unwrap();
            let len = audio.len();
            // 24000Hz * 1 channel * 60 ms / 1000 = 1440
            let size = 1440;
            let count = len / size;
            for n in 1..count {
                let start = (n - 1) * size;
                let end = cmp::min(n * size, len);
                let packet = encoder.encode_vec_float(&audio[start..end], size).unwrap();
                match tx.send(packet).await {
                    Ok(_) => (),
                    Err(error) => {
                        tracing::info!("output packet error = {}", error);
                        break;
                    }
                }
            }
            drop(tx);
        });
        ReceiverStream::new(rx)
    }
}
