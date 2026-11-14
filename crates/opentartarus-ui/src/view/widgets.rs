use crate::app::{Message, Tab};
use crate::theme;
use iced::border::Radius;
use iced::overlay::menu;
use iced::widget::{button, container, pick_list, text, Space};
use iced::{Background, Border, Color, Element, Length, Shadow, Theme, Vector};

// This module is the shared style vocabulary the view modules draw from.
// Every function below is covered by the tests at the bottom of this file.

/// A raised panel: the keypad card, the bind readout, the menu popup.
pub fn card_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(theme::COLOR_SURFACE)),
        text_color: Some(theme::COLOR_TEXT),
        border: Border {
            color: theme::COLOR_LINE,
            width: theme::BORDER_HAIRLINE,
            radius: theme::RADIUS_CARD.into(),
        },
        ..container::Style::default()
    }
}

/// A flat band: the sidebar and the inspector. Square, because these sit
/// between the header and the status bar and never touch a window corner.
pub fn surface_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(theme::COLOR_SURFACE)),
        text_color: Some(theme::COLOR_TEXT),
        ..container::Style::default()
    }
}

/// The top band. iced does not clip a child to its parent's rounded corners,
/// so the header must round its own two upper corners or it paints square over
/// the window pane's curve.
pub fn header_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(theme::COLOR_SURFACE)),
        text_color: Some(theme::COLOR_TEXT),
        border: Border {
            color: Color::TRANSPARENT,
            width: theme::BORDER_NONE,
            radius: Radius::new(theme::BORDER_NONE)
                .top_left(crate::app::window_radius())
                .top_right(crate::app::window_radius()),
        },
        ..container::Style::default()
    }
}

/// The bottom band, rounding the window pane's two lower corners.
pub fn status_container(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(theme::COLOR_SURFACE)),
        text_color: Some(theme::COLOR_TEXT),
        border: Border {
            color: Color::TRANSPARENT,
            width: theme::BORDER_NONE,
            radius: Radius::new(theme::BORDER_NONE)
                .bottom_left(crate::app::window_radius())
                .bottom_right(crate::app::window_radius()),
        },
        ..container::Style::default()
    }
}

fn control(
    background: Color,
    hovered: Color,
    text_color: Color,
    border_color: Color,
    radius: f32,
    border_width: f32,
    status: button::Status,
) -> button::Style {
    let fill = match status {
        button::Status::Hovered | button::Status::Pressed => hovered,
        _ => background,
    };
    button::Style {
        background: Some(Background::Color(fill)),
        text_color,
        border: Border {
            color: border_color,
            width: border_width,
            radius: radius.into(),
        },
        ..button::Style::default()
    }
}

/// The shadow a cap casts at rest, or while pressed into its panel.
pub fn cap_shadow(pressed: bool) -> Shadow {
    let (drop, blur) = if pressed {
        (theme::CAP_PRESSED_DROP, theme::CAP_PRESSED_BLUR)
    } else {
        (theme::CAP_SHADOW_DROP, theme::CAP_SHADOW_BLUR)
    };
    Shadow {
        color: Color {
            a: theme::CAP_SHADOW_ALPHA,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, drop),
        blur_radius: blur,
    }
}

/// The app's one material: a key.
///
/// Every control the user can push is built from this, so the window reads as
/// the same slab of caps the device itself is. A cap sits up off its panel,
/// carries a lit top edge, and presses in — losing the edge, darkening, and
/// pulling its shadow tight — when pushed.
pub fn keycap(
    resting: Color,
    hovered: Color,
    text_color: Color,
    radius: f32,
    status: button::Status,
) -> button::Style {
    let pressed = matches!(status, button::Status::Pressed);
    let fill = match status {
        button::Status::Pressed => theme::COLOR_KEY_PRESSED,
        button::Status::Hovered => hovered,
        _ => resting,
    };
    button::Style {
        background: Some(Background::Color(fill)),
        text_color,
        border: Border {
            // The lit edge is what makes it a cap rather than a swatch. A
            // pressed cap has sunk below the light, so it loses the edge.
            color: if pressed {
                Color::TRANSPARENT
            } else {
                theme::COLOR_CAP_EDGE
            },
            width: theme::BORDER_HAIRLINE,
            radius: radius.into(),
        },
        shadow: cap_shadow(pressed),
    }
}

/// The single accent-filled action on any screen.
pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let mut style = keycap(
        theme::COLOR_ACCENT,
        theme::COLOR_ACCENT_HOVER,
        Color::WHITE,
        theme::RADIUS_CONTROL,
        status,
    );
    // The accent cap keeps its own colour when pressed rather than falling
    // back to the neutral pressed fill, or the primary action would look
    // disabled at the moment it is used.
    if matches!(status, button::Status::Pressed) {
        style.background = Some(Background::Color(theme::COLOR_ACCENT_PRESSED));
    }
    style
}

