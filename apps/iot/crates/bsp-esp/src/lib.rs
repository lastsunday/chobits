#![no_std]

extern crate alloc;

#[cfg(feature = "esp32c6-devkitc-1")]
pub mod esp32c6_devkitc_1;

#[cfg(feature = "esp32c6-devkitc-1")]
pub use esp32c6_devkitc_1::{Board, BootButton, Ws2812RgbLed};
