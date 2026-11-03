use opentartarus_core::error::ErrorCode;
use opentartarus_core::remap::allow_grab;
use opentartarus_core::types::DeviceModel;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const SCAN_INTERVAL_MISSING_MS: u64 = 1000;
pub const SCAN_INTERVAL_ACTIVE_MS: u64 = 250;
const HEX_RADIX: u32 = 16;
const EVENT_HANDLER_PREFIX: &str = "event";
const DEV_INPUT: &str = "/dev/input";
const PROC_INPUT_DEVICES: &str = "/proc/bus/input/devices";
const HID_DEVICES: &str = "/sys/bus/hid/devices";
const VENDOR_FIELD: &str = "Vendor=";
const PRODUCT_FIELD: &str = "Product=";
const HANDLERS_FIELD: &str = "Handlers=";
const ABS_FIELD: &str = "ABS=";
const REL_FIELD: &str = "REL=";
const LED_FIELD: &str = "LED=";
const HANDLER_MOUSE_PREFIX: &str = "mouse";
const HANDLER_JOYSTICK_PREFIX: &str = "js";
const FUSER_HEADER_USER: &str = "USER";
const FUSER_HEADER_COMMAND: &str = "COMMAND";
const OPENRAZER_COMM_PREFIX: &str = "openrazer";
const OPENRGB_COMM_PREFIX: &str = "openrgb";
const POLYCHROMATIC_COMM_PREFIX: &str = "polychromatic";
const OPENRAZER_DAEMON_NAME: &str = "openrazer-daemon";
const PROC_DIR: &str = "/proc";
const PROC_FD_DIR: &str = "fd";
const PROC_COMM_FILE: &str = "comm";
const FUSER_BIN: &str = "fuser";
const FUSER_VERBOSE_FLAG: &str = "-v";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    pub model: DeviceModel,
    pub vid: u16,
    pub pid: u16,
    pub nodes: Vec<PathBuf>,
    pub grab_nodes: Vec<PathBuf>,
    /// The nodes in `grab_nodes` that are a gamepad (the V2 Pro's thumbstick).
    /// The daemon holds these only while the profile binds the stick.
    pub gamepad_nodes: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    BootKeyboard,
    ExtraKeys,
    Mouse,
    /// A joystick node (`js` handler), such as the V2 Pro's thumbstick.
    Gamepad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceIoKind {
    EvdevGrab,
    EvdevOpen,
    Uinput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusyDecision {
    Share,
    Conflict { holder: Option<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectStatus {
    Missing,
    Permission,
    Present,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceUiState {
    pub present: bool,
    pub model: Option<DeviceModel>,
    pub evdev_ok: bool,
    pub grab_conflict: bool,
}

pub fn model_from_pid(pid: u16) -> Option<DeviceModel> {
    DeviceModel::from_usb_pid(pid)
}

pub fn classify_input_id(vid: u16, pid: u16) -> Option<DeviceModel> {
    if !allow_grab(vid, pid) {
        return None;
    }
    model_from_pid(pid)
}

pub fn pick_first(found: Vec<Detected>) -> Option<Detected> {
    found.into_iter().next()
}

pub fn grab_conflict_message(os: &str) -> ErrorCode {
    classify_device_io(DeviceIoKind::EvdevGrab, os)
}

pub fn classify_device_io(kind: DeviceIoKind, os: &str) -> ErrorCode {
    match kind {
        DeviceIoKind::Uinput => ErrorCode::Permission,
        DeviceIoKind::EvdevGrab | DeviceIoKind::EvdevOpen => {
            if is_busy_os(os) {
                ErrorCode::GrabConflict
            } else {
                ErrorCode::Permission
            }
        }
    }
}

pub fn grab_targets(detected: &Detected) -> &[PathBuf] {
    if detected.grab_nodes.is_empty() {
        &detected.nodes
    } else {
        &detected.grab_nodes
    }
}

pub fn node_role_from_proc_block(block: &str) -> NodeRole {
    let has_abs = block_has_field(block, ABS_FIELD);
    let has_rel = block_has_field(block, REL_FIELD);
    let has_led = block_has_field(block, LED_FIELD);
    let has_mouse = handler_tokens(block).any(|token| token.starts_with(HANDLER_MOUSE_PREFIX));
    let has_joystick = handler_tokens(block).any(|token| {
        token
            .strip_prefix(HANDLER_JOYSTICK_PREFIX)
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
    });
    if has_joystick {
        NodeRole::Gamepad
    } else if has_abs {
        NodeRole::ExtraKeys
    } else if has_mouse || (has_rel && !has_led) {
        NodeRole::Mouse
    } else {
        NodeRole::BootKeyboard
    }
}

pub fn parse_fuser_verbose(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || (trimmed.contains(FUSER_HEADER_USER) && trimmed.contains(FUSER_HEADER_COMMAND))
        {
            continue;
        }
        let rest = trimmed
            .split_once(':')
            .map(|(_, rest)| rest)
            .unwrap_or(trimmed);
        let mut parts = rest.split_whitespace();
        let Some(_user) = parts.next() else {
            continue;
        };
        let Some(_pid) = parts.next() else {
            continue;
        };
        let Some(_access) = parts.next() else {
            continue;
        };
        let Some(cmd) = parts.next() else {
            continue;
        };
        if !names.iter().any(|name| name == cmd) {
            names.push(cmd.to_string());
        }
    }
    names
}

pub fn display_holder_name(raw: &str) -> String {
    if raw.to_ascii_lowercase().starts_with(OPENRAZER_COMM_PREFIX) {
        OPENRAZER_DAEMON_NAME.to_string()
    } else {
        raw.to_string()
    }
}

pub fn is_lighting_holder(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with(OPENRAZER_COMM_PREFIX)
        || lower.starts_with(OPENRGB_COMM_PREFIX)
        || lower.starts_with(POLYCHROMATIC_COMM_PREFIX)
}

pub fn busy_decision(holders: &[String]) -> BusyDecision {
    if holders.is_empty() {
        return BusyDecision::Conflict { holder: None };
    }
    if holders.iter().all(|holder| is_lighting_holder(holder)) {
        return BusyDecision::Share;
    }
    let holder = holders
        .iter()
        .find(|holder| !is_lighting_holder(holder))
        .or_else(|| holders.first())
        .map(|holder| display_holder_name(holder));
    BusyDecision::Conflict { holder }
}

pub fn grab_conflict_status_value(holder: Option<&str>) -> String {
    match holder {
        Some(name) if !name.is_empty() => display_holder_name(name),
        _ => ErrorCode::GrabConflict.user_message().to_string(),
    }
}

pub fn lookup_event_holders(path: &Path) -> Vec<String> {
    let from_proc = holders_from_proc(path);
    if from_proc.is_empty() {
        fuser_holders(path)
    } else {
        from_proc
    }
}

pub fn parse_hid_bus_id(name: &str) -> Option<(u16, u16)> {
    let mut parts = name.split(':');
    let _bus = parts.next()?;
    let vid = u16::from_str_radix(parts.next()?, HEX_RADIX).ok()?;
    let rest = parts.next()?;
    let pid_hex = rest.split('.').next()?;
    let pid = u16::from_str_radix(pid_hex, HEX_RADIX).ok()?;
    Some((vid, pid))
}

pub fn parse_proc_input_devices(text: &str) -> Vec<Detected> {
    let mut order: Vec<(u16, u16)> = Vec::new();
    let mut groups: HashMap<(u16, u16), Vec<(PathBuf, NodeRole)>> = HashMap::new();

    for block in text.split("\n\n") {
        let Some((vid, pid)) = parse_proc_ids(block) else {
            continue;
        };
        if !allow_grab(vid, pid) {
            continue;
        }
        let key = (vid, pid);
        if !groups.contains_key(&key) {
            order.push(key);
        }
        groups
            .entry(key)
            .or_default()
            .extend(parse_proc_node_entries(block));
    }

    order
        .into_iter()
        .filter_map(|(vid, pid)| {
            let model = classify_input_id(vid, pid)?;
            let entries = groups.remove(&(vid, pid)).unwrap_or_default();
            Some(detected_from_entries(model, vid, pid, entries))
        })
        .collect()
}

pub fn hid_catalog_from_names(names: &[&str]) -> Vec<Detected> {
    let mut seen: HashSet<(u16, u16)> = HashSet::new();
    let mut out = Vec::new();
    for name in names {
        let Some((vid, pid)) = parse_hid_bus_id(name) else {
            continue;
        };
        if !seen.insert((vid, pid)) {
            continue;
        }
        let Some(model) = classify_input_id(vid, pid) else {
            continue;
        };
        out.push(Detected {
            model,
            vid,
            pid,
            nodes: Vec::new(),
            grab_nodes: Vec::new(),
            gamepad_nodes: Vec::new(),
        });
    }
    out
}

pub fn merge_catalog(proc: Vec<Detected>, hid: Vec<Detected>) -> Vec<Detected> {
    if proc.is_empty() {
        hid
    } else {
        proc
    }
}

pub fn detect_status(found: Option<&Detected>, readable: impl Fn(&Path) -> bool) -> DetectStatus {
    let Some(dev) = found else {
        return DetectStatus::Missing;
    };
    if dev.nodes.is_empty() {
        return DetectStatus::Permission;
    }
    if dev.nodes.iter().all(|path| readable(path)) {
        DetectStatus::Present
    } else {
        DetectStatus::Permission
    }
}

pub fn device_ui_state(
    status: DetectStatus,
    model: Option<DeviceModel>,
    grab_conflict: bool,
) -> DeviceUiState {
    match status {
        DetectStatus::Missing => DeviceUiState {
            present: false,
            model: None,
            evdev_ok: true,
            grab_conflict: false,
        },
        DetectStatus::Permission => DeviceUiState {
            present: true,
            model,
            evdev_ok: false,
            grab_conflict: false,
        },
        DetectStatus::Present => DeviceUiState {
            present: true,
            model,
            evdev_ok: true,
            grab_conflict,
        },
    }
}

pub fn snapshot_changed(prev: &DeviceUiState, next: &DeviceUiState) -> bool {
    prev != next
}

pub fn scan_interval_ms(grabbed: bool) -> u64 {
    if grabbed {
        SCAN_INTERVAL_ACTIVE_MS
    } else {
        SCAN_INTERVAL_MISSING_MS
    }
}

pub fn enumerate_tartarus() -> Vec<Detected> {
    let proc_text = fs::read_to_string(PROC_INPUT_DEVICES).unwrap_or_default();
    let proc = parse_proc_input_devices(&proc_text);
    let hid_names = hid_dir_names();
    let hid_refs: Vec<&str> = hid_names.iter().map(String::as_str).collect();
    let hid = hid_catalog_from_names(&hid_refs);
    merge_catalog(proc, hid)
}

fn hid_dir_names() -> Vec<String> {
    let Ok(entries) = fs::read_dir(HID_DEVICES) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

fn parse_proc_ids(block: &str) -> Option<(u16, u16)> {
    let line = block.lines().find(|line| line.starts_with("I:"))?;
    let vid = field_hex(line, VENDOR_FIELD)?;
    let pid = field_hex(line, PRODUCT_FIELD)?;
    Some((vid, pid))
}

fn field_hex(line: &str, key: &str) -> Option<u16> {
    let start = line.find(key)? + key.len();
    let token = line[start..].split_whitespace().next()?;
    u16::from_str_radix(token, HEX_RADIX).ok()
}

fn parse_proc_event_nodes(block: &str) -> Vec<PathBuf> {
    handler_tokens(block)
        .filter(|token| is_event_handler(token))
        .map(|token| PathBuf::from(DEV_INPUT).join(token))
        .collect()
}

fn parse_proc_node_entries(block: &str) -> Vec<(PathBuf, NodeRole)> {
    let role = node_role_from_proc_block(block);
    parse_proc_event_nodes(block)
        .into_iter()
        .map(|path| (path, role))
        .collect()
}

fn detected_from_entries(
    model: DeviceModel,
    vid: u16,
    pid: u16,
    entries: Vec<(PathBuf, NodeRole)>,
) -> Detected {
    let nodes: Vec<PathBuf> = entries.iter().map(|(path, _)| path.clone()).collect();
    let gamepad_nodes: Vec<PathBuf> = entries
        .iter()
        .filter(|(_, role)| *role == NodeRole::Gamepad)
        .map(|(path, _)| path.clone())
        .collect();
    let mut grab_nodes: Vec<PathBuf> = entries
        .into_iter()
        .filter(|(_, role)| is_remap_node(model, *role))
        .map(|(path, _)| path)
        .collect();
    if grab_nodes.is_empty() {
        grab_nodes = nodes.clone();
    }
    Detected {
        model,
        vid,
        pid,
        nodes,
        grab_nodes,
        gamepad_nodes,
    }
}

/// Which nodes carry the presses the daemon remaps. The V2 and Pro send
/// their keys on the extra-keys node, and OpenRazer holds the boot keyboard.
/// The V2 Pro sends its keys on the boot keyboard and its stick on a gamepad
/// node, so it needs every node.
fn is_remap_node(model: DeviceModel, role: NodeRole) -> bool {
    match model {
        DeviceModel::V2Pro => true,
        DeviceModel::V2 | DeviceModel::Pro => {
            matches!(role, NodeRole::ExtraKeys | NodeRole::Mouse)
        }
    }
}

fn handler_tokens(block: &str) -> impl Iterator<Item = &str> {
    block
        .lines()
        .find(|line| line.starts_with("H:"))
        .into_iter()
        .flat_map(|line| line.split_whitespace())
        .map(|token| token.strip_prefix(HANDLERS_FIELD).unwrap_or(token))
}

fn block_has_field(block: &str, field: &str) -> bool {
    block.lines().any(|line| line.contains(field))
}

fn is_busy_os(os: &str) -> bool {
    os.contains("EBUSY") || os.contains("Device or resource busy")
}

fn holders_from_proc(path: &Path) -> Vec<String> {
    let Ok(real) = fs::canonicalize(path) else {
        return Vec::new();
    };
    let Ok(proc) = fs::read_dir(PROC_DIR) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in proc.flatten() {
        let pid = entry.file_name();
        if !pid
            .to_string_lossy()
            .as_bytes()
            .iter()
            .all(u8::is_ascii_digit)
        {
            continue;
        }
        let fd_dir = entry.path().join(PROC_FD_DIR);
        let Ok(fds) = fs::read_dir(fd_dir) else {
            continue;
        };
        let hit = fds.flatten().any(|fd| {
            let Ok(dest) = fs::read_link(fd.path()) else {
                return false;
            };
            dest == path || dest == real || fs::canonicalize(&dest).is_ok_and(|got| got == real)
        });
        if !hit {
            continue;
        }
        let Ok(comm) = fs::read_to_string(entry.path().join(PROC_COMM_FILE)) else {
            continue;
        };
        let comm = comm.trim();
        if comm.is_empty() || names.iter().any(|name| name == comm) {
            continue;
        }
        names.push(comm.to_string());
    }
    names
}

fn fuser_holders(path: &Path) -> Vec<String> {
    let Ok(output) = Command::new(FUSER_BIN)
        .arg(FUSER_VERBOSE_FLAG)
        .arg(path)
        .output()
    else {
        return Vec::new();
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_fuser_verbose(&text)
}

fn is_event_handler(token: &str) -> bool {
    token.starts_with(EVENT_HANDLER_PREFIX)
        && token
            .as_bytes()
            .get(EVENT_HANDLER_PREFIX.len())
            .is_some_and(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentartarus_core::constants::{
        USB_PID_NAGA_PRO_1, USB_PID_TARTARUS_PRO, USB_PID_TARTARUS_V2, USB_VID_RAZER,
    };
    use std::path::Path;

    const PROC_V2_AND_NAGA: &str = "\
I: Bus=0003 Vendor=1532 Product=008f Version=0111
N: Name=\"Razer Naga Pro\"
H: Handlers=event22 mouse0

I: Bus=0003 Vendor=1532 Product=022b Version=0111
N: Name=\"Razer Razer Tartarus V2\"
H: Handlers=sysrq kbd leds event270
B: EV=120013
B: LED=7

I: Bus=0003 Vendor=1532 Product=022b Version=0111
N: Name=\"Razer Razer Tartarus V2\"
H: Handlers=sysrq kbd event271
B: EV=10001f
B: REL=1040
B: ABS=10100000000

I: Bus=0003 Vendor=1532 Product=022b Version=0111
N: Name=\"Razer Razer Tartarus V2\"
H: Handlers=event272 mouse10
B: EV=17
B: REL=903

I: Bus=0003 Vendor=1234 Product=5678 Version=0111
N: Name=\"OpenTartarus Keyboard\"
H: Handlers=sysrq kbd event273
";

    const FUSER_OPENRAZER: &str = "\
                     USER        PID ACCESS COMMAND
/dev/input/event270: shane      5782 f.... openrazer-daemo
/dev/input/event271: shane      5782 f.... openrazer-daemo
";

    const FUSER_REMAPPER: &str = "\
                     USER        PID ACCESS COMMAND
/dev/input/event271: root       4169 F.... input-remapper
";

    fn v2_detected(nodes: &[&str]) -> Detected {
        Detected {
            model: DeviceModel::V2,
            vid: USB_VID_RAZER,
            pid: USB_PID_TARTARUS_V2,
            nodes: nodes.iter().map(PathBuf::from).collect(),
            grab_nodes: Vec::new(),
            gamepad_nodes: Vec::new(),
        }
    }

    #[test]
    fn naga_is_never_classified() {
        assert!(classify_input_id(USB_VID_RAZER, USB_PID_NAGA_PRO_1).is_none());
        assert!(!allow_grab(USB_VID_RAZER, USB_PID_NAGA_PRO_1));
        assert_eq!(
            classify_input_id(USB_VID_RAZER, USB_PID_TARTARUS_V2),
            Some(DeviceModel::V2)
        );
        assert_eq!(
            classify_input_id(USB_VID_RAZER, USB_PID_TARTARUS_PRO),
            Some(DeviceModel::Pro)
        );
    }

    #[test]
    fn first_enumerated_wins() {
        let a = Detected {
            model: DeviceModel::Pro,
            vid: USB_VID_RAZER,
            pid: USB_PID_TARTARUS_PRO,
            nodes: vec![],
            grab_nodes: vec![],
            gamepad_nodes: vec![],
        };
        let b = v2_detected(&[]);
        assert_eq!(pick_first(vec![a, b]).unwrap().model, DeviceModel::Pro);
    }

    /// Trimmed from a real V2 Pro's /proc/bus/input/devices.
    const PROC_V2_PRO: &str = "\
I: Bus=0003 Vendor=1532 Product=02eb Version=0101
N: Name=\"Generic X-Box pad\"
H: Handlers=event6 js0
B: EV=20000b

I: Bus=0003 Vendor=1532 Product=02eb Version=0100
N: Name=\"Razer Razer Tartarus V2 Pro Keyboard\"
H: Handlers=sysrq kbd leds event25
B: EV=12001f
B: REL=1040
B: ABS=100000000
B: LED=7

I: Bus=0003 Vendor=1532 Product=02eb Version=0100
N: Name=\"Razer Razer Tartarus V2 Pro Mouse\"
H: Handlers=event26 mouse0
B: EV=17
B: REL=903

I: Bus=0003 Vendor=1532 Product=02eb Version=0100
N: Name=\"Razer Razer Tartarus V2 Pro\"
H: Handlers=event27
B: EV=9
B: ABS=10000000000

I: Bus=0003 Vendor=1532 Product=02eb Version=0111
N: Name=\"Razer Razer Tartarus V2 Pro\"
H: Handlers=sysrq kbd leds event28
B: EV=120013
B: LED=7
";

    #[test]
    fn v2_pro_grabs_its_boot_keyboard_and_marks_the_gamepad() {
        use opentartarus_core::constants::USB_PID_TARTARUS_V2_PRO;
        let found = parse_proc_input_devices(PROC_V2_PRO);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].model, DeviceModel::V2Pro);
        assert_eq!(found[0].pid, USB_PID_TARTARUS_V2_PRO);
        let all: Vec<PathBuf> = ["event6", "event25", "event26", "event27", "event28"]
            .iter()
            .map(|name| PathBuf::from("/dev/input").join(name))
            .collect();
        assert_eq!(found[0].nodes, all);
        // event28 is the boot keyboard, and it is where the V2 Pro's keys arrive.
        assert_eq!(grab_targets(&found[0]), all.as_slice());
        assert_eq!(
            found[0].gamepad_nodes,
            vec![PathBuf::from("/dev/input/event6")]
        );
    }

    #[test]
    fn busy_is_grab_conflict() {
        assert_eq!(
            grab_conflict_message("EBUSY: Device or resource busy"),
            ErrorCode::GrabConflict
        );
    }

    #[test]
    fn access_denied_is_permission_not_missing() {
        assert_eq!(
            grab_conflict_message("Permission denied (os error 13)"),
            ErrorCode::Permission
        );
        assert_eq!(grab_conflict_message("EACCES"), ErrorCode::Permission);
    }

    #[test]
    fn proc_finds_v2_by_vid_pid_despite_openrazer_name() {
        let found = parse_proc_input_devices(PROC_V2_AND_NAGA);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].model, DeviceModel::V2);
        assert_eq!(found[0].vid, USB_VID_RAZER);
        assert_eq!(found[0].pid, USB_PID_TARTARUS_V2);
        assert_eq!(
            found[0].nodes,
            vec![
                PathBuf::from("/dev/input/event270"),
                PathBuf::from("/dev/input/event271"),
                PathBuf::from("/dev/input/event272"),
            ]
        );
        assert_eq!(
            grab_targets(&found[0]),
            &[
                PathBuf::from("/dev/input/event271"),
                PathBuf::from("/dev/input/event272"),
            ]
        );
    }

    #[test]
    fn extra_keys_and_mouse_are_remap_targets_boot_kbd_is_not() {
        assert_eq!(
            node_role_from_proc_block(
                "H: Handlers=sysrq kbd leds event270\nB: EV=120013\nB: LED=7\n"
            ),
            NodeRole::BootKeyboard
        );
        assert_eq!(
            node_role_from_proc_block(
                "H: Handlers=sysrq kbd event271\nB: REL=1040\nB: ABS=10100000000\n"
            ),
            NodeRole::ExtraKeys
        );
        assert_eq!(
            node_role_from_proc_block("H: Handlers=event272 mouse10\nB: REL=903\n"),
            NodeRole::Mouse
        );
    }

    #[test]
    fn evdev_busy_is_grab_conflict_not_not_found() {
        assert_eq!(
            classify_device_io(
                DeviceIoKind::EvdevGrab,
                "Device or resource busy (os error 16)"
            ),
            ErrorCode::GrabConflict
        );
        assert_ne!(
            classify_device_io(
                DeviceIoKind::EvdevGrab,
                "Device or resource busy (os error 16)"
            ),
            ErrorCode::NotFound
        );
        assert_eq!(
            classify_device_io(
                DeviceIoKind::EvdevOpen,
                "No such file or directory (os error 2)"
            ),
            ErrorCode::Permission
        );
        assert_ne!(
            classify_device_io(
                DeviceIoKind::EvdevOpen,
                "No such file or directory (os error 2)"
            ),
            ErrorCode::NotFound
        );
        let missing = device_ui_state(DetectStatus::Missing, None, false);
        assert!(!missing.present);
        assert!(!missing.grab_conflict);
    }

    #[test]
    fn uinput_busy_is_permission_not_grab_conflict() {
        assert_eq!(
            classify_device_io(
                DeviceIoKind::Uinput,
                "Device or resource busy (os error 16)"
            ),
            ErrorCode::Permission
        );
        assert_ne!(
            classify_device_io(DeviceIoKind::Uinput, "EBUSY"),
            ErrorCode::GrabConflict
        );
    }

    #[test]
    fn openrazer_holder_is_shared_not_conflict() {
        let holders = parse_fuser_verbose(FUSER_OPENRAZER);
        assert_eq!(holders, vec!["openrazer-daemo".to_string()]);
        assert_eq!(busy_decision(&holders), BusyDecision::Share);
        assert_eq!(
            display_holder_name("openrazer-daemo"),
            OPENRAZER_DAEMON_NAME
        );
    }

    #[test]
    fn remapper_ebusy_is_grab_conflict_with_process_name() {
        let holders = parse_fuser_verbose(FUSER_REMAPPER);
        assert_eq!(
            busy_decision(&holders),
            BusyDecision::Conflict {
                holder: Some("input-remapper".to_string())
            }
        );
        assert_eq!(
            grab_conflict_status_value(Some("input-remapper")),
            "input-remapper"
        );
        assert_eq!(
            grab_conflict_status_value(None),
            ErrorCode::GrabConflict.user_message()
        );
        let ui = device_ui_state(DetectStatus::Present, Some(DeviceModel::V2), true);
        assert!(ui.present);
        assert!(ui.grab_conflict);
        assert_eq!(detect_status(None, |_| true), DetectStatus::Missing);
    }

    #[test]
    fn empty_proc_is_missing_unless_hid_lists_tartarus() {
        assert!(parse_proc_input_devices("").is_empty());
        assert_eq!(detect_status(None, |_| true), DetectStatus::Missing);
        let hid = hid_catalog_from_names(&[
            "0003:1532:008F.0002",
            "0003:1532:022B.0019",
            "0003:1532:022B.001A",
        ]);
        assert_eq!(hid.len(), 1);
        assert_eq!(hid[0].pid, USB_PID_TARTARUS_V2);
        assert!(hid[0].nodes.is_empty());
        assert_eq!(
            detect_status(hid.first(), |_| true),
            DetectStatus::Permission
        );
        let merged = merge_catalog(Vec::new(), hid);
        assert_eq!(
            detect_status(merged.first(), |_| false),
            DetectStatus::Permission
        );
    }

    #[test]
    fn unreadable_evdev_nodes_are_permission_not_missing() {
        let found = v2_detected(&["/dev/input/event270", "/dev/input/event271"]);
        let status = detect_status(Some(&found), |_path: &Path| false);
        assert_eq!(status, DetectStatus::Permission);
        let ui = device_ui_state(status, Some(DeviceModel::V2), false);
        assert!(ui.present);
        assert!(!ui.evdev_ok);
        assert!(!ui.grab_conflict);
    }

    #[test]
    fn readable_nodes_are_present() {
        let found = v2_detected(&["/dev/input/event270"]);
        let status = detect_status(Some(&found), |_| true);
        assert_eq!(status, DetectStatus::Present);
        let busy = device_ui_state(status, Some(DeviceModel::V2), true);
        assert!(busy.present);
        assert!(busy.evdev_ok);
        assert!(busy.grab_conflict);
    }

    #[test]
    fn device_changed_emits_only_when_snapshot_changes() {
        let missing = device_ui_state(DetectStatus::Missing, None, false);
        let permission = device_ui_state(DetectStatus::Permission, Some(DeviceModel::V2), false);
        assert!(snapshot_changed(&missing, &permission));
        assert!(!snapshot_changed(&permission, &permission));
        assert_eq!(scan_interval_ms(false), SCAN_INTERVAL_MISSING_MS);
        assert_eq!(scan_interval_ms(true), SCAN_INTERVAL_ACTIVE_MS);
        // Read through the function, not the two constants: comparing the
        // constants directly folds to a fixed value at compile time and
        // stops being a test at all.
        let [missing_ms, active_ms] = [scan_interval_ms(false), scan_interval_ms(true)];
        assert!(
            missing_ms > active_ms,
            "an absent pad must be polled less often than a live one"
        );
    }

    #[test]
    fn hid_name_parse_is_case_insensitive_hex() {
        assert_eq!(
            parse_hid_bus_id("0003:1532:022B.0019"),
            Some((USB_VID_RAZER, USB_PID_TARTARUS_V2))
        );
        assert_eq!(
            parse_hid_bus_id("0003:1532:022b.001a"),
            Some((USB_VID_RAZER, USB_PID_TARTARUS_V2))
        );
        assert_eq!(
            parse_hid_bus_id("0003:1532:008F.0002"),
            Some((USB_VID_RAZER, USB_PID_NAGA_PRO_1))
        );
        assert!(classify_input_id(USB_VID_RAZER, USB_PID_NAGA_PRO_1).is_none());
    }
}
