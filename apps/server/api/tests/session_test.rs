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
    config::{
        AsrModel, LlmModel, TtsModel, asr::AsrConfig, audio::AudioConfig, llm::LlmConfig,
        session::SessionConfig, tts::TtsConfig, vad::VadConfig,
    },
    llm::LlmFactory,
    mcp::{
        client::server::ServerMcpClient,
        mcp_host::{McpHost, UnionMcpHost},
    },
    setup_mcp,
    tts::TtsFactory,
    util::audio::pcm_decode,
    vad::VadFactory,
    ws::session::SessionBuilder,
};

use api::{
    ws::frame::{Frame, FrameResult},
    ws::session::{Session, listener::DefaultListener},
};
use framework::id::gen_id;
use rmcp::{
    model::{
        CallToolResult, Content, Icon, Implementation, InitializeResult, JsonObject,
        JsonRpcMessage, JsonRpcResponse, JsonRpcVersion2_0, ListToolsResult, ProtocolVersion,
        RawTextContent, RequestId, ServerCapabilities, object,
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
use tracing::{debug, info};
use tracing_test::traced_test;
use utoipa_axum::router::OpenApiRouter;

mod common;
use common::{router_client::RouterClient, setup_database, tear_down};

#[tokio::test]
#[traced_test]
#[ignore]
/// hello paramter input and output the hello result
/// cargo test --test session_test -- test_chat_flow_hello --ignored --nocapture
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
/// cargo test --test session_test -- test_chat_flow_listen_manual --ignored --nocapture
async fn test_chat_flow_listen_manual() -> anyhow::Result<()> {
    let audio = get_audio();
    let (mut session, container, state) = create_session().await?;
    // let session_id = session.id.clone();
    session.start().await?;
    let mut output = session.output_frame().await;
    // TODO: need refactor,remove tokio::spawn
    let join_handle = tokio::spawn(async move {
        while let Some(data) = output.next().await {
            // debug!("session id = {}, data = {:?}", session_id, data);
            match data {
                Ok(frame_result) => match frame_result {
                    FrameResult::HelloResult(_hello_message) => {}
                    FrameResult::STTResult(stt_message) => {
                        info!("{:?}", stt_message);
                    }
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
/// cargo test --test session_test -- test_chat_flow_listen_auto --ignored --nocapture
async fn test_chat_flow_listen_auto() -> anyhow::Result<()> {
    let audio = get_audio();
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
            debug!("session id = {}, data = {:?}", session_id, data);
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
    let mut to_next_step = false;
    while !to_next_step {
        to_next_step = next_step_for_sender.load(Ordering::Relaxed);
        sleep(Duration::from_millis(500)).await;
    }
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Start,
            mmod: Some(ListenMode::Auto),
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
    // silent time = 1800ms > config setting
    for _ in 0..30 {
        session
            .accept_frame(&Frame::Voice {
                data: vec![].as_ref(),
            })
            .await;
        sleep(Duration::from_millis(60)).await;
    }
    join_handle.await?;
    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(&container).await;
    Ok(())
}

#[tokio::test]
#[ignore]
/// listen voice by realtime mode and output the asr text result
/// cargo test --test session_test -- test_chat_flow_listen_realtime --ignored --nocapture
async fn test_chat_flow_listen_realtime() -> anyhow::Result<()> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .compact()
            .with_max_level(tracing::Level::DEBUG)
            .finish(),
    )
    .expect("Failed to set tracing subscriber");

    let audio = get_audio();

    let (mut session, container, state) = create_session().await?;
    let next_step = Arc::new(AtomicBool::new(false));
    let next_step_for_sender = next_step.clone();

    // let session_id = session.id.clone();
    session.start().await?;
    let mut output = session.output_frame().await;
    // TODO: need refactor,remove tokio::spawn
    let join_handle = tokio::spawn(async move {
        let mut count = 0;
        while let Some(data) = output.next().await {
            // debug!("session id = {}, data = {:?}", session_id, data);
            match data {
                Ok(frame_result) => match frame_result {
                    FrameResult::HelloResult(hello_message) => {
                        info!("{:?}", hello_message);
                    }
                    FrameResult::STTResult(stt_message) => {
                        info!("{:?}", stt_message);
                    }
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
    let mut to_next_step = false;
    while !to_next_step {
        to_next_step = next_step_for_sender.load(Ordering::Relaxed);
        sleep(Duration::from_millis(500)).await;
    }
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
    // silent time = 1800ms > config setting
    for _ in 0..30 {
        session
            .accept_frame(&Frame::Voice {
                data: vec![].as_ref(),
            })
            .await;
        sleep(Duration::from_millis(60)).await;
    }
    join_handle.await?;
    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(&container).await;
    Ok(())
}

#[tokio::test]
#[ignore]
/// get text message and output the asr text result
/// cargo test --test session_test -- test_chat_flow_handle_text_message_multiple_time --ignored --nocapture
async fn test_chat_flow_handle_text_message_multiple_time() -> anyhow::Result<()> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .compact()
            .with_max_level(tracing::Level::TRACE)
            .finish(),
    )
    .expect("Failed to set tracing subscriber");
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
    // let mut user_answer = vec![String::from("世界上第高的山是什么，只回答结果不用详细介绍")];
    let mut user_answer = vec![String::from("世界上第高的山是什么")];
    for index in 2..20 {
        let text = format!("第{}高的呢?", index).to_owned();
        user_answer.push(text);
    }
    for index in 0..user_answer.len() {
        session
            .accept_frame(&Frame::Listen(ListenMessage {
                state: ListenState::Detect,
                mmod: Some(service::chobits::message::listen::ListenMode::Manual),
                text: Some(user_answer.get(index).unwrap()),
                ..Default::default()
            }))
            .await;
        let frame_result = output.next().await.unwrap().unwrap();
        debug!("{:?}", &frame_result);
        assert!(matches!(frame_result, FrameResult::STTResult(..)));

        assert!(matches!(
            output.next().await.unwrap().unwrap(),
            FrameResult::TTSResult(TtsMessage {
                state: Some(TtsState::Start),
                ..
            })
        ));

        let frame_result = output.next().await.unwrap().unwrap();
        debug!("{:?}", &frame_result);
        assert!(matches!(frame_result, FrameResult::LLMResult(..)));

        let frame_result = output.next().await.unwrap().unwrap();
        debug!("{:?}", frame_result);
        assert!(matches!(
            frame_result,
            FrameResult::TTSResult(TtsMessage {
                state: Some(TtsState::SentenceStart),
                ..
            })
        ));
        // has some audio result,detect first one
        let frame_result = output.next().await.unwrap().unwrap();
        debug!("{:?}", frame_result);
        assert!(matches!(
            frame_result,
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
    }
    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(&container).await;
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// get text message and output the asr text result
/// cargo test --test session_test -- test_chat_flow_handle_text_message --ignored --nocapture
async fn test_chat_flow_handle_text_message() -> anyhow::Result<()> {
    let (mut session, container, state) = create_session().await?;
    let session_id = session.id.clone();
    session.start().await?;
    let mut output = session.output_frame().await;
    // TODO: need refactor,remove tokio::spawn
    let join_handle = tokio::spawn(async move {
        while let Some(data) = output.next().await {
            debug!("session id = {}, data = {:?}", session_id, data);
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
/// cargo test --test session_test -- test_chat_flow_break --ignored --nocapture
async fn test_chat_flow_break() -> anyhow::Result<()> {
    let (mut session, container, state) = create_session().await?;
    let session_id = session.id.clone();
    session.start().await?;
    let mut output = session.output_frame().await;
    let mut count = 0;
    // TODO: need refactor,remove tokio::spawn
    let join_handle = tokio::spawn(async move {
        while let Some(data) = output.next().await {
            debug!("session id = {}, data = {:?}", session_id, data);
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
/// cargo test --test session_test -- test_mcp_flow_server_client --ignored --nocapture
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
    //         debug!("session id = {}, data = {:?}", session_id, data);
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
                    description: None,
                },
                instructions: None,
            },
        ))))
        .await;

    let frame_result = output.next().await.unwrap().unwrap();
    debug!("{:?}", &frame_result);
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
            text: Some("现在几点"),
            ..Default::default()
        }))
        .await;

    let frame_result = output.next().await.unwrap().unwrap();
    debug!("{:?}", &frame_result);
    assert!(matches!(frame_result, FrameResult::STTResult(..)));

    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::Start),
            ..
        })
    ));

    let frame_result = output.next().await.unwrap().unwrap();
    debug!("{:?}", &frame_result);
    assert!(matches!(frame_result, FrameResult::LLMResult(..)));

    let frame_result = output.next().await.unwrap().unwrap();
    debug!("{:?}", frame_result);
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

#[tokio::test]
#[traced_test]
#[ignore]
/// Shell command:
/// ``` shell
/// cargo test --test session_test -- test_mcp_flow_device_client --ignored --nocapture
/// ```
async fn test_mcp_flow_device_client() -> anyhow::Result<()> {
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
    //         debug!("session id = {}, data = {:?}", session_id, data);
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
                    description: None,
                },
                instructions: None,
            },
        ))))
        .await;

    let frame_result = output.next().await.unwrap().unwrap();
    debug!("{:?}", &frame_result);
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
            text: Some("get device status"),
            ..Default::default()
        }))
        .await;

    let frame_result = output.next().await.unwrap().unwrap();
    debug!("{:?}", &frame_result);
    assert!(matches!(frame_result, FrameResult::STTResult(..)));

    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::Start),
            ..
        })
    ));

    let frame_result = output.next().await.unwrap().unwrap();
    debug!("{:?}", &frame_result);
    assert!(matches!(frame_result, FrameResult::McpResult(..)));

    let mcp_tool_call_response = r#"
    {
    "audio_speaker": {
        "volume": 50,
        "muted": false
    },
    "screen": {
        "brightness": 80,
        "theme": "light"
    },
    "battery": {
        "level": 85,
        "charging": false
    },
    "network": {
        "connected": true,
        "type": "wifi"
    }
    }"#;
    session
        .accept_frame(&Frame::Mcp(McpMessage::new(to_json_rpc_response(
            request_id.fetch_add(1, Ordering::Relaxed),
            CallToolResult {
                content: vec![Content {
                    raw: rmcp::model::RawContent::Text(RawTextContent {
                        text: mcp_tool_call_response.to_string(),
                        meta: None,
                    }),
                    annotations: None,
                }],
                structured_content: None,
                is_error: None,
                meta: None,
            },
        ))))
        .await;

    let frame_result = output.next().await.unwrap().unwrap();
    debug!("{:?}", &frame_result);
    assert!(matches!(frame_result, FrameResult::LLMResult(..)));

    let frame_result = output.next().await.unwrap().unwrap();
    debug!("{:?}", frame_result);
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
    debug!("init vad factory");
    VadFactory::init(Arc::new(VadConfig {
        path: Some(String::from("data/vad/model/onnx-community/silero-vad/")),
        num_threads: Some(4),
    }))
    .await;
    debug!("init vad factory successfully");
    debug!("init asr factory");
    AsrFactory::init(Arc::new(AsrConfig {
        model: Some(AsrModel::Qwen3),
        path: Some(String::from("data/asr/model/Qwen/Qwen3-ASR-0.6B/")),
    }))
    .await;
    debug!("init asr factory successfully");
    tracing::debug!("init llm factory");
    LlmFactory::init(Arc::new(LlmConfig {
        model: Some(LlmModel::Qwen3),
        path: Some(String::from("data/llm/model/unsloth/Qwen3-1.7B-GGUF/")),
    }))
    .await;
    tracing::debug!("init llm factory successfully");
    tracing::debug!("init tts factory");
    let audio_config = Arc::new(AudioConfig {
        input_sample_rate: Some(16000),
        input_frame_duration: Some(60_u64),
        input_channel: Some(1),
        output_sample_rate: Some(16000),
        output_channel: Some(1),
        output_frame_duration: Some(60_u64),
    });
    TtsFactory::init(
        Arc::new(TtsConfig {
            model: Some(TtsModel::Voxcpm),
            path: Some(String::from("data/tts/model/openbmb/VoxCPM-0.5B/")),
            reference_prompt_text: Some(String::from(
                "一定被灰太狼给吃了，我已经为他准备好了花圈了",
            )),
            reference_prompt_wav_path: Some(String::from("file://data/tts/reference/voice_05.wav")),
        }),
        audio_config.clone(),
    )
    .await?;
    tracing::debug!("init tts factory successfully");

    let (container, state) = setup_database().await;

    // server client
    let router = OpenApiRouter::new();
    let ct = tokio_util::sync::CancellationToken::new();
    let router = setup_mcp(router, state.clone(), ct.child_token())
        .split_for_parts()
        .0;
    let mcp_config = StreamableHttpClientTransportConfig {
        uri: "/mcp".into(),
        ..Default::default()
    };
    let client = RouterClient { router };
    let transport = StreamableHttpClientTransport::with_client(client, mcp_config);
    let mut server_client = ServerMcpClient::new(transport).await?;
    server_client.init().await?;

    let id = gen_id();
    let mut mcp_host = UnionMcpHost::new(Some(id.clone()));
    // server client add
    mcp_host.add_client(Box::new(server_client)).await;
    let session_config = Arc::new(SessionConfig {
        close_connection_no_voice_time: Some(30000),
        silence_voice_timeout: Some(1200),
        system_prompt: Some(String::from(
            "你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等。",
        )),
        max_prompt_len: Some(3000),
    });
    let vad_config = VadConfig {
        path: Some(String::from("data/vad/model/onnx-community/silero-vad/")),
        num_threads: Some(4),
    };
    let session = SessionBuilder::new()
        .with_listener(Box::new(DefaultListener::new(
            Arc::new(Mutex::new(VadFactory::create_model(&Arc::new(vad_config)))),
            AsrFactory::global().default(),
            audio_config.clone(),
        )))
        .with_id(id.clone())
        .with_model(LlmFactory::global().default())
        .with_mcp_host(Arc::new(Mutex::new(mcp_host)))
        .with_config(session_config.clone())
        .with_audio_config(audio_config.clone())
        .build();
    Ok((session, container, state))
}

fn get_audio() -> Vec<Vec<u8>> {
    use std::path::PathBuf;

    let wav_file: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "resources",
        "test",
        "samples_jfk.wav",
    ]
    .iter()
    .collect();
    debug!("{}", wav_file.display());
    let (pcm_data, sample_rate) = pcm_decode(wav_file).unwrap();
    debug!(
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
    debug!("size = {}", size);
    let len = pcm_data.len();
    let mut count = len / size;
    if len % size > 0 {
        count += 1;
    }
    debug!("count = {}", count);
    let mut audio: Vec<Vec<u8>> = Vec::new();

    for n in 0..count {
        let start = n * size;
        let end = cmp::min((n + 1) * size, len);
        let packet = encoder
            .encode_vec_float(&pcm_data[start..end], size)
            .unwrap();
        audio.push(packet);
    }
    audio
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
