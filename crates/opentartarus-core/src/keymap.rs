use crate::types::{DeviceModel, KeyId, KeyToken, Modifier, MouseButton};
use std::collections::BTreeMap;

pub const EV_KEY: u16 = 1;
pub const EV_REL: u16 = 2;
pub const EV_ABS: u16 = 3;

pub const REL_WHEEL: u16 = 8;

pub const ABS_X: u16 = 0;
pub const ABS_Y: u16 = 1;
/// The right stick's axes. Synapse can set the V2 Pro's stick to send these.
pub const ABS_RX: u16 = 3;
pub const ABS_RY: u16 = 4;
pub const ABS_HAT0X: u16 = 16;
pub const ABS_HAT0Y: u16 = 17;

/// The Tartarus Pro's analog stick reports 0..=255 around 128.
const ANALOG_AXIS_CENTER: i32 = 128;
const ANALOG_AXIS_MAX: i32 = 255;
/// The Tartarus V2 Pro's thumbstick is a gamepad (xpad) stick: -32768..=32767
/// around 0, with up and left negative.
const GAMEPAD_AXIS_CENTER: i32 = 0;
pub const GAMEPAD_AXIS_MAX: i32 = 32767;

const KEY_ESC: u16 = 1;
const KEY_1: u16 = 2;
const KEY_2: u16 = 3;
const KEY_3: u16 = 4;
const KEY_4: u16 = 5;
const KEY_5: u16 = 6;
const KEY_6: u16 = 7;
const KEY_7: u16 = 8;
const KEY_8: u16 = 9;
const KEY_9: u16 = 10;
const KEY_0: u16 = 11;
const KEY_MINUS: u16 = 12;
const KEY_EQUAL: u16 = 13;
const KEY_BACKSPACE: u16 = 14;
const KEY_TAB: u16 = 15;
const KEY_Q: u16 = 16;
const KEY_W: u16 = 17;
const KEY_E: u16 = 18;
const KEY_R: u16 = 19;
const KEY_T: u16 = 20;
const KEY_Y: u16 = 21;
const KEY_U: u16 = 22;
const KEY_I: u16 = 23;
const KEY_O: u16 = 24;
const KEY_P: u16 = 25;
const KEY_LEFTBRACE: u16 = 26;
const KEY_RIGHTBRACE: u16 = 27;
const KEY_ENTER: u16 = 28;
const KEY_LEFTCTRL: u16 = 29;
const KEY_A: u16 = 30;
const KEY_S: u16 = 31;
const KEY_D: u16 = 32;
const KEY_F: u16 = 33;
const KEY_G: u16 = 34;
const KEY_H: u16 = 35;
const KEY_J: u16 = 36;
const KEY_K: u16 = 37;
const KEY_L: u16 = 38;
const KEY_SEMICOLON: u16 = 39;
const KEY_APOSTROPHE: u16 = 40;
const KEY_GRAVE: u16 = 41;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_BACKSLASH: u16 = 43;
const KEY_Z: u16 = 44;
const KEY_X: u16 = 45;
const KEY_C: u16 = 46;
const KEY_V: u16 = 47;
const KEY_B: u16 = 48;
const KEY_N: u16 = 49;
const KEY_M: u16 = 50;
const KEY_COMMA: u16 = 51;
const KEY_DOT: u16 = 52;
const KEY_SLASH: u16 = 53;
const KEY_RIGHTSHIFT: u16 = 54;
const KEY_KPASTERISK: u16 = 55;
const KEY_LEFTALT: u16 = 56;
const KEY_SPACE: u16 = 57;
const KEY_CAPSLOCK: u16 = 58;
const KEY_F1: u16 = 59;
const KEY_F2: u16 = 60;
const KEY_F3: u16 = 61;
const KEY_F4: u16 = 62;
const KEY_F5: u16 = 63;
const KEY_F6: u16 = 64;
const KEY_F7: u16 = 65;
const KEY_F8: u16 = 66;
const KEY_F9: u16 = 67;
const KEY_F10: u16 = 68;
const KEY_NUMLOCK: u16 = 69;
const KEY_SCROLLLOCK: u16 = 70;
const KEY_KP7: u16 = 71;
const KEY_KP8: u16 = 72;
const KEY_KP9: u16 = 73;
const KEY_KPMINUS: u16 = 74;
const KEY_KP4: u16 = 75;
const KEY_KP5: u16 = 76;
const KEY_KP6: u16 = 77;
const KEY_KPPLUS: u16 = 78;
const KEY_KP1: u16 = 79;
const KEY_KP2: u16 = 80;
const KEY_KP3: u16 = 81;
const KEY_KP0: u16 = 82;
const KEY_KPDOT: u16 = 83;
const KEY_F11: u16 = 87;
const KEY_F12: u16 = 88;
const KEY_KPENTER: u16 = 96;
const KEY_RIGHTCTRL: u16 = 97;
const KEY_KPSLASH: u16 = 98;
const KEY_SYSRQ: u16 = 99;
const KEY_RIGHTALT: u16 = 100;
const KEY_HOME: u16 = 102;
const KEY_UP: u16 = 103;
const KEY_PAGEUP: u16 = 104;
const KEY_LEFT: u16 = 105;
const KEY_RIGHT: u16 = 106;
const KEY_END: u16 = 107;
const KEY_DOWN: u16 = 108;
const KEY_PAGEDOWN: u16 = 109;
const KEY_INSERT: u16 = 110;
const KEY_DELETE: u16 = 111;
const KEY_MUTE: u16 = 113;
const KEY_VOLUMEDOWN: u16 = 114;
const KEY_VOLUMEUP: u16 = 115;
const KEY_PAUSE: u16 = 119;
const KEY_LEFTMETA: u16 = 125;
const KEY_RIGHTMETA: u16 = 126;
const KEY_F24: u16 = 194;
/// What the daemon reports the V2 Pro's Hypershift button as. The pad sends
/// it only on its Razer HID interface, never as an input event.
pub const KEY_FN: u16 = 464;
/// The gamepad's left-stick click. The V2 Pro's thumbstick click sends it.
pub const BTN_THUMBL: u16 = 317;
pub const BTN_THUMBR: u16 = 318;

