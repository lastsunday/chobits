use api::{
    AppState,
    asr::AsrFactory,
    config::{
        AsrModel, LlmModel, TtsModel, VadModel, asr::AsrConfig, audio::AudioConfig, llm::LlmConfig,
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
    ws::frame::{Frame, FrameResult},
    ws::session::SessionBuilder,
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
use std::{
    cmp,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
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
    let mut session = create_mini_session().await;
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
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/*
2026-03-16T09:26:06.988023Z DEBUG frame: [RECV] Hello(HelloMessage { message: Message { mtype: Hello }, version: None, transport: None, audio_params: None, features: Some(Feature { mcp: Some(true), aec: None }), session_id: None })
2026-03-16T09:26:06.988091Z DEBUG frame: [SEND] HelloResult(HelloMessage { message: Message { mtype: Hello }, version: None, transport: Some(Websocket), audio_params: Some(AudioParam { format: Opus, sample_rate: 16000, channels: 1, frame_duration: 60 }), features: None, session_id: Some("d6rspbklm6jn11rmp49g") })
2026-03-16T09:26:06.988133Z DEBUG frame: [SEND] McpResult(McpRequest { message: Message { mtype: Mcp }, session_id: Some("d6rspbklm6jn11rmp49g"), payload: JsonRpcRequest { jsonrpc: JsonRpcVersion2_0, id: Number(0), request: Request { method: "initialize", params: {"capabilities": Object {}, "clientInfo": Object {"name": String("rmcp"), "version": String("0.15.0")}, "protocolVersion": String("2025-06-18")}, extensions: Extensions } } })
2026-03-16T09:26:07.037845Z DEBUG frame: [RECV] Mcp(McpMessage { message: Message { mtype: Mcp }, payload: Response(JsonRpcResponse { jsonrpc: JsonRpcVersion2_0, id: Number(0), result: {"capabilities": Object {"tools": Object {}}, "protocolVersion": String("2025-06-18"), "serverInfo": Object {"name": String("Web测试设备"), "version": String("1.0.0")}} }) })
2026-03-16T09:26:07.037933Z DEBUG frame: [SEND] McpResult(McpRequest { message: Message { mtype: Mcp }, session_id: Some("d6rspbklm6jn11rmp49g"), payload: JsonRpcRequest { jsonrpc: JsonRpcVersion2_0, id: Number(1), request: Request { method: "tools/list", params: {}, extensions: Extensions } } })
2026-03-16T09:26:07.045113Z DEBUG frame: [RECV] Mcp(McpMessage { message: Message { mtype: Mcp }, payload: Response(JsonRpcResponse { jsonrpc: JsonRpcVersion2_0, id: Number(1), result: {"tools": Array [Object {"description": String("Provides the real-time information of the device, including the current status of the audio speaker, screen, battery, network, etc.\nUse this tool for: \n1. Answering questions about current condition (e.g. what is the current volume of the audio speaker?)\n2. As the first step to control the device (e.g. turn up / down the volume of the audio speaker, etc.)"), "inputSchema": Object {"properties": Object {}, "type": String("object")}, "name": String("self.get_device_status")}, Object {"description": String("Set the volume of the audio speaker. If the current volume is unknown, you must call `self.get_device_status` tool first and then call this tool."), "inputSchema": Object {"properties": Object {"volume": Object {"maximum": Number(100), "minimum": Number(0), "type": String("integer")}}, "required": Array [String("volume")], "type": String("object")}, "name": String("self.audio_speaker.set_volume")}, Object {"description": String("Set the brightness of the screen."), "inputSchema": Object {"properties": Object {"brightness": Object {"maximum": Number(100), "minimum": Number(0), "type": String("integer")}}, "required": Array [String("brightness")], "type": String("object")}, "name": String("self.screen.set_brightness")}]} }) })
2026-03-16T09:26:09.324390Z DEBUG frame: [RECV] Listen(ListenMessage { message: Message { mtype: Listen }, session_id: None, state: Start, mmod: Some(Manual), text: None })
2026-03-16T09:26:09.813342Z TRACE frame: [RECV] Voice
2026-03-16T09:26:11.850049Z DEBUG frame: [RECV] Listen(ListenMessage { message: Message { mtype: Listen }, session_id: None, state: Stop, mmod: Some(Manual), text: None })
2026-03-16T09:26:13.505227Z DEBUG frame: [SEND] STTResult(SttMessage { message: Message { mtype: Stt }, session_id: Some("d6rspbklm6jn11rmp49g"), text: Some("现在几点？") })
2026-03-16T09:26:13.505307Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rspbklm6jn11rmp49g"), state: Some(Start), text: None })
2026-03-16T09:26:22.902319Z DEBUG frame: [SEND] LLMResult(LlmMessage { message: Message { mtype: Llm }, session_id: Some("d6rspbklm6jn11rmp49g"), emotion: Some("happy"), text: Some("🙂") })
2026-03-16T09:26:22.902383Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rspbklm6jn11rmp49g"), state: Some(SentenceStart), text: Some("2026年3月16日17点26分16秒（北京时间）") })
2026-03-16T09:26:22.902418Z TRACE frame: [SEND] Audio
2026-03-16T09:26:28.526632Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rspbklm6jn11rmp49g"), state: Some(SentenceEnd), text: None })
2026-03-16T09:26:28.526648Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rspbklm6jn11rmp49g"), state: Some(Stop), text: None })
*/
/// listen voice by manual mode and output the asr text result
/// cargo test -F cuda --test session_test -- test_chat_flow_listen_manual --exact --ignored --nocapture
async fn test_chat_flow_listen_manual() -> anyhow::Result<()> {
    let audio = get_audio();
    let mut session = create_mini_session().await;
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
            mmod: Some(service::chobits::message::listen::ListenMode::Manual),

            ..Default::default()
        }))
        .await;
    loop {
        let data = output.next().await.unwrap().unwrap();
        if let FrameResult::TTSResult(tts_message) = data {
            match tts_message.state {
                Some(TtsState::Stop) => break,
                Some(_) => {}
                None => {
                    //skip
                }
            }
        }
    }
    session.stop().await;
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/*
[RECV] Hello(HelloMessage { message: Message { mtype: Hello }, version: Some(1), transport: Some(Websocket), audio_params: Some(AudioParam { format: Opus, sample_rate: 16000, channels: 1, frame_duration: 60 }), features: None, session_id: None })
[SEND] HelloResult(HelloMessage { message: Message { mtype: Hello }, version: None, transport: Some(Websocket), audio_params: Some(AudioParam { format: Opus, sample_rate: 16000, channels: 1, frame_duration: 60 }), features: None, session_id: Some("d6rt1rklm6jnl6b7cck0") })
[RECV] Listen(ListenMessage { message: Message { mtype: Listen }, session_id: Some("d6rt1rklm6jnl6b7cck0"), state: Start, mmod: Some(Auto), text: None })
[RECV] Voice
[SEND] STTResult(SttMessage { message: Message { mtype: Stt }, session_id: Some("d6ruu3clm6jrmr2f5itg"), text: Some("Hello") })
[SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6ruu3clm6jrmr2f5itg"), state: Some(Start), text: None })
[SEND] LLMResult(LlmMessage { message: Message { mtype: Llm }, session_id: Some("d6ruu3clm6jrmr2f5itg"), emotion: Some("happy"), text: Some("🙂") })
[SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6ruu3clm6jrmr2f5itg"), state: Some(SentenceStart), text: Some("Hello!") })
[RECV] Voice
[SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6ruu3clm6jrmr2f5itg"), state: Some(SentenceEnd), text: None })
[SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6ruu3clm6jrmr2f5itg"), state: Some(Stop), text: None })
...
[SEND] CloseResult
*/
/// listen voice by auto mode and output the asr text result
/// cargo test -F cuda --test session_test -- test_chat_flow_listen_auto --exact --ignored --nocapture
async fn test_chat_flow_listen_auto() -> anyhow::Result<()> {
    let audio = get_audio();
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
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Start,
            mmod: Some(ListenMode::Auto),
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::STTResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::Start),
            ..
        })
    ));
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::LLMResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::SentenceStart),
            ..
        })
    ));
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::AudioResult(..)
    ));

    let mut frame_result = output.next().await.unwrap().unwrap();
    while let Some(data) = output.next().await {
        let data = data.unwrap();
        match data {
            FrameResult::AudioResult(_audio_message) => {
                continue;
            }
            _ => {
                frame_result = data;
                break;
            }
        }
    }
    debug!("{:?}", &frame_result);
    assert!(matches!(
        frame_result,
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::SentenceEnd),
            ..
        })
    ));
    while let Some(data) = output.next().await {
        let data = data.unwrap();
        debug!("{:?}", data);
        match data {
            FrameResult::TTSResult(tts_message) => match tts_message.state {
                Some(state) => {
                    if state == TtsState::Stop {
                        break;
                    }
                }
                None => {
                    continue;
                }
            },
            _ => {
                continue;
            }
        }
    }

    for n in 0..audio.len() {
        session
            .accept_frame(&Frame::Voice {
                data: audio.get(n).unwrap(),
            })
            .await;
    }

    info!("send voice");
    info!("audio len = {}", audio.len());
    for n in 0..audio.len() {
        session
            .accept_frame(&Frame::Voice {
                data: audio.get(n).unwrap(),
            })
            .await;
        sleep(Duration::from_millis(20)).await;
    }
    info!("send silent voice");
    // 16000Hz * 1 channel * 20 ms / 1000 = 320 samples -> frameSize
    // 20ms * 360 = 7200ms
    // silent time = 7200ms > config setting
    for _ in 0..360 {
        session
            .accept_frame(&Frame::Voice {
                data: vec![0u8; 320].as_ref(),
            })
            .await;
        sleep(Duration::from_millis(20)).await;
    }
    while let Some(data) = output.next().await {
        let data = data.unwrap();
        match data {
            FrameResult::CloseResult => {
                break;
            }
            _ => {
                continue;
            }
        }
    }

    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(container).await;
    Ok(())
}

