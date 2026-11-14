use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceModel {
    V2,
    Pro,
    /// The Tartarus V2 Pro (`1532:02eb`): 27 keys in rows of 6, 6, 6, 5 and 4,
    /// a gamepad-style thumbstick with a click, and no scroll wheel.
    #[serde(rename = "v2pro")]
    V2Pro,
}

impl DeviceModel {
    pub const ALL: [DeviceModel; 3] = [DeviceModel::V2, DeviceModel::Pro, DeviceModel::V2Pro];

    pub fn usb_pid(self) -> u16 {
        match self {
            Self::V2 => crate::constants::USB_PID_TARTARUS_V2,
            Self::Pro => crate::constants::USB_PID_TARTARUS_PRO,
            Self::V2Pro => crate::constants::USB_PID_TARTARUS_V2_PRO,
        }
    }

    pub fn from_usb_pid(pid: u16) -> Option<Self> {
        Self::ALL.into_iter().find(|model| model.usb_pid() == pid)
    }

    /// Whether this model physically has `key`.
    pub fn has_key(self, key: KeyId) -> bool {
        match self {
            Self::V2 => !key.is_analog() && !key.is_v2_pro_only(),
            Self::Pro => !key.is_thumb() && !key.is_v2_pro_only(),
            Self::V2Pro => {
                !key.is_thumb()
                    && !matches!(
                        key,
                        KeyId::WheelUp | KeyId::WheelDown | KeyId::WheelClick | KeyId::Mode
                    )
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum KeyId {
    #[serde(rename = "kp01")]
    Kp01,
    #[serde(rename = "kp02")]
    Kp02,
    #[serde(rename = "kp03")]
    Kp03,
    #[serde(rename = "kp04")]
    Kp04,
    #[serde(rename = "kp05")]
    Kp05,
    #[serde(rename = "kp06")]
    Kp06,
    #[serde(rename = "kp07")]
    Kp07,
    #[serde(rename = "kp08")]
    Kp08,
    #[serde(rename = "kp09")]
    Kp09,
    #[serde(rename = "kp10")]
    Kp10,
    #[serde(rename = "kp11")]
    Kp11,
    #[serde(rename = "kp12")]
    Kp12,
    #[serde(rename = "kp13")]
    Kp13,
    #[serde(rename = "kp14")]
    Kp14,
    #[serde(rename = "kp15")]
    Kp15,
    #[serde(rename = "kp16")]
    Kp16,
    #[serde(rename = "kp17")]
    Kp17,
    #[serde(rename = "kp18")]
    Kp18,
    #[serde(rename = "kp19")]
    Kp19,
    #[serde(rename = "kp20")]
    Kp20,
    #[serde(rename = "kp21")]
    Kp21,
    #[serde(rename = "kp22")]
    Kp22,
    #[serde(rename = "kp23")]
    Kp23,
    #[serde(rename = "kp24")]
    Kp24,
    #[serde(rename = "kp25")]
    Kp25,
    #[serde(rename = "kp26")]
    Kp26,
    #[serde(rename = "kp27")]
    Kp27,
    #[serde(rename = "wheel_up")]
    WheelUp,
    #[serde(rename = "wheel_down")]
    WheelDown,
    #[serde(rename = "wheel_click")]
    WheelClick,
    #[serde(rename = "mode")]
    Mode,
    #[serde(rename = "thumb_n")]
    ThumbN,
    #[serde(rename = "thumb_ne")]
    ThumbNe,
    #[serde(rename = "thumb_e")]
    ThumbE,
    #[serde(rename = "thumb_se")]
    ThumbSe,
    #[serde(rename = "thumb_s")]
    ThumbS,
    #[serde(rename = "thumb_sw")]
    ThumbSw,
    #[serde(rename = "thumb_w")]
    ThumbW,
    #[serde(rename = "thumb_nw")]
    ThumbNw,
    #[serde(rename = "analog_up")]
    AnalogUp,
    #[serde(rename = "analog_down")]
    AnalogDown,
    #[serde(rename = "analog_left")]
    AnalogLeft,
    #[serde(rename = "analog_right")]
    AnalogRight,
    #[serde(rename = "stick_click")]
    StickClick,
    /// The V2 Pro's Hypershift button. The pad reports it only on its Razer
    /// HID interface, and the daemon turns that into a press of this key.
    /// Unbound, it is held to reach the profile's Hypershift layer.
    #[serde(rename = "hypershift")]
    Hypershift,
}

impl KeyId {
    /// Every physical key, in declaration order.
    /// 20 grid keys + 3 wheel + mode + 8 thumb directions + 4 analog = 36,
    /// plus the V2 Pro's 7 extra keys, stick click and Hypershift = 45.
    /// Used by exhaustive tests and by the keypad widget's geometry check.
    pub const ALL: [KeyId; 45] = [
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
        KeyId::ThumbN,
        KeyId::ThumbNe,
        KeyId::ThumbE,
        KeyId::ThumbSe,
        KeyId::ThumbS,
        KeyId::ThumbSw,
        KeyId::ThumbW,
        KeyId::ThumbNw,
        KeyId::AnalogUp,
        KeyId::AnalogDown,
        KeyId::AnalogLeft,
        KeyId::AnalogRight,
        KeyId::Kp21,
        KeyId::Kp22,
        KeyId::Kp23,
        KeyId::Kp24,
        KeyId::Kp25,
        KeyId::Kp26,
        KeyId::Kp27,
        KeyId::StickClick,
        KeyId::Hypershift,
    ];

    /// Keys only the Tartarus V2 Pro has.
    pub fn is_v2_pro_only(self) -> bool {
        matches!(
            self,
            Self::Kp21
                | Self::Kp22
                | Self::Kp23
                | Self::Kp24
                | Self::Kp25
                | Self::Kp26
                | Self::Kp27
                | Self::StickClick
                | Self::Hypershift
        )
    }

    /// The thumbstick's directions and its click.
    pub fn is_stick(self) -> bool {
        self.is_analog() || self == Self::StickClick
    }

    pub fn is_analog(self) -> bool {
        matches!(
            self,
            Self::AnalogUp | Self::AnalogDown | Self::AnalogLeft | Self::AnalogRight
        )
    }

    pub fn is_thumb(self) -> bool {
        matches!(
            self,
            Self::ThumbN
                | Self::ThumbNe
                | Self::ThumbE
                | Self::ThumbSe
                | Self::ThumbS
                | Self::ThumbSw
                | Self::ThumbW
                | Self::ThumbNw
        )
    }

    pub(crate) fn from_binding_key(key: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(key.to_owned())).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyToken {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    #[serde(rename = "0")]
    Num0,
    #[serde(rename = "1")]
    Num1,
    #[serde(rename = "2")]
    Num2,
    #[serde(rename = "3")]
    Num3,
    #[serde(rename = "4")]
    Num4,
    #[serde(rename = "5")]
    Num5,
    #[serde(rename = "6")]
    Num6,
    #[serde(rename = "7")]
    Num7,
    #[serde(rename = "8")]
    Num8,
    #[serde(rename = "9")]
    Num9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Escape,
    Tab,
    Backspace,
    Enter,
    Space,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    Minus,
    Equal,
    LeftBrace,
    RightBracket,
    Backslash,
    Semicolon,
    Apostrophe,
    Grave,
    Comma,
    Dot,
    Slash,
    #[serde(rename = "num0")]
    NumPad0,
    #[serde(rename = "num1")]
    NumPad1,
    #[serde(rename = "num2")]
    NumPad2,
    #[serde(rename = "num3")]
    NumPad3,
    #[serde(rename = "num4")]
    NumPad4,
    #[serde(rename = "num5")]
    NumPad5,
    #[serde(rename = "num6")]
    NumPad6,
    #[serde(rename = "num7")]
    NumPad7,
    #[serde(rename = "num8")]
    NumPad8,
    #[serde(rename = "num9")]
    NumPad9,
    NumEnter,
    NumPlus,
    NumMinus,
    NumSlash,
    NumStar,
    NumDot,
    PrintScreen,
    ScrollLock,
    Pause,
    CapsLock,
    NumLock,
    VolumeUp,
    VolumeDown,
    Mute,
    /// The modifier keys as keys in their own right: held for sprint,
    /// crouch or walk in most games. Serialised as `leftshift`, `leftctrl`
    /// and `leftalt`.
    LeftShift,
    LeftCtrl,
    LeftAlt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Super,
    CtrlR,
    ShiftR,
    AltR,
    SuperR,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDir {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Down,
    Up,
    Tap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MouseTarget {
    Button { button: MouseButton },
    Scroll { scroll: ScrollDir },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum MacroKind {
    Key {
        key: KeyToken,
        modifiers: Vec<Modifier>,
    },
    Mouse {
        #[serde(flatten)]
        target: MouseTarget,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroStep {
    #[serde(flatten)]
    pub kind: MacroKind,
    pub edge: Edge,
    pub delay_ms: u32,
}

fn default_hold_repeat_rate_ms() -> u32 {
    crate::constants::HOLD_REPEAT_RATE_DEFAULT_MS
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Key {
        key: KeyToken,
        modifiers: Vec<Modifier>,
    },
    Macro {
        steps: Vec<MacroStep>,
    },
    Mouse {
        #[serde(flatten)]
        target: MouseTarget,
    },
    HoldRepeat {
        inner: Box<Action>,
        #[serde(default = "default_hold_repeat_rate_ms")]
        rate_ms: u32,
    },
    /// Makes the named profile live. The engine only records the request;
    /// the daemon carries it out. See `RemapEngine::take_profile_switch`.
    SwitchProfile {
        profile: String,
    },
    /// Steps to the next profile in list order, wrapping at the end.
    NextProfile,
}

/// A profile change a key asked for. The remap engine records it and the
/// daemon carries it out, because only the daemon can load and activate a
/// profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSwitch {
    To(String),
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LightingEffect {
    Static,
    Wave,
    Spectrum,
    Breath,
    Reactive,
    Starlight,
    None,
    /// A colour for every key, from `Lighting::frame`. Only the V2 Pro's LED
    /// grid is mapped; other models show the home row's colour, static.
    Custom,
}

/// One colour per key, row by row. On the V2 Pro, cell `[row][column]` is
/// the key at that place in `labels::V2_PRO_ROWS`.
pub type LightFrame = Vec<Vec<[u8; 3]>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lighting {
    pub effect: LightingEffect,
    pub brightness: u8,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<LightFrame>,
    /// Moves a custom frame. Ignored by the other effects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<crate::animation::Animation>,
}

impl Lighting {
    /// The one colour that stands for this lighting, for swatches and for
    /// pads that can't show a frame: the home row's first key for a custom
    /// frame, else the plain colour.
    pub fn main_color(&self) -> Option<[u8; 3]> {
        if self.effect == LightingEffect::Custom {
            if let Some(color) = self
                .frame
                .as_ref()
                .and_then(|frame| frame.get(crate::lighting::HOME_ROW))
                .and_then(|row| row.first())
            {
                return Some(*color);
            }
        }
        self.color
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameId {
    Default,
    /// Games the pack no longer ships a profile for. The variants stay so a
    /// copy saved by an earlier version still loads, as a custom profile.
    LeagueOfLegends,
    #[serde(rename = "dota-2")]
    Dota2,
    WorldOfWarcraft,
    FinalFantasyXiv,
    PathOfExile,
    #[serde(rename = "overwatch-2")]
    Overwatch2,
    Valorant,
    #[serde(rename = "counter-strike-2")]
    CounterStrike2,
    ApexLegends,
    Fortnite,
    #[serde(rename = "diablo-4")]
    Diablo4,
    EldenRing,
    #[serde(rename = "guild-wars-2")]
    GuildWars2,
    ElderScrollsOnline,
    Minecraft,
    Deadlock,
    /// A profile the user made, not one from the shipped pack.
    Custom,
}

/// What the thumbstick does, chosen per profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StickMode {
    /// Each direction presses its key. A diagonal holds two.
    #[default]
    Keys8,
    /// Only the direction pushed furthest presses its key.
    Keys4,
    /// Games get the stick as the left stick of a virtual controller.
    AnalogLeft,
    /// Games get the stick as the right stick of a virtual controller.
    AnalogRight,
}

impl StickMode {
    pub const ALL: [StickMode; 4] = [
        StickMode::Keys8,
        StickMode::Keys4,
        StickMode::AnalogLeft,
        StickMode::AnalogRight,
    ];

    pub fn is_analog(self) -> bool {
        matches!(self, Self::AnalogLeft | Self::AnalogRight)
    }

    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Which set of bindings a key uses: the profile's own, or the Hypershift
/// layer, used while the V2 Pro's Hypershift button is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    #[default]
    Base,
    Hypershift,
}

#[derive(Debug, Deserialize)]
struct ProfileDe {
    id: String,
    name: String,
    game: GameId,
    device_models: Vec<DeviceModel>,
    bindings: BTreeMap<String, Action>,
    #[serde(default)]
    model_bindings: BTreeMap<DeviceModel, BTreeMap<String, Action>>,
    #[serde(default)]
    hypershift: BTreeMap<String, Action>,
    #[serde(default)]
    stick_mode: StickMode,
    lighting: Lighting,
    #[serde(default)]
    setup_note: Option<String>,
    #[serde(default)]
    shipped_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "ProfileDe")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub game: GameId,
    pub device_models: Vec<DeviceModel>,
    pub bindings: BTreeMap<KeyId, Action>,
    /// A layout of its own for one model, used on that model instead of
    /// `bindings`. The V2 Pro's face is shaped differently from the V2's, so
    /// a shipped profile can place its keys for that pad on their own. A
    /// model without an entry here uses `bindings`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub model_bindings: BTreeMap<DeviceModel, BTreeMap<KeyId, Action>>,
    /// What keys do while Hypershift is held. Only the V2 Pro has the
    /// button. A key with no binding here keeps its normal one.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hypershift: BTreeMap<KeyId, Action>,
    /// What the thumbstick does with this profile. Rotation is not here: it
    /// is about the player's hand, so it is one setting for every profile.
    #[serde(default, skip_serializing_if = "StickMode::is_default")]
    pub stick_mode: StickMode,
    pub lighting: Lighting,
    /// A one-line step the player must take in the game before this layout
    /// works, such as adding a second key for an action that defaults to a
    /// bare Shift or Ctrl, which the pad cannot send. Only some profiles
    /// need one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_note: Option<String>,
    /// Set only on a user's copy of a shipped profile that the user has not
    /// changed, and holds the revision of the shipped content it came from.
    /// When the pack ships a new layout, such a copy is replaced; an edited
    /// copy has `None` here and is never touched. See `store::copy_on_apply`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shipped_revision: Option<String>,
    #[serde(skip)]
    pub(crate) unknown_key_ids: bool,
}

impl From<ProfileDe> for Profile {
    fn from(raw: ProfileDe) -> Self {
        let mut unknown_key_ids = false;
        let mut parse = |raw: BTreeMap<String, Action>| {
            let mut out = BTreeMap::new();
            for (key, action) in raw {
                match KeyId::from_binding_key(&key) {
                    Some(id) => {
                        out.insert(id, action);
                    }
                    None => unknown_key_ids = true,
                }
            }
            out
        };
        let bindings = parse(raw.bindings);
        let model_bindings = raw
            .model_bindings
            .into_iter()
            .map(|(model, layout)| (model, parse(layout)))
            .collect();
        let hypershift = parse(raw.hypershift);
        Self {
            id: raw.id,
            name: raw.name,
            game: raw.game,
            device_models: raw.device_models,
            bindings,
            model_bindings,
            hypershift,
            stick_mode: raw.stick_mode,
            lighting: raw.lighting,
            setup_note: raw.setup_note,
            shipped_revision: raw.shipped_revision,
            unknown_key_ids,
        }
    }
}
