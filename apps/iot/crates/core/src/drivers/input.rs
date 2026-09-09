use alloc::boxed::Box;

/// Active-low contract for a physical button: returns `true` when pressed.
pub trait Button {
    fn is_pressed(&self) -> bool;
}

/// A button gesture after debounce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonEvent {
    Click,
    LongPress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Button(ButtonEvent),
}

pub const BUTTON_SCAN_MS: u64 = 10;

/// Shared base tick of the app's input loop. Every source cadence is a whole
/// multiple, so each device polls on a stable grid regardless of the clock.
pub const INPUT_BASE_MS: u64 = 5;

pub const LONG_PRESS_MS: u64 = 600;

/// Consecutive identical samples needed to confirm a state change.
const DEBOUNCE_SAMPLES: u32 = 3;

/// Poll-based input source. The intent resolver drives this at a fixed
/// cadence and converts the output into [`crate::intent::Intent`]s.
pub trait InputSource {
    fn poll(&mut self, now_ms: u64) -> Option<InputEvent>;
}

/// Combines/coalesces raw input events into higher-level gestures.
/// Implementations can accumulate state (e.g. multi-press counters, hold
/// durations) and suppress intermediate events.
pub trait EventAggregator {
    fn feed(&mut self, event: InputEvent, now_ms: u64) -> Option<InputEvent>;
}

/// Identity aggregator — passes every event through unchanged.
pub struct PassThrough;

impl EventAggregator for PassThrough {
    fn feed(&mut self, event: InputEvent, _now_ms: u64) -> Option<InputEvent> {
        Some(event)
    }
}

/// A polled input source scheduled at its own cadence, paired with the
/// aggregator that turns its raw samples into the events the intent resolver
/// understands. The input task polls it on the shared base tick, gating each
/// source to its own cadence.
pub struct PollEntry {
    source: Box<dyn InputSource>,
    aggregator: Box<dyn EventAggregator>,
    cadence_ms: u64,
    next_at_ms: u64,
}

impl PollEntry {
    pub fn new(
        source: Box<dyn InputSource>,
        aggregator: Box<dyn EventAggregator>,
        cadence_ms: u64,
    ) -> Self {
        // First sample lands on the cadence grid, so debounce windows stay
        // uniform straight from boot.
        Self {
            source,
            aggregator,
            cadence_ms,
            next_at_ms: cadence_ms,
        }
    }

    /// The source's cadence, a whole multiple of the shared base tick.
    pub fn cadence_ms(&self) -> u64 {
        self.cadence_ms
    }

    /// When this source is next due on the shared grid.
    pub fn next_at_ms(&self) -> u64 {
        self.next_at_ms
    }

    /// Poll the source and fold the raw sample through the aggregator.
    pub fn poll(&mut self, now_ms: u64) -> Option<InputEvent> {
        let event = self.source.poll(now_ms)?;
        self.aggregator.feed(event, now_ms)
    }

    /// Advance the source's schedule past `now_ms`, keeping it on its own
    /// cadence grid with no catch-up burst.
    pub fn advance_past(&mut self, now_ms: u64) {
        while self.next_at_ms <= now_ms {
            self.next_at_ms = self.next_at_ms.wrapping_add(self.cadence_ms);
        }
    }
}

/// Debounced button scanner, distinguishing clicks from long-presses by
/// hold duration.
pub struct ButtonScanner<B: Button> {
    button: B,
    pressed: bool,
    held_since_ms: u64,
    same_count: u32,
}

impl<B: Button> ButtonScanner<B> {
    pub fn new(button: B) -> Self {
        Self {
            button,
            pressed: false,
            held_since_ms: 0,
            same_count: 0,
        }
    }

    fn read(&mut self, now_ms: u64) -> Option<ButtonEvent> {
        let raw = self.button.is_pressed();

        if raw == self.pressed {
            self.same_count = 0;
            return None;
        }

        self.same_count += 1;
        if self.same_count < DEBOUNCE_SAMPLES {
            return None;
        }

        self.same_count = 0;
        self.pressed = raw;

        if raw {
            self.held_since_ms = now_ms;
            None
        } else {
            let held = now_ms.saturating_sub(self.held_since_ms);
            Some(if held >= LONG_PRESS_MS {
                ButtonEvent::LongPress
            } else {
                ButtonEvent::Click
            })
        }
    }
}

impl<B: Button> InputSource for ButtonScanner<B> {
    fn poll(&mut self, now_ms: u64) -> Option<InputEvent> {
        self.read(now_ms).map(InputEvent::Button)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    struct FakeButton<'a> {
        pressed: &'a Cell<bool>,
    }