pub const BTN_LEFT: u16 = 272;
pub const BTN_RIGHT: u16 = 273;
pub const BTN_MIDDLE: u16 = 274;
pub const BTN_SIDE: u16 = 275;
pub const BTN_EXTRA: u16 = 276;

const WHEEL_UP: i32 = 1;
const WHEEL_DOWN: i32 = -1;
pub const KEY_PRESS_QUALIFIER: i32 = 1;

/// Keypad plus wheel/mode fixture shared by production maps and tests.
/// Qualifier is applied in [`KeyMap`]: press `1` for keys, `+1`/`-1` for wheel.
const V2_KEYS: &[(KeyId, u16, u16)] = &[
    (KeyId::Kp01, EV_KEY, KEY_1),
    (KeyId::Kp02, EV_KEY, KEY_2),
    (KeyId::Kp03, EV_KEY, KEY_3),
    (KeyId::Kp04, EV_KEY, KEY_4),
    (KeyId::Kp05, EV_KEY, KEY_5),
    (KeyId::Kp06, EV_KEY, KEY_6),
    (KeyId::Kp07, EV_KEY, KEY_7),
    (KeyId::Kp08, EV_KEY, KEY_8),
    (KeyId::Kp09, EV_KEY, KEY_9),
    (KeyId::Kp10, EV_KEY, KEY_0),
    (KeyId::Kp11, EV_KEY, KEY_Q),
    (KeyId::Kp12, EV_KEY, KEY_W),
    (KeyId::Kp13, EV_KEY, KEY_E),
    (KeyId::Kp14, EV_KEY, KEY_R),
    (KeyId::Kp15, EV_KEY, KEY_T),
    (KeyId::Kp16, EV_KEY, KEY_Y),
    (KeyId::Kp17, EV_KEY, KEY_U),
    (KeyId::Kp18, EV_KEY, KEY_I),
    (KeyId::Kp19, EV_KEY, KEY_O),
    (KeyId::Kp20, EV_KEY, KEY_P),
    (KeyId::WheelUp, EV_REL, REL_WHEEL),
    (KeyId::WheelDown, EV_REL, REL_WHEEL),
    (KeyId::WheelClick, EV_KEY, BTN_MIDDLE),
    (KeyId::Mode, EV_KEY, KEY_F24),
];

