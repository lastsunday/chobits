#![no_std]

extern crate alloc;

#[cfg(feature = "esp32c6-devkitc-1")]
pub mod esp32c6_devkitc_1;

#[cfg(feature = "esp32c6-devkitc-1")]
pub use esp32c6_devkitc_1::{Board, PullButton, Ws2812RgbLed};

#[cfg(feature = "lckfb-szpi-esp32s3")]
pub mod lckfb_szpi_esp32s3;

#[cfg(feature = "lckfb-szpi-esp32s3")]
pub use lckfb_szpi_esp32s3::{Board, DisplayLight, PullButton};

/// Board-agnostic real components: chip drivers parameterized over the bus /
/// pin instances handed in by the board wiring.
pub mod components;

#[cfg(feature = "display-light")]
pub mod virtual_components;
