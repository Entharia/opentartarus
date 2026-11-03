use crate::constants::{
    HOLD_REPEAT_RATE_MAX_MS, HOLD_REPEAT_RATE_MIN_MS, MACRO_MAX_DELAY_MS, MACRO_MAX_STEPS,
};
use crate::error::ErrorCode;
use crate::types::{
    Action, DeviceModel, KeyId, Lighting, LightingEffect, MacroKind, MacroStep, Modifier, Profile,
};
use std::collections::BTreeMap;

const PROFILE_ID_MAX_LEN: usize = 64;
const PROFILE_NAME_MAX_LEN: usize = 64;
/// A setup note is one short instruction, not a manual.
const SETUP_NOTE_MAX_LEN: usize = 240;
const BRIGHTNESS_MAX: u8 = 100;
const MODIFIERS_MAX: usize = 4;

impl Profile {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.unknown_key_ids {
            return Err(ErrorCode::InvalidProfile);
        }
        if !valid_id(&self.id) {
            return Err(ErrorCode::InvalidProfile);
        }
        if !valid_name(&self.name) {
            return Err(ErrorCode::InvalidProfile);
        }
        validate_device_models(&self.device_models)?;
        validate_lighting(&self.lighting)?;
        validate_setup_note(self.setup_note.as_deref())?;
        for action in self.bindings.values() {
            validate_action(action)?;
        }
        for (model, layout) in &self.model_bindings {
            // A model's own layout is only for a pad the profile supports,
            // and only names keys that pad has.
            if !self.device_models.contains(model) {
                return Err(ErrorCode::InvalidProfile);
            }
            for (key, action) in layout {
                if !model.has_key(*key) {
                    return Err(ErrorCode::UnknownKey);
                }
                validate_action(action)?;
            }
        }
        Ok(())
    }

    /// The layout used on `model`: its own one if the profile has it,
    /// otherwise the shared `bindings`. With no model known, the shared one.
    pub fn layout(&self, model: Option<DeviceModel>) -> &BTreeMap<KeyId, Action> {
        model
            .and_then(|m| self.model_bindings.get(&m))
            .unwrap_or(&self.bindings)
    }

    /// The layout an edit made on `model` changes, chosen the same way as
    /// `layout`, so an edit lands in the layout the pad is using.
    pub fn layout_mut(&mut self, model: Option<DeviceModel>) -> &mut BTreeMap<KeyId, Action> {
        match model.filter(|m| self.model_bindings.contains_key(m)) {
            Some(m) => self.model_bindings.entry(m).or_default(),
            None => &mut self.bindings,
        }
    }

    pub fn bindings_for_model(&self, model: DeviceModel) -> BTreeMap<KeyId, Action> {
        self.layout(Some(model))
            .iter()
            .filter(|(key, _)| model.has_key(**key))
            .map(|(key, action)| (*key, action.clone()))
            .collect()
    }
}

/// Joins the words of a slug. Also what a name's runs of punctuation and
/// whitespace collapse to, so `My  Raid / Layout` becomes `my-raid-layout`.
const SLUG_SEPARATOR: char = '-';
/// What a name that has no usable characters at all becomes, so a profile
/// named `!!!` still gets a valid id rather than an empty one.
const SLUG_FALLBACK: &str = "profile";

