+++
title = "分层与组合"
weight = 20
+++

# 分层与组合

## 分层

`iot-core(纯逻辑) → iot-chip-esp(esp 家族运行时) → iot-bsp-esp(每板接线) → iot-app(单任务二进制)`

- **接线只出现在 `bsp/` 板模块**：`Board::new(Peripherals)` 内固定引脚；业务代码禁止出现引脚号
- **iot-chip-esp**：芯片初始化/日志/RTOS/panic 定义；应用层入口宏 `#[esp_rtos::main]` 留在 app `main.rs`（esp-hal/esp-rtos 在 app 仅 feature 门控供入口解析）
- **iot-bsp-esp**：板差异收敛在 `type Board` 别名 + run 分发；每板一个 `#[cfg(feature)]` 分支
- **iot-app**：应用层，产品名 `vanling`；业务零芯片依赖；flash/RAM 容量走链接与分区，不进类型系统
- **命名约定**：板层用板名、应用层用产品名 `vanling`，不重复
- **main 入口**：按家族 feature 分支、每家族一份（esp 家族别名 `esp32c6`）

## 组合（三轴正交）

硬件（板 feature）⊥ 模块（模块 feature）⊥ 能力（`iot-core` trait）。模块只见能力、板只供能力。

- **组合点 = bin 板清单**：能力 move 注入、对能力泛型、同能力板共享一份；启用缺能力的模块 = 编译错，不静默
- **产品档 = feature 别名**：档定模块集、板定支持集，交点编译期裁剪校验
- **模块可配置** = feature 开关 + const 参数
- **渲染层运行时可插拔**：`iot-core` 零分配中央 `RenderController`（reconcile 闭包回调）+ `iot-app` 堆上 `Vec<Box<dyn Renderer>>` 注册表（`embedded-alloc` 32KB 堆，boot LED in-task 注册，跨任务 Web/Audio renderer 走 `RENDER_BUS` + `Send`）

## 日志

只用 `log` façade（`log::info!` 等）；输出通道由家族 `iot-chip-esp` 初始化（esp 为 `esp_println::logger`）；业务代码禁止 `println!` / `esp_println::println!`

## 新增板

1. `bsp-esp/` 增板模块（Board + HasXxx traits）+ feature 门控
2. `app/src/main.rs` 加 `type Board`/run 分发 cfg 分支
3. app feature 透传 `iot-bsp-esp/<board>`
4. CI matrix 加一行
5. 验证：`cargo build -p iot-app --bin vanling --target riscv32imac-unknown-none-elf --no-default-features --features <board>`

## 新增芯片家族

esp 家族仓固定为 `iot-chip-esp`/`iot-bsp-esp`；新增非 esp 家族：

1. 新增 `iot-chip-<family>`/`iot-bsp-<family>` crate
2. app 家族别名 feature 门控该家族 entry deps；`main` 加家族入口块（`#[cfg(feature)]`）
3. 工具链目标 + `rust-toolchain.toml` + CI matrix 行
4. `moon`/`lefthook` 的 `--workspace --target` 改按固件作用域
5. 验证