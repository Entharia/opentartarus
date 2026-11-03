use crate::app::Message;
use crate::theme::{self, with_opacity};
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Program, Stroke, Text};
use iced::{alignment, Point, Rectangle, Renderer, Size, Theme};
use opentartarus_core::labels::{bind_label, v2_pro_key_number, V2_PRO_ROWS};
use opentartarus_core::types::{Action, DeviceModel, KeyId};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Keypad {
    pub model: Option<DeviceModel>,
    pub bindings: BTreeMap<KeyId, Action>,
    pub selected: Option<KeyId>,
    pub faded: bool,
    pub hovered: Option<KeyId>,
}

const KEY_NUMBER_TEXT: f32 = 8.5;
const KEY_TEXT_SIZE: f32 = 12.5;
const NUMBER_INSET: f32 = 6.0;
const CORNER_KEY: f32 = 9.0;
const CORNER_THUMB: f32 = 12.0;
const CORNER_DPAD: f32 = 7.0;
const SELECTED_GLOW_WIDTH: f32 = 4.0;

/// The glyph a non-binding key shows when it has no binding of its own.
fn face_glyph(id: KeyId) -> Option<&'static str> {
    match id {
        KeyId::WheelUp => Some(theme::WHEEL_UP_GLYPH),
        KeyId::WheelClick => Some(theme::WHEEL_CLICK_GLYPH),
        KeyId::WheelDown => Some(theme::WHEEL_DOWN_GLYPH),
        KeyId::Mode => Some(theme::MODE_LABEL),
        KeyId::ThumbN | KeyId::AnalogUp => Some("\u{2191}"),
        KeyId::ThumbS | KeyId::AnalogDown => Some("\u{2193}"),
        KeyId::ThumbW | KeyId::AnalogLeft => Some("\u{2190}"),
        KeyId::ThumbE | KeyId::AnalogRight => Some("\u{2192}"),
        KeyId::ThumbNe => Some("\u{2197}"),
        KeyId::ThumbSe => Some("\u{2198}"),
        KeyId::ThumbSw => Some("\u{2199}"),
        KeyId::ThumbNw => Some("\u{2196}"),
        KeyId::StickClick => Some(theme::STICK_CLICK_GLYPH),
        _ => None,
    }
}

/// The two-digit number printed on the physical key: `kp01`-`kp20` on the
/// V2 and Pro, and the 27 face keys counted along the rows on the V2 Pro.
fn key_number(id: KeyId, model: Option<DeviceModel>) -> Option<String> {
    if model == Some(DeviceModel::V2Pro) {
        return v2_pro_key_number(id).map(|number| format!("{number:02}"));
    }
    GRID_ROW_KEYS
        .iter()
        .position(|candidate| *candidate == id)
        .map(|index| format!("{:02}", index + 1))
        .or_else(|| (id == KeyId::Kp20).then(|| String::from("20")))
}

fn corner_for(id: KeyId, model: Option<DeviceModel>) -> f32 {
    if model == Some(DeviceModel::V2Pro) {
        return if id.is_stick() {
            CORNER_DPAD
        } else {
            CORNER_KEY
        };
    }
    if id == KeyId::Kp20 {
        CORNER_THUMB
    } else if GRID_ROW_KEYS.contains(&id) {
        CORNER_KEY
    } else {
        CORNER_DPAD
    }
}

impl Program<Message> for Keypad {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let opacity = if self.faded {
            theme::DISCONNECTED_OPACITY
        } else {
            FULL_OPACITY
        };
        let bound_fill = with_opacity(theme::COLOR_KEY, opacity);
        let unbound_fill = with_opacity(theme::COLOR_KEY_UNBOUND, opacity);
        let hover_fill = with_opacity(theme::COLOR_KEY_HOVER, opacity);
        let text_color = with_opacity(theme::COLOR_TEXT, opacity);
        let faint_color = with_opacity(theme::COLOR_TEXT_FAINT, opacity);
        let edge_color = with_opacity(theme::COLOR_CAP_EDGE, opacity);
        let bevel_color = with_opacity(
            iced::Color {
                a: theme::CAP_SHADOW_ALPHA,
                ..iced::Color::BLACK
            },
            opacity,
        );
        let accent = with_opacity(theme::COLOR_ACCENT, opacity);
        let accent_soft = with_opacity(theme::COLOR_ACCENT_SOFT, opacity);

        // The empty middle of the thumb pad, drawn first so keys sit above it.
        if !matches!(self.model, Some(DeviceModel::Pro | DeviceModel::V2Pro)) {
            let centre = thumb_pad_center_rect(bounds.size());
            frame.fill_text(Text {
                content: String::from(theme::THUMB_CENTER_LABEL),
                position: Point::new(centre.center_x(), centre.center_y()),
                color: faint_color,
                size: iced::Pixels(KEY_NUMBER_TEXT),
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Center,
                ..Text::default()
            });
        }

