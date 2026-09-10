use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::input::{IntentBus, StateWatch};
use embassy_futures::select::select;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};
use iot_core::drivers::light::{Rgb, RgbLight, breathe, should_repaint};
use iot_core::render::{
    Activity, LightAppearance, RenderController, Renderer, SATURATION, Slot, SlotAppearance,
};
use iot_core::state::{Breath, DeviceManager, DeviceState};

/// Frame cadence of the render loop.
pub const STEP_MS: u32 = 20;

/// Capacity of the cross-task render bus.
pub const RENDER_BUS_CAPACITY: usize = 8;

/// Cross-task render messages: renderers register when their resource exists
/// (e.g. a connected Web client) and unregister when it disappears. The
/// render task is the single owner draining this bus.
pub enum RenderMsg {
    /// Cross-task renderers must be `Send`; in-task boot renderers (the light)
    /// register directly with [`Render::register`].
    Register(Box<dyn Renderer + Send>),
    /// P1: runtime renderers (Web/Audio) drop on disconnect.
    Unregister(RenderToken),
}

/// Opaque handle to a registered renderer, assigned by the render layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderToken(pub u64);

/// Cross-task render channel type. The render task consumes it via an explicit
/// argument (never a hidden global); producers (future Web/Audio renderers on
/// other tasks) send to [`RENDER_BUS`].
pub type RenderBus = Channel<CriticalSectionRawMutex, RenderMsg, RENDER_BUS_CAPACITY>;

/// Single shared render bus: storage for the cross-task renderer registry and
/// the producers' global entry point.
pub static RENDER_BUS: RenderBus = RenderBus::new();

struct Entry {
    token: RenderToken,
    activity: Activity,
    renderer: Box<dyn Renderer>,
}

/// Render-layer host: central controller, renderer registry, cross-task bus.
pub struct Render {
    controller: RenderController,
    renderers: Vec<Entry>,
    next_token: u64,
}

impl Render {
    pub const fn new() -> Self {
        Self {
            controller: RenderController::new(),
            renderers: Vec::new(),
            next_token: 0,
        }
    }

    pub fn register(&mut self, renderer: Box<dyn Renderer>, now_ms: u32) -> RenderToken {
        let token = RenderToken(self.next_token);
        self.next_token += 1;
        let mut entry = Entry {
            token,
            activity: Activity::Idle,
            renderer,
        };
        // Catch a fresh renderer up with the surface before the next diff.
        if let Some(appearance) = self.controller.current(entry.renderer.slot()) {
            entry.activity = entry.renderer.on_appearance(appearance, now_ms);
        }
        self.renderers.push(entry);
        token
    }

    pub fn unregister(&mut self, token: RenderToken) {
        if let Some(index) = self.renderers.iter().position(|e| e.token == token) {
            self.renderers.remove(index);
        }
    }

    /// One frame: reconcile `state` against the controller, fan changes out to
    /// matching renderers, then step time-driven animations. Returns `true`
    /// while any renderer still animates. `on_change` fires once per changed
    /// frame.
    pub fn tick(
        &mut self,
        bus: &RenderBus,
        state: &DeviceState,
        now_ms: u32,
        mut on_change: impl FnMut(&DeviceState),
    ) -> bool {
        self.drain_bus(bus, now_ms);
        let controller = &mut self.controller;
        let entries = &mut self.renderers;
        let changed = controller.reconcile(state, |appearance| {
            let slot = appearance.slot();
            for entry in entries.iter_mut() {
                if entry.renderer.slot() == slot {
                    entry.activity = entry.renderer.on_appearance(appearance, now_ms);
                }
            }
        });
        if changed {
            on_change(state);
        }
        let mut any = false;
        for entry in self.renderers.iter_mut() {
            if entry.activity == Activity::TimeDriven {
                entry.activity = entry.renderer.step(now_ms);
            }
            if entry.activity == Activity::TimeDriven {
                any = true;
            }
        }
        any
    }

