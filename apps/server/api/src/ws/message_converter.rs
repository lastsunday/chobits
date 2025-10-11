use crate::ws::frame::Frame;
use axum::extract::ws::Message;
use serde_json::Value;
use service::chobits::message::{
    abort::AbortMessage, close::CloseMessage, hello::HelloMessage, listen::ListenMessage,
    mcp::McpMessage,
};
use std::ops::ControlFlow;
use tracing::debug;

pub async fn convert_to_frame(msg: Message) -> ControlFlow<Option<Frame>, Option<Frame>> {
    match msg {
        Message::Text(t) => match serde_json::from_slice::<Value>(t.as_bytes()) {
            Ok(json) => {
                debug!("is object = {},data = {}", json.is_object(), json);
                if json.is_object() {
                    match json.get("type") {
                        Some(mtype) => {
                            tracing::info!("mtype = {:?}", mtype);
                            if mtype == r#"hello"# {
                                match serde_json::from_slice::<HelloMessage>(t.as_bytes()) {
                                    Ok(message) => {
                                        return ControlFlow::Continue(Some(Frame::Hello(message)));
                                    }
                                    Err(_) => {
                                        return ControlFlow::Continue(Some(Frame::UnknowText(t)));
                                    }
                                }
                            } else if mtype == r#"listen"# {
                                match serde_json::from_slice::<ListenMessage>(t.as_bytes()) {
                                    Ok(message) => {
                                        return ControlFlow::Continue(Some(Frame::Listen(message)));
                                    }
                                    Err(_) => {
                                        return ControlFlow::Continue(Some(Frame::UnknowText(t)));
                                    }
                                }
                            } else if mtype == r#"abort"# {
                                match serde_json::from_slice::<AbortMessage>(t.as_bytes()) {
                                    Ok(message) => {
                                        return ControlFlow::Continue(Some(Frame::Abort(message)));
                                    }
                                    Err(_) => {
                                        return ControlFlow::Continue(Some(Frame::UnknowText(t)));
                                    }
                                }
                            } else if mtype == r#"mcp"# {
                                match serde_json::from_slice::<McpMessage>(t.as_bytes()) {
                                    Ok(message) => {
                                        return ControlFlow::Continue(Some(Frame::Mcp(message)));
                                    }
                                    Err(_) => {
                                        return ControlFlow::Continue(Some(Frame::UnknowText(t)));
                                    }
                                }
                            }
                        }
                        None => {
                            tracing::info!("can't find type field");
                        }
                    }
                } else {
                    tracing::info!("unknow json message = {}", json)
                }
            }
            Err(_) => {
                return ControlFlow::Continue(Some(Frame::UnknowText(t)));
            }
        },

        Message::Binary(d) => {
            return ControlFlow::Continue(Some(Frame::Voice(d)));
        }

        Message::Close(c) => match c {
            Some(cf) => {
                return ControlFlow::Break(Some(Frame::Close(CloseMessage::new(
                    cf.code,
                    String::from(cf.reason.as_str()),
                ))));
            }
            None => {
                return ControlFlow::Break(None);
            }
        },

        Message::Pong(v) => {
            return ControlFlow::Continue(Some(Frame::Pong(v)));
        }

        // You should never need to manually handle Message::Ping, as axum's websocket library
        // will do so for you automagically by replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            return ControlFlow::Continue(Some(Frame::Ping(v)));
        }
    }
    ControlFlow::Continue(None)
}
