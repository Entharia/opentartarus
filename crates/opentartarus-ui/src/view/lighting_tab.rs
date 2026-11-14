use crate::app::{hex_code, uses_color, App, EffectChoice, Message, PaintMode, Phase, PresetShelf};
use crate::keys::{HEX_INPUT_ID, SAVE_PRESET_INPUT_ID};
use crate::light_pad::{self, LightPad};
use crate::theme;
use crate::view::widgets::{
    accent_text_button, card_container, danger_text_button, dot, quiet_button, row_button,
    section_label, selected_effect_button, selected_row_button, swatch,
};
use iced::widget::{button, column, container, row, scrollable, slider, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme};
use opentartarus_core::animation::{render, AnimationKind, Press, SPEED_MAX, SPEED_MIN};
use opentartarus_core::labels::{v2_pro_key_number, V2_PRO_ROWS};
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

/// How often the preview pretends a key was pressed, for the animations
/// that react to presses, and how many recent presses it keeps.
const PREVIEW_PRESS_EVERY_SECS: f32 = 0.7;
const PREVIEW_PRESSES: usize = 3;

/// Made-up presses for the preview: a different face key every so often.
fn preview_presses(t: f32) -> Vec<Press> {
    let keys: Vec<(usize, usize)> = V2_PRO_ROWS
        .iter()
        .enumerate()
        .flat_map(|(row, keys)| (0..keys.len()).map(move |column| (row, column)))
        .collect();
    let latest = (t / PREVIEW_PRESS_EVERY_SECS).floor() as i64;
    (0..PREVIEW_PRESSES as i64)
        .map(|back| latest - back)
        .filter(|tick| *tick >= 0)
        .map(|tick| {
            let pick = (tick as usize).wrapping_mul(7919).wrapping_add(13) % keys.len();
            let (row, column) = keys[pick];
            Press {
                row,
                column,
                age: t - tick as f32 * PREVIEW_PRESS_EVERY_SECS,
            }
        })
        .collect()
}

