+++
title = "TODO"
weight = 204
+++

# TODO

> 项目演进入口。按功能模块分类，状态列（`✅已完成` / `❌丢弃` / 空=未完成）反映每个模块的演进脉络（已完成 → 进行中 → 丢弃决策）。测试列 `✅` = 已有对应测试。
>
> 功能探索、技术选型与参考项目见 [定位与取舍参考](@/development/server/research.md)。实际开发优先级以项目 Roadmap 为准。修复前请阅读 [AGENTS.md](https://github.com/anomalyco/vanling/blob/main/AGENTS.md) 了解开发规范。

## 安全 & 认证

| 优先级 | 项目 | 位置 | 描述 | 开源方案/类库 | 状态 | 测试 |
|------|------|------|------|------|------|------|
| 🔴 P0 | WS 认证 | `api/src/ws/mod.rs` | WS Upgrade 阶段 inline 验证 Bearer Token（JWT），支持 Authorization header + query 浏览器回退 | 已有: `axum` | ✅已完成 2026-08-26 | ✅ |
| 🔴 P0 | Invite Code 系统 | 新功能 | 无邀请码生成/验证/管理，新用户注册无控制 | 建议: `nanoid` — 短 URL 安全 ID 生成器 | | |
| 🟠 P1 | Rate Limiting | `api/src/auth.rs` | 无登录限流/暴力破解防护 | 建议: `tower-governor` + `governor` — GCRA 算法 | | |
| 🟠 P1 | Refresh 吊销 | `api/src/auth.rs` | refresh token 无吊销机制，logout 仅客户端清除 | 已有: `redis-rs` + `sea-orm` | | |
| 🟠 P1 | Token 日志 | `api/src/auth.rs` | access token 明文记录在 tracing span | 已有: `tracing` | | |
| 🟠 P1 | MCP 认证 | `api/src/mcp/mod.rs` | `/mcp` 端点认证被注释掉 | 已有: `rmcp` | | |
| 🟠 P1 | OTA v2 | `api/src/ota.rs` + `api/src/device.rs` | 设备注册 + 激活验证 + admin CRUD 已实现，OTA v2 仍待完成 | — | | |

## 语音输入与处理

| 优先级 | 项目 | 位置 | 描述 | 开源方案/类库 | 状态 | 测试 |
|------|------|------|------|------|------|------|
| 🔴 P0 | Wake Word 支持 | `api/src/ws/` + 协议层 | ESP32 客户端已有 ESP-SR 离线唤醒，服务端需处理 `wake_word` 消息类型（协议当前无此字段） | — (ESP32 端 ESP-SR，服务端仅协议解析) | | |
| 🟠 P1 | 声纹识别 (Voiceprint) | 新功能 | 与 ASR 并行识别说话人身份并注入 LLM 实现个性化回复；需注册/管理/识别流程 + DB 声纹向量 + LLM 上下文注入 | 已有: `sherpa-onnx` (ECAPA-TDNN/WeSpeaker) | | |
| 🟠 P1 | Opus 除零 | `api/src/ws/default_listener.rs` | channels=0 / sample_rate=0 时除零 | — | | |
| 🟠 P1 | 音频热路径克隆 | `api/src/ws/default_listener.rs` | 每 20ms `data.to_vec()` 频繁克隆 | — | | |
| 🟠 P1 | 分层轮次检测 | 新功能 | 4 层架构替代纯 VAD：Silero VAD → 持续时间 → 标点/语义完整性 → 填充词；当前仅 Layer 1 | 建议: LiveKit `v1-mini`; OpenAI 配置模式; arxiv 2606.13450 | | |
| 🟠 P1 | 动态 VAD 参数 | `api/src/ws/default_listener.rs` + `vad/` | `silence_voice_timeout=1200ms` 配置时固定；需按对话状态运行时调整 | 参考: vui `asr_worker.py:198-206`; speech-to-speech `RuntimeConfig` | | |
| 🟠 P1 | ASR Settle + Speculative Reopen | `api/src/ws/default_listener.rs` | VAD silence 后立即 ASR 可能丢失尾部音素；无 turn reopen 机制 | 参考: vui `voice_turn.py:696-771`; HF speech-to-speech `speculative_turns.py` | | |
| 🟡 P2 | 填充词/犹豫检测 | `api/src/ws/default_listener.rs` | ASR 输出 `uh/um/呃/嗯` + 短音频（<3s）→ 不触发 LLM | 参考: vui `_ends_with_filler`; desert-ant-labs/uhm | | |
| 🟡 P2 | VAD 采样率 | `api/src/component/vad/` | 硬编码 16kHz，非 16kHz 输入无声失败 | — | | |
| 🟡 P2 | ASR | `api/src/component/asr/` | XAsr (sherpa-onnx)，无 `Sync` trait，仅 16kHz 单声道 | 已有: `sherpa-onnx` | | |
| 🟡 P2 | 环境监听模式 | 新功能 | 非唤醒词触发的被动监听 + 主动响应，需隐私架构（Nabla 模式：不存储原始音频） | — | | |
| 🔵 P3 | Speaker Diarization | `api/src/listener/` | 多人场景下说话人分离 | 已有: `sherpa-onnx` / 建议: `polyvoice` | | |
| 🔵 P3 | AEC 服务端降噪 | `api/src/ws/default_listener.rs` | vanling 仅客户端 AEC | 建议: `aec3` — 纯 Rust WebRTC AEC3 | | |
| 🔵 P3 | WebRTC 实时音视频 | 新功能 | 低延迟 + Live2D + 多模态视觉 + MCP | 建议: `webrtc-rs` (v0.17.x) | | |
| 🔵 P3 | 音频标准化集成 | `api/src/util/compressor.rs` → 管道 | `adaptive_normalize()` 已实现但未集成到 TTS 输出管道 | — | | |
| 🔵 P3 | 引导式推理对话 | 新功能 | 教育 AI 引导模式，需 LLM prompt 工程 + 对话状态管理 | — | | |

## 语言模型与推理

| 优先级 | 项目 | 位置 | 描述 | 开源方案/类库 | 状态 | 测试 |
|------|------|------|------|------|------|------|
| 🔴 P0 | LLM 线程安全 + Echo | `api/src/component/llm/model/qwen3/mod.rs` + `echo/mod.rs` | `thread::spawn` + `block_on` 补 `catch_unwind`，panic 不再静默崩溃 | — | ✅已完成 2026-07-24 | |
| 🟠 P1 | 情绪识别完善 | `api/src/component/llm/model/` (analyze_emotion) | 当前 stub 返回 "happy"；需音频特征 + 文本情感双通道融合以调整 TTS 语气 | 已有: `sherpa-onnx` (SER 模型) | | |
| 🟠 P1 | 个性化记忆/长期偏好 | 新功能 | 当前仅 chat history；需用户画像 + 遗忘曲线 + 向量检索 | 建议: `qdrant` + `rig-core` | | |
| 🟠 P1 | RAG 知识库 | MCP 或内置模块 | MCP 框架可接入但无内置向量检索 | 已有: `rig-core` (10+ 向量存储后端) | | |
| 🟠 P1 | Intent 识别 | 新功能 | 无独立 intent 层 | 已有: `rmcp` (function_call) | | |
| 🟠 P1 | LLM 历史阻塞 | `api/src/component/llm/model/qwen3/mod.rs` | DB 落盘导致完整线程阻塞 | — | | |
| 🟡 P2 | Agent 任务编排 | 新功能 | LLM + MCP 工具链自主执行复杂任务 | 已有: `rig-core` (Agent/Chain/Router) | | |
| 🔵 P3 | describe O(n) | `api/src/component/llm/model/qwen3/mod.rs` | 实时构建全消息历史 | — | | |

## 语音合成 (TTS)

| 优先级 | 项目 | 位置 | 描述 | 开源方案/类库 | 状态 | 测试 |
|------|------|------|------|------|------|------|
| 🟠 P1 | 两阶段流式 TTS 架构 | `api/src/component/ling/splitter.rs` + `round.rs` + `api/src/component/tts/mod.rs` | Phase 1 token 级首包 + Phase 2 句子级稳态；TTS callback 边生成边 Opus 编码，延迟由 `STT+LLM+TTS` 降为 `max(STT,LLM,TTS)` | 参考: Qwen3-TTS-streaming; vui `chunk_words`; arxiv 2603.05413 | ✅已完成 2026-07-23 | |
| 🟠 P1 | 音频 Fade-out | `api/src/component/tts/` | TTS 尾部 ~200ms 线性淡出消除 click | 参考: vui `tts_worker.py:713-783` | ✅已完成 2026-07-23 | |
| 🟠 P1 | 音频 Hold Buffer | `api/src/component/tts/` | 尾部 ~240ms 帧缓存（fade-out 已实现，hold buffer 残留） | 参考: vui `tts_worker.py:713-783` | | |
| 🟠 P1 | sentencex 集成 | `api/src/component/ling/splitter.rs` | Splitter 支持缩写、上下文 lookahead、最小句长，中英等 200+ 语言 | 已有: `sentencex` | ✅已完成 2026-07-23 | |
| 🟠 P1 | TTFA/RTF 性能测量 | `api/src/component/tts/mod.rs` | 每请求 TTFA/RTF 采样，P50/P90/P95 百分位持久化 | 参考: Dupdub TTS latency; Sherlock Calls P95 | ✅已完成 2026-07-23 | |
| 🟠 P1 | Piper/Kokoro TTS 集成 | `api/src/component/tts/` | 可替换或补充 MatchaTTS；Piper 适合边缘部署，Kokoro 最佳质量/体积比 | 建议: `sherpa-onnx` (Piper ONNX 模型) | | |
| 🟠 P1 | 多语言 TTS | `api/src/component/tts/` | 当前仅单语言；ESP32 已支持 25+ 语言 ASR 需匹配 | 建议: `sherpa-onnx` (Piper/VITS ONNX 模型) | | |
| 🟠 P1 | Quick Reply 预回复 | 新功能 | LLM 推理期间先播放"我在"/"来了"，降低感知延迟 | — | | |
| 🟠 P1 | 动态 TTS 声音切换 | 新功能 | 基于声纹识别自动切换 TTS 音色 | 已有: `sherpa-onnx` | | |
| 🟠 P1 | Opus-to-PCM 解码预缓冲 | `service/src/session/round.rs` | 客户端无预缓冲，网络抖动导致播放卡顿 | 参考: speech-to-speech `leftover_samples`; sherpa-onnx Opus decoder | | |
| 🟡 P2 | 句间静音优化 | `api/src/component/tts/` | 固定句间静音 → 按标点动态调整（逗号 0.3s / 句号 0.6s） | 参考: RealtimeVoiceChat `ENGINE_SILENCES` | | |
| 🟡 P2 | 对话韵律 TTS | 新功能 | 生成呼吸/犹豫/笑声韵律，让语音更像真人 | 建议: `csm.rs` — Rust Sesame CSM (AGPL-3.0) | | |
| 🟡 P2 | 情绪自适应语调 | 新功能 | 检测 600+ 情感标签并自适应调整 TTS 语气 | 建议: `voirs-emotion` | | |
| 🔵 P3 | 声音克隆 | `api/src/component/tts/` | MatchaTTS 已支持 reference audio，需暴露配置接口 | 建议: `sherpa-onnx` (speaker embedding) | | |
| 🔵 P3 | TTS 循环克隆 | `api/src/component/tts/` | `Arc<str>` vs `String` 克隆风暴 | — | | |
| 🟠 P1 | Hann Crossfade 防 click | `api/src/component/tts/mod.rs` | Hann 窗 crossfade（512 samples = 32ms）消除 chunk 边界 click | 参考: Qwen3-TTS-streaming `overlap_samples=512` | ❌丢弃 2026-07-23 — 输入 PCM 连续，无需 crossfade | |
| 🟠 P1 | 首包静音裁剪 + 40ms preroll | `api/src/component/tts/mod.rs` | 裁剪首包 leading silence，保留 40ms preroll 软起音 | 参考: speech-to-speech `trim_silence()` | ❌丢弃 2026-07-23 — 与流式架构不兼容 | |

## 工具与集成

| 优先级 | 项目 | 位置 | 描述 | 开源方案/类库 | 状态 | 测试 |
|------|------|------|------|------|------|------|
| 🟠 P1 | 家居集成 (Home Assistant) | MCP 或独立模块 | 智能家居是语音助手核心场景 | 已有: `rmcp` (HA MCP Server) | | |
| 🟠 P1 | 设备间通话 | MCP tool 或独立功能 | 设备间像电话一样互相呼叫，需 MQTT gateway + 通讯录管理 | 建议: `rumqttc` — 纯 Rust MQTT 客户端 | | |
| 🟠 P1 | 音乐播放 | MCP tool 或独立功能 | 行业标配；支持 LRC 歌词同步 | 建议: `rodio` — 跨平台音频播放 | | |
| 🟠 P1 | Timer/提醒/闹钟 | MCP tool 或独立功能 | 行业标配，当前无任何定时/提醒机制 | 建议: `tokio-cron-scheduler` | | |
| 🟡 P2 | 插件系统 | 新功能 | 无插件架构，功能扩展需修改核心代码 | 建议: `wasmtime` — WASI P2 + Component Model | | |
| 🟡 P2 | MCP Market / 工具市场 | 新功能 | 聚合第三方市场 + 一键导入 + 热加载 | 已有: `rmcp` | | |
| 🟡 P2 | MCP 调试控制台 | 新功能 | 每 Agent 独立 MCP 端点，实时调用测试 + per-agent 工具过滤 | 已有: `rmcp` | | |
| 🟡 P2 | MCP 工具聚合 | 新功能 | 预置工具库（钉钉/QQ/系统监控/WebPilot/数学计算） | 已有: `rmcp` | | |
| 🟡 P2 | MQTT 网关 | 新功能 | MQTT+UDP → WS 桥接：分布式部署 + 负载均衡 + HMAC 认证 | 建议: `rumqttc` — 纯 Rust，tokio 原生 | | |
| 🟡 P2 | 配置向导 + 全链路测试 | 新功能 | 首次运行向导 + 每组件延迟测试 + 可视化 | — | | |

## 会话与设备

| 优先级 | 项目 | 位置 | 描述 | 开源方案/类库 | 状态 | 测试 |
|------|------|------|------|------|------|------|
| 🔴 P0 | stop_round 竞态 | `service/src/session/round.rs` | `llm_tts_handle` 与 `stop_round` 补同步，消除 use-after-cancel | — | ✅已完成 2026-07-24 | |
| 🟠 P1 | 设备管理 | `api/src/device.rs` | 列表/激活/禁用/启用/删除/详情 CRUD | 已有: `sea-orm` | ✅已完成 2026-07-30 | |
| 🟠 P1 | Continued Conversation | `service/src/session/` | 回复后麦克风短暂保持开放，允许免唤醒词追问 | — | | |
| 🟠 P1 | 时钟溢出 | `service/src/session/mod.rs` | `Local::now()` 非单调，减法可溢出 | 已有: `jiff` (单调时钟) | | |
| 🟠 P1 | Recorder 无上限 | `api/src/record/recorder.rs` | `Vec<RecordEntry>` 无大小限制，高并发内存无限增长 | — | | |
| 🔵 P3 | Session 导出/删除 | `api/src/record/` | Session 仅可查看，不可导出或删除 | — | | |

## 协议与传输

| 优先级 | 项目 | 位置 | 描述 | 开源方案/类库 | 状态 | 测试 |
|------|------|------|------|------|------|------|
| 🟡 P2 | 消息类型 | `api/src/ws/frame.rs` | 缺失 `system`、`alert`、`custom`、`wake_word` 消息类型（对比 xiaozhi-esp32 规范） | — | | |
| 🟡 P2 | 多 ASR/TTS Provider | `api/src/component/asr/` + `api/src/component/tts/` | vanling 仅 1 ASR + 1 TTS | 已有: `sherpa-onnx` (多模型切换) | | |

## 基础设施

| 优先级 | 项目 | 位置 | 描述 | 开源方案/类库 | 状态 | 测试 |
|------|------|------|------|------|------|------|
| 🔴 P0 | Signal 宏 | `framework/src/signal.rs` | 修复不存在的 `debug_error!` 宏，非 unix 可编译 | — | ✅已完成 2026-07-24 | |
| 🟠 P1 | Panic 处理 | `framework/src/panic.rs` | `eprintln!` → `tracing::error!`，接入 Sentry | 已有: `tracing` | ✅已完成 2026-07-24 | |
| 🟠 P1 | email 约束 | `migration/src/m20241230_000001_init.rs` | entity 标注 `#[sea_orm(unique)]`，迁移未实现 UNIQUE | 已有: `sea-orm` (migration fix) | | |
| 🟠 P1 | 外键约束 | `migration/src/m20241230_000001_init.rs` | 缺少 FK: `round.session_id`、`round_data.round_id`、`frame.round_id` | 已有: `sea-orm` (migration fix) | | |
| 🟠 P1 | MCP 锁顺序 | `api/src/mcp/mcp_host.rs` | UnionMcpHost device/server 锁顺序 ABBA，可能死锁 | — | | |
| 🟠 P1 | 优雅关闭顺序 | `framework/src/signal.rs` + `apps/server` | 各模块关闭顺序缺失 | — | | |
| 🟠 P1 | Runtime 竞态 | `framework/src/runtime.rs` | `OnceLock` 初始化存在竞态 | — | | |
| 🔵 P3 | 时间戳自动填充 | `entity/src/config.rs` | `Config` 实体缺失 `ActiveModelBehavior`，时间戳未自动填充 | 已有: `sea-orm` | | |
| 🔵 P3 | MCP 错误处理 | `api/src/mcp/` | 不完善 | 已有: `rmcp` | | |

## 前端 (apps/server-ui)

| 优先级 | 项目 | 位置 | 描述 | 开源方案/类库 | 状态 | 测试 |
|------|------|------|------|------|------|------|
| 🟠 P1 | Dashboard 页 | `routes/_pathlessLayout.admin/index.tsx` | StatCard/TrendsChart/LatencyChart/RecentSessionsTable/LatencyTable | 已有: `@mantine/core` v9 + `@tanstack/react-query` | ✅已完成 2026-07-30 | |
| 🟠 P1 | User CRUD 管理 UI | 新页面 | 无用户列表/创建/删除/角色管理界面 | 已有: `@mantine/core` v9 | | |
| 🟠 P1 | 多用户/RBAC 管理 | 新功能 | Dashboard 应含 Token 用量/对话时长/设备活跃度/可视化和 RBAC | 已有: `@mantine/core` v9 | | |
| 🔵 P3 | 系统监控 | 新页面 | 无服务器健康/连接数/资源使用/错误率仪表盘 | 已有: `@mantine/core` v9 + `@tanstack/react-query` | | |
| 🔵 P3 | MCP 仪表盘 | 新功能 | 多端点管理 + 工具同步 + 群组访问控制 + 日志 | 已有: `@mantine/core` v9 | | |

## 移动端 (apps/app)

| 优先级 | 项目 | 位置 | 描述 | 开源方案/类库 | 状态 | 测试 |
|------|------|------|------|------|------|------|
| 🟠 P1 | Flutter WS 集成 | `apps/app/` | Flutter app 存在脚手架但未集成 WS + 认证 | 建议: `web_socket_channel` | | |
| 🟠 P1 | App CI/CD | `.github/workflows/` | 无 iOS/Android 构建/签名/发布流水线 | GitHub Actions | | |

## 状态说明

| 状态 | 含义 |
|------|------|
| （空） | 未完成 |
| ✅已完成 YYYY-MM-DD | 已完成，附完成日期 |
| ❌丢弃 — 原因 | 不再推进，附原因说明 |

## 优先级说明

| 等级 | 含义 | 行动 |
|------|------|------|
| 🔴 P0 | 必须立即修复 | 编译错误、无认证、数据不完整 |
| 🟠 P1 | 应该修复 | 竞态、内存泄漏、安全风险、核心功能缺失 |
| 🟡 P2 | 功能缺失 | 协议不完整、配置化不足 |
| 🔵 P3 | 优化 | 性能、代码质量、非核心功能 |