use axum::{body::Bytes, extract::ws::Utf8Bytes};
use service::chobits::message::{
    abort::AbortMessage, close::CloseMessage, hello::HelloMessage, listen::ListenMessage,
};

#[derive(Debug)]
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
