use crate::error::ErrorCode;
use crate::types::{LightFrame, Lighting};

/// The V2 Pro's LED grid: 5 rows of 6, one row per row of keys. Rows that
/// are shorter than 6 keys leave their last cells unused.
pub const FRAME_ROWS: usize = 5;
pub const FRAME_COLUMNS: usize = 6;
/// The rest row, keys 11 to 15 plus 23.
pub const HOME_ROW: usize = 2;
const UNLIT: [u8; 3] = [0, 0, 0];

pub trait LightingClient {
    fn apply(&mut self, lighting: &Lighting) -> Result<(), ErrorCode>;
    fn available(&self) -> bool;
}

pub struct NullLighting;

pub struct RecordingLighting {
    pub available: bool,
    pub last: Option<Lighting>,
}

pub struct LightingChain<P, F> {
    primary: P,
    fallback: F,
}

impl<P, F> LightingChain<P, F> {
    pub fn new(primary: P, fallback: F) -> Self {
        Self { primary, fallback }
    }

    pub fn primary(&mut self) -> &mut P {
        &mut self.primary
    }

    pub fn fallback(&mut self) -> &mut F {
        &mut self.fallback
    }
}

impl<P: LightingClient, F: LightingClient> LightingClient for LightingChain<P, F> {
    fn apply(&mut self, lighting: &Lighting) -> Result<(), ErrorCode> {
        if self.primary.available() {
            return self.primary.apply(lighting);
        }
        if self.fallback.available() {
            return self.fallback.apply(lighting);
        }
        Err(ErrorCode::Lighting)
    }

    fn available(&self) -> bool {
        self.primary.available() || self.fallback.available()
    }
}

pub fn openrazer_lighting_ready(
    daemon_owns_bus: bool,
    tartarus_sysfs: bool,
    other_frontend_count: usize,
) -> bool {
    let _ = other_frontend_count;
    daemon_owns_bus || tartarus_sysfs
}

pub fn peer_frontends_make_openrazer_missing(_other_frontend_count: usize) -> bool {
    false
}

pub fn should_request_openrazer_bus_name() -> bool {
    false
}

pub fn brightness_to_razer(brightness: u8) -> u8 {
    let v = (f32::from(brightness) * 2.55).round();
    v.clamp(0.0, 255.0) as u8
}

pub fn wave_sysfs_value() -> u8 {
    1
}

/// The bytes razerkbd's `matrix_custom_frame` takes for a whole frame: per
/// row, its index, the first and last column, then an RGB triple per column.
/// Missing cells go dark, so a short frame still covers the pad.
pub fn custom_frame_payload(frame: &LightFrame) -> Vec<u8> {
    let mut payload = Vec::with_capacity(FRAME_ROWS * (3 + FRAME_COLUMNS * 3));
    for row in 0..FRAME_ROWS {
        payload.extend_from_slice(&[row as u8, 0, (FRAME_COLUMNS - 1) as u8]);
        for column in 0..FRAME_COLUMNS {
            let rgb = frame
                .get(row)
                .and_then(|cells| cells.get(column))
                .copied()
                .unwrap_or(UNLIT);
            payload.extend_from_slice(&rgb);
        }
    }
    payload
}

/// A frame with every key in `row` set to `rgb`. Rows past the frame's
/// end are added first.
pub fn fill_row(frame: &mut LightFrame, row: usize, rgb: [u8; 3]) {
    if row >= FRAME_ROWS {
        return;
    }
    while frame.len() <= row {
        frame.push(vec![UNLIT; FRAME_COLUMNS]);
    }
    frame[row] = vec![rgb; FRAME_COLUMNS];
}

/// A frame with every key the same colour.
pub fn solid_frame(rgb: [u8; 3]) -> LightFrame {
    vec![vec![rgb; FRAME_COLUMNS]; FRAME_ROWS]
}

impl LightingClient for NullLighting {
    fn apply(&mut self, _lighting: &Lighting) -> Result<(), ErrorCode> {
        Err(ErrorCode::Lighting)
    }

    fn available(&self) -> bool {
        false
    }
}

impl LightingClient for RecordingLighting {
    fn apply(&mut self, lighting: &Lighting) -> Result<(), ErrorCode> {
        if self.available {
            self.last = Some(lighting.clone());
            Ok(())
        } else {
            Err(ErrorCode::Lighting)
        }
    }

