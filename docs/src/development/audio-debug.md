# Audio Debugging Journey

## Problem

实时 WebSocket TTS 音频播放：开始正常，几秒后出现明显**卡顿**和**电流声**。

## Investigation

### 1. Validate server output

- Session 集成测试（`test_tts_audio_collect` / `test_tts_vits`）产生 WAV 文件，播放干净
- **结论：服务端音频管线（TTS → resample → Opus → WebSocket）无质量问题**

### 2. Rule out each tunable

| 改动 | 效果 |
|------|------|
| `opus-rs` → `opus` crate (C binding) | 无效 |
| 输出采样率 16000 → 24000Hz | 无效 |
| `sherpa-onnx::LinearResampler` → `rubato::FftFixedIn` | 无效 |
| `Application::LowDelay` → `Application::Audio` + FEC | 无效 |
| `Signal::Auto` → `Signal::Voice` + `Bandwidth::Superwideband` | 无效 |
| VBR → CBR (`set_vbr(false)`) | 无效 |
| 旧 pacing: elapsed 相对时间 → 绝对目标时间 | 无效 |

### 3. Client code analysis

查阅 `apps/server-ui/public/test/device/js/` 下的客户端代码：

**Opus 解码（主线程同步 WASM 调用）：**

```javascript
// player.js
decode: function (opusData) {
    const decodedSamples = mod._opus_decode(    // ← 同步 WASM
        this.decoderPtr, opusPtr, opusData.length,
        pcmPtr, this.frameSize, 0
    );
    // 阻塞主线程直到解码完成
}
```

**Web Audio API 链式调度（每段 120ms）：**

```javascript
// stream-context.js
const startTime = Math.max(this.scheduledEndTime, currentTime);
this.source.start(startTime);
this.scheduledEndTime = startTime + audioBuffer.duration;
```

**批量取帧（内层循环最多 99 包）：**

```javascript
// player.js
const data = await this.queue.dequeue(99, 30);
// 99 包一次喂入解码循环
```

## Root Cause

### 时序线

```
TTS 模型推理 (~13s)           TTS::Start          AudioResult × N
│────────────────────────────┤─────────────────────►
                             │
audio_start_time = T=0       │
                             │   (13 秒后所有帧到达 OutputController)
                             │
Frame 228:                   │
  paced_index = 228 - 10     │
  target = 0 + 218 × 60ms    │
         = 13080ms ≈ 13s     │
  now = 13s + ϵ > target     │
  → sleep(0) → 立即发送      │
                             ▼
                    228 帧爆发发送 (<10ms)
```

### 具体机制

1. **VITS 模型**对完整句子生成连续 PCM（~13 秒推理时间）
2. PCM 被分割为 228 帧（60ms/帧），经 mpsc channel 同时到达 `OutputController`
3. **旧 pacing 逻辑**以 `audio_start_time`（TTS::Start 时刻）为基准做绝对目标计算：
   ```
   target = audio_start_time + paced_index × 60ms
   ```
   所有目标都在 13 秒前 → `now > target` → 永远不 sleep → **228 帧 <10ms 内爆发发送**
4. **客户端**瞬间收到 200+ Opus 包 → 解码循环单次批量处理全部
5. WASM 中 `_opus_decode` 是**同步调用**，阻塞 JS 主线程 >1 秒
6. 主线程被阻塞期间，Web Audio API 无法创建下一段 `AudioBufferSourceNode`
7. `scheduledEndTime` 已过 → 链式调度断裂 → 产生可听中断和噪声

### 为什么不影响离线 WAV？

- 离线测试（`test_tts_audio_collect`）解码全部 Opus 帧为连续 PCM 后写入文件
- 没有实时调度约束，WASM 解码阻塞不产生听觉影响
- 离线 WAV = 所有帧无缝拼接 → 完全干净

## Fix

### 方案

用 `tokio::time::interval_at` + `MissedTickBehavior::Skip` **惰性创建**，首帧立即发，后续帧从当前时刻起严格 20ms 间距。

### 代码

```rust
// output_controller.rs — 关键改动

use tokio::time::{Duration, Instant, MissedTickBehavior, interval_at};

pub struct OutputController {
    interval: Option<tokio::time::Interval>,
    frame_duration: u64,
    // ...
}

impl OutputController {
    async fn pace_audio(&mut self) {
        if let Some(interval) = &mut self.interval {
            interval.tick().await;  // 等 20ms → 发送 → 再等 20ms → 发送...
        } else {
            let start = Instant::now() + Duration::from_millis(self.frame_duration);
            let mut intv = interval_at(start, Duration::from_millis(self.frame_duration));
            intv.set_missed_tick_behavior(MissedTickBehavior::Skip);
            self.interval = Some(intv); // 首帧立即发，interval 从下一帧起算
        }
    }
}
```

### 改进后时序

```
TTS 推理 (~13s)           TTS::Start     AudioResult × N
│────────────────────────┤───────────────►
                         │
Frame 1:                 │
  interval = None        │
  → 创建 interval(now+20ms, 20ms)  ← 不 tick，立即发送
                         │
Frame 2:                 │
  interval.tick().await  │  → 等 20ms → 发送
Frame 3:                 │
  interval.tick().await  │  → 等 20ms → 发送
Frame 4:                 │
  interval.tick().await  │  → 等 20ms → 发送
  ...                    │  严格 20ms/帧
                         ▼
                    客户端稳定播放
```

### 效果对比

| 指标 | 旧方案 | 新方案 |
|------|--------|--------|
| 初始突发 | 228 帧 (<10ms) | 1 帧（首帧立即发）|
| 稳态间隔 | 无（全部爆发） | 严格 20ms/帧 |
| 客户端批解码 | 200+ 帧/批 | 1 帧/批 |
| 主线程阻塞 | >1 秒 | <1ms |
| Web Audio API | 调度断裂 | 连续调度 |

## Key Files

| 文件 | 作用 |
|------|------|
| `apps/server/api/src/ws/session/output_controller.rs` | 帧发送 pacing 逻辑（修复位置） |
| `apps/server-ui/public/test/device/js/core/audio/player.js` | 客户端 Opus 解码 + 批量取帧 |
| `apps/server-ui/public/test/device/js/core/audio/stream-context.js` | Web Audio API 链式调度 |
| `apps/server/api/src/tts/model/vits/mod.rs` | VITS 模型音频生成 |
| `apps/server/api/src/ws/mod.rs` | WebSocket 帧发送 |
| `apps/server/api/src/tts/mod.rs` | `encode_sample_to_tts_packet` 帧分割编码 |
