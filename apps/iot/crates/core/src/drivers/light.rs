#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// How a color is painted across a light surface. The renderer picks the
/// fill; surfaces without spatial resolution (a single LED) map any fill to
/// the same color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    /// The whole surface carries the same color.
    Uniform,
    /// A vertical brightness ramp, full at the top fading to black at the
    /// bottom, over a constant hue/saturation.
    VerticalGradient,
}

/// Abstraction over a light surface that accepts an RGB color.
pub trait RgbLight {
    /// Paints the surface flat.
    fn set_rgb(&mut self, color: Rgb) {
        self.set_fill(Fill::Uniform, color);
    }

    /// Paints the surface in the chosen [`Fill`].
    fn set_fill(&mut self, fill: Fill, color: Rgb);

    /// Drives a backlight channel to a percentage of full brightness. Surfaces
    /// with no separate backlight (a bare WS2812) keep the default no-op.
    fn set_backlight(&mut self, _level_pct: u8) {}

    /// Minimum per-channel delta that warrants a repaint. Slow-refresh
    /// surfaces (panel RAM rewritten over SPI) return a step > 0 to skip
    /// imperceptible drift; instant LEDs keep the default 0.
    fn repaint_step(&self) -> u8 {
        0
    }
}

const FIXED_POINT: u64 = 1 << 16;

