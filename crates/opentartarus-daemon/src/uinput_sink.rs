use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, InputEvent, InputId, KeyCode,
    RelativeAxisCode, UinputAbsSetup,
};
use opentartarus_core::error::ErrorCode;
use opentartarus_core::keymap::{
    modifier_to_evdev, mouse_button_to_evdev, token_to_evdev, BTN_EXTRA, BTN_LEFT, BTN_MIDDLE,
    BTN_RIGHT, BTN_SIDE, BTN_THUMBL, BTN_THUMBR, EV_ABS, EV_REL, REL_WHEEL,
};
use opentartarus_core::remap::{Emitted, EventSink};
use opentartarus_core::types::{KeyToken, Modifier, MouseButton};

pub const KEYBOARD_NAME: &str = "OpenTartarus Keyboard";
pub const MOUSE_NAME: &str = "OpenTartarus Mouse";
pub const CONTROLLER_NAME: &str = "OpenTartarus Controller";
/// The virtual controller uses the usual Xbox 360 identity and layout, the
/// one Linux game input (SDL, Steam, Proton) maps without a config. It must
/// not use the pad's own Razer id: the daemon would then grab it as a pad.
const CONTROLLER_ID: (u16, u16, u16) = (0x045e, 0x028e, 0x0110);
const STICK_MIN: i32 = -32768;
const STICK_MAX: i32 = 32767;
const STICK_FUZZ: i32 = 16;
const STICK_FLAT: i32 = 128;
const TRIGGER_MAX: i32 = 255;
const REL_HWHEEL: u16 = 6;
const KEY_CODE_MIN: u16 = 1;
const KEY_CODE_MAX: u16 = 255;

pub struct UinputSink {
    keyboard: VirtualDevice,
    mouse: VirtualDevice,
    /// Made the first time a profile hands the stick to games, so a player
    /// who never does that never sees an extra controller in their games.
    controller: Option<VirtualDevice>,
}

impl UinputSink {
    pub fn open() -> Result<Self, ErrorCode> {
        Ok(Self {
            keyboard: build_keyboard()?,
            mouse: build_mouse()?,
            controller: None,
        })
    }

    fn controller(&mut self) -> Option<&mut VirtualDevice> {
        if self.controller.is_none() {
            match build_controller() {
                Ok(device) => self.controller = Some(device),
                Err(_) => {
                    crate::log::write(&ErrorCode::Permission.log_line(Some("virtual controller")));
                    return None;
                }
            }
        }
        self.controller.as_mut()
    }
}

impl EventSink for UinputSink {
    fn emit(&mut self, e: Emitted) {
        let event = InputEvent::new(e.ev_type, e.code, e.value);
        let device = if routes_to_controller(&e) {
            match self.controller() {
                Some(device) => device,
                None => return,
            }
        } else if routes_to_keyboard(&e) {
            &mut self.keyboard
        } else {
            &mut self.mouse
        };
        if let Err(err) = device.emit(&[event]) {
            crate::log::write(&ErrorCode::Io.log_line(Some(&err.to_string())));
        }
    }
}

pub fn token_from_evdev(code: u16) -> Option<KeyToken> {
    ALL_KEY_TOKENS
        .iter()
        .copied()
        .find(|token| token_to_evdev(*token) == code)
}

/// Stick axes and stick buttons only come from analog stick mode.
fn routes_to_controller(e: &Emitted) -> bool {
    e.ev_type == EV_ABS || e.code == BTN_THUMBL || e.code == BTN_THUMBR
}

fn routes_to_keyboard(e: &Emitted) -> bool {
    if e.ev_type == EV_REL || is_mouse_button(e.code) {
        return false;
    }
    e.keyboard
}

fn is_mouse_button(code: u16) -> bool {
    matches!(
        code,
        c if c == BTN_LEFT || c == BTN_RIGHT || c == BTN_MIDDLE || c == BTN_SIDE || c == BTN_EXTRA
    )
}

