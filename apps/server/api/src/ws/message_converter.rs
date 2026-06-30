use axum::extract::ws::Message;
use serde::Deserialize;
use serde_json::Value;
use service::chobits::message::close::CloseMessage;
use service::ws::frame::Frame;
use std::ops::ControlFlow;

fn try_parse<'a, T, F>(data: &'a [u8], f: F) -> ControlFlow<Option<Frame<'a>>, Option<Frame<'a>>>
where
    T: Deserialize<'a>,
    F: FnOnce(T) -> Frame<'a>,
{
    match serde_json::from_slice::<T>(data) {
        Ok(msg) => ControlFlow::Continue(Some(f(msg))),
        Err(_) => ControlFlow::Continue(Some(Frame::UnknowText { data })),
    }
}

pub fn convert_to_frame<'a>(msg: &'a Message) -> ControlFlow<Option<Frame<'a>>, Option<Frame<'a>>> {
    match msg {
        Message::Text(data) => {
            let data = data.as_bytes();
            match serde_json::from_slice::<Value>(data) {
                Ok(json) if json.is_object() => match json.get("type").and_then(|v| v.as_str()) {
                    Some("hello") => try_parse(data, Frame::Hello),
                    Some("listen") => try_parse(data, Frame::Listen),
                    Some("abort") => try_parse(data, Frame::Abort),
                    Some("mcp") => try_parse(data, Frame::Mcp),
                    _ => {
                        tracing::warn!("unknown message type");
                        ControlFlow::Continue(None)
                    }
                },
                Ok(json) => {
                    tracing::warn!("unknown json message = {json}");
                    ControlFlow::Continue(None)
                }
                Err(_) => ControlFlow::Continue(Some(Frame::UnknowText { data })),
            }
        }

        Message::Binary(data) => ControlFlow::Continue(Some(Frame::Voice { data })),

        Message::Close(c) => match c {
            Some(cf) => ControlFlow::Break(Some(Frame::Close(CloseMessage::new(
                cf.code,
                cf.reason.as_str(),
            )))),
            None => ControlFlow::Break(None),
        },

        Message::Pong(data) => ControlFlow::Continue(Some(Frame::Pong { data })),

        Message::Ping(data) => ControlFlow::Continue(Some(Frame::Ping { data })),
    }
}
