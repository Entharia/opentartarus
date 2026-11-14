use crate::app::{uses_color, App, EffectChoice, Message, Phase};
use crate::light_pad::{self, LightPad};
use crate::theme;
use crate::view::widgets::{
    card_container, dot, quiet_button, row_button, section_label, selected_effect_button,
    selected_row_button, swatch,
};
use iced::widget::{button, column, container, row, scrollable, slider, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme};
use opentartarus_core::light_presets::{
    preset_by_id, preset_for_frame, LightPreset, PresetGroup, PRESETS,
};
use opentartarus_core::lighting::{solid_frame, FRAME_ROWS};
use opentartarus_core::types::{DeviceModel, LightFrame, LightingEffect};

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
/// What the pad shows for the effects that cycle through colours.
const CYCLING_PREVIEW_PRESET: &str = "prism";
const CHANNEL_NAMES: [&str; 3] = ["R", "G", "B"];

/// Lighting is only editable once the daemon is up and a backend answered.
pub fn lighting_is_editable(app: &App) -> bool {
    app.phase == Phase::Running && app.openrazer
}

/// The dot on an effect chip. Colour-driven effects show the chosen colour;
/// the rest get a fixed hint so each chip is distinguishable.
pub fn effect_swatch_color(effect: LightingEffect, current: Option<[u8; 3]>) -> Color {
    let chosen = current.unwrap_or(theme::DEFAULT_LIGHT_COLOR);
    let chosen = Color::from_rgb8(chosen[0], chosen[1], chosen[2]);
    match effect {
        LightingEffect::Static | LightingEffect::Custom => chosen,
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
    }
}

/// What every LED shows for the current effect, for the pad picture.
pub fn preview_frame(app: &App) -> LightFrame {
    let lighting = &app.lighting;
    match lighting.effect {
        LightingEffect::Custom => lighting.frame.clone().unwrap_or_default(),
        LightingEffect::Wave | LightingEffect::Spectrum => preset_by_id(CYCLING_PREVIEW_PRESET)
            .map(LightPreset::frame)
            .unwrap_or_default(),
        LightingEffect::None => solid_frame(UNLIT),
        LightingEffect::Static
        | LightingEffect::Breath
        | LightingEffect::Reactive
        | LightingEffect::Starlight => {
            solid_frame(lighting.color.unwrap_or(theme::DEFAULT_LIGHT_COLOR))
        }
    }
}