        for (id, rect) in key_rects(bounds.size(), self.model) {
            let binding = self.bindings.get(&id);
            let path = Path::rounded_rectangle(
                rect.position(),
                rect.size(),
                iced::border::Radius::from(corner_for(id, self.model)),
            );
            let fill = if self.hovered == Some(id) {
                hover_fill
            } else if binding.is_some() {
                bound_fill
            } else {
                unbound_fill
            };
            // A bevel, not a blurred shadow: the canvas has no blur, so this
            // is an offset copy that peeks out below the cap and reads as its
            // lower edge. Drawn first, so each cap sits on its own bevel
            // rather than on its neighbour's.
            let bevel = Path::rounded_rectangle(
                Point::new(rect.x, rect.y + theme::CAP_SHADOW_DROP),
                rect.size(),
                iced::border::Radius::from(corner_for(id, self.model)),
            );
            frame.fill(&bevel, bevel_color);
            frame.fill(&path, fill);
            // The lit top edge. A full stroke would ring the cap; only the
            // upper arc catches the light, so the lower half is left dark.
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(edge_color)
                    .with_width(theme::BORDER_HAIRLINE),
            );
            if self.selected == Some(id) {
                frame.stroke(
                    &path,
                    Stroke::default()
                        .with_color(accent_soft)
                        .with_width(SELECTED_GLOW_WIDTH),
                );
                frame.stroke(
                    &path,
                    Stroke::default()
                        .with_color(accent)
                        .with_width(theme::BORDER_SELECTED),
                );
            }
            if let Some(number) = key_number(id, self.model) {
                frame.fill_text(Text {
                    content: number,
                    position: Point::new(rect.x + NUMBER_INSET, rect.y + NUMBER_INSET),
                    color: faint_color,
                    size: iced::Pixels(KEY_NUMBER_TEXT),
                    horizontal_alignment: alignment::Horizontal::Left,
                    vertical_alignment: alignment::Vertical::Top,
                    ..Text::default()
                });
            }
            let label = bind_label(binding);
            let (content, color) = if label.is_empty() {
                (face_glyph(id).unwrap_or_default().to_string(), faint_color)
            } else {
                (label, text_color)
            };
            if !content.is_empty() {
                frame.fill_text(Text {
                    content,
                    position: Point::new(rect.center_x(), rect.center_y()),
                    color,
                    size: iced::Pixels(KEY_TEXT_SIZE),
                    horizontal_alignment: alignment::Horizontal::Center,
                    vertical_alignment: alignment::Vertical::Center,
                    ..Text::default()
                });
            }
        }
        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (iced::event::Status, Option<Message>) {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(local) = cursor.position_in(bounds) {
                    if let Some(id) = hit_test(local, bounds.size(), self.model) {
                        return (iced::event::Status::Captured, Some(Message::SelectKey(id)));
                    }
                }
                (iced::event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let under = cursor
                    .position_in(bounds)
                    .and_then(|local| hit_test(local, bounds.size(), self.model));
                if under == self.hovered {
                    // Do not flood the runtime with no-change hover messages.
                    return (iced::event::Status::Ignored, None);
                }
                (iced::event::Status::Ignored, Some(Message::HoverKey(under)))
            }
            _ => (iced::event::Status::Ignored, None),
        }
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if let Some(local) = cursor.position_in(bounds) {
            if hit_test(local, bounds.size(), self.model).is_some() {
                return mouse::Interaction::Pointer;
            }
        }
        mouse::Interaction::default()
    }
}

fn hit_test(local: Point, size: Size, model: Option<DeviceModel>) -> Option<KeyId> {
    key_rects(size, model)
        .into_iter()
        .find(|(_, rect)| rect.contains(local))
        .map(|(id, _)| id)
}

const GRID_ROW_LENGTHS: [usize; 4] = [5, 5, 5, 4];
const GRID_ROW_KEYS: [KeyId; 19] = [
    KeyId::Kp01,
    KeyId::Kp02,
    KeyId::Kp03,
    KeyId::Kp04,
    KeyId::Kp05,
    KeyId::Kp06,
    KeyId::Kp07,
    KeyId::Kp08,
    KeyId::Kp09,
    KeyId::Kp10,
    KeyId::Kp11,
    KeyId::Kp12,
    KeyId::Kp13,
    KeyId::Kp14,
    KeyId::Kp15,
    KeyId::Kp16,
    KeyId::Kp17,
    KeyId::Kp18,
    KeyId::Kp19,
];
const WHEEL_COLUMN_KEYS: [KeyId; 4] = [
    KeyId::WheelUp,
    KeyId::WheelClick,
    KeyId::WheelDown,
    KeyId::Mode,
];

