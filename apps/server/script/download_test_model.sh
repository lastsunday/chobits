#!/bin/bash
mkdir -p data
cd data || exit
mkdir -p asr
cd asr || exit
#whisper-tiny
wget https://huggingface.co/openai/whisper-tiny/resolve/169d4a4341b33bc18d8881c4b69c2e104e1cc0af/model.safetensors -O model.safetensors
wget https://huggingface.co/openai/whisper-tiny/resolve/169d4a4341b33bc18d8881c4b69c2e104e1cc0af/tokenizer.json -O tokenizer.json
wget https://huggingface.co/openai/whisper-tiny/resolve/169d4a4341b33bc18d8881c4b69c2e104e1cc0af/config.json -O config.json
cd ..
mkdir -p tts
cd tts || exit
wget https://github.com/mzdk100/kokoro/releases/download/V1.1/kokoro-v1.1-zh.onnx -O model.onnx
wget https://github.com/mzdk100/kokoro/releases/download/V1.1/voices-v1.1-zh.bin -O voice.bin
cd ..
mkdir -p vad
cd vad || exit
wget https://huggingface.co/onnx-community/silero-vad/resolve/ddc9a7e80d6758f6fc795a1e8a04b798eb929d3a/onnx/model.onnx -O model.onnx
cd ..
mkdir -p llm || exit
# 0.6B
# wget --directory-prefix=llm/unsloth/Qwen3-0.6B-GGUF/ https://huggingface.co/Qwen/Qwen3-0.6B/resolve/167b8104f88905a951069f5f95f9776908da5f68/tokenizer.json
# wget --directory-prefix=llm/unsloth/Qwen3-0.6B-GGUF/ https://huggingface.co/unsloth/Qwen3-0.6B-GGUF/resolve/058b74ede71731e4a323a88d68da1386519ec6fc/Qwen3-0.6B-Q4_K_M.gguf
# 1.7B
wget --directory-prefix=llm/unsloth/Qwen3-1.7B-GGUF/ https://huggingface.co/Qwen/Qwen3-1.7B/resolve/70d244cc86ccca08cf5af4e1e306ecf908b1ad5e/tokenizer.json
wget --directory-prefix=llm/unsloth/Qwen3-1.7B-GGUF/ https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/d7f544eead698dbd1f15126ef60b45a1e1933222/Qwen3-1.7B-Q4_K_M.gguf
cd ..
cd ..
