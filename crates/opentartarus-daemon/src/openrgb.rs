use opentartarus_core::constants::{USB_PID_TARTARUS_PRO, USB_PID_TARTARUS_V2, USB_VID_RAZER};
use opentartarus_core::error::ErrorCode;
use opentartarus_core::lighting::LightingClient;
use opentartarus_core::types::{Lighting, LightingEffect};
use std::path::PathBuf;
use std::process::{Command, Stdio};

const OPENRGB_BIN: &str = "openrgb";
const OPENRGB_NO_AUTOCONNECT: &str = "--noautoconnect";
const OPENRGB_DEVICE_FLAG: &str = "-d";
const OPENRGB_MODE_FLAG: &str = "-m";
const OPENRGB_COLOR_FLAG: &str = "-c";
const OPENRGB_BRIGHTNESS_FLAG: &str = "-b";
const OPENRGB_MODE_OFF: &str = "Off";
const OPENRGB_MODE_STATIC: &str = "Static";
const OPENRGB_MODE_BREATHING: &str = "Breathing";
const OPENRGB_MODE_SPECTRUM: &str = "Spectrum Cycle";
const OPENRGB_MODE_WAVE: &str = "Wave";
pub const OPENRGB_TARTARUS_V2_NAME: &str = "Razer Tartarus V2";
pub const OPENRGB_TARTARUS_PRO_NAME: &str = "Tartarus Pro";
const OPENRGB_TARTARUS_SEARCH: &str = "Tartarus";
const HEX_CHANNEL_WIDTH: usize = 2;

pub struct OpenRgbClient {
    vid: Option<u16>,
    pid: Option<u16>,
}

impl OpenRgbClient {
    pub fn new(vid: Option<u16>, pid: Option<u16>) -> Self {
        Self { vid, pid }
    }

    pub fn set_usb(&mut self, vid: u16, pid: u16) {
        self.vid = Some(vid);
        self.pid = Some(pid);
    }

    pub fn clear_usb(&mut self) {
        self.vid = None;
        self.pid = None;
    }
}

impl LightingClient for OpenRgbClient {
    fn apply(&mut self, lighting: &Lighting) -> Result<(), ErrorCode> {
        let args = openrgb_args(lighting, self.vid, self.pid)?;
        run_openrgb(&args)
    }

    fn available(&self) -> bool {
        openrgb_available(openrgb_binary_present(), self.vid, self.pid)
    }
}

pub fn openrgb_available(binary_present: bool, vid: Option<u16>, pid: Option<u16>) -> bool {
    binary_present && openrgb_ids_supported(vid, pid)
}

pub fn openrgb_ids_supported(vid: Option<u16>, pid: Option<u16>) -> bool {
    match (vid, pid) {
        (Some(vid), Some(pid)) => opentartarus_core::remap::allow_grab(vid, pid),
        (None, None) => true,
        _ => false,
    }
}

pub fn openrgb_device_name(vid: Option<u16>, pid: Option<u16>) -> Option<&'static str> {
    match (vid, pid) {
        (Some(USB_VID_RAZER), Some(USB_PID_TARTARUS_V2)) => Some(OPENRGB_TARTARUS_V2_NAME),
        (Some(USB_VID_RAZER), Some(USB_PID_TARTARUS_PRO)) => Some(OPENRGB_TARTARUS_PRO_NAME),
        (None, None) => Some(OPENRGB_TARTARUS_SEARCH),
        _ => None,
    }
}

pub fn openrgb_mode(effect: LightingEffect) -> Result<&'static str, ErrorCode> {
    match effect {
        LightingEffect::None => Ok(OPENRGB_MODE_OFF),
        // OpenRGB doesn't know these pads' grids; a frame shows its home row.
        LightingEffect::Static | LightingEffect::Custom => Ok(OPENRGB_MODE_STATIC),
        LightingEffect::Breath => Ok(OPENRGB_MODE_BREATHING),
        LightingEffect::Spectrum => Ok(OPENRGB_MODE_SPECTRUM),
        LightingEffect::Wave => Ok(OPENRGB_MODE_WAVE),
        LightingEffect::Reactive | LightingEffect::Starlight => Err(ErrorCode::Lighting),
    }
}

