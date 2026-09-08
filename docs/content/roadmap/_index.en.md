+++
title = "Roadmap"
weight = 20
sort_by = "weight"
[extra]
source_file_hash = "b22ba7ac0a54418c66a3489dd783d734c9690b57"
translated_at = "2026-09-08T09:22:19Z"
+++

## Overview

### Socket Session

```txt │
                                            ┌──────────────────────────────────┐     │
                                            │Register│                         │     │
                                            │────────┘                         │     │
                  ┌─────────────────────────│  1.inner root                    │◄────│
                  │                         └──────────────────────────────────┘     │
                  │                                           │                      │
                  │                                           │                      │
                  ▼                                           ▼                      │
  ┌──────────────────────────────────┐      ┌──────────────┐───────────────────┐     │
  │Auth│                             │      │Activate&Bind │                   │     │
  │────┘                             │      └──────────────┘                   │     │
  │  1.OTA(v1,v2)                    │◄─────│  1.OTA(v1,v2)                    │◄────│
  │  2.Nostr                         │      │  2.Nostr                         │     │
  └──────────────────────────────────┘      └──────────────────────────────────┘     │
                  │              │                                                   │
                  │              └────────────────────────────┐                      │
                  │ (Bearer Token)                            │ (JWT)                │
                  ▼                                           ▼                      │
  ┌───────────────────┐──────────────┐       ╔══════════════════════════════════╗    │
  │Protocol Translator│              │       ║Restful │                         ║    │
  └───────────────────┘              │       ║────────┘                         ║────┘
  │  1.XiaoZhi                       │       ║                                  ║
  └──────────────────────────────────┘       ╚══════════════════════════════════╝
                  │
                  │
                  ▼
  ┌────────────┐─────────────────────┐
  │Input Filter│                     │
  └────────────┘                     │
  │  1.Recorder                      │
  │  2.McpRouter                     │───────────────────────────────────────────┐
  └──────────────────────────────────┘                                           │
                  │                                                              │
                  │ Frame                                                        │ Frame
                  ▼                                                              ▼
  ┌───────────────────────────────────────────────────┐       ┌──────────────────────────────────┐
  │ Session │                                         │       │ Mcp Session │                    │
  │─────────┘                                         │       │─────────────┘                    │
  │     ▲                            │                │       │                                  │
  │     │                            │ Pipeline Event │       │                                  │
  │     │ Round Event                │                │       │   ┌───────────────────────────┐  │
  │     │                            ▼                │       │   │Mcp Registry│              │  │
  │   ┌───────────────────────┐──────────────────┐    │       │   │────────────┘              │  │
  │   │ Round(shadow/running) │                  │    │   ┬──────►│  1.Device Mcp Client      │  │
  │   └───────────────────────┘                  │    │   │   │   │  2.External Mcp Client    │  │
  │   │                 ▲            │           │    │   │   │   │                           │  │
  │   │                 │            │ Pipeline  │    │   │   │   │                           │  │
  │   │                 │ Tap Event  │ Event     │    │   │   │   │                           │  │
  │   │                 │ (broadcast)▼           │    │   │   │   │                           │  │
  │   │               ┌──────────┐───────────┐   │    │   │   │   │                           │  │
  │   │               │ Pipeline │           │   │    │   │   │   └───────────────────────────┘  │
  │   │               └──────────┘           │   │    │   │   └──────────────────────────────────┘
  │   │               │ 1. VAD               │   │    │   │       │
  │   │               │ 2. ASR               │   │    │   │       │
  │   │               │ 3. Turn              │   │    │   │       │
  │   │               │ 4. Ling(LLM)    ◄─────────────────┘       │
  │   │               │ 5. TTS               │   │    │           │
  │   │               └──────────────────────┘   │    │           │
  │   │                              │           │    │           │
  │   │                              │ Pipeline  │    │           │
  │   │                              │ Event     │    │           │
  │   │                              ▼           │    │           │
  │   └──────────────────────────────────────────┘    │           │
  │                                  │                │           │
  │                                  │ Pipeline       │           │
  │                                  │ Event          │           │
  │                                  ▼                │           │
  └───────────────────────────────────────────────────┘           │
                  │                                               │
                  │ FrameResult                                   │
                  │                                               │
                  ▼                                               │
  ┌─────────────┐────────────────────┐                            │
  │Output Filter│                    │         FrameResult        │
  └─────────────┘                    │◄───────────────────────────┘
  │  1.Recorder                      │
  └──────────────────────────────────┘
                  │
                  │
                  ▼
  ┌───────────────────┐──────────────┐
  │Protocol Translator│              │
  └───────────────────┘              │
  │  1.XiaoZhi                       │
  └──────────────────────────────────┘
                  │
                  │
                  ▼
```

