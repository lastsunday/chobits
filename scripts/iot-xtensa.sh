#!/usr/bin/env bash
set -euo pipefail

# Runs a cargo command for the xtensa-esp32s3 target, dispatching by
# environment so one moon task serves both CI/release and macOS (Intel):
#
#   - native: a complete espup "esp" toolchain (pinned to 1.95.0.0, matching
#     the Docker image below) is available -> run cargo directly;
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
# need the xtensa-esp32s3-elf GCC for linking and the rust-src component for
# -Z build-std. `rustup toolchain list` alone is not enough (a bare/partial
# toolchain dir would otherwise fail native instead of falling back).
has_native_esp() {
  [ -x "$ESP_DIR/bin/rustc" ] &&
    [ -x "$ESP_DIR/xtensa-esp32s3-elf/bin/xtensa-esp32s3-elf-gcc" ] &&
    [ -d "$ESP_DIR/lib/rustlib/src/rust/library" ]
}

has_docker() {
  command -v docker >/dev/null 2>&1
}

run_native() {
  [ -f "$HOME/export-esp.sh" ] && . "$HOME/export-esp.sh"
  [ -f "$ESP_DIR/export-esp.sh" ] && . "$ESP_DIR/export-esp.sh"
  export PATH="$ESP_DIR/xtensa-esp32s3-elf/bin:$PATH"
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