/// What every LED shows for the current effect, for the pad picture.
pub fn preview_frame(app: &App) -> LightFrame {
    let lighting = &app.lighting;
    match lighting.effect {
        LightingEffect::Custom => {
            let frame = lighting.frame.clone().unwrap_or_default();
            match lighting.animation {
                Some(animation) => render(
                    &frame,
                    animation,
                    app.preview_time,
                    &preview_presses(app.preview_time),
                ),
                None => frame,
            }
        }
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

fn animation_chip(app: &App, kind: Option<AnimationKind>) -> Element<'_, Message> {
    let selected = app.lighting.animation.map(|a| a.kind) == kind;
    let name = kind.map_or(theme::ANIMATION_NONE, AnimationKind::name);
    let style = if selected {
        selected_effect_button
    } else {
        quiet_button
    };
    button(container(text(name).size(theme::TEXT_SMALL)).center_x(Length::Fill))
        .width(Length::Fill)
        .padding([theme::SPACE_SM, theme::SPACE_MD])
        .on_press(Message::LightingAnimation(kind))
        .style(style)
        .into()
}

/// The custom frame's animation: still or one of the kinds, and its speed.
fn animation_control(app: &App) -> Element<'_, Message> {
    let choices: Vec<Option<AnimationKind>> = std::iter::once(None)
        .chain(AnimationKind::ALL.into_iter().map(Some))
        .collect();
    let mut grid = column![].spacing(theme::SPACE_SM);
    for chunk in choices.chunks(theme::EFFECT_GRID_COLUMNS) {
        let mut line = row![].spacing(theme::SPACE_SM);
        for kind in chunk {
            line = line.push(animation_chip(app, *kind));
        }
        for _ in chunk.len()..theme::EFFECT_GRID_COLUMNS {
            line = line.push(Space::with_width(Length::Fill));
        }
        grid = grid.push(line);
    }
    let mut section = column![section_label(theme::LABEL_ANIMATION), grid].spacing(theme::SPACE_SM);
    if let Some(animation) = app.lighting.animation {
        if animation.kind.uses_presses() {
            section = section.push(
                text(theme::ANIMATION_PRESS_HINT)
                    .size(theme::TEXT_SMALL)
                    .color(theme::COLOR_TEXT_FAINT),
            );
        }
        section = section
            .push(labelled(
                theme::LABEL_SPEED.to_string(),
                format!("{} / {}", animation.speed, SPEED_MAX),
            ))
            .push(slider(
                SPEED_MIN..=SPEED_MAX,
                animation.speed,
                Message::LightingSpeed,
            ));
    }
    section.into()
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
        painted_label(app)
    } else {
        theme::LABEL_COLOR.to_string()
    };
    let hex = hex_code(rgb);
    let copy_label = if app.copied_hex.as_deref() == Some(hex.as_str()) {
        format!("{}{hex}", theme::COPIED_PREFIX)
    } else {
        format!("{hex}{}", theme::COPY_HINT_SUFFIX)
    };
    let header = row![
        section_label(label),
        Space::with_width(Length::Fill),
        button(text(copy_label).size(theme::TEXT_SMALL))
            .padding([theme::SPACE_XXS, theme::SPACE_SM])
            .on_press(Message::CopyHex)
            .style(accent_text_button),
    ]
    .align_y(Alignment::Center);
    let hex_box = text_input(
        theme::HEX_PLACEHOLDER,
        app.hex_draft.as_deref().unwrap_or(""),
    )
    .id(text_input::Id::new(HEX_INPUT_ID))
    .size(theme::TEXT_SMALL)
    .width(Length::Fixed(theme::HEX_INPUT_WIDTH))
    .on_input(Message::HexDraftChanged)
    .on_submit(Message::SubmitHex);

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

    let presets = row![presets, Space::with_width(Length::Fill), hex_box]
        .spacing(theme::SPACE_SM)
        .align_y(Alignment::Center);
    column![
        header,
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

/// What the colour controls are editing, in words: "Colour of row 3",
/// "Colour of column 2" or "Colour of key 14".
fn painted_label(app: &App) -> String {
    let (row, column) = app.lighting_cell;
    match app.paint_mode {
        PaintMode::Row => format!("{}{}", theme::LABEL_ROW_COLOR_PREFIX, row + 1),
        PaintMode::Column => format!("{}{}", theme::LABEL_COLUMN_COLOR_PREFIX, column + 1),
        PaintMode::Key => {
            let number = V2_PRO_ROWS
                .get(row)
                .and_then(|keys| keys.get(column))
                .and_then(|id| v2_pro_key_number(*id))
                .unwrap_or(0);
            format!("{}{number:02}", theme::LABEL_KEY_COLOR_PREFIX)
        }
    }
}

fn paint_mode_name(mode: PaintMode) -> &'static str {
    match mode {
        PaintMode::Row => theme::PAINT_MODE_ROW,
        PaintMode::Column => theme::PAINT_MODE_COLUMN,
        PaintMode::Key => theme::PAINT_MODE_KEY,
    }
}

/// `Paint: Row | Column | Key`, under the pad while a frame is being edited.
fn paint_modes(app: &App) -> Element<'_, Message> {
    let mut line = row![text(theme::LABEL_PAINT)
        .size(theme::TEXT_SMALL)
        .color(theme::COLOR_TEXT_FAINT)]
    .spacing(theme::SPACE_XS)
    .align_y(Alignment::Center);
    for mode in PaintMode::ALL {
        let style = if app.paint_mode == mode {
            selected_effect_button
        } else {
            quiet_button
        };
        line = line.push(
            button(text(paint_mode_name(mode)).size(theme::TEXT_SMALL))
                .padding([theme::SPACE_XS, theme::SPACE_MD])
                .on_press(Message::PaintModePick(mode))
                .style(style),
        );
    }
    line.into()
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
                selected: if custom {
                    app.painted_cells()
                } else {
                    Vec::new()
                },
                keys_pickable: offers_custom(app),
            },
            theme::LIGHT_PAD_HEIGHT,
        );
        let mut top = column![pad]
            .spacing(theme::SPACE_SM)
            .align_x(Alignment::Center);
        if app.model == Some(DeviceModel::V2Pro) {
            if custom {
                top = top.push(paint_modes(app));
            }
            top = top.push(
                text(pad_hint(app))
                    .size(theme::TEXT_SMALL)
                    .color(theme::COLOR_TEXT_FAINT),
            );
        }

        let mut controls = column![effect_chips(app), brightness_control(app)]
            .spacing(theme::SPACE_XL)
            .max_width(theme::LIGHTING_CONTROLS_WIDTH);
        if custom {
            controls = controls.push(animation_control(app));
        }
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