/// Turns a display name into a profile id that `valid_id` accepts and that
/// none of `taken` already uses.
///
/// Lowercase ASCII letters and digits pass through; everything else collapses
/// to one separator. A leading separator is dropped because an id must start
/// with a letter or digit. A collision gets a numeric suffix: `layout`,
/// `layout-2`, `layout-3`. The result is trimmed to the id length limit before
/// the suffix is added, so the suffix is never what pushes it over.
pub fn profile_id_for_name(name: &str, taken: &[String]) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut pending_separator = false;
    for ch in name.chars() {
        let lowered = ch.to_ascii_lowercase();
        if lowered.is_ascii_lowercase() || lowered.is_ascii_digit() {
            if pending_separator && !slug.is_empty() {
                slug.push(SLUG_SEPARATOR);
            }
            pending_separator = false;
            slug.push(lowered);
        } else {
            pending_separator = true;
        }
    }
    if slug.is_empty() {
        slug.push_str(SLUG_FALLBACK);
    }

    // Leave room for the widest suffix this loop could ever need before the
    // limit, so a long name plus `-99` still fits.
    let suffix_room = SLUG_SEPARATOR.len_utf8() + SLUG_MAX_SUFFIX_DIGITS;
    let base_max = PROFILE_ID_MAX_LEN.saturating_sub(suffix_room);
    let base: String = slug.chars().take(base_max).collect();
    let base = base.trim_end_matches(SLUG_SEPARATOR).to_owned();

    if !taken.contains(&base) {
        return base;
    }
    let mut counter: u32 = SLUG_FIRST_SUFFIX;
    loop {
        let candidate = format!("{base}{SLUG_SEPARATOR}{counter}");
        if !taken.contains(&candidate) {
            return candidate;
        }
        counter += 1;
    }
}

/// The second copy of a name is `-2`, never `-1`: the first has no suffix.
const SLUG_FIRST_SUFFIX: u32 = 2;
/// Digits reserved for the collision suffix when trimming the base.
const SLUG_MAX_SUFFIX_DIGITS: usize = 2;

fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || bytes.len() > PROFILE_ID_MAX_LEN {
        return false;
    }
    let first = bytes[0];
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

fn valid_name(name: &str) -> bool {
    let count = name.trim().chars().count();
    (1..=PROFILE_NAME_MAX_LEN).contains(&count)
}

fn validate_setup_note(note: Option<&str>) -> Result<(), ErrorCode> {
    match note {
        None => Ok(()),
        Some(text) => {
            let count = text.trim().chars().count();
            if (1..=SETUP_NOTE_MAX_LEN).contains(&count) {
                Ok(())
            } else {
                Err(ErrorCode::InvalidProfile)
            }
        }
    }
}

fn validate_device_models(models: &[DeviceModel]) -> Result<(), ErrorCode> {
    if models.is_empty() {
        return Err(ErrorCode::InvalidProfile);
    }
    let mut seen = Vec::with_capacity(models.len());
    for model in models {
        if seen.contains(model) {
            return Err(ErrorCode::InvalidProfile);
        }
        seen.push(*model);
    }
    Ok(())
}

fn validate_lighting(lighting: &Lighting) -> Result<(), ErrorCode> {
    if lighting.brightness > BRIGHTNESS_MAX {
        return Err(ErrorCode::InvalidProfile);
    }
    if lighting.effect == LightingEffect::Static && lighting.color.is_none() {
        return Err(ErrorCode::InvalidProfile);
    }
    Ok(())
}

fn validate_modifiers(modifiers: &[Modifier]) -> Result<(), ErrorCode> {
    if modifiers.len() > MODIFIERS_MAX {
        return Err(ErrorCode::InvalidProfile);
    }
    let mut seen = Vec::with_capacity(modifiers.len());
    for modifier in modifiers {
        if seen.contains(modifier) {
            return Err(ErrorCode::InvalidProfile);
        }
        seen.push(*modifier);
    }
    Ok(())
}

fn validate_macro(steps: &[MacroStep]) -> Result<(), ErrorCode> {
    if steps.is_empty() || steps.len() > MACRO_MAX_STEPS {
        return Err(ErrorCode::InvalidProfile);
    }
    for step in steps {
        if step.delay_ms > MACRO_MAX_DELAY_MS {
            return Err(ErrorCode::InvalidProfile);
        }
        match &step.kind {
            MacroKind::Key { modifiers, .. } => validate_modifiers(modifiers)?,
            MacroKind::Mouse { .. } => {}
        }
    }
    Ok(())
}

