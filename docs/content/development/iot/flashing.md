+++
title = "固件安装"
weight = 30
+++

# 固件安装

本页说明如何将 vanling 固件安装到开发板上。也有[浏览器一键安装入口](../../../flasher/index.html)可用。

## 前置条件

- 一块受支持的开发板（当前为 esp32c6）
- 一个固件产物。发布产物（含 `merged.bin`）来自 CD release（tag `vanling-iot@x.y.z`）；本地则用：
  ```sh
  # 在 apps/iot 下
  moon run build
  espflash save-image --chip esp32c6 --merge \
    --flash-size 4mb --flash-mode dio --flash-freq 40mhz \
    target/riscv32imac-unknown-none-elf/debug/vanling \
    vanling-merged.bin
  ```

## 单一设备安装

### 1. 浏览器直刷（推荐）

打开 [Vanling 固件安装器](../../../flasher/index.html)（Chrome / Edge 桌面版）：
1. 拖入 `merged.bin`
2. 检查解析出的目标芯片与版本信息
3. USB 连接开发板
4. 点「连接设备」，确认芯片比对通过后点「写入固件」

浏览器直刷基于 WebSerial 与 esptool-js，无需安装任何本地工具。

### 2. ESP Flash Download Tool（GUI）

- Windows：Espressif 官方 [ESP Flash Download Tool](https://www.espressif.com/en/support/download/other-tools)
- 地址填写与 `save-image --merge` 相同：整片镜像写 `0x0`
- 选择合适 SPI 参数（4MB / DIO / 40MHz）

### 3. esptool 命令行

```sh
python -m esptool --chip esp32c6 write_flash 0x0 vanling-merged.bin
```

`merged.bin` 已是整片镜像，只需写 `0x0` 一个地址。

## 批量安装

- 逐台并行运行 esptool（同一 PC 多 USB 口）：
  ```sh
  for p in /dev/ttyUSB{0..3}; do
    python -m esptool --port "$p" --baud 921600 write_flash 0x0 vanling-merged.bin &
  done; wait
  ```
- 工厂模式：烧录器支持 CRC32/数据校验，可接入治具流水线；esptool 自带 `mass_mfg`（只读工厂镜像批量烧录模板）可配置 MAC/序列号变量。

## 调试用（开发者）

```sh
# apps/iot 下直接编译并烧录（espflash 在 devShell 提供）
moon run flash
# 或手动
cargo build && espflash flash target/.../vanling
espflash monitor   # console 内带回车重启
```

## 固件构成

发布产物的 `merged.bin`（`save-image --merge`）为整片 Flash 镜像：

| 段        | 地址       | 说明                           |
| --------- | ---------- | ------------------------------ |
| bootloader| `0x0`      | 二级引导程序，固定脚本产物     |
| partition | `0x8000`   | 分区表                         |
| app       | `0x10000`  | vanling 应用 + 数据            |

> 布局以产出 bin 时实际分区表为准；安装时整片写 `0x0` 即可，不必单个段分别写。

## 浏览器安装原理

安装器页面是完全静态的（托管于 GitHub Pages）。`docs/static/flasher/` 内含：

- `index.html` + `main.js`：自解析镜像头（magic `0xE9`、chip ID、flash 参数、段表、XOR 交叉校验、app 描述符 `0xABCD5432`）。
- `vendor/esptool-js@0.6.0.bundle.js`：Espressif 官方 WebSerial 烧录内核（Apache-2.0）。

不受 OTA（联网升级）影响：安装器解决首次刷入与整机还原，OTA 解决后续增量升级。