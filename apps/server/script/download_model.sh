#!/bin/bash
mkdir -p data
cd data || exit
mkdir -p asr
cd asr || exit
wget https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2
tar xvf sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2
rm -rf sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2
cd ..
mkdir -p tts
cd tts || exit
wget https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_1.tar.bz2
tar xvfj kokoro-multi-lang-v1_1.tar.bz2
rm -rf kokoro-multi-lang-v1_1.tar.bz2
cd ..
mkdir -p vad
cd vad || exit
wget https://github.com/snakers4/silero-vad/raw/refs/tags/v5.1.2/src/silero_vad/data/silero_vad.onnx
cd ..
mkdir -p llm || exit
# 0.6B
# wget https://huggingface.co/Qwen/Qwen3-0.6B/resolve/167b8104f88905a951069f5f95f9776908da5f68/tokenizer.json -O llm/tokenizer.json
# wget https://huggingface.co/unsloth/Qwen3-0.6B-GGUF/resolve/058b74ede71731e4a323a88d68da1386519ec6fc/Qwen3-0.6B-Q4_K_M.gguf -O llm/model.gguf
# 1.7B
wget https://huggingface.co/Qwen/Qwen3-1.7B/resolve/70d244cc86ccca08cf5af4e1e306ecf908b1ad5e/tokenizer.json -O llm/tokenizer.json
wget https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/d7f544eead698dbd1f15126ef60b45a1e1933222/Qwen3-1.7B-Q4_K_M.gguf -O llm/model.gguf
cd ..
cd ..