    fn available(&self) -> bool {
        self.available
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::types::{Lighting, LightingEffect};

    #[test]
    fn static_writes_rgb_on_mock() {
        let mut c = RecordingLighting {
            available: true,
            last: None,
        };
        let l = Lighting {
            effect: LightingEffect::Static,
            brightness: 80,
            color: Some([0, 180, 255]),
            frame: None,
        };
        c.apply(&l).unwrap();
        assert_eq!(c.last.unwrap().color, Some([0, 180, 255]));
        assert_eq!(brightness_to_razer(80), 204);
        assert_eq!(wave_sysfs_value(), 1);
    }

    #[test]
    fn custom_frame_payload_covers_every_row_and_pads_missing_cells() {
        let frame = vec![vec![[1, 2, 3]; FRAME_COLUMNS], vec![[9, 9, 9]]];
        let payload = custom_frame_payload(&frame);
        assert_eq!(payload.len(), FRAME_ROWS * (3 + FRAME_COLUMNS * 3));
        assert_eq!(&payload[..6], &[0, 0, 5, 1, 2, 3]);
        let row1 = &payload[21..42];
        assert_eq!(&row1[..6], &[1, 0, 5, 9, 9, 9]);
        assert!(row1[6..].iter().all(|b| *b == 0), "missing cells go dark");
        let row4 = &payload[84..];
        assert_eq!(&row4[..3], &[4, 0, 5]);
    }

    #[test]
    fn fill_row_grows_the_frame_and_ignores_rows_past_the_grid() {
        let mut frame = Vec::new();
        fill_row(&mut frame, HOME_ROW, [5, 6, 7]);
        assert_eq!(frame.len(), HOME_ROW + 1);
        assert_eq!(frame[HOME_ROW], vec![[5, 6, 7]; FRAME_COLUMNS]);
        assert_eq!(frame[0], vec![UNLIT; FRAME_COLUMNS]);
        fill_row(&mut frame, FRAME_ROWS, [1, 1, 1]);
        assert_eq!(frame.len(), HOME_ROW + 1);
    }

    #[test]
    fn main_color_of_a_frame_is_the_home_row() {
        let mut frame = solid_frame([1, 1, 1]);
        fill_row(&mut frame, HOME_ROW, [200, 0, 50]);
        let lighting = Lighting {
            effect: LightingEffect::Custom,
            brightness: 80,
            color: Some([9, 9, 9]),
            frame: Some(frame),
        };
        assert_eq!(lighting.main_color(), Some([200, 0, 50]));
        let plain = Lighting {
            effect: LightingEffect::Static,
            ..lighting
        };
        assert_eq!(plain.main_color(), Some([9, 9, 9]));
    }

    #[test]
    fn absent_returns_lighting_and_caller_can_still_apply_profile() {
        let mut c = NullLighting;
        assert!(!c.available());
        let err = c
            .apply(&Lighting {
                effect: LightingEffect::None,
                brightness: 80,
                color: None,
                frame: None,
            })
            .unwrap_err();
        assert_eq!(err, ErrorCode::Lighting);
        assert_eq!(err.user_message(), "Lighting needs OpenRazer or OpenRGB.");
    }

    fn sample() -> Lighting {
        Lighting {
            effect: LightingEffect::Spectrum,
            brightness: 40,
            color: None,
            frame: None,
        }
    }

    #[test]
    fn lighting_available_when_org_razer_owned() {
        assert!(openrazer_lighting_ready(true, false, 0));
        assert!(openrazer_lighting_ready(true, false, 2));
        assert!(openrazer_lighting_ready(false, true, 0));
        assert!(!openrazer_lighting_ready(false, false, 0));
    }

    #[test]
    fn daemon_up_means_lighting_even_with_other_frontends() {
        const POLYCHROMATIC_AND_RAZERGENIE: usize = 2;
        assert!(openrazer_lighting_ready(
            true,
            false,
            POLYCHROMATIC_AND_RAZERGENIE
        ));
        assert!(openrazer_lighting_ready(
            true,
            true,
            POLYCHROMATIC_AND_RAZERGENIE
        ));
        assert!(openrazer_lighting_ready(
            false,
            true,
            POLYCHROMATIC_AND_RAZERGENIE
        ));
        assert!(!openrazer_lighting_ready(
            false,
            false,
            POLYCHROMATIC_AND_RAZERGENIE
        ));
        assert!(!openrazer_lighting_ready(false, false, 0));
        assert!(!peer_frontends_make_openrazer_missing(
            POLYCHROMATIC_AND_RAZERGENIE
        ));
        assert!(!should_request_openrazer_bus_name());
    }

    #[test]
    fn chain_uses_openrazer_first_then_openrgb() {
        let mut chain = LightingChain::new(
            RecordingLighting {
                available: true,
                last: None,
            },
            RecordingLighting {
                available: true,
                last: None,
            },
        );
        assert!(chain.available());
        chain.apply(&sample()).unwrap();
        assert_eq!(
            chain.primary().last.as_ref().unwrap().effect,
            LightingEffect::Spectrum
        );
        assert!(chain.fallback().last.is_none());
    }

    #[test]
    fn chain_uses_openrgb_when_openrazer_is_down() {
        let mut chain = LightingChain::new(
            NullLighting,
            RecordingLighting {
                available: true,
                last: None,
            },
        );
        assert!(chain.available());
        chain.apply(&sample()).unwrap();
        assert_eq!(chain.fallback().last.as_ref().unwrap().brightness, 40);
    }

    #[test]
    fn chain_does_not_call_openrgb_while_openrazer_reports_available() {
        struct AvailableFail;
        impl LightingClient for AvailableFail {
            fn apply(&mut self, _lighting: &Lighting) -> Result<(), ErrorCode> {
                Err(ErrorCode::Lighting)
            }
            fn available(&self) -> bool {
                true
            }
        }
        let mut chain = LightingChain::new(
            AvailableFail,
            RecordingLighting {
                available: true,
                last: None,
            },
        );
        assert!(chain.available());
        assert_eq!(chain.apply(&sample()), Err(ErrorCode::Lighting));
        assert!(chain.fallback().last.is_none());
    }
}
