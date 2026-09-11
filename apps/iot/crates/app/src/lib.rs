#![no_std]

extern crate alloc;

use alloc::boxed::Box;

use embassy_futures::join::join;
use iot_core::drivers::board::{Board as BoardTrait, HasInput, HasLight};
use iot_core::state::DeviceManager;

pub mod input;
pub mod render;

use input::{DEVICE_STATE, INTENT_BUS, input_task};
use render::{LightRenderer, RENDER_BUS, Render, render_loop};

/// Application composition: takes the board's light and input sources,
/// registers the light renderer, and runs the two persistent tasks (input +
/// render). Generic over capabilities, so every board wiring the same
/// capabilities is served by this single copy.
pub async fn run<B>(mut board: B) -> !
where
    B: BoardTrait + HasLight + HasInput + 'static,
{
    let light = board.take_light().expect("board has a wired light");
    let sources = board.take_input().expect("board has wired input");

    let mut render = Render::new();
    render.register(Box::new(LightRenderer::new(light)), 0);
    let never = join(
        input_task(&INTENT_BUS, &DEVICE_STATE, sources),
        render_loop(
            &INTENT_BUS,
            &DEVICE_STATE,
            &RENDER_BUS,
            render,
            DeviceManager::new(),
        ),
    )
    .await;
    match never {}
}
