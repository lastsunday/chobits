use api::ws::{WebSocketState, handle_socket};
use axum::extract::ws::Message;
use futures::{SinkExt, StreamExt};

#[tokio::test]
async fn ws() {
    // Need to use "futures" channels rather than "tokio" channels as they implement `Sink` and
    // `Stream`
    let (socket_write, mut test_rx) = futures_channel::mpsc::channel(1024);
    let (mut test_tx, socket_read) = futures_channel::mpsc::channel(1024);

    tokio::spawn(handle_socket(
        socket_write,
        socket_read,
        WebSocketState::new(),
    ));

    test_tx.send(Ok(Message::Text("foo".into()))).await.unwrap();
}