### Restful

```txt
                                                               ┌────┐──────────────────────────┐
                                                               │Docs│                          │
                                                               └────┘                          │
                                                               │  1.Scalar                     │
                                                               │                               │
                            │                                  └───────────────────────────────┘
                            │ Request
                            │
                            ▼
  ┌──────────────────────────────────────────────────┐         ┌────┐──────────────────────────┐
  │Layer│                                            │         │Task│                          │
  │─────┘                                            │         └────┘                          │
  │                                                  │         │  1.Security cleanup loop      │
  │           ┌────────┐─────────────────────┐       │         │                               │
  │           │CORS    │                     │       │         │                               │
  │           └────────┘                     │       │         └───────────────────────────────┘
  │           │                              │       │
  │           └──────────────────────────────┘       │
  │                         │                        │
  │                         │                        │
  │                         ▼                        │
  │           ┌────────┐─────────────────────┐       │
  │           │Tracing │                     │       │
  │           └────────┘                     │       │
  │           │                              │       │
  │           └──────────────────────────────┘       │
  │                         │                        │
  │                         │                        │
  │                         ▼                        │
  │           ┌───────────┐──────────────────┐       │
  │           │Body Limit │                  │       │
  │           └───────────┘                  │       │
  │           │                              │       │
  │           └──────────────────────────────┘       │
  │                         │                        │
  │                         │                        │
  │                         ▼                        │
  │           ┌────────┐─────────────────────┐       │
  │           │Timeout │                     │       │
  │           └────────┘                     │       │
  │           │                              │       │
  │           └──────────────────────────────┘       │
  │                         │                        │
  │                         │                        │
  │                         ▼                        │
  │           ┌────────┐─────────────────────┐       │
  │           │Security│                     │       │
  │           └────────┘                     │       │
  │           │  1.Access log                │       │
  │           │  2.Rate Limit                │       │
  │           │  3.Auth                      │       │
  │           └──────────────────────────────┘       │
  │                                                  │
  │                                                  │
  │                                                  │
  └──────────────────────────────────────────────────┘
                            │
                            │ Response
                            │
                            ▼

```

### IOT

