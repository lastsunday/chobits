use axum::{body::Bytes, extract::ws::Utf8Bytes};
use service::chobits::message::{
    abort::AbortMessage, close::CloseMessage, hello::HelloMessage, listen::ListenMessage,
    llm::LlmMessage, stt::SttMessage,
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
}

#[derive(Debug)]
pub enum FrameResult {
    HelloResult(HelloMessage),
    STTResult(SttMessage),
    LLMResult(LlmMessage),
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {}
