+++
title = "Server"
weight = 100
sort_by = "weight"
[extra]
source_file_hash = "fcca62e811b0f20f5cc84bc6456e78ab6fe4bfb8"
translated_at = "2026-09-09T00:00:00Z"
+++

# Server

vanling server is built with Rust + axum, implementing the Xiaozhi smart speaker protocol with real-time voice conversation capabilities.

This section contains the following documents:

| Page | Weight | Description |
|------|--------|-------------|
| [Core Architecture](@/development/server/architecture.en.md) | 200 | Overall architecture, Session state machine, Round lifecycle |
| [Dialogue Flow](@/development/server/dialogue-flow.en.md) | 201 | Complete sequence diagrams for handshake and communication phases |
| [WebSocket Protocol](@/development/server/websocket-protocol.en.md) | 202 | Protocol field definitions and binary format |
| [Models and Deployment](@/development/server/models-and-deployment.en.md) | 203 | AI model specs, config system, Downloader, CUDA deployment |
| [TODO](@/development/server/TODO.en.md) | 204 | Completed/in-progress checklist (work entry point) |
| [Pipeline Redesign](@/development/server/pipeline-redesign.en.md) | 205 | Unified pipeline refactor plan (PipelineNode replaces Ling/Tts/Listener) |
| [Components and Module Layout](@/development/server/components.en.md) | 206 | Core skeleton hierarchy, dual-layer component, ling special node, sub-pipeline plan |
| [Positioning and Trade-off Reference](@/development/server/research.en.md) | 207 | Feature exploration, technology selection, reference projects |
