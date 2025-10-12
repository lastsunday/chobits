use service::chobits::message::{
    abort::AbortMessage,
    audio::AudioMessage,
    close::CloseMessage,
    hello::HelloMessage,
    listen::ListenMessage,
    llm::LlmMessage,
    mcp::{McpMessage, McpRequest},
    stt::SttMessage,
    tts::TtsMessage,
};

#[derive(Debug, Clone)]
pub enum Frame<'a> {
    Hello(HelloMessage),
    Listen(ListenMessage<'a>),
    UnknowText { data: &'a [u8] },
    Voice { data: &'a [u8] },
    Abort(AbortMessage<'a>),
    Ping { data: &'a [u8] },
    Pong { data: &'a [u8] },
    Close(CloseMessage<'a>),
    Mcp(McpMessage),
}

#[derive(Debug)]
pub enum FrameResult {
    HelloResult(HelloMessage),
    STTResult(SttMessage),
    LLMResult(LlmMessage),
    TTSResult(TtsMessage),
    AudioResult(AudioMessage),
    CloseResult,
    McpResult(McpRequest),
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {}
