use alloc::vec::Vec;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Watch;
use embassy_time::{Duration, Ticker};
use iot_core::drivers::input::{INPUT_BASE_MS, PollEntry};
use iot_core::intent::{Intent, resolve_intent};
use iot_core::state::DeviceState;

/// Intent channel: input task writes resolved intents, the render loop drains.
pub type IntentBus = Channel<CriticalSectionRawMutex, Intent, 8>;

/// Latest device-state snapshot, read by the intent resolver when it rebases
/// the next intent. The single writer is the render loop.
pub type StateWatch = Watch<CriticalSectionRawMutex, DeviceState, 1>;

/// Intent bus storage; passed as `&'static` into the tasks so no consumer
/// reaches the global by name.
pub static INTENT_BUS: IntentBus = IntentBus::new();

/// Device-state watch storage; passed as `&'static` into the tasks.
pub static DEVICE_STATE: StateWatch = StateWatch::new();

/// Drains every input source at the shared base tick, gating each source to
/// its own cadence, and forwards resolved intents onto the intent bus. `try_send`
/// drops intents when the bus is full: dropping a press is preferable to
/// blocking the scan loop, which would skew debounce timing.
pub async fn input_task(
    intent_bus: &'static IntentBus,
    device_state: &'static StateWatch,
    mut sources: Vec<PollEntry>,
) -> ! {
    let mut ticker = Ticker::every(Duration::from_millis(INPUT_BASE_MS));
    let mut now_ms: u64 = 0;
    loop {
        // Ticks stay anchored to the base clock even when a source takes
        // inconsistent time to sample, so slow devices never alias.
        ticker.next().await;
        now_ms = now_ms.wrapping_add(INPUT_BASE_MS);
        // The resolver rebases every raw event against the freshest snapshot,
        // so a state change the render loop already broadcast is not re-applied
        // as a duplicate intent.
        let snapshot = device_state.try_get().unwrap_or_default();
        for entry in sources.iter_mut() {
            if entry.next_at_ms() > now_ms {
                continue;
            }
            if let Some(event) = entry.poll(now_ms)
                && let Some(intent) = resolve_intent(event, &snapshot)
            {
                let _ = intent_bus.try_send(intent);
            }
            // Advance past the current time, keeping the source on its own
            // cadence grid with no catch-up burst.
            entry.advance_past(now_ms);
        }
    }
}
