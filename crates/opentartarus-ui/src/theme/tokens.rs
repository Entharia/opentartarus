use iced::Color;

const CHANNEL_MAX: f32 = 255.0;

pub const LINE_ALPHA: f32 = 0.07;
pub const LINE_STRONG_ALPHA: f32 = 0.12;
pub const ACCENT_SOFT_ALPHA: f32 = 0.16;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / CHANNEL_MAX,
        g: g as f32 / CHANNEL_MAX,
        b: b as f32 / CHANNEL_MAX,
        a: 1.0,
    }
}

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color {
        r: r as f32 / CHANNEL_MAX,
        g: g as f32 / CHANNEL_MAX,
        b: b as f32 / CHANNEL_MAX,
        a,
    }
}

// Surfaces, back to front. Each step is wide enough to read as a separate
// object without a border doing the work, which is what keeps the window
// looking solid rather than wireframed.
pub const COLOR_BACKGROUND: Color = rgb(0x17, 0x17, 0x1c);
pub const COLOR_SURFACE: Color = rgb(0x21, 0x21, 0x27);
pub const COLOR_RAISED: Color = rgb(0x2c, 0x2c, 0x35);
pub const COLOR_KEY: Color = rgb(0x38, 0x38, 0x43);
pub const COLOR_KEY_HOVER: Color = rgb(0x45, 0x45, 0x52);
pub const COLOR_KEY_UNBOUND: Color = rgb(0x28, 0x28, 0x30);

// Hairlines.
pub const COLOR_LINE: Color = rgba(0xff, 0xff, 0xff, LINE_ALPHA);
pub const COLOR_LINE_STRONG: Color = rgba(0xff, 0xff, 0xff, LINE_STRONG_ALPHA);

// Text, brightest to faintest.
pub const COLOR_TEXT: Color = rgb(0xe8, 0xe8, 0xee);
pub const COLOR_TEXT_DIM: Color = rgb(0x9b, 0x9b, 0xaa);
/// The byte form of `COLOR_TEXT_FAINT`, for callers that build a `Color`
/// from `[u8; 3]` at the point of use (the sidebar's profile-swatch
/// fallback). Declared first so `COLOR_TEXT_FAINT` derives from it below:
/// one source of truth for the colour, not two literals kept in sync by
/// hand.
pub const SWATCH_FALLBACK_RGB: [u8; 3] = [0x6f, 0x6f, 0x80];
pub const COLOR_TEXT_FAINT: Color = rgb(
    SWATCH_FALLBACK_RGB[0],
    SWATCH_FALLBACK_RGB[1],
    SWATCH_FALLBACK_RGB[2],
);

// Meaning.
pub const COLOR_ACCENT: Color = rgb(0x35, 0x84, 0xe4);
pub const COLOR_ACCENT_HOVER: Color = rgb(0x46, 0x93, 0xf0);
pub const COLOR_ACCENT_PRESSED: Color = rgb(0x2a, 0x6d, 0xc2);
pub const COLOR_ACCENT_SOFT: Color = rgba(0x35, 0x84, 0xe4, ACCENT_SOFT_ALPHA);
pub const COLOR_OK: Color = rgb(0x3a, 0xd0, 0x7f);
pub const COLOR_DANGER: Color = rgb(0xe0, 0x5a, 0x5a);

// Window and band sizes.
pub const WINDOW_WIDTH: f32 = 1120.0;
pub const WINDOW_HEIGHT: f32 = 760.0;
pub const HEADER_HEIGHT: f32 = 52.0;
pub const STATUS_BAR_HEIGHT: f32 = 30.0;
pub const PROFILE_LIST_WIDTH: f32 = 206.0;
pub const INSPECTOR_WIDTH: f32 = 290.0;
pub const MENU_WIDTH: f32 = 200.0;

// Corner radii. The window is frameless, so it rounds its own corners; the
// compositor is not doing it for us.
pub const RADIUS_WINDOW: f32 = 12.0;
pub const RADIUS_CARD: f32 = 12.0;
pub const RADIUS_CONTROL: f32 = 9.0;
pub const RADIUS_KEY: f32 = 10.0;
pub const RADIUS_PILL: f32 = 99.0;

