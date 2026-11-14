use crate::app::{uses_color, App, Message, Phase};
use crate::theme;
use crate::view::widgets::{
    card_container, quiet_button, row_button, section_label, selected_effect_button,
    selected_row_button, swatch,
};
use iced::widget::{button, column, container, row, scrollable, slider, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme};
use opentartarus_core::light_presets::{preset_for_frame, LightPreset, PresetGroup, PRESETS};
use opentartarus_core::lighting::FRAME_ROWS;
use opentartarus_core::types::{LightFrame, LightingEffect};

const BREATH_PREVIEW_ALPHA: f32 = 0.5;
const WAVE_PREVIEW: [u8; 3] = [0x00, 0xb4, 0xff];
const SPECTRUM_PREVIEW: [u8; 3] = [0xa2, 0x59, 0xff];
const REACTIVE_PREVIEW: [u8; 3] = [0xf8, 0xb7, 0x00];
const STARLIGHT_PREVIEW: [u8; 3] = [0xe8, 0xe8, 0xee];
const PERCENT_SUFFIX: &str = "%";
/// Key widths along each V2 Pro row, in single-key units. Each row's first
/// key is double, and so are 24 and 20. Every row but the last spans 7.
const ROW_KEY_UNITS: [&[u16]; FRAME_ROWS] = [
    &[2, 1, 1, 1, 1, 1],
    &[2, 1, 1, 1, 1, 1],
    &[2, 1, 1, 1, 1, 1],
    &[2, 1, 1, 1, 2],
    &[2, 1, 1, 2],
];
const ROW_UNITS: u16 = 7;
const UNLIT: [u8; 3] = [0, 0, 0];

/// Lighting is only editable once the daemon is up and a backend answered.
pub fn lighting_is_editable(app: &App) -> bool {
    app.phase == Phase::Running && app.openrazer
}

/// The preview colour on an effect card. Colour-driven effects show the chosen
/// colour; the rest get a fixed hint so each card is distinguishable.
pub fn effect_swatch_color(effect: LightingEffect, current: Option<[u8; 3]>) -> Color {
    let chosen = current.unwrap_or(theme::DEFAULT_LIGHT_COLOR);
    let chosen = Color::from_rgb8(chosen[0], chosen[1], chosen[2]);
    match effect {
        LightingEffect::Static => chosen,
        LightingEffect::Breath => Color {
            a: BREATH_PREVIEW_ALPHA,
            ..chosen
        },
        LightingEffect::Reactive => Color::from_rgb8(
            REACTIVE_PREVIEW[0],
            REACTIVE_PREVIEW[1],
            REACTIVE_PREVIEW[2],
        ),
        LightingEffect::Starlight => Color::from_rgb8(
            STARLIGHT_PREVIEW[0],
            STARLIGHT_PREVIEW[1],
            STARLIGHT_PREVIEW[2],
        ),
        LightingEffect::Wave => Color::from_rgb8(WAVE_PREVIEW[0], WAVE_PREVIEW[1], WAVE_PREVIEW[2]),
        LightingEffect::Spectrum => Color::from_rgb8(
            SPECTRUM_PREVIEW[0],
            SPECTRUM_PREVIEW[1],
            SPECTRUM_PREVIEW[2],
        ),
        LightingEffect::None => theme::COLOR_BACKGROUND,
        // The card draws the frame itself; this is its fallback colour.
        LightingEffect::Custom => chosen,
    }
}

fn rgb_color(rgb: [u8; 3]) -> Color {
    Color::from_rgb8(rgb[0], rgb[1], rgb[2])
}

fn cell_rgb(frame: &LightFrame, row: usize, column: usize) -> [u8; 3] {
    frame
        .get(row)
        .and_then(|cells| cells.get(column))
        .copied()
        .unwrap_or(UNLIT)
}