pub fn openrgb_args(
    lighting: &Lighting,
    vid: Option<u16>,
    pid: Option<u16>,
) -> Result<Vec<String>, ErrorCode> {
    let device = openrgb_device_name(vid, pid).ok_or(ErrorCode::Lighting)?;
    let mode = openrgb_mode(lighting.effect)?;
    let mut args = vec![
        OPENRGB_NO_AUTOCONNECT.to_string(),
        OPENRGB_DEVICE_FLAG.to_string(),
        device.to_string(),
        OPENRGB_MODE_FLAG.to_string(),
        mode.to_string(),
        OPENRGB_BRIGHTNESS_FLAG.to_string(),
        lighting.brightness.to_string(),
    ];
    if openrgb_mode_uses_color(lighting.effect) {
        let rgb = lighting.main_color().unwrap_or([u8::MAX, u8::MAX, u8::MAX]);
        args.push(OPENRGB_COLOR_FLAG.to_string());
        args.push(rgb_hex(rgb));
    }
    Ok(args)
}

fn openrgb_mode_uses_color(effect: LightingEffect) -> bool {
    matches!(
        effect,
        LightingEffect::Static | LightingEffect::Breath | LightingEffect::Custom
    )
}

fn rgb_hex(rgb: [u8; 3]) -> String {
    format!(
        "{:0width$X}{:0width$X}{:0width$X}",
        rgb[0],
        rgb[1],
        rgb[2],
        width = HEX_CHANNEL_WIDTH
    )
}

fn openrgb_binary_present() -> bool {
    path_has_binary(OPENRGB_BIN)
}

fn path_has_binary(name: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| executable_in(dir, name))
}

fn executable_in(dir: PathBuf, name: &str) -> bool {
    let path = dir.join(name);
    path.is_file()
}

fn run_openrgb(args: &[String]) -> Result<(), ErrorCode> {
    let status = Command::new(OPENRGB_BIN)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| ErrorCode::Lighting)?;
    if status.success() {
        Ok(())
    } else {
        Err(ErrorCode::Lighting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lighting(effect: LightingEffect, brightness: u8, color: Option<[u8; 3]>) -> Lighting {
        Lighting {
            effect,
            brightness,
            color,
            frame: None,
        }
    }

    #[test]
    fn tartarus_v2_name_matches_live_openrgb_list() {
        assert_eq!(
            openrgb_device_name(Some(USB_VID_RAZER), Some(USB_PID_TARTARUS_V2)),
            Some("Razer Tartarus V2")
        );
        assert_eq!(
            openrgb_device_name(Some(USB_VID_RAZER), Some(USB_PID_TARTARUS_PRO)),
            Some("Tartarus Pro")
        );
        assert_eq!(openrgb_device_name(None, None), Some("Tartarus"));
        assert!(openrgb_device_name(
            Some(USB_VID_RAZER),
            Some(opentartarus_core::constants::USB_PID_NAGA_PRO_1)
        )
        .is_none());
    }

    #[test]
    fn available_ignores_naga_and_needs_binary() {
        assert!(!openrgb_available(false, None, None));
        assert!(openrgb_available(true, None, None));
        assert!(openrgb_available(
            true,
            Some(USB_VID_RAZER),
            Some(USB_PID_TARTARUS_V2)
        ));
        assert!(!openrgb_available(
            true,
            Some(USB_VID_RAZER),
            Some(opentartarus_core::constants::USB_PID_NAGA_PRO_1)
        ));
    }

    #[test]
    fn args_use_documented_cli_and_verified_modes() {
        let static_args = openrgb_args(
            &lighting(LightingEffect::Static, 80, Some([0, 180, 255])),
            Some(USB_VID_RAZER),
            Some(USB_PID_TARTARUS_V2),
        )
        .unwrap();
        assert_eq!(
            static_args,
            vec![
                "--noautoconnect",
                "-d",
                "Razer Tartarus V2",
                "-m",
                "Static",
                "-b",
                "80",
                "-c",
                "00B4FF",
            ]
        );
        let spectrum = openrgb_args(
            &lighting(LightingEffect::Spectrum, 40, None),
            Some(USB_VID_RAZER),
            Some(USB_PID_TARTARUS_V2),
        )
        .unwrap();
        assert_eq!(spectrum[4], "Spectrum Cycle");
        assert!(!spectrum.contains(&"-c".to_string()));
        assert!(openrgb_args(
            &lighting(LightingEffect::Reactive, 50, Some([1, 2, 3])),
            Some(USB_VID_RAZER),
            Some(USB_PID_TARTARUS_V2),
        )
        .is_err());
    }
}
