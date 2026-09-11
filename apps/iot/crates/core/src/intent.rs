use crate::drivers::input::{ButtonEvent, InputEvent};
use crate::drivers::light::{GROUP_CAPACITY, Rgb};
use crate::state::{Breath, DeviceManager, DeviceState, LightState};

/// Warm color palette for solid mode, cycled by double-click advance.
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

/// Solid-mode brightness steps (8-bit) cycled by long-press advance on the
/// solid color; the color rides on top unchanged.
pub const SOLID_BRIGHTNESS_STEPS: [u8; 3] = [60, 140, 255];

/// A color-group preset for breathing mode, switched by double-click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorGroup {
    pub hue_span: u8,
    pub group: [u8; GROUP_CAPACITY],
    pub group_len: u8,
}

/// Color-group presets cycled by double-click advance on breathing mode.
pub const COLOR_GROUPS: [ColorGroup; 4] = [
    // Warm sunset: amber, golden, deep rose.
    ColorGroup {
        hue_span: 255,
        group: [16, 32, 3, 0, 0, 0, 0],
        group_len: 3,
    },
    // Rainbow seven: red → orange → yellow → green → cyan → blue → purple,
    // walking the hue wheel the short way around.
    ColorGroup {
        hue_span: 255,
        group: [0, 21, 43, 85, 128, 170, 213],
        group_len: 7,
    },
    // Standalone warm white.
    ColorGroup {
        hue_span: 255,
        group: [28, 0, 0, 0, 0, 0, 0],
        group_len: 1,
    },
    // Full-hue sweep across the whole wheel.
    ColorGroup {
        hue_span: 255,
        group: [0, 0, 0, 0, 0, 0, 0],
        group_len: 0,
    },
];

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
/// palette/period/group tables.
pub fn resolve_intent(event: InputEvent, current: &DeviceState) -> Option<Intent> {
    match event {
        InputEvent::Button(ButtonEvent::Click) => Some(Intent::Light(click(current.light))),
        InputEvent::Button(ButtonEvent::DoubleClick) => {
            double_click(current.light).map(Intent::Light)
        }
        InputEvent::Button(ButtonEvent::LongPress) => long_press(current.light).map(Intent::Light),
    }
}

fn click(current: LightState) -> LightState {
    match current {
        LightState::Off => LightState::default(),
        LightState::Breath { .. } => LightState::Solid {
            color: PALETTE[0],
            brightness: DeviceManager::SOLID_DEFAULT_BRIGHTNESS,
        },
        LightState::Solid { .. } => LightState::Off,
    }
}

fn double_click(current: LightState) -> Option<LightState> {
    match current {
        LightState::Off => None,
        LightState::Breath(breath) => {
            let next_group = advance_color_group(&breath);
            Some(LightState::Breath(Breath {
                hue_span: next_group.hue_span,
                group: next_group.group,
                group_len: next_group.group_len,
                ..breath
            }))
        }
        LightState::Solid { color, brightness } => {
            let next_color = advance_palette(color);
            Some(LightState::Solid {
                color: next_color,
                brightness,
            })
        }
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
        LightState::Solid { color, brightness } => {
            let next = advance_u8(&SOLID_BRIGHTNESS_STEPS, brightness);
            Some(LightState::Solid {
                color,
                brightness: next,
            })
        }
    }
}

/// Look up `current` in `table` and return the next value (wrapping).
/// Falls back to the first entry if `current` is not found.
fn advance(table: &[u32], current: u32) -> u32 {
    let idx = table.iter().position(|&v| v == current).unwrap_or(0);
    table[(idx + 1) % table.len()]
}

fn advance_u8(table: &[u8], current: u8) -> u8 {
    let idx = table.iter().position(|&v| v == current).unwrap_or(0);
    table[(idx + 1) % table.len()]
}

/// Look up `current` in `palette` and return the next color (wrapping).
/// Falls back to the first entry if `current` is not found.
fn advance_palette(current: Rgb) -> Rgb {
    let idx = PALETTE.iter().position(|&c| c == current).unwrap_or(0);
    PALETTE[(idx + 1) % PALETTE.len()]
}

