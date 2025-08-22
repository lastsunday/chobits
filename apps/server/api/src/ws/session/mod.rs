use crate::config;
use crate::ws::frame::{Frame, FrameError, FrameResult};
use core::result::Result;
use framework::id::gen_id;
use futures::Stream;
use service::chobits::message::hello::{AudioParam, HelloMessage};
use service::chobits::message::{AudioFormat, Transport};
use tokio::sync::mpsc::{Sender, channel};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, instrument};

#[derive(Debug)]
pub struct Session {
    pub id: String,
    pub current_round: Option<Box<Round>>,
    tx: Option<Sender<Result<FrameResult, FrameError>>>,
}

impl Session {
    pub fn new() -> Self {
        let (tx, rx) = channel::<Result<FrameResult, FrameError>>(1);
        Self {
            id: gen_id(),
            current_round: None,
            tx: None,
        }
    }

    #[instrument(skip(self), name="Session start",fields(id = %self.id))]
    pub async fn start(&mut self) {
        info!("start");
    }

    #[instrument(skip(self), name="Session stop" fields(id = %self.id))]
    pub async fn stop(&mut self) {
        if let Some(round) = self.current_round.clone() {
            round.stop().await;
        }
        info!("end");
    }

    #[instrument(skip(self), name="Session new round",fields(id = %self.id))]
    pub async fn new_round(&mut self) {
        info!("new round");
        if let Some(round) = self.current_round.clone() {
            round.stop().await;
        }
        let tx = self
            .tx
            .clone()
            .expect("tx not create,maybe new round method before output frame method");
        self.current_round = Some(Box::new(Round::new(self.id.clone(), tx)));
        let round = self.current_round.clone().unwrap();
        round.start().await;
    }

    pub async fn accept_frame(&mut self, frame: Frame) {
        match frame {
            Frame::Hello(hello_message) => {
                self.new_round().await;
                self.handle_connect(hello_message).await;
            }
            Frame::Listen(listen_message) => todo!(),
            Frame::UnknowText(utf8_bytes) => todo!(),
            Frame::Voice(bytes) => todo!(),
            Frame::Abort(abort_message) => todo!(),
            Frame::Ping(bytes) => todo!(),
            Frame::Pong(bytes) => todo!(),
            Frame::Close(close_message) => {
                if let Some(round) = self.current_round.clone() {
                    round.stop().await;
                }
            }
        }
    }

    pub async fn output_frame(
        &mut self,
    ) -> impl Stream<Item = Result<FrameResult, FrameError>> + Unpin + Send + 'static {
        let (tx, rx) = channel::<Result<FrameResult, FrameError>>(10);
        self.tx = Some(tx);
        ReceiverStream::new(rx)
    }

    pub async fn stop_round(&mut self) {}

    async fn handle_connect(&mut self, hello_message: HelloMessage) {
        let tx = self.tx.clone().unwrap();
        let audio_config = config::get().audio();
        let data = HelloMessage {
            message: service::chobits::message::Message {
                mtype: service::chobits::message::Type::Hello,
            },
            transport: Some(Transport::Websocket),
            audio_params: Some(AudioParam {
                format: AudioFormat::Opus,
                sample_rate: audio_config.output_sample_rate(),
                channels: audio_config.output_channel(),
                frame_duration: audio_config.output_frame_duration(),
            }),
            version: None,
            features: None,
            session_id: Some(self.id.clone()),
        };
        tx.send(Ok(FrameResult::HelloResult(data))).await;
    }
}

#[derive(Debug, Clone)]
pub struct Round {
    pub parent_id: String,
    pub id: String,
    tx: Sender<Result<FrameResult, FrameError>>,
}

impl Round {
    pub fn new(parent_id: String, tx: Sender<Result<FrameResult, FrameError>>) -> Self {
        Self {
            parent_id,
            id: gen_id(),
            tx,
        }
    }

    #[instrument(skip(self), name="Round start",fields(id = %self.id,parent_id = %self.parent_id))]
    pub async fn start(&self) {
        info!("start");
    }

    pub async fn accept_frame(&mut self, frame: Frame) {}

    #[instrument(skip(self), name="Round stop",fields(id = %self.id,parent_id = %self.parent_id))]
    pub async fn stop(&self) {
        info!("stop");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use service::chobits::message::hello::HelloMessage;
    use tokio_stream::StreamExt;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_chat_flow_hello() {
        let mut session = Session::new();
        session.start().await;
        let mut output = session.output_frame().await;
        let join_handle = tokio::spawn(async move {
            let mut count = 0;
            while let Some(data) = output.next().await {
                match data {
                    Ok(frame_result) => match frame_result {
                        FrameResult::HelloResult(hello_message) => {
                            return;
                        }
                    },
                    Err(_) => {
                        break;
                    }
                }
            }
            panic!("receive hello message error");
        });
        let hello_frame = Frame::Hello(HelloMessage {
            ..Default::default()
        });
        session.accept_frame(hello_frame.clone()).await;
        session.stop().await;
        join_handle.await.unwrap();
    }
}
