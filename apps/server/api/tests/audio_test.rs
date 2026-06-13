use api::util::audio::pcm_decode;
use std::cmp;
use std::path::PathBuf;
use tracing::info;
use tracing_test::traced_test;
use wavers::write;

#[tokio::test]
#[traced_test]
/// cargo test --test audio_test -- test_audio_encode_decode --nocapture
async fn test_audio_encode_decode() {
    // 1. get wav file
    let wav_file: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "resources",
        "test",
        "samples_jfk.wav",
    ]
    .iter()
    .collect();
    info!("{}", wav_file.display());
    // 2. get pcm data
    let (pcm_data, sample_rate) = pcm_decode(wav_file).unwrap();
    info!(
        "pcm_data len = {},sample_rate = {}",
        pcm_data.len(),
        sample_rate
    );
    // the follow code is output wav file to test
    let fp = "./pcm_decode_data.wav";
    let sr: i32 = 16000;
    let _ = write(fp, &pcm_data, sr, 1);

    const ENCODE_SAMPLE_RATE: u32 = 16000;
    let mut encoder = opus_rs::OpusEncoder::new(
        ENCODE_SAMPLE_RATE as i32,
        1,
        opus_rs::Application::Audio,
    )
    .unwrap();

    // 16000Hz * 1 channel * 60 ms / 1000 = 960
    const MONO_60MS: usize = ENCODE_SAMPLE_RATE as usize * 60 / 1000;
    let size = MONO_60MS;
    info!("size = {}", size);
    let len = pcm_data.len();
    let mut count = len / size;
    if len % size > 0 {
        count += 1;
    }
    info!("count = {}", count);
    let mut audio: Vec<Vec<u8>> = Vec::new();

    // 3. encode wav to opus packet
    for n in 0..count {
        let start = n * size;
        let end = cmp::min((n + 1) * size, len);
        //info!("start = {},end = {}", start, end);
        let mut packet = vec![0u8; 4000];
        let encoded_len = encoder.encode(&pcm_data[start..end], size, &mut packet).unwrap();
        packet.truncate(encoded_len);
        // info!("packet len = {}", packet.len());
        audio.push(packet);
    }
    let audio_len = audio.len();
    info!("audio len = {}", audio_len);

    // 4. decode opus packet to pcm data
    let mut decoder = opus_rs::OpusDecoder::new(sample_rate as i32, 1).unwrap();
    let mut decode_data: Vec<f32> = Vec::new();
    for n in 0..audio_len {
        let mut samples = vec![0f32; size];
        let data = audio.get(n).unwrap();
        let len = decoder.decode(data, size, &mut samples).unwrap();
        decode_data.append(&mut samples[..len].to_vec());
    }

    // the follow code is output wav file to test
    info!("decode_data len = {}", decode_data.len());
    let fp = "./after_decode.wav";
    let sr: i32 = 16000;
    let _ = write(fp, &decode_data, sr, 1);
}
