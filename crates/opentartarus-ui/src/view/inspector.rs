use crate::app::{App, Message, ProfileRow, StickChoice};
use crate::keys::COMBO_INPUT_ID;
use crate::theme;
use crate::view::widgets::{
    chip, danger_text_button, menu_style, pick_list_style, primary_button, quiet_button,
    section_label, surface_container, switch_track,
};
use iced::widget::{
    button, column, container, pick_list, row, scrollable, slider, text, text_input, Space,
};
use iced::{Alignment, Background, Border, Element, Length, Theme};
use opentartarus_core::constants::{STICK_ROTATION_MAX_DEG, STICK_ROTATION_STEP_DEG};
use opentartarus_core::labels::{bind_label, key_display_name_for, key_position_text_for};
use opentartarus_core::types::{
    Action, KeyId, KeyToken, Layer, MouseButton, MouseTarget, ScrollDir, StickMode,
};

pub const MOUSE_TARGETS: [(&str, MouseTarget); 7] = [
    (
        theme::MOUSE_LEFT,
        MouseTarget::Button {
            button: MouseButton::Left,
        },
    ),
    (
        theme::MOUSE_RIGHT,
        MouseTarget::Button {
            button: MouseButton::Right,
        },
    ),
    (
        theme::MOUSE_MIDDLE,
        MouseTarget::Button {
            button: MouseButton::Middle,
        },
    ),
    (
        theme::MOUSE_BACK,
        MouseTarget::Button {
            button: MouseButton::Back,
        },
    ),
    (
        theme::MOUSE_FORWARD,
        MouseTarget::Button {
            button: MouseButton::Forward,
        },
    ),
    (
        theme::MOUSE_WHEEL_UP,
        MouseTarget::Scroll {
            scroll: ScrollDir::Up,
        },
    ),
    (
        theme::MOUSE_WHEEL_DOWN,
        MouseTarget::Scroll {
            scroll: ScrollDir::Down,
        },
    ),
];

/// iced 0.13 has no flow layout, so the chips wrap in two fixed rows.
const MOUSE_ROW_ONE: usize = 4;

/// Keys the combo box cannot type, because pressing one on its own is a
/// modifier there: offered as chips instead. Sprint, crouch and walk live
/// on these in most games.
pub const HELD_KEYS: [(&str, KeyToken); 3] = [
    (theme::HELD_SHIFT, KeyToken::LeftShift),
    (theme::HELD_CTRL, KeyToken::LeftCtrl),
    (theme::HELD_ALT, KeyToken::LeftAlt),
];

/// What the big readout shows, and whether the hold-repeat toggle is live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldRepeat {
    pub on: bool,
    pub enabled: bool,
}

pub fn bind_readout(action: Option<&Action>, profiles: &[ProfileRow]) -> String {
    let label = switch_readout(action, profiles)
        .or_else(|| mouse_target_readout(action))
        .unwrap_or_else(|| bind_label(action));
    if label.is_empty() {
        String::from(theme::UNBOUND_PLACEHOLDER)
    } else {
        label
    }
}

/// The big readout names a mouse target the same way the picker chips do
/// ("Left", "Wheel+"), not `bind_label`'s compact form ("M1"): that form
/// is for the tight space on a keycap in `keypad.rs`, not this headline
/// value. Looks the label up in `MOUSE_TARGETS` rather than restating it,
/// so the readout and the chip that picked it can never say different
/// things. `HoldRepeat` is unwrapped the same way `bind_label` unwraps it.
fn mouse_target_readout(action: Option<&Action>) -> Option<String> {
    match action {
        Some(Action::Mouse { target }) => MOUSE_TARGETS
            .iter()
            .find(|(_, candidate)| candidate == target)
            .map(|(label, _)| String::from(*label)),
        Some(Action::HoldRepeat { inner, .. }) => mouse_target_readout(Some(inner)),
        _ => None,
    }
}

/// A switch key reads as the profile it jumps to, by name. A profile that
/// has since been deleted still reads by id, so the key never looks unbound.
fn switch_readout(action: Option<&Action>, profiles: &[ProfileRow]) -> Option<String> {
    match action {
        Some(Action::SwitchProfile { profile }) => {
            let name = profiles
                .iter()
                .find(|row| &row.id == profile)
                .map_or(profile.as_str(), |row| row.name.as_str());
            Some(format!("{}{name}", theme::READOUT_SWITCH_PREFIX))
        }
        Some(Action::NextProfile) => Some(String::from(theme::READOUT_NEXT_PROFILE)),
        _ => None,
    }
}