fn validate_action(action: &Action) -> Result<(), ErrorCode> {
    match action {
        Action::Key { modifiers, .. } => validate_modifiers(modifiers),
        Action::Macro { steps } => validate_macro(steps),
        Action::Mouse { .. } => Ok(()),
        Action::SwitchProfile { profile } => {
            if valid_id(profile) {
                Ok(())
            } else {
                Err(ErrorCode::InvalidProfile)
            }
        }
        Action::NextProfile => Ok(()),
        Action::HoldRepeat { inner, rate_ms } => {
            if *rate_ms < HOLD_REPEAT_RATE_MIN_MS || *rate_ms > HOLD_REPEAT_RATE_MAX_MS {
                return Err(ErrorCode::InvalidProfile);
            }
            match inner.as_ref() {
                Action::Key { modifiers, .. } => validate_modifiers(modifiers),
                _ => Err(ErrorCode::InvalidProfile),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lol_json() -> &'static str {
        r#"{
          "id": "league-of-legends",
          "name": "League of Legends",
          "game": "league-of-legends",
          "device_models": ["v2", "pro"],
          "bindings": {
            "kp01": { "type": "key", "key": "q", "modifiers": [] }
          },
          "lighting": { "effect": "static", "brightness": 80, "color": [0, 180, 255] }
        }"#
    }

    fn taken(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn a_plain_name_slugs_to_lowercase_hyphens() {
        assert_eq!(profile_id_for_name("My Raid Layout", &[]), "my-raid-layout");
    }

    #[test]
    fn punctuation_and_repeated_spaces_collapse_to_one_separator() {
        assert_eq!(
            profile_id_for_name("  My  Raid / Layout!! ", &[]),
            "my-raid-layout"
        );
    }

    #[test]
    fn a_leading_symbol_does_not_produce_a_leading_separator() {
        // An id must start with a letter or digit.
        assert_eq!(profile_id_for_name("#1 Setup", &[]), "1-setup");
    }

    #[test]
    fn a_name_with_nothing_usable_still_gets_a_valid_id() {
        assert_eq!(profile_id_for_name("!!!", &[]), SLUG_FALLBACK);
        assert!(valid_id(&profile_id_for_name("!!!", &[])));
    }

    #[test]
    fn a_collision_gets_a_numbered_suffix_starting_at_two() {
        assert_eq!(
            profile_id_for_name("Layout", &taken(&["layout"])),
            "layout-2"
        );
        assert_eq!(
            profile_id_for_name("Layout", &taken(&["layout", "layout-2"])),
            "layout-3"
        );
    }

    #[test]
    fn a_suffix_never_pushes_an_id_over_the_length_limit() {
        let long = "x".repeat(PROFILE_ID_MAX_LEN * 2);
        let first = profile_id_for_name(&long, &[]);
        assert!(valid_id(&first), "trimmed base must be a valid id");
        let second = profile_id_for_name(&long, std::slice::from_ref(&first));
        assert!(
            valid_id(&second),
            "suffixed id must still be valid: {second}"
        );
        assert!(second.ends_with("-2"));
        assert!(second.len() <= PROFILE_ID_MAX_LEN);
    }

    #[test]
    fn every_generated_id_passes_the_same_rule_profiles_are_validated_by() {
        for name in [
            "Default",
            "Über Setup",
            "123",
            "a-b-c",
            "Trailing-",
            "-Leading",
        ] {
            let id = profile_id_for_name(name, &[]);
            assert!(valid_id(&id), "{name:?} produced invalid id {id:?}");
        }
    }

    #[test]
    fn accepts_league_of_legends_json() {
        let p: Profile = serde_json::from_str(lol_json()).unwrap();
        p.validate().unwrap();
        assert_eq!(p.id, "league-of-legends");
    }

    #[test]
    fn rejects_unknown_key_id() {
        let raw = lol_json().replace("kp01", "kp99");
        let p: Profile = serde_json::from_str(&raw).unwrap();
        assert_eq!(p.validate().unwrap_err(), ErrorCode::InvalidProfile);
    }

    #[test]
    fn rejects_bad_id_empty_name_brightness_and_hold_repeat_macro() {
        let mut p: Profile = serde_json::from_str(lol_json()).unwrap();
        p.id = "Nope".into();
        assert_eq!(p.validate().unwrap_err(), ErrorCode::InvalidProfile);
        p.id = "ok".into();
        p.name = "   ".into();
        assert_eq!(p.validate().unwrap_err(), ErrorCode::InvalidProfile);
        p.name = "Ok".into();
        p.lighting.brightness = 101;
        assert_eq!(p.validate().unwrap_err(), ErrorCode::InvalidProfile);
        p.lighting.brightness = 80;
        p.bindings.insert(
            KeyId::Kp01,
            Action::HoldRepeat {
                inner: Box::new(Action::Macro { steps: vec![] }),
                rate_ms: 40,
            },
        );
        assert_eq!(p.validate().unwrap_err(), ErrorCode::InvalidProfile);
    }

    #[test]
    fn a_switch_target_must_be_a_well_formed_id_and_next_needs_nothing() {
        let mut p: Profile = serde_json::from_str(lol_json()).unwrap();
        p.bindings.insert(
            KeyId::Kp01,
            Action::SwitchProfile {
                profile: "dota-2".into(),
            },
        );
        assert!(p.validate().is_ok());
        p.bindings.insert(
            KeyId::Kp01,
            Action::SwitchProfile {
                profile: "Dota 2".into(),
            },
        );
        assert_eq!(p.validate().unwrap_err(), ErrorCode::InvalidProfile);
        p.bindings.insert(KeyId::Kp01, Action::NextProfile);
        assert!(p.validate().is_ok());
        p.bindings.insert(
            KeyId::Kp01,
            Action::HoldRepeat {
                inner: Box::new(Action::NextProfile),
                rate_ms: 40,
            },
        );
        assert_eq!(
            p.validate().unwrap_err(),
            ErrorCode::InvalidProfile,
            "a switch cannot hold-repeat"
        );
    }

    #[test]
    fn a_setup_note_must_be_short_and_not_blank() {
        let mut p: Profile = serde_json::from_str(lol_json()).unwrap();
        assert_eq!(
            p.setup_note, None,
            "the league profile needs no in-game step"
        );
        p.setup_note = Some("Add Home as a second key for Crouch.".into());
        assert!(p.validate().is_ok());
        p.setup_note = Some("   ".into());
        assert_eq!(p.validate().unwrap_err(), ErrorCode::InvalidProfile);
        p.setup_note = Some("x".repeat(SETUP_NOTE_MAX_LEN + 1));
        assert_eq!(p.validate().unwrap_err(), ErrorCode::InvalidProfile);
    }

    #[test]
    fn a_model_layout_replaces_the_shared_one_on_that_model_only() {
        let mut p: Profile = serde_json::from_str(lol_json()).unwrap();
        p.device_models = DeviceModel::ALL.to_vec();
        let shared = p.bindings.clone();
        let mine = BTreeMap::from([(
            KeyId::StickClick,
            Action::Key {
                key: crate::types::KeyToken::Space,
                modifiers: vec![],
            },
        )]);
        p.model_bindings.insert(DeviceModel::V2Pro, mine.clone());
        assert!(p.validate().is_ok());
        assert_eq!(p.layout(Some(DeviceModel::V2Pro)), &mine);
        assert_eq!(p.bindings_for_model(DeviceModel::V2Pro), mine);
        assert_eq!(p.layout(Some(DeviceModel::V2)), &shared);
        assert_eq!(p.layout(None), &shared);

        p.layout_mut(Some(DeviceModel::V2Pro))
            .remove(&KeyId::StickClick);
        assert!(p.model_bindings[&DeviceModel::V2Pro].is_empty());
        assert_eq!(p.bindings, shared, "editing the V2 Pro leaves the rest");
        p.layout_mut(Some(DeviceModel::Pro)).remove(&KeyId::Kp01);
        assert!(!p.bindings.contains_key(&KeyId::Kp01));
        assert!(!p.model_bindings.contains_key(&DeviceModel::Pro));

        let json = serde_json::to_value(&p).unwrap();
        assert!(json["model_bindings"]["v2pro"].is_object());
        let back: Profile = serde_json::from_value(json).unwrap();
        assert_eq!(back, p, "a model layout survives a save");
        p.model_bindings.clear();
        let json = serde_json::to_value(&p).unwrap();
        assert!(json.get("model_bindings").is_none(), "none, no field");
    }

    #[test]
    fn a_model_layout_must_fit_its_pad() {
        let mut p: Profile = serde_json::from_str(lol_json()).unwrap();
        let space = Action::Key {
            key: crate::types::KeyToken::Space,
            modifiers: vec![],
        };
        p.model_bindings.insert(
            DeviceModel::V2Pro,
            BTreeMap::from([(KeyId::Kp21, space.clone())]),
        );
        assert_eq!(
            p.validate().unwrap_err(),
            ErrorCode::InvalidProfile,
            "a layout for a pad the profile does not support"
        );
        p.device_models.push(DeviceModel::V2Pro);
        assert!(p.validate().is_ok());
        p.model_bindings
            .get_mut(&DeviceModel::V2Pro)
            .unwrap()
            .insert(KeyId::ThumbN, space);
        assert_eq!(
            p.validate().unwrap_err(),
            ErrorCode::UnknownKey,
            "the V2 Pro has no thumb pad"
        );
        let raw = r#"{"id":"x","name":"x","game":"default","device_models":["v2pro"],
            "bindings":{},"model_bindings":{"v2pro":{"kp99":{"type":"key","key":"a","modifiers":[]}}},
            "lighting":{"effect":"none","brightness":80}}"#;
        let bad: Profile = serde_json::from_str(raw).unwrap();
        assert_eq!(bad.validate().unwrap_err(), ErrorCode::InvalidProfile);
    }

    #[test]
    fn v2_drops_analog_keeps_thumb_pro_drops_thumb_keeps_analog() {
        let mut p: Profile = serde_json::from_str(lol_json()).unwrap();
        p.bindings.insert(
            KeyId::AnalogUp,
            Action::Key {
                key: crate::types::KeyToken::W,
                modifiers: vec![],
            },
        );
        p.bindings.insert(
            KeyId::ThumbN,
            Action::Key {
                key: crate::types::KeyToken::W,
                modifiers: vec![],
            },
        );
        let v2 = p.bindings_for_model(DeviceModel::V2);
        assert!(v2.contains_key(&KeyId::ThumbN));
        assert!(!v2.contains_key(&KeyId::AnalogUp));
        let pro = p.bindings_for_model(DeviceModel::Pro);
        assert!(pro.contains_key(&KeyId::AnalogUp));
        assert!(!pro.contains_key(&KeyId::ThumbN));
    }

    #[test]
    fn error_sentences_match_spec() {
        assert_eq!(
            ErrorCode::Permission.user_message(),
            "OpenTartarus can’t talk to your keypad yet."
        );
        assert_eq!(
            ErrorCode::Disconnect.user_message(),
            "Tartarus disconnected."
        );
        assert_eq!(
            ErrorCode::Lighting.user_message(),
            "Lighting needs OpenRazer or OpenRGB.",
        );
        assert_eq!(
            ErrorCode::GrabConflict.user_message(),
            "Something else is using the Tartarus. Quit that app and reopen OpenTartarus."
        );
        assert_eq!(
            ErrorCode::InvalidProfile.user_message(),
            "That profile file is damaged. Revert to shipped or pick another."
        );
        assert_eq!(
            ErrorCode::UnknownKey.user_message(),
            "That key isn’t on this keypad."
        );
        assert_eq!(
            ErrorCode::RecordBusy.user_message(),
            "Already recording. Press a key or click Cancel."
        );
        assert_eq!(
            ErrorCode::NotFound.user_message(),
            "That profile isn’t installed."
        );
        assert_eq!(
            ErrorCode::Io.user_message(),
            "OpenTartarus couldn’t save. Try again."
        );
    }
}
