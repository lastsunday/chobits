use crate::drivers::light::Rgb;
use crate::intent::Intent;

/// All state owned by the device manager.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceState {
    pub light: LightState,
}

/// A breathing schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breath {
    pub period_ms: u32,
    pub hue_period_ms: u32,
    pub max_brightness: u8,
}

/// Light behavior, read by the render loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightState {
    Off,
    Solid { color: Rgb },
    Breath(Breath),
}

impl LightState {
    pub const fn boot() -> Self {
        LightState::Breath(Breath {
            period_ms: DeviceManager::DEFAULT_PERIOD_MS,
            hue_period_ms: DeviceManager::DEFAULT_HUE_PERIOD_MS,
            max_brightness: DeviceManager::DEFAULT_MAX_BRIGHTNESS,
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
    pub const DEFAULT_PERIOD_MS: u32 = 2_000;
    pub const DEFAULT_HUE_PERIOD_MS: u32 = 8_000;
    pub const DEFAULT_MAX_BRIGHTNESS: u8 = 80;

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
        period_ms: 2_000,
        hue_period_ms: 8_000,
        max_brightness: 80,
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
        });
        assert_eq!(
            manager.light_state(),
            LightState::Solid {
                color: Rgb(200, 100, 50)
            }
        );

        manager.apply(LightState::Off);
        manager.apply(LightState::Solid {
            color: Rgb(1, 2, 3),
        });
        assert_eq!(
            manager.light_state(),
            LightState::Solid {
                color: Rgb(1, 2, 3)
            }
        );
    }

    #[test]
    fn apply_breath_replaces_schedule() {
        let mut manager = DeviceManager::new();
        manager.apply(LightState::Breath(Breath {
            period_ms: 1_000,
            hue_period_ms: 3_000,
            max_brightness: 40,
        }));
        assert_eq!(
            manager.light_state(),
            LightState::Breath(Breath {
                period_ms: 1_000,
                hue_period_ms: 3_000,
                max_brightness: 40,
            })
        );
    }

    #[test]
    fn state_snapshot_matches_light_state() {
        let mut manager = DeviceManager::new();
        manager.apply(LightState::Solid {
            color: Rgb(9, 8, 7),
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
        }));
        assert_eq!(
            manager.light_state(),
            LightState::Solid {
                color: Rgb(3, 4, 5)
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
