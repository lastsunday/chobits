use std::path::Path;
use std::{sync::Arc, thread};

use api::{
    common::ModelError,
    config::{TtsModel, audio::AudioConfig, tts::TtsConfig},
    tts::TtsFactory,
};
use futures::{Stream, executor::block_on};
use tokio::sync::mpsc::channel;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tracing::info;
use tracing_test::traced_test;
use wavers::write;

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_default --ignored --nocapture
async fn test_tts_default() -> anyhow::Result<()> {
    const ENCODE_SAMPLE_RATE: u32 = 16000;
    // 16000Hz * 1 channel * 20 ms / 1000 = 320
    const MONO_20MS: usize = ENCODE_SAMPLE_RATE as usize * 20 / 1000;
    let size = MONO_20MS;
    TtsFactory::init(
        Arc::new(TtsConfig {
            model: Some(TtsModel::Mute),
            ..Default::default()
        }),
        Arc::new(AudioConfig {
            ..Default::default()
        }),
    )
    .await?;
    let tts = TtsFactory::global().default();
    let text_stream = tts_stream(String::from("我不知道将去何方，但我已经在路上。"));
    let mut tts_stream = tts.stream(Box::pin(text_stream)).await;

    let mut audio: Vec<Vec<u8>> = Vec::new();
    while let Some(data) = tts_stream.next().await {
        match data {
            Ok(data) => {
                info!("{:?}", data.text);
                match data.audio {
                    Some(data) => {
                        audio.append(&mut data.clone());
                    }
                    None => {
                        audio.append(&mut vec![]);
                    }
                }
            }
            Err(e) => {
                panic!("{:?}", e);
            }
        }
    }
    let audio_len = audio.len();
    info!("audio len = {}", audio_len);

    // 4. decode opus packet to pcm data
    let mut decoder = opus_rs::OpusDecoder::new(ENCODE_SAMPLE_RATE as i32, 1).unwrap();
    let mut decode_data: Vec<f32> = Vec::new();
    for n in 0..audio_len {
        let mut samples = vec![0f32; size];
        let data = audio.get(n).unwrap();
        let len = decoder.decode(data, size, &mut samples).unwrap();
        decode_data.append(&mut samples[..len].to_vec());
    }

    // the follow code is output wav file to test
    info!("decode_data len = {}", decode_data.len());
    std::fs::create_dir_all("./test_data")?;
    let fp = "./test_data/test_tts_default.wav";
    let sr: i32 = 16000;
    let _ = write(fp, &decode_data, sr, 1);
    Ok(())
}

