use std::thread;

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
    // 16000Hz * 1 channel * 60 ms / 1000 = 960
    const MONO_60MS: usize = ENCODE_SAMPLE_RATE as usize * 60 / 1000;
    let size = MONO_60MS;
    TtsFactory::init(
        TtsConfig {
            model: Some(TtsModel::Voxcpm),
            path: Some(String::from("data/tts/model/openbmb/VoxCPM-0.5B/")),
            reference_prompt_text: Some(String::from(
                "一定被灰太狼给吃了，我已经为他准备好了花圈了",
            )),
            reference_prompt_wav_path: Some(String::from("file://data/tts/reference/voice_05.wav")),
        },
        AudioConfig {
            input_sample_rate: Some(16000),
            input_frame_duration: Some(60_u64),
            input_channel: Some(1),
            output_sample_rate: Some(16000),
            output_channel: Some(1),
            output_frame_duration: Some(60_u64),
        },
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
                audio.append(&mut data.audio.clone());
            }
            Err(e) => {
                panic!("{:?}", e);
            }
        }
    }
    let audio_len = audio.len();
    info!("audio len = {}", audio_len);

    // 4. decode opus packet to pcm data
    let mut decoder = opus::Decoder::new(ENCODE_SAMPLE_RATE, opus::Channels::Mono).unwrap();
    let mut decode_data: Vec<f32> = Vec::new();
    for n in 0..audio_len {
        let mut samples = vec![0f32; size];
        let data = audio.get(n).unwrap();
        let len = decoder.decode_float(data, &mut samples, false).unwrap();
        decode_data.append(&mut samples[..len].to_vec());
    }

    // the follow code is output wav file to test
    info!("decode_data len = {}", decode_data.len());
    let fp = "./test_tts_default.wav";
    let sr: i32 = 16000;
    let _ = write(fp, &decode_data, sr, 1);
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
