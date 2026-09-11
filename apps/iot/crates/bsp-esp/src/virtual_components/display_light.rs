use alloc::vec::Vec;
use iot_core::drivers::light::{Fill, Rgb, RgbLight, scale_brightness, vertical_brightness};

use crate::components::backlight::Backlight;
use crate::components::st7789::St7789;

/// Minimum per-channel color delta that warrants a full-frame repaint.
const REPAINT_STEP: u8 = 12;

/// ST7789 panel framed as an `RgbLight` surface.
pub struct DisplayLight {
    panel: St7789,
    frame: Vec<u8>,
    screen_color: Option<(Fill, Rgb)>,
    backlight: Backlight,
}

impl DisplayLight {
    /// Raise the backlight to full as part of bring-up; the LEDC PWM channel
    /// keeps the active-low pin pulled low (bright) until a renderer write.
    pub fn new(panel: St7789, mut backlight: Backlight) -> Self {
        backlight.set_level_pct(100);
        log::info!("[DISPLAY] backlight raised");
        Self {
            panel,
            frame: Vec::new(),
            screen_color: None,
            backlight,
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

    fn set_backlight(&mut self, level_pct: u8) {
        self.backlight.set_level_pct(level_pct);
    }

    fn set_fill(&mut self, fill: Fill, color: Rgb) {
        if self.screen_color == Some((fill, color)) {
            return;
        }
        self.screen_color = Some((fill, color));
        self.frame.resize(self.frame_bytes(), 0);

        let height = self.panel.height();
        let colors = |row: u16| match fill {
            Fill::Uniform => color,
            Fill::VerticalGradient => scale_brightness(color, vertical_brightness(row, height)),
        };
        let width = usize::from(self.panel.width());
        for (row, px) in self.frame.chunks_exact_mut(width * 2).enumerate() {
            let rgb565 = rgb565(colors(row as u16));
            let hi = (rgb565 >> 8) as u8;
            let lo = rgb565 as u8;
            for pixel in px.chunks_exact_mut(2) {
                pixel[0] = hi;
                pixel[1] = lo;
            }
        }

        if let Err(e) = self.panel.write_frame(&self.frame) {
            log::error!("[DISPLAY] frame write failed: {e:?}");
        }
    }
}

/// Pack an RGB color into ST7789 big-endian RGB565.
fn rgb565(color: Rgb) -> u16 {
    let r = color.0 as u16;
    let g = color.1 as u16;
    let b = color.2 as u16;
    ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3)
}
