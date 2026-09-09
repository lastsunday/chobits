+++
title = "TODO"
weight = 204

[extra]
translated_at = "2026-09-09T03:46:58Z"
source_file_hash = "92d7c27b304cdcd717183534c688d616816b3cbd"
+++

# TODO

> Project evolution entry point. Organized by feature module; the status column (`✅Completed` / `❌Discarded` / empty=in progress) shows each module's progression (completed → in progress → discarded decisions). Test column `✅` = has corresponding tests.
>
> Feature exploration, technology selection and reference projects live in [Positioning and Trade-off Reference](@/development/server/research.en.md). Actual development priorities follow the project Roadmap. Before fixing, read [AGENTS.md](https://github.com/anomalyco/vanling/blob/main/AGENTS.md) for development conventions.

## Security & Authentication

| Priority | Item | Location | Description | Open Source / Libraries | Status | Test |
|------|------|------|------|------|------|------|
| 🔴 P0 | WS Authentication | `api/src/ws/mod.rs` | WS handler verifies Bearer Token (JWT) inline at upgrade. Authorization header + query param browser fallback | Existing: `axum` | ✅Completed 2026-08-26 | ✅ |
| 🔴 P0 | Invite Code System | New feature | No invite code generation/verification/admin; no control over new user registration | Suggest: `nanoid` — short URL-safe ID generator | | |
| 🟠 P1 | Rate Limiting | `api/src/auth.rs` | No login rate limiting/brute-force protection | Suggest: `tower-governor` + `governor` — GCRA algorithm | | |
| 🟠 P1 | Refresh Revocation | `api/src/auth.rs` | No refresh token revocation — logout only clears client-side | Existing: `redis-rs` + `sea-orm` | | |
| 🟠 P1 | Token Logging | `api/src/auth.rs` | Access tokens logged in plaintext in tracing spans | Existing: `tracing` | | |
| 🟠 P1 | MCP Authentication | `api/src/mcp/mod.rs` | `/mcp` endpoint auth is commented out | Existing: `rmcp` | | |
| 🟠 P1 | OTA v2 | `api/src/ota.rs` + `api/src/device.rs` | Device registration + activation + admin CRUD implemented; OTA v2 pending | — | | |

## Voice Input & Processing

| Priority | Item | Location | Description | Open Source / Libraries | Status | Test |
|------|------|------|------|------|------|------|
| 🔴 P0 | Wake Word Support | `api/src/ws/` + protocol layer | ESP32 client already has ESP-SR offline wake word; server needs `wake_word` message type (protocol lacks this field) | — (ESP32-side ESP-SR; server-side protocol parsing only) | | |
| 🟠 P1 | Speaker Identification (Voiceprint) | New feature | Identify speaker in parallel with ASR and inject into LLM for personalized replies; needs register/manage/identify flow + DB voiceprint vectors + LLM context injection | Existing: `sherpa-onnx` (ECAPA-TDNN/WeSpeaker) | | |
| 🟠 P1 | Opus Division by Zero | `api/src/ws/default_listener.rs` | Divide by zero when channels=0 / sample_rate=0 | — | | |
| 🟠 P1 | Audio Hot-path Cloning | `api/src/ws/default_listener.rs` | `data.to_vec()` called every 20ms — frequent cloning | — | | |
| 🟠 P1 | Tiered Turn Detection | New feature | 4-layer architecture replacing pure VAD: Silero VAD → duration check → punctuation/semantic completeness → filler words; currently Layer 1 only | Suggest: LiveKit `v1-mini`; OpenAI config pattern; arxiv 2606.13450 | | |
| 🟠 P1 | Dynamic VAD Parameters | `api/src/ws/default_listener.rs` + `vad/` | `silence_voice_timeout=1200ms` fixed at config time; needs runtime adjustment by conversation state | Ref: vui `asr_worker.py:198-206`; speech-to-speech `RuntimeConfig` | | |
| 🟠 P1 | ASR Settle + Speculative Reopen | `api/src/ws/default_listener.rs` | Immediate ASR after VAD silence may lose trailing phonemes; no turn reopen mechanism | Ref: vui `voice_turn.py:696-771`; HF speech-to-speech `speculative_turns.py` | | |
| 🟡 P2 | Filler Word / Hesitation Detection | `api/src/ws/default_listener.rs` | ASR outputs `uh/um/呃/嗯` + short audio (<3s) → skip LLM trigger | Ref: vui `_ends_with_filler`; desert-ant-labs/uhm | | |
| 🟡 P2 | VAD Sample Rate | `api/src/component/vad/` | Hardcoded 16kHz — silent failure on non-16kHz input | — | | |
| 🟡 P2 | ASR | `api/src/component/asr/` | XAsr (sherpa-onnx), lacks `Sync` trait, 16kHz mono only | Existing: `sherpa-onnx` | | |
| 🟡 P2 | Ambient Listening Mode | New feature | Passive listening + proactive response without wake word; needs privacy architecture (Nabla: no raw audio storage) | — | | |
| 🔵 P3 | Speaker Diarization | `api/src/listener/` | Speaker separation in multi-user scenarios | Existing: `sherpa-onnx` / Suggest: `polyvoice` | | |
| 🔵 P3 | AEC Server-side Noise Reduction | `api/src/ws/default_listener.rs` | vanling has client-side AEC only | Suggest: `aec3` — pure Rust WebRTC AEC3 | | |
| 🔵 P3 | WebRTC Real-time Audio/Video | New feature | Low latency + Live2D + multimodal vision + MCP | Suggest: `webrtc-rs` (v0.17.x) | | |
| 🔵 P3 | Audio Normalization Integration | `api/src/util/compressor.rs` → pipeline | `adaptive_normalize()` implemented but not integrated into TTS output pipeline | — | | |
| 🔵 P3 | Guided Reasoning Conversation | New feature | Education-AI guided mode; needs LLM prompt engineering + conversation state management | — | | |

## Language Model & Reasoning

| Priority | Item | Location | Description | Open Source / Libraries | Status | Test |
|------|------|------|------|------|------|------|
| 🔴 P0 | LLM Thread Safety + Echo | `api/src/component/llm/model/qwen3/mod.rs` + `echo/mod.rs` | `thread::spawn` + `block_on` wrapped with `catch_unwind`, no more silent panic crash | — | ✅Completed 2026-07-24 | |
| 🟠 P1 | Emotion Recognition Completion | `api/src/component/llm/model/` (analyze_emotion) | Stub returning "happy"; needs dual-channel audio-feature + text-sentiment fusion to adjust TTS tone | Existing: `sherpa-onnx` (SER models) | | |
| 🟠 P1 | Personalized Memory / Long-term Preferences | New feature | Currently only chat history; needs user profile + forgetting curve + vector retrieval | Suggest: `qdrant` + `rig-core` | | |
| 🟠 P1 | RAG Knowledge Base | MCP or built-in module | MCP framework can integrate but has no built-in vector retrieval | Existing: `rig-core` (10+ vector storage backends) | | |
| 🟠 P1 | Intent Recognition | New feature | No independent intent layer | Existing: `rmcp` (function_call) | | |
| 🟠 P1 | LLM History Blocking | `api/src/component/llm/model/qwen3/mod.rs` | DB write blocks full thread | — | | |
| 🟡 P2 | Agent Task Orchestration | New feature | LLM + MCP tool chain executes complex tasks autonomously | Existing: `rig-core` (Agent/Chain/Router) | | |
| 🔵 P3 | describe O(n) | `api/src/component/llm/model/qwen3/mod.rs` | Real-time full message history construction | — | | |

## Speech Synthesis (TTS)

| Priority | Item | Location | Description | Open Source / Libraries | Status | Test |
|------|------|------|------|------|------|------|
| 🟠 P1 | Two-phase Streaming TTS Architecture | `api/src/component/ling/splitter.rs` + `round.rs` + `api/src/component/tts/mod.rs` | Phase 1 token-level first chunk + Phase 2 sentence-level steady state; TTS callback encodes Opus while generating; latency reduced from `STT+LLM+TTS` to `max(STT,LLM,TTS)` | Ref: Qwen3-TTS-streaming; vui `chunk_words`; arxiv 2603.05413 | ✅Completed 2026-07-23 | |
| 🟠 P1 | Audio Fade-out | `api/src/component/tts/` | ~200ms linear fade-out at TTS tail eliminates clicks | Ref: vui `tts_worker.py:713-783` | ✅Completed 2026-07-23 | |
| 🟠 P1 | Audio Hold Buffer | `api/src/component/tts/` | Buffer last ~240ms frames (fade-out done; hold buffer outstanding) | Ref: vui `tts_worker.py:713-783` | | |
| 🟠 P1 | sentencex Integration | `api/src/component/ling/splitter.rs` | Splitter supports abbreviations, context look-ahead, minimum sentence length; 200+ languages | Existing: `sentencex` | ✅Completed 2026-07-23 | |
| 🟠 P1 | TTFA/RTF Performance Metrics | `api/src/component/tts/mod.rs` | Per-request TTFA/RTF sampling; P50/P90/P95 percentiles persisted | Ref: Dupdub TTS latency; Sherlock Calls P95 | ✅Completed 2026-07-23 | |
| 🟠 P1 | Piper/Kokoro TTS Integration | `api/src/component/tts/` | Can replace or supplement MatchaTTS; Piper for edge deployment, Kokoro best quality/size ratio | Suggest: `sherpa-onnx` (Piper ONNX models) | | |
| 🟠 P1 | Multi-language TTS | `api/src/component/tts/` | Currently single-language; ESP32 supports 25+ ASR languages that need matching | Suggest: `sherpa-onnx` (Piper/VITS ONNX models) | | |
| 🟠 P1 | Quick Reply Pre-response | New feature | Play "I'm here" / "Coming" during LLM inference to reduce perceived latency | — | | |
| 🟠 P1 | Dynamic TTS Voice Switching | New feature | Auto-switch TTS voice based on speaker identification | Existing: `sherpa-onnx` | | |
| 🟠 P1 | Opus-to-PCM Decoder Pre-buffering | `service/src/session/round.rs` | No client pre-buffering — network jitter causes playback stutter | Ref: speech-to-speech `leftover_samples`; sherpa-onnx Opus decoder | | |
| 🟡 P2 | Inter-sentence Silence Optimization | `api/src/component/tts/` | Fixed silence → dynamic by punctuation (comma 0.3s / period 0.6s) | Ref: RealtimeVoiceChat `ENGINE_SILENCES` | | |
| 🟡 P2 | Conversational Prosody TTS | New feature | Breathing/hesitation/laughter prosody for more human-like speech | Suggest: `csm.rs` — Rust Sesame CSM (AGPL-3.0) | | |
| 🟡 P2 | Emotion-adaptive Intonation | New feature | Detect 600+ emotion tags and adaptively adjust TTS tone | Suggest: `voirs-emotion` | | |
| 🔵 P3 | Voice Cloning | `api/src/component/tts/` | MatchaTTS supports reference audio — needs exposed config interface | Suggest: `sherpa-onnx` (speaker embedding) | | |
| 🔵 P3 | TTS Loop Cloning | `api/src/component/tts/` | `Arc<str>` vs `String` cloning storm | — | | |
| 🟠 P1 | Hann Crossfade Anti-click | `api/src/component/tts/mod.rs` | Hann window crossfade (512 samples = 32ms) removes chunk-boundary clicks | Ref: Qwen3-TTS-streaming `overlap_samples=512` | ❌Discarded 2026-07-23 — Input PCM is continuous, crossfade unnecessary | |
| 🟠 P1 | Leading Silence Trimming + 40ms Preroll | `api/src/component/tts/mod.rs` | Trim leading silence on first packet, keep 40ms preroll soft start | Ref: speech-to-speech `trim_silence()` | ❌Discarded 2026-07-23 — Incompatible with streaming architecture | |

## Tools & Integration

| Priority | Item | Location | Description | Open Source / Libraries | Status | Test |
|------|------|------|------|------|------|------|
| 🟠 P1 | Smart Home Integration (Home Assistant) | MCP or standalone | Smart home is a core voice assistant scenario | Existing: `rmcp` (HA MCP Server) | | |
| 🟠 P1 | Cross-device Calling | MCP tool or standalone | Devices call each other like phones; needs MQTT gateway + contact management | Suggest: `rumqttc` — pure Rust MQTT client | | |
| 🟠 P1 | Music Playback | MCP tool or standalone | Industry standard; LRC lyrics sync support | Suggest: `rodio` — cross-platform audio playback | | |
| 🟠 P1 | Timer/Reminders/Alarms | MCP tool or standalone | Industry standard; no timing/reminder mechanism yet | Suggest: `tokio-cron-scheduler` | | |
| 🟡 P2 | Plugin System | New feature | No plugin architecture — feature extensions require core-code changes | Suggest: `wasmtime` — WASI P2 + Component Model | | |
| 🟡 P2 | MCP Market / Tool Marketplace | New feature | Aggregate third-party markets + one-click import + hot-reload | Existing: `rmcp` | | |
| 🟡 P2 | MCP Debug Console | New feature | Per-agent independent MCP endpoints, real-time call testing + per-agent tool filtering | Existing: `rmcp` | | |
| 🟡 P2 | MCP Tool Aggregation | New feature | Pre-built tool library (DingTalk/QQ/system monitor/WebPilot/math) | Existing: `rmcp` | | |
| 🟡 P2 | MQTT Gateway | New feature | MQTT+UDP → WS bridging: distributed deployment + load balancing + HMAC auth | Suggest: `rumqttc` — pure Rust, tokio native | | |
| 🟡 P2 | Configuration Wizard + E2E Testing | New feature | First-run wizard + per-component latency tests + visualization | — | | |

## Session & Device

| Priority | Item | Location | Description | Open Source / Libraries | Status | Test |
|------|------|------|------|------|------|------|
| 🔴 P0 | stop_round Race Condition | `service/src/session/round.rs` | Synchronized `llm_tts_handle` with `stop_round`, eliminating use-after-cancel | — | ✅Completed 2026-07-24 | |
| 🟠 P1 | Device Management | `api/src/device.rs` | Device list/activate/disable/enable/delete/details CRUD | Existing: `sea-orm` | ✅Completed 2026-07-30 | |
| 🟠 P1 | Continued Conversation | `service/src/session/` | Mic stays open briefly after reply for wake-word-free follow-ups | — | | |
| 🟠 P1 | Clock Overflow | `service/src/session/mod.rs` | `Local::now()` non-monotonic — subtraction can overflow | Existing: `jiff` (monotonic clock) | | |
| 🟠 P1 | Recorder No Limit | `api/src/record/recorder.rs` | `Vec<RecordEntry>` unbounded — memory grows without limit under concurrency | — | | |
| 🔵 P3 | Session Export/Delete | `api/src/record/` | Sessions view-only — no export or deletion | — | | |

## Protocol & Transport

| Priority | Item | Location | Description | Open Source / Libraries | Status | Test |
|------|------|------|------|------|------|------|
| 🟡 P2 | Message Types | `api/src/ws/frame.rs` | Missing `system`, `alert`, `custom`, `wake_word` message types (vs xiaozhi-esp32 spec) | — | | |
| 🟡 P2 | Multi ASR/TTS Provider | `api/src/component/asr/` + `api/src/component/tts/` | vanling has 1 ASR + 1 TTS only | Existing: `sherpa-onnx` (multi-model switching) | | |

## Infrastructure

| Priority | Item | Location | Description | Open Source / Libraries | Status | Test |
|------|------|------|------|------|------|------|
| 🔴 P0 | Signal Macro | `framework/src/signal.rs` | Fixed non-existent `debug_error!` macro; compiles on non-unix | — | ✅Completed 2026-07-24 | |
| 🟠 P1 | Panic Handling | `framework/src/panic.rs` | `eprintln!` → `tracing::error!`, wired into Sentry | Existing: `tracing` | ✅Completed 2026-07-24 | |
| 🟠 P1 | email Constraint | `migration/src/m20241230_000001_init.rs` | Entity has `#[sea_orm(unique)]` but migration lacks UNIQUE | Existing: `sea-orm` (migration fix) | | |
| 🟠 P1 | FK Constraints | `migration/src/m20241230_000001_init.rs` | Missing FKs: `round.session_id`, `round_data.round_id`, `frame.round_id` | Existing: `sea-orm` (migration fix) | | |
| 🟠 P1 | MCP Lock Ordering | `api/src/mcp/mcp_host.rs` | UnionMcpHost device/server lock order ABBA — possible deadlock | — | | |
| 🟠 P1 | Graceful Shutdown Order | `framework/src/signal.rs` + `apps/server` | Missing shutdown sequence across modules | — | | |
| 🟠 P1 | Runtime Race Condition | `framework/src/runtime.rs` | `OnceLock` initialization race | — | | |
| 🔵 P3 | Timestamp Auto-fill | `entity/src/config.rs` | `Config` entity missing `ActiveModelBehavior` — timestamps not auto-filled | Existing: `sea-orm` | | |
| 🔵 P3 | MCP Error Handling | `api/src/mcp/` | Incomplete | Existing: `rmcp` | | |

## Frontend (apps/server-ui)

| Priority | Item | Location | Description | Open Source / Libraries | Status | Test |
|------|------|------|------|------|------|------|
| 🟠 P1 | Dashboard Page | `routes/_pathlessLayout.admin/index.tsx` | StatCard/TrendsChart/LatencyChart/RecentSessionsTable/LatencyTable | Existing: `@mantine/core` v9 + `@tanstack/react-query` | ✅Completed 2026-07-30 | |
| 🟠 P1 | User CRUD Admin UI | New page | No user list/create/delete/role management interface | Existing: `@mantine/core` v9 | | |
| 🟠 P1 | Multi-user/RBAC Management | New feature | Dashboard should include token usage/conversation duration/device activity/visualization + RBAC | Existing: `@mantine/core` v9 | | |
| 🔵 P3 | System Monitoring | New page | No server health/connection count/resource usage/error rate dashboard | Existing: `@mantine/core` v9 + `@tanstack/react-query` | | |
| 🔵 P3 | MCP Dashboard | New feature | Multi-endpoint management + tool sync + group access control + logs | Existing: `@mantine/core` v9 | | |

## Mobile (apps/app)

| Priority | Item | Location | Description | Open Source / Libraries | Status | Test |
|------|------|------|------|------|------|------|
| 🟠 P1 | Flutter WS Integration | `apps/app/` | Flutter app exists as scaffold but has no WS + auth integration | Suggest: `web_socket_channel` | | |
| 🟠 P1 | App CI/CD | `.github/workflows/` | No iOS/Android build/sign/release pipeline | GitHub Actions | | |

## Status Legend

| Status | Meaning |
|--------|---------|
| (empty) | In progress / not completed |
| ✅Completed YYYY-MM-DD | Done, with completion date |
| ❌Discarded — reason | Won't implement, with reason |

## Priority Legend

| Level | Meaning | Action |
|-------|---------|--------|
| 🔴 P0 | Must fix immediately | Compile errors, no auth, incomplete data |
| 🟠 P1 | Should fix | Race conditions, memory leaks, security risks, missing core features |
| 🟡 P2 | Feature gaps | Incomplete protocols, insufficient configurability |
| 🔵 P3 | Optimization | Performance, code quality, non-core features |