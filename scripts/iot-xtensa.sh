#!/usr/bin/env bash
set -euo pipefail

# Runs a cargo command for the xtensa-esp32s3 target, dispatching by
# environment so one moon task serves both CI/release and macOS (Intel):
#
#   - native: a complete espup "esp" toolchain (pinned to 1.95.0.0, matching
#     the Docker image below) is available -> run cargo directly. Works with
#     both the unified xtensa-esp-elf layout (espup 0.17+) and the legacy
#     per-chip xtensa-esp32s3-elf layout;
#   - docker: esp toolchain missing/incomplete (e.g. macOS Intel, where esp-rs
#     stopped shipping toolchains at v1.91+) -> fall back to the
#     espressif/idf-rust container;
#   - otherwise: fail with a hint instead of guessing.
#
# Args are forwarded verbatim to cargo (e.g. `check -p iot-app --target ...`).

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="$ROOT/apps/iot"
IMAGE="espressif/idf-rust:esp32s3_1.95.0.0"
ESP_DIR="${ESPUP_TOOLCHAIN_DIR:-$HOME/.rustup/toolchains/esp}"

# Only a complete espup install counts as native: besides rustc itself we
# need a GCC linker and the rust-src component for -Z build-std.
# espup 0.17+ installs the unified xtensa-esp-elf layout; older versions
# used per-chip directories (e.g. xtensa-esp32s3-elf).
has_native_esp() {
  local has_gcc=""
  if ls "$ESP_DIR"/xtensa-esp-elf/esp-*/bin/xtensa-esp-elf-gcc >/dev/null 2>&1; then
    has_gcc=1
  elif [ -x "$ESP_DIR/xtensa-esp32s3-elf/bin/xtensa-esp32s3-elf-gcc" ]; then
    has_gcc=1
  fi
  [ -x "$ESP_DIR/bin/rustc" ] &&
    [ -n "$has_gcc" ] &&
    [ -d "$ESP_DIR/lib/rustlib/src/rust/library" ]
}

has_docker() {
  command -v docker >/dev/null 2>&1
}

run_native() {
  [ -f "$HOME/export-esp.sh" ] && . "$HOME/export-esp.sh"
  [ -f "$ESP_DIR/export-esp.sh" ] && . "$ESP_DIR/export-esp.sh"
  local gcc_dir=""
  if [ -d "$ESP_DIR/xtensa-esp-elf" ]; then
    gcc_dir="$(find "$ESP_DIR/xtensa-esp-elf" -maxdepth 2 -type d -name bin 2>/dev/null | head -1)"
  elif [ -d "$ESP_DIR/xtensa-esp32s3-elf" ]; then
    gcc_dir="$ESP_DIR/xtensa-esp32s3-elf/bin"
  fi
  if [ -n "$gcc_dir" ]; then
    export PATH="$gcc_dir:$PATH"
  fi
  cd "$PROJECT"
  RUSTUP_TOOLCHAIN=esp cargo "$@"
}

run_docker() {
  docker run --rm \
    -v "$PROJECT:/project" \
    -v "$HOME/.cargo/registry:/home/esp/.cargo/registry" \
    -w /project \
    "$IMAGE" \
    bash -lc 'source /home/esp/export-esp.sh && cargo --offline "$@"' \
    bash "$@"
}

if has_native_esp; then
  echo "[iot-xtensa] native esp toolchain" >&2
  run_native "$@"
elif has_docker; then
  echo "[iot-xtensa] docker fallback ($IMAGE)" >&2
  run_docker "$@"
else
  echo "no esp toolchain (espup install -n esp -v 1.95.0.0) nor docker available" >&2
  exit 1
fi