```txt

                                          ┌───────────────┐───────────────┐
                                          │ Reporter      │               │
                                          └───────────────┘               │
                                          │                               │
        ┌───────────────────┐──┐          │                               │
        │Device Signal Input│  │          │                               │
        └───────────────────┘  │          │                               │
  │────►│  1.Button            │───┐      │                               │
  │     │                      │   │      └───────────────────────────────┘
  │     └──────────────────────┘   │                │        ▲
  │                                │                │ Query  │ Query Response
  │                                │                ▼        │
  │                                │      ┌───────────────┐───────────────┐
  │                                │      │ State Manager │               │
  │     ┌────────────┐─────────┐   │      └───────────────┘               │
  │     │Other Input │         │   │      │                               │
  │     └────────────┘         │   │      │                               │
  │────►│  1.Intent(JSON)      │   │      │                               │
  │     │                      │   │      │                               │
  │     └──────────────────────┘   │      │                               │
  │             ▲  │               │      └───────────────────────────────┘
  │             │  │               │                 │             ▲   │
  │             │  │               │ Signal          │             │   │
  │             │  │               │                 │             │   │
  │             │  │               ▼                 ▼             │   │
  │             │  │              ┌───────────────┐──────┐         │   │
  │             │  │  Intent      │Intent Resolver│      │         │   │
  │             │  └────────────► └───────────────┘      │         │   │
  │             └─────────────────│                      │         │   │
  │                   Ack         │                      │         │   │
  │                               └──────────────────────┘         │   │
  │                                             │                  │   │
  │                                             │ Intent           │   │
  │                                             │                  │   │
  │                   ┌─────────────────────────│──────────────────│───│──┐
  │                   │ Loop Logic│             │                  │   │  │
  │                   │───────────┘             │                  │   │  │
  │                   │                         ▼                  │   │  │
  │                   │           ┌───────────┐───────────────┐    │   │  │
  │                   │           │ Intent Bus│               │    │   │  │
  │                   │           └───────────┘               │    │   │  │
  │                   │           │                           │    │   │  │
  │                   │           │                           │    │   │  │
  │                   │           └───────────────────────────┘    │   │  │
  │                   │                        │                   │   │  │
  │                   │                        │ Intent            │   │  │
  │                   │                        │                   │   │  │
  │                   │                        ▼                   │   │  │
  │                   │           ┌──────────┐────────────────┐    │   │  │
  │                   │           │ Logic    │                │    │   │  │
  │                   │           └──────────┘                │◄───┘   │  │
  │                   │           │                           │        │  │
  │                   │           │                           │        │  │
  │                   │           └───────────────────────────┘        │  │
  │                   │                        │                       │  │
  │                   │                        │                       │  │
  │                   │                        │                       │  │
  │                   │                        ▼                       │  │
  │                   │           ┌──────────┐────────────────┐        │  │
  │                   │           │ Render   │                │        │  │
  │                   │           └──────────┘                │◄───────┘  │
  │                   │           │                           │           │
  │                   │           │                           │           │
  │                   │           └───────────────────────────┘           │
  │                   │               │                                   │
  │                   │               │                                   │
  │                   │               │ Send Signal                       │
  │                   │               │                                   │
  │                   └───────────────│───────────────────────────────────┘
  │                                   │
  │                                   │
  │                                   │
  │                                   │
  │                                   ▼
  │                   ┌───────┐───────────────────────────────────────────┐
  │                   │ Board │                                           │
  │                   └───────┘                                           │
  │                   │                                                   │
  │                   │                                                   │
  │                   │       ┌───────────────────────────────────┐       │
  │                   │       │ Device│                           │       │
  └───────────────────│       │───────┘                           │       │
                      │       │  1. RGBLED                        │       │
                      │       │                                   │       │
                      │       │                                   │       │
                      │       └───────────────────────────────────┘       │
                      │                                                   │
                      └───────────────────────────────────────────────────┘

```

## Status

Column reference:

| Column | Description                 | Possible Values                         |
| ------ | --------------------------- | --------------------------------------- |
| Status | Feature completeness        | ✅ Complete / ⚠️ Defective (stub/incomplete) / ❌ Not implemented |
| Test   | Automated tests exist       | ✅ Yes / ❌ No / — N/A                 |
| Item   | Feature name                | —                                       |
| Desc   | Brief description           | —                                       |
| Link   | File path or Issue link     | (TBD)                                   |

### Register

| Status | Test | Item       | Description                              | Link |
| ------ | ---- | ---------- | ---------------------------------------- | ---- |
| ❌     | ❌   | Inner root | Migration only, no runtime register endpoint |  |

### Auth

#### OTA Authentication