// Spacing scale.
pub const SPACE_XXS: f32 = 2.0;
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
pub const SPACE_LG: f32 = 16.0;
pub const SPACE_XL: f32 = 20.0;

// Type scale.
pub const TEXT_TITLE: f32 = 15.0;
pub const TEXT_HEADING: f32 = 14.0;
pub const TEXT_BODY: f32 = 13.0;
pub const TEXT_SMALL: f32 = 12.0;
pub const TEXT_LABEL: f32 = 10.5;
/// Chip labels use a tight line box so the glyphs sit centred in the cap.
/// iced's default of 1.3 puts all the extra leading below the baseline,
/// which reads as text riding high.
pub const LINE_HEIGHT_TIGHT: f32 = 1.0;
pub const BIND_VALUE_TEXT: f32 = 22.0;

// Small fixed element sizes.
pub const LOGO_SIZE: f32 = 26.0;
pub const ICON_BUTTON_SIZE: f32 = 30.0;
pub const STATUS_DOT_SIZE: f32 = 7.0;
pub const SWATCH_SIZE: f32 = 9.0;
pub const PRESET_SWATCH_SIZE: f32 = 30.0;
pub const SWITCH_WIDTH: f32 = 38.0;
pub const SWITCH_HEIGHT: f32 = 22.0;
pub const EFFECT_GRID_COLUMNS: usize = 4;
/// The pad picture on the lighting tab, and the column of controls under it.
pub const LIGHT_PAD_HEIGHT: f32 = 300.0;
pub const LIGHTING_CONTROLS_WIDTH: f32 = 520.0;
pub const CURRENT_SWATCH_SIZE: f32 = 64.0;
pub const CHANNEL_LABEL_WIDTH: f32 = 12.0;
pub const CHANNEL_VALUE_WIDTH: f32 = 28.0;
/// The small pad pictures in the preset list.
pub const PRESET_THUMB_WIDTH: f32 = 52.0;
pub const PRESET_THUMB_ROW_HEIGHT: f32 = 5.0;
pub const PRESET_THUMB_GAP: f32 = 1.5;
pub const BORDER_NONE: f32 = 0.0;
pub const BORDER_HAIRLINE: f32 = 1.0;
pub const BORDER_SELECTED: f32 = 2.0;

// --- Keycap material -------------------------------------------------------
//
// The device this app configures is a slab of keys, so every control in the
// app is built from one material: a soft square that sits up off its panel and
// presses in when you push it. These are the numbers that make a surface read
// as a cap rather than a rectangle.

/// How far a resting cap's shadow falls. Straight down, because the light is
/// directly above; an angled shadow would imply a light source the rest of the
/// window does not have.
pub const CAP_SHADOW_DROP: f32 = 1.5;
/// Soft, not a hard offset block. A zero-blur shadow is a costume.
pub const CAP_SHADOW_BLUR: f32 = 4.0;
pub const CAP_SHADOW_ALPHA: f32 = 0.22;
/// Pressed caps sit closer to the panel, so their shadow shortens and tightens.
pub const CAP_PRESSED_DROP: f32 = 1.0;
pub const CAP_PRESSED_BLUR: f32 = 2.0;
/// The lit top edge of a cap, drawn as a hairline border one step brighter
/// than the fill.
pub const CAP_EDGE_ALPHA: f32 = 0.06;
pub const COLOR_CAP_EDGE: Color = rgba(0xff, 0xff, 0xff, CAP_EDGE_ALPHA);
/// A pressed cap loses its lit edge and darkens.
pub const COLOR_KEY_PRESSED: Color = rgb(0x2b, 0x2b, 0x34);

// Behaviour values carried over unchanged from the old theme.rs.
pub const DISCONNECTED_OPACITY: f32 = 0.40;
pub const BRIGHTNESS_MIN: u8 = 0;
pub const BRIGHTNESS_MAX: u8 = 100;
pub const COLOR_CHANNEL_MAX: u8 = 255;
pub const DEFAULT_LIGHT_COLOR: [u8; 3] = [255, 255, 255];
pub const DEFAULT_BRIGHTNESS: u8 = 80;

