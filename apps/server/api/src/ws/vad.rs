pub trait Vad {
    fn process(data: &[u8]) -> VadResult;
}

pub enum VadResult {
    NoSpeech,
    SpeechStart,
    SpeechContinue,
    SpeechEnd,
    Error,
}