const KEY_WIDTH: f32 = 50.0;
const KEY_HEIGHT: f32 = 44.0;
const KEY_GAP: f32 = 8.0;
const ROW4_WIDE_KEY_SCALE: f32 = 1.5;
const WHEEL_WIDTH: f32 = 38.0;
const WHEEL_HEIGHT: f32 = 28.0;
const THUMB_KEY_WIDTH: f32 = 74.0;
const THUMB_KEY_HEIGHT: f32 = 52.0;
const THUMB_CLUSTER_GAP: f32 = 22.0;
const DPAD_COLUMNS: usize = 3;
const DPAD_ROWS: usize = 3;
const DPAD_CELL_WIDTH: f32 = 30.0;
const DPAD_CELL_HEIGHT: f32 = 26.0;
const DPAD_GAP: f32 = 5.0;
const SECTION_GAP: f32 = 16.0;

const WIDEST_ROW: f32 = 5.0;
const GRID_ROW_COUNT: f32 = 4.0;
const GRID_WIDTH: f32 = KEY_WIDTH * WIDEST_ROW + KEY_GAP * (WIDEST_ROW - 1.0);
const GRID_HEIGHT: f32 = KEY_HEIGHT * GRID_ROW_COUNT + KEY_GAP * (GRID_ROW_COUNT - 1.0);
const TOP_WIDTH: f32 = GRID_WIDTH + KEY_GAP + WHEEL_WIDTH;
const DPAD_WIDTH: f32 =
    DPAD_CELL_WIDTH * DPAD_COLUMNS as f32 + DPAD_GAP * (DPAD_COLUMNS as f32 - 1.0);
const DPAD_HEIGHT: f32 = DPAD_CELL_HEIGHT * DPAD_ROWS as f32 + DPAD_GAP * (DPAD_ROWS as f32 - 1.0);
const THUMB_ROW_WIDTH: f32 = THUMB_KEY_WIDTH + THUMB_CLUSTER_GAP + DPAD_WIDTH;
/// `DPAD_HEIGHT` (88) exceeds `THUMB_KEY_HEIGHT` (52).
/// `derived_constants_pick_the_larger_term` guards this.
const THUMB_ROW_HEIGHT: f32 = DPAD_HEIGHT;
/// `TOP_WIDTH` (328) exceeds `THUMB_ROW_WIDTH` (196). Same guard.
const NATURAL_WIDTH: f32 = TOP_WIDTH;
const NATURAL_HEIGHT: f32 = GRID_HEIGHT + SECTION_GAP + THUMB_ROW_HEIGHT;

const FULL_OPACITY: f32 = 1.0;
const HALF: f32 = 2.0;
/// How far the pad may grow past its natural size. The cluster is 328x304 at
/// scale 1.0, which leaves most of the card empty in the default 1120x760
/// window, so it scales up to fill the space. The cap stops the keys turning
/// comically large on a maximised window.
const MAX_SCALE: f32 = 2.0;

/// The eight thumb-pad directions, by 3x3 cell. The centre cell is absent on
/// purpose: it is drawn as a label but is not clickable.
const THUMB_PAD_CELLS: [(KeyId, usize, usize); 8] = [
    (KeyId::ThumbNw, 0, 0),
    (KeyId::ThumbN, 1, 0),
    (KeyId::ThumbNe, 2, 0),
    (KeyId::ThumbW, 0, 1),
    (KeyId::ThumbE, 2, 1),
    (KeyId::ThumbSw, 0, 2),
    (KeyId::ThumbS, 1, 2),
    (KeyId::ThumbSe, 2, 2),
];

/// The Pro's analog stick uses the four edge cells of the same 3x3 grid.
const ANALOG_PAD_CELLS: [(KeyId, usize, usize); 4] = [
    (KeyId::AnalogUp, 1, 0),
    (KeyId::AnalogLeft, 0, 1),
    (KeyId::AnalogRight, 2, 1),
    (KeyId::AnalogDown, 1, 2),
];

/// Uniform scale and offset that centre the natural-size cluster in `size`.
/// How far a drawn key reaches past its own rect: half the selection glow
/// (a stroke straddles its path) plus the bevel that hangs below. The cluster
/// is fitted inside a canvas shrunk by this on every side, so an edge key's
/// glow and bevel land inside the canvas instead of being clipped by it.
const DRAW_OVERHANG: f32 = SELECTED_GLOW_WIDTH / HALF + theme::CAP_SHADOW_DROP;

fn fit(size: Size) -> (f32, f32, f32) {
    fit_natural(size, NATURAL_WIDTH, NATURAL_HEIGHT)
}

fn fit_natural(size: Size, natural_width: f32, natural_height: f32) -> (f32, f32, f32) {
    let usable_width = (size.width - DRAW_OVERHANG * HALF).max(0.0);
    let usable_height = (size.height - DRAW_OVERHANG * HALF).max(0.0);
    let scale = (usable_width / natural_width)
        .min(usable_height / natural_height)
        .min(MAX_SCALE);
    let offset_x = DRAW_OVERHANG + (usable_width - natural_width * scale) / HALF;
    let offset_y = DRAW_OVERHANG + (usable_height - natural_height * scale) / HALF;
    (scale, offset_x, offset_y)
}

