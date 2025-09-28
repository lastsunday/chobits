use axum::{body::Bytes, extract::ws::Utf8Bytes};
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
pub enum Frame {
    Hello(HelloMessage),
    Listen(ListenMessage),
    UnknowText(Utf8Bytes),
    Voice(Bytes),
    Abort(AbortMessage),
    Ping(Bytes),
    Pong(Bytes),
    Close(CloseMessage),
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
