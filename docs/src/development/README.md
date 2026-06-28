# 开发文档

Chobits 服务端和相关项目的开发指南。

## [服务端](./server/architecture.md)

服务端架构设计、业务数据流、协议参考、模型规格与部署。

- [核心架构](./server/architecture.md) — 会话状态机、并发模型、工厂模式
- [对话流程](./server/dialogue-flow.md) — 握手、通讯、Listen Mode、MCP 流程
- [WebSocket 协议](./server/websocket-protocol.md) — 协议字段参考
- [模型与部署](./server/models-and-deployment.md) — 模型规格、CUDA 安装、参考规范
- [待办事项](./server/TODO.md) — 待办事项

## [客户端](./clients/app.md)

客户端应用开发文档。

- [App（Flutter）](./clients/app.md)
- [管理后台（React）](./clients/server-ui.md)
- [ESP32](./clients/esp32.md)

## [调试](./debugging/vad-listener.md)

调试和诊断相关文档。

- [VAD 与 Listener](./debugging/vad-listener.md)
- [Audio 调试](./debugging/audio-debug.md)
- [ASR 调试](./debugging/asr-debug.md)

## [模型下载](./downloader.md)

模型下载工具的使用说明。

## 相关项目

生态相关项目文档。

- [xiaozhi-esp32](./related-project/xiaozhi-esp32.md)
- [xiaozhi-esp32-server](./related-project/xiaozhi-esp32-server.md)
- [xiaozhi-esp32-server-java](./related-project/xiaozhi-esp32-server-java.md)
- [xiaozhi-android-client](./related-project/xiaozhi-android-client.md)