/// Whether the selected binding sends more than one action per press. Those
/// are the bindings most games' rules forbid, so the panel says so.
pub fn automates_input(action: Option<&Action>) -> bool {
    matches!(
        action,
        Some(Action::Macro { .. }) | Some(Action::HoldRepeat { .. })
    )
}

fn automation_warning<'a>() -> Element<'a, Message> {
    text(theme::AUTOMATION_WARNING)
        .size(theme::TEXT_SMALL)
        .color(theme::COLOR_DANGER)
        .into()
}

/// Only a plain key can repeat. A hold-repeat that already wraps something
/// else reads as on, but the toggle stays dead so the user cannot make it
/// worse.
pub fn hold_repeat_state(action: Option<&Action>) -> HoldRepeat {
    match action {
        Some(Action::Key { .. }) => HoldRepeat {
            on: false,
            enabled: true,
        },
        Some(Action::HoldRepeat { inner, .. }) => HoldRepeat {
            on: true,
            enabled: matches!(inner.as_ref(), Action::Key { .. }),
        },
        _ => HoldRepeat {
            on: false,
            enabled: false,
        },
    }
}

/// Shown while no key is selected. When the active profile needs a step in
/// the game first, that step is spelled out here, where the player looks
/// before touching any key.
fn empty_state(app: &App) -> Element<'_, Message> {
    let mut body = column![
        text(theme::EMPTY_STATE_TITLE)
            .size(theme::TEXT_HEADING)
            .color(theme::COLOR_TEXT_DIM),
        text(theme::EMPTY_STATE_BODY)
            .size(theme::TEXT_BODY)
            .color(theme::COLOR_TEXT_FAINT),
    ]
    .spacing(theme::SPACE_SM)
    .align_x(Alignment::Center);
    if let Some(note) = &app.setup_note {
        body = body.push(
            column![
                section_label(theme::LABEL_SETUP_NOTE),
                text(note)
                    .size(theme::TEXT_SMALL)
                    .color(theme::COLOR_TEXT_DIM)
                    .center(),
            ]
            .spacing(theme::SPACE_XS)
            .padding([theme::SPACE_LG, 0.0])
            .align_x(Alignment::Center),
        );
    }
    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn readout<'a>(action: Option<&Action>, profiles: &[ProfileRow]) -> Element<'a, Message> {
    container(
        column![
            section_label(theme::LABEL_SENDS),
            text(bind_readout(action, profiles)).size(theme::BIND_VALUE_TEXT),
        ]
        .spacing(theme::SPACE_XS)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(theme::SPACE_LG)
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(theme::COLOR_RAISED)),
        text_color: Some(theme::COLOR_TEXT),
        border: Border {
            color: theme::COLOR_LINE,
            width: theme::BORDER_HAIRLINE,
            radius: theme::RADIUS_CONTROL.into(),
        },
        ..container::Style::default()
    })
    .into()
}

fn switch<'a>(state: HoldRepeat) -> Element<'a, Message> {
    let track = switch_track(state.on);

    let label_color = if state.enabled {
        theme::COLOR_TEXT
    } else {
        theme::COLOR_TEXT_FAINT
    };
    let mut toggle = button(track).padding(0).style(quiet_button);
    if state.enabled {
        toggle = toggle.on_press(Message::HoldRepeat(!state.on));
    }

    row![
        text(theme::HOLD_REPEAT_LABEL)
            .size(theme::TEXT_BODY)
            .color(label_color),
        Space::with_width(Length::Fill),
        toggle,
    ]
    .align_y(Alignment::Center)
    .into()
}

fn mouse_chips<'a>() -> Element<'a, Message> {
    let mut first = row![].spacing(theme::SPACE_XS);
    let mut second = row![].spacing(theme::SPACE_XS);
    for (index, (label, target)) in MOUSE_TARGETS.into_iter().enumerate() {
        if index < MOUSE_ROW_ONE {
            first = first.push(chip(label, Message::MousePick(target)));
        } else {
            second = second.push(chip(label, Message::MousePick(target)));
        }
    }
    column![section_label(theme::LABEL_MOUSE), first, second]
        .spacing(theme::SPACE_SM)
        .into()
}