/// The V2 Pro: five rows of 6, 6, 6, 5 and 4 keys, and under them the
/// thumbstick as a 3x3 grid with its four directions on the edges and its
/// click in the middle. It has no wheel column.
/// Widest row in key units: a double key plus five singles, or two doubles
/// plus three singles.
const V2_PRO_WIDEST_ROW: f32 = 7.0;
const V2_PRO_ROW_COUNT: f32 = 5.0;
/// A double key spans two key widths and the gap between them, so it lines up
/// with the singles in the rows around it.
const V2_PRO_DOUBLE_WIDTH: f32 = KEY_WIDTH * 2.0 + KEY_GAP;
const V2_PRO_GRID_WIDTH: f32 = KEY_WIDTH * V2_PRO_WIDEST_ROW + KEY_GAP * (V2_PRO_WIDEST_ROW - 1.0);
const V2_PRO_GRID_HEIGHT: f32 = KEY_HEIGHT * V2_PRO_ROW_COUNT + KEY_GAP * (V2_PRO_ROW_COUNT - 1.0);
const V2_PRO_NATURAL_WIDTH: f32 = V2_PRO_GRID_WIDTH;
const V2_PRO_NATURAL_HEIGHT: f32 = V2_PRO_GRID_HEIGHT + SECTION_GAP + DPAD_HEIGHT;
const V2_PRO_STICK_CELLS: [(KeyId, usize, usize); 5] = [
    (KeyId::AnalogUp, 1, 0),
    (KeyId::AnalogLeft, 0, 1),
    (KeyId::StickClick, 1, 1),
    (KeyId::AnalogRight, 2, 1),
    (KeyId::AnalogDown, 1, 2),
];

/// The first key of every row is double width, and so is the last key of
/// rows 4 and 5 (keys 23 and 27). The rest are single.
fn v2_pro_key_width(row: usize, slot: usize, length: usize) -> f32 {
    let last = slot + 1 == length;
    if slot == 0 || (row >= 3 && last) {
        V2_PRO_DOUBLE_WIDTH
    } else {
        KEY_WIDTH
    }
}

fn v2_pro_rects(size: Size) -> Vec<(KeyId, Rectangle)> {
    let (scale, offset_x, offset_y) =
        fit_natural(size, V2_PRO_NATURAL_WIDTH, V2_PRO_NATURAL_HEIGHT);
    let place = |x: f32, y: f32, w: f32, h: f32| {
        Rectangle::new(
            Point::new(offset_x + x * scale, offset_y + y * scale),
            Size::new(w * scale, h * scale),
        )
    };
    let mut out = Vec::new();
    let mut y = 0.0f32;
    for (row, keys) in V2_PRO_ROWS.iter().enumerate() {
        let widths: Vec<f32> = (0..keys.len())
            .map(|slot| v2_pro_key_width(row, slot, keys.len()))
            .collect();
        // Rows start flush left, as on the pad: the short bottom row lines
        // up with the keys above it instead of sitting centred.
        let mut x = 0.0f32;
        for (id, width) in keys.iter().zip(widths) {
            out.push((*id, place(x, y, width, KEY_HEIGHT)));
            x += width + KEY_GAP;
        }
        y += KEY_HEIGHT + KEY_GAP;
    }
    let stick_top = V2_PRO_GRID_HEIGHT + SECTION_GAP;
    let stick_left = (V2_PRO_NATURAL_WIDTH - DPAD_WIDTH) / HALF;
    for (id, column, row) in V2_PRO_STICK_CELLS {
        let cell_x = stick_left + column as f32 * (DPAD_CELL_WIDTH + DPAD_GAP);
        let cell_y = stick_top + row as f32 * (DPAD_CELL_HEIGHT + DPAD_GAP);
        out.push((id, place(cell_x, cell_y, DPAD_CELL_WIDTH, DPAD_CELL_HEIGHT)));
    }
    out
}

