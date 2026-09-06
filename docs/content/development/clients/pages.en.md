+++
title = "Page Implementation"
weight = 303
[extra]
source_file_hash = "eb4947f4f13f87d25964a5c49959361cd31e1159"
translated_at = "2026-09-06T12:53:24Z"
+++

# Page Implementation

This doc describes how clients (Flutter App / esp32c6 firmware) implement the page model and output channels from §3 of the [Shared Client Protocol](shared-protocol.en.md). The protocol constrains only "shared page vocabulary, protocol-driven `chat` page, state as reducer projection"; all other implementation details are device-local.

## 1. Four-Layer Architecture

```
① SessionService —— WS + reducer, produces the single fact source ChatProjection{ state, messages, subtitle, emotion }
② FeedbackHub   —— subscribes to ①, drives Led/Audio/Vib/Web channels (cross-cutting projection)
③ DisplayStack  —— page manager + status bar + overlay (display channel)
④ Renderer      —— one per page, pure consumer (Flutter widget / esp32c6 framebuffer)
```

②③ are both subscribers of ①: the chat page and LED/earcon do not compete — they are parallel channels over the same projection, synchronized by construction.

## 2. Page Descriptor

Each page = a 4-tuple:

- `id` (§3 page-table vocabulary: chat / home / config / ota / settings)
- content model: manages its own data source
- input handler: which inputs it accepts (see §4 input routing)
- renderer: data → visuals

## 3. Display Stack Rules

The page manager has exactly three responsibilities:

- **Single foreground, switchable**: exactly one foreground page at any time, with a back/switch stack
- **Layering**: status bar (network/battery/clock) is persistently drawn below pages; overlay (notification/alert) is transiently drawn above pages, but does **not** switch the foreground page
- **Input dispatch**: route inputs per the three-way routing in §4

## 4. Three-Way Input Routing

| Input | Destination | Examples |
| --- | --- | --- |
| Session semantics | reducer (①) | wake / push-to-talk / barge-in / text → `listen_*`/`abort`/`text` |
| Page interaction | foreground page (③) | config form / settings toggle / back |
| Global / privacy | chrome layer | mic off, DND (outside the state machine, persistent independent item) |

## 5. Chat Page (Key Difference)

- Data source = **subscribe to `ChatProjection`**, not raw frames — frames are already folded upstream by SessionService; the page only re-renders on projection change
- Rendering = `f(projection)`, idempotent, flicker-free (the payoff of §3's idempotent no-op)
- Mounted only when the voice peripheral is compiled in; voice-less devices have no reducer and no chat page, other pages work as usual

## 6. Non-Chat Pages

| Page | Data source | Update method |
| --- | --- | --- |
| home | Telemetry snapshot (HTTP state store / MCP direct read) | Local polling/push, read → render |
| config | Local form model + HTTP submission | Local events/buttons |
| ota | OTA driver events | Progress → render |
| settings | Locally persisted configuration | Toggle/buttons |

## 7. Capability Gate

- With the `voice` peripheral: ① includes reducer + chat page; ② full feedback hub
- Without the `voice` peripheral: ① degrades to a command surface, ③ keeps 4 pages + status bar, ② optional channels only (Led/Web, etc.)
- esp32c6 without an OS: page management = "id + dirty flag"; overlay/status bar are drawing-layer overlays

## 8. Example: Full Voice-Turn Walk-Through

esp32c6: OLED + tri-color LED + speaker + Action button.

1. **Press Action button** → local event `listen_start(manual)` enters the reducer: `idle→listening`. The projection change broadcasts to all channels: the chat page shows "Listening…" and clears the previous message; the LED lights `mic_live` red (audio streaming, privacy baseline); the speaker plays a local "beep" earcon; the Web channel pushes the new state.
2. **User speaks** → upstream audio goes through the server pipeline; `stt` (partial) frames return → the reducer updates the user message in place (`is_interim:true`), the chat page scrolls and shows it in gray.
3. **Result returns** → `llm` frame → reducer: `listening→thinking` (mic stops, LED turns blue); assistant streamed text updates the user/assistant messages in place, `emotion` overridden per frame.
4. **`tts.start`** → reducer: `thinking→speaking`. The chat page drops `subtitle{text}` sentence by sentence; LED steady white (contrast baseline against idle); TTS voice is **product content output**, layered apart from the feedback earcon.
5. **`tts.stop`** → reducer: `speaking→idle`. The chat page keeps the last message and returns to idle; LED goes off (idle = contrast baseline across all channels).
6. **Throughout** the status bar is persistent (network/battery/clock); when OTA completes, the `overlay` "firmware upgrade available" tops the display for 5s then disappears, leaving the foreground chat page unaffected.

> The chat page only reacts to `ChatProjection` changes and never parses frames; LED/Audio/Web share the same projection, so they stay synchronized by construction.