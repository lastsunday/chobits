use crate::ws::frame::Frame;
use axum::extract::ws::Message;
use serde_json::Value;
use service::chobits::message::{
    abort::AbortMessage, close::CloseMessage, hello::HelloMessage, listen::ListenMessage,
    mcp::McpMessage,
};
use std::ops::ControlFlow;
use tracing::debug;

pub async fn convert_to_frame<'a>(
    msg: &'a Message,
) -> ControlFlow<Option<Frame<'a>>, Option<Frame<'a>>> {
    match msg {
        Message::Text(data) => {
            let data = data.as_bytes();
            match serde_json::from_slice::<Value>(data) {
                Ok(json) => {
                    debug!("is object = {},data = {}", json.is_object(), json);
                    if json.is_object() {
                        match json.get("type") {
                            Some(mtype) => {
                                tracing::info!("mtype = {:?}", mtype);
                                if mtype == r#"hello"# {
                                    match serde_json::from_slice::<HelloMessage>(data) {
                                        Ok(message) => {
                                            return ControlFlow::Continue(Some(Frame::Hello(
                                                message,
                                            )));
                                        }
                                        Err(_) => {
                                            return ControlFlow::Continue(Some(
                                                Frame::UnknowText { data },
                                            ));
                                        }
                                    }
                                } else if mtype == r#"listen"# {
                                    match serde_json::from_slice::<ListenMessage>(data) {
                                        Ok(message) => {
                                            return ControlFlow::Continue(Some(Frame::Listen(
                                                message,
                                            )));
                                        }
                                        Err(_) => {
                                            return ControlFlow::Continue(Some(
                                                Frame::UnknowText { data },
                                            ));
                                        }
                                    }
                                } else if mtype == r#"abort"# {
                                    match serde_json::from_slice::<AbortMessage>(data) {
                                        Ok(message) => {
                                            return ControlFlow::Continue(Some(Frame::Abort(
                                                message,
                                            )));
                                        }
                                        Err(_) => {
                                            return ControlFlow::Continue(Some(
                                                Frame::UnknowText { data },
                                            ));
                                        }
                                    }
                                } else if mtype == r#"mcp"# {
                                    match serde_json::from_slice::<McpMessage>(data) {
                                        Ok(message) => {
                                            return ControlFlow::Continue(Some(Frame::Mcp(
                                                message,
                                            )));
                                        }
                                        Err(_) => {
                                            return ControlFlow::Continue(Some(
                                                Frame::UnknowText { data },
                                            ));
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
                    return ControlFlow::Continue(Some(Frame::UnknowText { data }));
                }
            }
        }

        Message::Binary(data) => {
            return ControlFlow::Continue(Some(Frame::Voice { data }));
        }

        Message::Close(c) => match c {
            Some(cf) => {
                return ControlFlow::Break(Some(Frame::Close(CloseMessage::new(
                    cf.code,
                    cf.reason.as_str(),
                ))));
            }
            None => {
                return ControlFlow::Break(None);
            }
        },

        Message::Pong(data) => {
            return ControlFlow::Continue(Some(Frame::Pong { data }));
        }

        // You should never need to manually handle Message::Ping, as axum's websocket library
        // will do so for you automagically by replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them here.
        Message::Ping(data) => {
            return ControlFlow::Continue(Some(Frame::Ping { data }));
        }
    }
    ControlFlow::Continue(None)
}
