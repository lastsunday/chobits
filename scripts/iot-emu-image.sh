#!/usr/bin/env bash
set -euo pipefail

# Rebuild + merge the C6 emulator image (console-uart) in one step, then let
# iot-emu.sh run it. Used by `moon run iot:emu-smoke` in CI (Linux, native)
# and locally. esp-emu only scans UART0, so this build forces the esp-println
# UART console instead of the device-build `auto` (USB-Serial/JTAG).

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/apps/iot"

TARGET_DIR="$ROOT/apps/iot/target/emu"
ELF="$TARGET_DIR/riscv32imac-unknown-none-elf/release/vanling"
BIN="$ROOT/dist/vanling-iot-esp32c6-devkitc-emu-merged.bin"

# ---- 1. build with UART console (into a dedicated target dir, never clobbers the
#          default C6 build used by image-c6 / flashing)
CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release -p iot-app --bin vanling \
  --target riscv32imac-unknown-none-elf \
  --no-default-features --features esp32c6-devkitc-1,console-uart
[ -f "$ELF" ] || { echo "[iot-emu-image] missing $ELF" >&2; exit 1; }

# ---- 2. merge whole-flash image for esp-emu
mkdir -p "$ROOT/dist"
espflash save-image --chip esp32c6 --merge "$ELF" "$BIN"
echo "[iot-emu-image] wrote $BIN" >&2