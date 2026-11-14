//! Animations over a custom frame. The pad's own effects can't run on a
//! frame of our own, so the daemon draws these itself: `render` turns a
//! frame, an animation and a moment into the frame to show then. The window
//! runs the same code for its preview.

use crate::lighting::{FRAME_COLUMNS, FRAME_ROWS, ROW_KEYS};
use crate::types::LightFrame;
use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;

pub const SPEED_MIN: u8 = 1;
pub const SPEED_MAX: u8 = 10;
pub const SPEED_DEFAULT: u8 = 5;
/// How often the daemon sends a new frame: about 30 a second.
pub const FRAME_INTERVAL_MS: u64 = 33;
/// Presses older than this no longer change any frame.
pub const PRESS_MEMORY_SECS: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnimationKind {
    /// The whole pad fades out and back in.
    Breathe,
    /// Two quick beats, then a rest.
    Heartbeat,
    /// A bright band sweeps left to right.
    Wave,
    /// A bright band runs down the rows.
    Rows,
    /// The row colours roll down the pad.
    Scroll,
    /// Keys sparkle one by one.
    Twinkle,
    /// Every key flickers like a candle.
    Flicker,
    /// The colours turn around the colour wheel, flowing across the pad.
    HueDrift,
    /// Drops of light fall down the columns.
    Rain,
    /// The pad rests dim and a pressed key flares up.
    Reactive,
    /// A ring of light spreads out from a pressed key.
    Ripple,
}

impl AnimationKind {
    pub const ALL: [AnimationKind; 11] = [
        AnimationKind::Breathe,
        AnimationKind::Heartbeat,
        AnimationKind::Wave,
        AnimationKind::Rows,
        AnimationKind::Scroll,
        AnimationKind::Twinkle,
        AnimationKind::Flicker,
        AnimationKind::HueDrift,
        AnimationKind::Rain,
        AnimationKind::Reactive,
        AnimationKind::Ripple,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Breathe => "breathe",
            Self::Heartbeat => "heartbeat",
            Self::Wave => "wave",
            Self::Rows => "rows",
            Self::Scroll => "scroll",
            Self::Twinkle => "twinkle",
            Self::Flicker => "flicker",
            Self::HueDrift => "hue drift",
            Self::Rain => "rain",
            Self::Reactive => "reactive",
            Self::Ripple => "ripple",
        }
    }

    /// Whether key presses drive it.
    pub fn uses_presses(self) -> bool {
        matches!(self, Self::Reactive | Self::Ripple)
    }
}

fn default_speed() -> u8 {
    SPEED_DEFAULT
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animation {
    pub kind: AnimationKind,
    #[serde(default = "default_speed")]
    pub speed: u8,
}

impl Animation {
    pub fn new(kind: AnimationKind) -> Self {
        Self {
            kind,
            speed: SPEED_DEFAULT,
        }
    }

    pub fn is_valid(&self) -> bool {
        (SPEED_MIN..=SPEED_MAX).contains(&self.speed)
    }

    /// How many times faster than the base pace it runs: from 0.35 at the
    /// slowest to about 3 at the fastest.
    fn rate(&self) -> f32 {
        let step = f32::from(self.speed.clamp(SPEED_MIN, SPEED_MAX) - SPEED_MIN);
        0.35 + step * 0.3
    }
}

/// A key press, as the animations see it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Press {
    pub row: usize,
    pub column: usize,
    /// Seconds since the key went down.
    pub age: f32,
}

/// The middle of a key in single-key units, `x` across and `y` down. Each
/// row's first key is double, and so is the last key of the bottom two rows.
pub fn key_center(row: usize, column: usize) -> (f32, f32) {
    let x = if column == 0 {
        1.0
    } else if row >= 3 && column + 1 == ROW_KEYS[row.min(FRAME_ROWS - 1)] {
        column as f32 + 2.0
    } else {
        column as f32 + 1.5
    };
    (x, row as f32 + 0.5)
}

const PAD_WIDTH_UNITS: f32 = 7.0;
const WHITE: [u8; 3] = [255, 255, 255];