fn key_rects(size: Size, model: Option<DeviceModel>) -> Vec<(KeyId, Rectangle)> {
    if model == Some(DeviceModel::V2Pro) {
        return v2_pro_rects(size);
    }
    let (scale, offset_x, offset_y) = fit(size);
    let place = |x: f32, y: f32, w: f32, h: f32| {
        Rectangle::new(
            Point::new(offset_x + x * scale, offset_y + y * scale),
            Size::new(w * scale, h * scale),
        )
    };
    let mut out = Vec::with_capacity(KeyId::ALL.len());

    // Rows 1-4. Every row is centred on the widest row.
    let mut index = 0usize;
    let mut y = 0.0f32;
    for (row, length) in GRID_ROW_LENGTHS.iter().enumerate() {
        let is_last_row = row + 1 == GRID_ROW_LENGTHS.len();
        let first_width = if is_last_row {
            KEY_WIDTH * ROW4_WIDE_KEY_SCALE
        } else {
            KEY_WIDTH
        };
        let row_width =
            first_width + KEY_WIDTH * (*length as f32 - 1.0) + KEY_GAP * (*length as f32 - 1.0);
        let mut x = (GRID_WIDTH - row_width) / HALF;
        for slot in 0..*length {
            let width = if slot == 0 { first_width } else { KEY_WIDTH };
            out.push((GRID_ROW_KEYS[index], place(x, y, width, KEY_HEIGHT)));
            x += width + KEY_GAP;
            index += 1;
        }
        y += KEY_HEIGHT + KEY_GAP;
    }

    // Wheel column, right of the grid, top aligned.
    let wheel_x = GRID_WIDTH + KEY_GAP;
    for (slot, id) in WHEEL_COLUMN_KEYS.iter().enumerate() {
        let wheel_y = slot as f32 * (WHEEL_HEIGHT + KEY_GAP);
        out.push((*id, place(wheel_x, wheel_y, WHEEL_WIDTH, WHEEL_HEIGHT)));
    }

    // Thumb row, centred under the top block.
    let row_top = GRID_HEIGHT + SECTION_GAP;
    let row_left = (NATURAL_WIDTH - THUMB_ROW_WIDTH) / HALF;
    let thumb_key_y = row_top + (THUMB_ROW_HEIGHT - THUMB_KEY_HEIGHT) / HALF;
    out.push((
        KeyId::Kp20,
        place(row_left, thumb_key_y, THUMB_KEY_WIDTH, THUMB_KEY_HEIGHT),
    ));

    let pad_left = row_left + THUMB_KEY_WIDTH + THUMB_CLUSTER_GAP;
    let cells: &[(KeyId, usize, usize)] = match model {
        Some(DeviceModel::Pro) => &ANALOG_PAD_CELLS,
        _ => &THUMB_PAD_CELLS,
    };
    for (id, column, row) in cells {
        let cell_x = pad_left + *column as f32 * (DPAD_CELL_WIDTH + DPAD_GAP);
        let cell_y = row_top + *row as f32 * (DPAD_CELL_HEIGHT + DPAD_GAP);
        out.push((
            *id,
            place(cell_x, cell_y, DPAD_CELL_WIDTH, DPAD_CELL_HEIGHT),
        ));
    }
    out
}

/// The empty middle of the thumb pad. Drawn, never clickable, so it is not in
/// `key_rects`.
fn thumb_pad_center_rect(size: Size) -> Rectangle {
    let (scale, offset_x, offset_y) = fit(size);
    let row_top = GRID_HEIGHT + SECTION_GAP;
    let row_left = (NATURAL_WIDTH - THUMB_ROW_WIDTH) / HALF;
    let pad_left = row_left + THUMB_KEY_WIDTH + THUMB_CLUSTER_GAP;
    let x = pad_left + DPAD_CELL_WIDTH + DPAD_GAP;
    let y = row_top + DPAD_CELL_HEIGHT + DPAD_GAP;
    Rectangle::new(
        Point::new(offset_x + x * scale, offset_y + y * scale),
        Size::new(DPAD_CELL_WIDTH * scale, DPAD_CELL_HEIGHT * scale),
    )
}

