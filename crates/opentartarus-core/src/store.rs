use crate::error::ErrorCode;
use crate::light_presets::SavedPreset;
use crate::paths::Paths;
use crate::types::Profile;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AppStateFile {
    #[serde(default)]
    pub active_profile_id: String,
    /// How far the thumbstick's directions are turned, in degrees. One
    /// setting for every profile, because it follows the player's hand.
    #[serde(default)]
    pub stick_rotation_deg: i32,
}

/// Profiles live as `<id>.json`. One constant, used to write the path and to
/// recognise a file when listing the directory, so the two cannot disagree.
const PROFILE_FILE_EXTENSION: &str = "json";

fn profile_path(paths: &Paths, id: &str) -> PathBuf {
    paths
        .profiles_dir
        .join(format!("{id}.{PROFILE_FILE_EXTENSION}"))
}

fn ensure_profile_dirs(paths: &Paths) -> Result<(), ErrorCode> {
    fs::create_dir_all(&paths.profiles_dir).map_err(|_| ErrorCode::Io)?;
    if let Some(parent) = paths.state_file.parent() {
        fs::create_dir_all(parent).map_err(|_| ErrorCode::Io)?;
    }
    Ok(())
}

fn parse_profile(bytes: &[u8]) -> Result<Profile, ErrorCode> {
    serde_json::from_slice(bytes).map_err(|_| ErrorCode::InvalidProfile)
}

pub fn write_profile(paths: &Paths, profile: &Profile) -> Result<(), ErrorCode> {
    ensure_profile_dirs(paths)?;
    let path = profile_path(paths, &profile.id);
    let json = serde_json::to_vec_pretty(profile).map_err(|_| ErrorCode::InvalidProfile)?;
    fs::write(path, json).map_err(|_| ErrorCode::Io)
}

pub fn read_user_profile(paths: &Paths, id: &str) -> Result<Profile, ErrorCode> {
    let path = profile_path(paths, id);
    let bytes = fs::read(path).map_err(|_| ErrorCode::Io)?;
    parse_profile(&bytes)
}

/// FNV-1a, 64 bit. This marks which revision of a shipped profile a user's
/// copy was made from. It only has to change when the shipped bytes change
/// and be identical on every machine and every Rust release, which is why it
/// is written out here rather than taken from `DefaultHasher`: that one is
/// explicitly not stable across releases, so a stored value would rot.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

pub fn content_revision(bytes: &[u8]) -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

/// Writes the shipped content as the user's copy and marks it as untouched,
/// so a later pack update can recognise it and replace it. The file is named
/// by `id`, the profile the caller asked for, which is what every other
/// function here uses to find it again.
fn write_shipped_copy(
    paths: &Paths,
    shipped_json: &str,
    id: &str,
    revision: &str,
) -> Result<Profile, ErrorCode> {
    let mut profile = parse_profile(shipped_json.as_bytes())?;
    profile.shipped_revision = Some(revision.to_owned());
    ensure_profile_dirs(paths)?;
    let json = serde_json::to_vec_pretty(&profile).map_err(|_| ErrorCode::InvalidProfile)?;
    fs::write(profile_path(paths, id), json).map_err(|_| ErrorCode::Io)?;
    Ok(profile)
}

/// The editable copy of a shipped profile, created on first use.
///
/// A copy the user has never changed is replaced whenever the pack ships
/// different content, so an improved layout reaches players instead of
/// sitting behind a copy made months ago. A copy the user has edited carries
/// no revision mark and is always kept: their work is never overwritten
/// without them asking, which is what `revert_to_shipped` is for.
pub fn copy_on_apply(paths: &Paths, shipped_json: &str, id: &str) -> Result<Profile, ErrorCode> {
    ensure_profile_dirs(paths)?;
    let revision = content_revision(shipped_json.as_bytes());
    if profile_path(paths, id).exists() {
        let existing = read_user_profile(paths, id)?;
        let is_untouched_and_current =
            existing.shipped_revision.as_deref() == Some(revision.as_str());
        let is_edited = existing.shipped_revision.is_none();
        if is_untouched_and_current || is_edited {
            return Ok(existing);
        }
    }
    write_shipped_copy(paths, shipped_json, id, &revision)
}