/// The frame to show `t` seconds into `animation` over `base`.
pub fn render(base: &LightFrame, animation: Animation, t: f32, presses: &[Press]) -> LightFrame {
    let rate = animation.rate();
    let time = t * rate;
    let mut out = vec![vec![[0u8; 3]; FRAME_COLUMNS]; FRAME_ROWS];
    for (row, cells) in out.iter_mut().enumerate() {
        for (column, cell) in cells.iter_mut().enumerate() {
            let rgb = base_cell(base, row, column);
            if rgb == [0, 0, 0] && !animation.kind.uses_presses() {
                continue;
            }
            *cell = match animation.kind {
                AnimationKind::Breathe => {
                    let wave = 0.5 - 0.5 * (TAU * time / 4.0).cos();
                    scale(rgb, 0.12 + 0.88 * wave)
                }
                AnimationKind::Heartbeat => {
                    let phase = (time / 1.6).fract();
                    let beat = pulse(phase, 0.0, 0.06).max(0.7 * pulse(phase, 0.22, 0.06));
                    scale(rgb, 0.2 + 0.8 * beat)
                }
                AnimationKind::Wave => {
                    let (x, _) = key_center(row, column);
                    let phase = x / PAD_WIDTH_UNITS - time * 0.5;
                    scale(rgb, 0.2 + 0.8 * (0.5 + 0.5 * (TAU * phase).cos()))
                }
                AnimationKind::Rows => {
                    let phase = row as f32 / FRAME_ROWS as f32 - time * 0.4;
                    scale(rgb, 0.2 + 0.8 * (0.5 + 0.5 * (TAU * phase).cos()))
                }
                AnimationKind::Scroll => scroll_cell(base, row, column, time * 0.6),
                AnimationKind::Twinkle => {
                    let seed = key_seed(row, column);
                    let period = 1.6 + 2.2 * unit(seed);
                    let phase = (time / period + unit(seed ^ 0x9e37)).fract();
                    let sparkle = (TAU * phase).sin().max(0.0).powi(10);
                    mix(scale(rgb, 0.35 + 0.65 * sparkle), WHITE, 0.35 * sparkle)
                }
                AnimationKind::Flicker => {
                    let n = smooth_noise(key_seed(row, column), time * 6.0);
                    scale(rgb, 0.55 + 0.45 * n)
                }
                AnimationKind::HueDrift => {
                    let (x, y) = key_center(row, column);
                    let turn = time * 0.08 + (x + y) * 0.03;
                    rotate_hue(rgb, turn)
                }
                AnimationKind::Rain => rain_cell(rgb, row, column, time),
                AnimationKind::Reactive => reactive_cell(rgb, row, column, presses, rate),
                AnimationKind::Ripple => ripple_cell(rgb, row, column, presses, rate),
            };
        }
    }
    out
}

fn base_cell(base: &LightFrame, row: usize, column: usize) -> [u8; 3] {
    base.get(row)
        .and_then(|cells| cells.get(column))
        .copied()
        .unwrap_or([0, 0, 0])
}

fn scale(rgb: [u8; 3], level: f32) -> [u8; 3] {
    let level = level.clamp(0.0, 1.0);
    rgb.map(|c| (f32::from(c) * level).round() as u8)
}

