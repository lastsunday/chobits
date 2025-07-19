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
tar xvf kokoro-multi-lang-v1_1.tar.bz2
rm -rf kokoro-multi-lang-v1_1.tar.bz2
cd ..
mkdir -p vad
cd vad || exit
wget wget https://github.com/snakers4/silero-vad/raw/refs/tags/v5.1.2/src/silero_vad/data/silero_vad.onnx
cd ..
