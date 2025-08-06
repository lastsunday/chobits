<p align="center">
    <img width="180" src="docs\logo.svg" alt="chobits">
</p>
<h1 align="center">Ch❤️‍🩹bits</h1>

[![build-server](https://github.com/lastsunday/chobits/actions/workflows/build-server.yml/badge.svg)](https://github.com/lastsunday/chobits/actions/workflows/build-server.yml)

<p align="left">
  <a href="https://github.com/lastsunday/chobits/releases">
    <img alt="GitHub Release" src="https://img.shields.io/github/v/release/lastsunday/chobits?logo=docker" />
  </a>
  <a href="https://github.com/lastsunday/chobits/graphs/contributors">
    <img alt="GitHub Contributors" src="https://img.shields.io/github/contributors/lastsunday/chobits?logo=github" />
  </a>
  <a href="https://github.com/lastsunday/chobits/issues">
    <img alt="Issues" src="https://img.shields.io/github/issues/lastsunday/chobits?color=0088ff" />
  </a>
  <a href="https://github.com/lastsunday/chobits/pulls">
    <img alt="GitHub Pull Requests" src="https://img.shields.io/github/issues-pr/lastsunday/chobits?color=0088ff" />
  </a>
  <a href="https://github.com/lastsunday/chobits/blob/main/LICENSE">
    <img alt="GitHub License" src="https://img.shields.io/badge/license-MIT-white?labelColor=black" />
  </a>
  <a href="https://github.com/lastsunday/chobits">
    <img alt="Stars" src="https://img.shields.io/github/stars/lastsunday/chobits?color=ffcb47&labelColor=black" />
  </a>
</p>

<details open>
<summary>Technology stack</summary>

1. Rust
   1. Axum
   2. sea-orm
2. Reactjs
3. Database
   1. Postgres
   2. Sqlite

</details>

<details>
<summary>Development</summary>

- apps/server-ui

```shell
pnpm exec nx run @chobits/server-ui:dev
```

- apps/server

```shell
pnpm exec nx run @chobits/server-ui:build
./apps/server/script/download_model.sh
pnpm nx run chobits-server:run
```

</details>

<details>
<summary>Feature</summary>

### Server

1. Api
   1. Auth
      - [x] HTTP
        - [x] JWT
      - [ ] WebSocket
        - [ ] JWT
   2. Ota
      - [x] Ws Url
2. Audio
   - [x] TTS(语音合成)
   - [x] VAD(语音活动检测)
   - [x] ASR(语音识别)
   - [x] 多语言识别
   - [ ] 语音处理
   - [ ] 声纹
3. Video
4. 大模型
   - [x] 智能对话
   - [ ] 视觉感知
   - [ ] 意图识别
     - [ ] Function Call 函数调用
   - [ ] 记忆系统
5. MCP

### Server UI

1. Other
   - [ ] Home
   - [x] Login
   - [x] Reset password

### Other

1. Api
   - [x] Api docs
2. Database
   - [x] Migration
3. I18n
   - [x] Server
   - [x] Server UI
4. Config
   - [x] Server
   - [x] Server UI
5. Logger
   - [ ] File log
6. Cicd
   - [ ] Github Action
     - [x] test
     - [x] build
     - [x] release
   - [x] Docker(server + server-ui)
   - [x] Bin(server + server-ui)
7. Testing
   - [ ] Unit Test
   - [ ] BDD
   - [ ] E2E
   </details>

## Support Component

| 模块名称         | 组件                  |
| ---------------- | --------------------- |
| ASR(语音识别)    | sherpa-rs(SenseVoice) |
| LLM(大模型)      | candle(Qwen3)         |
| VLLM(视觉大模型) |                       |
| TTS(语音合成)    | sherpa-rs(Kokoro)     |
| Intent(意图识别) |                       |
| Memory(记忆功能) |                       |

## Thanks

<https://github.com/78/xiaozhi-esp32>

<https://github.com/xinnan-tech/xiaozhi-esp32-server>

<https://github.com/joey-zhou/xiaozhi-esp32-server-java>
