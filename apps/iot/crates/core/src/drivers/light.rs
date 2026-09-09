#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// Abstraction over a light surface that accepts an RGB color.
pub trait RgbLight {
    fn set_rgb(&mut self, color: Rgb);
}

const FIXED_POINT: u64 = 1 << 16;

/// Smooth sin² brightness envelope scaled to `[0, max_brightness]`.
pub fn smooth_brightness(elapsed_ms: u32, period_ms: u32, max_brightness: u8) -> u8 {
    if max_brightness == 0 {
        return 0;
    }

    let period = u64::from(period_ms.max(1));
    let phase = u64::from(elapsed_ms) % period;
    let phase_fraction = phase * FIXED_POINT / period;

    (u64::from(max_brightness) * hann_weight(phase_fraction) / FIXED_POINT) as u8
}

/// Bhaskara approximation of sin²(π·u), fixed point at `FIXED_POINT`.
fn hann_weight(phase_fraction: u64) -> u64 {
    let parabola = phase_fraction * (FIXED_POINT - phase_fraction) / FIXED_POINT;
    let bhaskara_sin = 16 * FIXED_POINT * parabola / (5 * FIXED_POINT - 4 * parabola);
    bhaskara_sin * bhaskara_sin / FIXED_POINT
}

/// Hue on a 256-step wheel, advancing linearly and wrapping every `period_ms`.
pub fn hue_phase(elapsed_ms: u32, period_ms: u32) -> u8 {
    let period = u64::from(period_ms.max(1));
    let phase = u64::from(elapsed_ms) % period;
    let hue_tick = phase * 256 / period;
    hue_tick as u8
}

/// Convert HSV to RGB (256-step hue, 8-bit saturation/value).
pub fn hsv_to_rgb(hue: u8, saturation: u8, value: u8) -> Rgb {
    let sat = u32::from(saturation);
    let val = u32::from(value);
    let x = u32::from(hue) * 6;
    let region = x / 256;
    let remainder = x % 256;

    let floor = val * (255 - sat) / 255;
    let ascending = val * (255 - sat * (255 - remainder) / 256) / 255;
    let descending = val * (255 - sat * remainder / 256) / 255;

    match region {
        0 => Rgb(val as u8, ascending as u8, floor as u8),
        1 => Rgb(descending as u8, val as u8, floor as u8),
        2 => Rgb(floor as u8, val as u8, ascending as u8),
        3 => Rgb(floor as u8, descending as u8, val as u8),
        4 => Rgb(ascending as u8, floor as u8, val as u8),
        _ => Rgb(val as u8, floor as u8, descending as u8),
    }
}

