use std::{
    cmp,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    time::Duration,
};

use api::{
    AppState,
    asr::AsrFactory,
    config,
    llm::LlmFactory,
    mcp::{
        client::server::ServerMcpClient,
        mcp_host::{McpHost, UnionMcpHost},
    },
    setup_mcp,
    tts::TtsFactory,
    util::audio::pcm_decode,
    vad::VadFactory,
    ws::session::{SessionBuilder, SessionConfig},
};

use api::{
    ws::frame::{Frame, FrameResult},
    ws::session::{Session, listener::DefaultListener},
};
use framework::id::gen_id;
use rmcp::{
    model::{
        Icon, Implementation, InitializeResult, JsonObject, JsonRpcMessage, JsonRpcResponse,
        JsonRpcVersion2_0, ListToolsResult, ProtocolVersion, RequestId, ServerCapabilities, object,
    },
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::Serialize;
use service::chobits::message::{
    audio::AudioMessage,
    hello::{Feature, HelloMessage},
    listen::{ListenMessage, ListenMode, ListenState},
    mcp::McpMessage,
    tts::{TtsMessage, TtsState},
};
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tokio::{sync::Mutex, time::sleep};
use tokio_stream::StreamExt;
use tracing::info;
use tracing_test::traced_test;
use utoipa_axum::router::OpenApiRouter;

mod common;
use common::{router_client::RouterClient, setup_database, tear_down};

#[tokio::test]
#[traced_test]
#[ignore]
/// hello paramter input and output the hello result
/// cargo test --test session_test -- test_chat_flow_hello --ignored --show-output
async fn test_chat_flow_hello() -> anyhow::Result<()> {
    let (mut session, container, state) = create_session().await?;
    session.start().await?;
    let mut output = session.output_frame().await;
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::HelloResult(..)
    ));
    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(&container).await;
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// listen voice by manual mode and output the asr text result
/// cargo test --test session_test -- test_chat_flow_listen_manual --ignored --show-output
async fn test_chat_flow_listen_manual() -> anyhow::Result<()> {
    use std::path::PathBuf;

    let wav_file: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "resources",
        "test",
        "samples_jfk.wav",
    ]
    .iter()
    .collect();
    info!("{}", wav_file.display());
    let (pcm_data, sample_rate) = pcm_decode(wav_file).unwrap();
    info!(
        "pcm_data len = {},sample_rate = {}",
        pcm_data.len(),
        sample_rate
    );

    // the follow code is output wav file to test
    // use wavers::{AudioSample, ConvertSlice, ConvertTo, Samples, read, write};
    // let fp = "./decode_pcm_data.wav";
    // let sr: i32 = 16000;
    // write(fp, &pcm_data, sr, 1);

    const ENCODE_SAMPLE_RATE: u32 = 16000;
    let mut encoder = opus::Encoder::new(
        ENCODE_SAMPLE_RATE,
        opus::Channels::Mono,
        opus::Application::Audio,
    )
    .unwrap();

    // 16000Hz * 1 channel * 60 ms / 1000 = 960
    const MONO_60MS: usize = ENCODE_SAMPLE_RATE as usize * 60 / 1000;
    let size = MONO_60MS;
    info!("size = {}", size);
    let len = pcm_data.len();
    let mut count = len / size;
    if len % size > 0 {
        count += 1;
    }
    info!("count = {}", count);
    let mut audio: Vec<Vec<u8>> = Vec::new();

    for n in 0..count {
        let start = n * size;
        let end = cmp::min((n + 1) * size, len);
        let packet = encoder
            .encode_vec_float(&pcm_data[start..end], size)
            .unwrap();
        audio.push(packet);
    }

    let (mut session, container, state) = create_session().await?;
    let session_id = session.id.clone();
    session.start().await?;
    let mut output = session.output_frame().await;
    // TODO: need refactor,remove tokio::spawn
    let join_handle = tokio::spawn(async move {
        while let Some(data) = output.next().await {
            info!("session id = {}, data = {:?}", session_id, data);
            match data {
                Ok(frame_result) => match frame_result {
                    FrameResult::HelloResult(_hello_message) => {}
                    FrameResult::STTResult(_stt_message) => {}
                    FrameResult::LLMResult(_llm_message) => {}
                    FrameResult::TTSResult(tts_message) => {
                        if let Some(state) = tts_message.state
                            && TtsState::Stop == state
                        {
                            return;
                        }
                    }
                    FrameResult::AudioResult(_audio_message) => {}
                    _ => {
                        panic!("unexpected frame result");
                    }
                },
                Err(e) => {
                    panic!("{:?}", e);
                }
            }
        }
        panic!("receive hello message error");
    });
    let hello_frame = Frame::Hello(HelloMessage {
        ..Default::default()
    });
    session.accept_frame(&hello_frame).await;
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Start,
            mmod: Some(service::chobits::message::listen::ListenMode::Manual),
            ..Default::default()
        }))
        .await;
    for n in 0..audio.len() {
        session
            .accept_frame(&Frame::Voice {
                data: audio.get(n).unwrap(),
            })
            .await;
    }
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Stop,
            ..Default::default()
        }))
        .await;
    join_handle.await?;
    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(&container).await;
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// listen voice by auto mode and output the asr text result
/// cargo test --test session_test -- test_chat_flow_listen_auto --ignored --show-output
async fn test_chat_flow_listen_auto() -> anyhow::Result<()> {
    use std::path::PathBuf;

    let wav_file: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "resources",
        "test",
        "samples_jfk.wav",
    ]
    .iter()
    .collect();
    info!("{}", wav_file.display());
    let (pcm_data, sample_rate) = pcm_decode(wav_file).unwrap();
    info!(
        "pcm_data len = {},sample_rate = {}",
        pcm_data.len(),
        sample_rate
    );

    // the follow code is output wav file to test
    // use wavers::{AudioSample, ConvertSlice, ConvertTo, Samples, read, write};
    // let fp = "./decode_pcm_data.wav";
    // let sr: i32 = 16000;
    // write(fp, &pcm_data, sr, 1);

    const ENCODE_SAMPLE_RATE: u32 = 16000;
    let mut encoder = opus::Encoder::new(
        ENCODE_SAMPLE_RATE,
        opus::Channels::Mono,
        opus::Application::Audio,
    )
    .unwrap();

    // 16000Hz * 1 channel * 60 ms / 1000 = 960
    const MONO_60MS: usize = ENCODE_SAMPLE_RATE as usize * 60 / 1000;
    let size = MONO_60MS;
    info!("size = {}", size);
    let len = pcm_data.len();
    let mut count = len / size;
    if len % size > 0 {
        count += 1;
    }
    info!("count = {}", count);
    let mut audio: Vec<Vec<u8>> = Vec::new();

    for n in 0..count {
        let start = n * size;
        let end = cmp::min((n + 1) * size, len);
        let packet = encoder
            .encode_vec_float(&pcm_data[start..end], size)
            .unwrap();
        audio.push(packet);
    }

    let (mut session, container, state) = create_session().await?;
    let session_id = session.id.clone();
    session.start().await?;
    let mut output = session.output_frame().await;
    let next_step = Arc::new(AtomicBool::new(false));
    let next_step_for_sender = next_step.clone();
    // TODO: need refactor,remove tokio::spawn
    let join_handle = tokio::spawn(async move {
        let mut count = 0;
        while let Some(data) = output.next().await {
            info!("session id = {}, data = {:?}", session_id, data);
            match data {
                Ok(frame_result) => match frame_result {
                    FrameResult::HelloResult(_hello_message) => {}
                    FrameResult::STTResult(_stt_message) => {}
                    FrameResult::LLMResult(_llm_message) => {}
                    FrameResult::TTSResult(tts_message) => {
                        let state = tts_message.state;
                        if let Some(state) = state
                            && TtsState::Stop == state
                        {
                            count += 1;
                            next_step.store(true, Ordering::Relaxed);
                            //when next round tts stop after wake tts round
                            if count >= 2 {
                                return;
                            }
                        }
                    }
                    FrameResult::AudioResult(_audio_message) => {}
                    _ => {
                        panic!("unexpected frame result");
                    }
                },
                Err(_) => {
                    break;
                }
            }
        }
        panic!("receive hello message error");
    });
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    for n in 0..audio.len() {
        session
            .accept_frame(&Frame::Voice {
                data: audio.get(n).unwrap(),
            })
            .await;
    }
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Detect,
            mmod: None,
            text: Some("Hello"),
            ..Default::default()
        }))
        .await;
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Start,
            mmod: Some(ListenMode::Auto),
            ..Default::default()
        }))
        .await;
    let mut to_next_step = false;
    info!("before next step");
    while !to_next_step {
        to_next_step = next_step_for_sender.load(Ordering::Relaxed);
        sleep(Duration::from_millis(500)).await;
    }
    info!("after next step");
    for n in 0..audio.len() {
        session
            .accept_frame(&Frame::Voice {
                data: audio.get(n).unwrap(),
            })
            .await;
    }
    join_handle.await?;
    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(&container).await;
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// listen voice by realtime mode and output the asr text result
/// cargo test --test session_test -- test_chat_flow_listen_realtime --ignored --show-output
async fn test_chat_flow_listen_realtime() -> anyhow::Result<()> {
    use std::path::PathBuf;

    let wav_file: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "resources",
        "test",
        "samples_jfk.wav",
    ]
    .iter()
    .collect();
    info!("{}", wav_file.display());
    let (pcm_data, sample_rate) = pcm_decode(wav_file).unwrap();
    info!(
        "pcm_data len = {},sample_rate = {}",
        pcm_data.len(),
        sample_rate
    );

    // the follow code is output wav file to test
    // use wavers::{AudioSample, ConvertSlice, ConvertTo, Samples, read, write};
    // let fp = "./decode_pcm_data.wav";
    // let sr: i32 = 16000;
    // write(fp, &pcm_data, sr, 1);

    const ENCODE_SAMPLE_RATE: u32 = 16000;
    let mut encoder = opus::Encoder::new(
        ENCODE_SAMPLE_RATE,
        opus::Channels::Mono,
        opus::Application::Audio,
    )
    .unwrap();

    // 16000Hz * 1 channel * 60 ms / 1000 = 960
    const MONO_60MS: usize = ENCODE_SAMPLE_RATE as usize * 60 / 1000;
    let size = MONO_60MS;
    info!("size = {}", size);
    let len = pcm_data.len();
    let mut count = len / size;
    if len % size > 0 {
        count += 1;
    }
    info!("count = {}", count);
    let mut audio: Vec<Vec<u8>> = Vec::new();

    for n in 0..count {
        let start = n * size;
        let end = cmp::min((n + 1) * size, len);
        let packet = encoder
            .encode_vec_float(&pcm_data[start..end], size)
            .unwrap();
        audio.push(packet);
    }

    let (mut session, container, state) = create_session().await?;
    let session_id = session.id.clone();
    session.start().await?;
    let mut output = session.output_frame().await;
    // TODO: need refactor,remove tokio::spawn
    let join_handle = tokio::spawn(async move {
        let mut count = 0;
        while let Some(data) = output.next().await {
            info!("session id = {}, data = {:?}", session_id, data);
            match data {
                Ok(frame_result) => match frame_result {
                    FrameResult::HelloResult(_hello_message) => {}
                    FrameResult::STTResult(_stt_message) => {}
                    FrameResult::LLMResult(_llm_message) => {}
                    FrameResult::TTSResult(tts_message) => {
                        let state = tts_message.state;
                        if let Some(state) = state
                            && TtsState::Stop == state
                        {
                            count += 1;
                            //when next round tts stop after wake tts round
                            if count >= 2 {
                                return;
                            }
                        }
                    }
                    FrameResult::AudioResult(_audio_message) => {}
                    _ => {
                        panic!("unexpected frame result");
                    }
                },
                Err(_) => {
                    break;
                }
            }
        }
        panic!("receive hello message error");
    });
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    for n in 0..audio.len() {
        session
            .accept_frame(&Frame::Voice {
                data: audio.get(n).unwrap(),
            })
            .await;
    }
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Detect,
            mmod: None,
            text: Some("Hello"),
            ..Default::default()
        }))
        .await;
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Start,
            mmod: Some(ListenMode::RealTime),
            ..Default::default()
        }))
        .await;
    for n in 0..audio.len() {
        session
            .accept_frame(&Frame::Voice {
                data: audio.get(n).unwrap(),
            })
            .await;
    }
    join_handle.await?;
    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(&container).await;
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// get text message and output the asr text result
/// cargo test --test session_test -- test_chat_flow_handle_text_message --ignored --show-output
async fn test_chat_flow_handle_text_message() -> anyhow::Result<()> {
    let (mut session, container, state) = create_session().await?;
    let session_id = session.id.clone();
    session.start().await?;
    let mut output = session.output_frame().await;
    // TODO: need refactor,remove tokio::spawn
    let join_handle = tokio::spawn(async move {
        while let Some(data) = output.next().await {
            info!("session id = {}, data = {:?}", session_id, data);
            match data {
                Ok(frame_result) => match frame_result {
                    FrameResult::HelloResult(_hello_message) => {}
                    FrameResult::STTResult(_stt_message) => {}
                    FrameResult::LLMResult(_llm_message) => {}
                    FrameResult::TTSResult(tts_message) => {
                        let state = tts_message.state;
                        if let Some(state) = state
                            && TtsState::Stop == state
                        {
                            return;
                        }
                    }
                    FrameResult::AudioResult(_audio_message) => {}
                    _ => {
                        panic!("unexpected frame result");
                    }
                },
                Err(_) => {
                    break;
                }
            }
        }
        panic!("receive hello message error");
    });
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Detect,
            mmod: Some(service::chobits::message::listen::ListenMode::Manual),
            text: Some("Hello"),
            ..Default::default()
        }))
        .await;
    join_handle.await?;
    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(&container).await;
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// when a round running and has a break event,the output stream will stop the original output
/// cargo test --test session_test -- test_chat_flow_break --ignored --show-output
async fn test_chat_flow_break() -> anyhow::Result<()> {
    let (mut session, container, state) = create_session().await?;
    let session_id = session.id.clone();
    session.start().await?;
    let mut output = session.output_frame().await;
    let mut count = 0;
    // TODO: need refactor,remove tokio::spawn
    let join_handle = tokio::spawn(async move {
        while let Some(data) = output.next().await {
            info!("session id = {}, data = {:?}", session_id, data);
            match data {
                Ok(frame_result) => match frame_result {
                    FrameResult::HelloResult(_hello_message) => {}
                    FrameResult::STTResult(_stt_message) => {}
                    FrameResult::LLMResult(_llm_message) => {}
                    FrameResult::TTSResult(tts_message) => {
                        let state = tts_message.state;
                        if let Some(state) = state
                            && TtsState::Stop == state
                        {
                            count += 1;
                            //when next round tts stop after wake tts round
                            if count >= 2 {
                                return;
                            }
                        }
                    }
                    FrameResult::AudioResult(_audio_message) => {}
                    _ => {
                        panic!("unexpected frame result");
                    }
                },
                Err(_) => {
                    break;
                }
            }
        }
        panic!("receive hello message error");
    });
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Detect,
            mmod: Some(service::chobits::message::listen::ListenMode::Manual),
            text: Some("Hello"),
            ..Default::default()
        }))
        .await;
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Detect,
            mmod: Some(service::chobits::message::listen::ListenMode::Manual),
            text: Some("Hello"),
            ..Default::default()
        }))
        .await;
    join_handle.await?;
    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(&container).await;
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// Shell command:
/// ``` shell
/// cargo test --test session_test -- test_mcp_flow_server_client --ignored --show-output
/// ```
/// 1. [Device -> Server] hello request
/// 2. [Server -> Device] hello response
/// 3.1.1. [Server -> Device] mcp initialize request
/// 3.1.2. [Device -> Server] mcp initialize response
/// 3.1.3. [Server -> Device] mcp tools list request
/// 3.1.4. [Device -> Server] mcp tools list response
/// 3.2.1. [Device -> Server] voice request
/// 3.2.2. [Device -> Server] detect wake request
/// 4.1.1. [Device -> Server] listen start reqeust
/// 4.1.2. [Device -> Server] voice request (loop forever)
/// 4.2.0.1. [Server] vad
/// 4.2.0.2. [Server] asr
/// 4.2.0.3. [Server] llm (user input replace by wake word)
/// 4.2.1. [Server -> Device] llm text response (for detect wake word)
/// 4.2.2. [Server -> Device] tts response (for detect wake wake word)
/// 5.1.0.1. [Server] vad
/// 5.1.0.2. [Server] asr
/// 5.1.0.3. [Server] llm
/// 5.1.0.4. [Server -> Device] mcp call tool(for device call)
/// 5.1.0.5. [Device -> Server] mcp call response
/// 5.1.0.6. [Server] llm
/// 5.1.1. [Server -> Device] llm text response
/// 5.1.2. [Server -> Device] tts response
async fn test_mcp_flow_server_client() -> anyhow::Result<()> {
    test_mcp_flow(String::from("现在几点")).await
}