fn cell<'a>(rgb: [u8; 3], units: u16, height: f32, radius: f32) -> Element<'a, Message> {
    let color = rgb_color(rgb);
    container(Space::new(Length::Fill, Length::Fixed(height)))
        .width(Length::FillPortion(units))
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(color)),
            border: Border {
                color: theme::COLOR_LINE_STRONG,
                width: theme::BORDER_HAIRLINE,
                radius: radius.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// One row of the pad drawn in its frame colours, keys at their widths.
fn frame_row<'a>(frame: &LightFrame, row: usize, height: f32, gap: f32) -> Element<'a, Message> {
    let mut line = iced::widget::Row::new().spacing(gap);
    let mut used = 0;
    for (column, units) in ROW_KEY_UNITS[row].iter().enumerate() {
        line = line.push(cell(
            cell_rgb(frame, row, column),
            *units,
            height,
            height / 3.0,
        ));
        used += units;
    }
    if used < ROW_UNITS {
        line = line.push(Space::with_width(Length::FillPortion(ROW_UNITS - used)));
    }
    line.into()
}

/// A small picture of a whole frame, for effect and preset cards.
fn frame_thumbnail<'a>(frame: &LightFrame, height: f32) -> Element<'a, Message> {
    let mut grid = column![].spacing(theme::FRAME_THUMB_GAP);
    for row in 0..FRAME_ROWS {
        grid = grid.push(frame_row(frame, row, height, theme::FRAME_THUMB_GAP));
    }
    grid.into()
}

fn effect_card(app: &App, effect: LightingEffect) -> Element<'_, Message> {
    let selected = app.lighting.effect == effect;
    let preview = effect_swatch_color(effect, app.lighting.color);
    let picture: Element<'_, Message> = match (effect, &app.lighting.frame) {
        (LightingEffect::Custom, Some(frame)) => container(frame_thumbnail(
            frame,
            (theme::EFFECT_SWATCH_HEIGHT - theme::FRAME_THUMB_GAP * 4.0) / FRAME_ROWS as f32,
        ))
        .height(Length::Fixed(theme::EFFECT_SWATCH_HEIGHT))
        .into(),
        _ => container(Space::new(
            Length::Fill,
            Length::Fixed(theme::EFFECT_SWATCH_HEIGHT),
        ))
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(preview)),
            border: Border {
                color: theme::COLOR_LINE_STRONG,
                width: theme::BORDER_HAIRLINE,
                radius: theme::RADIUS_CONTROL.into(),
            },
            ..container::Style::default()
        })
        .into(),
    };
    let card = column![
        picture,
        text(crate::app::effect_name(effect)).size(theme::TEXT_SMALL),
    ]
    .spacing(theme::SPACE_SM)
    .align_x(Alignment::Center);

    let style = if selected {
        selected_effect_button
    } else {
        quiet_button
    };
    button(card)
        .width(Length::Fill)
        .padding(theme::SPACE_SM)
        .on_press(Message::LightingEffect(crate::app::EffectChoice(effect)))
        .style(style)
        .into()
}

fn effect_grid(app: &App) -> Element<'_, Message> {
    let mut grid = column![].spacing(theme::SPACE_SM);
    for chunk in app.offered_effects().chunks(theme::EFFECT_GRID_COLUMNS) {
        let mut line = row![].spacing(theme::SPACE_SM);
        for choice in chunk {
            line = line.push(effect_card(app, choice.0));
        }
        // Keep the last row's cards the same width as a full row's.
        for _ in chunk.len()..theme::EFFECT_GRID_COLUMNS {
            line = line.push(Space::with_width(Length::Fill));
        }
        grid = grid.push(line);
    }
    grid.into()
}

