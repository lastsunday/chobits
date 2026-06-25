use api::ws::frame::{Frame, FrameResult};
use service::chobits::message::{
    audio::AudioMessage,
    hello::HelloMessage,
    listen::{ListenMessage, ListenMode, ListenState},
    tts::{TtsMessage, TtsState},
};
use std::time::Duration;
use tokio::time::sleep;
use tokio_stream::StreamExt;
use tracing::{debug, info};
use tracing_test::traced_test;

use crate::common::tear_down;
use crate::session::helpers::{create_mini_session, create_session, get_audio};

#[tokio::test]
#[traced_test]
async fn test_chat_flow_hello() -> anyhow::Result<()> {
    let mut session = create_mini_session().await;
    session.start().await?;
    let (mut output, _, _, _, _) = session.output_frame().await;
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::HelloResult(..)
    ));
    session.stop().await;
    Ok(())
}

#[tokio::test]
#[traced_test]
/*
2026-03-16T09:26:06.988023Z DEBUG frame: [RECV] Hello(HelloMessage { message: Message { mtype: Hello }, version: None, transport: None, audio_params: None, features: Some(Feature { mcp: Some(true), aec: None }), session_id: None })
2026-03-16T09:26:06.988091Z DEBUG frame: [SEND] HelloResult(HelloMessage { message: Message { mtype: Hello }, version: None, transport: Some(Websocket), audio_params: Some(AudioParam { format: Opus, sample_rate: 16000, channels: 1, frame_duration: 60 }), features: None, session_id: Some("d6rspbklm6jn11rmp49g") })
2026-03-16T09:26:06.988133Z DEBUG frame: [SEND] McpResult(McpRequest { message: Message { mtype: Mcp }, session_id: Some("d6rspbklm6jn11rmp49g"), payload: JsonRpcRequest { jsonrpc: JsonRpcVersion2_0, id: Number(0), request: Request { method: "initialize", params: {"capabilities": Object {}, "clientInfo": Object {"name": String("rmcp"), "version": String("0.15.0")}, "protocolVersion": String("2025-06-18")}, extensions: Extensions } } })
2026-03-16T09:26:07.037845Z DEBUG frame: [RECV] Mcp(McpMessage { message: Message { mtype: Mcp }, payload: Response(JsonRpcResponse { jsonrpc: JsonRpcVersion2_0, id: Number(0), result: {"capabilities": Object {"tools": Object {}}, "protocolVersion": String("2025-06-18"), "serverInfo": Object {"name": String("Web测试设备"), "version": String("1.0.0")}} }) })
2026-03-16T09:26:07.037933Z DEBUG frame: [SEND] McpResult(McpRequest { message: Message { mtype: Mcp }, session_id: Some("d6rspbklm6jn11rmp49g"), payload: JsonRpcRequest { jsonrpc: JsonRpcVersion2_0, id: Number(1), request: Request { method: "tools/list", params: {}, extensions: Extensions } } })
2026-03-16T09:26:07.045113Z DEBUG frame: [RECV] Mcp(McpMessage { message: Message { mtype: Mcp }, payload: Response(JsonRpcResponse { jsonrpc: JsonRpcVersion2_0, id: Number(1), result: {"tools": Array [Object {"description": String("Provides the real-time information of the device, including the current status of the audio speaker, screen, battery, network, etc.\nUse this tool for: \n1. Answering questions about current condition (e.g. what is the current volume of the audio speaker?)\n2. As the first step to control the device (e.g. turn up / down the volume of the audio speaker, etc.)"), "inputSchema": Object {"properties": Object {}, "type": String("object")}, "name": String("self.get_device_status")}, Object {"description": String("Set the volume of the audio speaker. If the current volume is unknown, you must call `self.get_device_status` tool first and then call this tool."), "inputSchema": Object {"properties": Object {"volume": Object {"maximum": Number(100), "minimum": Number(0), "type": String("integer")}}, "required": Array [String("volume")], "type": String("object")}, "name": String("self.audio_speaker.set_volume")}, Object {"description": String("Set the brightness of the screen."), "inputSchema": Object {"properties": Object {"brightness": Object {"maximum": Number(100), "minimum": Number(0), "type": String("integer")}}, "required": Array [String("brightness")], "type": String("object")}, "name": String("self.screen.set_brightness")}]} }) })
*/
async fn test_chat_flow_listen_manual() -> anyhow::Result<()> {
    let audio = get_audio();
    let mut session = create_mini_session().await;
    session.start().await?;
    let (mut output, _, _, _, _) = session.output_frame().await;
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
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
        let data = output.next().await.unwrap().payload.unwrap();
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

// TODO: timed out (>60s) - hangs waiting for TTS pipeline output (MatchaTts channel back-pressure)
#[tokio::test]
#[traced_test]
async fn test_chat_flow_listen_auto() -> anyhow::Result<()> {
    info!("step: get_audio");
    let audio = get_audio();
    info!("step: create_session");
    let (mut session, container, state) = create_session().await?;
    info!("step: start");
    session.start().await?;
    info!("step: output_frame");
    let (mut output, _, _, _, _) = session.output_frame().await;
    info!("step: output_frame done");

    // Hello
    info!("step: hello");
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::HelloResult(..)
    ));
    info!("step: hello done");

    // Listen(Start, Auto) → Wake round
    info!("step: listen start auto");
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Start,
            mmod: Some(ListenMode::Auto),
            ..Default::default()
        }))
        .await;
    info!("step: listen start auto done");

    // First round: STTResult → TTSResult(Start) → LLMResult → SentenceStart → Audio* → SentenceEnd → Stop
    info!("step: wait stt");
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::STTResult(..)
    ));
    info!("step: stt done");

    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::Start),
            ..
        })
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::LLMResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::SentenceStart),
            ..
        })
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::AudioResult(..)
    ));
    info!("step: wait sentence end");
    loop {
        let msg = output.next().await.unwrap().payload.unwrap();
        match msg {
            FrameResult::TTSResult(TtsMessage {
                state: Some(TtsState::SentenceEnd),
                ..
            }) => break,
            FrameResult::AudioResult(..) => continue,
            _ => panic!("unexpected frame: {:?}", msg),
        }
    }
    info!("step: sentence end done");
    info!("step: wait stop");
    loop {
        let msg = output.next().await.unwrap().payload.unwrap();
        if let FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::Stop),
            ..
        }) = msg
        {
            break;
        }
    }
    info!("step: first round done");

    // Send audio to trigger second round
    for packet in &audio {
        session.accept_frame(&Frame::Voice { data: packet }).await;
    }
    // Brief silence with sleeps to advance wall clock past VAD silence timeout (1200ms)
    for _ in 0..80 {
        session
            .accept_frame(&Frame::Voice { data: &[0u8; 320] })
            .await;
        sleep(Duration::from_millis(20)).await;
    }

    // Second round: same message sequence
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::STTResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::Start),
            ..
        })
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::LLMResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::SentenceStart),
            ..
        })
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::AudioResult(..)
    ));
    loop {
        let msg = output.next().await.unwrap().payload.unwrap();
        match msg {
            FrameResult::TTSResult(TtsMessage {
                state: Some(TtsState::SentenceEnd),
                ..
            }) => break,
            FrameResult::AudioResult(..) => continue,
            _ => panic!("unexpected frame: {:?}", msg),
        }
    }
    loop {
        let msg = output.next().await.unwrap().payload.unwrap();
        if let FrameResult::TTSResult(TtsMessage {
            state: Some(TtsState::Stop),
            ..
        }) = msg
        {
            break;
        }
    }

    session.stop().await;
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::CloseResult
    ));
    let _ = &state.conn.close().await?;
    tear_down(container).await;
    Ok(())
}

