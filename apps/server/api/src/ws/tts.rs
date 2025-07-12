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
            let (audio, took) = instance.synth(text, Voice::Zf001(1)).await.unwrap();
            let mut encoder = opus::Encoder::new(
                SAMPLE_RATE,
                opus::Channels::Mono,
                opus::Application::LowDelay,
            )
            .unwrap();
            let len = audio.len();
            let size = calcalute_tts_packet_size(SAMPLE_RATE, DELAY_MILLIS) as usize;
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

//Sampling rate of input signal (Hz) This must be one of 8000, 12000, 16000, 24000, or 48000.
//采样率
pub static SAMPLE_RATE: u32 = 24000;
//WebSocket 发送间隔 ≈ 帧长度
//one frame (2.5, 5, 10, 20, 40 or 60 ms)
pub static DELAY_MILLIS: u64 = 60;

pub fn calcalute_tts_packet_size(sample_rate: u32, delay_millis: u64) -> usize {
    // 16000Hz * 1 channel * 60 ms / 1000 = 960
    (sample_rate as usize) * 1 * (delay_millis as usize) / 1000
}