/// A normal secondary control.
pub fn quiet_button(_theme: &Theme, status: button::Status) -> button::Style {
    keycap(
        theme::COLOR_KEY,
        theme::COLOR_KEY_HOVER,
        theme::COLOR_TEXT,
        theme::RADIUS_CONTROL,
        status,
    )
}

/// A small cap, used for the mouse targets and the profile switch. It has
/// the same corner as every other control: a pill's long curve turned the
/// hairline edge into a dotted line.
pub fn chip_button(_theme: &Theme, status: button::Status) -> button::Style {
    keycap(
        theme::COLOR_KEY,
        theme::COLOR_KEY_HOVER,
        theme::COLOR_TEXT,
        theme::RADIUS_CONTROL,
        status,
    )
}

/// A chip: a small cap with a short label. Built here so every chip gets
/// the same tight line box, which is what keeps the label centred.
pub fn chip<'a>(label: &'a str, message: Message) -> iced::widget::Button<'a, Message> {
    button(
        text(label)
            .size(theme::TEXT_SMALL)
            .line_height(theme::LINE_HEIGHT_TIGHT),
    )
    .padding([theme::SPACE_XS, theme::SPACE_MD])
    .on_press(message)
    .style(chip_button)
}

/// The drop-down, in the same material as a resting cap. iced's stock style
/// is light grey, which would be the one thing in the window not built from
/// the app's own material.
pub fn pick_list_style(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let fill = match status {
        pick_list::Status::Active => theme::COLOR_KEY,
        pick_list::Status::Hovered | pick_list::Status::Opened => theme::COLOR_KEY_HOVER,
    };
    pick_list::Style {
        text_color: theme::COLOR_TEXT,
        placeholder_color: theme::COLOR_TEXT_FAINT,
        handle_color: theme::COLOR_TEXT_DIM,
        background: Background::Color(fill),
        border: Border {
            color: theme::COLOR_CAP_EDGE,
            width: theme::BORDER_HAIRLINE,
            radius: theme::RADIUS_CONTROL.into(),
        },
    }
}

/// The drop-down's open list: a raised panel, with the chosen row in the
/// soft accent the sidebar uses for its selected row.
pub fn menu_style(_theme: &Theme) -> menu::Style {
    menu::Style {
        background: Background::Color(theme::COLOR_RAISED),
        border: Border {
            color: theme::COLOR_LINE,
            width: theme::BORDER_HAIRLINE,
            radius: theme::RADIUS_CONTROL.into(),
        },
        text_color: theme::COLOR_TEXT,
        selected_text_color: theme::COLOR_TEXT,
        selected_background: Background::Color(theme::COLOR_ACCENT_SOFT),
    }
}

/// An unselected sidebar row.
pub fn row_button(_theme: &Theme, status: button::Status) -> button::Style {
    control(
        Color::TRANSPARENT,
        theme::COLOR_RAISED,
        theme::COLOR_TEXT_DIM,
        Color::TRANSPARENT,
        theme::RADIUS_CONTROL,
        theme::BORDER_NONE,
        status,
    )
}

/// The selected sidebar row: an accent tint carries the selection, with no
/// border in any state (redesign spec, sidebar section).
pub fn selected_row_button(_theme: &Theme, status: button::Status) -> button::Style {
    control(
        theme::COLOR_ACCENT_SOFT,
        theme::COLOR_ACCENT_SOFT,
        theme::COLOR_TEXT,
        Color::TRANSPARENT,
        theme::RADIUS_CONTROL,
        theme::BORDER_NONE,
        status,
    )
}

/// The selected lighting effect card. Unlike `selected_row_button`, a card
/// in a grid has no other cue for which effect is active, so this style
/// keeps a visible accent border to carry that meaning.
pub fn selected_effect_button(_theme: &Theme, status: button::Status) -> button::Style {
    control(
        theme::COLOR_ACCENT_SOFT,
        theme::COLOR_ACCENT_SOFT,
        theme::COLOR_TEXT,
        theme::COLOR_ACCENT,
        theme::RADIUS_CONTROL,
        theme::BORDER_SELECTED,
        status,
    )
}

/// Destructive text with no fill: Clear this binding, delete a macro step.
pub fn danger_text_button(_theme: &Theme, status: button::Status) -> button::Style {
    control(
        Color::TRANSPARENT,
        theme::with_opacity(theme::COLOR_DANGER, theme::ACCENT_SOFT_ALPHA),
        theme::COLOR_DANGER,
        Color::TRANSPARENT,
        theme::RADIUS_CONTROL,
        theme::BORDER_NONE,
        status,
    )
}

