use crate::drivers::light::{GROUP_CAPACITY, Rgb};
use crate::intent::Intent;

/// All state owned by the device manager.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceState {
    pub light: LightState,
}

/// A breathing schedule: brightness envelope plus the hue path it travels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breath {
    pub period_ms: u32,
    pub hue_period_ms: u32,
    /// Hue sweep width; `0` means a static standalone color. Ignored when
    /// `group_len > 0`.
    pub hue_span: u8,
    /// Trough value of the brightness envelope. A nonzero floor keeps the
    /// breathing glow lit instead of fading fully to black.
    pub min_brightness: u8,
    pub max_brightness: u8,
    pub saturation: u8,
    /// Base hue the sweep starts from (or holds when `hue_span == 0`). Ignored
    /// when `group_len > 0`.
    pub hue: u8,
    /// Color waypoints on the 256-step hue wheel. `group_len` picks the
    /// trajectory: `0` = the `hue`/`hue_span` sweep above; `1` = a static
    /// standalone color, `group[0]`; `>=2` = rotate through the waypoints,
    /// walking each `group[i] -> group[i+1]` segment over
    /// `hue_period_ms / group_len` and wrapping `group[last] -> group[0]`.
    pub group: [u8; Self::MAX_GROUP],
    pub group_len: u8,
}

impl Breath {
    pub const MAX_GROUP: usize = GROUP_CAPACITY;
}

/// Light behavior, read by the render loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightState {
    Off,
    Solid { color: Rgb, brightness: u8 },
    Breath(Breath),
}

impl LightState {
    pub const fn boot() -> Self {
        LightState::Breath(Breath {
            period_ms: DeviceManager::DEFAULT_PERIOD_MS,
            hue_period_ms: DeviceManager::DEFAULT_HUE_PERIOD_MS,
            hue_span: DeviceManager::DEFAULT_HUE_SPAN,
            min_brightness: DeviceManager::DEFAULT_MIN_BRIGHTNESS,
            max_brightness: DeviceManager::DEFAULT_MAX_BRIGHTNESS,
            saturation: DeviceManager::DEFAULT_SATURATION,
            hue: DeviceManager::DEFAULT_HUE,
            group: DeviceManager::DEFAULT_GROUP,
            group_len: DeviceManager::DEFAULT_GROUP_LEN,
        })
    }
}

impl Default for LightState {
    fn default() -> Self {
        LightState::boot()
    }
}

/// Single owner of device state. Producers apply absolute target states
/// wholesale, so the device never needs to remember "last breath".
pub struct DeviceManager {
    state: DeviceState,
}

impl DeviceManager {
    pub const DEFAULT_PERIOD_MS: u32 = 3_000;
    pub const DEFAULT_HUE_PERIOD_MS: u32 = 60_000;
    pub const DEFAULT_HUE_SPAN: u8 = 255;
    pub const DEFAULT_MIN_BRIGHTNESS: u8 = 24;
    pub const DEFAULT_MAX_BRIGHTNESS: u8 = 80;
    pub const DEFAULT_SATURATION: u8 = 200;
    pub const DEFAULT_HUE: u8 = 0;
    /// Warm sunset: amber, golden, deep rose — low-blue palette favored by
    /// circadian research for a calm ambient breathing glow.
    pub const DEFAULT_GROUP: [u8; Breath::MAX_GROUP] = [16, 32, 3, 0, 0, 0, 0];
    pub const DEFAULT_GROUP_LEN: u8 = 3;

    /// Full-brightness entry point for solid mode; dimming is a long-press
    /// advance from here.
    pub const SOLID_DEFAULT_BRIGHTNESS: u8 = 255;

    /// Backlight floor for the breathing envelope: the panel never sinks below
    /// this percent while pixels are lit, so the trough stays a dim glow
    /// instead of a hard on/off blip.
    pub const BACKLIGHT_FLOOR_PCT: u8 = 12;

    pub const fn new() -> Self {
        Self {
            state: DeviceState {
                light: LightState::boot(),
            },
        }
    }

    pub fn apply(&mut self, state: LightState) {
        self.state.light = state;
    }

