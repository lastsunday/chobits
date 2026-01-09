use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct TtsConfig {
    model: Option<Model>,
    path: Option<String>,
    //参照音频字幕
    reference_prompt_text: Option<String>,
    //参照音频路径
    reference_prompt_wav_path: Option<String>,
}

impl TtsConfig {
    pub fn new() -> Self {
        Self {
            // model: Some(Model::Kokoro),
            // path: Some(String::from("data/tts/model/mzdk100/kokoro/")),
            // reference_prompt_text: None,
            // reference_prompt_wav_path: None,
            model: Some(Model::Voxcpm),
            path: Some(String::from("data/tts/model/openbmb/VoxCPM-0.5B/")),
            reference_prompt_text: Some(String::from(
                "一定被灰太狼给吃了，我已经为他准备好了花圈了",
            )),
            reference_prompt_wav_path: Some(String::from("file://data/tts/reference/voice_05.wav")),
        }
    }

    pub fn path(&self) -> &str {
        self.path.as_deref().unwrap_or_default()
    }

    pub fn model(&self) -> Model {
        self.model.clone().unwrap_or_default()
    }

    pub fn reference_prompt_text(&self) -> &str {
        self.reference_prompt_text.as_deref().unwrap_or_default()
    }

    pub fn reference_prompt_wav_path(&self) -> &str {
        self.reference_prompt_wav_path
            .as_deref()
            .unwrap_or_default()
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq, Default)]
pub enum Model {
    #[default]
    Kokoro,
    Voxcpm,
}