fn held_key_chips<'a>() -> Element<'a, Message> {
    let mut line = row![].spacing(theme::SPACE_XS);
    for (label, key) in HELD_KEYS {
        line = line.push(chip(label, Message::KeyPick(key)));
    }
    column![section_label(theme::LABEL_HELD_KEYS), line]
        .spacing(theme::SPACE_SM)
        .into()
}

fn macro_steps<'a>(action: &Action) -> Element<'a, Message> {
    let Action::Macro { steps } = action else {
        return Space::with_height(0).into();
    };
    let mut list = column![section_label(theme::LABEL_MACRO)].spacing(theme::SPACE_SM);
    for (index, step) in steps.iter().enumerate() {
        let delay = step.delay_ms.to_string();
        list = list.push(
            row![
                text(format!("{}{}", theme::MACRO_STEP_PREFIX, index + 1))
                    .size(theme::TEXT_SMALL)
                    .width(Length::Fill),
                text_input(theme::MACRO_DELAY_PLACEHOLDER, &delay)
                    .size(theme::TEXT_SMALL)
                    .on_input(move |raw| Message::MacroDelay(index, raw))
                    .width(Length::Fixed(theme::MENU_WIDTH / 3.0)),
                button(text(theme::BUTTON_DELETE_STEP))
                    .on_press(Message::DeleteMacroStep(index))
                    .style(danger_text_button),
            ]
            .spacing(theme::SPACE_SM)
            .align_y(Alignment::Center),
        );
    }
    list = list.push(
        button(text(theme::BUTTON_ADD_STEP).size(theme::TEXT_SMALL))
            .on_press(Message::AddMacroStep)
            .style(quiet_button),
    );
    list.into()
}

/// `Next profile` as a chip, and a drop-down of every profile to jump to.
/// A drop-down rather than chips: there are sixteen shipped profiles plus
/// the user's own, which is too many chips for the panel.
fn switch_profile_controls(app: &App) -> Element<'_, Message> {
    let next = chip(theme::CHIP_NEXT_PROFILE, Message::NextProfilePick);
    let jump = pick_list(
        app.profile_choices(),
        app.switch_target_choice(),
        Message::SwitchProfilePick,
    )
    .placeholder(theme::PROFILE_PICK_PLACEHOLDER)
    .text_size(theme::TEXT_SMALL)
    .style(pick_list_style)
    .menu_style(menu_style)
    .width(Length::Fill);
    column![
        section_label(theme::LABEL_SWITCH_PROFILE),
        row![next, jump]
            .spacing(theme::SPACE_XS)
            .align_y(Alignment::Center),
    ]
    .spacing(theme::SPACE_SM)
    .into()
}

const STICK_CHOICES: [StickChoice; 4] = [
    StickChoice(StickMode::Keys8),
    StickChoice(StickMode::Keys4),
    StickChoice(StickMode::AnalogLeft),
    StickChoice(StickMode::AnalogRight),
];

/// How the stick is read (per profile) and how far it is turned (for all
/// profiles).
fn stick_controls(app: &App) -> Element<'_, Message> {
    let mode = pick_list(
        STICK_CHOICES,
        Some(StickChoice(app.stick_mode)),
        Message::StickModePick,
    )
    .text_size(theme::TEXT_SMALL)
    .style(pick_list_style)
    .menu_style(menu_style)
    .width(Length::Fill);
    let rotation = row![
        slider(
            -STICK_ROTATION_MAX_DEG..=STICK_ROTATION_MAX_DEG,
            app.stick_rotation_deg,
            Message::StickRotationChanged,
        )
        .step(STICK_ROTATION_STEP_DEG)
        .on_release(Message::StickRotationReleased),
        text(format!("{}°", app.stick_rotation_deg))
            .size(theme::TEXT_SMALL)
            .width(Length::Shrink),
    ]
    .spacing(theme::SPACE_SM)
    .align_y(Alignment::Center);
    let mut panel = column![section_label(theme::LABEL_THUMBSTICK), mode].spacing(theme::SPACE_SM);
    if app.stick_mode.is_analog() {
        panel = panel.push(
            text(theme::STICK_ANALOG_HINT)
                .size(theme::TEXT_SMALL)
                .color(theme::COLOR_TEXT_DIM),
        );
    }
    panel
        .push(section_label(theme::LABEL_STICK_ROTATION))
        .push(rotation)
        .into()
}

