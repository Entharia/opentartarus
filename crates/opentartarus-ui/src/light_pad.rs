//! The pad as the lighting tab draws it: the Keys tab's caps, each lit in
//! the colour its LED shows. On the V2 Pro only the 27 face keys have LEDs,
//! so the stick and the Hypershift key are left out, and a click picks the
//! key's row.

use crate::app::Message;
use crate::keypad::{corner_for, key_rects};
use crate::theme::{self, with_opacity};
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Program, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme};
use opentartarus_core::labels::v2_pro_grid_position;
use opentartarus_core::types::{DeviceModel, KeyId, LightFrame};

#[derive(Debug, Clone)]
pub struct LightPad {
    pub model: Option<DeviceModel>,
    /// What each LED shows, before brightness. On the V2 Pro, cell
    /// `[row][column]` is the key at that place; other pads use `[0][0]`
    /// for every key.
    pub frame: LightFrame,
    pub brightness: u8,
    /// The row the colour controls edit, outlined. `None` hides it.
    pub selected_row: Option<usize>,
    /// Whether clicking a key picks its row.
    pub rows_pickable: bool,
}

/// An unlit LED still shows a little of its colour, so a dim pad never
/// reads as off.
const MIN_GLOW: f32 = 0.25;
const GLOW_WIDTH: f32 = 6.0;
const GLOW_ALPHA: f32 = 0.22;
const SELECTED_GLOW_WIDTH: f32 = 4.0;
const UNLIT: [u8; 3] = [0, 0, 0];
/// Room around the keys for the halo and the selection outline.
const EDGE_ROOM: f32 = GLOW_WIDTH;

/// Where each lit key is drawn. On the V2 Pro that is the face keys alone,
/// scaled up together to fill `size` and centred; other pads draw as on the
/// Keys tab.
fn lit_rects(size: Size, model: Option<DeviceModel>) -> Vec<(KeyId, Rectangle)> {
    let all = key_rects(size, model);
    if model != Some(DeviceModel::V2Pro) {
        return all;
    }
    let face: Vec<(KeyId, Rectangle)> = all
        .into_iter()
        .filter(|(id, _)| v2_pro_grid_position(*id).is_some())
        .collect();
    let Some(bounds) = face
        .iter()
        .map(|(_, rect)| *rect)
        .reduce(|a, b| a.union(&b))
    else {
        return face;
    };
    let room_w = (size.width - EDGE_ROOM * 2.0).max(0.0);
    let room_h = (size.height - EDGE_ROOM * 2.0 - theme::CAP_SHADOW_DROP).max(0.0);
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return face;
    }
    let scale = (room_w / bounds.width).min(room_h / bounds.height);
    let left = (size.width - bounds.width * scale) / 2.0;
    let top = (size.height - bounds.height * scale) / 2.0;
    face.into_iter()
        .map(|(id, rect)| {
            let placed = Rectangle::new(
                Point::new(
                    left + (rect.x - bounds.x) * scale,
                    top + (rect.y - bounds.y) * scale,
                ),
                Size::new(rect.width * scale, rect.height * scale),
            );
            (id, placed)
        })
        .collect()
}

impl LightPad {
    /// The LED colour under `id`, or `None` for a key with no LED.
    fn led(&self, id: KeyId) -> Option<[u8; 3]> {
        let rgb = if self.model == Some(DeviceModel::V2Pro) {
            let (row, column) = v2_pro_grid_position(id)?;
            self.frame.get(row)?.get(column).copied()?
        } else {
            self.frame.first()?.first().copied()?
        };
        (rgb != UNLIT).then_some(rgb)
    }

    /// `rgb` as the pad shows it at the current brightness, laid over an
    /// unlit cap.
    fn lit(&self, rgb: [u8; 3]) -> Color {
        let level = f32::from(self.brightness.min(theme::BRIGHTNESS_MAX))
            / f32::from(theme::BRIGHTNESS_MAX);
        let strength = MIN_GLOW + (1.0 - MIN_GLOW) * level;
        let base = theme::COLOR_KEY_UNBOUND;
        let led = Color::from_rgb8(rgb[0], rgb[1], rgb[2]);
        Color::from_rgb(
            base.r + (led.r - base.r) * strength,
            base.g + (led.g - base.g) * strength,
            base.b + (led.b - base.b) * strength,
        )
    }

    fn row_at(&self, local: Point, size: Size) -> Option<usize> {
        if !self.rows_pickable || self.model != Some(DeviceModel::V2Pro) {
            return None;
        }
        lit_rects(size, self.model)
            .into_iter()
            .find(|(_, rect)| rect.contains(local))
            .and_then(|(id, _)| v2_pro_grid_position(id))
            .map(|(row, _)| row)
    }
}