/// Every profile id that has a file in the user's profiles directory, in
/// filename order. A missing directory is an empty list, not an error: a
/// fresh install has no user profiles yet.
pub fn list_user_profile_ids(paths: &Paths) -> Result<Vec<String>, ErrorCode> {
    let entries = match fs::read_dir(&paths.profiles_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(ErrorCode::Io),
    };
    let mut ids = Vec::new();
    for entry in entries {
        let path = entry.map_err(|_| ErrorCode::Io)?.path();
        let is_json = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == PROFILE_FILE_EXTENSION)
            .unwrap_or(false);
        if !is_json {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            ids.push(stem.to_owned());
        }
    }
    ids.sort();
    Ok(ids)
}

/// Removes a user profile's file. Deleting one that is not there is not an
/// error: the end state the caller wanted is the state it is in.
pub fn delete_user_profile(paths: &Paths, id: &str) -> Result<(), ErrorCode> {
    match fs::remove_file(profile_path(paths, id)) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ErrorCode::Io),
    }
}

/// Throws away the user's edits and puts the shipped content back. The copy
/// is marked untouched again, so it follows later pack updates.
pub fn revert_to_shipped(
    paths: &Paths,
    shipped_json: &str,
    id: &str,
) -> Result<Profile, ErrorCode> {
    let revision = content_revision(shipped_json.as_bytes());
    write_shipped_copy(paths, shipped_json, id, &revision)
}

fn read_state(paths: &Paths) -> Result<AppStateFile, ErrorCode> {
    if !paths.state_file.exists() {
        return Ok(AppStateFile::default());
    }
    let bytes = fs::read(&paths.state_file).map_err(|_| ErrorCode::Io)?;
    serde_json::from_slice(&bytes).map_err(|_| ErrorCode::InvalidProfile)
}

fn write_state(paths: &Paths, state: &AppStateFile) -> Result<(), ErrorCode> {
    ensure_profile_dirs(paths)?;
    let json = serde_json::to_string(state).map_err(|_| ErrorCode::InvalidProfile)?;
    fs::write(&paths.state_file, json).map_err(|_| ErrorCode::Io)
}

pub fn write_active_id(paths: &Paths, id: &str) -> Result<(), ErrorCode> {
    let mut state = read_state(paths).unwrap_or_default();
    state.active_profile_id = id.to_owned();
    write_state(paths, &state)
}

pub fn read_active_id(paths: &Paths) -> Result<Option<String>, ErrorCode> {
    let state = read_state(paths)?;
    Ok(Some(state.active_profile_id).filter(|id| !id.is_empty()))
}

pub fn write_stick_rotation(paths: &Paths, degrees: i32) -> Result<(), ErrorCode> {
    let mut state = read_state(paths).unwrap_or_default();
    state.stick_rotation_deg = degrees;
    write_state(paths, &state)
}

/// The saved stick rotation, or none if the file is missing or unreadable.
pub fn read_stick_rotation(paths: &Paths) -> i32 {
    read_state(paths)
        .map(|state| state.stick_rotation_deg)
        .ok()
        .filter(|deg| crate::validate::valid_stick_rotation(*deg))
        .unwrap_or(0)
}

/// The user's saved lighting presets, in the order they were made. A missing
/// or unreadable file reads as none, and broken entries are skipped.
pub fn read_saved_presets(paths: &Paths) -> Vec<SavedPreset> {
    fs::read(&paths.light_presets_file)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<SavedPreset>>(&bytes).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(SavedPreset::is_valid)
        .collect()
}