/// What the Tartarus V2 Pro sends on its own, with no software running:
/// the left of a keyboard. Each key is given the `KeyId` of the V2 key that
/// sends the same thing, so every profile built for the V2 lands on the same
/// keys. The seven keys the V2 does not have get `kp21`-`kp27`.
const V2_PRO_KEYS: &[(KeyId, u16, u16)] = &[
    (KeyId::Kp01, EV_KEY, KEY_1),
    (KeyId::Kp02, EV_KEY, KEY_2),
    (KeyId::Kp03, EV_KEY, KEY_3),
    (KeyId::Kp04, EV_KEY, KEY_4),
    (KeyId::Kp05, EV_KEY, KEY_5),
    (KeyId::Kp06, EV_KEY, KEY_TAB),
    (KeyId::Kp07, EV_KEY, KEY_Q),
    (KeyId::Kp08, EV_KEY, KEY_W),
    (KeyId::Kp09, EV_KEY, KEY_E),
    (KeyId::Kp10, EV_KEY, KEY_R),
    (KeyId::Kp11, EV_KEY, KEY_CAPSLOCK),
    (KeyId::Kp12, EV_KEY, KEY_A),
    (KeyId::Kp13, EV_KEY, KEY_S),
    (KeyId::Kp14, EV_KEY, KEY_D),
    (KeyId::Kp15, EV_KEY, KEY_F),
    (KeyId::Kp16, EV_KEY, KEY_LEFTSHIFT),
    (KeyId::Kp17, EV_KEY, KEY_Z),
    (KeyId::Kp18, EV_KEY, KEY_X),
    (KeyId::Kp19, EV_KEY, KEY_C),
    (KeyId::Kp20, EV_KEY, KEY_SPACE),
    (KeyId::Kp21, EV_KEY, KEY_GRAVE),
    (KeyId::Kp22, EV_KEY, KEY_T),
    (KeyId::Kp23, EV_KEY, KEY_G),
    (KeyId::Kp24, EV_KEY, KEY_V),
    (KeyId::Kp25, EV_KEY, KEY_LEFTCTRL),
    (KeyId::Kp26, EV_KEY, KEY_B),
    (KeyId::Kp27, EV_KEY, KEY_I),
    (KeyId::StickClick, EV_KEY, BTN_THUMBL),
    // The click of the stick when Synapse set it to act as the right stick.
    (KeyId::StickClick, EV_KEY, BTN_THUMBR),
    (KeyId::Hypershift, EV_KEY, KEY_FN),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvdevTuple {
    pub ev_type: u16,
    pub code: u16,
    pub qualifier: i32,
}

#[derive(Debug, Clone)]
pub struct KeyMap {
    inner: BTreeMap<(u16, u16, i32), KeyId>,
}

impl KeyMap {
    pub fn v2() -> Self {
        Self::from_table(V2_KEYS)
    }

    pub fn pro() -> Self {
        Self::from_table(V2_KEYS)
    }

    pub fn v2_pro() -> Self {
        Self::from_table(V2_PRO_KEYS)
    }

    pub fn for_model(model: DeviceModel) -> Self {
        match model {
            DeviceModel::V2 => Self::v2(),
            DeviceModel::Pro => Self::pro(),
            DeviceModel::V2Pro => Self::v2_pro(),
        }
    }

    fn from_table(table: &[(KeyId, u16, u16)]) -> Self {
        let mut inner = BTreeMap::new();
        for &(id, ev_type, code) in table {
            let qualifier = match id {
                KeyId::WheelUp => WHEEL_UP,
                KeyId::WheelDown => WHEEL_DOWN,
                _ => KEY_PRESS_QUALIFIER,
            };
            inner.insert((ev_type, code, qualifier), id);
        }
        Self { inner }
    }

    pub fn get(&self, ev_type: u16, code: u16, qualifier: i32) -> Option<KeyId> {
        self.resolve(EvdevTuple {
            ev_type,
            code,
            qualifier,
        })
    }

    pub fn resolve(&self, tuple: EvdevTuple) -> Option<KeyId> {
        self.inner
            .get(&(tuple.ev_type, tuple.code, tuple.qualifier))
            .copied()
    }

    pub fn contains(&self, id: KeyId) -> bool {
        self.inner.values().any(|&mapped| mapped == id)
    }
}

pub fn token_to_evdev(token: KeyToken) -> u16 {
    match token {
        KeyToken::A => KEY_A,
        KeyToken::B => KEY_B,
        KeyToken::C => KEY_C,
        KeyToken::D => KEY_D,
        KeyToken::E => KEY_E,
        KeyToken::F => KEY_F,
        KeyToken::G => KEY_G,
        KeyToken::H => KEY_H,
        KeyToken::I => KEY_I,
        KeyToken::J => KEY_J,
        KeyToken::K => KEY_K,
        KeyToken::L => KEY_L,
        KeyToken::M => KEY_M,
        KeyToken::N => KEY_N,
        KeyToken::O => KEY_O,
        KeyToken::P => KEY_P,
        KeyToken::Q => KEY_Q,
        KeyToken::R => KEY_R,
        KeyToken::S => KEY_S,
        KeyToken::T => KEY_T,
        KeyToken::U => KEY_U,
        KeyToken::V => KEY_V,
        KeyToken::W => KEY_W,
        KeyToken::X => KEY_X,
        KeyToken::Y => KEY_Y,
        KeyToken::Z => KEY_Z,
        KeyToken::Num0 => KEY_0,
        KeyToken::Num1 => KEY_1,
        KeyToken::Num2 => KEY_2,
        KeyToken::Num3 => KEY_3,
        KeyToken::Num4 => KEY_4,
        KeyToken::Num5 => KEY_5,
        KeyToken::Num6 => KEY_6,
        KeyToken::Num7 => KEY_7,
        KeyToken::Num8 => KEY_8,
        KeyToken::Num9 => KEY_9,
        KeyToken::F1 => KEY_F1,
        KeyToken::F2 => KEY_F2,
        KeyToken::F3 => KEY_F3,
        KeyToken::F4 => KEY_F4,
        KeyToken::F5 => KEY_F5,
        KeyToken::F6 => KEY_F6,
        KeyToken::F7 => KEY_F7,
        KeyToken::F8 => KEY_F8,
        KeyToken::F9 => KEY_F9,
        KeyToken::F10 => KEY_F10,
        KeyToken::F11 => KEY_F11,
        KeyToken::F12 => KEY_F12,
        KeyToken::Escape => KEY_ESC,
        KeyToken::Tab => KEY_TAB,
        KeyToken::Backspace => KEY_BACKSPACE,
        KeyToken::Enter => KEY_ENTER,
        KeyToken::Space => KEY_SPACE,
        KeyToken::Insert => KEY_INSERT,
        KeyToken::Delete => KEY_DELETE,
        KeyToken::Home => KEY_HOME,
        KeyToken::End => KEY_END,
        KeyToken::PageUp => KEY_PAGEUP,
        KeyToken::PageDown => KEY_PAGEDOWN,
        KeyToken::Up => KEY_UP,
        KeyToken::Down => KEY_DOWN,
        KeyToken::Left => KEY_LEFT,
        KeyToken::Right => KEY_RIGHT,
        KeyToken::Minus => KEY_MINUS,
        KeyToken::Equal => KEY_EQUAL,
        KeyToken::LeftBrace => KEY_LEFTBRACE,
        KeyToken::RightBracket => KEY_RIGHTBRACE,
        KeyToken::Backslash => KEY_BACKSLASH,
        KeyToken::Semicolon => KEY_SEMICOLON,
        KeyToken::Apostrophe => KEY_APOSTROPHE,
        KeyToken::Grave => KEY_GRAVE,
        KeyToken::Comma => KEY_COMMA,
        KeyToken::Dot => KEY_DOT,
        KeyToken::Slash => KEY_SLASH,
        KeyToken::NumPad0 => KEY_KP0,
        KeyToken::NumPad1 => KEY_KP1,
        KeyToken::NumPad2 => KEY_KP2,
        KeyToken::NumPad3 => KEY_KP3,
        KeyToken::NumPad4 => KEY_KP4,
        KeyToken::NumPad5 => KEY_KP5,
        KeyToken::NumPad6 => KEY_KP6,
        KeyToken::NumPad7 => KEY_KP7,
        KeyToken::NumPad8 => KEY_KP8,
        KeyToken::NumPad9 => KEY_KP9,
        KeyToken::NumEnter => KEY_KPENTER,
        KeyToken::NumPlus => KEY_KPPLUS,
        KeyToken::NumMinus => KEY_KPMINUS,
        KeyToken::NumSlash => KEY_KPSLASH,
        KeyToken::NumStar => KEY_KPASTERISK,
        KeyToken::NumDot => KEY_KPDOT,
        KeyToken::PrintScreen => KEY_SYSRQ,
        KeyToken::ScrollLock => KEY_SCROLLLOCK,
        KeyToken::Pause => KEY_PAUSE,
        KeyToken::CapsLock => KEY_CAPSLOCK,
        KeyToken::NumLock => KEY_NUMLOCK,
        KeyToken::VolumeUp => KEY_VOLUMEUP,
        KeyToken::VolumeDown => KEY_VOLUMEDOWN,
        KeyToken::Mute => KEY_MUTE,
        KeyToken::LeftShift => KEY_LEFTSHIFT,
        KeyToken::LeftCtrl => KEY_LEFTCTRL,
        KeyToken::LeftAlt => KEY_LEFTALT,
    }
}

pub fn modifier_to_evdev(modifier: Modifier) -> u16 {
    match modifier {
        Modifier::Ctrl => KEY_LEFTCTRL,
        Modifier::Shift => KEY_LEFTSHIFT,
        Modifier::Alt => KEY_LEFTALT,
        Modifier::Super => KEY_LEFTMETA,
        Modifier::CtrlR => KEY_RIGHTCTRL,
        Modifier::ShiftR => KEY_RIGHTSHIFT,
        Modifier::AltR => KEY_RIGHTALT,
        Modifier::SuperR => KEY_RIGHTMETA,
    }
}

pub fn mouse_button_to_evdev(button: MouseButton) -> u16 {
    match button {
        MouseButton::Left => BTN_LEFT,
        MouseButton::Right => BTN_RIGHT,
        MouseButton::Middle => BTN_MIDDLE,
        MouseButton::Back => BTN_SIDE,
        MouseButton::Forward => BTN_EXTRA,
    }
}

pub fn decode_hat(hat_x: i32, hat_y: i32) -> Option<KeyId> {
    match (hat_x.signum(), hat_y.signum()) {
        (0, -1) => Some(KeyId::ThumbN),
        (1, -1) => Some(KeyId::ThumbNe),
        (1, 0) => Some(KeyId::ThumbE),
        (1, 1) => Some(KeyId::ThumbSe),
        (0, 1) => Some(KeyId::ThumbS),
        (-1, 1) => Some(KeyId::ThumbSw),
        (-1, 0) => Some(KeyId::ThumbW),
        (-1, -1) => Some(KeyId::ThumbNw),
        _ => None,
    }
}

pub fn analog_axis_ratio(value: i32) -> f32 {
    let travel = (ANALOG_AXIS_MAX - ANALOG_AXIS_CENTER) as f32;
    (value - ANALOG_AXIS_CENTER) as f32 / travel
}

/// The resting value of a model's stick axes.
pub fn analog_axis_center(model: DeviceModel) -> i32 {
    match model {
        DeviceModel::V2Pro => GAMEPAD_AXIS_CENTER,
        DeviceModel::V2 | DeviceModel::Pro => ANALOG_AXIS_CENTER,
    }
}

/// How far a model's stick is pushed, from -1.0 to 1.0. Negative is up or left.
pub fn analog_axis_ratio_for(model: DeviceModel, value: i32) -> f32 {
    match model {
        DeviceModel::V2Pro => {
            let travel = (GAMEPAD_AXIS_MAX - GAMEPAD_AXIS_CENTER) as f32;
            ((value - GAMEPAD_AXIS_CENTER) as f32 / travel).clamp(-1.0, 1.0)
        }
        DeviceModel::V2 | DeviceModel::Pro => analog_axis_ratio(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{KeyId, KeyToken, Modifier};

    #[test]
    fn locked_linux_key_codes() {
        assert_eq!(token_to_evdev(KeyToken::Q), 16);
        assert_eq!(token_to_evdev(KeyToken::C), 46);
        assert_eq!(token_to_evdev(KeyToken::E), 18);
        assert_eq!(token_to_evdev(KeyToken::Num1), 2);
        assert_eq!(token_to_evdev(KeyToken::Num2), 3);
        assert_eq!(modifier_to_evdev(Modifier::Ctrl), 29);
    }

    #[test]
    fn v2_fixture_kp01_kp02_and_model_coverage() {
        let v2 = KeyMap::v2();
        assert_eq!(
            v2.resolve(EvdevTuple {
                ev_type: 1,
                code: 2,
                qualifier: 1
            }),
            Some(KeyId::Kp01)
        );
        assert_eq!(v2.get(1, 2, 1), Some(KeyId::Kp01));
        assert_eq!(v2.get(1, 3, 1), Some(KeyId::Kp02));
        for id in keypad_and_wheel_mode() {
            assert!(v2.contains(id), "V2 map missing {id:?}");
        }
        let pro = KeyMap::pro();
        for id in keypad_and_wheel_mode() {
            assert!(pro.contains(id), "Pro map missing {id:?}");
        }
        assert!(!v2.contains(KeyId::ThumbN));
        assert!(!v2.contains(KeyId::AnalogUp));
        assert!(!pro.contains(KeyId::ThumbN));
        assert!(!pro.contains(KeyId::AnalogUp));
    }

    #[test]
    fn v2_pro_fixture_matches_what_the_pad_sends() {
        let map = KeyMap::v2_pro();
        // Recorded from a real V2 Pro (1532:02eb), row by row.
        let rows: [&[(u16, KeyId)]; 5] = [
            &[
                (KEY_GRAVE, KeyId::Kp21),
                (KEY_1, KeyId::Kp01),
                (KEY_2, KeyId::Kp02),
                (KEY_3, KeyId::Kp03),
                (KEY_4, KeyId::Kp04),
                (KEY_5, KeyId::Kp05),
            ],
            &[
                (KEY_TAB, KeyId::Kp06),
                (KEY_Q, KeyId::Kp07),
                (KEY_W, KeyId::Kp08),
                (KEY_E, KeyId::Kp09),
                (KEY_R, KeyId::Kp10),
                (KEY_T, KeyId::Kp22),
            ],
            &[
                (KEY_CAPSLOCK, KeyId::Kp11),
                (KEY_A, KeyId::Kp12),
                (KEY_S, KeyId::Kp13),
                (KEY_D, KeyId::Kp14),
                (KEY_F, KeyId::Kp15),
                (KEY_G, KeyId::Kp23),
            ],
            &[
                (KEY_LEFTSHIFT, KeyId::Kp16),
                (KEY_Z, KeyId::Kp17),
                (KEY_X, KeyId::Kp18),
                (KEY_C, KeyId::Kp19),
                (KEY_V, KeyId::Kp24),
            ],
            &[
                (KEY_LEFTCTRL, KeyId::Kp25),
                (KEY_B, KeyId::Kp26),
                (KEY_I, KeyId::Kp27),
                (KEY_SPACE, KeyId::Kp20),
            ],
        ];
        for row in rows {
            for (code, id) in row {
                assert_eq!(map.get(EV_KEY, *code, 1), Some(*id), "code {code}");
            }
        }
        assert_eq!(map.get(EV_KEY, BTN_THUMBL, 1), Some(KeyId::StickClick));
        for id in KeyId::ALL {
            if id.is_analog() {
                continue;
            }
            assert_eq!(
                map.contains(id),
                DeviceModel::V2Pro.has_key(id),
                "V2 Pro map and has_key disagree on {id:?}"
            );
        }
    }

    #[test]
    fn gamepad_stick_ratio_is_centred_on_zero() {
        assert_eq!(analog_axis_ratio_for(DeviceModel::V2Pro, 0), 0.0);
        assert!((analog_axis_ratio_for(DeviceModel::V2Pro, 32767) - 1.0).abs() < 1e-6);
        assert!(analog_axis_ratio_for(DeviceModel::V2Pro, -32768) <= -1.0);
        assert!((analog_axis_ratio_for(DeviceModel::Pro, 255) - 1.0).abs() < 1e-6);
    }

    fn keypad_and_wheel_mode() -> [KeyId; 24] {
        [
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
            KeyId::Kp20,
            KeyId::WheelUp,
            KeyId::WheelDown,
            KeyId::WheelClick,
            KeyId::Mode,
        ]
    }

    #[test]
    fn a_bare_modifier_key_sends_the_same_code_the_modifier_does() {
        use crate::types::Modifier;
        assert_eq!(
            token_to_evdev(KeyToken::LeftShift),
            modifier_to_evdev(Modifier::Shift)
        );
        assert_eq!(
            token_to_evdev(KeyToken::LeftCtrl),
            modifier_to_evdev(Modifier::Ctrl)
        );
        assert_eq!(
            token_to_evdev(KeyToken::LeftAlt),
            modifier_to_evdev(Modifier::Alt)
        );
    }
}