fn color_row(app: &App) -> Element<'_, Message> {
    let mut presets = row![].spacing(theme::SPACE_SM).align_y(Alignment::Center);
    for rgb in theme::LIGHTING_PRESETS {
        presets = presets.push(
            button(swatch(rgb, theme::PRESET_SWATCH_SIZE))
                .padding(0)
                .on_press(Message::LightingPreset(rgb))
                .style(quiet_button),
        );
    }
    let rgb = app.edited_color();
    let channels = row![
        slider(0..=theme::COLOR_CHANNEL_MAX, rgb[0], |v| {
            Message::LightingColor(0, v)
        }),
        slider(0..=theme::COLOR_CHANNEL_MAX, rgb[1], |v| {
            Message::LightingColor(1, v)
        }),
        slider(0..=theme::COLOR_CHANNEL_MAX, rgb[2], |v| {
            Message::LightingColor(2, v)
        }),
    ]
    .spacing(theme::SPACE_SM);

    column![section_label(theme::LABEL_COLOR), presets, channels,]
        .spacing(theme::SPACE_SM)
        .into()
}

/// The pad in its frame colours. Clicking a row picks it for the colour
/// controls below.
fn row_picker(app: &App) -> Element<'_, Message> {
    let empty = Vec::new();
    let frame = app.lighting.frame.as_ref().unwrap_or(&empty);
    let mut rows = column![].spacing(theme::SPACE_XS);
    for row in 0..FRAME_ROWS {
        let style = if row == app.lighting_row {
            selected_row_button
        } else {
            row_button
        };
        rows = rows.push(
            button(frame_row(
                frame,
                row,
                theme::FRAME_KEY_HEIGHT,
                theme::SPACE_XS,
            ))
            .width(Length::Fill)
            .padding(theme::SPACE_XS)
            .on_press(Message::LightingRow(row))
            .style(style),
        );
    }
    column![
        section_label(theme::LABEL_ROWS),
        text(theme::ROWS_HINT)
            .size(theme::TEXT_SMALL)
            .color(theme::COLOR_TEXT_FAINT),
        rows,
    ]
    .spacing(theme::SPACE_SM)
    .into()
}

fn preset_card(preset: &'static LightPreset, selected: bool) -> Element<'static, Message> {
    let card = column![
        frame_thumbnail(&preset.frame(), theme::PRESET_THUMB_ROW_HEIGHT),
        text(preset.name).size(theme::TEXT_SMALL),
    ]
    .spacing(theme::SPACE_XS)
    .align_x(Alignment::Center);
    let style = if selected {
        selected_effect_button
    } else {
        quiet_button
    };
    button(card)
        .width(Length::Fill)
        .padding(theme::SPACE_SM)
        .on_press(Message::LightFramePreset(preset.id))
        .style(style)
        .into()
}

/// Every preset, under its group's name.
fn preset_gallery(app: &App) -> Element<'_, Message> {
    let current = app.lighting.frame.as_ref().and_then(preset_for_frame);
    let mut gallery = column![].spacing(theme::SPACE_MD);
    for group in PresetGroup::ALL {
        let presets: Vec<&'static LightPreset> =
            PRESETS.iter().filter(|p| p.group == group).collect();
        let mut grid = column![].spacing(theme::SPACE_SM);
        for chunk in presets.chunks(theme::PRESET_GRID_COLUMNS) {
            let mut line = row![].spacing(theme::SPACE_SM);
            for preset in chunk {
                let selected = current.is_some_and(|c| c.id == preset.id);
                line = line.push(preset_card(preset, selected));
            }
            for _ in chunk.len()..theme::PRESET_GRID_COLUMNS {
                line = line.push(Space::with_width(Length::Fill));
            }
            grid = grid.push(line);
        }
        gallery = gallery.push(column![section_label(group.name()), grid].spacing(theme::SPACE_SM));
    }
    gallery.into()
}