#[tokio::test]
#[ignore]
/*
2026-03-16T07:51:51.451299Z DEBUG frame: [RECV] Hello(HelloMessage { message: Message { mtype: Hello }, version: Some(1), transport: Some(Websocket), audio_params: Some(AudioParam { format: Opus, sample_rate: 16000, channels: 1, frame_duration: 60 }), features: Some(Feature { mcp: Some(true), aec: None }), session_id: None })
2026-03-16T07:51:51.453883Z DEBUG frame: [SEND] HelloResult(HelloMessage { message: Message { mtype: Hello }, version: None, transport: Some(Websocket), audio_params: Some(AudioParam { format: Opus, sample_rate: 16000, channels: 1, frame_duration: 60 }), features: None, session_id: Some("d6rrd5slm6ji1occegj0") })
2026-03-16T07:51:51.453939Z DEBUG frame: [SEND] McpResult(McpRequest { message: Message { mtype: Mcp }, session_id: Some("d6rrd5slm6ji1occegj0"), payload: JsonRpcRequest { jsonrpc: JsonRpcVersion2_0, id: Number(0), request: Request { method: "initialize", params: {"capabilities": Object {}, "clientInfo": Object {"name": String("rmcp"), "version": String("0.15.0")}, "protocolVersion": String("2025-06-18")}, extensions: Extensions } } })
2026-03-16T07:51:51.480161Z TRACE frame: [RECV] Voice
2026-03-16T07:51:51.562556Z DEBUG frame: [RECV] Listen(ListenMessage { message: Message { mtype: Listen }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Detect, mmod: None, text: Some("你好小智") })
2026-03-16T07:51:53.755786Z DEBUG frame: [RECV] Listen(ListenMessage { message: Message { mtype: Listen }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Start, mmod: Some(RealTime), text: None })
2026-03-16T07:51:53.755847Z DEBUG frame: [RECV] Mcp(McpMessage { message: Message { mtype: Mcp }, payload: Response(JsonRpcResponse { jsonrpc: JsonRpcVersion2_0, id: Number(0), result: {"capabilities": Object {"tools": Object {}}, "protocolVersion": String("2024-11-05"), "serverInfo": Object {"name": String("lichuang-dev"), "version": String("2.2.4")}} }) })
2026-03-16T07:51:53.755903Z DEBUG frame: [SEND] McpResult(McpRequest { message: Message { mtype: Mcp }, session_id: Some("d6rrd5slm6ji1occegj0"), payload: JsonRpcRequest { jsonrpc: JsonRpcVersion2_0, id: Number(1), request: Request { method: "tools/list", params: {}, extensions: Extensions } } })
2026-03-16T07:51:53.759800Z TRACE frame: [RECV] Voice
2026-03-16T07:51:53.759932Z DEBUG frame: [SEND] STTResult(SttMessage { message: Message { mtype: Stt }, session_id: Some("d6rrd5slm6ji1occegj0"), text: Some("你好小智") })
2026-03-16T07:51:53.759949Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Some(Start), text: None })
2026-03-16T07:51:53.760358Z TRACE frame: [RECV] Voice
2026-03-16T07:51:53.799160Z DEBUG frame: [RECV] Mcp(McpMessage { message: Message { mtype: Mcp }, payload: Response(JsonRpcResponse { jsonrpc: JsonRpcVersion2_0, id: Number(1), result: {"tools": Array [Object {"description": String("Provides the real-time information of the device, including the current status of the audio speaker, screen, battery, network, etc.\nUse this tool for: \n1. Answering questions about current condition (e.g. what is the current volume of the audio speaker?)\n2. As the first step to control the device (e.g. turn up / down the volume of the audio speaker, etc.)"), "inputSchema": Object {"properties": Object {}, "type": String("object")}, "name": String("self.get_device_status")}, Object {"description": String("Set the volume of the audio speaker. If the current volume is unknown, you must call `self.get_device_status` tool first and then call this tool."), "inputSchema": Object {"properties": Object {"volume": Object {"maximum": Number(100), "minimum": Number(0), "type": String("integer")}}, "required": Array [String("volume")], "type": String("object")}, "name": String("self.audio_speaker.set_volume")}, Object {"description": String("Set the brightness of the screen."), "inputSchema": Object {"properties": Object {"brightness": Object {"maximum": Number(100), "minimum": Number(0), "type": String("integer")}}, "required": Array [String("brightness")], "type": String("object")}, "name": String("self.screen.set_brightness")}, Object {"description": String("Set the theme of the screen. The theme can be `light` or `dark`."), "inputSchema": Object {"properties": Object {"theme": Object {"type": String("string")}}, "required": Array [String("theme")], "type": String("object")}, "name": String("self.screen.set_theme")}, Object {"description": String("Always remember you have a camera. If the user asks you to see something, use this tool to take a photo and then explain it.\nArgs:\n  `question`: The question that you want to ask about the photo.\nReturn:\n  A JSON object that provides the photo information."), "inputSchema": Object {"properties": Object {"question": Object {"type": String("string")}}, "required": Array [String("question")], "type": String("object")}, "name": String("self.camera.take_photo")}, Object {"description": String("End this conversation and enter WiFi configuration mode.\n**CAUTION** You must ask the user to confirm this action."), "inputSchema": Object {"properties": Object {}, "type": String("object")}, "name": String("self.system.reconfigure_wifi")}]} }) })
2026-03-16T07:51:53.799222Z TRACE frame: [RECV] Voice
2026-03-16T07:51:58.042580Z DEBUG frame: [SEND] LLMResult(LlmMessage { message: Message { mtype: Llm }, session_id: Some("d6rrd5slm6ji1occegj0"), emotion: Some("happy"), text: Some("🙂") })
2026-03-16T07:51:58.042633Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Some(SentenceStart), text: Some("你好！") })
2026-03-16T07:51:58.042646Z TRACE frame: [SEND] Audio
2026-03-16T07:51:58.052116Z TRACE frame: [RECV] Voice
2026-03-16T07:51:59.079549Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Some(SentenceEnd), text: None })
2026-03-16T07:51:59.143718Z TRACE frame: [RECV] Voice
2026-03-16T07:52:00.149167Z DEBUG frame: [SEND] LLMResult(LlmMessage { message: Message { mtype: Llm }, session_id: Some("d6rrd5slm6ji1occegj0"), emotion: Some("happy"), text: Some("🙂") })
2026-03-16T07:52:00.149215Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Some(SentenceStart), text: Some("有什么可以帮助你的吗？") })
2026-03-16T07:52:00.149239Z TRACE frame: [SEND] Audio
2026-03-16T07:52:00.171715Z TRACE frame: [RECV] Voice
2026-03-16T07:52:03.632112Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Some(SentenceEnd), text: None })
2026-03-16T07:52:03.632139Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Some(Stop), text: None })
2026-03-16T07:52:03.659201Z TRACE frame: [RECV] Voice
2026-03-16T07:52:34.559563Z DEBUG frame: [SEND] STTResult(SttMessage { message: Message { mtype: Stt }, session_id: Some("d6rrd5slm6ji1occegj0"), text: Some("現在幾點？") })
2026-03-16T07:52:34.559627Z TRACE frame: [RECV] Voice
2026-03-16T07:52:34.559627Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Some(Start), text: None })
2026-03-16T07:52:34.559757Z TRACE frame: [RECV] Voice
2026-03-16T07:52:55.979754Z DEBUG frame: [SEND] LLMResult(LlmMessage { message: Message { mtype: Llm }, session_id: Some("d6rrd5slm6ji1occegj0"), emotion: Some("happy"), text: Some("🙂") })
2026-03-16T07:52:55.979802Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Some(SentenceStart), text: Some("现在是2026年3月16日，当前时间为上午15:52。") })
2026-03-16T07:52:55.979840Z TRACE frame: [SEND] Audio
2026-03-16T07:52:56.013138Z TRACE frame: [RECV] Voice
2026-03-16T07:53:01.845134Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Some(SentenceEnd), text: None })
2026-03-16T07:53:01.845194Z DEBUG frame: [SEND] TTSResult(TtsMessage { message: Message { mtype: Tts }, session_id: Some("d6rrd5slm6ji1occegj0"), state: Some(Stop), text: None })
2026-03-16T07:53:01.892041Z TRACE frame: [RECV] Voice
* */
/// listen voice by realtime mode and output the asr text result
/// cargo test -F cuda --test session_test -- test_chat_flow_listen_realtime --exact --ignored --nocapture
async fn test_chat_flow_listen_realtime() -> anyhow::Result<()> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .compact()
            .with_max_level(tracing::Level::TRACE)
            .finish(),
    )
    .expect("Failed to set tracing subscriber");
    let audio = get_audio();

    let (mut session, container, state) = create_session().await?;
    // let session_id = session.id.clone();
    session.start().await?;
    let mut output = session.output_frame().await;
    info!("send hello");
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::HelloResult(..)
    ));
    info!("send before hello voice");
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
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::STTResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::TTSResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::LLMResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::TTSResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::AudioResult(..)
    ));

    let mut frame_result = output.next().await.unwrap().unwrap();
    while let Some(data) = output.next().await {
        let data = data.unwrap();
        match data {
            FrameResult::AudioResult(_audio_message) => {
                continue;
            }
            _ => {
                frame_result = data;
                break;
            }
        }
    }
    debug!("{:?}", &frame_result);
    assert!(matches!(frame_result, FrameResult::TTSResult(..)));

    info!("send voice");
    info!("audio len = {}", audio.len());
    for n in 0..audio.len() {
        session
            .accept_frame(&Frame::Voice {
                data: audio.get(n).unwrap(),
            })
            .await;
        sleep(Duration::from_millis(20)).await;
    }
    info!("send silent voice");
    // 16000Hz * 1 channel * 20 ms / 1000 = 320 samples -> frameSize
    // 20ms * 90 = 1800ms
    // silent time = 1800ms > config setting
    for _ in 0..90 {
        session
            .accept_frame(&Frame::Voice {
                data: vec![0u8; 320].as_ref(),
            })
            .await;
        sleep(Duration::from_millis(20)).await;
    }
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::LLMResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::TTSResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::AudioResult(..)
    ));

    let mut frame_result = output.next().await.unwrap().unwrap();
    while let Some(data) = output.next().await {
        let data = data.unwrap();
        match data {
            FrameResult::AudioResult(_audio_message) => {
                continue;
            }
            _ => {
                frame_result = data;
                break;
            }
        }
    }
    assert!(matches!(
        frame_result,
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::SentenceEnd),
            ..
        })
    ));
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::Stop),
            ..
        })
    ));
    for _ in 0..120 {
        session
            .accept_frame(&Frame::Voice {
                data: vec![0u8; 320].as_ref(),
            })
            .await;
        sleep(Duration::from_millis(20)).await;
    }

    while let Some(data) = output.next().await {
        let data = data.unwrap();
        if let FrameResult::TTSResult(tts_message) = data
            && let Some(TtsState::Stop) = tts_message.state
        {
            break;
        }
    }
    for _ in 0..120 {
        session
            .accept_frame(&Frame::Voice {
                data: vec![0u8; 320].as_ref(),
            })
            .await;
        sleep(Duration::from_millis(20)).await;
    }

    info!("close result checking");
    assert!(matches!(
        output.next().await.unwrap().unwrap(),
        FrameResult::CloseResult
    ));

    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(container).await;
    Ok(())
}

