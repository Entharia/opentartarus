use opentartarus_core::constants::{RECORD_TIMEOUT_MAX_MS, RECORD_TIMEOUT_MS, USB_VID_RAZER};
use opentartarus_core::error::ErrorCode;
use opentartarus_core::ipc::Method;
use opentartarus_core::labels::profile_row_order;
use opentartarus_core::lighting::LightingClient;
use opentartarus_core::pack::{is_shipped, shipped_json, shipped_profile, SHIPPED_IDS};
use opentartarus_core::paths::Paths;
use opentartarus_core::record::{RecordOutcome, Recorder};
use opentartarus_core::remap::RemapEngine;
use opentartarus_core::store::{
    copy_on_apply, delete_user_profile, list_user_profile_ids, read_user_profile,
    revert_to_shipped, write_active_id, write_profile,
};
use opentartarus_core::types::{
    Action, DeviceModel, GameId, KeyId, KeyToken, Lighting, Modifier, MouseButton, MouseTarget,
    Profile, ProfileSwitch, ScrollDir,
};
use opentartarus_core::validate::profile_id_for_name;
use serde_json::{json, Value};

const PROFILE_SOURCE_USER: &str = "user";
const PROFILE_SOURCE_SHIPPED: &str = "shipped";
const USB_ID_HEX_WIDTH: usize = 4;

pub struct DaemonState<L: LightingClient> {
    pub paths: Paths,
    pub engine: RemapEngine,
    pub recorder: Recorder,
    pub lighting: L,
    pub active_id: Option<String>,
    pub device_present: bool,
    pub model: Option<DeviceModel>,
    pub openrazer_available: bool,
    pub evdev_ok: bool,
    pub uinput_ok: bool,
    pub grab_conflict: Option<String>,
}

pub fn handle_request<L: LightingClient>(
    state: &mut DaemonState<L>,
    method: Method,
    params: Value,
    now_ms: u64,
) -> Result<Value, ErrorCode> {
    match method {
        Method::GetStatus => Ok(get_status(state)),
        Method::ListProfiles => list_profiles(state),
        Method::ApplyProfile => apply_profile(state, &params),
        Method::SetBinding => set_binding(state, &params),
        Method::ClearBinding => clear_binding(state, &params),
        Method::StartRecord => start_record(state, &params, now_ms),
        Method::StopRecord => Ok(stop_record(state)),
        Method::SubmitRecord => submit_record(state, &params),
        Method::SetLighting => set_lighting(state, &params),
        Method::RevertProfile => revert_profile(state, &params),
        Method::CreateProfile => create_profile(state, &params),
        Method::DeleteProfile => delete_profile(state, &params),
        Method::ShowWindow => Ok(json!({})),
        Method::QuitDaemon => Ok(json!({ "stopping": true })),
    }
}

fn usb_id_hex(id: u16) -> String {
    format!("{id:0width$x}", width = USB_ID_HEX_WIDTH)
}

fn get_status<L: LightingClient>(state: &DaemonState<L>) -> Value {
    let (model, vid, pid) = if state.device_present {
        let model = match state.model {
            Some(m) => json!(m),
            None => Value::Null,
        };
        let (vid, pid) = match state.model {
            Some(model) => (
                json!(usb_id_hex(USB_VID_RAZER)),
                json!(usb_id_hex(model.usb_pid())),
            ),
            None => (Value::Null, Value::Null),
        };
        (model, vid, pid)
    } else {
        (Value::Null, Value::Null, Value::Null)
    };
    let record = match state.recorder.session() {
        Some(session) => json!({
            "active": true,
            "key_id": session.key_id,
            "deadline_ms": session.deadline_ms,
        }),
        None => json!({
            "active": false,
            "key_id": null,
            "deadline_ms": null,
        }),
    };
    json!({
        "device": {
            "present": state.device_present,
            "model": model,
            "vid": vid,
            "pid": pid,
        },
        "openrazer": { "available": state.openrazer_available },
        "active_profile_id": state.active_id,
        "record": record,
        "permissions": {
            "uinput": state.uinput_ok,
            "evdev": state.evdev_ok,
        },
        "grab_conflict": state.grab_conflict,
    })
}

fn user_profile_exists(paths: &Paths, id: &str) -> bool {
    paths.profiles_dir.join(format!("{id}.json")).exists()
}

/// Shipped profiles first, in pack order, then every custom profile found in
/// the user's directory. A shipped profile can be reverted but not deleted; a
/// custom one is the reverse.
fn list_profiles<L: LightingClient>(state: &DaemonState<L>) -> Result<Value, ErrorCode> {
    let mut profiles = Vec::with_capacity(SHIPPED_IDS.len());
    for id in SHIPPED_IDS {
        let user_exists = user_profile_exists(&state.paths, id);
        let profile = if user_exists {
            read_user_profile(&state.paths, id)?
        } else {
            shipped_profile(id)?
        };
        profiles.push(profile_row(
            state,
            &profile,
            if user_exists {
                PROFILE_SOURCE_USER
            } else {
                PROFILE_SOURCE_SHIPPED
            },
        ));
    }
    for id in list_user_profile_ids(&state.paths)? {
        if is_shipped(&id) {
            continue;
        }
        let profile = read_user_profile(&state.paths, &id)?;
        profiles.push(profile_row(state, &profile, PROFILE_SOURCE_USER));
    }
    Ok(json!({ "profiles": profiles }))
}