/// Smooth sin² brightness envelope scaled to `[min_brightness, max_brightness]`.
/// A nonzero floor keeps the trough lit instead of fading fully to black.
pub fn smooth_brightness(
    elapsed_ms: u32,
    period_ms: u32,
    min_brightness: u8,
    max_brightness: u8,
) -> u8 {
    let min = u64::from(min_brightness.min(max_brightness));
    let scale = u64::from(max_brightness) - min;
    if scale == 0 {
        return min as u8;
    }

    let period = u64::from(period_ms.max(1));
    let phase = u64::from(elapsed_ms) % period;
    let phase_fraction = phase * FIXED_POINT / period;

    (min + scale * hann_weight(phase_fraction) / FIXED_POINT) as u8
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

/// Max color waypoints a `group` array can carry; unused slots pad with `0`.
pub const GROUP_CAPACITY: usize = 7;

/// Scale every `color` channel toward black by `brightness`/255.
pub fn scale_brightness(color: Rgb, brightness: u8) -> Rgb {
    let scale = u16::from(brightness);
    let scale_channel = |c: u8| (u16::from(c) * scale / 255) as u8;
    Rgb(
        scale_channel(color.0),
        scale_channel(color.1),
        scale_channel(color.2),
    )
}

/// Per-row brightness scale for [`Fill::VerticalGradient`]: `255` at the top
/// row fading to `0` at the bottom.
pub fn vertical_brightness(row: u16, height: u16) -> u8 {
    if height <= 1 {
        return 255;
    }
    let denominator = u32::from(height) - 1;
    let numerator = denominator - u32::from(row).min(u32::from(height) - 1);
    (255u32 * numerator / denominator) as u8
}

/// Soft color-graduated breathing light at `elapsed_ms`. The hue path is picked
/// by `group_len`, see `group_hue`; `base_hue`/`hue_span` drive the sweep when
/// the group is unused. `hue_span == 0` holds a static color, making standalone
/// a parameter value rather than a branch.
#[allow(clippy::too_many_arguments)]
pub fn breathe(
    elapsed_ms: u32,
    breath_period_ms: u32,
    hue_period_ms: u32,
    hue_span: u8,
    min_brightness: u8,
    max_brightness: u8,
    saturation: u8,
    base_hue: u8,
    group: [u8; GROUP_CAPACITY],
    group_len: u8,
) -> Rgb {
    let value = smooth_brightness(elapsed_ms, breath_period_ms, min_brightness, max_brightness);
    let hue = group_hue(
        elapsed_ms,
        hue_period_ms,
        hue_span,
        base_hue,
        group,
        group_len,
    );
    hsv_to_rgb(hue, saturation, value)
}

/// Hue at `elapsed_ms` along the configured path:
/// `group_len == 0` walks the `hue`/`hue_span` sweep (`0` span holds
/// `base_hue`); `group_len == 1` holds `group[0]`; `group_len >= 2` rotates
/// through the waypoints, walking each adjacent pair over
/// `hue_period_ms / group_len` and wrapping the last back to the first.
pub fn group_hue(
    elapsed_ms: u32,
    hue_period_ms: u32,
    hue_span: u8,
    base_hue: u8,
    group: [u8; GROUP_CAPACITY],
    group_len: u8,
) -> u8 {
    match group_len {
        0 => u8::wrapping_add(base_hue, hue_offset(hue_span, hue_period_ms, elapsed_ms)),
        1 => group[0],
        _ => {
            let leg_ms = (hue_period_ms / u32::from(group_len)).max(1);
            let cycle_ms = leg_ms * u32::from(group_len);
            let t = elapsed_ms % cycle_ms;
            let waypoint = (t / leg_ms) as usize;
            let segment_ms = t % leg_ms;
            let fraction = u64::from(segment_ms) * 256 / u64::from(leg_ms);
            let from = i32::from(group[waypoint]);
            let to = i32::from(group[(waypoint + 1) % usize::from(group_len)]);
            // Shortest signed arc around the 256-step wheel, so a leg crossing
            // hue 0 (e.g. 250→16) walks through the boundary instead of the
            // long way around.
            let delta = ((to - from + 384) % 256) - 128;
            let hue = from + delta * (fraction as i32) / 256;
            hue.rem_euclid(256) as u8
        }
    }
}

/// Hue sweep offset in `[0, hue_span]`, replaying the span over `hue_period_ms`.
/// A zero span or a degenerate period holds the offset still.
fn hue_offset(hue_span: u8, hue_period_ms: u32, elapsed_ms: u32) -> u8 {
    if hue_span == 0 || hue_period_ms == 0 {
        return 0;
    }
    (u32::from(hue_span) * u32::from(hue_phase(elapsed_ms, hue_period_ms)) / 256) as u8
}

/// Backlight duty for the breathing envelope at `elapsed_ms`: the same
/// `[min_brightness, max_brightness]` wave the pixels ride, remapped to
/// `[floor_pct, 100]` percent so the panel follows the pixels instead of
/// switching binary on/off. A degenerate flat envelope holds full brightness.
pub fn backlight_level(
    elapsed_ms: u32,
    period_ms: u32,
    min_brightness: u8,
    max_brightness: u8,
    floor_pct: u8,
) -> u8 {
    let min = u32::from(min_brightness.min(max_brightness));
    let max = u32::from(max_brightness);
    let span = max - min;
    if span == 0 {
        return 100;
    }
    let value = u32::from(smooth_brightness(
        elapsed_ms,
        period_ms,
        min_brightness,
        max_brightness,
    ));
    let room = 100 - u32::from(floor_pct.min(100));
    (u32::from(floor_pct.min(100)) + (value - min) * room / span) as u8
}

/// Whether `next` differs from `last` enough to repaint a slow refresh surface.
/// Any channel moving by at least `step` counts as a visible change; `step == 0`
/// repaints on every difference, so an Off-to-idle transition always goes out.
pub fn should_repaint(last: Rgb, next: Rgb, step: u8) -> bool {
    if last == next {
        return false;
    }
    let delta = |a: u8, b: u8| u8::abs_diff(a, b);
    delta(last.0, next.0) >= step || delta(last.1, next.1) >= step || delta(last.2, next.2) >= step
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_brightness_starts_off() {
        assert_eq!(smooth_brightness(0, 2000, 0, 255), 0);
        assert_eq!(smooth_brightness(2000, 2000, 0, 255), 0);
    }

    #[test]
    fn smooth_brightness_peaks_at_midpoint() {
        assert_eq!(smooth_brightness(1000, 2000, 0, 255), 255);
        assert_eq!(smooth_brightness(1000, 2000, 0, 80), 80);
    }

    #[test]
    fn smooth_brightness_is_symmetric() {
        assert_eq!(
            smooth_brightness(500, 2000, 0, 255),
            smooth_brightness(1500, 2000, 0, 255)
        );
        assert_eq!(
            smooth_brightness(250, 2000, 0, 255),
            smooth_brightness(1750, 2000, 0, 255)
        );
        assert_eq!(
            smooth_brightness(333, 2000, 0, 255),
            smooth_brightness(1667, 2000, 0, 255)
        );
    }

    #[test]
    fn smooth_brightness_is_bounded_by_max() {
        for t in (0..=2000).step_by(13) {
            let v = smooth_brightness(t, 2000, 0, 80);
            assert!(v <= 80, "t={t}: {v} > 80");
        }
    }

    #[test]
    fn smooth_brightness_rises_then_falls() {
        let rising: Vec<u8> = (0..=1000)
            .step_by(100)
            .map(|t| smooth_brightness(t, 2000, 0, 255))
            .collect();
        for w in rising.windows(2) {
            assert!(w[0] <= w[1], "not monotonic rising: {:?}", rising);
        }
        let falling: Vec<u8> = (1000..=2000)
            .step_by(100)
            .map(|t| smooth_brightness(t, 2000, 0, 255))
            .collect();
        for w in falling.windows(2) {
            assert!(w[0] >= w[1], "not monotonic falling: {:?}", falling);
        }
    }

    #[test]
    fn smooth_brightness_softens_trough_and_peak() {
        assert!(smooth_brightness(900, 2000, 0, 255) > smooth_brightness(500, 2000, 0, 255) / 2);
        assert!(smooth_brightness(100, 2000, 0, 255) < 16);
        assert!(smooth_brightness(1900, 2000, 0, 255) < 16);
    }

    #[test]
    fn smooth_brightness_never_panics_on_zero_period() {
        assert_eq!(smooth_brightness(0, 0, 0, 255), 0);
        assert_eq!(smooth_brightness(123, 0, 0, 255), 0);
        assert_eq!(smooth_brightness(123, 2000, 0, 0), 0);
    }

    #[test]
    fn smooth_brightness_floor_keeps_trough_lit() {
        // A nonzero floor means the trough never reaches black.
        assert_eq!(smooth_brightness(0, 2000, 24, 80), 24);
        assert_eq!(smooth_brightness(2000, 2000, 24, 80), 24);
        assert_eq!(smooth_brightness(1000, 2000, 24, 80), 80);
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
        let color = breathe(
            1000,
            2000,
            8000,
            255,
            0,
            128,
            255,
            0,
            [0, 0, 0, 0, 0, 0, 0],
            0,
        );
        assert_eq!(color.0, 128);
        assert!(color.1 > 0 && color.1 < 128);
        assert_eq!(color.2, 0);
    }

    #[test]
    fn breathe_starts_off() {
        let color = breathe(0, 2000, 8000, 255, 0, 80, 200, 0, [0, 0, 0, 0, 0, 0, 0], 0);
        assert_eq!(color, Rgb(0, 0, 0));
    }

    #[test]
    fn breathe_never_panics_on_zero_periods() {
        let color = breathe(123, 0, 0, 255, 0, 80, 200, 0, [0, 0, 0, 0, 0, 0, 0], 0);
        assert_eq!(color, Rgb(0, 0, 0));
    }

    #[test]
    fn breathe_with_zero_span_holds_base_hue() {
        // Symmetric envelope times give equal value; with a zero span the hue
        // must not drift, so the two frames are identical.
        let rising = breathe(
            500,
            2000,
            8000,
            0,
            0,
            128,
            255,
            170,
            [0, 0, 0, 0, 0, 0, 0],
            0,
        );
        let falling = breathe(
            1500,
            2000,
            8000,
            0,
            0,
            128,
            255,
            170,
            [0, 0, 0, 0, 0, 0, 0],
            0,
        );
        assert_eq!(rising, falling);
        // Blue (hue 170) holds without any sweep mixing in another channel.
        assert!(rising.2 > rising.0, "blue dominates at hue 170");
    }

    #[test]
    fn breathe_sweep_starts_at_base_hue() {
        // At t=0 the sweep offset is 0, so the output is the base hue only.
        let color = breathe(
            0,
            2000,
            8000,
            100,
            0,
            128,
            255,
            42,
            [0, 0, 0, 0, 0, 0, 0],
            0,
        );
        let static_at_42 = breathe(0, 2000, 8000, 0, 0, 128, 255, 42, [0, 0, 0, 0, 0, 0, 0], 0);
        assert_eq!(color, static_at_42);
    }

    #[test]
    fn breathe_sweep_span_scales_offset() {
        // Half the span at the same clock position offsets half as far in hue.
        let full = breathe(
            1000,
            2000,
            8000,
            255,
            0,
            128,
            255,
            0,
            [0, 0, 0, 0, 0, 0, 0],
            0,
        );
        let half = breathe(
            1000,
            2000,
            8000,
            128,
            0,
            128,
            255,
            0,
            [0, 0, 0, 0, 0, 0, 0],
            0,
        );
        assert_ne!(full, half);
        // Both sit at value 128 in the same hue region (floor stays black);
        // the wider span advances further, so its gradient channel is stronger.
        assert_eq!(full.0, 128);
        assert_eq!(half.0, 128);
        assert_eq!(full.2, 0);
        assert_eq!(half.2, 0);
        assert!(full.1 > half.1);
    }

    #[test]
    fn breathe_saturation_shrinks_color_value() {
        let full = breathe(
            1000,
            2000,
            8000,
            0,
            0,
            128,
            255,
            0,
            [0, 0, 0, 0, 0, 0, 0],
            0,
        );
        let muted = breathe(1000, 2000, 8000, 0, 0, 128, 64, 0, [0, 0, 0, 0, 0, 0, 0], 0);
        // Hue 0 is pure red at full saturation; lower saturation fades the
        // other channels in toward gray, lifting them off zero.
        assert_eq!(full.0, 128);
        assert_eq!(muted.0, 128);
        assert!(muted.1 > full.1, "lower saturation lifts the green channel");
        assert!(muted.2 > full.2, "lower saturation lifts the blue channel");
    }

    #[test]
    fn group_rotation_interpolates_between_waypoints() {
        // group [16, 32, 100]/3, hue_period 3000 → legs of 1000ms.
        // At the midpoint of the first leg the hue is halfway 16→32.
        let halfway = group_hue(500, 3000, 255, 0, [16, 32, 100, 0, 0, 0, 0], 3);
        assert_eq!(halfway, 24);
        // The full frame carries the same hue through the brightness envelope.
        let color = breathe(
            1000,
            2000,
            3000,
            255,
            0,
            128,
            255,
            0,
            [16, 32, 100, 0, 0, 0, 0],
            3,
        );
        assert_eq!(
            color,
            hsv_to_rgb(32, 255, smooth_brightness(1000, 2000, 0, 128))
        );
    }

    #[test]
    fn group_rotation_wraps_last_waypoint_to_head() {
        // t=2500 of a 3000ms cycle sits on the final leg (100→16 wrap);
        // 2500-2000=500ms into it, halfway to 16.
        let halfway = group_hue(2500, 3000, 255, 0, [16, 32, 100, 0, 0, 0, 0], 3);
        assert_eq!(halfway, 58);
        let head = group_hue(3000, 3000, 255, 0, [16, 32, 100, 0, 0, 0, 0], 3);
        assert_eq!(head, 16);
    }

    #[test]
    fn group_rotation_len_one_holds_static_color() {
        let a = group_hue(0, 8000, 255, 0, [170, 0, 0, 0, 0, 0, 0], 1);
        let b = group_hue(7000, 8000, 255, 0, [170, 0, 0, 0, 0, 0, 0], 1);
        assert_eq!(a, b);
        assert_eq!(a, 170);
    }

    #[test]
    fn group_rotation_handles_hue_wheel_wrap() {
        // 250→16 crosses hue 0 the short way; a quarter (f=64) in lands at
        // 255, the midpoint at 5, the end at 16.
        assert_eq!(
            group_hue(125, 1000, 255, 0, [250, 16, 0, 0, 0, 0, 0], 2),
            255
        );
        assert_eq!(group_hue(250, 1000, 255, 0, [250, 16, 0, 0, 0, 0, 0], 2), 5);
        assert_eq!(
            group_hue(500, 1000, 255, 0, [250, 16, 0, 0, 0, 0, 0], 2),
            16
        );
    }

    #[test]
    fn group_len_zero_keeps_sweep_path() {
        let swept = breathe(
            1234,
            2000,
            8000,
            128,
            0,
            128,
            200,
            42,
            [0, 0, 0, 0, 0, 0, 0],
            0,
        );
        let swept_hue = u8::wrapping_add(42, hue_offset(128, 8000, 1234));
        assert_eq!(
            swept,
            hsv_to_rgb(swept_hue, 200, smooth_brightness(1234, 2000, 0, 128))
        );
    }

    #[test]
    fn scale_brightness_full_is_identity() {
        assert_eq!(scale_brightness(Rgb(200, 100, 50), 255), Rgb(200, 100, 50));
    }

    #[test]
    fn scale_brightness_midpoint_halves_channels() {
        assert_eq!(scale_brightness(Rgb(200, 100, 50), 128).0, 100);
        assert_eq!(scale_brightness(Rgb(100, 0, 0), 128).0, 50);
    }

    #[test]
    fn scale_brightness_zero_is_black() {
        assert_eq!(scale_brightness(Rgb(255, 255, 255), 0), Rgb(0, 0, 0));
    }

    #[test]
    fn vertical_brightness_fades_top_to_bottom() {
        let height = 5;
        assert_eq!(vertical_brightness(0, height), 255);
        assert_eq!(vertical_brightness(height - 1, height), 0);
        assert_eq!(vertical_brightness(2, height), 127);
        let rows: Vec<u8> = (0..height)
            .map(|r| vertical_brightness(r, height))
            .collect();
        for w in rows.windows(2) {
            assert!(w[0] > w[1], "not strictly fading: {:?}", rows);
        }
    }

    #[test]
    fn vertical_brightness_single_row_stays_full() {
        assert_eq!(vertical_brightness(0, 1), 255);
    }

    #[test]
    fn vertical_brightness_midpoint_for_even_height() {
        // Even heights have no exact mid row: the two center rows straddle 128.
        let height = 4;
        assert_eq!(vertical_brightness(1, height), 170);
        assert_eq!(vertical_brightness(2, height), 85);
    }

    #[test]
    fn should_repaint_skips_identical_color() {
        assert!(!should_repaint(Rgb(10, 20, 30), Rgb(10, 20, 30), 1));
    }

    #[test]
    fn should_repaint_skips_sub_step_drift() {
        assert!(!should_repaint(Rgb(10, 20, 30), Rgb(19, 21, 28), 12));
        assert!(!should_repaint(Rgb(10, 20, 30), Rgb(10, 31, 30), 12));
        assert!(!should_repaint(Rgb(10, 20, 30), Rgb(10, 20, 41), 12));
    }

    #[test]
    fn should_repaint_triggers_at_step_or_more() {
        assert!(should_repaint(Rgb(10, 20, 30), Rgb(22, 20, 30), 12));
        assert!(should_repaint(Rgb(10, 20, 30), Rgb(10, 32, 30), 12));
        assert!(should_repaint(Rgb(10, 20, 30), Rgb(10, 20, 42), 12));
        assert!(should_repaint(Rgb(0, 0, 0), Rgb(255, 255, 255), 12));
    }

    #[test]
    fn should_repaint_step_zero_repaints_on_any_difference() {
        assert!(should_repaint(Rgb(10, 20, 30), Rgb(10, 21, 30), 0));
        assert!(!should_repaint(Rgb(10, 21, 30), Rgb(10, 21, 30), 0));
    }

    #[test]
    fn backlight_level_tracks_the_envelope_above_a_floor() {
        // Trough maps to the floor, peak to full brightness.
        assert_eq!(backlight_level(0, 2000, 0, 255, 12), 12);
        assert_eq!(backlight_level(1000, 2000, 0, 255, 12), 100);
        // The floor shifts the whole curve up but never passes 100.
        assert_eq!(backlight_level(0, 2000, 0, 255, 20), 20);
        assert_eq!(backlight_level(1000, 2000, 0, 255, 20), 100);
    }

    #[test]
    fn backlight_level_flat_envelope_holds_full_brightness() {
        assert_eq!(backlight_level(0, 2000, 80, 80, 12), 100);
        assert_eq!(backlight_level(1337, 5, 40, 40, 12), 100);
    }

    #[test]
    fn backlight_level_floor_never_exceeds_one_hundred() {
        assert_eq!(backlight_level(0, 2000, 0, 255, 150), 100);
    }
}
