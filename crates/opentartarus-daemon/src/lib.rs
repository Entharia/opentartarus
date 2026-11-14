pub mod animator;
pub mod device;
pub mod handler;
pub mod hypershift;
pub mod log;
pub mod openrazer;
pub mod openrgb;
pub mod perms;
pub mod playback;
pub mod server;
pub mod spawn_ui;
pub mod tray;
pub mod uinput_sink;

use crate::openrazer::OpenRazerClient;
use crate::openrgb::OpenRgbClient;
use opentartarus_core::lighting::LightingChain;

pub type DaemonLighting = LightingChain<OpenRazerClient, OpenRgbClient>;

pub fn daemon_lighting(vid: Option<u16>, pid: Option<u16>) -> DaemonLighting {
    LightingChain::new(OpenRazerClient::new(vid, pid), OpenRgbClient::new(vid, pid))
}

pub fn set_daemon_lighting_usb(lighting: &mut DaemonLighting, ids: Option<(u16, u16)>) {
    match ids {
        Some((vid, pid)) => {
            lighting.primary().set_usb(vid, pid);
            lighting.fallback().set_usb(vid, pid);
        }
        None => {
            lighting.primary().clear_usb();
            lighting.fallback().clear_usb();
        }
    }
}
