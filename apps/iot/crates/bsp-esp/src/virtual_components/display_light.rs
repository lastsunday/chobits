use alloc::vec::Vec;
use esp_hal::gpio::Output;
use iot_core::drivers::light::{Rgb, RgbLight};

use crate::components::st7789::St7789;

/// Minimum per-channel color delta that warrants a full-frame repaint.
const REPAINT_STEP: u8 = 12;

/// ST7789 panel framed as an `RgbLight` surface.
pub struct DisplayLight {
    panel: St7789,
    frame: Vec<u8>,
    frame_color: Option<Rgb>,
}

impl DisplayLight {
    /// Raise the backlight as part of bring-up; the pin stays driven after
    /// `new` returns.
    pub fn new(panel: St7789, mut bl: Output<'static>) -> Self {
        bl.set_high();
        Self {
            panel,
            frame: Vec::new(),
            frame_color: None,
        }
    }

    fn frame_bytes(&self) -> usize {
        usize::from(self.panel.width()) * usize::from(self.panel.height()) * 2
    }
}

impl RgbLight for DisplayLight {
    fn repaint_step(&self) -> u8 {
        REPAINT_STEP
    }

    fn set_rgb(&mut self, color: Rgb) {
        let r = color.0 as u16;
        let g = color.1 as u16;
        let b = color.2 as u16;
        let rgb565 = ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3);
        let hi = (rgb565 >> 8) as u8;
        let lo = rgb565 as u8;

        if self.frame_color != Some(color) {
            self.frame_color = Some(color);
            self.frame.resize(self.frame_bytes(), 0);
            for px in self.frame.chunks_exact_mut(2) {
                px[0] = hi;
                px[1] = lo;
            }
        }
        self.panel.write_frame(&self.frame).unwrap();
    }
}