/// What the inspector says about Hypershift for the selected key, if
/// anything: how the Hypershift key behaves, or that an edit lands in the
/// Hypershift layer.
pub fn layer_note(layer: Layer, key_id: KeyId, action: Option<&Action>) -> Option<&'static str> {
    match (layer, key_id, action) {
        (Layer::Hypershift, _, _) => Some(theme::HYPERSHIFT_LAYER_HINT),
        (Layer::Base, KeyId::Hypershift, None) => Some(theme::HYPERSHIFT_KEY_HINT),
        (Layer::Base, KeyId::Hypershift, Some(_)) => Some(theme::HYPERSHIFT_KEY_BOUND_HINT),
        _ => None,
    }
}

pub fn inspector(app: &App) -> Element<'_, Message> {
    let content: Element<'_, Message> = match app.selected_key {
        _ if app.tab == crate::app::Tab::Lighting => crate::view::lighting_tab::presets_panel(app),
        None => empty_state(app),
        Some(key_id) => {
            let action = app.selected_action();
            let state = hold_repeat_state(action.as_ref());
            let record_label = if app.recording {
                theme::BUTTON_CANCEL
            } else {
                theme::BUTTON_RECORD_KEY
            };
            let record_message = if app.recording {
                Message::StopRecord
            } else {
                Message::Record
            };

            let mut panel = column![
                row![
                    text(key_display_name_for(app.model, key_id)).size(theme::TEXT_TITLE),
                    text(key_position_text_for(app.model, key_id))
                        .size(theme::TEXT_SMALL)
                        .color(theme::COLOR_TEXT_FAINT),
                ]
                .spacing(theme::SPACE_SM)
                .align_y(Alignment::End),
                readout(action.as_ref(), &app.profiles),
                button(text(record_label).size(theme::TEXT_BODY))
                    .width(Length::Fill)
                    .padding(theme::SPACE_SM)
                    .on_press(record_message)
                    .style(primary_button),
                text_input(theme::COMBO_PLACEHOLDER, &app.combo_text)
                    .id(iced::widget::text_input::Id::new(COMBO_INPUT_ID))
                    .size(theme::TEXT_BODY)
                    .on_input(Message::ComboChanged),
                switch(state),
                held_key_chips(),
                mouse_chips(),
                switch_profile_controls(app),
            ]
            .spacing(theme::SPACE_MD);

            if let Some(note) = layer_note(app.shown_layer(), key_id, action.as_ref()) {
                panel = panel.push(
                    text(note)
                        .size(theme::TEXT_SMALL)
                        .color(theme::COLOR_TEXT_DIM),
                );
            }
            if app.shows_stick_settings() {
                panel = panel.push(stick_controls(app));
            }
            if let Some(macro_action @ Action::Macro { .. }) = &action {
                panel = panel.push(macro_steps(macro_action));
            }
            if automates_input(action.as_ref()) {
                panel = panel.push(automation_warning());
            }

            // `Clear this binding` sits below the scroll area, not inside it.
            // iced panics outright if a scrollable's content fills the axis it
            // scrolls, so the spacer that used to push this button down cannot
            // live inside the scrollable.
            let clear = button(text(theme::BUTTON_CLEAR_BINDING).size(theme::TEXT_SMALL))
                .width(Length::Fill)
                .padding(theme::SPACE_SM)
                .on_press(Message::ClearBinding)
                .style(danger_text_button);

            column![scrollable(panel).height(Length::Fill), clear,]
                .spacing(theme::SPACE_MD)
                .height(Length::Fill)
                .into()
        }
    };

    container(content)
        .width(Length::Fixed(theme::INSPECTOR_WIDTH))
        .height(Length::Fill)
        .padding(theme::SPACE_LG)
        .style(surface_container)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentartarus_core::types::{Action, KeyToken, MouseButton, MouseTarget};

    fn key(token: KeyToken) -> Action {
        Action::Key {
            key: token,
            modifiers: vec![],
        }
    }

    #[test]
    fn an_unbound_key_reads_as_a_dash_not_an_empty_box() {
        assert_eq!(bind_readout(None, &[]), theme::UNBOUND_PLACEHOLDER);
    }

    #[test]
    fn a_bound_key_reads_as_its_label() {
        assert_eq!(bind_readout(Some(&key(KeyToken::Q)), &[]), "Q");
        let mouse = Action::Mouse {
            target: MouseTarget::Button {
                button: MouseButton::Left,
            },
        };
        assert_eq!(bind_readout(Some(&mouse), &[]), theme::MOUSE_LEFT);
    }

    #[test]
    fn hold_repeat_is_offered_only_for_plain_key_bindings() {
        let plain = hold_repeat_state(Some(&key(KeyToken::Q)));
        assert!(plain.enabled, "a plain key can repeat");
        assert!(!plain.on);

        let held = Action::HoldRepeat {
            inner: Box::new(key(KeyToken::Q)),
            rate_ms: 40,
        };
        let held = hold_repeat_state(Some(&held));
        assert!(held.enabled);
        assert!(held.on, "an existing hold-repeat reads as on");
    }

    #[test]
    fn hold_repeat_is_refused_for_macros_mouse_and_nothing() {
        let macro_action = Action::Macro { steps: Vec::new() };
        assert!(!hold_repeat_state(Some(&macro_action)).enabled);

        let mouse = Action::Mouse {
            target: MouseTarget::Button {
                button: MouseButton::Right,
            },
        };
        assert!(!hold_repeat_state(Some(&mouse)).enabled);

        let nothing = hold_repeat_state(None);
        assert!(!nothing.enabled);
        assert!(!nothing.on);
    }

    #[test]
    fn hold_repeat_wrapping_a_macro_is_not_editable() {
        let odd = Action::HoldRepeat {
            inner: Box::new(Action::Macro { steps: Vec::new() }),
            rate_ms: 40,
        };
        let state = hold_repeat_state(Some(&odd));
        assert!(state.on, "it is on, because it is wrapped");
        assert!(!state.enabled, "but the toggle must not offer to change it");
    }

    #[test]
    fn the_held_key_chips_are_the_three_bare_modifiers_with_distinct_labels() {
        let keys: Vec<KeyToken> = HELD_KEYS.iter().map(|(_, key)| *key).collect();
        assert_eq!(
            keys,
            vec![KeyToken::LeftShift, KeyToken::LeftCtrl, KeyToken::LeftAlt]
        );
        let mut labels: Vec<&str> = HELD_KEYS.iter().map(|(label, _)| *label).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), HELD_KEYS.len());
    }

    #[test]
    fn only_macros_and_hold_repeat_count_as_automation() {
        assert!(automates_input(Some(&Action::Macro { steps: Vec::new() })));
        assert!(automates_input(Some(&Action::HoldRepeat {
            inner: Box::new(key(KeyToken::Q)),
            rate_ms: 40,
        })));
        assert!(!automates_input(Some(&key(KeyToken::Q))));
        assert!(!automates_input(Some(&Action::NextProfile)));
        assert!(!automates_input(None));
    }

    #[test]
    fn every_mouse_target_gets_exactly_one_chip() {
        assert_eq!(MOUSE_TARGETS.len(), 7);
        let mut labels: Vec<&str> = MOUSE_TARGETS.iter().map(|(label, _)| *label).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), MOUSE_TARGETS.len(), "labels must be unique");
    }

    #[test]
    fn a_switch_reads_as_the_profile_name_and_falls_back_to_the_id() {
        let rows = vec![ProfileRow {
            id: "dota-2".into(),
            name: "Dota 2".into(),
            is_active: false,
            can_revert: true,
            can_delete: false,
            color: None,
        }];
        let jump = Action::SwitchProfile {
            profile: "dota-2".into(),
        };
        assert_eq!(bind_readout(Some(&jump), &rows), "Switch to Dota 2");
        let gone = Action::SwitchProfile {
            profile: "deleted".into(),
        };
        assert_eq!(bind_readout(Some(&gone), &rows), "Switch to deleted");
        assert_eq!(
            bind_readout(Some(&Action::NextProfile), &rows),
            "Next profile"
        );
        assert!(
            !hold_repeat_state(Some(&jump)).enabled,
            "a switch cannot repeat"
        );
    }
}
