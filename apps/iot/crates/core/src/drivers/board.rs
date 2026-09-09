use crate::drivers::input::PollEntry;
use crate::drivers::light::RgbLight;
use alloc::vec::Vec;

/// A hardware board instance. Implemented per board in the bsp crate.
pub trait Board: Sized {}

/// Board exposing a light channel (an RGB light surface).
///
/// A channel may drive several pixels wired in the same chain (e.g. a WS2812
/// strip); boards with distinct light kinds add further `HasXxx` traits.
pub trait HasLight: Board {
    /// Owned light surface; `'static` so it can live behind a boxed renderer
    /// in the render task for the board's lifetime.
    type Light: RgbLight + 'static;

    /// Take the board's light. Returns `None` when not wired or already taken.
    fn take_light(&mut self) -> Option<Self::Light>;
}

/// Board providing the input sources for the input pipeline.
///
/// The board owns the wiring and the driver choice, so a touchscreen board
/// simply assembles different sources behind the same trait; the app consumes
/// the ready-made entries and never names a specific device.
pub trait HasInput: Board {
    /// Take the board's input sources. Returns `None` when no input is wired
    /// or it was already taken.
    fn take_input(&mut self) -> Option<Vec<PollEntry>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drivers::input::{BUTTON_SCAN_MS, Button, ButtonScanner, PassThrough};
    use crate::drivers::light::Rgb;
    use alloc::{boxed::Box, vec};

    struct FakeLight;

    impl RgbLight for FakeLight {
        fn set_rgb(&mut self, _color: Rgb) {}
    }

    struct FakeButton;

    impl Button for FakeButton {
        fn is_pressed(&self) -> bool {
            false
        }
    }

    struct FakeBoard {
        light: Option<FakeLight>,
        input: Option<Vec<PollEntry>>,
    }

    impl Board for FakeBoard {}

    impl HasLight for FakeBoard {
        type Light = FakeLight;

        fn take_light(&mut self) -> Option<Self::Light> {
            self.light.take()
        }
    }

    impl HasInput for FakeBoard {
        fn take_input(&mut self) -> Option<Vec<PollEntry>> {
            self.input.take()
        }
    }

    #[test]
    fn has_light_delivers_the_board_light_once() {
        let mut board = FakeBoard {
            light: Some(FakeLight),
            input: None,
        };
        let mut light = board.take_light().expect("light present");
        light.set_rgb(Rgb(1, 2, 3));
        assert!(board.take_light().is_none(), "taken exactly once");
    }

    #[test]
    fn has_input_delivers_sources_then_none() {
        let mut board = FakeBoard {
            light: None,
            input: Some(vec![PollEntry::new(
                Box::new(ButtonScanner::new(FakeButton)),
                Box::new(PassThrough),
                BUTTON_SCAN_MS,
            )]),
        };
        let sources = board.take_input().expect("input present");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].cadence_ms(), BUTTON_SCAN_MS);
        assert!(board.take_input().is_none(), "taken exactly once");
    }
}
