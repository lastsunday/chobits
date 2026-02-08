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

| Model                         | Memory | File Size | Remark |
| ----------------------------- | ------ | --------- | ------ |
| openai/whisper-tiny           | 0.45GB | 0.15GB    |        |
| openai/whisper-small          | 1.1GB  | 0.96GB    |        |
| Qwen/Qwen3-ASR-0.6B           | 2GB    | 1.88GB    |        |
| openai/whisper-large-v3-turbo | 4GB    | 1.62GB    |        |

### TTS

| Model               | Memory | File Size | Remark |
| ------------------- | ------ | --------- | ------ |
| mzdk100/kokoro      | 0.12GB | 0.37GB    |        |
| openbmb/VoxCPM-0.5B | 2GB    | 1.61GB    |        |

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
