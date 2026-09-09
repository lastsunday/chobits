use crate::drivers::light::Rgb;
use crate::state::{Breath, DeviceState, LightState};

/// Saturation used by the breathing animation.
pub const SATURATION: u8 = 200;

/// A device slot the render layer knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Light,
}

/// The declared appearance of the light, derived from state; breathing is the
/// schedule, not a color frame — time-driven rendering is the renderer's call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightAppearance {
    Off,
    Color(Rgb),
    Breathing(Breath),
}

/// The intended appearance of every known slot, derived from [`DeviceState`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotAppearance {
    Light(LightAppearance),
}

impl SlotAppearance {
    /// Which slot this appearance belongs to.
    pub fn slot(&self) -> Slot {
        match self {
            SlotAppearance::Light(_) => Slot::Light,
        }
    }
}

/// Appearance of the light under `state`, without time information.
pub fn light_appearance(state: LightState) -> LightAppearance {
    match state {
        LightState::Off => LightAppearance::Off,
        LightState::Solid { color } => LightAppearance::Color(color),
        LightState::Breath(breath) => LightAppearance::Breathing(breath),
    }
}

/// How a renderer wants to be driven after an appearance sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    /// Appearance settled; no callbacks until the next state change.
    Idle,
    /// The render layer must step this renderer every tick to animate.
    TimeDriven,
}

/// Pluggable binding between one device and the render layer: the controller
/// decides *what* to show (central state diff), the renderer answers *how*,
/// including whether it needs time-driven frames. Not `Send`: renderers stay in
/// the render task; cross-task registration wraps them in `Box<dyn Renderer +
/// Send>` at the app seam.
pub trait Renderer {
    fn slot(&self) -> Slot;

    /// A slot's appearance changed; apply it to the device.
    fn on_appearance(&mut self, appearance: SlotAppearance, now_ms: u32) -> Activity;

    /// One animation frame, called only while the renderer reported
    /// [`Activity::TimeDriven`].
    fn step(&mut self, now_ms: u32) -> Activity;
}

/// Central judge of the device surface: diffs successive [`DeviceState`]
/// snapshots and reports only the appearances that changed, via a callback the
/// render layer fans out to matching renderers. Allocation-free.
pub struct RenderController {
    last: Option<DeviceState>,
}

impl RenderController {
    pub const fn new() -> Self {
        Self { last: None }
    }

    /// Diff `state` against the last snapshot, invoking `notify` per changed
    /// slot; the first call reports every known slot. Returns whether any slot
    /// changed, so bindings can react once.
    pub fn reconcile(
        &mut self,
        state: &DeviceState,
        mut notify: impl FnMut(SlotAppearance),
    ) -> bool {
        let changed = match &self.last {
            None => true,
            Some(last) => last != state,
        };
        if changed {
            self.last = Some(state.clone());
            notify(SlotAppearance::Light(light_appearance(state.light)));
        }
        changed
    }

    /// Last synced appearance for `slot` (or `None` before the first sync), so a
    /// freshly registered renderer can catch up before the next diff.
    pub fn current(&self, slot: Slot) -> Option<SlotAppearance> {
        let state = self.last.as_ref()?;
        Some(match slot {
            Slot::Light => SlotAppearance::Light(light_appearance(state.light)),
        })
    }
}