/// Every shelf the side panel offers: the shipped groups, then the user's.
const SHELVES: [PresetShelf; 5] = [
    PresetShelf::Group(PresetGroup::WorldOfWarcraft),
    PresetShelf::Group(PresetGroup::Games),
    PresetShelf::Group(PresetGroup::Aesthetic),
    PresetShelf::Group(PresetGroup::Functional),
    PresetShelf::Saved,
];
const SHELF_CHIPS_PER_ROW: usize = 3;

fn shelf_label(shelf: PresetShelf) -> &'static str {
    match shelf {
        PresetShelf::Group(PresetGroup::WorldOfWarcraft) => theme::PRESET_GROUP_WOW,
        PresetShelf::Group(PresetGroup::Games) => theme::PRESET_GROUP_GAMES,
        PresetShelf::Group(PresetGroup::Aesthetic) => theme::PRESET_GROUP_AESTHETIC,
        PresetShelf::Group(PresetGroup::Functional) => theme::PRESET_GROUP_FUNCTIONAL,
        PresetShelf::Saved => theme::PRESET_SHELF_SAVED,
    }
}

fn shelf_caption(shelf: PresetShelf) -> &'static str {
    match shelf {
        PresetShelf::Group(PresetGroup::WorldOfWarcraft) => theme::PRESET_CAPTION_WOW,
        PresetShelf::Group(PresetGroup::Games) => theme::PRESET_CAPTION_GAMES,
        PresetShelf::Group(PresetGroup::Aesthetic) => theme::PRESET_CAPTION_AESTHETIC,
        PresetShelf::Group(PresetGroup::Functional) => theme::PRESET_CAPTION_FUNCTIONAL,
        PresetShelf::Saved => theme::PRESET_CAPTION_SAVED,
    }
}

fn shelf_chips(app: &App) -> Element<'_, Message> {
    let mut grid = column![].spacing(theme::SPACE_XS);
    for chunk in SHELVES.chunks(SHELF_CHIPS_PER_ROW) {
        let mut line = row![].spacing(theme::SPACE_XS);
        for shelf in chunk {
            let style = if app.preset_shelf == *shelf {
                selected_effect_button
            } else {
                quiet_button
            };
            line = line.push(
                button(
                    container(text(shelf_label(*shelf)).size(theme::TEXT_SMALL))
                        .center_x(Length::Fill),
                )
                .width(Length::Fill)
                .padding([theme::SPACE_XS, theme::SPACE_SM])
                .on_press(Message::PresetShelfPick(*shelf))
                .style(style),
            );
        }
        for _ in chunk.len()..SHELF_CHIPS_PER_ROW {
            line = line.push(Space::with_width(Length::Fill));
        }
        grid = grid.push(line);
    }
    grid.into()
}

fn preset_button<'a>(
    frame: &LightFrame,
    name: &'a str,
    selected: bool,
    message: Message,
) -> iced::widget::Button<'a, Message> {
    let style = if selected {
        selected_row_button
    } else {
        row_button
    };
    button(
        row![frame_thumbnail(frame), text(name).size(theme::TEXT_BODY)]
            .spacing(theme::SPACE_MD)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([theme::SPACE_SM, theme::SPACE_SM])
    .on_press(message)
    .style(style)
}