fn mix(a: [u8; 3], b: [u8; 3], amount: f32) -> [u8; 3] {
    let amount = amount.clamp(0.0, 1.0);
    let mut out = [0u8; 3];
    for i in 0..3 {
        let v = f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * amount;
        out[i] = v.round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// A bump of height 1 at `centre` of the cycle, `width` wide.
fn pulse(phase: f32, centre: f32, width: f32) -> f32 {
    let d = (phase - centre) / width;
    (-d * d).exp()
}

fn key_seed(row: usize, column: usize) -> u32 {
    (row as u32).wrapping_mul(0x85eb_ca6b) ^ (column as u32).wrapping_mul(0xc2b2_ae35) ^ 0x27d4_eb2f
}

/// A repeatable number in 0..1 for `seed`.
fn unit(seed: u32) -> f32 {
    let mut x = seed;
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// Noise in 0..1 that changes smoothly with `t`.
fn smooth_noise(seed: u32, t: f32) -> f32 {
    let step = t.floor();
    let local = t - step;
    let at = |k: f32| unit(seed ^ (k as i64 as u32).wrapping_mul(0x9e37_79b9));
    let eased = local * local * (3.0 - 2.0 * local);
    at(step) + (at(step + 1.0) - at(step)) * eased
}

/// The colour from the row `shift` rows above, blended so the rows slide.
fn scroll_cell(base: &LightFrame, row: usize, column: usize, shift: f32) -> [u8; 3] {
    let source = |offset: usize| {
        let from = (row + FRAME_ROWS * 4 - offset % FRAME_ROWS) % FRAME_ROWS;
        let column = column.min(ROW_KEYS[from] - 1);
        base_cell(base, from, column)
    };
    let whole = shift.floor();
    let local = shift - whole;
    mix(source(whole as usize), source(whole as usize + 1), local)
}

fn rain_cell(rgb: [u8; 3], row: usize, column: usize, time: f32) -> [u8; 3] {
    const TRAIL: f32 = 2.5;
    let seed = key_seed(0, column);
    let speed = 1.2 + unit(seed) * 1.3;
    let span = FRAME_ROWS as f32 + TRAIL + 1.5 * unit(seed ^ 0x51);
    let head = (time * speed + span * unit(seed ^ 0xa5)) % span;
    let behind = head - row as f32;
    let level = if (0.0..1.0).contains(&behind) {
        1.0
    } else if (1.0..1.0 + TRAIL).contains(&behind) {
        1.0 - (behind - 1.0) / TRAIL
    } else {
        0.0
    };
    let lit = scale(rgb, 0.08 + 0.92 * level);
    if (0.0..1.0).contains(&behind) {
        mix(lit, WHITE, 0.3)
    } else {
        lit
    }
}

fn reactive_cell(rgb: [u8; 3], row: usize, column: usize, presses: &[Press], rate: f32) -> [u8; 3] {
    let fade = 0.9 / rate.max(0.1);
    let glow = presses
        .iter()
        .filter(|p| p.row == row && p.column == column)
        .map(|p| (1.0 - p.age / fade).max(0.0))
        .fold(0.0f32, f32::max);
    let shown = if rgb == [0, 0, 0] { WHITE } else { rgb };
    mix(scale(shown, 0.15 + 0.85 * glow), WHITE, 0.4 * glow * glow)
}

fn ripple_cell(rgb: [u8; 3], row: usize, column: usize, presses: &[Press], rate: f32) -> [u8; 3] {
    let (x, y) = key_center(row, column);
    let reach = 9.0;
    let glow = presses
        .iter()
        .map(|p| {
            let (px, py) = key_center(p.row, p.column);
            let distance = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            let radius = p.age * 6.0 * rate;
            let ring = (-(distance - radius).powi(2) / 0.8).exp();
            ring * (1.0 - radius / reach).max(0.0)
        })
        .fold(0.0f32, f32::max);
    let shown = if rgb == [0, 0, 0] { WHITE } else { rgb };
    mix(scale(shown, 0.25 + 0.75 * glow), WHITE, 0.45 * glow)
}

/// `rgb` turned `turns` of the way around the colour wheel.
fn rotate_hue(rgb: [u8; 3], turns: f32) -> [u8; 3] {
    let [r, g, b] = rgb.map(|c| f32::from(c) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    if delta <= f32::EPSILON {
        return rgb;
    }
    let hue = if max == r {
        ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    } / 6.0;
    let hue = (hue + turns).rem_euclid(1.0) * 6.0;
    let x = delta * (1.0 - (hue % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hue as u32 {
        0 => (delta, x, 0.0),
        1 => (x, delta, 0.0),
        2 => (0.0, delta, x),
        3 => (0.0, x, delta),
        4 => (x, 0.0, delta),
        _ => (delta, 0.0, x),
    };
    [r1, g1, b1].map(|c| ((c + min) * 255.0).round().clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::solid_frame;

    const RED: [u8; 3] = [255, 0, 0];

    fn at(kind: AnimationKind, t: f32) -> LightFrame {
        render(&solid_frame(RED), Animation::new(kind), t, &[])
    }

    #[test]
    fn every_animation_keeps_the_grid_shape_and_stays_lit_somewhere() {
        for kind in AnimationKind::ALL {
            for step in 0..40 {
                let frame = at(kind, step as f32 * 0.25);
                assert_eq!(frame.len(), FRAME_ROWS);
                assert!(frame.iter().all(|row| row.len() == FRAME_COLUMNS));
                assert!(
                    frame.iter().flatten().any(|c| *c != [0, 0, 0]),
                    "{} went fully dark",
                    kind.name()
                );
            }
        }
    }

    #[test]
    fn frame_animations_leave_dark_cells_dark() {
        let mut base = solid_frame(RED);
        base[0][3] = [0, 0, 0];
        for kind in AnimationKind::ALL.iter().filter(|k| !k.uses_presses()) {
            let frame = render(&base, Animation::new(*kind), 1.3, &[]);
            assert_eq!(frame[0][3], [0, 0, 0], "{}", kind.name());
        }
    }

    #[test]
    fn breathe_dims_and_returns() {
        let low = at(AnimationKind::Breathe, 0.0)[0][0];
        let speed_rate = Animation::new(AnimationKind::Breathe).rate();
        let high = at(AnimationKind::Breathe, 2.0 / speed_rate)[0][0];
        assert!(low[0] < 60, "starts dim: {low:?}");
        assert!(high[0] > 240, "peaks bright: {high:?}");
    }

    #[test]
    fn wave_lights_columns_differently_at_one_moment() {
        let frame = at(AnimationKind::Wave, 0.4);
        assert_ne!(frame[1][0], frame[1][3]);
    }

    #[test]
    fn scroll_moves_row_colours_down() {
        let mut base = solid_frame([0, 0, 255]);
        base[0] = vec![RED; FRAME_COLUMNS];
        let rate = Animation::new(AnimationKind::Scroll).rate();
        let one_row = 1.0 / (0.6 * rate);
        let frame = render(&base, Animation::new(AnimationKind::Scroll), one_row, &[]);
        assert_eq!(frame[1][0], RED, "the top row's colour moved down one");
    }

    #[test]
    fn reactive_flares_the_pressed_key_and_fades() {
        let press = |age| Press {
            row: 2,
            column: 1,
            age,
        };
        let kind = Animation::new(AnimationKind::Reactive);
        let base = solid_frame(RED);
        let fresh = render(&base, kind, 0.0, &[press(0.0)]);
        let old = render(&base, kind, 0.0, &[press(1.9)]);
        assert!(fresh[2][1][0] > old[2][1][0]);
        assert!(fresh[2][1][1] > 0, "a fresh press is whitened");
        assert_eq!(old[2][1], fresh[0][0], "an old press is back at rest");
    }

    #[test]
    fn ripple_reaches_neighbours_after_the_press() {
        let kind = Animation::new(AnimationKind::Ripple);
        let base = solid_frame(RED);
        let rest = render(&base, kind, 0.0, &[])[2][3];
        let later = render(
            &base,
            kind,
            0.0,
            &[Press {
                row: 2,
                column: 1,
                age: 2.0 / (6.0 * kind.rate()),
            }],
        );
        assert!(later[2][3][0] > rest[0], "the ring passes two keys over");
    }

    #[test]
    fn hue_rotation_round_trips() {
        assert_eq!(rotate_hue(RED, 0.0), RED);
        assert_eq!(rotate_hue(RED, 1.0 / 3.0), [0, 255, 0]);
        assert_eq!(rotate_hue([10, 10, 10], 0.5), [10, 10, 10]);
    }

    #[test]
    fn speed_is_checked_and_scales_the_pace() {
        assert!(Animation::new(AnimationKind::Wave).is_valid());
        let slow = Animation {
            kind: AnimationKind::Wave,
            speed: SPEED_MIN,
        };
        let fast = Animation {
            speed: SPEED_MAX,
            ..slow
        };
        assert!(slow.rate() < fast.rate());
        assert!(!Animation { speed: 0, ..slow }.is_valid());
        assert!(!Animation {
            speed: SPEED_MAX + 1,
            ..slow
        }
        .is_valid());
    }

    #[test]
    fn key_centres_follow_the_double_keys() {
        assert_eq!(key_center(0, 0), (1.0, 0.5));
        assert_eq!(key_center(0, 1), (2.5, 0.5));
        assert_eq!(key_center(3, 4), (6.0, 3.5), "key 24 is double");
        assert_eq!(key_center(4, 3), (5.0, 4.5), "key 20 is double");
    }

    #[test]
    fn json_names_are_kebab_case_and_speed_defaults() {
        let a: Animation = serde_json::from_str(r#"{"kind":"hue-drift"}"#).unwrap();
        assert_eq!(a, Animation::new(AnimationKind::HueDrift));
    }
}
