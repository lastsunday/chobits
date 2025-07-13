use std::{cmp, sync::Arc};

use futures::Stream;
pub use sherpa_rs::tts::KokoroTts;
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
/*
https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/kokoro.html
wget https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_1.tar.bz2
tar xf kokoro-multi-lang-v1_1.tar.bz2
rm kokoro-multi-lang-v1_1.tar.bz2
*/
impl Tts for TtsKokoro {
    fn output(&self, text: String) -> impl Stream<Item = Vec<u8>> + Unpin + Send {
        let text = text.clone();
        let (tx, rx) = channel(1);
        let instance = self.instance.clone();
        tokio::spawn(async move {
            let mut instance = instance.lock().await;
            //0->af_alloy, 1->af_aoede, 2->af_bella, 3->af_heart, 4->af_jessica,
            //5->af_kore, 6->af_nicole, 7->af_nova, 8->af_river, 9->af_sarah,
            //10->af_sky, 11->am_adam, 12->am_echo, 13->am_eric, 14->am_fenrir,
            //15->am_liam, 16->am_michael, 17->am_onyx, 18->am_puck, 19->am_santa,
            //20->bf_alice, 21->bf_emma, 22->bf_isabella, 23->bf_lily, 24->bm_daniel,
            //25->bm_fable, 26->bm_george, 27->bm_lewis, 28->ef_dora, 29->em_alex,
            //30->ff_siwis, 31->hf_alpha, 32->hf_beta, 33->hm_omega, 34->hm_psi,
            //35->if_sara, 36->im_nicola, 37->jf_alpha, 38->jf_gongitsune,
            //39->jf_nezumi, 40->jf_tebukuro, 41->jm_kumo,
            //42->pf_dora, 43->pm_alex, 44->pm_santa, 45->zf_xiaobei, 46->zf_xiaoni,
            //47->zf_xiaoxiao, 48->zf_xiaoyi,49->zm_yunjian, 50->zm_yunxi,
            //51->zm_yunxia, 52->zm_yunyang,
            let sid = 47;
            let audio = instance.create(&text, sid, 1.0).unwrap();
            let sample = audio.samples;
            let mut encoder = opus::Encoder::new(
                SAMPLE_RATE,
                opus::Channels::Mono,
                opus::Application::LowDelay,
            )
            .unwrap();
            tracing::info!(
                "get_sample_rate = {:?}, get_bitrate = {:?}, get_final_range = {:?}, get_vbr_constraint = {:?}, get_vbr = {:?}, get_bandwidth = {:?}, get_lookahead = {:?}",
                encoder.get_sample_rate(),
                encoder.get_bitrate(),
                encoder.get_final_range(),
                encoder.get_vbr_constraint(),
                encoder.get_vbr(),
                encoder.get_bandwidth(),
                encoder.get_lookahead()
            );
            let len = sample.len();
            let size = calcalute_tts_packet_size(SAMPLE_RATE, DELAY_MILLIS) as usize;
            let count = len / size;
            for n in 1..count {
                let start = (n - 1) * size;
                let end = cmp::min(n * size, len);
                let packet = encoder.encode_vec_float(&sample[start..end], size).unwrap();
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