/// Accent-coloured text with no fill: a recovery action such as reverting a
/// profile, which should read as "go back," not as destructive.
pub fn accent_text_button(_theme: &Theme, status: button::Status) -> button::Style {
    control(
        Color::TRANSPARENT,
        theme::with_opacity(theme::COLOR_ACCENT, theme::ACCENT_SOFT_ALPHA),
        theme::COLOR_ACCENT,
        Color::TRANSPARENT,
        theme::RADIUS_CONTROL,
        theme::BORDER_NONE,
        status,
    )
}

/// A title-bar control. Quiet: just the glyph at rest, a soft fill under the
/// pointer. The caps are for things that map to keys on the device; window
/// controls are not, so they stay out of the way.
pub fn window_control_button(_theme: &Theme, status: button::Status) -> button::Style {
    control(
        Color::TRANSPARENT,
        theme::COLOR_RAISED,
        theme::COLOR_TEXT_DIM,
        Color::TRANSPARENT,
        theme::RADIUS_CONTROL,
        theme::BORDER_NONE,
        status,
    )
}

/// The close control. Same quiet treatment, but it lights red under the
/// pointer, the one place in the row where colour carries meaning.
pub fn close_control_button(_theme: &Theme, status: button::Status) -> button::Style {
    control(
        Color::TRANSPARENT,
        theme::COLOR_DANGER,
        theme::COLOR_TEXT_DIM,
        Color::TRANSPARENT,
        theme::RADIUS_CONTROL,
        theme::BORDER_NONE,
        status,
    )
}

/// The app menu's own button: a rounded square, not a circle, because circles
/// in this header mean "window control". Quiet until hovered.
pub fn app_menu_button(_theme: &Theme, status: button::Status) -> button::Style {
    // Same quiet treatment as the window controls, so the four read as one
    // row of glyphs rather than one odd one out.
    control(
        Color::TRANSPARENT,
        theme::COLOR_RAISED,
        theme::COLOR_TEXT_DIM,
        Color::TRANSPARENT,
        theme::RADIUS_CONTROL,
        theme::BORDER_NONE,
        status,
    )
}

/// The glyph colour for a window control, given whether the pointer is on it.
/// The close control goes white on its red hover; the others just brighten.
pub fn window_glyph_color(hovered: bool, is_close: bool) -> Color {
    match (hovered, is_close) {
        (true, true) => Color::WHITE,
        (true, false) => theme::COLOR_TEXT,
        (false, _) => theme::COLOR_TEXT_DIM,
    }
}

/// The small uppercase label above a group of controls.
/// Takes an owned string so callers can pass a freshly built label, such as
/// the brightness percentage, without fighting the borrow checker.
pub fn section_label<'a>(label: impl Into<String>) -> Element<'a, Message> {
    text(label.into().to_uppercase())
        .size(theme::TEXT_LABEL)
        .color(theme::COLOR_TEXT_FAINT)
        .into()
}

/// The status dot colour: green while the daemon runs, faint otherwise.
pub fn hairline_color(status_ok: bool) -> Color {
    if status_ok {
        theme::COLOR_OK
    } else {
        theme::COLOR_TEXT_FAINT
    }
}

/// A small filled circle, used in the header pill and the status bar.
pub fn dot<'a>(color: Color) -> Element<'a, Message> {
    container(Space::new(
        Length::Fixed(theme::STATUS_DOT_SIZE),
        Length::Fixed(theme::STATUS_DOT_SIZE),
    ))
    .style(move |_theme: &Theme| container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: theme::RADIUS_PILL.into(),
        },
        ..container::Style::default()
    })
    .into()
}

/// A colour chip: the profile swatch, the lighting presets.
/// An on/off switch: a pill, accent-filled when on. The caller wraps it in
/// a button and decides what a press does.
pub fn switch_track<'a>(on: bool) -> Element<'a, Message> {
    let fill = if on {
        theme::COLOR_ACCENT
    } else {
        theme::COLOR_RAISED
    };
    container(Space::new(
        Length::Fixed(theme::SWITCH_WIDTH),
        Length::Fixed(theme::SWITCH_HEIGHT),
    ))
    .style(move |_theme: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            color: Color::TRANSPARENT,
            width: theme::BORDER_NONE,
            radius: theme::RADIUS_PILL.into(),
        },
        ..container::Style::default()
    })
    .into()
}