    impl Button for FakeButton<'_> {
        fn is_pressed(&self) -> bool {
            self.pressed.get()
        }
    }

    #[test]
    fn click_fires_after_debounce_on_release() {
        let pressed = Cell::new(false);
        let btn = FakeButton { pressed: &pressed };
        let mut scanner = ButtonScanner::new(btn);

        // 3 samples pressed → stable
        for ms in [0, 10, 20] {
            pressed.set(true);
            assert!(scanner.poll(ms).is_none());
        }
        pressed.set(true);
        assert!(scanner.poll(30).is_none());

        // 3 samples released → fires Click
        pressed.set(false);
        assert!(scanner.poll(40).is_none());
        assert!(scanner.poll(50).is_none());
        assert_eq!(
            scanner.poll(60),
            Some(InputEvent::Button(ButtonEvent::Click))
        );
    }

    #[test]
    fn long_press_fires_on_release_after_threshold() {
        assert_eq!(hold_and_release(800), Some(ButtonEvent::LongPress));
    }

    #[test]
    fn debounce_ignores_glitch_shorter_than_n_samples() {
        let pressed = Cell::new(false);
        let btn = FakeButton { pressed: &pressed };
        let mut scanner = ButtonScanner::new(btn);

        // 2 samples pressed (less than DEBOUNCE_SAMPLES)
        pressed.set(true);
        assert!(scanner.poll(0).is_none());
        assert!(scanner.poll(10).is_none());
        pressed.set(false);
        assert!(scanner.poll(20).is_none());
        assert!(scanner.poll(30).is_none());
        assert!(!scanner.pressed, "should still be released");
    }

    #[test]
    fn scan_interval_matches_design() {
        assert_eq!(BUTTON_SCAN_MS, 10);
    }

    #[test]
    fn base_interval_divides_button_cadence() {
        assert_eq!(BUTTON_SCAN_MS % INPUT_BASE_MS, 0);
    }

    #[test]
    fn long_press_threshold_matches_design() {
        assert_eq!(LONG_PRESS_MS, 600);
    }

    /// Press the button, let it debounce to held at t=20, then release and
    /// confirm a released edge after `hold` ms. Returns the resulting gesture.
    fn hold_and_release(hold_ms: u64) -> Option<ButtonEvent> {
        let pressed = Cell::new(false);
        let btn = FakeButton { pressed: &pressed };
        let mut scanner = ButtonScanner::new(btn);

        // Press down; debounce confirms press at t=20 (held_since=20).
        pressed.set(true);
        for ms in 0..=30 {
            scanner.poll(ms * 10);
        }

        // Release starting at t=hold; debounce confirms the released edge at
        // hold+20 (3 consecutive released samples), computing held=hold_ms.
        pressed.set(false);
        let mut outcome = None;
        for ms in 0..=20 {
            if let Some(InputEvent::Button(e)) = scanner.poll(hold_ms + ms * 10) {
                outcome = Some(e);
            }
        }
        outcome
    }

    #[test]
    fn long_press_boundary_is_inclusive_at_600ms() {
        // held = 599ms → click
        assert_eq!(hold_and_release(599), Some(ButtonEvent::Click));
        // held = 600ms → long press (>= threshold)
        assert_eq!(hold_and_release(600), Some(ButtonEvent::LongPress));
        // held = 610ms → long press
        assert_eq!(hold_and_release(610), Some(ButtonEvent::LongPress));
    }

    #[test]
    fn passthrough_forwards_event() {
        let mut agg = PassThrough;
        let event = InputEvent::Button(ButtonEvent::Click);
        assert_eq!(agg.feed(event, 100), Some(event));
    }

    #[test]
    fn poll_entry_anchors_first_sample_on_cadence_grid() {
        // Leak an owned cell so the boxed, `'static` input source can share
        // the press state with the test drive loop.
        let pressed: &'static Cell<bool> = Box::leak(Box::new(Cell::new(false)));
        let btn = FakeButton { pressed };
        let mut entry = PollEntry::new(
            Box::new(ButtonScanner::new(btn)),
            Box::new(PassThrough),
            BUTTON_SCAN_MS,
        );
        assert_eq!(entry.next_at_ms(), BUTTON_SCAN_MS);
        entry.advance_past(BUTTON_SCAN_MS);
        assert_eq!(entry.next_at_ms(), 2 * BUTTON_SCAN_MS);
        entry.advance_past(2 * BUTTON_SCAN_MS - 1);
        assert_eq!(entry.next_at_ms(), 2 * BUTTON_SCAN_MS);
        entry.advance_past(25);
        assert_eq!(entry.next_at_ms(), 3 * BUTTON_SCAN_MS);
    }

    #[test]
    fn poll_entry_forwards_debounced_button_click() {
        let pressed: &'static Cell<bool> = Box::leak(Box::new(Cell::new(false)));
        let btn = FakeButton { pressed };
        let mut entry = PollEntry::new(
            Box::new(ButtonScanner::new(btn)),
            Box::new(PassThrough),
            BUTTON_SCAN_MS,
        );
        for ms in [0, 10, 20] {
            pressed.set(true);
            assert!(entry.poll(ms).is_none());
        }
        pressed.set(false);
        assert!(entry.poll(40).is_none());
        assert!(entry.poll(50).is_none());
        assert_eq!(entry.poll(60), Some(InputEvent::Button(ButtonEvent::Click)));
    }
}
