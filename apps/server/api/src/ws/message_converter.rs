use std::ops::ControlFlow;

use axum::extract::ws::Message;
use serde_json::Value;
use service::chobits::message::{abort::AbortMessage, hello::HelloMessage, listen::ListenMessage};

use crate::ws::frame::Frame;

pub async fn convert_to_frame(msg: Message) -> ControlFlow<(), Option<Frame>> {
    match msg {
        Message::Text(t) => match serde_json::from_slice::<Value>(t.as_bytes()) {
            Ok(json) => {
                tracing::info!("is object = {},data = {}", json.is_object(), json);
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
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>>  sent close with code {} and reason `{}`",
                    cf.code, cf.reason
                );
            } else {
                println!(">>>  somehow sent close message without CloseFrame");
            }
            return ControlFlow::Break(());
        }

        Message::Pong(v) => {
            println!(">>>  sent pong with {v:?}");
        }
        // You should never need to manually handle Message::Ping, as axum's websocket library
        // will do so for you automagically by replying with Pong and copying the v according to
        // spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            println!(">>>  sent ping with {v:?}");
        }
    }
    ControlFlow::Continue(None)
}
