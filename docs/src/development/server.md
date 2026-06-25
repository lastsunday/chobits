# Server

## File structure

_TODO_

## Data flow

**_TODO_**

## Model

### LLM

| Model                   | Memory | File Size | Remark                 |
| ----------------------- | ------ | --------- | ---------------------- |
| unsloth/Qwen3-1.7B-GGUF | 2.5GB  | 1.11GB    | Qwen3-1.7B-Q4_K_M.gguf |

### ASR

| Model                                    | Memory  | File Size | Language              | CER (TTS 闭环) |
| ---------------------------------------- | ------- | --------- | --------------------- | -------------- |
| SenseVoice (sherpa-onnx)                 | ~600MB  | 228MB     | 中/英/日/韩/粤        | 0.00%(A)       |

### TTS

| Model                        | Memory   | File Size | Remark             |
| ---------------------------- | -------- | --------- | ------------------ |
| MatchaTts (sherpa-onnx)      | ~500MB   | 72MB + 76MB (vocoder) | 中文/中英双语 |
| Mute                         | 0        | 0          | 静音 (测试用)    |

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
