use std::thread;
use std::{cmp, sync::Arc};

use crate::config;
use crate::ws::common::ModelError;
use futures::Stream;
use futures::executor::block_on;
pub use sherpa_rs::tts::KokoroTts;
use tokio::sync::{Mutex, mpsc::channel};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

pub trait Tts {
    fn output(&self, text: String) -> impl Stream<Item = Vec<u8>> + Unpin + Send;
    fn output_stream(
        &self,
        text_stream: impl Stream<Item = core::result::Result<String, ModelError>>
        + Unpin
        + Send
        + 'static,
    ) -> impl Stream<Item = core::result::Result<TtsData, TtsError>> + Unpin + Send + 'static;
}

pub struct TtsData {
    pub audio: Vec<Vec<u8>>,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("init error")]
    Init,
    #[error("encode error")]
    Encode,
    #[error("text error")]
    Text,
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
        let (tx, rx) = channel(10);
        let instance = self.instance.clone();
        tokio::spawn(async move {
            let audio_config = config::get().audio();
            let sample_rate = audio_config.output_sample_rate();
            let channel = audio_config.output_channel();
            let frame_duration = audio_config.output_frame_duration();
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
                sample_rate,
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
            let size = calcalute_tts_packet_size(sample_rate, channel, frame_duration);
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

    fn output_stream(
        &self,
        mut text_stream: impl Stream<Item = core::result::Result<String, ModelError>>
        + Unpin
        + Send
        + 'static,
    ) -> impl Stream<Item = core::result::Result<TtsData, TtsError>> + Unpin + Send + 'static {
        let (tx, rx) = channel(10);
        let instance = self.instance.clone();
        thread::spawn(move || {
            block_on(async move {
                while let Some(text) = text_stream.next().await {
                    let instance = instance.clone();
                    let tx = tx.clone();
                    match text {
                        Ok(text) => {
                            tracing::info!("[TTS] receive, text = {}", text);
                            let audio_config = config::get().audio();
                            let sample_rate = audio_config.output_sample_rate();
                            let channel = audio_config.output_channel();
                            let frame_duration = audio_config.output_frame_duration();
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
                            let sid = 50;

                            let audio = instance.create(&text, sid, 1.0).unwrap();
                            let sample = audio.samples;
                            let mut encoder = opus::Encoder::new(
                                sample_rate,
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
                            let size =
                                calcalute_tts_packet_size(sample_rate, channel, frame_duration);
                            let count = len / size;
                            let mut audio = Vec::new();
                            for n in 1..count {
                                let start = (n - 1) * size;
                                let end = cmp::min(n * size, len);
                                let packet =
                                    encoder.encode_vec_float(&sample[start..end], size).unwrap();
                                audio.push(packet);
                            }
                            let data = TtsData {
                                audio,
                                text: text.to_string(),
                            };
                            if let Err(e) = tx.send(Ok(data)).await {
                                tracing::info!("output packet error = {}", e);
                            } else {
                                tracing::info!("[TTS] encode and send audio success");
                            }
                        }
                        Err(_e) => {
                            if let Err(e) = tx.send(Err(TtsError::Text)).await {
                                tracing::error!("send error failure = {}", e);
                            }
                        }
                    }
                }
                drop(tx);
            })
        });
        ReceiverStream::new(rx)
    }
}

pub fn calcalute_tts_packet_size(sample_rate: u32, channel: u32, delay_millis: u64) -> usize {
    sample_rate as usize * channel as usize * delay_millis as usize / 1000
}