pub fn swatch<'a>(rgb: [u8; 3], size: f32) -> Element<'a, Message> {
    let color = Color::from_rgb8(rgb[0], rgb[1], rgb[2]);
    container(Space::new(Length::Fixed(size), Length::Fixed(size)))
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(color)),
            border: Border {
                color: theme::COLOR_LINE_STRONG,
                width: theme::BORDER_HAIRLINE,
                radius: theme::RADIUS_CONTROL.into(),
            },
            ..container::Style::default()
        })
        .into()
}

const TAB_ORDER: [(Tab, &str); 2] = [
    (Tab::Keys, theme::TAB_KEYS),
    (Tab::Lighting, theme::TAB_LIGHTING),
];

/// Returned as a function so the test can inspect both states without
/// rendering the strip.
pub fn tab_button_style(active: bool) -> fn(&Theme, button::Status) -> button::Style {
    if active {
        active_tab_button
    } else {
        idle_tab_button
    }
}

fn active_tab_button(_theme: &Theme, _status: button::Status) -> button::Style {
    control(
        theme::COLOR_RAISED,
        theme::COLOR_RAISED,
        theme::COLOR_TEXT,
        Color::TRANSPARENT,
        theme::RADIUS_CONTROL,
        theme::BORDER_NONE,
        button::Status::Active,
    )
}

fn idle_tab_button(_theme: &Theme, status: button::Status) -> button::Style {
    control(
        Color::TRANSPARENT,
        theme::COLOR_RAISED,
        theme::COLOR_TEXT_DIM,
        Color::TRANSPARENT,
        theme::RADIUS_CONTROL,
        theme::BORDER_NONE,
        status,
    )
}

