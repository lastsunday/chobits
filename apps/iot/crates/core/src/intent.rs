use crate::drivers::input::{ButtonEvent, InputEvent};
use crate::drivers::light::Rgb;
use crate::state::{Breath, DeviceState, LightState};

/// Warm color palette for solid mode, cycled by long-press advance.
/// Value 120 keeps the LED visible but not overpowering alongside the
/// breathing mode's soft glow.
pub const PALETTE: [Rgb; 4] = [
    Rgb(255, 210, 140),
    Rgb(230, 120, 190),
    Rgb(120, 180, 255),
    Rgb(160, 255, 200),
];

/// Breathing periods (ms) cycled by long-press advance.
pub const BREATH_PERIODS_MS: [u32; 3] = [2_000, 1_200, 3_000];

/// Absolute target state for the device manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Light(LightState),
}

impl Intent {
    pub fn light_target(self) -> LightState {
        match self {
            Intent::Light(state) => state,
        }
    }
}

/// Stateless intent resolver. Maps a raw input event to an absolute target
/// state by reading the current snapshot and advancing through the
/// palette/period tables.
pub fn resolve_intent(event: InputEvent, current: &DeviceState) -> Option<Intent> {
    match event {
        InputEvent::Button(ButtonEvent::Click) => Some(Intent::Light(click(current.light))),
        InputEvent::Button(ButtonEvent::LongPress) => long_press(current.light).map(Intent::Light),
    }
}

fn click(current: LightState) -> LightState {
    match current {
        LightState::Off => LightState::default(),
        LightState::Breath { .. } => LightState::Solid { color: PALETTE[0] },
        LightState::Solid { .. } => LightState::Off,
    }
}

fn long_press(current: LightState) -> Option<LightState> {
    match current {
        LightState::Off => None,
        LightState::Breath(breath) => {
            let next_period = advance(&BREATH_PERIODS_MS, breath.period_ms);
            Some(LightState::Breath(Breath {
                period_ms: next_period,
                ..breath
            }))
        }
        LightState::Solid { color } => {
            let next_color = advance_palette(color);
            Some(LightState::Solid { color: next_color })
        }
    }
}

/// Look up `current` in `table` and return the next value (wrapping).
/// Falls back to the first entry if `current` is not found.
fn advance(table: &[u32], current: u32) -> u32 {
    let idx = table.iter().position(|&v| v == current).unwrap_or(0);
    table[(idx + 1) % table.len()]
}

fn advance_palette(current: Rgb) -> Rgb {
    let idx = PALETTE.iter().position(|&c| c == current).unwrap_or(0);
    PALETTE[(idx + 1) % PALETTE.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DeviceManager;

    const DEFAULT_BREATH: LightState = LightState::Breath(Breath {
        period_ms: DeviceManager::DEFAULT_PERIOD_MS,
        hue_period_ms: DeviceManager::DEFAULT_HUE_PERIOD_MS,
        max_brightness: DeviceManager::DEFAULT_MAX_BRIGHTNESS,
    });

    fn state(light: LightState) -> DeviceState {
        DeviceState { light }
    }

    #[test]
    fn click_off_to_breath() {
        assert_eq!(
            resolve_intent(
                InputEvent::Button(ButtonEvent::Click),
                &state(LightState::Off)
            ),
            Some(Intent::Light(DEFAULT_BREATH))
        );
    }

    #[test]
    fn click_breath_to_solid() {
        assert_eq!(
            resolve_intent(
                InputEvent::Button(ButtonEvent::Click),
                &state(DEFAULT_BREATH)
            ),
            Some(Intent::Light(LightState::Solid { color: PALETTE[0] }))
        );
    }

    #[test]
    fn click_solid_to_off() {
        assert_eq!(
            resolve_intent(
                InputEvent::Button(ButtonEvent::Click),
                &state(LightState::Solid { color: PALETTE[2] })
            ),
            Some(Intent::Light(LightState::Off))
        );
    }

    #[test]
    fn long_press_off_is_noop() {
        assert_eq!(
            resolve_intent(
                InputEvent::Button(ButtonEvent::LongPress),
                &state(LightState::Off)
            ),
            None
        );
    }

    #[test]
    fn long_press_breath_advances_period() {
        let result = resolve_intent(
            InputEvent::Button(ButtonEvent::LongPress),
            &state(DEFAULT_BREATH),
        )
        .unwrap()
        .light_target();
        // 2000 → next in [2000, 1200, 3000] = 1200
        assert_eq!(
            result,
            LightState::Breath(Breath {
                period_ms: 1_200,
                hue_period_ms: DeviceManager::DEFAULT_HUE_PERIOD_MS,
                max_brightness: DeviceManager::DEFAULT_MAX_BRIGHTNESS,
            })
        );
    }

    #[test]
    fn long_press_breath_wraps_period() {
        let slow = Breath {
            period_ms: 3_000,
            hue_period_ms: DeviceManager::DEFAULT_HUE_PERIOD_MS,
            max_brightness: DeviceManager::DEFAULT_MAX_BRIGHTNESS,
        };
        let result = resolve_intent(
            InputEvent::Button(ButtonEvent::LongPress),
            &state(LightState::Breath(slow)),
        )
        .unwrap()
        .light_target();
        // 3000 (last) → wraps to 2000
        assert_eq!(
            result,
            LightState::Breath(Breath {
                period_ms: 2_000,
                ..slow
            })
        );
    }

    #[test]
    fn long_press_solid_advances_color() {
        let result = resolve_intent(
            InputEvent::Button(ButtonEvent::LongPress),
            &state(LightState::Solid { color: PALETTE[0] }),
        )
        .unwrap()
        .light_target();
        assert_eq!(result, LightState::Solid { color: PALETTE[1] });
    }

    #[test]
    fn long_press_solid_wraps_color() {
        let last = *PALETTE.last().unwrap();
        let result = resolve_intent(
            InputEvent::Button(ButtonEvent::LongPress),
            &state(LightState::Solid { color: last }),
        )
        .unwrap()
        .light_target();
        assert_eq!(result, LightState::Solid { color: PALETTE[0] });
    }

    #[test]
    fn advance_falls_back_to_head_when_current_not_in_table() {
        // 9999 not in the table → treat as head (2000) and advance to 1200
        let result = advance(&BREATH_PERIODS_MS, 9999);
        assert_eq!(result, BREATH_PERIODS_MS[1]);
    }

    #[test]
    fn advance_palette_falls_back_to_head_when_current_not_in_table() {
        // 0,0,0 not in the palette → treat as head and advance to PALETTE[1]
        let result = advance_palette(Rgb(0, 0, 0));
        assert_eq!(result, PALETTE[1]);
    }

    #[test]
    fn full_click_cycle_breath_solid_off() {
        let mut light = LightState::Off;
        light = resolve_intent(InputEvent::Button(ButtonEvent::Click), &state(light))
            .unwrap()
            .light_target();
        assert!(matches!(light, LightState::Breath(_)));

        light = resolve_intent(InputEvent::Button(ButtonEvent::Click), &state(light))
            .unwrap()
            .light_target();
        assert!(matches!(light, LightState::Solid { .. }));

        light = resolve_intent(InputEvent::Button(ButtonEvent::Click), &state(light))
            .unwrap()
            .light_target();
        assert_eq!(light, LightState::Off);
    }

    #[test]
    fn intent_led_target_extracts_led_state() {
        let intent = Intent::Light(DEFAULT_BREATH);
        assert_eq!(intent.light_target(), DEFAULT_BREATH);
    }
}