fn offers_custom(app: &App) -> bool {
    app.offered_effects()
        .iter()
        .any(|choice| choice.0 == LightingEffect::Custom)
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

fn cell<'a>(rgb: [u8; 3], units: u16, height: f32) -> Element<'a, Message> {
    let color = if rgb == UNLIT {
        theme::COLOR_KEY_UNBOUND
    } else {
        rgb_color(rgb)
    };
    container(Space::new(Length::Fill, Length::Fixed(height)))
        .width(Length::FillPortion(units))
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(color)),
            border: Border {
                radius: (height / 3.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

/// A small picture of a whole frame, keys at their widths, for preset rows.
fn frame_thumbnail<'a>(frame: &LightFrame) -> Element<'a, Message> {
    let gap = theme::PRESET_THUMB_GAP;
    let mut grid = column![].spacing(gap);
    for (row, units) in ROW_KEY_UNITS.iter().enumerate() {
        let mut line = iced::widget::Row::new().spacing(gap);
        let mut used = 0;
        for (column, width) in units.iter().enumerate() {
            line = line.push(cell(
                cell_rgb(frame, row, column),
                *width,
                theme::PRESET_THUMB_ROW_HEIGHT,
            ));
            used += width;
        }
        if used < ROW_UNITS {
            line = line.push(Space::with_width(Length::FillPortion(ROW_UNITS - used)));
        }
        grid = grid.push(line);
    }
    container(grid)
        .width(Length::Fixed(theme::PRESET_THUMB_WIDTH))
        .into()
}

fn effect_chip(app: &App, choice: EffectChoice) -> Element<'_, Message> {
    let effect = choice.0;
    let selected = app.lighting.effect == effect;
    let color = effect_swatch_color(effect, app.lighting.main_color());
    let label = row![
        dot(color),
        text(crate::app::effect_name(effect)).size(theme::TEXT_SMALL),
    ]
    .spacing(theme::SPACE_SM)
    .align_y(Alignment::Center);
    let style = if selected {
        selected_effect_button
    } else {
        quiet_button
    };
    button(container(label).center_x(Length::Fill))
        .width(Length::Fill)
        .padding([theme::SPACE_SM, theme::SPACE_MD])
        .on_press(Message::LightingEffect(choice))
        .style(style)
        .into()
}

fn effect_chips(app: &App) -> Element<'_, Message> {
    let mut grid = column![].spacing(theme::SPACE_SM);
    for chunk in app.offered_effects().chunks(theme::EFFECT_GRID_COLUMNS) {
        let mut line = row![].spacing(theme::SPACE_SM);
        for choice in chunk {
            line = line.push(effect_chip(app, *choice));
        }
        // Keep the last row's chips the same width as a full row's.
        for _ in chunk.len()..theme::EFFECT_GRID_COLUMNS {
            line = line.push(Space::with_width(Length::Fill));
        }
        grid = grid.push(line);
    }
    column![section_label(theme::LABEL_EFFECT), grid]
        .spacing(theme::SPACE_SM)
        .into()
}

/// A label on the left with its value on the right, above a control.
fn labelled<'a>(label: String, value: String) -> Element<'a, Message> {
    row![
        section_label(label),
        Space::with_width(Length::Fill),
        text(value)
            .size(theme::TEXT_SMALL)
            .color(theme::COLOR_TEXT_DIM),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn brightness_control(app: &App) -> Element<'_, Message> {
    column![
        labelled(
            theme::LABEL_BRIGHTNESS.to_string(),
            format!("{}{}", app.lighting.brightness, PERCENT_SUFFIX),
        ),
        slider(
            theme::BRIGHTNESS_MIN..=theme::BRIGHTNESS_MAX,
            app.lighting.brightness,
            Message::LightingBrightness,
        ),
    ]
    .spacing(theme::SPACE_SM)
    .into()
}

fn color_control(app: &App) -> Element<'_, Message> {
    let rgb = app.edited_color();
    let label = if app.lighting.effect == LightingEffect::Custom {
        format!("{}{}", theme::LABEL_ROW_COLOR_PREFIX, app.lighting_row + 1)
    } else {
        theme::LABEL_COLOR.to_string()
    };
    let hex = format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);

    let mut presets = row![].spacing(theme::SPACE_SM).align_y(Alignment::Center);
    for preset in theme::LIGHTING_PRESETS {
        presets = presets.push(
            button(swatch(preset, theme::PRESET_SWATCH_SIZE))
                .padding(0)
                .on_press(Message::LightingPreset(preset))
                .style(quiet_button),
        );
    }
    let mut channels = column![].spacing(theme::SPACE_XS);
    for (channel, name) in CHANNEL_NAMES.iter().enumerate() {
        channels = channels.push(
            row![
                text(*name)
                    .size(theme::TEXT_SMALL)
                    .color(theme::COLOR_TEXT_FAINT)
                    .width(Length::Fixed(theme::CHANNEL_LABEL_WIDTH)),
                slider(0..=theme::COLOR_CHANNEL_MAX, rgb[channel], move |v| {
                    Message::LightingColor(channel, v)
                }),
                text(rgb[channel].to_string())
                    .size(theme::TEXT_SMALL)
                    .color(theme::COLOR_TEXT_DIM)
                    .width(Length::Fixed(theme::CHANNEL_VALUE_WIDTH))
                    .align_x(Alignment::End),
            ]
            .spacing(theme::SPACE_SM)
            .align_y(Alignment::Center),
        );
    }

    column![
        labelled(label, hex),
        row![
            swatch(rgb, theme::CURRENT_SWATCH_SIZE),
            column![presets, channels].spacing(theme::SPACE_SM),
        ]
        .spacing(theme::SPACE_MD)
        .align_y(Alignment::Center),
    ]
    .spacing(theme::SPACE_SM)
    .into()
}