#[tokio::test]
#[traced_test]
/// cargo test --test tts_test -- test_tts_mute --nocapture
async fn test_tts_mute() -> anyhow::Result<()> {
    TtsFactory::init(
        Arc::new(TtsConfig {
            model: Some(TtsModel::Mute),
            ..Default::default()
        }),
        Arc::new(AudioConfig {
            ..Default::default()
        }),
    )
    .await?;
    let tts = TtsFactory::global().default();
    let text_stream = tts_stream(String::from("我不知道将去何方，但我已经在路上。"));
    let mut tts_stream = tts.stream(Box::pin(text_stream)).await;
    let mut audio: Vec<Vec<u8>> = Vec::new();
    while let Some(data) = tts_stream.next().await {
        match data {
            Ok(data) => {
                assert_eq!(data.text, "我不知道将去何方，但我已经在路上。");
                match data.audio {
                    Some(data) => {
                        audio.append(&mut data.clone());
                    }
                    None => {
                        audio.append(&mut vec![]);
                    }
                }
            }
            Err(e) => {
                panic!("{:?}", e);
            }
        }
    }
    let audio_len = audio.len();
    assert_eq!(0, audio_len);
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_pocket --ignored --nocapture
/// 先下载模型和参考音频：
///   cargo run --bin chobits-server -- downloader install tts pocket_tts default --all
///   cargo run --bin chobits-server -- downloader install reference audio xiyangyang --all
///   cargo run --bin chobits-server -- downloader install reference audio bria --all
async fn test_tts_pocket() -> anyhow::Result<()> {
    let ws_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let model_path = ws_root
        .join("data/tts/model/pocket/default/")
        .to_string_lossy()
        .into_owned();

    let ref_wav = ws_root.join("data/tts/reference/test_wavs/bria.wav");

    let tts = TtsFactory::create_model(
        &TtsConfig {
            model: Some(TtsModel::PocketTts),
            path: Some(model_path),
            reference_prompt_wav_path: Some(ref_wav.to_string_lossy().into()),
            ..Default::default()
        },
        &AudioConfig {
            output_sample_rate: Some(16000),
            output_channel: Some(1),
            output_frame_duration: Some(20),
            ..Default::default()
        },
    )
    .await?;

    let text = "Today as always, men fall into two groups: slaves and free men. Whoever \
        does not have two-thirds of his day for himself, is a slave, whatever \
        he may be: a statesman, a businessman, an official, or a scholar. \
        Friends fell out often because life was changing so fast. The easiest \
        thing in the world was to lose touch with someone.";
    let text_stream = tts_stream(String::from(text));
    let mut tts_stream = tts.stream(Box::pin(text_stream)).await;

    let mut all_packets: Vec<Vec<u8>> = Vec::new();
    let mut raw_pcm: Option<(Vec<f32>, i32)> = None;
    while let Some(data) = tts_stream.next().await {
        match data {
            Ok(data) => {
                info!("text: {}", data.text);
                if let Some(packets) = data.audio {
                    all_packets.extend(packets);
                }
                if raw_pcm.is_none() {
                    raw_pcm = data.raw_pcm;
                }
            }
            Err(e) => panic!("{:?}", e),
        }
    }

    assert!(
        !all_packets.is_empty(),
        "Expected audio packets from PocketTTS"
    );

    // Save raw PCM (skip Opus)
    if let Some((samples, sr)) = &raw_pcm {
        std::fs::create_dir_all("./test_data")?;
        let _ = wavers::write("./test_data/test_tts_pocket_raw.wav", samples, *sr, 1);
        info!("raw PCM: {} samples at {}Hz", samples.len(), sr);
    }

    let decode_fs = 320; // 16000Hz * 1ch * 20ms / 1000
    let mut decoder = opus_rs::OpusDecoder::new(16000, 1).unwrap();
    let mut decoded = Vec::new();
    for packet in &all_packets {
        let mut samples = vec![0f32; decode_fs];
        if let Ok(len) = decoder.decode(packet, decode_fs, &mut samples) {
            decoded.extend_from_slice(&samples[..len]);
        }
    }
    assert!(decoded.len() > 1000, "Decoded audio too short");
    info!("decoded {} PCM samples", decoded.len());

    std::fs::create_dir_all("./test_data")?;
    let _ = wavers::write("./test_data/test_tts_pocket.wav", &decoded, 16000, 1);
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// cargo test --test tts_test -- test_tts_vits --ignored --nocapture
/// 先下载模型：
///   cargo run --bin chobits-server -- downloader install tts vits melo-tts-zh_en --all
async fn test_tts_vits() -> anyhow::Result<()> {
    let ws_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let model_path = ws_root
        .join("data/tts/model/vits/melo-tts-zh_en/")
        .to_string_lossy()
        .into_owned();

    let tts = TtsFactory::create_model(
        &TtsConfig {
            model: Some(TtsModel::Vits),
            path: Some(model_path),
            ..Default::default()
        },
        &AudioConfig {
            output_sample_rate: Some(16000),
            output_channel: Some(1),
            output_frame_duration: Some(20),
            ..Default::default()
        },
    )
    .await?;

    let text = "对于有媒体报道称，“特朗普说，如果中国不在霍尔木兹海峡护航问题上提供协助，他将推迟访华”，林剑说，中方注意到美方已就媒体不实报道公开作出澄清，表示有关报道是完全错误的，强调访问与霍尔木兹海峡通航问题无关。
,This is a 中英文的 text to speech 测试例子。";
    let text_stream = tts_stream(String::from(text));
    let mut tts_stream = tts.stream(Box::pin(text_stream)).await;

    let mut all_packets: Vec<Vec<u8>> = Vec::new();
    let mut raw_pcm: Option<(Vec<f32>, i32)> = None;
    while let Some(data) = tts_stream.next().await {
        match data {
            Ok(data) => {
                info!("text: {}", data.text);
                if let Some(packets) = data.audio {
                    all_packets.extend(packets);
                }
                if raw_pcm.is_none() {
                    raw_pcm = data.raw_pcm;
                }
            }
            Err(e) => panic!("{:?}", e),
        }
    }

    assert!(
        !all_packets.is_empty(),
        "Expected audio packets from VitsTTS"
    );

    // Save raw PCM (skip Opus)
    if let Some((samples, sr)) = &raw_pcm {
        std::fs::create_dir_all("./test_data")?;
        let _ = wavers::write("./test_data/test_tts_vits_raw.wav", samples, *sr, 1);
        info!("raw PCM: {} samples at {}Hz", samples.len(), sr);
    }

    let decode_fs = 320; // 16000Hz * 1ch * 20ms / 1000
    let mut decoder = opus_rs::OpusDecoder::new(16000, 1).unwrap();
    let mut decoded = Vec::new();
    for packet in &all_packets {
        let mut samples = vec![0f32; decode_fs];
        if let Ok(len) = decoder.decode(packet, decode_fs, &mut samples) {
            decoded.extend_from_slice(&samples[..len]);
        }
    }
    assert!(decoded.len() > 1000, "Decoded audio too short");
    info!("decoded {} PCM samples", decoded.len());

    std::fs::create_dir_all("./test_data")?;
    let _ = wavers::write("./test_data/test_tts_vits.wav", &decoded, 16000, 1);
    Ok(())
}

fn tts_stream(
    text: String,
) -> impl Stream<Item = core::result::Result<String, ModelError>> + Unpin + Send + 'static {
    let (tx, rx) = channel::<core::result::Result<String, ModelError>>(10);
    thread::spawn(move || {
        block_on(async move {
            let _ = tx.send(Ok(text)).await;
            drop(tx);
        })
    });
    ReceiverStream::new(rx)
}