#[tokio::test]
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
2026-03-16T07:51:53.799160Z DEBUG frame: [RECV] Mcp(McpMessage { message: Message { mtype: Mcp }, payload: Response(JsonRpcResponse { jsonrpc: JsonRpcVersion2_0, id: Number(1), result: {"tools": Array [Object {"description": String("Provides the real-time information of the device, including the current status of the audio speaker, screen, battery, network, etc.\nUse this tool for: \n1. Answering questions about current condition (e.g. what is the current volume of the audio speaker?)\n2. As the first step to control the device (e.g. turn up / down the volume of the audio speaker, etc.)"), "inputSchema": Object {"properties": Object {}, "type": String("object")}, "name": String("self.get_device_status")}, Object {"description": String("Set the volume of the audio speaker. If the current volume is unknown, you must call `self.get_device_status` tool first and then call this tool."), "inputSchema": Object {"properties": Object {"volume": Object {"maximum": Number(100), "minimum": Number(0), "type": String("integer")}}, "required": Array [String("volume")], "type": String("object")}, "name": String("self.audio_speaker.set_volume")}, Object {"description": String("Set the brightness of the screen."), "inputSchema": Object {"properties": Object {"brightness": Object {"maximum": Number(100), "minimum": Number(0), "type": String("integer")}}, "required": Array [String("brightness")], "type": String("object")}, "name": String("self.screen.set_brightness")}, Object {"description": String("Set the theme of the screen. The theme can be `light` or `dark`."), "inputSchema": Object {"properties": Object {"theme": Object {"type": String("string")}}, "required": Array [String("theme")], "type": String("object")}, "name": String("self.screen.set_theme")}, Object {"description": String("Always remember you have a camera. If the user asks you to see something, use this tool to take a photo and then explain it.\nArgs:\n  `question`: The question that you want to ask about the photo.\nR... (line truncated to 2000 chars)
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
// TODO: 1) race condition — Wake pipeline output drained before Listen(Start, RealTime), assertions
//        after Listen(Start) never receive STTResult/TTSResult (already consumed by drain).
//        2) second round uses create_session (MatchaTts) → channel back-pressure like listen_auto.
// TODO: also timed out (>60s) - hangs in drain loop or after
async fn test_chat_flow_listen_realtime() -> anyhow::Result<()> {
    let _ = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .compact()
            .with_max_level(tracing::Level::TRACE)
            .finish(),
    );
    let audio = get_audio();

    let (mut session, container, state) = create_session().await?;
    // let session_id = session.id.clone();
    session.start().await?;
    let (mut output, _, _, _, _) = session.output_frame().await;
    info!("send hello");
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
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
    // drain: wait for Wake pipeline to complete
    while let Some(data) = output.next().await {
        let data = data.payload.unwrap();
        match data {
            FrameResult::TTSResult(tts_message) => {
                if let Some(TtsState::Stop) = tts_message.state {
                    break;
                }
            }
            _ => continue,
        }
    }
    session
        .accept_frame(&Frame::Listen(ListenMessage {
            state: ListenState::Start,
            mmod: Some(ListenMode::RealTime),
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::STTResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::TTSResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::LLMResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::TTSResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::AudioResult(..)
    ));

    let mut frame_result = output.next().await.unwrap().payload.unwrap();
    while let Some(data) = output.next().await {
        let data = data.payload.unwrap();
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
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::LLMResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::TTSResult(..)
    ));
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::AudioResult(..)
    ));

    let mut frame_result = output.next().await.unwrap().payload.unwrap();
    while let Some(data) = output.next().await {
        let data = data.payload.unwrap();
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
        output.next().await.unwrap().payload.unwrap(),
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
        let data = data.payload.unwrap();
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
        output.next().await.unwrap().payload.unwrap(),
        FrameResult::CloseResult
    ));

    session.stop().await;
    let _ = &state.conn.close().await?;
    tear_down(container).await;
    Ok(())
}

#[tokio::test]
async fn test_chat_flow_listen_realtime_silent_voice_connection_timeout() -> anyhow::Result<()> {
    let _ = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .compact()
            .with_max_level(tracing::Level::DEBUG)
            .finish(),
    );
    let mut session = create_mini_session().await;
    session.start().await?;
    let (mut output, _, _, _, _) = session.output_frame().await;
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
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
    // drain Wake pipeline before Listen(Start, RealTime) to avoid epoch bump
    // discarding the Wake pipeline's STTResult
    while let Some(data) = output.next().await {
        let data = data.payload.unwrap();
        match data {
            FrameResult::TTSResult(tts_message) => {
                if let Some(TtsState::Stop) = tts_message.state {
                    break;
                }
            }
            _ => continue,
        }
    }
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
        let data = output.next().await.unwrap().payload.unwrap();
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
        let data = output.next().await.unwrap().payload.unwrap();
        if let FrameResult::CloseResult = data {
            break;
        }
    }
    session.stop().await;
    Ok(())
}

#[tokio::test]
async fn test_chat_flow_handle_text_message_multiple_time() -> anyhow::Result<()> {
    let _ = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .compact()
            .with_max_level(tracing::Level::TRACE)
            .finish(),
    );
    let (mut session, container, state) = create_session().await?;
    session.start().await?;
    let (mut output, _, _, _, _) = session.output_frame().await;
    session
        .accept_frame(&Frame::Hello(HelloMessage {
            ..Default::default()
        }))
        .await;
    assert!(matches!(
        output.next().await.unwrap().payload.unwrap(),
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
        let frame_result = output.next().await.unwrap().payload.unwrap();
        debug!("{:?}", &frame_result);
        assert!(matches!(frame_result, FrameResult::STTResult(..)));

        assert!(matches!(
            output.next().await.unwrap().payload.unwrap(),
            FrameResult::TTSResult(TtsMessage {
                state: Some(TtsState::Start),
                ..
            })
        ));

        let frame_result = output.next().await.unwrap().payload.unwrap();
        debug!("{:?}", &frame_result);
        assert!(matches!(frame_result, FrameResult::LLMResult(..)));

        let frame_result = output.next().await.unwrap().payload.unwrap();
        debug!("{:?}", frame_result);
        assert!(matches!(
            frame_result,
            FrameResult::TTSResult(TtsMessage {
                state: Some(TtsState::SentenceStart),
                ..
            })
        ));
        // has some audio result,detect first one
        let frame_result = output.next().await.unwrap().payload.unwrap();
        debug!("{:?}", frame_result);
        assert!(matches!(
            frame_result,
            FrameResult::AudioResult(AudioMessage { .. })
        ));

        while let Some(data) = output.next().await {
            match data.payload {
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
async fn test_chat_flow_handle_text_message() -> anyhow::Result<()> {
    let (mut session, container, state) = create_session().await?;
    let session_id = session.id.clone();
    session.start().await?;
    let (mut output, _, _, _, _) = session.output_frame().await;
    // TODO: need refactor,remove tokio::spawn
    let join_handle = tokio::spawn(async move {
        while let Some(data) = output.next().await {
            debug!("session id = {}, data = {:?}", session_id, data.payload);
            match data.payload {
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

// TODO: timed out (>60s) - hangs waiting for TTS pipeline output (MatchaTts channel back-pressure)
#[tokio::test]
#[traced_test]
async fn test_chat_flow_break() -> anyhow::Result<()> {
    let (mut session, container, state) = create_session().await?;
    let session_id = session.id.clone();
    session.start().await?;
    let (mut output, _, _, _, _) = session.output_frame().await;
    let mut count = 0;
    // TODO: need refactor,remove tokio::spawn
    let join_handle = tokio::spawn(async move {
        while let Some(data) = output.next().await {
            debug!("session id = {}, data = {:?}", session_id, data.payload);
            match data.payload {
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