fn pad_hint(app: &App) -> &'static str {
    if app.lighting.effect == LightingEffect::Custom {
        theme::PAD_HINT_CUSTOM
    } else {
        theme::PAD_HINT_PICK_ROW
    }
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
        let custom = app.lighting.effect == LightingEffect::Custom;
        let pad = light_pad::widget(
            LightPad {
                model: app.model,
                frame: preview_frame(app),
                brightness: app.lighting.brightness,
                selected_row: custom.then_some(app.lighting_row),
                rows_pickable: offers_custom(app),
            },
            theme::LIGHT_PAD_HEIGHT,
        );
        let mut top = column![pad]
            .spacing(theme::SPACE_SM)
            .align_x(Alignment::Center);
        if app.model == Some(DeviceModel::V2Pro) {
            top = top.push(
                text(pad_hint(app))
                    .size(theme::TEXT_SMALL)
                    .color(theme::COLOR_TEXT_FAINT),
            );
        }

        let mut controls = column![effect_chips(app), brightness_control(app)]
            .spacing(theme::SPACE_XL)
            .max_width(theme::LIGHTING_CONTROLS_WIDTH);
        if uses_color(app.lighting.effect) || custom {
            controls = controls.push(color_control(app));
        }

        let page = column![top, controls]
            .spacing(theme::SPACE_XL)
            .width(Length::Fill)
            .align_x(Alignment::Center);
        // Room on the right so the scrollbar doesn't cover the controls.
        scrollable(container(page).padding(iced::Padding::ZERO.right(theme::SPACE_MD)))
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

fn group_label(group: PresetGroup) -> &'static str {
    match group {
        PresetGroup::WorldOfWarcraft => theme::PRESET_GROUP_WOW,
        PresetGroup::Games => theme::PRESET_GROUP_GAMES,
        PresetGroup::Aesthetic => theme::PRESET_GROUP_AESTHETIC,
        PresetGroup::Functional => theme::PRESET_GROUP_FUNCTIONAL,
    }
}

fn group_caption(group: PresetGroup) -> &'static str {
    match group {
        PresetGroup::WorldOfWarcraft => theme::PRESET_CAPTION_WOW,
        PresetGroup::Games => theme::PRESET_CAPTION_GAMES,
        PresetGroup::Aesthetic => theme::PRESET_CAPTION_AESTHETIC,
        PresetGroup::Functional => theme::PRESET_CAPTION_FUNCTIONAL,
    }
}

fn group_chips(app: &App) -> Element<'_, Message> {
    let mut grid = column![].spacing(theme::SPACE_XS);
    for pair in PresetGroup::ALL.chunks(2) {
        let mut line = row![].spacing(theme::SPACE_XS);
        for group in pair {
            let style = if app.preset_group == *group {
                selected_effect_button
            } else {
                quiet_button
            };
            line = line.push(
                button(
                    container(text(group_label(*group)).size(theme::TEXT_SMALL))
                        .center_x(Length::Fill),
                )
                .width(Length::Fill)
                .padding([theme::SPACE_XS, theme::SPACE_SM])
                .on_press(Message::PresetGroupPick(*group))
                .style(style),
            );
        }
        grid = grid.push(line);
    }
    grid.into()
}

fn preset_row(preset: &'static LightPreset, selected: bool) -> Element<'static, Message> {
    let style = if selected {
        selected_row_button
    } else {
        row_button
    };
    button(
        row![
            frame_thumbnail(&preset.frame()),
            text(preset.name).size(theme::TEXT_BODY),
        ]
        .spacing(theme::SPACE_MD)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([theme::SPACE_SM, theme::SPACE_SM])
    .on_press(Message::LightFramePreset(preset.id))
    .style(style)
    .into()
}

/// The right-hand panel on the lighting tab: presets, one group at a time.
pub fn presets_panel(app: &App) -> Element<'_, Message> {
    let title = text(theme::LABEL_PRESETS).size(theme::TEXT_TITLE);
    if !lighting_is_editable(app) || !offers_custom(app) {
        return column![
            title,
            text(theme::PRESETS_NEED_V2_PRO)
                .size(theme::TEXT_SMALL)
                .color(theme::COLOR_TEXT_FAINT),
        ]
        .spacing(theme::SPACE_SM)
        .into();
    }
    let current = app.lighting.frame.as_ref().and_then(preset_for_frame);
    let mut list = column![].spacing(theme::SPACE_XXS);
    for preset in PRESETS.iter().filter(|p| p.group == app.preset_group) {
        let selected = app.lighting.effect == LightingEffect::Custom
            && current.is_some_and(|c| c.id == preset.id);
        list = list.push(preset_row(preset, selected));
    }
    column![
        title,
        group_chips(app),
        text(group_caption(app.preset_group))
            .size(theme::TEXT_SMALL)
            .color(theme::COLOR_TEXT_FAINT),
        scrollable(container(list).padding(iced::Padding::ZERO.right(theme::SPACE_MD)))
            .height(Length::Fill),
    ]
    .spacing(theme::SPACE_MD)
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
