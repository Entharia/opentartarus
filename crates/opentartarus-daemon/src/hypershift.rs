//! The Tartarus V2 Pro's Hypershift button. The pad never sends it as an
//! input event: it reports it on its Razer HID interface (USB interface 1)
//! as a vendor report, `04 01 ...` while held and `04 00 ...` on release.
//! With OpenRazer's driver loaded the report is swallowed before evdev, and
//! without it nothing maps the report to a key, so the daemon reads it from
//! that interface's hidraw node. Reading hidraw takes nothing from the
//! driver or from other readers.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

const HIDRAW_CLASS: &str = "/sys/class/hidraw";
const DEV_DIR: &str = "/dev";
const UEVENT_PATH: &str = "device/uevent";
const HID_ID_FIELD: &str = "HID_ID=";
const HID_PHYS_FIELD: &str = "HID_PHYS=";
/// The interface the pad sends its Razer reports on.
const HYPERSHIFT_INTERFACE_SUFFIX: &str = "/input1";
const RAZER_REPORT_ID: u8 = 0x04;
/// The code the report carries for a held Hypershift (Razer's Fn).
const HYPERSHIFT_CODE: u8 = 0x01;
/// Razer reports are at most 48 bytes; room for a few queued ones.
const READ_BUFFER: usize = 256;

/// Whether a raw report says Hypershift is held. `None` for a report that
/// is not the Razer one and says nothing about it.
pub fn hypershift_held(report: &[u8]) -> Option<bool> {
    let (&id, body) = report.split_first()?;
    (id == RAZER_REPORT_ID).then(|| body.contains(&HYPERSHIFT_CODE))
}

/// Whether a hidraw node's `uevent` text is the pad's Razer interface.
pub fn is_hypershift_interface(uevent: &str, vid: u16, pid: u16) -> bool {
    let id = format!("{vid:08X}:{pid:08X}");
    let field = |key: &str| {
        uevent
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap_or_default()
    };
    field(HID_ID_FIELD).to_ascii_uppercase().ends_with(&id)
        && field(HID_PHYS_FIELD).ends_with(HYPERSHIFT_INTERFACE_SUFFIX)
}

fn find_node(vid: u16, pid: u16) -> Option<PathBuf> {
    let entries = fs::read_dir(HIDRAW_CLASS).ok()?;
    entries.flatten().find_map(|entry| {
        let uevent = fs::read_to_string(entry.path().join(UEVENT_PATH)).ok()?;
        is_hypershift_interface(&uevent, vid, pid)
            .then(|| PathBuf::from(DEV_DIR).join(entry.file_name()))
    })
}

/// The open hidraw node and the last state read from it.
pub struct HypershiftReader {
    file: File,
    held: bool,
}

impl HypershiftReader {
    /// Opens the pad's Razer interface. `None` if it is missing or cannot
    /// be read; the rest of the pad works without it.
    pub fn open(vid: u16, pid: u16) -> Option<Self> {
        let path = find_node(vid, pid)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .ok()?;
        Some(Self { file, held: false })
    }

    pub fn as_raw_fd(&self) -> std::os::fd::RawFd {
        use std::os::fd::AsRawFd;
        self.file.as_raw_fd()
    }

    /// Reads every waiting report and returns each change of state, oldest
    /// first. `Err` when the node is gone.
    pub fn read_changes(&mut self) -> std::io::Result<Vec<bool>> {
        let mut changes = Vec::new();
        let mut buf = [0u8; READ_BUFFER];
        loop {
            match self.file.read(&mut buf) {
                Ok(0) => return Ok(changes),
                // hidraw hands over one report per read.
                Ok(n) => {
                    if let Some(held) = hypershift_held(&buf[..n]) {
                        if held != self.held {
                            self.held = held;
                            changes.push(held);
                        }
                    }
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => return Ok(changes),
                Err(err) if err.kind() == ErrorKind::Interrupted => {}
                Err(err) => return Err(err),
            }
        }
    }

    /// Whether the last report had Hypershift held.
    pub fn held(&self) -> bool {
        self.held
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(second: u8) -> Vec<u8> {
        let mut r = vec![0u8; 48];
        r[0] = RAZER_REPORT_ID;
        r[1] = second;
        r
    }

    #[test]
    fn the_razer_report_says_whether_hypershift_is_held() {
        assert_eq!(hypershift_held(&report(0x01)), Some(true));
        assert_eq!(hypershift_held(&report(0x00)), Some(false));
        assert_eq!(hypershift_held(&[0, 0, 0x35, 0, 0, 0, 0, 0]), None);
        assert_eq!(hypershift_held(&[]), None);
    }

    #[test]
    fn only_interface_one_of_the_pad_is_picked() {
        let uevent = |phys: &str| {
            format!(
                "DRIVER=razerkbd\nHID_ID=0003:00001532:000002EB\nHID_NAME=Razer\nHID_PHYS={phys}\n"
            )
        };
        let (vid, pid) = (0x1532, 0x02eb);
        assert!(is_hypershift_interface(
            &uevent("usb-0000:00:00.0-1/input1"),
            vid,
            pid
        ));
        assert!(!is_hypershift_interface(
            &uevent("usb-0000:00:00.0-1/input2"),
            vid,
            pid
        ));
        assert!(!is_hypershift_interface(
            &uevent("usb-0000:00:00.0-1/input1"),
            vid,
            0x022b
        ));
    }
}
