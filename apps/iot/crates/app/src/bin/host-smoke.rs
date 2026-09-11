//! Host smoke for the app composition: boots [`iot_app::run`] on a fake
//! board under the std backend and asserts the real intent → render pipeline
//! paints a click target. This is the CI regression gate that needs no ESP
//! hardware (see docs/content/development/iot/emulation.md).
//!
//! Build/run: `cargo run -p iot-app --bin host-smoke --no-default-features
//! --features host` (gated by `required-features` so esp builds skip it).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};
use iot_app::run;
use iot_core::drivers::board::{Board as BoardTrait, HasInput, HasLight};
use iot_core::drivers::input::{
    BUTTON_SCAN_MS, Button, ButtonScanner, DoubleClickAggregator, PollEntry,
};
use iot_core::drivers::light::{Fill, Rgb, RgbLight};
use iot_core::intent::PALETTE;

/// Recorded paint operations, so the smoke can assert what `run` drew.
static PAINTED: Mutex<Vec<(Fill, Rgb)>> = Mutex::new(Vec::new());

/// Firmware-visible press state, flipped by the smoke scenario.
static PRESSED: AtomicBool = AtomicBool::new(false);

struct HostButton;

impl Button for HostButton {
    fn is_pressed(&self) -> bool {
        PRESSED.load(Ordering::SeqCst)
    }
}

struct HostLight;

impl RgbLight for HostLight {
    fn set_fill(&mut self, fill: Fill, color: Rgb) {
        PAINTED.lock().unwrap().push((fill, color));
    }

    fn set_backlight(&mut self, _level_pct: u8) {}
}

struct HostBoard {
    light: Option<HostLight>,
    input: Option<Vec<PollEntry>>,
}

impl HostBoard {
    fn new() -> Self {
        Self {
            light: Some(HostLight),
            input: Some(vec![PollEntry::new(
                Box::new(ButtonScanner::new(HostButton)),
                Box::new(DoubleClickAggregator::new()),
                BUTTON_SCAN_MS,
            )]),
        }
    }
}

impl BoardTrait for HostBoard {}

impl HasLight for HostBoard {
    type Light = HostLight;

    fn take_light(&mut self) -> Option<Self::Light> {
        self.light.take()
    }
}

impl HasInput for HostBoard {
    fn take_input(&mut self) -> Option<Vec<PollEntry>> {
        self.input.take()
    }
}

fn painted(color: Rgb) -> bool {
    PAINTED.lock().unwrap().iter().any(|&(_, c)| c == color)
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Boot the real composition: input task + render loop on a fake board.
    // `run` and the std executor never return, so the scenario verdicts by
    // terminating the process explicitly.
    let board = HostBoard::new();
    match select(run(board), scenario()).await {
        Either::First(never) => match never {},
        Either::Second(()) => {}
    }
}

async fn scenario() {
    // 1. Boot: the breathing light paints some frames immediately.
    Timer::after(Duration::from_millis(200)).await;
    assert!(
        !PAINTED.lock().unwrap().is_empty(),
        "boot breath did not paint any frame"
    );

    // 2. Single click: breathing → solid `PALETTE[0]`.
    PRESSED.store(true, Ordering::SeqCst);
    Timer::after(Duration::from_millis(120)).await;
    PRESSED.store(false, Ordering::SeqCst);

    // Click resolves after debounce + double-click window expiration.
    let mut attempts = 0;
    while !painted(PALETTE[0]) && attempts < 500 {
        Timer::after(Duration::from_millis(10)).await;
        attempts += 1;
    }
    assert!(painted(PALETTE[0]), "click did not paint palette[0]");
    std::process::exit(0);
}