| Status | Test | Item   | Description                                               | Link |
| ------ | ---- | ------ | --------------------------------------------------------- | ---- |
| ✅     | ✅   | OTA v1 | Basic device auth, returns ws_url+token (real JWT for activated, empty otherwise) |  |
| ❌     | ❌   | Firmware distribution | OTA returns version: "0.0.1" + url: null, no actual firmware download link |  |
| ❌     | ❌   | MQTT support | OTA returns mqtt: null, MQTT protocol not implemented |  |
| ❌     | ❌   | OTA v2 | Full auth + device info report + firmware distribution (returns static data) |  |

#### Nostr Authentication

| Status | Test | Item         | Description | Link |
| ------ | ---- | ------------ | ----------- | ---- |
| ❌     | ❌   | NIP-98 auth  | Not implemented |  |

#### WS Authentication

| Status | Test | Item               | Description                       | Link |
| ------ | ---- | ------------------ | --------------------------------- | ---- |
| ✅     | ✅   | Bearer Token verify | Authorization header + query param fallback, JWT decode verification |  |

### Activate & Bind

#### OTA Activation

| Status | Test | Item   | Description                               | Link |
| ------ | ---- | ------ | ----------------------------------------- | ---- |
| ✅     | ✅   | OTA v1 | Activation code verification + device info persisted |      |
| ❌     | ❌   | OTA v2 | Full activation flow (with challenge verification), not implemented |  |

#### Nostr Binding

| Status | Test | Item            | Description | Link |
| ------ | ---- | --------------- | ----------- | ---- |
| ❌     | ❌   | Bind / Role registration | Not implemented |  |

### Protocol Translator

#### XiaoZhi

| Status | Test | Item            | Description                                  | Link |
| ------ | ---- | --------------- | -------------------------------------------- | ---- |
| ✅     | ❌   | Input frame parsing | hello/listen/abort/mcp/voice fully parsed  |      |
| ✅     | ❌   | Output frame serialization | STT/LLM/TTS/Audio/Error fully serialized |  |
| ❌     | ❌   | Missing message types | system/alert/wake_word not implemented    |  |

### Input Filter

#### Recorder (Input)

| Status | Test | Item        | Description                | Link |
| ------ | ---- | ----------- | -------------------------- | ---- |
| ✅     | ✅   | Input persistence | Frame data persisted to database |  |

#### McpRouter

| Status | Test | Item          | Description                          | Link |
| ------ | ---- | ------------- | ------------------------------------ | ---- |
| ✅     | ✅   | MCP frame routing | Intercepts Frame::Mcp → McpSession |  |

### Session

#### Listener

| Status | Test | Item                    | Description                                      | Link |
| ------ | ---- | ----------------------- | ------------------------------------------------ | ---- |
| ✅     | ✅   | VAD (Silero)            | Earshot implementation, frame-level voice detection |  |
| ✅     | ✅   | ASR (sherpa-onnx)       | XAsr model, 16kHz mono                          |      |
| ✅     | ✅   | Voice Break detection   | Silence timeout triggers TurnComplete            |      |
| ✅     | ✅   | Continued Conversation  | auto/realtime mode, auto-listens after TTS       |      |
| ❌     | ❌   | Semantic VAD / Hierarchical turns | Layer1 Silero only, missing punctuation/semantic layers |  |

#### Round

| Status | Test | Item                   | Description                                  | Link |
| ------ | ---- | ---------------------- | -------------------------------------------- | ---- |
| ✅     | ✅   | Shadow / Running rounds | Pre-creates shadow round for lower switch latency |  |
| ✅     | ✅   | Barge-in + lockout      | Configurable lockout window (default 250ms)  |      |
| ✅     | ✅   | Epoch message filtering | Discards stale rounds, preserves TTS state notifications |  |
| ✅     | —    | stop_round race condition | Fixed, 5s timeout fallback                 |      |
| ✅     | ✅   | AudioPacer              | Rate-limits audio output by output_frame_duration |  |

#### LingCore