pub fn write_saved_presets(paths: &Paths, presets: &[SavedPreset]) -> Result<(), ErrorCode> {
    fs::create_dir_all(&paths.config_dir).map_err(|_| ErrorCode::Io)?;
    let json = serde_json::to_vec_pretty(presets).map_err(|_| ErrorCode::Io)?;
    fs::write(&paths.light_presets_file, json).map_err(|_| ErrorCode::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_paths() -> Paths {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("opentartarus-store-{n}"));
        fs::create_dir_all(&root).unwrap();
        Paths::from_dirs(root.join("cfg"), root.join("run"), root.join("state"))
    }

    #[test]
    fn the_stick_rotation_and_the_active_profile_share_the_state_file() {
        let paths = tmp_paths();
        assert_eq!(read_stick_rotation(&paths), 0, "nothing saved yet");
        assert_eq!(read_active_id(&paths).unwrap(), None);
        write_stick_rotation(&paths, 45).unwrap();
        assert_eq!(
            read_active_id(&paths).unwrap(),
            None,
            "a rotation alone is no profile"
        );
        write_active_id(&paths, "deadlock").unwrap();
        assert_eq!(
            read_stick_rotation(&paths),
            45,
            "kept when the profile changes"
        );
        write_stick_rotation(&paths, -90).unwrap();
        assert_eq!(read_active_id(&paths).unwrap().as_deref(), Some("deadlock"));
        assert_eq!(read_stick_rotation(&paths), -90);
        // A file from before the setting existed still reads.
        fs::write(&paths.state_file, r#"{"active_profile_id":"dota-2"}"#).unwrap();
        assert_eq!(read_stick_rotation(&paths), 0);
        assert_eq!(read_active_id(&paths).unwrap().as_deref(), Some("dota-2"));
        // A hand-edited value the app would not accept is ignored.
        fs::write(
            &paths.state_file,
            r#"{"active_profile_id":"x","stick_rotation_deg":7}"#,
        )
        .unwrap();
        assert_eq!(read_stick_rotation(&paths), 0);
    }

    const SHIPPED: &str = r#"{
      "id": "default",
      "name": "Default",
      "game": "default",
      "device_models": ["v2", "pro"],
      "bindings": {},
      "lighting": { "effect": "none", "brightness": 80 }
    }"#;

    #[test]
    fn saved_presets_round_trip_and_broken_ones_are_dropped() {
        let paths = tmp_paths();
        assert!(read_saved_presets(&paths).is_empty(), "nothing saved yet");
        let good = SavedPreset {
            name: "Raid".into(),
            frame: vec![vec![[1, 2, 3]; 6]; 5],
        };
        let broken = SavedPreset {
            name: "".into(),
            frame: Vec::new(),
        };
        write_saved_presets(&paths, &[good.clone(), broken]).unwrap();
        assert_eq!(read_saved_presets(&paths), vec![good]);
        fs::write(&paths.light_presets_file, b"not json").unwrap();
        assert!(read_saved_presets(&paths).is_empty());
    }

    #[test]
    fn listing_a_fresh_install_is_empty_not_an_error() {
        let paths = tmp_paths();
        assert!(!paths.profiles_dir.exists(), "fresh: no profiles dir yet");
        assert_eq!(list_user_profile_ids(&paths).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn listing_finds_json_profiles_sorted_and_ignores_other_files() {
        let paths = tmp_paths();
        copy_on_apply(&paths, SHIPPED, "zeta").unwrap();
        copy_on_apply(&paths, SHIPPED, "alpha").unwrap();
        fs::write(paths.profiles_dir.join("notes.txt"), b"not a profile").unwrap();
        fs::write(paths.profiles_dir.join("stray.json.bak"), b"{}").unwrap();
        assert_eq!(
            list_user_profile_ids(&paths).unwrap(),
            vec!["alpha".to_owned(), "zeta".to_owned()]
        );
    }

    #[test]
    fn deleting_removes_the_file_and_is_idempotent() {
        let paths = tmp_paths();
        copy_on_apply(&paths, SHIPPED, "mine").unwrap();
        assert!(paths.profiles_dir.join("mine.json").exists());
        delete_user_profile(&paths, "mine").unwrap();
        assert!(!paths.profiles_dir.join("mine.json").exists());
        // A second delete of a missing profile is the state we wanted.
        delete_user_profile(&paths, "mine").unwrap();
        assert_eq!(list_user_profile_ids(&paths).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn deleting_one_profile_leaves_the_others() {
        let paths = tmp_paths();
        copy_on_apply(&paths, SHIPPED, "keep").unwrap();
        copy_on_apply(&paths, SHIPPED, "drop").unwrap();
        delete_user_profile(&paths, "drop").unwrap();
        assert_eq!(
            list_user_profile_ids(&paths).unwrap(),
            vec!["keep".to_owned()]
        );
    }

    #[test]
    fn copy_on_apply_creates_a_marked_user_file_and_keeps_a_hand_edited_one() {
        let paths = tmp_paths();
        let p = copy_on_apply(&paths, SHIPPED, "default").unwrap();
        assert_eq!(p.id, "default");
        assert_eq!(
            p.shipped_revision.as_deref(),
            Some(content_revision(SHIPPED.as_bytes()).as_str()),
            "a fresh copy is marked with the revision it came from"
        );
        let stored = read_user_profile(&paths, "default").unwrap();
        assert_eq!(stored.name, "Default");
        assert_eq!(stored.bindings, p.bindings);

        // A file edited outside the app carries no mark, so it is left alone.
        let mutated = SHIPPED.replace("Default", "Hacked");
        fs::write(paths.profiles_dir.join("default.json"), mutated.as_bytes()).unwrap();
        let again = copy_on_apply(&paths, SHIPPED, "default").unwrap();
        assert_eq!(again.name, "Hacked");
        assert!(SHIPPED.contains("Default"), "the shipped text is untouched");
    }

    #[test]
    fn an_untouched_copy_follows_a_pack_update() {
        let paths = tmp_paths();
        copy_on_apply(&paths, SHIPPED, "default").unwrap();
        let newer = SHIPPED.replace(
            r#""bindings": {}"#,
            r#""bindings": { "kp01": { "type": "key", "key": "q", "modifiers": [] } }"#,
        );
        let refreshed = copy_on_apply(&paths, &newer, "default").unwrap();
        assert_eq!(
            refreshed.bindings.len(),
            1,
            "the new layout replaced the old copy"
        );
        assert_eq!(
            refreshed.shipped_revision.as_deref(),
            Some(content_revision(newer.as_bytes()).as_str()),
            "and is marked with the new revision"
        );
        // Applying the same pack content again is a no-op, not a rewrite.
        let same = copy_on_apply(&paths, &newer, "default").unwrap();
        assert_eq!(same.shipped_revision, refreshed.shipped_revision);
    }

    #[test]
    fn an_edited_copy_survives_a_pack_update() {
        let paths = tmp_paths();
        let mut mine = copy_on_apply(&paths, SHIPPED, "default").unwrap();
        // What the daemon does on any edit: drop the mark, then write.
        mine.name = "Mine".into();
        mine.shipped_revision = None;
        write_profile(&paths, &mine).unwrap();

        let newer = SHIPPED.replace("Default", "Renamed Upstream");
        let after = copy_on_apply(&paths, &newer, "default").unwrap();
        assert_eq!(
            after.name, "Mine",
            "a pack update must not discard user edits"
        );
        assert_eq!(after.shipped_revision, None);
    }

    #[test]
    fn revert_restores_shipped_content_and_marks_it_untouched_again() {
        let paths = tmp_paths();
        let mut mine = copy_on_apply(&paths, SHIPPED, "default").unwrap();
        mine.name = "Mine".into();
        mine.shipped_revision = None;
        write_profile(&paths, &mine).unwrap();

        let reverted = revert_to_shipped(&paths, SHIPPED, "default").unwrap();
        assert_eq!(reverted.name, "Default");
        assert_eq!(
            reverted.shipped_revision.as_deref(),
            Some(content_revision(SHIPPED.as_bytes()).as_str()),
            "a reverted copy follows pack updates again"
        );
    }

    #[test]
    fn a_content_revision_is_stable_and_changes_with_the_content() {
        let once = content_revision(SHIPPED.as_bytes());
        assert_eq!(
            once,
            content_revision(SHIPPED.as_bytes()),
            "same bytes, same value"
        );
        assert_ne!(
            once,
            content_revision(SHIPPED.replace("80", "70").as_bytes())
        );
        assert_eq!(once.len(), 16, "a fixed-width hex string");
    }

    #[test]
    fn revert_overwrites_a_hand_edited_user_file() {
        let paths = tmp_paths();
        copy_on_apply(&paths, SHIPPED, "default").unwrap();
        fs::write(
            paths.profiles_dir.join("default.json"),
            SHIPPED.replace("Default", "Edited"),
        )
        .unwrap();
        let p = revert_to_shipped(&paths, SHIPPED, "default").unwrap();
        assert_eq!(p.name, "Default");
        assert_eq!(
            read_user_profile(&paths, "default").unwrap().name,
            "Default"
        );
    }

    #[test]
    fn state_roundtrip() {
        let paths = tmp_paths();
        write_active_id(&paths, "default").unwrap();
        assert_eq!(read_active_id(&paths).unwrap().as_deref(), Some("default"));
    }
}