fn shipped_list(app: &App, group: PresetGroup) -> Element<'_, Message> {
    let current = current_frame(app).and_then(preset_for_frame);
    let mut list = column![].spacing(theme::SPACE_XXS);
    for preset in PRESETS.iter().filter(|p| p.group == group) {
        let selected = current.is_some_and(|c| c.id == preset.id);
        list = list.push(preset_button(
            &preset.frame(),
            preset.name,
            selected,
            Message::LightFramePreset(preset.id),
        ));
    }
    list.into()
}

fn saved_list(app: &App) -> Element<'_, Message> {
    if app.saved_presets.is_empty() {
        return text(theme::PRESETS_SAVED_EMPTY)
            .size(theme::TEXT_SMALL)
            .color(theme::COLOR_TEXT_DIM)
            .into();
    }
    let current = current_frame(app);
    let mut list = column![].spacing(theme::SPACE_XXS);
    for (index, preset) in app.saved_presets.iter().enumerate() {
        let selected = current == Some(&preset.frame);
        list = list.push(
            row![
                preset_button(
                    &preset.frame,
                    &preset.name,
                    selected,
                    Message::LightSavedPreset(index),
                ),
                button(text(theme::BUTTON_DELETE_PRESET).size(theme::TEXT_BODY))
                    .padding([theme::SPACE_XXS, theme::SPACE_SM])
                    .on_press(Message::DeleteSavedPreset(index))
                    .style(danger_text_button),
            ]
            .spacing(theme::SPACE_XXS)
            .align_y(Alignment::Center),
        );
    }
    list.into()
}

/// `+ Save these colours` under the list, or the name box once pressed.
/// Enter saves, Escape or the `x` cancels. Only a custom frame can be saved.
fn save_control(app: &App) -> Option<Element<'_, Message>> {
    if app.lighting.effect != LightingEffect::Custom {
        return None;
    }
    Some(match &app.save_preset_name {
        None => button(text(theme::BUTTON_SAVE_PRESET).size(theme::TEXT_BODY))
            .width(Length::Fill)
            .padding([theme::SPACE_XS, theme::SPACE_MD])
            .on_press(Message::StartSavePreset)
            .style(accent_text_button)
            .into(),
        Some(name) => row![
            text_input(theme::SAVE_PRESET_PLACEHOLDER, name)
                .id(text_input::Id::new(SAVE_PRESET_INPUT_ID))
                .size(theme::TEXT_BODY)
                .on_input(Message::SavePresetNameChanged)
                .on_submit(Message::SubmitSavePreset),
            button(text(theme::BUTTON_CANCEL_SAVE_PRESET).size(theme::TEXT_BODY))
                .padding([theme::SPACE_XXS, theme::SPACE_SM])
                .on_press(Message::CancelSavePreset)
                .style(danger_text_button),
        ]
        .spacing(theme::SPACE_XXS)
        .align_y(Alignment::Center)
        .into(),
    })
}

/// The frame on the pad right now, if it is a custom one.
fn current_frame(app: &App) -> Option<&LightFrame> {
    if app.lighting.effect == LightingEffect::Custom {
        app.lighting.frame.as_ref()
    } else {
        None
    }
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
    let list = match app.preset_shelf {
        PresetShelf::Group(group) => shipped_list(app, group),
        PresetShelf::Saved => saved_list(app),
    };
    let mut panel = column![
        title,
        shelf_chips(app),
        text(shelf_caption(app.preset_shelf))
            .size(theme::TEXT_SMALL)
            .color(theme::COLOR_TEXT_FAINT),
        scrollable(container(list).padding(iced::Padding::ZERO.right(theme::SPACE_MD)))
            .height(Length::Fill),
    ]
    .spacing(theme::SPACE_MD);
    if let Some(save) = save_control(app) {
        panel = panel.push(save);
    }
    panel.into()
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
    fn preview_presses_land_on_face_keys_and_age_forward() {
        let presses = preview_presses(5.0);
        assert_eq!(presses.len(), PREVIEW_PRESSES);
        for press in &presses {
            assert!(press.column < V2_PRO_ROWS[press.row].len());
            assert!(press.age >= 0.0);
        }
        assert!(
            preview_presses(0.1).len() == 1,
            "no presses before the start"
        );
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