| Status | Test | Item                  | Description                                   | Link |
| ------ | ---- | --------------------- | --------------------------------------------- | ---- |
| ✅     | ✅   | Qwen3 LLM (candle)    | Streaming generation, thread-safety fixed     |      |
| ✅     | ✅   | Splitter (sentencex)  | Chinese/English sentence segmentation, 700ms timeout fallback |  |
| ✅     | ✅   | ToolCall loop         | LLM → MCP tool → result injection → LLM regeneration |  |
| ✅     | ✅   | History management    | Context truncation (max_prompt_len) and persistence |  |
| ⚠️     | ❌   | Emotion recognition   | Always returns "happy", no real analysis      |      |
| ❌     | ❌   | Personalized memory   | Long-term preferences/user profiles not implemented |  |

#### TTS

| Status | Test | Item                | Description                              | Link |
| ------ | ---- | ------------------- | ---------------------------------------- | ---- |
| ✅     | ✅   | MatchaTTS + Opus    | sherpa-onnx engine, Opus encoding output |      |
| ✅     | ✅   | Two-stage streaming | Token-level first packet fast + sentence-level steady |  |
| ✅     | ❌   | TTFA/RTF metrics    | First-packet delay / real-time factor monitoring |  |
| ✅     | ❌   | Fade-out            | Tail fade-out to eliminate clicks        |      |

### McpSession

#### McpRegistry

| Status | Test | Item              | Description                              | Link |
| ------ | ---- | ----------------- | ---------------------------------------- | ---- |
| ✅     | ✅   | Tool registration/aggregation | Multi-client tool merging, name-routed invocation |  |

#### Device Mcp Client

| Status | Test | Item         | Description                        | Link |
| ------ | ---- | ------------ | ---------------------------------- | ---- |
| ✅     | ✅   | rmcp transport | Device MCP protocol bridge, full handshake |  |

#### External Mcp Client

| Status | Test | Item           | Description                              | Link |
| ------ | ---- | -------------- | ---------------------------------------- | ---- |
| ✅     | ✅   | HTTP remote MCP | Connects to external MCP servers, tool list cached |  |

#### MCP Authentication

| Status | Test | Item           | Description                | Link |
| ------ | ---- | -------------- | -------------------------- | ---- |
| ✅     | ✅   | /mcp endpoint auth | Bearer JWT auth + rmcp default Host validation |  |

### Output Filter

#### Recorder (Output)

| Status | Test | Item          | Description                       | Link |
| ------ | ---- | ------------- | --------------------------------- | ---- |
| ✅     | ✅   | Output persistence | Frame/round data persisted to database |  |

### Restful

| Status | Test | Item                           | Description                                    | Link |
| ------ | ---- | ------------------------------ | ---------------------------------------------- | ---- |
| ✅     | ✅   | POST /api/auth/login           | Account password auth, returns access/refresh token |  |
| ✅     | ✅   | POST /api/auth/access_token    | Refresh token exchange                        |      |
| ✅     | ✅   | POST /api/auth/reset_password  | JWT auth, old password verification            |      |
| ✅     | ✅   | GET /api/auth/user             | Returns current user info                      |      |
| ✅     | ❌   | GET /api/stats/summary        | Overview stats (devices/sessions/messages/latency avg) |  |
| ✅     | ❌   | GET /api/stats/trends         | Trend data (daily device/session/message trends) |  |
| ✅     | ❌   | GET /api/stats/latency        | Latency percentile data (P50/P90/P99 etc.)     |      |
| ✅     | ✅   | Rate Limiting                  | GitHub-style buckets: auth/ota per-IP fixed window (20/15min, 30/min) + core per-user (1000/h) + X-RateLimit-* response headers + per-account login failure lockout (5/15min) |  |
| ✅     | ✅   | GET /api/security/rate_limit   | Authenticated introspection endpoint returning auth/ota/core quotas and live usage; does not consume quota |  |
| ❌     | ❌   | Invite Code system             | Registration invite code generation/verification not implemented |  |
| ✅     | ❌   | Device management CRUD         | List/activate (by code/by ID)/disable/enable/delete endpoints implemented (no detail) |  |