/// Soft color-graduated breathing light at `elapsed_ms`.
pub fn breathe(
    elapsed_ms: u32,
    breath_period_ms: u32,
    hue_period_ms: u32,
    max_brightness: u8,
    saturation: u8,
) -> Rgb {
    let value = smooth_brightness(elapsed_ms, breath_period_ms, max_brightness);
    let hue = hue_phase(elapsed_ms, hue_period_ms);
    hsv_to_rgb(hue, saturation, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_brightness_starts_off() {
        assert_eq!(smooth_brightness(0, 2000, 255), 0);
        assert_eq!(smooth_brightness(2000, 2000, 255), 0);
    }

    #[test]
    fn smooth_brightness_peaks_at_midpoint() {
        assert_eq!(smooth_brightness(1000, 2000, 255), 255);
        assert_eq!(smooth_brightness(1000, 2000, 80), 80);
    }

    #[test]
    fn smooth_brightness_is_symmetric() {
        assert_eq!(
            smooth_brightness(500, 2000, 255),
            smooth_brightness(1500, 2000, 255)
        );
        assert_eq!(
            smooth_brightness(250, 2000, 255),
            smooth_brightness(1750, 2000, 255)
        );
        assert_eq!(
            smooth_brightness(333, 2000, 255),
            smooth_brightness(1667, 2000, 255)
        );
    }

    #[test]
    fn smooth_brightness_is_bounded_by_max() {
        for t in (0..=2000).step_by(13) {
            let v = smooth_brightness(t, 2000, 80);
            assert!(v <= 80, "t={t}: {v} > 80");
        }
    }

    #[test]
    fn smooth_brightness_rises_then_falls() {
        let rising: Vec<u8> = (0..=1000)
            .step_by(100)
            .map(|t| smooth_brightness(t, 2000, 255))
            .collect();
        for w in rising.windows(2) {
            assert!(w[0] <= w[1], "not monotonic rising: {:?}", rising);
        }
        let falling: Vec<u8> = (1000..=2000)
            .step_by(100)
            .map(|t| smooth_brightness(t, 2000, 255))
            .collect();
        for w in falling.windows(2) {
            assert!(w[0] >= w[1], "not monotonic falling: {:?}", falling);
        }
    }

    #[test]
    fn smooth_brightness_softens_trough_and_peak() {
        assert!(smooth_brightness(900, 2000, 255) > smooth_brightness(500, 2000, 255) / 2);
        assert!(smooth_brightness(100, 2000, 255) < 16);
        assert!(smooth_brightness(1900, 2000, 255) < 16);
    }

    #[test]
    fn smooth_brightness_never_panics_on_zero_period() {
        assert_eq!(smooth_brightness(0, 0, 255), 0);
        assert_eq!(smooth_brightness(123, 0, 255), 0);
        assert_eq!(smooth_brightness(123, 2000, 0), 0);
    }

    #[test]
    fn hue_phase_advances_and_wraps() {
        assert_eq!(hue_phase(0, 8000), 0);
        assert_eq!(hue_phase(2000, 8000), 64);
        assert_eq!(hue_phase(4000, 8000), 128);
        assert_eq!(hue_phase(7999, 8000), 255);
        assert_eq!(hue_phase(8000, 8000), 0);
    }

    #[test]
    fn hue_phase_never_panics_on_zero_period() {
        assert_eq!(hue_phase(123, 0), 0);
    }

    #[test]
    fn hsv_red_green_blue_primaries() {
        let red = hsv_to_rgb(0, 255, 255);
        assert_eq!(red.0, 255);
        assert!(red.1 <= 2);
        assert_eq!(red.2, 0);

        let green = hsv_to_rgb(85, 255, 255);
        assert!(green.0 <= 2);
        assert_eq!(green.1, 255);
        assert_eq!(green.2, 0);

        let blue = hsv_to_rgb(170, 255, 255);
        assert!(blue.0 <= 2);
        assert!(blue.1 <= 4);
        assert_eq!(blue.2, 255);
    }

    #[test]
    fn hsv_gray_when_unsaturated() {
        assert_eq!(hsv_to_rgb(37, 0, 200), Rgb(200, 200, 200));
        assert_eq!(hsv_to_rgb(200, 0, 80), Rgb(80, 80, 80));
    }

    #[test]
    fn hsv_black_when_value_zero() {
        assert_eq!(hsv_to_rgb(100, 255, 0), Rgb(0, 0, 0));
    }

    #[test]
    fn hsv_wheel_is_continuous() {
        let a = hsv_to_rgb(0, 255, 255);
        let b = hsv_to_rgb(255, 255, 255);
        assert_eq!(a.0, 255);
        assert_eq!(b.0, 255);
        assert!(a.1 <= 2 && b.1 <= 2);
        assert!(a.2 <= 2 && b.2 <= 8);
    }

    #[test]
    fn breathe_composes_brightness_and_hue() {
        let color = breathe(1000, 2000, 8000, 128, 255);
        assert_eq!(color.0, 128);
        assert!(color.1 > 0 && color.1 < 128);
        assert_eq!(color.2, 0);
    }

    #[test]
    fn breathe_starts_off() {
        let color = breathe(0, 2000, 8000, 80, 200);
        assert_eq!(color, Rgb(0, 0, 0));
    }

    #[test]
    fn breathe_never_panics_on_zero_periods() {
        let color = breathe(123, 0, 0, 80, 200);
        assert_eq!(color, Rgb(0, 0, 0));
    }
}