fn build_keyboard() -> Result<VirtualDevice, ErrorCode> {
    let mut keys = AttributeSet::<KeyCode>::new();
    for code in KEY_CODE_MIN..=KEY_CODE_MAX {
        keys.insert(KeyCode::new(code));
    }
    for token in ALL_KEY_TOKENS {
        keys.insert(KeyCode::new(token_to_evdev(*token)));
    }
    for modifier in ALL_MODIFIERS {
        keys.insert(KeyCode::new(modifier_to_evdev(*modifier)));
    }
    VirtualDevice::builder()
        .map_err(|_| ErrorCode::Permission)?
        .name(KEYBOARD_NAME)
        .with_keys(&keys)
        .map_err(|_| ErrorCode::Permission)?
        .build()
        .map_err(|_| ErrorCode::Permission)
}

fn build_mouse() -> Result<VirtualDevice, ErrorCode> {
    let mut buttons = AttributeSet::<KeyCode>::new();
    for button in ALL_MOUSE_BUTTONS {
        buttons.insert(KeyCode::new(mouse_button_to_evdev(*button)));
    }
    let mut axes = AttributeSet::<RelativeAxisCode>::new();
    axes.insert(RelativeAxisCode::REL_WHEEL);
    axes.insert(RelativeAxisCode::REL_HWHEEL);
    debug_assert_eq!(RelativeAxisCode::REL_WHEEL.0, REL_WHEEL);
    debug_assert_eq!(RelativeAxisCode::REL_HWHEEL.0, REL_HWHEEL);
    VirtualDevice::builder()
        .map_err(|_| ErrorCode::Permission)?
        .name(MOUSE_NAME)
        .with_keys(&buttons)
        .map_err(|_| ErrorCode::Permission)?
        .with_relative_axes(&axes)
        .map_err(|_| ErrorCode::Permission)?
        .build()
        .map_err(|_| ErrorCode::Permission)
}

fn build_controller() -> Result<VirtualDevice, ErrorCode> {
    let mut buttons = AttributeSet::<KeyCode>::new();
    for button in [
        KeyCode::BTN_SOUTH,
        KeyCode::BTN_EAST,
        KeyCode::BTN_NORTH,
        KeyCode::BTN_WEST,
        KeyCode::BTN_TL,
        KeyCode::BTN_TR,
        KeyCode::BTN_SELECT,
        KeyCode::BTN_START,
        KeyCode::BTN_MODE,
        KeyCode::BTN_THUMBL,
        KeyCode::BTN_THUMBR,
    ] {
        buttons.insert(button);
    }
    debug_assert_eq!(KeyCode::BTN_THUMBL.0, BTN_THUMBL);
    debug_assert_eq!(KeyCode::BTN_THUMBR.0, BTN_THUMBR);
    let stick = AbsInfo::new(0, STICK_MIN, STICK_MAX, STICK_FUZZ, STICK_FLAT, 0);
    let trigger = AbsInfo::new(0, 0, TRIGGER_MAX, 0, 0, 0);
    let hat = AbsInfo::new(0, -1, 1, 0, 0, 0);
    let (vendor, product, version) = CONTROLLER_ID;
    let mut builder = VirtualDevice::builder()
        .map_err(|_| ErrorCode::Permission)?
        .name(CONTROLLER_NAME)
        .input_id(InputId::new(BusType::BUS_USB, vendor, product, version))
        .with_keys(&buttons)
        .map_err(|_| ErrorCode::Permission)?;
    for (axis, info) in [
        (AbsoluteAxisCode::ABS_X, stick),
        (AbsoluteAxisCode::ABS_Y, stick),
        (AbsoluteAxisCode::ABS_RX, stick),
        (AbsoluteAxisCode::ABS_RY, stick),
        (AbsoluteAxisCode::ABS_Z, trigger),
        (AbsoluteAxisCode::ABS_RZ, trigger),
        (AbsoluteAxisCode::ABS_HAT0X, hat),
        (AbsoluteAxisCode::ABS_HAT0Y, hat),
    ] {
        builder = builder
            .with_absolute_axis(&UinputAbsSetup::new(axis, info))
            .map_err(|_| ErrorCode::Permission)?;
    }
    builder.build().map_err(|_| ErrorCode::Permission)
}