### Admin UI

| Status | Test | Item               | Description                                          | Link |
| ------ | ---- | ------------------ | ---------------------------------------------------- | ---- |
| ✅     | ❌   | Dashboard page     | 6 StatCards, TrendsChart, LatencyChart, RecentSessionsTable, LatencyTable |      |
| ✅     | ✅   | Session list/details | Search/date filter/pagination/SessionDetail component |  |
| ✅     | ✅   | Security event monitoring | Rate-limit/login events persisted + admin page query (type/IP filter + pagination) + 30-day auto cleanup |  |
| ❌     | ❌   | User CRUD UI       | User list/create/delete/role management not implemented |  |
| ❌     | ❌   | RBAC management    | Token usage/conversation duration/device activity/permissions not implemented |  |
| ❌     | ❌   | MCP dashboard      | Multi-endpoint management/tool sync/access control/logs not implemented |  |
| ❌     | ❌   | System monitoring  | Server health/connections/resource usage/error rates not implemented |  |

### Flutter App

| Status | Test | Item                | Description                                             | Link |
| ------ | ---- | ------------------- | ------------------------------------------------------- | ---- |
| ❌     | ❌   | Shared client protocol | device identity (isomorphic to IoT), WS voice + telemetry + MCP tool surface | [Spec](../../development/clients/shared-protocol.en.md) |
| ❌     | ❌   | USB peripheral telemetry | Android first (OTG/USB-Host), iOS voice + screen only |      |
| ❌     | ❌   | CI/CD               | iOS/Android build/signing/publishing pipeline not implemented |  |

### IoT

#### Current (P0)

| Status | Test | Item                | Description                                             | Link |
| ------ | ---- | ------------------- | ------------------------------------------------------- | ---- |
| ❌     | ❌   | apps/iot scaffold   | Independent Cargo workspace (four layers: core/chip/bsp/app); board-agnostic core + esp runtime chip + pure-board bsp + single-task app; esp32c6 no_std (riscv32imac target, no espup/ESP-IDF required) |      |
| ❌     | ❌   | core peripheral/capability modules | base (network/config/HTTP/OTA) + optional peripheral capability traits combined arbitrarily; layering core → chip → bsp → app, bsp selects board via feature (esp-bsp-rs pattern), modules assembled via iot-core capability interfaces |  |
| ❌     | ❌   | LED app             | First in-app peripheral sub-feature (feature-gated `iot-core` Led capability + bsp WS2812 impl); ESP32-C6-DevKitC-1 onboard WS2812 RGB LED on GPIO8, driven via esp-hal-smartled + RMT (Blocking); async breathing via esp-rtos (Embassy runtime) (`iot-app::breath`) |  |
| ❌     | ❌   | Button app          | Layered input architecture: `ButtonScanner` (10ms scan + N=3 consecutive-sample debounce, produces `InputEvent{Click,LongPress}`) → `InputMap` (stateless pure fn `resolve(InputEvent + &LedState) -> Option<Intent>`, lookup current value in PALETTE/BREATH_PERIODS_MS tables, wrap-around advance) → `Intent::Led(LedState)` (absolute target state, subsystem-categorized, server commands use same bus homogeneously) → `Channel<'static, 8>` queue (button producer `try_send`, drop-on-full so debounce timing is never broken) → `DeviceManager.apply(LedState)` → render; current LedState shared via `embassy-sync::Watch<T, 1>` snapshot (single writer = consumer); ESP32-C6-DevKitC-1 BootButton on GPIO9 (pull-up, low = pressed) |  |
| ❌     | ❌   | Build/flash tasks   | moon check/test/build/flash, artifacts .elf/.bin; espflash in flake.nix devShell (riscv32imac target runner) |      |
| ❌     | ❌   | CI integration      | moon ci runs automatically, compiles firmware binaries and uploads artifacts |      |
| ❌     | ❌   | CD release          | iot release workflow (tag `vanling-iot@x.y.z`), firmware artifacts published to GitHub Release |      |