impl Program<Message> for LightPad {
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
        let bevel_color = Color {
            a: theme::CAP_SHADOW_ALPHA,
            ..Color::BLACK
        };
        let rects = lit_rects(bounds.size(), self.model);
        for (id, rect) in &rects {
            let radius = iced::border::Radius::from(corner_for(*id, self.model));
            let path = Path::rounded_rectangle(rect.position(), rect.size(), radius);
            let bevel = Path::rounded_rectangle(
                Point::new(rect.x, rect.y + theme::CAP_SHADOW_DROP),
                rect.size(),
                radius,
            );
            frame.fill(&bevel, bevel_color);
            match self.led(*id) {
                Some(rgb) => {
                    let color = self.lit(rgb);
                    // A soft halo, as the light spills around a lit cap.
                    frame.stroke(
                        &path,
                        Stroke::default()
                            .with_color(with_opacity(color, GLOW_ALPHA))
                            .with_width(GLOW_WIDTH),
                    );
                    frame.fill(&path, color);
                }
                None => frame.fill(&path, theme::COLOR_KEY_UNBOUND),
            }
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(theme::COLOR_CAP_EDGE)
                    .with_width(theme::BORDER_HAIRLINE),
            );
        }
        if let (Some(selected), Some(DeviceModel::V2Pro)) = (self.selected_row, self.model) {
            for (id, rect) in &rects {
                if v2_pro_grid_position(*id).map(|(row, _)| row) != Some(selected) {
                    continue;
                }
                let path = Path::rounded_rectangle(
                    rect.position(),
                    rect.size(),
                    iced::border::Radius::from(corner_for(*id, self.model)),
                );
                frame.stroke(
                    &path,
                    Stroke::default()
                        .with_color(theme::COLOR_ACCENT_SOFT)
                        .with_width(SELECTED_GLOW_WIDTH),
                );
                frame.stroke(
                    &path,
                    Stroke::default()
                        .with_color(theme::COLOR_TEXT)
                        .with_width(theme::BORDER_SELECTED),
                );
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
        if let canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(row) = cursor
                .position_in(bounds)
                .and_then(|local| self.row_at(local, bounds.size()))
            {
                return (
                    iced::event::Status::Captured,
                    Some(Message::LightingRow(row)),
                );
            }
        }
        (iced::event::Status::Ignored, None)
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match cursor
            .position_in(bounds)
            .and_then(|local| self.row_at(local, bounds.size()))
        {
            Some(_) => mouse::Interaction::Pointer,
            None => mouse::Interaction::default(),
        }
    }
}

pub fn widget(pad: LightPad, height: f32) -> iced::Element<'static, Message> {
    iced::widget::canvas(pad)
        .width(iced::Length::Fill)
        .height(iced::Length::Fixed(height))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentartarus_core::lighting::solid_frame;

    fn pad(model: Option<DeviceModel>, frame: LightFrame) -> LightPad {
        LightPad {
            model,
            frame,
            brightness: 100,
            selected_row: None,
            rows_pickable: true,
        }
    }

    #[test]
    fn v2_pro_keys_take_their_own_cell_and_the_stick_stays_unlit() {
        let mut frame = solid_frame([1, 1, 1]);
        frame[2][0] = [200, 0, 0];
        let pad = pad(Some(DeviceModel::V2Pro), frame);
        assert_eq!(pad.led(KeyId::Kp11), Some([200, 0, 0]));
        assert_eq!(pad.led(KeyId::Kp21), Some([1, 1, 1]));
        assert_eq!(pad.led(KeyId::StickClick), None);
        assert_eq!(pad.led(KeyId::Hypershift), None);
    }

    #[test]
    fn the_v2_pro_draws_only_its_face_keys_filling_the_canvas() {
        let size = Size::new(600.0, 300.0);
        let rects = lit_rects(size, Some(DeviceModel::V2Pro));
        assert_eq!(rects.len(), 27);
        assert!(rects
            .iter()
            .all(|(id, _)| !id.is_stick() && *id != KeyId::Hypershift));
        let union = rects
            .iter()
            .map(|(_, r)| *r)
            .reduce(|a, b| a.union(&b))
            .unwrap();
        assert!(union.x >= 0.0 && union.y >= 0.0);
        assert!(union.x + union.width <= size.width + 0.5);
        assert!(union.y + union.height <= size.height + 0.5);
        let fills_height =
            union.height >= size.height - 2.0 * EDGE_ROOM - 2.0 * theme::CAP_SHADOW_DROP;
        let fills_width = union.width >= size.width - 2.0 * EDGE_ROOM - 1.0;
        assert!(fills_height || fills_width, "the face grows to the space");
    }

    #[test]
    fn black_cells_draw_as_unlit_caps() {
        let pad = pad(Some(DeviceModel::V2Pro), solid_frame(UNLIT));
        assert_eq!(pad.led(KeyId::Kp01), None);
    }

    #[test]
    fn other_pads_light_every_key_in_one_colour() {
        let pad = pad(Some(DeviceModel::V2), solid_frame([9, 8, 7]));
        assert_eq!(pad.led(KeyId::Kp01), Some([9, 8, 7]));
        assert_eq!(pad.led(KeyId::Kp20), Some([9, 8, 7]));
    }

    #[test]
    fn full_brightness_shows_the_led_colour() {
        let pad = pad(Some(DeviceModel::V2Pro), Vec::new());
        assert_eq!(pad.lit([255, 0, 0]), Color::from_rgb8(255, 0, 0));
        let dim = LightPad {
            brightness: 0,
            ..pad
        };
        assert_ne!(dim.lit([255, 0, 0]), Color::from_rgb8(255, 0, 0));
    }

    #[test]
    fn a_click_on_a_key_finds_its_row_only_where_rows_are_pickable() {
        let size = Size::new(600.0, 300.0);
        let v2_pro = pad(Some(DeviceModel::V2Pro), Vec::new());
        let rects = lit_rects(size, v2_pro.model);
        let (_, kp16) = rects.iter().find(|(id, _)| *id == KeyId::Kp16).unwrap();
        let centre = Point::new(kp16.center_x(), kp16.center_y());
        assert_eq!(v2_pro.row_at(centre, size), Some(3));
        let locked = LightPad {
            rows_pickable: false,
            ..v2_pro
        };
        assert_eq!(locked.row_at(centre, size), None);
        let v2 = pad(Some(DeviceModel::V2), Vec::new());
        assert_eq!(v2.row_at(centre, size), None);
    }
}
