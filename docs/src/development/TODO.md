# TODO

按项目目录分类的待办事项清单。修复前请阅读 [AGENTS.md](../../../AGENTS.md) 了解开发规范。

## apps/server

### WS Session

| 模块 | 文件 | 发现 | 严重程度 |
|------|------|------|----------|
| stop_round 竞态 | `api/src/ws/session/round.rs` | `llm_tts_handle` 与 `stop_round` 之间缺少同步，可能 use-after-cancel | 🔴 P0 |
| Opus 除零 | `api/src/ws/session/listener.rs` | channels=0 / sample_rate=0 时除零 | 🟡 P1 |
| 时钟溢出 | `api/src/ws/session/` | `Local::now()` 非单调，减法可溢出 | 🟡 P1 |
| OutputController | `api/src/ws/session/output_controller.rs` | `bounded(64)` 通道 + tokio interval 节流，结构合理 | ✅ 正常 |
| Listener | `api/src/ws/session/listener.rs` | VAD + ASR 编排，300ms 前缀缓冲，Opus 解码 | ✅ 正常 |
| Phase 状态机 | `api/src/ws/session/mod.rs` | 状态转换逻辑完整 | ✅ 正常 |

### 协议

| 模块 | 文件 | 发现 | 严重程度 |
|------|------|------|----------|
| 消息类型 | `api/src/ws/frame.rs` | 缺失 `system`、`alert`、`custom` 消息类型（对比 xiaozhi-esp32 规范） | ⚠️ P2 |
| Frame 枚举 | `api/src/ws/frame.rs` | 入站 Frame / 出站 FrameResult 结构清晰 | ✅ 正常 |
| 消息解析 | `api/src/ws/message_converter.rs` | WS Message → Frame 反序列化 | ✅ 正常 |

### AI 模块

| 模块 | 文件 | 发现 | 严重程度 |
|------|------|------|----------|
| LLM 线程安全 | `api/src/llm/client.rs` | `thread::spawn` + `block_on`，未 `catch_unwind`，panic 静默崩溃 | 🔴 P0 |
| VAD 采样率 | `api/src/vad/` | 硬编码 16kHz，非 16kHz 输入无声失败 | ⚠️ P2 |
| ASR | `api/src/asr/` | SenseVoice (sherpa-onnx)，无 `Sync` trait，仅 16kHz 单声道 | ⚠️ P2 |
| LLM 历史阻塞 | `api/src/llm/client.rs` | DB 落盘导致完整线程阻塞 | 🟡 P1 |
| describe O(n) | `api/src/llm/client.rs` | 实时构建全消息历史 | 🟢 P3 |
| TTS 循环克隆 | `api/src/tts/` | `Arc<str>` vs `String` 克隆风暴 | 🟢 P3 |
| TTS | `api/src/tts/` | MatchaTTS + Opus 编码，Factory 全局单例 | ✅ 正常 |

### 持久化

| 模块 | 文件 | 发现 | 严重程度 |
|------|------|------|----------|
| on_session_end 未调用 | `api/src/record/observer.rs` | `SessionObserver::on_session_end` 定义了但从未调用，DB session.end_time 永远为 NULL | 🔴 P0 |
| RecordCollector 无上限 | `api/src/record/collector.rs` | `Vec<RoundBuffer>` 无大小限制，高并发内存无限增长 | 🟡 P1 |
| 双重序列化 | `api/src/record/` | record 路径双重 JSON 序列化 | 🟢 P3 |
| SessionObserver trait | `api/src/record/observer.rs` | 9 个生命周期钩子，设计良好 | ✅ 正常 |

### 安全

| 模块 | 文件 | 发现 | 严重程度 |
|------|------|------|----------|
| WS 认证 | `api/src/ws/mod.rs:59` | `// .layer(get_auth_layer())` 注释掉，所有 WS 连接未认证 | 🔴 P0 |
| JWT 密钥 | `api/src/config/mod.rs` | 硬编码默认密钥 `chobits-jwt-secret` | 🔴 P0 |
| Token 日志 | `api/src/auth.rs` | access token 明文记录在 tracing span | 🟡 P1 |
| Refresh 吊销 | `api/src/auth.rs` | refresh token 无吊销机制 | 🟡 P1 |

### MCP

| 模块 | 文件 | 发现 | 严重程度 |
|------|------|------|----------|
| 锁顺序风险 | `api/src/mcp/mcp_host.rs` | UnionMcpHost device/server 锁顺序 ABBA，可能死锁 | 🟡 P1 |
| 认证缺失 | `api/src/mcp/` | `/mcp` 端点无认证 | 🟡 P1 |
| 错误处理 | `api/src/mcp/` | 不完善 | 🟢 P3 |

### 数据库

| 模块 | 文件 | 发现 | 严重程度 |
|------|------|------|----------|
| email 约束 | `entity/src/user.rs` | entity 标注 `#[sea_orm(unique)]`，迁移未实现 UNIQUE | 🟡 P1 |
| 外键约束 | `migration/src/m20241230_000001_init.rs` | 缺少 FK: `round.session_id`、`round_data.round_id`、`frame.round_id` | 🟡 P1 |
| 时间戳自动填充 | `entity/src/` | `Config` 实体缺失 `ActiveModelBehavior`，时间戳未自动填充 | 🟢 P3 |

### 性能

| 问题 | 位置 | 描述 | 严重程度 |
|------|------|------|----------|
| 音频热路径克隆 | `api/src/ws/session/listener.rs` | 每 20ms `data.to_vec()` 频繁克隆 | 🟡 P1 |

## libs

### framework

| 模块 | 文件 | 发现 | 严重程度 |
|------|------|------|----------|
| signal 宏 | `framework/src/signal.rs` | 使用不存在的 `debug_error!` 宏，非 unix 编译失败 | 🔴 P0 |
| panic 处理 | `framework/src/panic.rs` | `eprintln!` 而非 `tracing::error!`，绕过 Sentry | 🟡 P1 |
| runtime 竞态 | `framework/src/runtime.rs` | `OnceLock` 初始化存在竞态 | 🟡 P1 |
| 优雅关闭 | `framework/src/signal.rs` | 缺少各模块关闭顺序 | 🟡 P1 |

## 跨项目

| 问题 | 涉及 | 描述 | 严重程度 |
|------|------|------|----------|
| 优雅关闭顺序 | apps/server + libs | 缺少各模块关闭顺序 | 🟡 P1 |

---

## 优先级说明

| 等级 | 含义 | 行动 |
|------|------|------|
| 🔴 P0 | 必须立即修复 | 编译错误、无认证、数据不完整 |
| 🟡 P1 | 应该修复 | 竞态、内存泄漏、安全风险 |
| ⚠️ P2 | 功能缺失 | 协议不完整、配置化不足 |
| 🟢 P3 | 优化 | 性能、代码质量 |
| ✅ 正常 | 无需处理 | 现有实现满足需求 |