#[tokio::test]
#[traced_test]
#[ignore]
/// Shell command:
/// ``` shell
/// cargo test --test session_test -- test_mcp_flow_device_client --ignored --show-output
/// ```
async fn test_mcp_flow_device_client() -> anyhow::Result<()> {
    test_mcp_flow(String::from("get device status")).await
}

async fn test_mcp_flow(text: String) -> anyhow::Result<()> {
    let device_mcp_tools_list_response: &'static str = r#"
[
  {
    "name": "self.get_device_status",
    "description": "Provides the real-time information of the device, including the current status of the audio speaker, screen, battery, network, etc.\nUse this tool for: \n1. Answering questions about current condition (e.g. what is the current volume of the audio speaker?)\n2. As the first step to control the device (e.g. turn up / down the volume of the audio speaker, etc.)",
    "inputSchema": {
      "properties": {},
      "type": "object"
    }
  },
  {
    "name": "self.audio_speaker.set_volume",
    "description": "Set the volume of the audio speaker. If the current volume is unknown, you must call `self.get_device_status` tool first and then call this tool.",
    "inputSchema": {
      "properties": {
        "volume": {
          "maximum": 100,
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": ["volume"],
      "type": "object"
    }
  },
  {
    "name": "self.screen.set_brightness",
    "description": "Set the brightness of the screen.",
    "inputSchema": {
      "properties": {
        "brightness": {
          "maximum": 100,
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": ["brightness"],
      "type": "object"
    }
  },
  {
    "name": "self.screen.set_theme",
    "description": "Set the theme of the screen. The theme can be `light` or `dark`.",
    "inputSchema": {
      "properties": {
        "theme": {
          "type": "string"
        }
      },
      "required": ["theme"],
      "type": "object"
    }
  },
  {
    "name": "self.camera.take_photo",
    "description": "Take a photo and explain it. Use this tool after the user asks you to see something.\nArgs:\n  `question`: The question that you want to ask about the photo.\nReturn:\n  A JSON object that provides the photo information.",
    "inputSchema": {
      "properties": {
        "question": {
          "type": "string"
        }
      },
      "required": ["question"],
      "type": "object"
    }
  }
]"#;

    let request_id = AtomicI64::new(0);
    let (mut session, container, state) = create_session().await?;
    session.start().await?;
    // let session_id = session.id.clone();
    let mut output = session.output_frame().await;
    // let join_handle = tokio::spawn(async move {
    //     while let Some(data) = output.next().await {
    //         info!("session id = {}, data = {:?}", session_id, data);
    //         match data {
    //             Ok(frame_result) => match frame_result {
    //                 FrameResult::HelloResult(_hello_message) => {}
    //                 FrameResult::STTResult(_stt_message) => {}
    //                 FrameResult::LLMResult(_llm_message) => {}
    //                 FrameResult::TTSResult(tts_message) => {
    //                     let state = tts_message.state;
    //                     if let Some(state) = state
    //                         && TtsState::Stop == state
    //                     {
    //                         return;
    //                     }
    //                 }
    //                 FrameResult::AudioResult(_audio_message) => {}
    //                 FrameResult::McpResult(..) => {}
    //                 FrameResult::CloseResult => {}
    //             },
    //             Err(e) => {
    //                 error!("{:?}", e);
    //                 break;
    //             }
    //         }
    //     }
    // });
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            features: Some(Feature {
                mcp: Some(true),
                aec: None,
            }),
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::HelloResult(..)
    ));
    let frame_result = output.next().await.unwrap().unwrap();
    assert!(matches!(frame_result, FrameResult::McpResult(..)));
    if let FrameResult::McpResult(request) = frame_result {
        assert_eq!(request.payload.request.method, "initialize");
    }

    session
        .accept_frame(&Frame::Mcp(McpMessage::new(to_json_rpc_response(
            request_id.fetch_add(1, Ordering::Relaxed),
            InitializeResult {
                protocol_version: ProtocolVersion::default(),
                capabilities: ServerCapabilities::default(),
                server_info: Implementation {
                    name: "icon-server".to_string(),
                    title: None,
                    version: "2.0.0".to_string(),
                    icons: Some(vec![Icon {
                        src: "https://example.com/server.png".to_string(),
                        mime_type: Some("image/png".to_string()),
                        sizes: None,
                    }]),
                    website_url: Some("https://docs.example.com".to_string()),
                },
                instructions: None,
            },
        ))))
        .await;

    let frame_result = output.next().await.unwrap().unwrap();
    assert!(matches!(frame_result, FrameResult::McpResult(..)));
    if let FrameResult::McpResult(request) = frame_result {
        assert_eq!(request.payload.request.method, "tools/list");
    }

    session
        .accept_frame(&Frame::Mcp(McpMessage::new(to_json_rpc_response(
            request_id.fetch_add(1, Ordering::Relaxed),
            ListToolsResult {
                next_cursor: None,
                tools: serde_json::from_str(device_mcp_tools_list_response).unwrap(),
                meta: None,
            },
        ))))
        .await;

    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Detect,
            mmod: Some(service::chobits::message::listen::ListenMode::Manual),
            text: Some(&text),
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::STTResult(..)
    ));

    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::LLMResult(..)
    ));

    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::Start),
            ..
        })
    ));

    let frame_result = output.next().await.unwrap().unwrap();
    info!("{:?}", frame_result);
    assert!(matches!(
        frame_result,
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::SentenceStart),
            ..
        })
    ));

    // has some audio result,detect first one
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::AudioResult(AudioMessage { .. })
    ));

    while let Some(data) = output.next().await {
        match data {
            Ok(frame_result) => {
                if let FrameResult::TTSResult(tts_message) = frame_result {
                    let state = tts_message.state;
                    if let Some(state) = state
                        && TtsState::Stop == state
                    {
                        break;
                    }
                }
            }
            Err(e) => {
                panic!("{:?}", e)
            }
        }
    }
    // join_handle.await.unwrap();
    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(&container).await;
    Ok(())
}