// Colour presets offered in the lighting tab.
pub const LIGHTING_PRESETS: [[u8; 3]; 7] = [
    [0x00, 0xb4, 0xff],
    [0xff, 0x46, 0x55],
    [0x3a, 0xd0, 0x7f],
    [0xf8, 0xb7, 0x00],
    [0xa2, 0x59, 0xff],
    [0xff, 0xff, 0xff],
    [0xe0, 0x5a, 0x5a],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_helper_maps_bytes_to_unit_range() {
        let black = rgb(0x00, 0x00, 0x00);
        assert_eq!((black.r, black.g, black.b, black.a), (0.0, 0.0, 0.0, 1.0));
        let white = rgb(0xff, 0xff, 0xff);
        assert_eq!((white.r, white.g, white.b, white.a), (1.0, 1.0, 1.0, 1.0));
        let accent = rgb(0x35, 0x84, 0xe4);
        assert!((accent.r - 0x35 as f32 / CHANNEL_MAX).abs() < f32::EPSILON);
        assert!((accent.g - 0x84 as f32 / CHANNEL_MAX).abs() < f32::EPSILON);
        assert!((accent.b - 0xe4 as f32 / CHANNEL_MAX).abs() < f32::EPSILON);
    }

    #[test]
    fn rgba_helper_keeps_alpha() {
        let soft = rgba(0x35, 0x84, 0xe4, ACCENT_SOFT_ALPHA);
        assert_eq!(soft.a, ACCENT_SOFT_ALPHA);
        assert_eq!(soft.r, COLOR_ACCENT.r);
        assert_eq!(soft.g, COLOR_ACCENT.g);
        assert_eq!(soft.b, COLOR_ACCENT.b);
    }

    /// Comparing two `const` values directly lets clippy fold the assertion to
    /// a compile-time constant and warn. Reading them through a runtime slice
    /// keeps the check a real test, and covers the whole ramp rather than one
    /// pair at a time.
    fn assert_increasing(name: &str, values: &[f32]) {
        for (index, pair) in values.windows(2).enumerate() {
            assert!(
                pair[1] > pair[0],
                "{name} must increase at step {index}: {pair:?}"
            );
        }
    }

    #[test]
    fn hairlines_are_translucent_white() {
        let lines = [COLOR_LINE, COLOR_LINE_STRONG];
        for line in lines {
            assert_eq!((line.r, line.g, line.b), (1.0, 1.0, 1.0));
        }
        assert_eq!(lines[0].a, LINE_ALPHA);
        assert_eq!(lines[1].a, LINE_STRONG_ALPHA);
        assert_increasing("hairline alpha", &lines.map(|line| line.a));
    }

    #[test]
    fn surfaces_get_lighter_as_they_get_closer() {
        let surfaces = [
            COLOR_BACKGROUND,
            COLOR_SURFACE,
            COLOR_RAISED,
            COLOR_KEY,
            COLOR_KEY_HOVER,
        ];
        assert_increasing("surface lightness", &surfaces.map(|color| color.r));
    }

    #[test]
    fn text_tiers_get_dimmer() {
        let tiers = [COLOR_TEXT_FAINT, COLOR_TEXT_DIM, COLOR_TEXT];
        assert_increasing("text lightness", &tiers.map(|color| color.r));
    }

    #[test]
    fn spacing_scale_is_strictly_increasing() {
        assert_increasing(
            "spacing scale",
            &[SPACE_XS, SPACE_SM, SPACE_MD, SPACE_LG, SPACE_XL],
        );
    }

    #[test]
    fn text_scale_is_strictly_decreasing() {
        let mut scale = [TEXT_LABEL, TEXT_SMALL, TEXT_BODY, TEXT_HEADING, TEXT_TITLE];
        assert_increasing("text scale, read small to large", &scale);
        scale.reverse();
        assert_eq!(scale[0], TEXT_TITLE, "the title is the largest size");
        assert_eq!(scale[scale.len() - 1], TEXT_LABEL, "the label is smallest");
    }
}