#### Near-term (P1)

| Status | Test | Item                | Description                                             | Link |
| ------ | ---- | ------------------- | ------------------------------------------------------- | ---- |
| ❌     | ❌   | Device-side config  | softAP config page + peripheral enable/disable + persistence |  |
| ❌     | ❌   | Telemetry path ① (persist) | HTTP short-transaction + device_key, stored to server state store |  |
| ❌     | ❌   | Screen display      | Page-based rendering peripheral (Display indicator; chat voice-only, protocol-driven; home/config/ota device-local pages, available to any device; LED/Audio/Web indicators pluggable) | [Spec](../../development/clients/shared-protocol.en.md) |
| ❌     | ❌   | Server capability negotiation | hello capability set → dynamic pipeline assembly + in-session hot reload |  |
| ❌     | ❌   | Telemetry path ② (session read) | MCP home_state tool queries store + device MCP real-time direct read |  |
| ❌     | ❌   | Server auth tightening | WS accepts device-identity JWT only (token_type=device), telemetry owned by device dimension |  |
| ❌     | ❌   | server IoT API      | telemetry ingestion + device management                |      |
| ❌     | ❌   | Business composition model | Three orthogonal axes (board feature ⊥ module feature ⊥ capability trait); modules see capabilities only, boards provide them; composition point = bin board manifest (capability in injected by move, missing capability = compile error, not silent); product profile = feature alias (profile picks module set, board picks support set, intersection validated at compile time) |  |
| ❌     | ❌   | Multi-chip portability base | app business is chip-free already; esp family runtime lives in `iot-chip-esp`, esp-hal/esp-rtos feature-gated in app per family; change/add chip = per-family crates `iot-chip-<family>`/`iot-bsp-<family>` (esp already `-esp`) + swap cfg'd family entry macro in main + toolchain/CI matrix (esp32s3 needs xtensa target) |  |
| ❌     | ❌   | First non-esp family migration | esp family crates are already `iot-chip-esp`/`iot-bsp-esp`; add `-stm32` variants → app family alias (`stm32f4 = ["iot-chip-stm32", "dep:embassy-stm32", …]`) → add `#[cfg(feature)]` family entry block in main (`#[embassy_executor::main]`) → toolchain thumbv7em + `rust-toolchain.toml`/CI matrix rows → scope `moon`/`lefthook` `--workspace --target` per firmware → special boards may write their own `boards/` manifest |  |

#### Long-term (P2)

| Status | Test | Item                | Description                                             | Link |
| ------ | ---- | ------------------- | ------------------------------------------------------- | ---- |
| ❌     | ❌   | Voice assistant peripheral | Opus codec + WS voice client + wake word           |      |
| ❌     | ❌   | MQTT extended telemetry | rumqttd broker + QoS/LWT online status + transport negotiation (prefer MQTT, fallback HTTP) |  |
| ❌     | ❌   | OTA firmware update | version check/download verify/dual-partition rollback   |      |
| ❌     | ❌   | SNTP time sync      | Trustworthy data timestamps                             |      |
| ❌     | ❌   | Reconnect/buffer    | Backfill + message buffering                            |      |

### Infrastructure

| Status | Test | Item                 | Description                                                         | Link |
| ------ | ---- | -------------------- | ------------------------------------------------------------------- | ---- |
| ✅     | ✅   | email UNIQUE constraint | user.email has UNIQUE constraint in migration (nullable, compatible with existing empty-email records) |      |
| ✅     | ✅   | Graceful shutdown order | Server CancellationToken drives axum/WS/Matrix graceful shutdown in order |      |
| ✅     | —    | Runtime race condition  | build() runs synchronously, set() happens-before any worker thread, no race |      |
| ✅     | ✅   | Timestamp auto-fill     | Config entity implements before_save, create/update timestamps auto-filled |      |