    /// Apply a bus intent to whichever subsystem it targets. The manager owns
    /// the subsystem mapping, so a new intent variant only grows this match and
    /// never touches the consuming task.
    pub fn apply_intent(&mut self, intent: Intent) {
        match intent {
            Intent::Light(state) => self.apply(state),
        }
    }

    pub fn state(&self) -> DeviceState {
        self.state.clone()
    }

    pub fn light_state(&self) -> LightState {
        self.state.light
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BREATH: LightState = LightState::Breath(Breath {
        period_ms: 3_000,
        hue_period_ms: 60_000,
        hue_span: 255,
        min_brightness: 24,
        max_brightness: 80,
        saturation: 200,
        hue: 0,
        group: [16, 32, 3, 0, 0, 0, 0],
        group_len: 3,
    });

    #[test]
    fn boots_with_default_breathing() {
        let manager = DeviceManager::new();
        assert_eq!(manager.light_state(), BREATH);
    }

    #[test]
    fn device_state_default_is_boot() {
        assert_eq!(DeviceState::default().light, BREATH);
    }

    #[test]
    fn apply_off_switches_light_off() {
        let mut manager = DeviceManager::new();
        manager.apply(LightState::Off);
        assert_eq!(manager.light_state(), LightState::Off);
    }

    #[test]
    fn apply_solid_replaces_any_state() {
        let mut manager = DeviceManager::new();
        manager.apply(LightState::Solid {
            color: Rgb(200, 100, 50),
            brightness: DeviceManager::SOLID_DEFAULT_BRIGHTNESS,
        });
        assert_eq!(
            manager.light_state(),
            LightState::Solid {
                color: Rgb(200, 100, 50),
                brightness: DeviceManager::SOLID_DEFAULT_BRIGHTNESS,
            }
        );

        manager.apply(LightState::Off);
        manager.apply(LightState::Solid {
            color: Rgb(1, 2, 3),
            brightness: DeviceManager::SOLID_DEFAULT_BRIGHTNESS,
        });
        assert_eq!(
            manager.light_state(),
            LightState::Solid {
                color: Rgb(1, 2, 3),
                brightness: DeviceManager::SOLID_DEFAULT_BRIGHTNESS,
            }
        );
    }

    #[test]
    fn apply_breath_replaces_schedule() {
        let mut manager = DeviceManager::new();
        manager.apply(LightState::Breath(Breath {
            period_ms: 1_000,
            hue_period_ms: 3_000,
            hue_span: 80,
            min_brightness: 8,
            max_brightness: 40,
            saturation: 120,
            hue: 170,
            group: [200, 250, 30, 0, 0, 0, 0],
            group_len: 3,
        }));
        assert_eq!(
            manager.light_state(),
            LightState::Breath(Breath {
                period_ms: 1_000,
                hue_period_ms: 3_000,
                hue_span: 80,
                min_brightness: 8,
                max_brightness: 40,
                saturation: 120,
                hue: 170,
                group: [200, 250, 30, 0, 0, 0, 0],
                group_len: 3,
            })
        );
    }

    #[test]
    fn state_snapshot_matches_light_state() {
        let mut manager = DeviceManager::new();
        manager.apply(LightState::Solid {
            color: Rgb(9, 8, 7),
            brightness: DeviceManager::SOLID_DEFAULT_BRIGHTNESS,
        });
        assert_eq!(manager.state().light, manager.light_state());
    }

    #[test]
    fn apply_intent_light_replaces_light_state() {
        let mut manager = DeviceManager::new();
        manager.apply_intent(crate::intent::Intent::Light(LightState::Off));
        assert_eq!(manager.light_state(), LightState::Off);
        manager.apply_intent(crate::intent::Intent::Light(LightState::Solid {
            color: Rgb(3, 4, 5),
            brightness: DeviceManager::SOLID_DEFAULT_BRIGHTNESS,
        }));
        assert_eq!(
            manager.light_state(),
            LightState::Solid {
                color: Rgb(3, 4, 5),
                brightness: DeviceManager::SOLID_DEFAULT_BRIGHTNESS,
            }
        );
    }

    #[test]
    fn default_matches_new() {
        assert_eq!(
            DeviceManager::default().light_state(),
            DeviceManager::new().light_state()
        );
    }
}
