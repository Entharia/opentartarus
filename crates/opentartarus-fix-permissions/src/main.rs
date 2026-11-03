use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

const GROUP_NAME: &str = "opentartarus";
// The rule must sort before systemd's 73-seat-late.rules, which applies the
// `uaccess` tag. Older builds wrote it as 99-, where the tag did nothing.
const RULES_PATH: &str = "/etc/udev/rules.d/70-opentartarus.rules";
const LEGACY_RULES_PATH: &str = "/etc/udev/rules.d/99-opentartarus.rules";
const UDEV_RULES: &str = include_str!("../../../packaging/udev/70-opentartarus.rules");
const PKEXEC_UID_VAR: &str = "PKEXEC_UID";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), HelperError> {
    let user = resolve_user_from_pkexec_uid()?;
    run_checked("groupadd", &["-f", GROUP_NAME], HelperError::GroupAdd)?;
    run_checked("usermod", &["-aG", GROUP_NAME, &user], HelperError::UserMod)?;
    write_udev_rules_if_missing()?;
    run_checked(
        "udevadm",
        &["control", "--reload-rules"],
        HelperError::UdevReload,
    )?;
    run_checked("udevadm", &["trigger"], HelperError::UdevTrigger)?;
    Ok(())
}

fn resolve_user_from_pkexec_uid() -> Result<String, HelperError> {
    let uid = env::var(PKEXEC_UID_VAR).map_err(|_| HelperError::MissingPkexecUid)?;
    if uid.parse::<u32>().is_err() {
        return Err(HelperError::InvalidPkexecUid);
    }
    let output = Command::new("id")
        .args(["-nu", &uid])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| HelperError::UserLookup)?;
    if !output.status.success() {
        return Err(HelperError::UserLookup);
    }
    let name = String::from_utf8(output.stdout).map_err(|_| HelperError::UserLookup)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(HelperError::UserLookup);
    }
    Ok(name.to_string())
}

fn write_udev_rules_if_missing() -> Result<(), HelperError> {
    // Only this helper ever wrote the old file, so it is safe to remove.
    let legacy = Path::new(LEGACY_RULES_PATH);
    if legacy.exists() {
        fs::remove_file(legacy).map_err(|_| HelperError::WriteRules)?;
    }
    let path = Path::new(RULES_PATH);
    if path.exists() {
        return Ok(());
    }
    fs::write(path, UDEV_RULES).map_err(|_| HelperError::WriteRules)
}

fn run_checked(program: &str, args: &[&str], on_fail: HelperError) -> Result<(), HelperError> {
    let status = Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| on_fail)?;
    if status.success() {
        Ok(())
    } else {
        Err(on_fail)
    }
}

#[derive(Clone, Copy)]
enum HelperError {
    MissingPkexecUid,
    InvalidPkexecUid,
    UserLookup,
    GroupAdd,
    UserMod,
    WriteRules,
    UdevReload,
    UdevTrigger,
}

impl std::fmt::Display for HelperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPkexecUid => write!(f, "PKEXEC_UID is not set"),
            Self::InvalidPkexecUid => write!(f, "PKEXEC_UID is not a valid uid"),
            Self::UserLookup => write!(f, "could not resolve user from PKEXEC_UID"),
            Self::GroupAdd => write!(f, "groupadd failed"),
            Self::UserMod => write!(f, "usermod failed"),
            Self::WriteRules => write!(f, "could not write udev rules"),
            Self::UdevReload => write!(f, "udevadm reload-rules failed"),
            Self::UdevTrigger => write!(f, "udevadm trigger failed"),
        }
    }
}
