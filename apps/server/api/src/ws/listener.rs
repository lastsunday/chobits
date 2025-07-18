use sherpa_rs::{
    sense_voice::{SenseVoiceConfig, SenseVoiceRecognizer},
    vad::{Vad, VadConfig},
};

pub struct Listener {
    voice_data: Box<Vec<f32>>,
    vad: Box<Vad>,
    recognizer: Box<SenseVoiceRecognizer>,
}

impl Default for Listener {
    fn default() -> Self {
        let config = VadConfig {
            //wget https://huggingface.co/deepghs/silero-vad-onnx/resolve/main/silero_vad.onnx
            model: "silero_vad.onnx".into(),
            min_silence_duration: 0.05,
            min_speech_duration: 0.05,
            max_speech_duration: 0.05,
            threshold: 0.05,
            window_size: 512 as i32,
            num_threads: Some(4),
            ..Default::default()
        };
        let vad = Vad::new(config, 3.0).unwrap();
        let config = SenseVoiceConfig {
            model: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/model.onnx".into(),
            tokens: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/tokens.txt".into(),
            language: String::from("auto"),
            num_threads: Some(4),
            provider: Some(String::from("cpu")),
            ..Default::default()
        };

        let recognizer: SenseVoiceRecognizer = SenseVoiceRecognizer::new(config).unwrap();
        Self {
            voice_data: Box::new(Vec::new()),
            vad: Box::new(vad),
            recognizer: Box::new(recognizer),
        }
    }
}

impl Listener {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn listen(&mut self, data: &[u8]) {
        //tracing::info!("voice len = {}", data.len());
        let sample_rate: u32 = 16000;
        // 16000Hz * 1 channel * 60 ms / 1000 = 960 samples -> frameSize
        let frame_size = (sample_rate * 60 / 1000) as usize;
        let mut samples = vec![0f32; frame_size];
        let mut decoder = opus::Decoder::new(sample_rate, opus::Channels::Mono).unwrap();
        let len = decoder.decode_float(&data, &mut samples, false).unwrap();
        let voice_data = self.voice_data.as_mut();
        voice_data.append(&mut samples);
        let vad = self.vad.as_mut();
        let window_size = 512;
        while voice_data.len() > window_size {
            let window: Vec<f32> = voice_data.drain(..window_size).collect();
            vad.accept_waveform(window.to_vec());
            if vad.is_speech() {
                while !vad.is_empty() {
                    let segment = vad.front();
                    let start_sec = (segment.start as f32) / sample_rate as f32;
                    let duration_sec = (segment.samples.len() as f32) / sample_rate as f32;
                    tracing::info!("start={}s duration={}s", start_sec, duration_sec);
                    let result = self.recognizer.transcribe(sample_rate, &segment.samples);
                    tracing::info!("recognizer result = {:?}", result);
                    vad.pop();
                }
            }
        }
    }
}