    fn drain_bus(&mut self, bus: &RenderBus, now_ms: u32) {
        while let Ok(msg) = bus.try_receive() {
            match msg {
                RenderMsg::Register(renderer) => {
                    self.register(renderer, now_ms);
                }
                RenderMsg::Unregister(token) => self.unregister(token),
            }
        }
    }
}

impl Default for Render {
    fn default() -> Self {
        Self::new()
    }
}

/// Renderer for a physical light surface (the WS2812 LED on the DevKitC-1).
/// Breathing is time-driven: the render
/// layer steps it every tick to produce frames from the schedule.
pub struct LightRenderer<R: RgbLight> {
    light: R,
    last: Option<Rgb>,
    breath: Option<Breath>,
}

impl<R: RgbLight> LightRenderer<R> {
    pub fn new(light: R) -> Self {
        Self {
            light,
            last: None,
            breath: None,
        }
    }

    fn drive(&mut self, color: Rgb) {
        let step = self.light.repaint_step();
        let repaint = match self.last {
            None => true,
            Some(last) => should_repaint(last, color, step),
        };
        if repaint {
            self.last = Some(color);
            self.light.set_rgb(color);
        }
    }
}

impl<R: RgbLight> Renderer for LightRenderer<R> {
    fn slot(&self) -> Slot {
        Slot::Light
    }

    fn on_appearance(&mut self, appearance: SlotAppearance, now_ms: u32) -> Activity {
        match appearance {
            SlotAppearance::Light(LightAppearance::Off) => {
                self.breath = None;
                self.drive(Rgb(0, 0, 0));
                Activity::Idle
            }
            SlotAppearance::Light(LightAppearance::Color(color)) => {
                self.breath = None;
                self.drive(color);
                Activity::Idle
            }
            SlotAppearance::Light(LightAppearance::Breathing(breath)) => {
                self.breath = Some(breath);
                self.drive(light_frame(now_ms, breath));
                Activity::TimeDriven
            }
        }
    }

    fn step(&mut self, now_ms: u32) -> Activity {
        match self.breath {
            Some(breath) => {
                self.drive(light_frame(now_ms, breath));
                Activity::TimeDriven
            }
            None => Activity::Idle,
        }
    }
}

fn light_frame(now_ms: u32, breath: Breath) -> Rgb {
    breathe(
        now_ms,
        breath.period_ms,
        breath.hue_period_ms,
        breath.max_brightness,
        SATURATION,
    )
}

/// Render loop: drains intents into the manager, broadcasts state-on-change,
/// and parks on the intent/render buses once the surface settles instead of
/// busy-stepping.
pub async fn render_loop(
    intent_bus: &'static IntentBus,
    device_state: &'static StateWatch,
    render_bus: &'static RenderBus,
    mut render: Render,
    mut manager: DeviceManager,
) -> ! {
    let mut elapsed_ms: u32 = 0;
    let mut next_frame: Instant = Instant::now();
    loop {
        while let Ok(intent) = intent_bus.try_receive() {
            manager.apply_intent(intent);
        }
        let active = render.tick(render_bus, &manager.state(), elapsed_ms, |state| {
            device_state.sender().send(state.clone());
        });
        elapsed_ms = elapsed_ms.wrapping_add(STEP_MS);
        if active {
            // Absolute deadline: work between frames never piles drift onto
            // the animation clock.
            next_frame += Duration::from_millis(STEP_MS.into());
            Timer::at(next_frame).await;
        } else {
            // The wake is a readiness signal, not a receive: it never pops, so
            // the drain at the top of the next pass sees the message.
            select(intent_bus.ready_to_receive(), render_bus.ready_to_receive()).await;
            // Re-anchor so missed frames while parked do not replay as a burst
            // when the light resumes animating.
            next_frame = Instant::now();
        }
    }
}
