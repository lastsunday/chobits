use std::sync::Arc;
use std::thread;
use std::time::Duration;

use futures::{Stream, executor::block_on};
use tokio::sync::Mutex;
use tokio::sync::mpsc::channel;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Debug, Clone)]
struct State {}

impl State {
    pub fn new() -> Self {
        Self {}
    }
}

#[tokio::test]
async fn test_controller_stream_by_ws_text() {
    let start_post_prompt = std::time::Instant::now();
    let state = Arc::new(Mutex::new(State::new()));
    let ws_text = ws_text("Hello".to_string()).await;
    let mut llm_output = llm(state.clone(), ws_text).await;
    while let Some(text) = llm_output.next().await {
        let mut tts_output = tts(text);
        while let Some(text) = tts_output.next().await {
            let dt = start_post_prompt.elapsed().as_millis();
            println!("{} : {}", dt, text);
        }
    }
}

#[tokio::test]
async fn test_controller_stream_by_ws_audio() {}

fn tts(text: String) -> impl Stream<Item = String> + Unpin + Send + 'static {
    let (tx, rx) = channel::<String>(5);
    thread::spawn(move || {
        block_on(async move {
            let _ = tx.send(format!("{} [TTS]->", text.clone())).await;
            drop(tx);
        })
    });
    ReceiverStream::new(rx)
}

async fn llm(
    _state: Arc<Mutex<State>>,
    text: String,
) -> impl Stream<Item = String> + Unpin + Send + 'static {
    let (tx, rx) = channel::<String>(5);
    thread::spawn(move || {
        block_on(async move {
            for count in 1..6 {
                thread::sleep(Duration::from_millis(100));
                let _ = tx.send(format!("{} [LLM]->{}", text.clone(), count)).await;
            }
            drop(tx);
        })
    });
    ReceiverStream::new(rx)
}

async fn ws_text(text: String) -> String {
    format!("{} [WS](Text)->", text).to_string()
}

// async fn asr(audio: String) -> String {
//     format!("{} [ASR]->", audio).to_string()
// }
//
// async fn vad(audio: String) -> String {
//     format!("{} [VAD]->", audio).to_string()
// }
