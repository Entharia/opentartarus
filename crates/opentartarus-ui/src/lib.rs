pub mod app;
pub mod client;
pub mod keypad;
pub mod keys;
pub mod light_pad;
pub mod theme;
pub mod view;

use crate::app::App;
use crate::theme::{WINDOW_HEIGHT, WINDOW_WIDTH};
use iced::Size;

const WINDOW_ICON_PNG: &[u8] = include_bytes!("../../../packaging/icons/opentartarus-64.png");

/// The window icon. `None` when the PNG cannot be decoded; the window then
/// opens without one, which is what happens today.
fn window_icon() -> Option<iced::window::Icon> {
    let decoder = png::Decoder::new(WINDOW_ICON_PNG);
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    buffer.truncate(info.buffer_size());
    iced::window::icon::from_rgba(buffer, info.width, info.height).ok()
}

pub fn run() -> iced::Result {
    iced::application(theme::APP_TITLE, App::update, App::view)
        .subscription(App::subscription)
        .theme(App::theme)
        // The keypad is drawn on a canvas. Without this its rounded caps come
        // out stair-stepped: iced only multisamples canvas geometry when asked.
        .antialiasing(true)
        .style(|_state, _theme| iced::application::Appearance {
            // The window paints nothing itself. `view` draws a rounded panel
            // instead, and the corners outside it stay clear, which is what
            // gives a frameless window rounded corners at all.
            background_color: iced::Color::TRANSPARENT,
            text_color: theme::COLOR_TEXT,
        })
        .window(iced::window::Settings {
            size: Size::new(WINDOW_WIDTH, WINDOW_HEIGHT),
            icon: window_icon(),
            // No system title bar: the app's own header is the title bar, so
            // the window reads as one surface instead of our chrome bolted
            // under the desktop's. The header supplies drag, minimize,
            // maximize and close.
            decorations: false,
            // Rounded corners need clear corner pixels. Only where the
            // session supports it: see app::window_is_rounded.
            transparent: crate::app::window_is_rounded(),
            ..iced::window::Settings::default()
        })
        .exit_on_close_request(false)
        .run_with(App::boot)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICON_SIDE_PX: u32 = 64;
    const RGBA_BYTES_PER_PIXEL: u32 = 4;

    #[test]
    fn window_icon_decodes_from_a_64x64_rgba_png() {
        let icon = window_icon().expect("the embedded PNG must decode to a window icon");
        let (rgba, size) = icon.into_raw();
        assert_eq!(size.width, ICON_SIDE_PX, "icon must be 64px wide");
        assert_eq!(size.height, ICON_SIDE_PX, "icon must be 64px tall");
        let expected_bytes = ICON_SIDE_PX * ICON_SIDE_PX * RGBA_BYTES_PER_PIXEL;
        assert_eq!(
            rgba.len(),
            expected_bytes as usize,
            "the buffer must hold 4 bytes (RGBA) per pixel"
        );
    }
}