#[tokio::test]
#[ignore]
/// listen voice by realtime mode and output the asr text result
/// cargo test --test session_test -- test_chat_flow_listen_realtime_silent_voice_connection_timeout --exact --ignored --nocapture
async fn test_chat_flow_listen_realtime_silent_voice_connection_timeout() -> anyhow::Result<()> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .compact()
            .with_max_level(tracing::Level::DEBUG)
            .finish(),
    )
    .expect("Failed to set tracing subscriber");
    let mut session = create_mini_session().await;
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
    // 16000Hz * 1 channel * 20 ms / 1000 = 320 samples -> frameSize
    // 20ms * 90 = 1800ms
    // silent time = 1800ms > config setting
    for _ in 0..90 {
        session
            .accept_frame(&Frame::Voice {
                data: vec![0u8; 320].as_ref(),
            })
            .await;
    }
    loop {
        let data = output.next().await.unwrap().unwrap();
        if let FrameResult::TTSResult(tts_message) = data {
            match tts_message.state {
                Some(TtsState::Stop) => break,
                Some(_) => {}
                None => (),
            }
        }
    }
    // silent 3600ms
    for _ in 0..180 {
        session
            .accept_frame(&Frame::Voice {
                data: vec![0u8; 320].as_ref(),
            })
            .await;
        sleep(Duration::from_millis(20)).await;
    }
    loop {
        let data = output.next().await.unwrap().unwrap();
        if let FrameResult::CloseResult = data {
            break;
        }
    }
    session.stop().await;
    Ok(())
}