/// Look up `breath`'s color group in `COLOR_GROUPS` and return the next
/// preset (wrapping); falls back to the first entry if not found.
fn advance_color_group(breath: &Breath) -> ColorGroup {
    let idx = COLOR_GROUPS
        .iter()
        .position(|c| c.group == breath.group && c.group_len == breath.group_len)
        .unwrap_or(0);
    COLOR_GROUPS[(idx + 1) % COLOR_GROUPS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DeviceManager;

    const DEFAULT_BREATH: LightState = LightState::Breath(BREATH_BASE);

    const BREATH_BASE: Breath = Breath {
        period_ms: DeviceManager::DEFAULT_PERIOD_MS,
        hue_period_ms: DeviceManager::DEFAULT_HUE_PERIOD_MS,
        hue_span: DeviceManager::DEFAULT_HUE_SPAN,
        min_brightness: DeviceManager::DEFAULT_MIN_BRIGHTNESS,
        max_brightness: DeviceManager::DEFAULT_MAX_BRIGHTNESS,
        saturation: DeviceManager::DEFAULT_SATURATION,
        hue: DeviceManager::DEFAULT_HUE,
        group: DeviceManager::DEFAULT_GROUP,
        group_len: DeviceManager::DEFAULT_GROUP_LEN,
    };

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
            Some(Intent::Light(LightState::Solid {
                color: PALETTE[0],
                brightness: DeviceManager::SOLID_DEFAULT_BRIGHTNESS,
            }))
        );
    }

    #[test]
    fn click_solid_to_off() {
        assert_eq!(
            resolve_intent(
                InputEvent::Button(ButtonEvent::Click),
                &state(LightState::Solid {
                    color: PALETTE[2],
                    brightness: 140,
                })
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
        // 3000 (default) → wraps to [2000, 1200, 3000] head = 2000
        assert_eq!(
            result,
            LightState::Breath(Breath {
                period_ms: 2_000,
                hue_period_ms: DeviceManager::DEFAULT_HUE_PERIOD_MS,
                hue_span: DeviceManager::DEFAULT_HUE_SPAN,
                min_brightness: DeviceManager::DEFAULT_MIN_BRIGHTNESS,
                max_brightness: DeviceManager::DEFAULT_MAX_BRIGHTNESS,
                saturation: DeviceManager::DEFAULT_SATURATION,
                hue: DeviceManager::DEFAULT_HUE,
                group: DeviceManager::DEFAULT_GROUP,
                group_len: DeviceManager::DEFAULT_GROUP_LEN,
            })
        );
    }

    #[test]
    fn long_press_breath_wraps_period() {
        let slow = Breath {
            period_ms: 3_000,
            hue_period_ms: DeviceManager::DEFAULT_HUE_PERIOD_MS,
            hue_span: DeviceManager::DEFAULT_HUE_SPAN,
            min_brightness: DeviceManager::DEFAULT_MIN_BRIGHTNESS,
            max_brightness: DeviceManager::DEFAULT_MAX_BRIGHTNESS,
            saturation: DeviceManager::DEFAULT_SATURATION,
            hue: DeviceManager::DEFAULT_HUE,
            group: DeviceManager::DEFAULT_GROUP,
            group_len: DeviceManager::DEFAULT_GROUP_LEN,
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
    fn long_press_solid_advances_brightness() {
        let result = resolve_intent(
            InputEvent::Button(ButtonEvent::LongPress),
            &state(LightState::Solid {
                color: PALETTE[0],
                brightness: 60,
            }),
        )
        .unwrap()
        .light_target();
        assert_eq!(
            result,
            LightState::Solid {
                color: PALETTE[0],
                brightness: 140,
            }
        );
    }

    #[test]
    fn long_press_solid_wraps_brightness() {
        let result = resolve_intent(
            InputEvent::Button(ButtonEvent::LongPress),
            &state(LightState::Solid {
                color: PALETTE[0],
                brightness: 255,
            }),
        )
        .unwrap()
        .light_target();
        assert_eq!(
            result,
            LightState::Solid {
                color: PALETTE[0],
                brightness: 60,
            }
        );
    }

    #[test]
    fn double_click_off_is_noop() {
        assert_eq!(
            resolve_intent(
                InputEvent::Button(ButtonEvent::DoubleClick),
                &state(LightState::Off)
            ),
            None
        );
    }

    #[test]
    fn double_click_breath_advances_color_group() {
        let result = resolve_intent(
            InputEvent::Button(ButtonEvent::DoubleClick),
            &state(DEFAULT_BREATH),
        )
        .unwrap()
        .light_target();
        // Warm sunset → rainbow seven.
        assert_eq!(
            result,
            LightState::Breath(Breath {
                hue_span: COLOR_GROUPS[1].hue_span,
                group: COLOR_GROUPS[1].group,
                group_len: COLOR_GROUPS[1].group_len,
                ..BREATH_BASE
            })
        );
    }

    #[test]
    fn double_click_breath_wraps_last_preset_to_head() {
        // Walk from rainbow seven through to the last preset, then confirm the
        // sweep preset (group_len == 0) is where the chain lands; a further
        // double click wraps back to warm sunset.
        let mut breath = Breath {
            hue_span: COLOR_GROUPS[1].hue_span,
            group: COLOR_GROUPS[1].group,
            group_len: COLOR_GROUPS[1].group_len,
            ..BREATH_BASE
        };
        for (i, preset) in COLOR_GROUPS.iter().enumerate().skip(2) {
            let result = resolve_intent(
                InputEvent::Button(ButtonEvent::DoubleClick),
                &state(LightState::Breath(breath)),
            )
            .unwrap()
            .light_target();
            let LightState::Breath(next) = result else {
                panic!("double click must stay in breathing");
            };
            assert_eq!(
                next.group_len, preset.group_len,
                "step {i}: wrong preset reached"
            );
            breath = next;
        }
        // Last preset (sweep) wraps to the head (warm sunset).
        let result = resolve_intent(
            InputEvent::Button(ButtonEvent::DoubleClick),
            &state(LightState::Breath(breath)),
        )
        .unwrap()
        .light_target();
        assert_eq!(result, LightState::Breath(BREATH_BASE));
    }

    #[test]
    fn double_click_solid_advances_palette() {
        let result = resolve_intent(
            InputEvent::Button(ButtonEvent::DoubleClick),
            &state(LightState::Solid {
                color: PALETTE[0],
                brightness: 140,
            }),
        )
        .unwrap()
        .light_target();
        assert_eq!(
            result,
            LightState::Solid {
                color: PALETTE[1],
                brightness: 140,
            }
        );
    }

    #[test]
    fn double_click_solid_wraps_palette() {
        let last = *PALETTE.last().unwrap();
        let result = resolve_intent(
            InputEvent::Button(ButtonEvent::DoubleClick),
            &state(LightState::Solid {
                color: last,
                brightness: 60,
            }),
        )
        .unwrap()
        .light_target();
        assert_eq!(
            result,
            LightState::Solid {
                color: PALETTE[0],
                brightness: 60,
            }
        );
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
    fn advance_u8_falls_back_to_head_when_current_not_in_table() {
        // 7 not in the steps → treat as head and advance to 140.
        let result = advance_u8(&SOLID_BRIGHTNESS_STEPS, 7);
        assert_eq!(result, SOLID_BRIGHTNESS_STEPS[1]);
    }

    #[test]
    fn advance_color_group_falls_back_to_head_when_current_not_in_table() {
        // An unknown group → treat as head (warm sunset) and advance to rainbow.
        let unknown = Breath {
            group: [200, 250, 30, 0, 0, 0, 0],
            group_len: 3,
            ..BREATH_BASE
        };
        let result = advance_color_group(&unknown);
        assert_eq!(result, COLOR_GROUPS[1]);
    }

    #[test]
    fn seven_color_preset_uses_every_hue_slot() {
        let expected: [u8; GROUP_CAPACITY] = [0, 21, 43, 85, 128, 170, 213];
        let rainbow = COLOR_GROUPS[1];
        assert_eq!(rainbow.group_len, expected.len() as u8);
        assert_eq!(rainbow.group, expected);
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
