+++
title = "页面实现"
weight = 303
+++

# 页面实现

本文描述客户端（Flutter App / esp32c6 固件）如何实现[共享客户端协议](shared-protocol.md) §3 的页面模型与输出通道。协议只约束"pages 共享名词、chat 页协议驱动、状态为 reducer 投影"；其余实现细节各端自决。

## 1. 四层架构

```
① SessionService —— WS + reducer，产出唯一事实源 ChatProjection{ state, messages, subtitle, emotion }
② FeedbackHub   —— 订阅①，驱动 Led/Audio/Vib/Web 通道（横切投影）
③ DisplayStack  —— 页管理 + status bar + overlay（显示通道）
④ 渲染器        —— 每页一个，纯消费（Flutter widget / esp32c6 帧缓冲）
```

②③都是①的订阅者：chat 页与 LED/耳音不构成竞争，而是同一投影的并列通道，天然同步。

## 2. Page 描述子

每页 = 4 元组：

- `id`：§3 页面表名词（chat / home / config / ota / settings）
- content model：自己管理自己的数据源
- input handler：接受哪些输入（见 §4 输入三路路由）
- renderer：数据 → 画面

## 3. 显示栈规则

页管理器的职责仅有三条：

- **前台唯一、可切换**：任一时刻恰有一个前台页，支持返回/切换栈
- **分层**：status bar（网络/电量/时钟）恒常绘制，位于页之下；overlay（通知/告警）瞬时压顶，位于页之上，但**不切换前台页**
- **输入分发**：按 §4 三路路由把输入送到对应去处

## 4. 输入三路路由

| 输入 | 去处 | 例子 |
| --- | --- | --- |
| 会话语义 | reducer（①） | 唤醒/对讲/打断/文本 → `listen_*`/`abort`/`text` |
| 页面交互 | 前台页（③） | 配置表单/设置 toggle/返回 |
| 全局/隐私 | chrome 层 | mic off、DND（不属状态机，独立常驻项） |

## 5. chat 页（关键差异）

- 数据源 = **订阅 `ChatProjection`**，不读原始帧——帧已在 SessionService 上游折叠成投影，页面只响应投影变更重渲
- 渲染 = `f(projection)`，幂等、无闪烁（§3 幂等 no-op 的兑现点）
- 仅语音外设编译时挂载；无 voice 设备 → 无 reducer、无 chat 页，其余页照常

## 6. 非 chat 页

| Page | 数据源 | 更新方式 |
| --- | --- | --- |
| home | 遥测快照（HTTP 状态仓库 / MCP 直查） | 本地轮询/推送，读→渲染 |
| config | 本地表单模型 + HTTP 下发 | 本地事件/按钮 |
| ota | OTA 驱动事件 | 进度→渲染 |
| settings | 本地持久化配置 | toggle/按钮 |

## 7. 能力门

- 有 `voice` 外设：①含 reducer + chat 页；②反馈 hub 全量
- 无 `voice` 外设：①退化为纯命令面，③剩 4 页 + status bar，②仅 Led/Web 等可选通道
- esp32c6 无 OS：页管理 = "id + dirty flag"，overlay/status bar 为绘制层叠加

## 8. 示例：语音回合全链路

esp32c6：OLED + 三色 LED + 扬声器 + Action 键。

1. **按 Action 键** → 本地事件 `listen_start(manual)` 进 reducer：`idle→listening`。投影变更广播到全部通道：chat 页显示"聆听中…"并清空上条消息；LED 亮 `mic_live` 红（采集音频中，隐私底线）；扬声器播本地耳音"滴"；Web 通道同步推新状态。
2. **说话** → WS 上行音频经 server pipeline 处理，`stt`（partial）帧返回 → reducer 就地更新 user 消息 `{is_interim:true}`，chat 页滚动跟随、灰字显示。
3. **结果返回** → `llm` 帧 → reducer：`listening→thinking`（MIC 停采，LED 转蓝）；助手流式文本就地更新 user/assistant 消息，`emotion` 随帧覆盖。
4. **`tts.start`** → reducer：`thinking→speaking`。chat 页逐句落 `subtitle{text}`；LED 常亮白（与 idle 对比基准）；TTS 声音为**产品内容输出**，与反馈耳音分层。
5. **`tts.stop`** → reducer：`speaking→idle`。chat 页保留最后消息回 idle 态；LED 熄灭（idle = 全通道对比基准）。
6. **全程** status bar 常驻（网络/电量/时钟）；期间 OTA 完成后 `overlay`"固件可升级"压顶 5s 自动消失，前台 chat 页不受影响。

> chat 页只响应 `ChatProjection` 变更，不解析帧；LED/Audio/Web 与其共享同一投影，故天然同步。