#[tokio::test]
#[ignore]
/// get text message and output the asr text result
/// cargo test --test session_test -- test_chat_flow_handle_text_message_multiple_time --exact --ignored --nocapture
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
    tear_down(container).await;
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// get text message and output the asr text result
/// cargo test --test session_test -- test_chat_flow_handle_text_message --exact --ignored --nocapture
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
    tear_down(container).await;
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// when a round running and has a break event,the output stream will stop the original output
/// cargo test --test session_test -- test_chat_flow_break --exact --ignored --nocapture
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
    tear_down(container).await;
    Ok(())
}

#[tokio::test]
#[ignore]
/// Shell command:
/// ``` shell
/// cargo test --test session_test -- test_mcp_flow_server_client --exact --ignored --nocapture
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
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .compact()
            .with_max_level(tracing::Level::TRACE)
            .finish(),
    )
    .expect("Failed to set tracing subscriber");
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
    tear_down(container).await;
    Ok(())
}

#[tokio::test]
#[traced_test]
#[ignore]
/// Shell command:
/// ``` shell
/// cargo test -F cuda --test session_test -- test_mcp_flow_device_client --exact --ignored --nocapture
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
    let mut output = session.output_frame().await;
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
    tear_down(container).await;
    Ok(())
}

async fn create_session()
-> Result<(Session, Option<ContainerAsync<Postgres>>, AppState), anyhow::Error> {
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
    let session_id = gen_id();
    let mut mcp_host = UnionMcpHost::new(Some(session_id.clone()));
    mcp_host.add_client(Box::new(server_client)).await;

    let audio_config = Arc::new(AudioConfig {
        input_sample_rate: Some(16000),
        input_frame_duration: Some(20_u64),
        input_channel: Some(1),
        output_sample_rate: Some(16000),
        output_channel: Some(1),
        output_frame_duration: Some(20_u64),
    });
    let session = SessionBuilder::new()
        .with_listener(Box::new(DefaultListener::new(
            Arc::new(Mutex::new(VadFactory::create_model(&Arc::new(VadConfig {
                model: Some(VadModel::Earshot),
                ..Default::default()
            })))),
            Arc::new(Mutex::new(AsrFactory::create_model(&AsrConfig {
                model: Some(AsrModel::Qwen3),
                path: Some(String::from("data/asr/model/Qwen/Qwen3-ASR-0.6B/")),
            }))),
            audio_config.clone(),
        )))
        .with_id(session_id.clone())
        .with_model(
           Arc::new( LlmFactory::create_model(&LlmConfig {
                model: Some(LlmModel::Qwen3),
                path: Some(String::from("data/llm/model/unsloth/Qwen3-1.7B-GGUF/")),
            }))
        )
            .with_tts(Arc::new(TtsFactory::create_model(&TtsConfig {
                model: Some(TtsModel::Mute),
                ..Default::default()
            }, &audio_config).await.unwrap()))
        .with_mcp_host(Arc::new(Mutex::new(mcp_host)))
        .with_config(Arc::new(SessionConfig {
            close_connection_no_voice_time: Some(3000),
            silence_voice_timeout: Some(1200),
            system_prompt: Some(String::from(
                "你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等。",
            )),
            max_prompt_len: Some(6000),
        }))
        .with_audio_config(audio_config.clone())
        .build();
    Ok((session, container, state))
}