async fn create_session()
-> Result<(Session, Option<ContainerAsync<Postgres>>, AppState), anyhow::Error> {
    info!("init vad factory");
    VadFactory::init().await;
    info!("init vad factory successfully");
    info!("init asr factory");
    AsrFactory::init().await;
    info!("init asr factory successfully");
    tracing::info!("init llm factory");
    LlmFactory::init().await;
    tracing::info!("init llm factory successfully");
    tracing::info!("init tts factory");
    TtsFactory::init().await?;
    tracing::info!("init tts factory successfully");

    let (container, state) = setup_database().await;

    // server client
    let router = OpenApiRouter::new();
    let ct = tokio_util::sync::CancellationToken::new();
    let router = setup_mcp(router, state.clone(), ct.child_token())
        .split_for_parts()
        .0;
    let config = StreamableHttpClientTransportConfig {
        uri: "/mcp".into(),
        ..Default::default()
    };
    let client = RouterClient { router };
    let transport = StreamableHttpClientTransport::with_client(client, config);
    let mut server_client = ServerMcpClient::new(transport).await?;
    server_client.init().await?;

    let id = gen_id();
    let mut mcp_host = UnionMcpHost::new(Some(id.clone()));
    // server client add
    mcp_host.add_client(Box::new(server_client)).await;
    let session_config = SessionConfig {
        close_connection_no_voice_time: Some(
            config::get().logic().close_connection_no_voice_time(),
        ),
    };
    let session = SessionBuilder::new()
        .with_listener(Box::new(DefaultListener::new(
            Arc::new(Mutex::new(VadFactory::create_model())),
            AsrFactory::global().default(),
        )))
        .with_id(id.clone())
        .with_model(LlmFactory::global().default())
        .with_mcp_host(Arc::new(Mutex::new(mcp_host)))
        .with_config(session_config)
        .build();
    Ok((session, container, state))
}

fn to_json_rpc_response<T>(id: i64, result: T) -> JsonRpcMessage
where
    T: Serialize,
{
    JsonRpcMessage::Response(JsonRpcResponse {
        jsonrpc: JsonRpcVersion2_0,
        id: RequestId::Number(id),
        result: to_json_object(result),
    })
}

fn to_json_object<T>(value: T) -> JsonObject
where
    T: Serialize,
{
    object(serde_json::to_value(value).unwrap())
}