const ALL_KEY_TOKENS: &[KeyToken] = &[
    KeyToken::A,
    KeyToken::B,
    KeyToken::C,
    KeyToken::D,
    KeyToken::E,
    KeyToken::F,
    KeyToken::G,
    KeyToken::H,
    KeyToken::I,
    KeyToken::J,
    KeyToken::K,
    KeyToken::L,
    KeyToken::M,
    KeyToken::N,
    KeyToken::O,
    KeyToken::P,
    KeyToken::Q,
    KeyToken::R,
    KeyToken::S,
    KeyToken::T,
    KeyToken::U,
    KeyToken::V,
    KeyToken::W,
    KeyToken::X,
    KeyToken::Y,
    KeyToken::Z,
    KeyToken::Num0,
    KeyToken::Num1,
    KeyToken::Num2,
    KeyToken::Num3,
    KeyToken::Num4,
    KeyToken::Num5,
    KeyToken::Num6,
    KeyToken::Num7,
    KeyToken::Num8,
    KeyToken::Num9,
    KeyToken::F1,
    KeyToken::F2,
    KeyToken::F3,
    KeyToken::F4,
    KeyToken::F5,
    KeyToken::F6,
    KeyToken::F7,
    KeyToken::F8,
    KeyToken::F9,
    KeyToken::F10,
    KeyToken::F11,
    KeyToken::F12,
    KeyToken::Escape,
    KeyToken::Tab,
    KeyToken::Backspace,
    KeyToken::Enter,
    KeyToken::Space,
    KeyToken::Insert,
    KeyToken::Delete,
    KeyToken::Home,
    KeyToken::End,
    KeyToken::PageUp,
    KeyToken::PageDown,
    KeyToken::Up,
    KeyToken::Down,
    KeyToken::Left,
    KeyToken::Right,
    KeyToken::Minus,
    KeyToken::Equal,
    KeyToken::LeftBrace,
    KeyToken::RightBracket,
    KeyToken::Backslash,
    KeyToken::Semicolon,
    KeyToken::Apostrophe,
    KeyToken::Grave,
    KeyToken::Comma,
    KeyToken::Dot,
    KeyToken::Slash,
    KeyToken::NumPad0,
    KeyToken::NumPad1,
    KeyToken::NumPad2,
    KeyToken::NumPad3,
    KeyToken::NumPad4,
    KeyToken::NumPad5,
    KeyToken::NumPad6,
    KeyToken::NumPad7,
    KeyToken::NumPad8,
    KeyToken::NumPad9,
    KeyToken::NumEnter,
    KeyToken::NumPlus,
    KeyToken::NumMinus,
    KeyToken::NumSlash,
    KeyToken::NumStar,
    KeyToken::NumDot,
    KeyToken::PrintScreen,
    KeyToken::ScrollLock,
    KeyToken::Pause,
    KeyToken::CapsLock,
    KeyToken::NumLock,
    KeyToken::VolumeUp,
    KeyToken::VolumeDown,
    KeyToken::Mute,
    KeyToken::LeftShift,
    KeyToken::LeftCtrl,
    KeyToken::LeftAlt,
];

const ALL_MODIFIERS: &[Modifier] = &[
    Modifier::Ctrl,
    Modifier::Shift,
    Modifier::Alt,
    Modifier::Super,
    Modifier::CtrlR,
    Modifier::ShiftR,
    Modifier::AltR,
    Modifier::SuperR,
];

const ALL_MOUSE_BUTTONS: &[MouseButton] = &[
    MouseButton::Left,
    MouseButton::Right,
    MouseButton::Middle,
    MouseButton::Back,
    MouseButton::Forward,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_device_names_match_spec() {
        assert_eq!(KEYBOARD_NAME, "OpenTartarus Keyboard");
        assert_eq!(MOUSE_NAME, "OpenTartarus Mouse");
        assert_eq!(EV_REL, 2);
    }
}