async fn create_mini_session() -> Session {
    let audio_config = Arc::new(AudioConfig {
        input_sample_rate: Some(16000),
        input_frame_duration: Some(20_u64),
        input_channel: Some(1),
        output_sample_rate: Some(16000),
        output_channel: Some(1),
        output_frame_duration: Some(20_u64),
    });
    let session_id = gen_id();
    SessionBuilder::new()
        .with_listener(Box::new(DefaultListener::new(
            Arc::new(Mutex::new(VadFactory::create_model(&Arc::new(VadConfig {
                model: Some(VadModel::Earshot),
                ..Default::default()
            })))),
            Arc::new(Mutex::new(AsrFactory::create_model(&AsrConfig {
                model:Some(AsrModel::Void),
                ..Default::default()
            }))),
            audio_config.clone(),
        )))
        .with_id(session_id.clone())
        .with_model(
           Arc::new( LlmFactory::create_model(&LlmConfig {
            model: Some(LlmModel::Echo),
            ..Default::default()
            }))
        )
            .with_tts(Arc::new(TtsFactory::create_model(&TtsConfig {
            model: Some(TtsModel::Mute),
            ..Default::default()
        }, &audio_config).await.unwrap()))
        .with_mcp_host(Arc::new(Mutex::new(UnionMcpHost::new(Some(session_id.clone())))))
        .with_config(Arc::new(SessionConfig {
            close_connection_no_voice_time: Some(3000),
            silence_voice_timeout: Some(1200),
            system_prompt: Some(String::from(
                "你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等。",
            )),
            max_prompt_len: Some(3000),
        }))
        .with_audio_config(audio_config.clone())
        .build()
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
    let mut encoder = opus_rs::OpusEncoder::new(
        ENCODE_SAMPLE_RATE as i32,
        1,
        opus_rs::Application::Audio,
    )
    .unwrap();

    // 16000Hz * 1 channel * 20 ms / 1000 = 320
    const MONO_20MS: usize = ENCODE_SAMPLE_RATE as usize * 20 / 1000;
    let size = MONO_20MS;
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
        let mut packet = vec![0u8; 4000];
        let encoded_len = encoder.encode(&pcm_data[start..end], size, &mut packet).unwrap();
        packet.truncate(encoded_len);
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