fn profile_row<L: LightingClient>(
    state: &DaemonState<L>,
    profile: &Profile,
    source: &str,
) -> Value {
    let shipped = is_shipped(&profile.id);
    json!({
        "id": profile.id,
        "name": profile.name,
        "game": profile.game,
        "source": source,
        "is_active": state.active_id.as_deref() == Some(profile.id.as_str()),
        "can_revert": shipped,
        "can_delete": !shipped,
    })
}

fn require_string(params: &Value, key: &str) -> Result<String, ErrorCode> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(ErrorCode::InvalidProfile)
}

fn parse_key_id(value: &Value) -> Result<KeyId, ErrorCode> {
    serde_json::from_value(value.clone()).map_err(|_| ErrorCode::UnknownKey)
}

fn param_key_id(params: &Value) -> Result<KeyId, ErrorCode> {
    parse_key_id(params.get("key_id").unwrap_or(&Value::Null))
}

fn apply_lighting_soft<L: LightingClient>(
    state: &mut DaemonState<L>,
    lighting: &Lighting,
) -> Result<(), ErrorCode> {
    match state.lighting.apply(lighting) {
        Ok(()) => Ok(()),
        Err(ErrorCode::Lighting) => {
            state.openrazer_available = state.lighting.available();
            crate::log::log_lighting_error();
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn apply_if_active<L: LightingClient>(
    state: &mut DaemonState<L>,
    profile: &Profile,
    apply_lights: bool,
) -> Result<(), ErrorCode> {
    if state.active_id.as_deref() != Some(profile.id.as_str()) {
        return Ok(());
    }
    state.engine.apply_profile(profile);
    if apply_lights {
        apply_lighting_soft(state, &profile.lighting)?;
    }
    Ok(())
}

/// The editable copy of a profile. A shipped id is copied into the user's
/// directory on first use; a custom id lives there already, and only there,
/// so a custom id with no file is not found.
fn load_user_copy<L: LightingClient>(
    state: &DaemonState<L>,
    id: &str,
) -> Result<Profile, ErrorCode> {
    match shipped_json(id) {
        Some(raw) => copy_on_apply(&state.paths, raw, id),
        None if user_profile_exists(&state.paths, id) => read_user_profile(&state.paths, id),
        None => Err(ErrorCode::NotFound),
    }
}

/// Saves a profile the user has just changed. Clearing the revision mark is
/// what tells `copy_on_apply` never to replace this file with a newer shipped
/// layout: the moment a profile is edited it stops being a copy of the pack.
fn persist_profile<L: LightingClient>(
    state: &mut DaemonState<L>,
    profile: &mut Profile,
    apply_lights: bool,
) -> Result<(), ErrorCode> {
    profile.validate()?;
    profile.shipped_revision = None;
    write_profile(&state.paths, profile)?;
    apply_if_active(state, profile, apply_lights)
}

fn apply_profile<L: LightingClient>(
    state: &mut DaemonState<L>,
    params: &Value,
) -> Result<Value, ErrorCode> {
    let id = require_string(params, "id")?;
    activate(state, &id)
}

/// Makes `id` the live profile: loads its editable copy, pushes it into the
/// remap engine, records it as active, and applies its lighting. Shared by
/// apply, create, and the fallback after deleting the active profile.
fn activate<L: LightingClient>(state: &mut DaemonState<L>, id: &str) -> Result<Value, ErrorCode> {
    let profile = load_user_copy(state, id)?;
    profile.validate()?;
    state.engine.apply_profile(&profile);
    write_active_id(&state.paths, id)?;
    state.active_id = Some(id.to_owned());
    apply_lighting_soft(state, &profile.lighting)?;
    Ok(json!({ "id": id }))
}

fn set_binding<L: LightingClient>(
    state: &mut DaemonState<L>,
    params: &Value,
) -> Result<Value, ErrorCode> {
    let profile_id = require_string(params, "profile_id")?;
    let key_id = param_key_id(params)?;
    let action: Action =
        serde_json::from_value(params.get("action").cloned().unwrap_or(Value::Null))
            .map_err(|_| ErrorCode::InvalidProfile)?;
    let mut profile = load_user_copy(state, &profile_id)?;
    profile.layout_mut(state.model).insert(key_id, action);
    persist_profile(state, &mut profile, false)?;
    Ok(json!({ "id": profile_id }))
}

fn clear_binding<L: LightingClient>(
    state: &mut DaemonState<L>,
    params: &Value,
) -> Result<Value, ErrorCode> {
    let profile_id = require_string(params, "profile_id")?;
    let key_id = param_key_id(params)?;
    let mut profile = load_user_copy(state, &profile_id)?;
    profile.layout_mut(state.model).remove(&key_id);
    persist_profile(state, &mut profile, false)?;
    Ok(json!({ "id": profile_id }))
}

fn start_record<L: LightingClient>(
    state: &mut DaemonState<L>,
    params: &Value,
    now_ms: u64,
) -> Result<Value, ErrorCode> {
    let key_id = param_key_id(params)?;
    let timeout_ms = params.get("timeout_ms").and_then(Value::as_u64);
    let timeout = timeout_ms
        .unwrap_or(RECORD_TIMEOUT_MS)
        .min(RECORD_TIMEOUT_MAX_MS);
    state.recorder.start(key_id, timeout_ms, now_ms)?;
    Ok(json!({
        "key_id": key_id,
        "timeout_ms": timeout,
    }))
}

fn stop_record<L: LightingClient>(state: &mut DaemonState<L>) -> Value {
    let cancelled = state.recorder.stop();
    json!({ "cancelled": cancelled })
}

fn submit_record<L: LightingClient>(
    state: &mut DaemonState<L>,
    params: &Value,
) -> Result<Value, ErrorCode> {
    if !state.recorder.is_active() {
        return Err(ErrorCode::NotFound);
    }
    let profile_id = state.active_id.clone().ok_or(ErrorCode::NotFound)?;
    let outcome = if let Some(key) = params.get("key") {
        let key: KeyToken =
            serde_json::from_value(key.clone()).map_err(|_| ErrorCode::InvalidProfile)?;
        let modifiers: Vec<Modifier> = match params.get("modifiers") {
            Some(v) => serde_json::from_value(v.clone()).map_err(|_| ErrorCode::InvalidProfile)?,
            None => Vec::new(),
        };
        state.recorder.submit_key(key, modifiers)?
    } else if let Some(button) = params.get("button") {
        let button: MouseButton =
            serde_json::from_value(button.clone()).map_err(|_| ErrorCode::InvalidProfile)?;
        state
            .recorder
            .submit_mouse(MouseTarget::Button { button })?
    } else if let Some(scroll) = params.get("scroll") {
        let scroll: ScrollDir =
            serde_json::from_value(scroll.clone()).map_err(|_| ErrorCode::InvalidProfile)?;
        state
            .recorder
            .submit_mouse(MouseTarget::Scroll { scroll })?
    } else {
        return Err(ErrorCode::InvalidProfile);
    };
    match outcome {
        RecordOutcome::Recorded { key_id, action } => {
            let mut profile = load_user_copy(state, &profile_id)?;
            profile
                .layout_mut(state.model)
                .insert(key_id, action.clone());
            persist_profile(state, &mut profile, false)?;
            Ok(json!({
                "key_id": key_id,
                "action": action,
            }))
        }
        RecordOutcome::Cancelled { .. } => Ok(json!({})),
    }
}

fn set_lighting<L: LightingClient>(
    state: &mut DaemonState<L>,
    params: &Value,
) -> Result<Value, ErrorCode> {
    let profile_id = require_string(params, "profile_id")?;
    let lighting: Lighting =
        serde_json::from_value(params.get("lighting").cloned().unwrap_or(Value::Null))
            .map_err(|_| ErrorCode::InvalidProfile)?;
    let mut profile = load_user_copy(state, &profile_id)?;
    profile.lighting = lighting;
    persist_profile(state, &mut profile, true)?;
    Ok(json!({ "id": profile_id }))
}

fn revert_profile<L: LightingClient>(
    state: &mut DaemonState<L>,
    params: &Value,
) -> Result<Value, ErrorCode> {
    let id = require_string(params, "id")?;
    let raw = shipped_json(&id).ok_or(ErrorCode::NotFound)?;
    let profile = revert_to_shipped(&state.paths, raw, &id)?;
    profile.validate()?;
    apply_if_active(state, &profile, true)?;
    Ok(json!({ "id": id }))
}

/// The profile a new one is copied from when the request names none: the
/// active profile, or the shipped default on a session with nothing applied.
const CREATE_FALLBACK_SOURCE: &str = "default";

/// Creates a custom profile as a copy of an existing one under a new name,
/// then makes it active. Copying rather than starting blank is the real use:
/// take a game's layout, change a few keys, keep it as your own.
fn create_profile<L: LightingClient>(
    state: &mut DaemonState<L>,
    params: &Value,
) -> Result<Value, ErrorCode> {
    let name = require_string(params, "name")?;
    let source_id = params
        .get("copy_from")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| state.active_id.clone())
        .unwrap_or_else(|| CREATE_FALLBACK_SOURCE.to_owned());

    let id = profile_id_for_name(&name, &profile_ids_in_order(state)?);

    let mut profile = load_user_copy(state, &source_id)?;
    profile.id = id.clone();
    profile.name = name.trim().to_owned();
    profile.game = GameId::Custom;
    // A custom profile is the user's own from here on. It must not follow
    // the layout it was copied from when the pack changes.
    profile.shipped_revision = None;
    profile.validate()?;
    write_profile(&state.paths, &profile)?;
    activate(state, &id)
}

/// Deletes a custom profile. Shipped profiles are refused: they can be
/// reverted, never removed. Deleting the active profile falls back to the
/// shipped default so the pad is never left with nothing applied.
fn delete_profile<L: LightingClient>(
    state: &mut DaemonState<L>,
    params: &Value,
) -> Result<Value, ErrorCode> {
    let id = require_string(params, "id")?;
    if is_shipped(&id) {
        return Err(ErrorCode::InvalidProfile);
    }
    if !user_profile_exists(&state.paths, &id) {
        return Err(ErrorCode::NotFound);
    }
    delete_user_profile(&state.paths, &id)?;
    let was_active = state.active_id.as_deref() == Some(id.as_str());
    if was_active {
        activate(state, CREATE_FALLBACK_SOURCE)?;
    }
    Ok(json!({ "id": id, "active_id": state.active_id }))
}

/// Every profile id the daemon lists, in list order: shipped first in pack
/// order, then custom. What `NextProfile` steps through, and what a new
/// profile's id must not collide with.
fn profile_ids_in_order<L: LightingClient>(
    state: &DaemonState<L>,
) -> Result<Vec<String>, ErrorCode> {
    let mut ids: Vec<String> = SHIPPED_IDS.iter().map(|id| (*id).to_owned()).collect();
    ids.extend(
        list_user_profile_ids(&state.paths)?
            .into_iter()
            .filter(|id| !is_shipped(id)),
    );
    Ok(profile_row_order(&ids))
}

/// The profile after the active one in list order, wrapping at the end.
/// With nothing applied, the first profile.
fn next_profile_id<L: LightingClient>(state: &DaemonState<L>) -> Result<String, ErrorCode> {
    let ids = profile_ids_in_order(state)?;
    if ids.is_empty() {
        return Err(ErrorCode::NotFound);
    }
    let next = state
        .active_id
        .as_deref()
        .and_then(|active| ids.iter().position(|id| id == active))
        .map_or(0, |index| (index + 1) % ids.len());
    ids.get(next).cloned().ok_or(ErrorCode::NotFound)
}

/// Carries out a profile change a key on the pad asked for, and reports
/// which profile is live now. Public because the device loop calls it: the
/// remap engine only records the request.
pub fn switch_profile<L: LightingClient>(
    state: &mut DaemonState<L>,
    switch: &ProfileSwitch,
) -> Result<String, ErrorCode> {
    let id = match switch {
        ProfileSwitch::To(id) => id.clone(),
        ProfileSwitch::Next => next_profile_id(state)?,
    };
    activate(state, &id)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentartarus_core::ipc::Method;
    use opentartarus_core::lighting::RecordingLighting;
    use opentartarus_core::pack::shipped_json;
    use opentartarus_core::paths::Paths;
    use opentartarus_core::record::Recorder;
    use opentartarus_core::remap::RemapEngine;
    use opentartarus_core::types::DeviceModel;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn state() -> DaemonState<RecordingLighting> {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("opentartarus-h-{n}"));
        let paths = Paths::from_dirs(root.join("c"), root.join("r"), root.join("s"));
        DaemonState {
            paths,
            engine: RemapEngine::new(DeviceModel::V2),
            recorder: Recorder::default(),
            lighting: RecordingLighting {
                available: true,
                last: None,
            },
            active_id: None,
            device_present: true,
            model: Some(DeviceModel::V2),
            openrazer_available: true,
            evdev_ok: true,
            uinput_ok: true,
            grab_conflict: None,
        }
    }

    #[test]
    fn apply_set_binding_and_status() {
        let mut st = state();
        let v = handle_request(&mut st, Method::ApplyProfile, json!({"id":"dota-2"}), 0).unwrap();
        assert_eq!(v["id"], "dota-2");
        assert_eq!(st.active_id.as_deref(), Some("dota-2"));
        handle_request(
            &mut st,
            Method::SetBinding,
            json!({"profile_id":"dota-2","key_id":"kp01","action":{"type":"key","key":"c","modifiers":["ctrl"]}}),
            1,
        )
        .unwrap();
        let s = handle_request(&mut st, Method::GetStatus, json!({}), 2).unwrap();
        assert_eq!(s["active_profile_id"], "dota-2");
        assert_eq!(s["device"]["model"], "v2");
    }

    #[test]
    fn record_submit_and_stop() {
        let mut st = state();
        handle_request(&mut st, Method::ApplyProfile, json!({"id":"default"}), 0).unwrap();
        handle_request(
            &mut st,
            Method::StartRecord,
            json!({"key_id":"kp01","timeout_ms":8000}),
            0,
        )
        .unwrap();
        handle_request(
            &mut st,
            Method::SubmitRecord,
            json!({"key":"c","modifiers":["ctrl"]}),
            1,
        )
        .unwrap();
        assert!(!st.recorder.is_active());
        handle_request(&mut st, Method::StartRecord, json!({"key_id":"kp02"}), 2).unwrap();
        let stop = handle_request(&mut st, Method::StopRecord, json!({}), 3).unwrap();
        assert_eq!(stop["cancelled"], true);
    }

    #[test]
    fn get_status_matches_spec_shape() {
        let mut st = state();
        let s = handle_request(&mut st, Method::GetStatus, json!({}), 0).unwrap();
        assert_eq!(s["device"]["present"], true);
        assert_eq!(s["device"]["model"], "v2");
        assert_eq!(s["device"]["vid"], "1532");
        assert_eq!(s["device"]["pid"], "022b");
        assert_eq!(s["openrazer"]["available"], true);
        assert_eq!(s["active_profile_id"], serde_json::Value::Null);
        assert_eq!(s["record"]["active"], false);
        assert_eq!(s["record"]["key_id"], serde_json::Value::Null);
        assert_eq!(s["record"]["deadline_ms"], serde_json::Value::Null);
        assert_eq!(s["permissions"]["uinput"], true);
        assert_eq!(s["permissions"]["evdev"], true);
        assert_eq!(s["grab_conflict"], serde_json::Value::Null);
        let _ = shipped_json("default");
        st.model = Some(DeviceModel::Pro);
        let pro = handle_request(&mut st, Method::GetStatus, json!({}), 1).unwrap();
        assert_eq!(pro["device"]["pid"], "0244");
        st.device_present = false;
        let gone = handle_request(&mut st, Method::GetStatus, json!({}), 2).unwrap();
        assert_eq!(gone["device"]["present"], false);
        assert_eq!(gone["device"]["model"], serde_json::Value::Null);
        assert_eq!(gone["device"]["vid"], serde_json::Value::Null);
        assert_eq!(gone["device"]["pid"], serde_json::Value::Null);
    }

    fn create(st: &mut DaemonState<RecordingLighting>, name: &str) -> String {
        let v = handle_request(st, Method::CreateProfile, json!({ "name": name }), 0).unwrap();
        v["id"].as_str().unwrap().to_owned()
    }

    fn row_ids(st: &mut DaemonState<RecordingLighting>) -> Vec<String> {
        let v = handle_request(st, Method::ListProfiles, json!({}), 0).unwrap();
        v["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn create_copies_the_active_profile_under_a_new_name_and_activates_it() {
        let mut st = state();
        handle_request(&mut st, Method::ApplyProfile, json!({"id": "dota-2"}), 0).unwrap();
        let id = create(&mut st, "My Raid Layout");
        assert_eq!(id, "my-raid-layout");
        assert_eq!(st.active_id.as_deref(), Some("my-raid-layout"));

        let mine = read_user_profile(&st.paths, &id).unwrap();
        let dota = shipped_profile("dota-2").unwrap();
        assert_eq!(mine.name, "My Raid Layout");
        assert_eq!(mine.game, GameId::Custom);
        assert_eq!(
            mine.bindings, dota.bindings,
            "starts as a copy of the source"
        );
        assert_eq!(mine.lighting, dota.lighting);
    }

    #[test]
    fn create_with_nothing_applied_copies_the_shipped_default() {
        let mut st = state();
        assert_eq!(st.active_id, None);
        let id = create(&mut st, "Blank Slate");
        let mine = read_user_profile(&st.paths, &id).unwrap();
        let default = shipped_profile("default").unwrap();
        assert_eq!(
            mine.bindings, default.bindings,
            "starts as a copy of the shipped default"
        );
        assert_eq!(mine.game, GameId::Custom);
    }

    #[test]
    fn create_honours_an_explicit_source() {
        let mut st = state();
        let v = handle_request(
            &mut st,
            Method::CreateProfile,
            json!({ "name": "Dota Tweaks", "copy_from": "dota-2" }),
            0,
        )
        .unwrap();
        let mine = read_user_profile(&st.paths, v["id"].as_str().unwrap()).unwrap();
        assert_eq!(mine.bindings, shipped_profile("dota-2").unwrap().bindings);
    }

    #[test]
    fn create_never_collides_with_a_shipped_id_or_an_earlier_custom_one() {
        let mut st = state();
        // "Default" would slug to "default", which the pack owns.
        assert_eq!(create(&mut st, "Default"), "default-2");
        assert_eq!(create(&mut st, "Default"), "default-3");
        assert_eq!(create(&mut st, "Fresh"), "fresh");
        assert_eq!(create(&mut st, "Fresh"), "fresh-2");
    }

    #[test]
    fn create_rejects_a_blank_name() {
        let mut st = state();
        let err = handle_request(&mut st, Method::CreateProfile, json!({ "name": "   " }), 0)
            .unwrap_err();
        assert_eq!(err, ErrorCode::InvalidProfile);
        let err = handle_request(&mut st, Method::CreateProfile, json!({}), 0).unwrap_err();
        assert_eq!(err, ErrorCode::InvalidProfile);
    }

    #[test]
    fn list_shows_custom_profiles_after_the_pack_with_the_right_flags() {
        let mut st = state();
        let id = create(&mut st, "Mine");
        let v = handle_request(&mut st, Method::ListProfiles, json!({}), 0).unwrap();
        let rows = v["profiles"].as_array().unwrap();
        assert_eq!(rows.len(), SHIPPED_IDS.len() + 1);
        let last = &rows[rows.len() - 1];
        assert_eq!(last["id"], id);
        assert_eq!(last["source"], "user");
        assert_eq!(last["can_revert"], false, "nothing shipped to revert to");
        assert_eq!(last["can_delete"], true);
        assert_eq!(last["is_active"], true);
        assert_eq!(
            rows[0]["can_delete"], false,
            "shipped rows cannot be deleted"
        );
    }

    #[test]
    fn a_custom_profile_can_be_applied_and_edited_like_any_other() {
        let mut st = state();
        let id = create(&mut st, "Mine");
        handle_request(&mut st, Method::ApplyProfile, json!({"id": "default"}), 0).unwrap();
        handle_request(&mut st, Method::ApplyProfile, json!({"id": id}), 1).unwrap();
        assert_eq!(st.active_id.as_deref(), Some(id.as_str()));
        handle_request(
            &mut st,
            Method::SetBinding,
            json!({
                "profile_id": id,
                "key_id": "kp01",
                "action": { "type": "key", "key": "q", "modifiers": [] }
            }),
            2,
        )
        .unwrap();
        let mine = read_user_profile(&st.paths, &id).unwrap();
        assert_eq!(
            mine.bindings.get(&KeyId::Kp01),
            Some(&Action::Key {
                key: KeyToken::Q,
                modifiers: vec![]
            }),
            "the edit lands on the custom copy"
        );
    }

    #[test]
    fn an_edit_on_the_v2_pro_changes_its_own_layout_and_leaves_the_shared_one() {
        let mut st = state();
        st.model = Some(DeviceModel::V2Pro);
        handle_request(&mut st, Method::ApplyProfile, json!({"id": "deadlock"}), 0).unwrap();
        let bind = |st: &mut DaemonState<_>, key_id: &str, key: &str| {
            handle_request(
                st,
                Method::SetBinding,
                json!({
                    "profile_id": "deadlock",
                    "key_id": key_id,
                    "action": { "type": "key", "key": key, "modifiers": [] }
                }),
                1,
            )
            .unwrap();
        };
        bind(&mut st, "kp12", "h");
        let shipped = shipped_profile("deadlock").unwrap();
        let mine = read_user_profile(&st.paths, "deadlock").unwrap();
        let h = Action::Key {
            key: KeyToken::H,
            modifiers: vec![],
        };
        assert_eq!(
            mine.model_bindings[&DeviceModel::V2Pro].get(&KeyId::Kp12),
            Some(&h)
        );
        assert_eq!(
            mine.bindings, shipped.bindings,
            "the V2 layout is untouched"
        );

        handle_request(
            &mut st,
            Method::ClearBinding,
            json!({"profile_id": "deadlock", "key_id": "stick_click"}),
            2,
        )
        .unwrap();
        let mine = read_user_profile(&st.paths, "deadlock").unwrap();
        assert!(!mine.model_bindings[&DeviceModel::V2Pro].contains_key(&KeyId::StickClick));

        // On a V2 the same edit goes to the shared layout.
        st.model = Some(DeviceModel::V2);
        bind(&mut st, "kp12", "j");
        let mine = read_user_profile(&st.paths, "deadlock").unwrap();
        assert_eq!(
            mine.bindings.get(&KeyId::Kp12),
            Some(&Action::Key {
                key: KeyToken::J,
                modifiers: vec![]
            })
        );
        assert_eq!(
            mine.model_bindings[&DeviceModel::V2Pro].get(&KeyId::Kp12),
            Some(&h)
        );
    }

    #[test]
    fn delete_removes_a_custom_profile_and_falls_back_when_it_was_active() {
        let mut st = state();
        let id = create(&mut st, "Mine");
        assert_eq!(st.active_id.as_deref(), Some(id.as_str()));
        let v = handle_request(&mut st, Method::DeleteProfile, json!({"id": id}), 0).unwrap();
        assert_eq!(
            v["active_id"], "default",
            "the pad is never left with nothing"
        );
        assert_eq!(st.active_id.as_deref(), Some("default"));
        assert!(!row_ids(&mut st).contains(&id));
    }

    #[test]
    fn delete_of_an_inactive_custom_profile_leaves_the_active_one_alone() {
        let mut st = state();
        let doomed = create(&mut st, "Doomed");
        let keeper = create(&mut st, "Keeper");
        assert_eq!(st.active_id.as_deref(), Some(keeper.as_str()));
        handle_request(&mut st, Method::DeleteProfile, json!({"id": doomed}), 0).unwrap();
        assert_eq!(st.active_id.as_deref(), Some(keeper.as_str()));
    }

    #[test]
    fn delete_refuses_shipped_profiles_and_unknown_ids() {
        let mut st = state();
        for id in SHIPPED_IDS {
            let err =
                handle_request(&mut st, Method::DeleteProfile, json!({"id": id}), 0).unwrap_err();
            assert_eq!(err, ErrorCode::InvalidProfile, "{id} must not be deletable");
        }
        let err =
            handle_request(&mut st, Method::DeleteProfile, json!({"id": "nope"}), 0).unwrap_err();
        assert_eq!(err, ErrorCode::NotFound);
    }

    #[test]
    fn list_profiles_shipped_order_and_user_source() {
        let mut st = state();
        handle_request(&mut st, Method::ApplyProfile, json!({"id": "default"}), 0).unwrap();
        let v = handle_request(&mut st, Method::ListProfiles, json!({}), 1).unwrap();
        let rows = v["profiles"].as_array().unwrap();
        let ids: Vec<&str> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(ids, SHIPPED_IDS.to_vec());
        assert_eq!(rows[0]["source"], "user");
        assert_eq!(rows[0]["is_active"], true);
        assert_eq!(rows[0]["can_revert"], true);
        assert_eq!(rows[1]["source"], "shipped");
        assert_eq!(rows[1]["can_revert"], true);
        assert_eq!(rows[1]["is_active"], false);
    }

    #[test]
    fn unknown_key_and_missing_profile() {
        use opentartarus_core::error::ErrorCode;
        let mut st = state();
        let err = handle_request(
            &mut st,
            Method::SetBinding,
            json!({"profile_id":"dota-2","key_id":"kp99","action":{"type":"key","key":"c","modifiers":[]}}),
            0,
        )
        .unwrap_err();
        assert_eq!(err, ErrorCode::UnknownKey);
        let err = handle_request(
            &mut st,
            Method::ClearBinding,
            json!({"profile_id":"no-such-profile","key_id":"kp01"}),
            1,
        )
        .unwrap_err();
        assert_eq!(err, ErrorCode::NotFound);
        let err =
            handle_request(&mut st, Method::ApplyProfile, json!({"id":"nope"}), 2).unwrap_err();
        assert_eq!(err, ErrorCode::NotFound);
    }

    #[test]
    fn lighting_error_does_not_fail_apply() {
        let mut st = state();
        st.lighting.available = false;
        let v = handle_request(&mut st, Method::ApplyProfile, json!({"id":"default"}), 0).unwrap();
        assert_eq!(v["id"], "default");
        assert!(!st.openrazer_available);
    }

    #[test]
    fn lighting_error_does_not_drop_device_or_latch_if_backend_still_up() {
        struct PeerFrontendLighting;
        impl LightingClient for PeerFrontendLighting {
            fn apply(&mut self, _lighting: &Lighting) -> Result<(), ErrorCode> {
                Err(ErrorCode::Lighting)
            }
            fn available(&self) -> bool {
                true
            }
        }
        let ready = state();
        let mut st = DaemonState {
            lighting: PeerFrontendLighting,
            device_present: true,
            openrazer_available: true,
            paths: ready.paths,
            engine: ready.engine,
            recorder: ready.recorder,
            active_id: None,
            model: Some(DeviceModel::V2),
            evdev_ok: true,
            uinput_ok: true,
            grab_conflict: None,
        };
        let v = handle_request(&mut st, Method::ApplyProfile, json!({"id":"default"}), 0).unwrap();
        assert_eq!(v["id"], "default");
        assert!(st.device_present);
        assert!(st.openrazer_available);
        let status = handle_request(&mut st, Method::GetStatus, json!({}), 1).unwrap();
        assert_eq!(status["device"]["present"], true);
        assert_eq!(status["openrazer"]["available"], true);
    }

    #[test]
    fn stop_record_without_session_is_ok() {
        let mut st = state();
        let stop = handle_request(&mut st, Method::StopRecord, json!({}), 0).unwrap();
        assert_eq!(stop["cancelled"], false);
    }

    #[test]
    fn start_record_result_includes_timeout_ms() {
        let mut st = state();
        handle_request(&mut st, Method::ApplyProfile, json!({"id":"default"}), 0).unwrap();
        let started = handle_request(
            &mut st,
            Method::StartRecord,
            json!({"key_id":"kp01","timeout_ms":8000}),
            100,
        )
        .unwrap();
        assert_eq!(started["key_id"], "kp01");
        assert_eq!(started["timeout_ms"], 8000);
        let s = handle_request(&mut st, Method::GetStatus, json!({}), 101).unwrap();
        assert_eq!(s["record"]["active"], true);
        assert_eq!(s["record"]["key_id"], "kp01");
        assert_eq!(s["record"]["deadline_ms"], 8100);
    }

    #[test]
    fn revert_show_quit_and_set_lighting() {
        let mut st = state();
        handle_request(&mut st, Method::ApplyProfile, json!({"id":"default"}), 0).unwrap();
        handle_request(
            &mut st,
            Method::SetLighting,
            json!({"profile_id":"default","lighting":{"effect":"spectrum","brightness":60}}),
            1,
        )
        .unwrap();
        let reverted =
            handle_request(&mut st, Method::RevertProfile, json!({"id":"default"}), 2).unwrap();
        assert_eq!(reverted["id"], "default");
        let shown = handle_request(&mut st, Method::ShowWindow, json!({}), 3).unwrap();
        assert_eq!(shown, json!({}));
        let quit = handle_request(&mut st, Method::QuitDaemon, json!({}), 4).unwrap();
        assert_eq!(quit["stopping"], true);
    }

    #[test]
    fn submit_record_without_session_is_not_found() {
        let mut st = state();
        handle_request(&mut st, Method::ApplyProfile, json!({"id":"default"}), 0).unwrap();
        let err = handle_request(
            &mut st,
            Method::SubmitRecord,
            json!({"key":"a","modifiers":[]}),
            1,
        )
        .unwrap_err();
        assert_eq!(err, ErrorCode::NotFound);
    }

    #[test]
    fn submit_record_persists_user_binding() {
        let mut st = state();
        handle_request(&mut st, Method::ApplyProfile, json!({"id":"default"}), 0).unwrap();
        handle_request(
            &mut st,
            Method::StartRecord,
            json!({"key_id":"kp01","timeout_ms":8000}),
            0,
        )
        .unwrap();
        handle_request(
            &mut st,
            Method::SubmitRecord,
            json!({"key":"c","modifiers":["ctrl"]}),
            1,
        )
        .unwrap();
        let profile = read_user_profile(&st.paths, "default").unwrap();
        assert!(matches!(
            profile.bindings.get(&KeyId::Kp01),
            Some(Action::Key { key: KeyToken::C, modifiers }) if modifiers == &[Modifier::Ctrl]
        ));
    }

    #[test]
    fn next_profile_steps_in_list_order_and_wraps() {
        let mut st = state();
        handle_request(&mut st, Method::ApplyProfile, json!({"id": "default"}), 0).unwrap();
        let second = SHIPPED_IDS[1];
        assert_eq!(
            switch_profile(&mut st, &ProfileSwitch::Next).unwrap(),
            second
        );
        assert_eq!(st.active_id.as_deref(), Some(second));

        let last = SHIPPED_IDS[SHIPPED_IDS.len() - 1];
        handle_request(&mut st, Method::ApplyProfile, json!({"id": last}), 0).unwrap();
        switch_profile(&mut st, &ProfileSwitch::Next).unwrap();
        assert_eq!(
            st.active_id.as_deref(),
            Some("default"),
            "wraps to the first"
        );
    }

    #[test]
    fn next_profile_reaches_custom_profiles_after_the_shipped_ones() {
        let mut st = state();
        let last = SHIPPED_IDS[SHIPPED_IDS.len() - 1];
        handle_request(&mut st, Method::ApplyProfile, json!({"id": last}), 0).unwrap();
        handle_request(&mut st, Method::CreateProfile, json!({"name": "Mine"}), 0).unwrap();
        handle_request(&mut st, Method::ApplyProfile, json!({"id": last}), 0).unwrap();
        switch_profile(&mut st, &ProfileSwitch::Next).unwrap();
        assert_eq!(st.active_id.as_deref(), Some("mine"));
        switch_profile(&mut st, &ProfileSwitch::Next).unwrap();
        assert_eq!(st.active_id.as_deref(), Some("default"));
    }

    #[test]
    fn switch_to_a_named_profile_activates_it_and_an_unknown_one_is_refused() {
        let mut st = state();
        assert_eq!(
            switch_profile(&mut st, &ProfileSwitch::To("dota-2".into())).unwrap(),
            "dota-2"
        );
        assert_eq!(st.active_id.as_deref(), Some("dota-2"));
        assert_eq!(
            switch_profile(&mut st, &ProfileSwitch::To("nope".into())).unwrap_err(),
            ErrorCode::NotFound
        );
        assert_eq!(
            st.active_id.as_deref(),
            Some("dota-2"),
            "a failed switch changes nothing"
        );
    }

    #[test]
    fn with_nothing_applied_next_starts_at_the_first_profile() {
        let mut st = state();
        switch_profile(&mut st, &ProfileSwitch::Next).unwrap();
        assert_eq!(st.active_id.as_deref(), Some(SHIPPED_IDS[0]));
    }

    #[test]
    fn applying_a_shipped_profile_marks_the_copy_and_editing_it_clears_the_mark() {
        let mut st = state();
        handle_request(&mut st, Method::ApplyProfile, json!({"id": "dota-2"}), 0).unwrap();
        let fresh = read_user_profile(&st.paths, "dota-2").unwrap();
        assert_eq!(
            fresh.shipped_revision.as_deref(),
            Some(
                opentartarus_core::store::content_revision(
                    shipped_json("dota-2").unwrap().as_bytes()
                )
                .as_str()
            ),
            "an untouched copy tracks the pack, so a better layout can reach it"
        );

        handle_request(
            &mut st,
            Method::SetBinding,
            json!({"profile_id":"dota-2","key_id":"kp01","action":{"type":"key","key":"c","modifiers":[]}}),
            1,
        )
        .unwrap();
        let edited = read_user_profile(&st.paths, "dota-2").unwrap();
        assert_eq!(
            edited.shipped_revision, None,
            "an edited profile must never be replaced by a pack update"
        );

        handle_request(&mut st, Method::RevertProfile, json!({"id": "dota-2"}), 2).unwrap();
        let reverted = read_user_profile(&st.paths, "dota-2").unwrap();
        assert!(
            reverted.shipped_revision.is_some(),
            "reverting puts the copy back under the pack's care"
        );
    }

    #[test]
    fn a_custom_profile_never_follows_the_pack() {
        let mut st = state();
        handle_request(&mut st, Method::ApplyProfile, json!({"id": "dota-2"}), 0).unwrap();
        let id = create(&mut st, "My Own");
        let mine = read_user_profile(&st.paths, &id).unwrap();
        assert_eq!(
            mine.shipped_revision, None,
            "a copy made by the user is theirs, not the pack's"
        );
    }
}
