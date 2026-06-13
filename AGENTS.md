# AGENTS.md

## 索引

- [1. 技术栈](#1-技术栈)
- [2. 项目结构与命名](#2-项目结构与命名)
- [3. 核心禁忌](#3-核心禁忌)
- [4. 开发工作流](#4-开发工作流)
  - [4.1 开发环境设置（Lix）](#41-开发环境设置lix)
- [5. 构建与 CI 调试](#5-构建与-ci-调试)
- [附录 架构概述](#附录-架构概述)

## 1. 技术栈

### Rust (apps/server)

- **Edition 2024** - 使用 RPIT 生命周期捕获规则等新语法
- **Web**: Axum 0.8 + tower-http + utoipa-axum (Scalar OpenAPI)
- **ORM**: Sea-ORM (sqlx-postgres, sqlx-sqlite, with-chrono, with-rust_decimal)
- **Auth**: jsonwebtoken (HS256, access + refresh token) + bcrypt
- **Config**: config crate (YAML) + figment (TOML)
- **ID**: xid (XID 格式)
- **DB**: SQLite (默认) / PostgreSQL (可选)
- **Error**: `thiserror` + `#[implement]` 宏
- **Testing**: cucumber (BDD) + testcontainers
- **ML**: Candle (candle-core, candle-transformers, candle-onnx, candle-nn)
- **Audio**: symphonia, wavers, resampler, opus
- **ASR**: qwen-asr
- **TTS**: kokoro-tts
- **VAD**: earshot (Silero VAD)
- **LLM**: rig-core, aha (GGUF 推理)
- **MCP**: rmcp (Model Context Protocol, 支持 SSE 与 Streamable HTTP)
- **Matrix**: ruma + ruma-client (聊天协议)
- **开发环境**: Nix (Lix) + flake + direnv（可复现开发环境）

### TypeScript

**管理后台 (apps/server-ui):**

- React 19 + Mantine v8 (@mantine/core, @mantine/hooks, @mantine/notifications)
- TanStack Router (文件路由, auto code-splitting) + TanStack Query (React Query)
- Axios + zod
- i18next + react-i18next
- UnoCSS
- Vite
- Prettier: `{ singleQuote: true }`

### Flutter (apps/app)

- 跨平台客户端应用 (WIP)
- Flutter SDK 版本管理: `.fvmrc`

## 2. 项目结构与命名

### 目录结构

```
├── docs/                      mdBook 项目文档
├── flake.nix                  Nix flake 配置
├── flake.lock                 Nix flake lock
├── .envrc                     direnv 自动激活（use flake）
├── rust-toolchain.toml        Rust 版本（由 flake 自动读取）
├── .node-version              Node.js 版本（由 flake 自动读取）
├── application-example.toml   示例配置文件
├── packages/                  workspace 占位
├── libs/                      workspace 占位
├── apps/
│   ├── server/                Rust 后端
│   │   ├── src/               应用入口 (main.rs, clap, logging, runtime, server, signal, restart)
│   │   ├── api/src/           API 路由层 + AI 模块 (ws, llm, tts, asr, vad, auth, ota, matrix, mcp 等)
│   │   ├── service/src/       业务逻辑层 (chobits/)
│   │   ├── entity/src/        Sea-ORM Entity
│   │   ├── migration/src/     数据库迁移
│   │   ├── web/src/           Web 层 (rust-embed 静态文件服务)
│   │   ├── framework/         框架层 (auth, config, data, error, logger, middleware, password 等)
│   │   ├── macros/            proc-macro crate (`#[implement]`, `#[config_example_generator]`)
│   │   └── build-metadata/    构建元数据 crate (git 信息)
│   ├── server-ui/src/         React 管理后台
│   ├── server-ui-e2e/         管理后台 E2E 测试 (Playwright)
│   └── app/                   Flutter 跨平台客户端应用
```

新建文件请严格遵循对应的目录位置。

### 命名风格

| 语言/层         | 命名风格                  | 示例                                        |
| --------------- | ------------------------- | ------------------------------------------- |
| Rust 变量/函数  | snake_case                | `create_routes`, `start_app`                |
| Rust 类型/Trait | PascalCase                | `AppState`, `TtsFactory`                    |
| Rust 模块名     | snake_case                | `ws.rs`, `llm.rs`                           |
| TS 变量/函数    | camelCase                 | `loadJobs`, `handleSubmit`                  |
| TS 组件/类      | PascalCase                | `RouteComponent`                            |
| TS 组件文件     | PascalCase + .tsx         |                                             |
| TS 非组件文件   | camelCase + .ts           | `http.ts`                                   |
| DB 表/字段      | snake_case                | `user`, `config`                            |
| URI             | snake_case                | `/api/auth/access_token`                    |

## 3. 核心禁忌

### 绝对不能做的

- **不要**引入新依赖前未检查现有依赖是否已满足需求
- **不要**使用非 Edition 2024 的 Rust 语法（如 `'_` 生命周期 elision 规则、`impl<T>` 旧式 trait bound 等）
- **不要**手动编辑 `flake.lock`（使用 `nix flake update` 更新）

### 必须遵守的

- **提交信息**: 必须使用 Conventional Commits 格式（`feat:` / `fix:` / `perf:` / `remove:` / `deprecate:` / `security:`）。Lefthook commit-msg hook 自动校验格式，不满足会被拒绝。如需跳过用 `git commit --no-verify`。如需在 changelog 中展示详细说明，在 footer 中写入 `CHANGELOG: <description>`。破坏性变更使用 `feat!:` 或 `BREAKING CHANGE:` footer
- **CHANGELOG 更新**: 发布前运行 `moon run <project>:bump` 自动版本升级、生成 CHANGELOG、commit 并 tag。底层调用 `scripts/bump.sh`
- Rust: 不要遗漏 `use framework::lib` 等模块导入
- Rust: 路由必须在 `create_router` 中通过 `OpenApiRouter` 组织
- Rust: 提交前运行 `cargo fmt && cargo clippy` 保持代码风格
- AI 模块 (LLM/TTS/VAD/ASR) 使用 Factory 模式，通过 `XxxFactory::init()` 初始化，通过 `XxxFactory::get()` 获取实例

## 4. 开发工作流

### 4.1 开发环境设置（Lix）

项目使用 **Lix**（Nix 的社区 fork）管理可复现的开发环境。

**首次设置：**

```bash
# 1. 安装 Lix
curl -sSf -L https://install.lix.systems/lix | sh -s -- install

# 2. 安装 direnv（推荐，自动激活环境）
nix profile install nixpkgs#direnv nixpkgs#nix-direnv
# 在 ~/.zshrc 或 ~/.bashrc 添加: eval "$(direnv hook zsh)"

# 3. 进入项目（direnv 自动激活，或手动 nix develop）
cd chobits
direnv allow
```

**devShell 选择：**

```bash
nix develop .#server    # 仅 Rust 后端（rustToolchain + openssl + sqlite + postgresql）
nix develop .#frontend  # 仅前端（nodejs + pnpm）
nix develop             # 默认完整环境（含 moon、just、mdbook、pkg-config 等）
```

### Git Hook & 模板自动安装

`.envrc` 中已配置 `lefthook install` + `git config commit.template`，进入目录时自动生效。

### 新增业务逻辑时 (Rust)

1. `migration/src/`: 创建 SQL 迁移
2. `entity/src/`: 更新 Sea-ORM 实体
3. `api/src/`: 实现路由 handler
4. `service/src/`: 实现具体业务逻辑
5. 在 `api/src/lib.rs` 的 `create_router` 中注册路由
6. 运行 `cargo check && cargo test` 验证

### 新增 AI 模型支持时

1. `api/src/llm/`（或 `tts/`, `asr/`, `vad/`）: 实现对应 Trait
2. 在 `api/src/config/` 中添加模型配置项
3. 在 Factory 中注册模型创建逻辑
4. 更新 `application-example.toml`

### 新增前端页面时 (server-ui)

1. `routes/`: 按 TanStack Router 文件路由约定新建 `.tsx` 文件
2. `api/`: 添加对应的 API 调用函数
3. `components/`: 组件文件
4. 翻译文本添加到 `public/locales/{lang}/{namespace}.json`
5. 运行 `moon run server-ui:typecheck` 验证类型

## 5. 构建与 CI 调试

### 构建工具

- **Monorepo**: Moon (@moonrepo/cli)
- **配置**: `.moon/workspace.yml`, `.moon/toolchains.yml`
- **JS/TS 任务**: Moon 自动从 `package.json` scripts 推断
- **Rust 任务**: 在 `moon.yml` 中显式定义
- **常用命令**:
  - `moon run <project>:<task>` — 运行某项目的特定任务
  - `moon run :<task>` — 所有项目运行某任务
  - `moon ci --affected` — CI 中运行受影响项目的 pipeline
  - `moon query projects` — 列出所有项目
  - `moon query tasks` — 列出所有任务
- **工具链**: 由 Nix flake 统一管理（Node.js → `.node-version`，Rust → `rust-toolchain.toml`，moon CLI 内置于 `flake.nix`）

### Fast Dev Loop

- Rust 路由改动后运行 `cargo check` 验证类型，不用 `cargo run` 全量编译
- 启动服务: `pnpm nx run chobits-server:run`
- 启动前端: `pnpm exec nx run @chobits/server-ui:dev`
- 运行测试: `cargo test --package api`

## 附录 架构概述

### WebSocket 会话生命周期

```
Client → WS Connect → Auth(JWT) → Session Created
  ↓
[VAD: Voice Activity Detection] → 检测到语音
  ↓
[ASR: Automatic Speech Recognition] → 语音转文字
  ↓
[LLM: Large Language Model] → 对话推理
  ↓
[TTS: Text to Speech] → 文字转语音
  ↓
Client ← Audio Stream
```

### Rust 子 crate 依赖关系

```
build-metadata (编译时 git 信息)
  └── framework (框架层: auth, config, error, database, id, logger, middleware, password, trace)
       ├── macros (proc-macro: #[implement], #[config_example_generator])
       ├── entity (Sea-ORM 实体)
       ├── migration (数据库迁移)
       ├── web (rust-embed 静态文件)
       ├── service (业务逻辑)
       └── api (路由 + AI 模型管道)
            └── server (应用入口)
```

### 框架错误码

| 范围 | 说明 |
|------|------|
| 1xxxxx | 基础错误 (数据库等) |
| 2xxxxx | 第三方错误 (JWT, 密码等) |
| 3xxxxx | 框架错误 (验证, 请求格式等) |
| 4xxxxx | 严重错误 (内部错误, 资源未找到) |
| 5xxxxx | 业务模块错误 (用户, OTA 等) |