impl Default for RenderController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOOT_BREATH: crate::state::Breath = crate::state::Breath {
        period_ms: 2_000,
        hue_period_ms: 8_000,
        max_brightness: 80,
    };

    fn state(light: LightState) -> DeviceState {
        DeviceState { light }
    }

    fn collect(ctl: &mut RenderController, s: &DeviceState) -> Vec<SlotAppearance> {
        let mut seen = Vec::new();
        ctl.reconcile(s, |a| seen.push(a));
        seen
    }

    #[test]
    fn first_reconcile_reports_boot_breathing() {
        let mut ctl = RenderController::new();
        let seen = collect(&mut ctl, &state(LightState::boot()));
        assert_eq!(
            seen,
            vec![SlotAppearance::Light(LightAppearance::Breathing(
                BOOT_BREATH
            ))]
        );
    }

    #[test]
    fn unchanged_state_stays_silent() {
        let mut ctl = RenderController::new();
        let boot = state(LightState::boot());
        collect(&mut ctl, &boot);
        assert_eq!(collect(&mut ctl, &boot), vec![]);
    }

    #[test]
    fn change_emits_new_appearance() {
        let mut ctl = RenderController::new();
        collect(&mut ctl, &state(LightState::boot()));
        let target = state(LightState::Solid {
            color: Rgb(1, 2, 3),
        });
        assert_eq!(
            collect(&mut ctl, &target),
            vec![SlotAppearance::Light(LightAppearance::Color(Rgb(1, 2, 3)))]
        );
    }

    #[test]
    fn off_emits_off_once() {
        let mut ctl = RenderController::new();
        collect(&mut ctl, &state(LightState::boot()));
        let off = state(LightState::Off);
        assert_eq!(
            collect(&mut ctl, &off),
            vec![SlotAppearance::Light(LightAppearance::Off)]
        );
        assert_eq!(collect(&mut ctl, &off), vec![]);
    }

    #[test]
    fn breathing_param_change_re_notifies() {
        let mut ctl = RenderController::new();
        collect(&mut ctl, &state(LightState::Breath(BOOT_BREATH)));
        let second = state(LightState::Breath(crate::state::Breath {
            period_ms: 1_000,
            hue_period_ms: 3_000,
            max_brightness: 40,
        }));
        let seen = collect(&mut ctl, &second);
        assert_eq!(seen.len(), 1);
        assert!(matches!(
            seen[0],
            SlotAppearance::Light(LightAppearance::Breathing(_))
        ));
    }

    #[test]
    fn solid_color_change_differs() {
        let mut ctl = RenderController::new();
        collect(
            &mut ctl,
            &state(LightState::Solid {
                color: Rgb(5, 5, 5),
            }),
        );
        let target = state(LightState::Solid {
            color: Rgb(9, 9, 9),
        });
        assert_eq!(
            collect(&mut ctl, &target),
            vec![SlotAppearance::Light(LightAppearance::Color(Rgb(9, 9, 9)))]
        );
    }

    #[test]
    fn current_reports_last_synced_appearance() {
        let mut ctl = RenderController::new();
        assert_eq!(ctl.current(Slot::Light), None);
        let target = state(LightState::Solid {
            color: Rgb(7, 6, 5),
        });
        collect(&mut ctl, &target);
        assert_eq!(
            ctl.current(Slot::Light),
            Some(SlotAppearance::Light(LightAppearance::Color(Rgb(7, 6, 5))))
        );
    }

    #[test]
    fn reconcile_reports_whether_state_changed() {
        let mut ctl = RenderController::new();
        let boot = state(LightState::boot());
        assert!(ctl.reconcile(&boot, |_| {}), "first sync is a change");
        assert!(!ctl.reconcile(&boot, |_| {}), "identical state is silent");
        let off = state(LightState::Off);
        assert!(ctl.reconcile(&off, |_| {}), "changed state is a change");
        assert!(!ctl.reconcile(&off, |_| {}), "settled state is silent");
    }

    #[test]
    fn light_appearance_maps_every_state() {
        assert_eq!(light_appearance(LightState::Off), LightAppearance::Off);
        assert_eq!(
            light_appearance(LightState::Solid {
                color: Rgb(1, 2, 3)
            }),
            LightAppearance::Color(Rgb(1, 2, 3))
        );
        assert!(matches!(
            light_appearance(LightState::Breath(crate::state::Breath {
                period_ms: 1,
                hue_period_ms: 2,
                max_brightness: 3
            })),
            LightAppearance::Breathing(_)
        ));
    }
}
