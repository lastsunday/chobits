# Server

## File structure

各模块位置与 AGENTS.md 目录树一致。`api/src/` 下按功能分模块（asr, auth, llm, tts, vad, ws, record, mcp, matrix 等），`service/src/` 为业务逻辑层，`entity/src/` 为 Sea-ORM Entity，`migration/src/` 为数据库迁移，`web/src/` 为静态文件服务。

## Data flow

参考 [Development](../development/README.md) 中的 Chat Flow 章节。

## Model

### LLM

| Model                        | Memory | File Size     | Remark                 |
| ---------------------------- | ------ | ------------- | ---------------------- |
| Qwen3-0.6B (Candle-GGUF)     | ~1GB   | 0.6B GGUF     | Qwen3-0.6B-Q4_K_M.gguf |
| Qwen3-1.7B (Candle-GGUF)     | ~2.5GB | 1.11GB        | Qwen3-1.7B-Q4_K_M.gguf |
| Echo                         | 0      | 0             | 回显 (测试用)          |

### ASR

| Model                                    | Memory  | File Size | Language              | CER (TTS 闭环) |
| ---------------------------------------- | ------- | --------- | --------------------- | -------------- |
| SenseVoice (sherpa-onnx)                 | ~600MB  | 228MB     | 中/英/日/韩/粤        | 0.00%(A)       |
| Void                                     | 0       | 0         | —                     | — (测试用)     |

### TTS

| Model                        | Memory   | File Size                     | Remark             |
| ---------------------------- | -------- | ----------------------------- | ------------------ |
| MatchaTts (sherpa-onnx)      | ~500MB   | 72MB + 76MB (vocoder)         | 中文/中英双语      |
| Mute                         | 0        | 0                             | 静音 (测试用)      |

### VAD

| Model                        | Memory   | File Size | Remark                 |
| ---------------------------- | -------- | --------- | ---------------------- |
| Earshot (Silero VAD)         | ~10MB    | 内嵌       | 纯 Rust 实现，无 ONNX  |
| Void                         | 0        | 0         | 固定返回有声 (测试用)  |

## CUDA Toolkit install in fedora 43 & setup env

```shell
sudo sh cuda_12.8.1_570.124.06_linux.run --toolkit --no-drm --silent --override
```

````.zshrc
export PATH="/usr/local/cuda/bin:$PATH"
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:/usr/local/cuda/lib64
export LIBRARY_PATH=$LIBRARY_PATH:/usr/local/cuda/lib64

``` shell
conda create -n cuda
# for candle library
conda install conda-forge::gcc==14.3.0
conda install conda-forge::gxx==14.3.0
# for openssl library
conda install anaconda::openssl
conda activate cuda
# develop...
````

## Spec

<https://rust-lang.github.io/api-guidelines/>

<https://rust-coding-guidelines.github.io/rust-coding-guidelines-zh/overview.html>