pub fn lighting_tab(app: &App) -> Element<'_, Message> {
    let body: Element<'_, Message> = if let Some(copy) = app.lighting_footer_message() {
        container(
            text(copy)
                .size(theme::TEXT_BODY)
                .color(theme::COLOR_TEXT_DIM),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    } else if !lighting_is_editable(app) {
        Space::with_height(Length::Fill).into()
    } else {
        let brightness_label = format!(
            "{} {}{}",
            theme::LABEL_BRIGHTNESS,
            app.lighting.brightness,
            PERCENT_SUFFIX
        );
        let mut panel = column![
            section_label(theme::LABEL_EFFECT),
            effect_grid(app),
            section_label(&brightness_label),
            slider(
                theme::BRIGHTNESS_MIN..=theme::BRIGHTNESS_MAX,
                app.lighting.brightness,
                Message::LightingBrightness,
            ),
        ]
        .spacing(theme::SPACE_MD);
        if uses_color(app.lighting.effect) {
            panel = panel.push(color_row(app));
        }
        if app.lighting.effect == LightingEffect::Custom {
            panel = panel.push(row_picker(app)).push(color_row(app));
        }
        if app
            .offered_effects()
            .iter()
            .any(|c| c.0 == LightingEffect::Custom)
        {
            panel = panel
                .push(section_label(theme::LABEL_PRESETS))
                .push(preset_gallery(app));
        }
        // Room on the right so the scrollbar doesn't cover the controls.
        scrollable(container(panel).padding(iced::Padding::ZERO.right(theme::SPACE_MD)))
            .height(Length::Fill)
            .into()
    };

    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(theme::SPACE_XL)
        .style(card_container)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Phase};
    use opentartarus_core::types::LightingEffect;

    // Struct-update syntax, not `App::default()` followed by field
    // assignments: clippy's `field_reassign_with_default` flags that shape,
    // and the workspace clippy budget is exact (matches the pattern already
    // used by view/sidebar.rs's and view/menu.rs's own `running` helpers).
    fn running() -> App {
        App {
            phase: Phase::Running,
            device_present: true,
            ever_present: true,
            openrazer: true,
            ..App::default()
        }
    }

    #[test]
    fn lighting_is_editable_only_when_a_backend_is_there() {
        let mut app = running();
        assert!(lighting_is_editable(&app));
        app.openrazer = false;
        assert!(!lighting_is_editable(&app), "no OpenRazer, no controls");
        assert!(app.lighting_footer_message().is_some());
    }

    #[test]
    fn lighting_is_not_editable_before_the_daemon_connects() {
        let mut app = running();
        app.phase = Phase::Connecting;
        assert!(!lighting_is_editable(&app));
    }

    #[test]
    fn colour_effects_preview_the_chosen_colour() {
        let chosen = Some([10u8, 20, 30]);
        let expected = Color::from_rgb8(10, 20, 30);
        assert_eq!(
            effect_swatch_color(LightingEffect::Static, chosen),
            expected
        );
        let breath = effect_swatch_color(LightingEffect::Breath, chosen);
        assert_eq!(
            (breath.r, breath.g, breath.b),
            (expected.r, expected.g, expected.b)
        );
        assert!(breath.a < expected.a, "breathing must read as dimmer");
    }

    #[test]
    fn colour_effects_fall_back_when_no_colour_is_set() {
        let fallback = Color::from_rgb8(
            theme::DEFAULT_LIGHT_COLOR[0],
            theme::DEFAULT_LIGHT_COLOR[1],
            theme::DEFAULT_LIGHT_COLOR[2],
        );
        assert_eq!(effect_swatch_color(LightingEffect::Static, None), fallback);
    }

    #[test]
    fn colourless_effects_get_fixed_previews_and_off_reads_as_dark() {
        let chosen = Some([10u8, 20, 30]);
        let wave = effect_swatch_color(LightingEffect::Wave, chosen);
        let spectrum = effect_swatch_color(LightingEffect::Spectrum, chosen);
        assert_ne!(wave, spectrum, "each effect needs its own preview");
        assert_ne!(wave, Color::from_rgb8(10, 20, 30));
        assert_eq!(
            effect_swatch_color(LightingEffect::None, chosen),
            theme::COLOR_BACKGROUND
        );
    }

    #[test]
    fn every_effect_offered_has_a_preview() {
        for choice in crate::app::LIGHTING_EFFECTS {
            let color = effect_swatch_color(choice.0, Some([1, 2, 3]));
            assert!(color.a > 0.0 || choice.0 == LightingEffect::None);
        }
    }
}
