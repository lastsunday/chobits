#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use embassy_executor::Spawner;
use iot_app::input::{DEVICE_STATE, INTENT_BUS, input_task};
use iot_app::render::{LightRenderer, RENDER_BUS, Render, render_loop};
use iot_core::drivers::board::{Board as BoardTrait, HasInput, HasLight};
use iot_core::state::DeviceManager;

#[cfg(feature = "esp32c6-devkitc-1")]
type Board = iot_bsp_esp::Board<'static>;

/// Heap for the pluggable renderer registry and other runtime allocation.
#[global_allocator]
static HEAP: embedded_alloc::Heap = embedded_alloc::Heap::empty();

static mut HEAP_MEM: [u8; 32 * 1024] = [0; 32 * 1024];

#[cfg(feature = "esp32c6")]
#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    // SAFETY: called exactly once before any allocation; the region is a
    // private static never aliased elsewhere.
    unsafe {
        HEAP.init(
            core::ptr::addr_of_mut!(HEAP_MEM) as usize,
            core::mem::size_of::<[u8; 32 * 1024]>(),
        );
    }

    let peripherals = iot_chip_esp::chip_init();
    iot_chip_esp::init_logging();
    log::info!("[IOT] boot ok");

    let (board, timg0, from_cpu_intr) = Board::new(peripherals).expect("failed to init board");

    iot_chip_esp::start_rtos(timg0.timer0, from_cpu_intr);
    run(board).await
}

/// Application composition: takes the board's light and input sources,
/// registers the light renderer, and runs the two persistent tasks (input +
/// render). Generic over capabilities, so every board wiring the same
/// capabilities is served by this single copy.
async fn run<B>(mut board: B) -> !
where
    B: BoardTrait + HasLight + HasInput + 'static,
{
    let light = board.take_light().expect("board has a wired light");
    let sources = board.take_input().expect("board has wired input");

    let mut render = Render::new();
    render.register(Box::new(LightRenderer::new(light)), 0);
    let never = embassy_futures::join::join(
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
