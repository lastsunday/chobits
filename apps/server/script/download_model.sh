#!/bin/bash
mkdir -p data
cd data || exit
mkdir -p asr
cd asr || exit
wget https://huggingface.co/openai/whisper-large-v3-turbo/resolve/41f01f3fe87f28c78e2fbf8b568835947dd65ed9/model.safetensors -O model.safetensors
wget https://huggingface.co/openai/whisper-large-v3-turbo/resolve/41f01f3fe87f28c78e2fbf8b568835947dd65ed9/tokenizer.json -O tokenizer.json
wget https://huggingface.co/openai/whisper-large-v3-turbo/resolve/41f01f3fe87f28c78e2fbf8b568835947dd65ed9/config.json -O config.json
cd ..
mkdir -p tts
# mkdir -p tts/model/mzdk100/kokoro
# wget https://github.com/mzdk100/kokoro/releases/download/V1.1/kokoro-v1.1-zh.onnx -O tts/model/mzdk100/kokoro/model.onnx
# wget https://github.com/mzdk100/kokoro/releases/download/V1.1/voices-v1.1-zh.bin -O tts/model/mzdk100/kokoro/voice.bin
mkdir -p tts/model/openbmb/VoxCPM-0.5B
wget https://huggingface.co/openbmb/VoxCPM-0.5B/resolve/f67d35a3848e0bec0fdb8c33e6fc92cf293ee72f/config.json -O tts/model/openbmb/VoxCPM-0.5B/config.json
wget https://huggingface.co/openbmb/VoxCPM-0.5B/resolve/f67d35a3848e0bec0fdb8c33e6fc92cf293ee72f/pytorch_model.bin -O tts/model/openbmb/VoxCPM-0.5B/pytorch_model.bin
wget https://huggingface.co/openbmb/VoxCPM-0.5B/resolve/f67d35a3848e0bec0fdb8c33e6fc92cf293ee72f/tokenizer.json -O tts/model/openbmb/VoxCPM-0.5B/tokenizer.json
wget https://huggingface.co/openbmb/VoxCPM-0.5B/resolve/f67d35a3848e0bec0fdb8c33e6fc92cf293ee72f/audiovae.pth -O tts/model/openbmb/VoxCPM-0.5B/audiovae.pth
wget https://github.com/jhqxxx/aha/blob/b3c6219e93e244d8f07a07066a798b4385a46631/assets/audio/voice_05.wav -O tts/reference/voice_05.wav
cd ..
mkdir -p vad
cd vad || exit
wget https://huggingface.co/onnx-community/silero-vad/resolve/ddc9a7e80d6758f6fc795a1e8a04b798eb929d3a/onnx/model.onnx -O model.onnx
cd ..
mkdir -p llm || exit
# 0.6B
# mkdir -p llm/model/unsloth/Qwen3-0.6B-GGUF
# wget https://huggingface.co/Qwen/Qwen3-0.6B/resolve/167b8104f88905a951069f5f95f9776908da5f68/tokenizer.json -O llm/model/unsloth/Qwen3-0.6B-GGUF/tokenizer.json
# wget https://huggingface.co/unsloth/Qwen3-0.6B-GGUF/resolve/058b74ede71731e4a323a88d68da1386519ec6fc/Qwen3-0.6B-Q4_K_M.gguf -O llm/model/unsloth/Qwen3-0.6B-GGUF/model.gguf
# 1.7B
mkdir -p llm/model/unsloth/Qwen3-1.7B-GGUF
wget https://huggingface.co/Qwen/Qwen3-1.7B/resolve/70d244cc86ccca08cf5af4e1e306ecf908b1ad5e/tokenizer.json -O llm/model/unsloth/Qwen3-1.7B-GGUF/tokenizer.json
wget https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/d7f544eead698dbd1f15126ef60b45a1e1933222/Qwen3-1.7B-Q4_K_M.gguf -O llm/model/unsloth/Qwen3-1.7B-GGUF/model.gguf
cd ..
cd ..