pub fn widget(keypad: Keypad) -> iced::Element<'static, Message> {
    iced::widget::canvas(keypad)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentartarus_core::types::{DeviceModel, KeyId};

    const BIG: Size = Size::new(900.0, 700.0);
    const TIGHT: Size = Size::new(220.0, 180.0);

    fn ids(size: Size, model: Option<DeviceModel>) -> Vec<KeyId> {
        key_rects(size, model)
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    }

    #[test]
    fn derived_constants_pick_the_larger_term() {
        // Read through runtime bindings so clippy cannot fold these into a
        // constant-valued assertion (`assertions_on_constants`).
        let top_width = TOP_WIDTH;
        let thumb_row_width = THUMB_ROW_WIDTH;
        assert!(
            top_width >= thumb_row_width,
            "NATURAL_WIDTH must be TOP_WIDTH"
        );
        let dpad_height = DPAD_HEIGHT;
        let thumb_key_height = THUMB_KEY_HEIGHT;
        assert!(
            dpad_height >= thumb_key_height,
            "THUMB_ROW_HEIGHT must be DPAD_HEIGHT"
        );
        assert_eq!(NATURAL_WIDTH, TOP_WIDTH);
        assert_eq!(THUMB_ROW_HEIGHT, DPAD_HEIGHT);
        assert_eq!(GRID_ROW_LENGTHS.iter().sum::<usize>(), GRID_ROW_KEYS.len());
    }

    #[test]
    fn v2_returns_every_key_once_and_no_analog() {
        let got = ids(BIG, Some(DeviceModel::V2));
        assert_eq!(got.len(), 32, "20 grid + 4 wheel column + 8 thumb");
        let mut sorted = got.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), got.len(), "no key may appear twice");
        assert!(got.contains(&KeyId::ThumbNw));
        assert!(got.contains(&KeyId::ThumbSe));
        assert!(!got.iter().any(|id| matches!(
            id,
            KeyId::AnalogUp | KeyId::AnalogDown | KeyId::AnalogLeft | KeyId::AnalogRight
        )));
    }

    #[test]
    fn pro_swaps_the_thumb_pad_for_the_analog_stick() {
        let got = ids(BIG, Some(DeviceModel::Pro));
        assert_eq!(got.len(), 28, "20 grid + 4 wheel column + 4 analog");
        for id in [
            KeyId::AnalogUp,
            KeyId::AnalogDown,
            KeyId::AnalogLeft,
            KeyId::AnalogRight,
        ] {
            assert!(got.contains(&id), "missing {id:?}");
        }
        assert!(!got.iter().any(|id| matches!(
            id,
            KeyId::ThumbN
                | KeyId::ThumbNe
                | KeyId::ThumbE
                | KeyId::ThumbSe
                | KeyId::ThumbS
                | KeyId::ThumbSw
                | KeyId::ThumbW
                | KeyId::ThumbNw
        )));
    }

    #[test]
    fn v2_pro_draws_27_keys_and_the_stick_without_a_wheel() {
        let got = ids(BIG, Some(DeviceModel::V2Pro));
        assert_eq!(got.len(), 32, "27 face keys + 4 stick directions + click");
        let mut sorted = got.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), got.len(), "no key may appear twice");
        for id in KeyId::ALL {
            assert_eq!(
                got.contains(&id),
                DeviceModel::V2Pro.has_key(id),
                "drawn keys and has_key disagree on {id:?}"
            );
        }
    }

    #[test]
    fn v2_pro_rows_stack_and_nothing_overlaps_or_clips() {
        let model = Some(DeviceModel::V2Pro);
        for size in [BIG, TIGHT, Size::new(550.0, 550.0)] {
            let rects = key_rects(size, model);
            for (i, (a_id, a)) in rects.iter().enumerate() {
                for (b_id, b) in rects.iter().skip(i + 1) {
                    let apart = a.x + a.width <= b.x + f32::EPSILON
                        || b.x + b.width <= a.x + f32::EPSILON
                        || a.y + a.height <= b.y + f32::EPSILON
                        || b.y + b.height <= a.y + f32::EPSILON;
                    assert!(apart, "{a_id:?} overlaps {b_id:?} at {size:?}");
                }
                let centre = Point::new(a.center_x(), a.center_y());
                assert_eq!(hit_test(centre, size, model), Some(*a_id));
                assert!(a.x >= 0.0 && a.y >= 0.0, "{a_id:?} left the bounds");
                assert!(
                    a.x + a.width <= size.width + 1.0 && a.y + a.height <= size.height + 1.0,
                    "{a_id:?} overflows at {size:?}"
                );
            }
            let y_of = |id: KeyId| rects.iter().find(|(k, _)| *k == id).unwrap().1.y;
            for pair in V2_PRO_ROWS.windows(2) {
                assert!(y_of(pair[0][0]) < y_of(pair[1][0]));
            }
            let lowest_key = rects
                .iter()
                .filter(|(id, _)| !id.is_stick())
                .map(|(_, r)| r.y + r.height)
                .fold(f32::MIN, f32::max);
            assert!(
                y_of(KeyId::AnalogUp) >= lowest_key,
                "stick sits below the keys"
            );
        }
    }

    #[test]
    fn v2_pro_double_keys_are_the_row_starts_and_keys_23_and_27() {
        let rects = key_rects(BIG, Some(DeviceModel::V2Pro));
        let rect_of = |id: KeyId| rects.iter().find(|(k, _)| *k == id).unwrap().1;
        let single = rect_of(KeyId::Kp02).width;
        let double_ids: Vec<KeyId> = V2_PRO_ROWS
            .iter()
            .map(|row| row[0])
            .chain([KeyId::Kp24, KeyId::Kp20])
            .collect();
        for (id, rect) in &rects {
            if id.is_stick() {
                continue;
            }
            let ratio = rect.width / single;
            if double_ids.contains(id) {
                assert!(ratio > 2.0 && ratio < 2.3, "{id:?} must be double: {ratio}");
            } else {
                assert!((ratio - 1.0).abs() < 0.01, "{id:?} must be single: {ratio}");
            }
        }
        // The double keys line up with the singles: row 2's third key starts
        // where row 1's third key does.
        assert!((rect_of(KeyId::Kp01).x - rect_of(KeyId::Kp07).x).abs() < 0.5);
        // The short bottom row starts flush left, like the rows above it.
        assert!((rect_of(KeyId::Kp25).x - rect_of(KeyId::Kp16).x).abs() < 0.5);
        assert!((rect_of(KeyId::Kp26).x - rect_of(KeyId::Kp17).x).abs() < 0.5);
    }

    #[test]
    fn v2_pro_numbers_follow_its_rows() {
        let model = Some(DeviceModel::V2Pro);
        assert_eq!(key_number(KeyId::Kp21, model).as_deref(), Some("01"));
        assert_eq!(key_number(KeyId::Kp20, model).as_deref(), Some("27"));
        assert_eq!(key_number(KeyId::AnalogUp, model), None);
        assert_eq!(key_number(KeyId::Kp21, Some(DeviceModel::V2)), None);
    }

    #[test]
    fn unknown_model_draws_the_v2_pad() {
        assert_eq!(ids(BIG, None), ids(BIG, Some(DeviceModel::V2)));
    }

    #[test]
    fn no_two_rects_overlap_at_either_size() {
        for size in [BIG, TIGHT] {
            let rects = key_rects(size, Some(DeviceModel::V2));
            for (i, (a_id, a)) in rects.iter().enumerate() {
                for (b_id, b) in rects.iter().skip(i + 1) {
                    let apart = a.x + a.width <= b.x + f32::EPSILON
                        || b.x + b.width <= a.x + f32::EPSILON
                        || a.y + a.height <= b.y + f32::EPSILON
                        || b.y + b.height <= a.y + f32::EPSILON;
                    assert!(apart, "{a_id:?} overlaps {b_id:?} at {size:?}");
                }
            }
        }
    }

    #[test]
    fn clicking_a_rect_centre_finds_that_key_at_either_size() {
        for size in [BIG, TIGHT] {
            for (id, rect) in key_rects(size, Some(DeviceModel::V2)) {
                let centre = Point::new(rect.center_x(), rect.center_y());
                assert_eq!(hit_test(centre, size, Some(DeviceModel::V2)), Some(id));
            }
        }
    }

    #[test]
    fn the_thumb_pad_centre_cell_is_not_clickable() {
        let rects = key_rects(BIG, Some(DeviceModel::V2));
        let west = rects.iter().find(|(id, _)| *id == KeyId::ThumbW).unwrap().1;
        let east = rects.iter().find(|(id, _)| *id == KeyId::ThumbE).unwrap().1;
        let north = rects.iter().find(|(id, _)| *id == KeyId::ThumbN).unwrap().1;
        let south = rects.iter().find(|(id, _)| *id == KeyId::ThumbS).unwrap().1;
        let middle = Point::new(
            (west.x + west.width + east.x) / 2.0,
            (north.y + north.height + south.y) / 2.0,
        );
        assert_eq!(hit_test(middle, BIG, Some(DeviceModel::V2)), None);
    }

    #[test]
    fn rows_are_five_five_five_four_and_stack_downwards() {
        let rects = key_rects(BIG, Some(DeviceModel::V2));
        let y_of = |id: KeyId| rects.iter().find(|(k, _)| *k == id).unwrap().1.y;
        let row1 = [
            KeyId::Kp01,
            KeyId::Kp02,
            KeyId::Kp03,
            KeyId::Kp04,
            KeyId::Kp05,
        ];
        let row2 = [
            KeyId::Kp06,
            KeyId::Kp07,
            KeyId::Kp08,
            KeyId::Kp09,
            KeyId::Kp10,
        ];
        let row3 = [
            KeyId::Kp11,
            KeyId::Kp12,
            KeyId::Kp13,
            KeyId::Kp14,
            KeyId::Kp15,
        ];
        let row4 = [KeyId::Kp16, KeyId::Kp17, KeyId::Kp18, KeyId::Kp19];
        for row in [
            row1.as_slice(),
            row2.as_slice(),
            row3.as_slice(),
            row4.as_slice(),
        ] {
            let first = y_of(row[0]);
            for id in row {
                assert_eq!(y_of(*id), first, "{id:?} left its row");
            }
        }
        assert!(y_of(KeyId::Kp01) < y_of(KeyId::Kp06));
        assert!(y_of(KeyId::Kp06) < y_of(KeyId::Kp11));
        assert!(y_of(KeyId::Kp11) < y_of(KeyId::Kp16));
    }

    #[test]
    fn key_sixteen_is_the_wide_key_and_row_four_is_centred() {
        let rects = key_rects(BIG, Some(DeviceModel::V2));
        let rect_of = |id: KeyId| rects.iter().find(|(k, _)| *k == id).unwrap().1;
        let wide = rect_of(KeyId::Kp16);
        let normal = rect_of(KeyId::Kp17);
        assert!(
            (wide.width - normal.width * ROW4_WIDE_KEY_SCALE).abs() < 0.5,
            "kp16 must be {ROW4_WIDE_KEY_SCALE}x wide"
        );
        let row1_left = rect_of(KeyId::Kp01).x;
        let row1_right = rect_of(KeyId::Kp05).x + rect_of(KeyId::Kp05).width;
        let row4_left = wide.x;
        let row4_right = rect_of(KeyId::Kp19).x + rect_of(KeyId::Kp19).width;
        let left_gap = row4_left - row1_left;
        let right_gap = row1_right - row4_right;
        assert!((left_gap - right_gap).abs() < 0.5, "row 4 must be centred");
    }

    #[test]
    fn the_thumb_key_sits_below_every_grid_row() {
        let rects = key_rects(BIG, Some(DeviceModel::V2));
        let thumb = rects.iter().find(|(id, _)| *id == KeyId::Kp20).unwrap().1;
        let lowest_grid = rects
            .iter()
            .filter(|(id, _)| GRID_ROW_KEYS.contains(id))
            .map(|(_, r)| r.y + r.height)
            .fold(f32::MIN, f32::max);
        assert!(thumb.y >= lowest_grid, "kp20 belongs in the thumb row");
    }

    #[test]
    fn the_cluster_is_centred_inside_its_bounds() {
        let rects = key_rects(BIG, Some(DeviceModel::V2));
        let left = rects.iter().map(|(_, r)| r.x).fold(f32::MAX, f32::min);
        let right = rects
            .iter()
            .map(|(_, r)| r.x + r.width)
            .fold(f32::MIN, f32::max);
        assert!(
            ((left) - (BIG.width - right)).abs() < 1.0,
            "left margin {left} must match right margin {}",
            BIG.width - right
        );
    }

    #[test]
    fn a_tight_bounds_scales_down_instead_of_clipping() {
        let rects = key_rects(TIGHT, Some(DeviceModel::V2));
        for (id, rect) in &rects {
            assert!(rect.x >= -f32::EPSILON, "{id:?} left the bounds");
            assert!(rect.y >= -f32::EPSILON, "{id:?} left the bounds");
            assert!(rect.x + rect.width <= TIGHT.width + 1.0, "{id:?} overflows");
            assert!(
                rect.y + rect.height <= TIGHT.height + 1.0,
                "{id:?} overflows"
            );
        }
        let big_key = key_rects(BIG, Some(DeviceModel::V2))[0].1;
        let tight_key = rects[0].1;
        assert!(
            tight_key.width < big_key.width,
            "tight bounds must shrink keys"
        );
    }

    #[test]
    fn nothing_drawn_around_an_edge_key_is_clipped_by_the_canvas() {
        // A key's rect is not all of it: the selection glow straddles the
        // rect's edge and the bevel hangs below. Every one of those has to
        // stay inside the canvas at every size, or an edge key loses part of
        // itself. This is the case the user saw: a snug fit put the leftmost
        // column's outer edge exactly on the canvas boundary.
        for size in [TIGHT, BIG, Size::new(550.0, 550.0), Size::new(328.0, 304.0)] {
            for (id, rect) in key_rects(size, Some(DeviceModel::V2)) {
                let glow = SELECTED_GLOW_WIDTH / HALF;
                assert!(
                    rect.x - glow >= -f32::EPSILON,
                    "{id:?} glow clips the left edge at {size:?}"
                );
                assert!(
                    rect.y - glow >= -f32::EPSILON,
                    "{id:?} glow clips the top edge at {size:?}"
                );
                assert!(
                    rect.x + rect.width + glow <= size.width + f32::EPSILON,
                    "{id:?} glow clips the right edge at {size:?}"
                );
                assert!(
                    rect.y + rect.height + glow + theme::CAP_SHADOW_DROP
                        <= size.height + f32::EPSILON,
                    "{id:?} bevel clips the bottom edge at {size:?}"
                );
            }
        }
    }

    #[test]
    fn a_roomy_bounds_grows_the_pad_instead_of_leaving_it_small() {
        // The centre card in the default 1120x760 window is about this size.
        let card = Size::new(624.0, 600.0);
        let (scale, _, _) = fit(card);
        assert!(
            scale > 1.0,
            "the pad must fill a roomy card, not sit at natural size: {scale}"
        );
        let natural_key = KEY_WIDTH;
        let grown = key_rects(card, Some(DeviceModel::V2))[0].1;
        assert!(
            grown.width > natural_key,
            "keys must grow with the card: {} vs {natural_key}",
            grown.width
        );
    }

    #[test]
    fn growth_stops_at_the_cap_and_stays_centred() {
        let huge = Size::new(4000.0, 3000.0);
        let (scale, offset_x, offset_y) = fit(huge);
        assert_eq!(scale, MAX_SCALE, "growth must stop at the cap");
        assert!(
            offset_x > 0.0 && offset_y > 0.0,
            "capped pad must stay centred"
        );
        let rects = key_rects(huge, Some(DeviceModel::V2));
        let left = rects.iter().map(|(_, r)| r.x).fold(f32::MAX, f32::min);
        let right = rects
            .iter()
            .map(|(_, r)| r.x + r.width)
            .fold(f32::MIN, f32::max);
        assert!(
            (left - (huge.width - right)).abs() < 1.0,
            "left margin {left} must match right margin {}",
            huge.width - right
        );
    }

    #[test]
    fn scaling_keeps_the_pad_proportions() {
        // Uniform scale means the width-to-height ratio never changes.
        let ratio_of = |size: Size| {
            let rects = key_rects(size, Some(DeviceModel::V2));
            let key = rects[0].1;
            key.width / key.height
        };
        let natural = KEY_WIDTH / KEY_HEIGHT;
        for size in [
            TIGHT,
            BIG,
            Size::new(624.0, 600.0),
            Size::new(4000.0, 300.0),
        ] {
            assert!(
                (ratio_of(size) - natural).abs() < 0.01,
                "keys must stay key-shaped at {size:?}"
            );
        }
    }
}
