+++
title = "Core Architecture"
weight = 200
[extra]
source_hash = "0000000000000000000000000000000000000000"
translated_at = "2026-06-28T18:00:00Z"
+++

# Core Architecture

## Session Lifecycle

### State Machine (Phase)

The Session manages the connection lifecycle through the `Phase` enum:

```mermaid
stateDiagram-v2
    [*] --> Hello: WebSocket connection established
    Hello --> ListenDetect: Hello handshake complete
    ListenDetect --> ListenAuto: Client Listen(Auto)
    ListenDetect --> ListenManual: Client Listen(Manual)
    ListenDetect --> ListenRealtime: Client Listen(Realtime)
    ListenAuto --> WaitRound: VAD detects end of speech
    ListenManual --> WaitRound: Client Listen(Stop)
    ListenRealtime --> WaitRound: Each VAD detection triggers processing
    WaitRound --> ListenDetect: Round complete
    WaitRound --> ListenAuto: Round complete & continue Auto mode
    WaitRound --> ListenRealtime: Round complete & continue Realtime mode
    Hello --> Stop: Connection close/Abort
    ListenDetect --> Stop: Connection close/Abort
    ListenAuto --> Stop: Connection close/Abort
    ListenManual --> Stop: Connection close/Abort
    ListenRealtime --> Stop: Connection close/Abort
    WaitRound --> Stop: Connection close/Abort
    Stop --> [*]: Resource cleanup
```

### Three Listen Modes

| Mode | Trigger | End Condition | Use Case |
|------|---------|---------------|----------|
| Auto | Client sends `Listen(Auto)` | VAD detects silence timeout | Voice-activated auto interaction |
| Manual | Client sends `Listen(Manual)` | Client sends `Listen(Stop)` | Push-to-talk |
| Realtime | Client sends `Listen(Realtime)` | Processed on each VAD detection | Real-time transcription |

### Core Structs

```
SessionBuilder (all dependencies injected at build)
  └── Session
       ├── id: String (XID)
       ├── phase: Phase (state machine)
       ├── output_epoch: AtomicU64 (Round incrementing counter)
       ├── cancel: CancellationToken (global cancel)
       ├── round: Option<Round> (current active Round)
       ├── listener: Box<dyn Listener> (VAD + ASR + audio buffer)
       ├── output_controller: OutputController (outbound flow control)
       └── observers: Vec<Arc<dyn SessionObserver>> (persistence callbacks)
```

## Concurrency Model

```mermaid
flowchart TB
    subgraph WebSocket Connection
        WS[axum WebSocket]
    end

    subgraph "Session::start() tasks"
        RECV[on_recv task]
        SEND[on_send task]
    end

    subgraph "OutputController task"
        OC[epoch filtering + audio pacing]
    end

    subgraph "Round::accept_command task"
        R[llm_tts_handle]
        subgraph "LLM thread::spawn"
            LLM[model.stream]
        end
        subgraph "TTS spawn_blocking"
            TTS[matcha generate]
        end
    end

    WS -->|inbound frames| RECV
    RECV -->|accept_frame| R
    R -->|OutputMessage| OC
    OC -->|paced messages| SEND
    SEND -->|outbound frames| WS
```

Key design decisions:
- `on_recv` and `on_send` use `tokio::spawn` (IO-intensive)
- LLM inference uses `thread::spawn` + `block_on` (CPU-intensive, does not block tokio runtime)
- TTS generation uses `tokio::task::spawn_blocking` (ONNX inference blocking)
- Rounds are isolated via `output_epoch`, old Round messages are automatically discarded
- OutputController is the sole throttling point, using `bounded(64)` channel for backpressure

## Factory Pattern

All AI components are managed via OnceLock global Factories:

```mermaid
flowchart LR
    VC[VadConfig] --> VF[VadFactory]
    VF -->|create_model| V[Box<dyn Vad>]
    AC[AsrConfig] --> AF[AsrFactory]
    AF -->|create_model| A[Arc<Mutex<Box<dyn Asr>>>]
    TC[TtsConfig] --> TF[TtsFactory]
    TF -->|create_model| T[Arc<Box<dyn Tts>>]
    LC[LlmConfig] --> LF[LlmFactory]
    LF -->|create_model| L[Arc<Box<dyn Model>>]

    VF -->|global & config| DefaultListener
    AF -->|global| DefaultListener
    TF -->|global| SessionBuilder
    LF -->|global| SessionBuilder
```

Initialization order (in `api::start`):
1. `Jwt::init(auth_config)` — JWT
2. Database connection + migrations
3. `TtsFactory::init(tts_config, audio_config)`
4. `VadFactory::init(vad_config)`
5. `AsrFactory::init(asr_config)`
6. `LlmFactory::init(llm_config)`
7. HTTP server start (+ optional Matrix client)