/// The Keys / Lighting switcher shown above the centre panel. Lives here,
/// not in `keys_tab.rs` or `lighting_tab.rs`, so both tabs render the exact
/// same strip from one implementation.
pub fn tab_strip<'a>(active: Tab) -> Element<'a, Message> {
    let mut strip = iced::widget::row![].spacing(theme::SPACE_XS);
    for (tab, label) in TAB_ORDER {
        strip = strip.push(
            button(text(label).size(theme::TEXT_SMALL))
                .padding([theme::SPACE_SM, theme::SPACE_LG])
                .on_press(Message::SelectTab(tab))
                .style(tab_button_style(tab == active)),
        );
    }
    container(strip.padding(theme::SPACE_XS))
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(theme::COLOR_SURFACE)),
            border: Border {
                color: theme::COLOR_LINE,
                width: theme::BORDER_HAIRLINE,
                radius: theme::RADIUS_CONTROL.into(),
            },
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::button;

    // Excludes `primary_button`: it is deliberately accent-filled, and its
    // border is set to the same accent colour as its own fill (see
    // `primary_button` above), so it draws no visible outline. Also
    // excludes `selected_row_button` and `selected_effect_button`: both
    // mark a selected state with accent colour by design (a soft fill for
    // the row, an accent border for the effect card) — each has its own
    // dedicated test below. Every other control here must never carry an
    // accent-coloured border at rest.
    fn styles() -> [fn(&Theme, button::Status) -> button::Style; 5] {
        [
            quiet_button,
            chip_button,
            row_button,
            danger_text_button,
            accent_text_button,
        ]
    }

    #[test]
    fn only_the_selected_row_paints_an_accent_background() {
        let theme = theme::theme();
        let selected = selected_row_button(&theme, button::Status::Active);
        assert_eq!(
            selected.background,
            Some(Background::Color(theme::COLOR_ACCENT_SOFT))
        );
        assert_eq!(
            selected.border.width,
            theme::BORDER_NONE,
            "no border in any state"
        );
        assert_eq!(selected.border.color, Color::TRANSPARENT);
        let plain = row_button(&theme, button::Status::Active);
        assert_eq!(
            plain.background,
            Some(Background::Color(Color::TRANSPARENT))
        );
    }

    #[test]
    fn the_selected_effect_card_draws_a_visible_accent_border() {
        let theme = theme::theme();
        let selected = selected_effect_button(&theme, button::Status::Active);
        assert_eq!(selected.border.color, theme::COLOR_ACCENT);
        assert_eq!(selected.border.width, theme::BORDER_SELECTED);
        assert_eq!(
            selected.background,
            Some(Background::Color(theme::COLOR_ACCENT_SOFT))
        );
    }

    #[test]
    fn no_resting_control_draws_an_accent_border() {
        let theme = theme::theme();
        for style in styles() {
            let resting = style(&theme, button::Status::Active);
            assert_ne!(
                resting.border.color,
                theme::COLOR_ACCENT,
                "a resting control must not be outlined in the accent colour"
            );
        }
    }

    #[test]
    fn quiet_and_chip_controls_lift_on_hover() {
        let theme = theme::theme();
        for style in [quiet_button, chip_button, row_button] {
            let resting = style(&theme, button::Status::Active);
            let hovered = style(&theme, button::Status::Hovered);
            assert_ne!(
                resting.background, hovered.background,
                "hover must be visible"
            );
        }
    }

    #[test]
    fn the_primary_button_is_the_only_accent_filled_control() {
        let theme = theme::theme();
        assert_eq!(
            primary_button(&theme, button::Status::Active).background,
            Some(Background::Color(theme::COLOR_ACCENT))
        );
        for style in [
            quiet_button,
            chip_button,
            row_button,
            danger_text_button,
            accent_text_button,
        ] {
            assert_ne!(
                style(&theme, button::Status::Active).background,
                Some(Background::Color(theme::COLOR_ACCENT))
            );
        }
    }

    #[test]
    fn destructive_text_is_danger_coloured_and_unfilled() {
        let theme = theme::theme();
        let style = danger_text_button(&theme, button::Status::Active);
        assert_eq!(style.text_color, theme::COLOR_DANGER);
        assert_eq!(
            style.background,
            Some(Background::Color(Color::TRANSPARENT))
        );
    }

    #[test]
    fn revert_text_is_accent_coloured_unfilled_and_borderless() {
        let theme = theme::theme();
        let style = accent_text_button(&theme, button::Status::Active);
        assert_eq!(style.text_color, theme::COLOR_ACCENT);
        assert_eq!(
            style.background,
            Some(Background::Color(Color::TRANSPARENT))
        );
        assert_eq!(style.border.width, theme::BORDER_NONE);
        assert_eq!(style.border.color, Color::TRANSPARENT);
    }

    #[test]
    fn cards_are_more_rounded_than_controls() {
        let theme = theme::theme();
        let card = card_container(&theme);
        let control = quiet_button(&theme, button::Status::Active);
        // `assert!(theme::RADIUS_CARD > theme::RADIUS_CONTROL)` compares two
        // `const` values directly, so clippy folds it to a compile-time
        // constant and warns (`assertions_on_constants`). Reading both
        // through a runtime slice first keeps this a real, failure-revealing
        // check, matching the pattern used in theme/tokens.rs's own tests.
        let radii = [theme::RADIUS_CARD, theme::RADIUS_CONTROL];
        assert!(radii[0] > radii[1]);
        assert_eq!(
            card.background,
            Some(Background::Color(theme::COLOR_SURFACE))
        );
        assert_eq!(control.border.width, theme::BORDER_HAIRLINE);
    }

    #[test]
    fn the_status_dot_is_green_when_running_and_faint_otherwise() {
        assert_eq!(hairline_color(true), theme::COLOR_OK);
        assert_eq!(hairline_color(false), theme::COLOR_TEXT_FAINT);
    }

    #[test]
    fn the_tab_strip_marks_exactly_one_tab_active() {
        let theme = theme::theme();
        let active = tab_button_style(true)(&theme, button::Status::Active);
        let idle = tab_button_style(false)(&theme, button::Status::Active);
        assert_eq!(
            active.background,
            Some(Background::Color(theme::COLOR_RAISED))
        );
        assert_eq!(idle.background, Some(Background::Color(Color::TRANSPARENT)));
        assert_eq!(active.text_color, theme::COLOR_TEXT);
        assert_eq!(idle.text_color, theme::COLOR_TEXT_DIM);
    }

    #[test]
    fn chips_are_small_caps_not_pills() {
        let theme = theme::theme();
        let chip = chip_button(&theme, button::Status::Active);
        let control = quiet_button(&theme, button::Status::Active);
        assert_eq!(
            chip.border.radius, control.border.radius,
            "a pill's long curve turns the hairline edge dotted"
        );
    }

    #[test]
    fn the_drop_down_and_its_menu_are_built_from_the_dark_material() {
        let theme = theme::theme();
        let rest = pick_list_style(&theme, pick_list::Status::Active);
        assert_eq!(rest.background, Background::Color(theme::COLOR_KEY));
        assert_eq!(rest.text_color, theme::COLOR_TEXT);
        let open = pick_list_style(&theme, pick_list::Status::Opened);
        assert_ne!(rest.background, open.background, "opening must be visible");
        let list = menu_style(&theme);
        assert_eq!(list.background, Background::Color(theme::COLOR_RAISED));
        assert_eq!(
            list.selected_background,
            Background::Color(theme::COLOR_ACCENT_SOFT)
        );
    }
}
