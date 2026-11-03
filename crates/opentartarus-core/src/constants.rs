pub const APP_NAME: &str = "opentartarus";
pub const USB_VID_RAZER: u16 = 0x1532;
pub const USB_PID_TARTARUS_V2: u16 = 0x022b;
pub const USB_PID_TARTARUS_PRO: u16 = 0x0244;
pub const USB_PID_TARTARUS_V2_PRO: u16 = 0x02eb;
pub const USB_PID_NAGA_PRO_1: u16 = 0x008f;
pub const USB_PID_NAGA_PRO_2: u16 = 0x0090;
pub const RECORD_TIMEOUT_MS: u64 = 8000;
pub const RECORD_TIMEOUT_MAX_MS: u64 = 15000;
pub const TAP_HOLD_MS: u64 = 10;
pub const MACRO_MAX_STEPS: usize = 32;
pub const MACRO_MAX_DELAY_MS: u32 = 5000;
pub const HOLD_REPEAT_RATE_MIN_MS: u32 = 20;
pub const HOLD_REPEAT_RATE_MAX_MS: u32 = 1000;
pub const HOLD_REPEAT_RATE_DEFAULT_MS: u32 = 40;
pub const IPC_MAX_MESSAGE_BYTES: u32 = 1_048_576;
pub const ANALOG_PRESS_RATIO: f32 = 0.40;
pub const ANALOG_RELEASE_RATIO: f32 = 0.30;
/// In 4-way mode the held direction stays held until another one is pushed
/// this much further than it, so a push near a diagonal does not flicker.
pub const STICK_FOUR_WAY_SWITCH_RATIO: f32 = 1.2;
/// Stick rotation is set in whole steps, from -180 to 180 degrees.
pub const STICK_ROTATION_STEP_DEG: i32 = 15;
pub const STICK_ROTATION_MAX_DEG: i32 = 180;
