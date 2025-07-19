use serde::Deserialize;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct TtsConfig {
    model: Option<String>,
    voices: Option<String>,
    tokens: Option<String>,
    data_dir: Option<String>,
    dict_dir: Option<String>,
    lexicon: Option<String>,
    num_threads: Option<i32>,
}

impl TtsConfig {
    pub fn new() -> Self {
        Self {
            model: Some(String::from("data/tts/kokoro-multi-lang-v1_1/model.onnx")),
            voices: Some(String::from("data/tts/kokoro-multi-lang-v1_1/voices.bin")),
            tokens: Some(String::from("data/tts/kokoro-multi-lang-v1_1/tokens.txt")),
            data_dir: Some(String::from(
                "data/tts/kokoro-multi-lang-v1_1/espeak-ng-data",
            )),
            dict_dir: Some(String::from("data/tts/kokoro-multi-lang-v1_1/dict")),
            lexicon: Some(String::from(
                "data/tts/kokoro-multi-lang-v1_1/lexicon-us-en.txt,data/tts/kokoro-multi-lang-v1_1/lexicon-zh.txt",
            )),
            num_threads: Some(4),
        }
    }

    pub fn model(&self) -> &str {
        self.model
            .as_deref()
            .unwrap_or("data/tts/kokoro-multi-lang-v1_1/model.onnx")
    }

    pub fn voices(&self) -> &str {
        self.voices
            .as_deref()
            .unwrap_or("data/tts/kokoro-multi-lang-v1_1/voices.bin")
    }

    pub fn tokens(&self) -> &str {
        self.tokens
            .as_deref()
            .unwrap_or("data/tts/kokoro-multi-lang-v1_1/tokens.txt")
    }
    pub fn data_dir(&self) -> &str {
        self.data_dir
            .as_deref()
            .unwrap_or("data/tts/kokoro-multi-lang-v1_1/espeak-ng-data")
    }
    pub fn dict_dir(&self) -> &str {
        self.dict_dir
            .as_deref()
            .unwrap_or("data/tts/kokoro-multi-lang-v1_1/dict")
    }
    pub fn lexicon(&self) -> &str {
        self.lexicon
            .as_deref()
            .unwrap_or("data/tts/kokoro-multi-lang-v1_1/lexicon-us-en.txt,data/kokoro-multi-lang-v1_1/lexicon-zh.txt")
    }
    pub fn num_threads(&self) -> i32 {
        self.num_threads.unwrap_or(4_i32)
    }
}
