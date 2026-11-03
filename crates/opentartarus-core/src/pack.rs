use crate::error::ErrorCode;
use crate::types::Profile;

pub const SHIPPED_IDS: [&str; 13] = [
    "default",
    "dota-2",
    "world-of-warcraft",
    "final-fantasy-xiv",
    "path-of-exile",
    "overwatch-2",
    "counter-strike-2",
    "diablo-4",
    "elden-ring",
    "guild-wars-2",
    "elder-scrolls-online",
    "minecraft",
    "deadlock",
];

pub fn shipped_json(id: &str) -> Option<&'static str> {
    match id {
        "default" => Some(include_str!("../profiles/default.json")),
        "dota-2" => Some(include_str!("../profiles/dota-2.json")),
        "world-of-warcraft" => Some(include_str!("../profiles/world-of-warcraft.json")),
        "final-fantasy-xiv" => Some(include_str!("../profiles/final-fantasy-xiv.json")),
        "path-of-exile" => Some(include_str!("../profiles/path-of-exile.json")),
        "overwatch-2" => Some(include_str!("../profiles/overwatch-2.json")),
        "counter-strike-2" => Some(include_str!("../profiles/counter-strike-2.json")),
        "diablo-4" => Some(include_str!("../profiles/diablo-4.json")),
        "elden-ring" => Some(include_str!("../profiles/elden-ring.json")),
        "guild-wars-2" => Some(include_str!("../profiles/guild-wars-2.json")),
        "elder-scrolls-online" => Some(include_str!("../profiles/elder-scrolls-online.json")),
        "minecraft" => Some(include_str!("../profiles/minecraft.json")),
        "deadlock" => Some(include_str!("../profiles/deadlock.json")),
        _ => None,
    }
}

/// Whether an id names a profile from the shipped pack. Shipped profiles can
/// be reverted but never deleted; custom ones are the reverse.
pub fn is_shipped(id: &str) -> bool {
    SHIPPED_IDS.contains(&id)
}

pub fn shipped_profile(id: &str) -> Result<Profile, ErrorCode> {
    let raw = shipped_json(id).ok_or(ErrorCode::NotFound)?;
    let p: Profile = serde_json::from_str(raw).map_err(|_| ErrorCode::InvalidProfile)?;
    p.validate()?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Action, DeviceModel, GameId, KeyId, KeyToken, Profile};
    use std::collections::BTreeMap;

    fn key_action(token: KeyToken) -> Action {
        Action::Key {
            key: token,
            modifiers: vec![],
        }
    }

    fn assert_key_bindings(profile: &Profile, expected: &[(KeyId, KeyToken)]) {
        assert_eq!(profile.bindings.len(), expected.len());
        for (key_id, token) in expected {
            assert_eq!(
                profile.bindings.get(key_id),
                Some(&key_action(*token)),
                "binding {key_id:?}"
            );
        }
    }

    const WASD_THUMBS: [(KeyId, KeyToken); 4] = [
        (KeyId::ThumbN, KeyToken::W),
        (KeyId::ThumbW, KeyToken::A),
        (KeyId::ThumbS, KeyToken::S),
        (KeyId::ThumbE, KeyToken::D),
    ];

    const NEW_GAME_IDS: [&str; 8] = [
        "overwatch-2",
        "counter-strike-2",
        "diablo-4",
        "elden-ring",
        "guild-wars-2",
        "elder-scrolls-online",
        "minecraft",
        "deadlock",
    ];

    #[test]
    fn is_shipped_knows_the_pack_and_nothing_else() {
        for id in SHIPPED_IDS {
            assert!(is_shipped(id), "{id} ships with the app");
        }
        assert!(!is_shipped("my-raid-layout"));
        assert!(!is_shipped(""));
        // Case matters: ids are lowercase slugs.
        assert!(!is_shipped("Default"));
    }

    #[test]
    fn a_custom_game_round_trips_through_json_as_custom() {
        let json = serde_json::to_string(&GameId::Custom).unwrap();
        assert_eq!(json, "\"custom\"");
        let back: GameId = serde_json::from_str("\"custom\"").unwrap();
        assert_eq!(back, GameId::Custom);
    }

    #[test]
    fn shipped_ids_are_pack_order() {
        assert_eq!(SHIPPED_IDS.len(), 13);
        assert_eq!(SHIPPED_IDS[0], "default");
        assert_eq!(
            &SHIPPED_IDS[1..5],
            &[
                "dota-2",
                "world-of-warcraft",
                "final-fantasy-xiv",
                "path-of-exile"
            ]
        );
        assert_eq!(&SHIPPED_IDS[5..], &NEW_GAME_IDS);
    }

    #[test]
    fn every_shipped_profile_validates() {
        for id in SHIPPED_IDS {
            let p = shipped_profile(id).unwrap();
            p.validate().unwrap();
            assert_eq!(p.id, id);
        }
    }

    // Razer's own layout: the left of a keyboard, arrows on the thumb pad.
    // Every game's defaults work with it and nothing needs setting up.
    #[test]
    fn default_layout_bindings_match_spec() {
        let p = shipped_profile("default").unwrap();
        assert_eq!(p.game, GameId::Default);
        assert_key_bindings(
            &p,
            &[
                (KeyId::Kp01, KeyToken::Num1),
                (KeyId::Kp02, KeyToken::Num2),
                (KeyId::Kp03, KeyToken::Num3),
                (KeyId::Kp04, KeyToken::Num4),
                (KeyId::Kp05, KeyToken::Num5),
                (KeyId::Kp06, KeyToken::Tab),
                (KeyId::Kp07, KeyToken::Q),
                (KeyId::Kp08, KeyToken::W),
                (KeyId::Kp09, KeyToken::E),
                (KeyId::Kp10, KeyToken::R),
                (KeyId::Kp11, KeyToken::CapsLock),
                (KeyId::Kp12, KeyToken::A),
                (KeyId::Kp13, KeyToken::S),
                (KeyId::Kp14, KeyToken::D),
                (KeyId::Kp15, KeyToken::F),
                (KeyId::Kp16, KeyToken::LeftShift),
                (KeyId::Kp17, KeyToken::Z),
                (KeyId::Kp18, KeyToken::X),
                (KeyId::Kp19, KeyToken::C),
                (KeyId::Kp20, KeyToken::Space),
                (KeyId::ThumbN, KeyToken::Up),
                (KeyId::ThumbW, KeyToken::Left),
                (KeyId::ThumbS, KeyToken::Down),
                (KeyId::ThumbE, KeyToken::Right),
            ],
        );
    }

    #[test]
    fn dota_bindings_match_spec() {
        let p = shipped_profile("dota-2").unwrap();
        assert_eq!(p.game, GameId::Dota2);
        assert_eq!(p.lighting.color, Some([200, 40, 40]));
        assert_key_bindings(
            &p,
            &[
                (KeyId::Kp01, KeyToken::Q),
                (KeyId::Kp02, KeyToken::W),
                (KeyId::Kp03, KeyToken::E),
                (KeyId::Kp04, KeyToken::R),
                (KeyId::Kp05, KeyToken::D),
                (KeyId::Kp06, KeyToken::F),
                (KeyId::Kp07, KeyToken::Z),
                (KeyId::Kp08, KeyToken::X),
                (KeyId::Kp09, KeyToken::C),
                (KeyId::Kp10, KeyToken::V),
                (KeyId::Kp11, KeyToken::B),
                (KeyId::Kp12, KeyToken::N),
                (KeyId::Kp13, KeyToken::F4),
                (KeyId::Kp14, KeyToken::A),
                (KeyId::Kp15, KeyToken::S),
                (KeyId::Kp16, KeyToken::Grave),
                (KeyId::Kp17, KeyToken::F2),
                (KeyId::Kp18, KeyToken::F3),
                (KeyId::Kp19, KeyToken::Tab),
                (KeyId::Kp20, KeyToken::F1),
            ],
        );
    }

    #[test]
    fn wow_bindings_match_spec() {
        let p = shipped_profile("world-of-warcraft").unwrap();
        assert_eq!(p.game, GameId::WorldOfWarcraft);
        assert_eq!(p.lighting.color, Some([255, 180, 0]));
        let mut expected = vec![
            (KeyId::Kp01, KeyToken::Num1),
            (KeyId::Kp02, KeyToken::Num2),
            (KeyId::Kp03, KeyToken::Num3),
            (KeyId::Kp04, KeyToken::Num4),
            (KeyId::Kp05, KeyToken::Num5),
            (KeyId::Kp06, KeyToken::Num6),
            (KeyId::Kp07, KeyToken::Num7),
            (KeyId::Kp08, KeyToken::Num8),
            (KeyId::Kp09, KeyToken::Num9),
            (KeyId::Kp10, KeyToken::Num0),
            (KeyId::Kp11, KeyToken::Minus),
            (KeyId::Kp12, KeyToken::Equal),
            (KeyId::Kp13, KeyToken::Tab),
            (KeyId::Kp14, KeyToken::F1),
            (KeyId::Kp15, KeyToken::F),
            (KeyId::Kp16, KeyToken::M),
            (KeyId::Kp17, KeyToken::B),
            (KeyId::Kp18, KeyToken::C),
            (KeyId::Kp19, KeyToken::Escape),
            (KeyId::Kp20, KeyToken::Space),
        ];
        expected.extend(WASD_THUMBS);
        assert_key_bindings(&p, &expected);
    }

    #[test]
    fn ffxiv_bindings_match_spec() {
        let p = shipped_profile("final-fantasy-xiv").unwrap();
        assert_eq!(p.game, GameId::FinalFantasyXiv);
        assert_eq!(p.lighting.color, Some([80, 160, 255]));
        let mut expected = vec![
            (KeyId::Kp01, KeyToken::Num1),
            (KeyId::Kp02, KeyToken::Num2),
            (KeyId::Kp03, KeyToken::Num3),
            (KeyId::Kp04, KeyToken::Num4),
            (KeyId::Kp05, KeyToken::Num5),
            (KeyId::Kp06, KeyToken::Num6),
            (KeyId::Kp07, KeyToken::Num7),
            (KeyId::Kp08, KeyToken::Num8),
            (KeyId::Kp09, KeyToken::Num9),
            (KeyId::Kp10, KeyToken::Num0),
            (KeyId::Kp11, KeyToken::Minus),
            (KeyId::Kp12, KeyToken::Equal),
            (KeyId::Kp13, KeyToken::Tab),
            (KeyId::Kp14, KeyToken::F),
            (KeyId::Kp15, KeyToken::R),
            (KeyId::Kp16, KeyToken::M),
            (KeyId::Kp17, KeyToken::I),
            (KeyId::Kp18, KeyToken::J),
            (KeyId::Kp19, KeyToken::C),
            (KeyId::Kp20, KeyToken::Space),
        ];
        expected.extend(WASD_THUMBS);
        assert_key_bindings(&p, &expected);
    }

    #[test]
    fn poe_bindings_match_spec() {
        let p = shipped_profile("path-of-exile").unwrap();
        assert_eq!(p.game, GameId::PathOfExile);
        assert_eq!(p.lighting.color, Some([140, 0, 0]));
        assert_key_bindings(
            &p,
            &[
                (KeyId::Kp01, KeyToken::Num1),
                (KeyId::Kp02, KeyToken::Num2),
                (KeyId::Kp03, KeyToken::Num3),
                (KeyId::Kp04, KeyToken::Num4),
                (KeyId::Kp05, KeyToken::Num5),
                (KeyId::Kp06, KeyToken::Q),
                (KeyId::Kp07, KeyToken::W),
                (KeyId::Kp08, KeyToken::E),
                (KeyId::Kp09, KeyToken::R),
                (KeyId::Kp10, KeyToken::T),
                (KeyId::Kp11, KeyToken::Tab),
                (KeyId::Kp12, KeyToken::X),
                (KeyId::Kp13, KeyToken::D),
                (KeyId::Kp14, KeyToken::Z),
                (KeyId::Kp15, KeyToken::G),
                (KeyId::Kp16, KeyToken::Enter),
                (KeyId::Kp17, KeyToken::I),
                (KeyId::Kp18, KeyToken::C),
                (KeyId::Kp19, KeyToken::P),
                (KeyId::Kp20, KeyToken::Space),
            ],
        );
    }

    #[test]
    fn overwatch_bindings_match_spec() {
        let p = shipped_profile("overwatch-2").unwrap();
        assert_eq!(p.game, GameId::Overwatch2);
        assert_eq!(p.lighting.color, Some([255, 107, 53]));
        let mut expected = vec![
            (KeyId::Kp01, KeyToken::Num1),
            (KeyId::Kp02, KeyToken::Num2),
            (KeyId::Kp03, KeyToken::H),
            (KeyId::Kp04, KeyToken::O),
            (KeyId::Kp05, KeyToken::Tab),
            (KeyId::Kp06, KeyToken::Q),
            (KeyId::Kp07, KeyToken::E),
            (KeyId::Kp08, KeyToken::X),
            (KeyId::Kp09, KeyToken::R),
            (KeyId::Kp10, KeyToken::V),
            (KeyId::Kp11, KeyToken::F),
            (KeyId::Kp12, KeyToken::LeftShift),
            (KeyId::Kp13, KeyToken::LeftCtrl),
            (KeyId::Kp14, KeyToken::C),
            (KeyId::Kp15, KeyToken::Z),
            (KeyId::Kp16, KeyToken::Enter),
            (KeyId::Kp17, KeyToken::Escape),
            (KeyId::Kp18, KeyToken::F1),
            (KeyId::Kp19, KeyToken::G),
            (KeyId::Kp20, KeyToken::Space),
        ];
        expected.extend(WASD_THUMBS);
        assert_key_bindings(&p, &expected);
    }

    #[test]
    fn counter_strike_bindings_match_spec() {
        let p = shipped_profile("counter-strike-2").unwrap();
        assert_eq!(p.game, GameId::CounterStrike2);
        assert_eq!(p.lighting.color, Some([222, 155, 53]));
        let mut expected = vec![
            (KeyId::Kp01, KeyToken::B),
            (KeyId::Kp02, KeyToken::Tab),
            (KeyId::Kp03, KeyToken::Y),
            (KeyId::Kp04, KeyToken::Z),
            (KeyId::Kp05, KeyToken::F),
            (KeyId::Kp06, KeyToken::Num1),
            (KeyId::Kp07, KeyToken::Num2),
            (KeyId::Kp08, KeyToken::Num3),
            (KeyId::Kp09, KeyToken::Num4),
            (KeyId::Kp10, KeyToken::Num5),
            (KeyId::Kp11, KeyToken::Q),
            (KeyId::Kp12, KeyToken::E),
            (KeyId::Kp13, KeyToken::R),
            (KeyId::Kp14, KeyToken::G),
            (KeyId::Kp15, KeyToken::LeftCtrl),
            (KeyId::Kp16, KeyToken::LeftShift),
            (KeyId::Kp17, KeyToken::Escape),
            (KeyId::Kp18, KeyToken::M),
            (KeyId::Kp19, KeyToken::C),
            (KeyId::Kp20, KeyToken::Space),
        ];
        expected.extend(WASD_THUMBS);
        assert_key_bindings(&p, &expected);
    }

    #[test]
    fn diablo_bindings_match_spec() {
        let p = shipped_profile("diablo-4").unwrap();
        assert_eq!(p.game, GameId::Diablo4);
        assert_eq!(p.lighting.color, Some([164, 32, 26]));
        let mut expected = vec![
            (KeyId::Kp01, KeyToken::C),
            (KeyId::Kp02, KeyToken::J),
            (KeyId::Kp03, KeyToken::O),
            (KeyId::Kp04, KeyToken::P),
            (KeyId::Kp05, KeyToken::Y),
            (KeyId::Kp06, KeyToken::E),
            (KeyId::Kp07, KeyToken::F),
            (KeyId::Kp08, KeyToken::Z),
            (KeyId::Kp09, KeyToken::T),
            (KeyId::Kp10, KeyToken::Tab),
            (KeyId::Kp11, KeyToken::Num1),
            (KeyId::Kp12, KeyToken::Num2),
            (KeyId::Kp13, KeyToken::Num3),
            (KeyId::Kp14, KeyToken::Num4),
            (KeyId::Kp15, KeyToken::Q),
            (KeyId::Kp16, KeyToken::Enter),
            (KeyId::Kp17, KeyToken::Escape),
            (KeyId::Kp20, KeyToken::Space),
        ];
        expected.extend(WASD_THUMBS);
        assert_key_bindings(&p, &expected);
    }

    #[test]
    fn elden_ring_bindings_match_spec() {
        let p = shipped_profile("elden-ring").unwrap();
        assert_eq!(p.game, GameId::EldenRing);
        assert_eq!(p.lighting.color, Some([201, 168, 96]));
        let mut expected = vec![
            (KeyId::Kp05, KeyToken::G),
            (KeyId::Kp06, KeyToken::Up),
            (KeyId::Kp07, KeyToken::Down),
            (KeyId::Kp08, KeyToken::Left),
            (KeyId::Kp09, KeyToken::Right),
            (KeyId::Kp11, KeyToken::E),
            (KeyId::Kp12, KeyToken::R),
            (KeyId::Kp13, KeyToken::Q),
            (KeyId::Kp14, KeyToken::F),
            (KeyId::Kp15, KeyToken::X),
            (KeyId::Kp17, KeyToken::Escape),
            (KeyId::Kp20, KeyToken::Space),
        ];
        expected.extend(WASD_THUMBS);
        assert_key_bindings(&p, &expected);
    }

    #[test]
    fn guild_wars_bindings_match_spec() {
        let p = shipped_profile("guild-wars-2").unwrap();
        assert_eq!(p.game, GameId::GuildWars2);
        assert_eq!(p.lighting.color, Some([190, 30, 45]));
        let mut expected = vec![
            (KeyId::Kp01, KeyToken::Num1),
            (KeyId::Kp02, KeyToken::Num2),
            (KeyId::Kp03, KeyToken::Num3),
            (KeyId::Kp04, KeyToken::Num4),
            (KeyId::Kp05, KeyToken::Num5),
            (KeyId::Kp06, KeyToken::Num6),
            (KeyId::Kp07, KeyToken::Num7),
            (KeyId::Kp08, KeyToken::Num8),
            (KeyId::Kp09, KeyToken::Num9),
            (KeyId::Kp10, KeyToken::Num0),
            (KeyId::Kp11, KeyToken::F1),
            (KeyId::Kp12, KeyToken::F2),
            (KeyId::Kp13, KeyToken::F3),
            (KeyId::Kp14, KeyToken::F4),
            (KeyId::Kp15, KeyToken::V),
            (KeyId::Kp16, KeyToken::Grave),
            (KeyId::Kp17, KeyToken::X),
            (KeyId::Kp18, KeyToken::F),
            (KeyId::Kp19, KeyToken::F5),
            (KeyId::Kp20, KeyToken::Space),
        ];
        expected.extend(WASD_THUMBS);
        assert_key_bindings(&p, &expected);
    }

    #[test]
    fn eso_bindings_match_spec() {
        let p = shipped_profile("elder-scrolls-online").unwrap();
        assert_eq!(p.game, GameId::ElderScrollsOnline);
        assert_eq!(p.lighting.color, Some([45, 130, 90]));
        let mut expected = vec![
            (KeyId::Kp01, KeyToken::Num1),
            (KeyId::Kp02, KeyToken::Num2),
            (KeyId::Kp03, KeyToken::Num3),
            (KeyId::Kp04, KeyToken::Num4),
            (KeyId::Kp05, KeyToken::Num5),
            (KeyId::Kp06, KeyToken::R),
            (KeyId::Kp07, KeyToken::Q),
            (KeyId::Kp08, KeyToken::E),
            (KeyId::Kp09, KeyToken::Grave),
            (KeyId::Kp10, KeyToken::H),
            (KeyId::Kp11, KeyToken::X),
            (KeyId::Kp12, KeyToken::LeftShift),
            (KeyId::Kp13, KeyToken::I),
            (KeyId::Kp14, KeyToken::M),
            (KeyId::Kp15, KeyToken::C),
            (KeyId::Kp16, KeyToken::Enter),
            (KeyId::Kp17, KeyToken::Escape),
            (KeyId::Kp18, KeyToken::K),
            (KeyId::Kp20, KeyToken::Space),
            (KeyId::Kp19, KeyToken::LeftCtrl),
        ];
        expected.extend(WASD_THUMBS);
        assert_key_bindings(&p, &expected);
    }

    #[test]
    fn minecraft_bindings_match_spec() {
        let p = shipped_profile("minecraft").unwrap();
        assert_eq!(p.game, GameId::Minecraft);
        assert_eq!(p.lighting.color, Some([94, 168, 64]));
        let mut expected = vec![
            (KeyId::Kp01, KeyToken::Num1),
            (KeyId::Kp02, KeyToken::Num2),
            (KeyId::Kp03, KeyToken::Num3),
            (KeyId::Kp04, KeyToken::Num4),
            (KeyId::Kp05, KeyToken::Num5),
            (KeyId::Kp06, KeyToken::Num6),
            (KeyId::Kp07, KeyToken::Num7),
            (KeyId::Kp08, KeyToken::Num8),
            (KeyId::Kp09, KeyToken::Num9),
            (KeyId::Kp10, KeyToken::E),
            (KeyId::Kp11, KeyToken::Q),
            (KeyId::Kp12, KeyToken::F),
            (KeyId::Kp13, KeyToken::T),
            (KeyId::Kp14, KeyToken::Tab),
            (KeyId::Kp15, KeyToken::Escape),
            (KeyId::Kp16, KeyToken::LeftShift),
            (KeyId::Kp17, KeyToken::LeftCtrl),
            (KeyId::Kp18, KeyToken::F3),
            (KeyId::Kp19, KeyToken::F5),
            (KeyId::Kp20, KeyToken::Space),
        ];
        expected.extend(WASD_THUMBS);
        assert_key_bindings(&p, &expected);
    }

    #[test]
    fn a_setup_note_exists_only_where_the_game_needs_an_in_game_step() {
        // Diablo needs a preset switched on; Elden Ring changed its layout
        // in a patch. Every other profile works with the game's defaults.
        let with_note = ["diablo-4", "elden-ring"];
        for id in SHIPPED_IDS {
            let p = shipped_profile(id).unwrap();
            assert_eq!(
                p.setup_note.is_some(),
                with_note.contains(&id),
                "{id}: setup note presence"
            );
        }
        let noted = shipped_profile(with_note[0]).unwrap();
        let json = serde_json::to_value(&noted).unwrap();
        assert!(json.get("setup_note").is_some(), "a note survives a save");
        let plain = shipped_profile("dota-2").unwrap();
        let json = serde_json::to_value(&plain).unwrap();
        assert!(
            json.get("setup_note").is_none(),
            "no note, no field written"
        );
    }

    #[test]
    fn every_new_profile_puts_movement_on_the_thumb_pad() {
        for id in NEW_GAME_IDS {
            let p = shipped_profile(id).unwrap();
            for (key_id, token) in WASD_THUMBS {
                assert_eq!(
                    p.bindings.get(&key_id),
                    Some(&Action::Key {
                        key: token,
                        modifiers: vec![]
                    }),
                    "{id} is missing {key_id:?}"
                );
            }
        }
    }

    #[test]
    fn no_new_profile_binds_a_diagonal_or_the_analog_stick() {
        for id in NEW_GAME_IDS {
            let p = shipped_profile(id).unwrap();
            for key_id in [
                KeyId::ThumbNe,
                KeyId::ThumbSe,
                KeyId::ThumbSw,
                KeyId::ThumbNw,
                KeyId::AnalogUp,
                KeyId::AnalogDown,
                KeyId::AnalogLeft,
                KeyId::AnalogRight,
            ] {
                assert!(
                    !p.bindings.contains_key(&key_id),
                    "{id} must leave {key_id:?} free"
                );
            }
        }
    }

    /// The V2 Pro's home row, under the four fingers at rest, and the rows
    /// just above and below it. With small hands these are the keys that can
    /// be hit without moving the hand.
    const V2_PRO_HOME: [KeyId; 4] = [KeyId::Kp11, KeyId::Kp12, KeyId::Kp13, KeyId::Kp14];
    const V2_PRO_NEAR: [KeyId; 8] = [
        KeyId::Kp06,
        KeyId::Kp07,
        KeyId::Kp08,
        KeyId::Kp09,
        KeyId::Kp16,
        KeyId::Kp17,
        KeyId::Kp18,
        KeyId::Kp19,
    ];
    /// The top row and the far right column: a stretch for a small hand.
    const V2_PRO_FAR: [KeyId; 8] = [
        KeyId::Kp21,
        KeyId::Kp01,
        KeyId::Kp02,
        KeyId::Kp03,
        KeyId::Kp04,
        KeyId::Kp05,
        KeyId::Kp22,
        KeyId::Kp23,
    ];

    fn v2_pro_layout(id: &str) -> BTreeMap<KeyId, Action> {
        shipped_profile(id)
            .unwrap()
            .model_bindings
            .get(&DeviceModel::V2Pro)
            .cloned()
            .unwrap_or_else(|| panic!("{id} has no V2 Pro layout"))
    }

    fn token(action: Option<&Action>) -> Option<KeyToken> {
        match action {
            Some(Action::Key { key, modifiers }) if modifiers.is_empty() => Some(*key),
            _ => None,
        }
    }

    #[test]
    fn every_shipped_profile_has_a_v2_pro_layout_that_fits_the_pad() {
        for id in SHIPPED_IDS {
            let p = shipped_profile(id).unwrap();
            assert_eq!(p.device_models, DeviceModel::ALL.to_vec(), "{id}");
            assert_eq!(
                p.model_bindings.keys().collect::<Vec<_>>(),
                vec![&DeviceModel::V2Pro],
                "{id}: only the V2 Pro has a layout of its own"
            );
            for (key_id, action) in v2_pro_layout(id) {
                assert!(DeviceModel::V2Pro.has_key(key_id), "{id} {key_id:?}");
                assert!(
                    token(Some(&action)).is_some(),
                    "{id} {key_id:?} must be a plain key, found {action:?}"
                );
            }
        }
    }

    #[test]
    fn v2_pro_layouts_keep_busy_keys_under_a_small_hand() {
        // Menus and maps may go to the far keys; nothing a fight needs.
        let menus = [
            KeyToken::Escape,
            KeyToken::Grave,
            KeyToken::M,
            KeyToken::C,
            KeyToken::P,
            KeyToken::F4,
            // Minecraft's upper hotbar, which the scroll wheel reaches too.
            KeyToken::Num6,
            KeyToken::Num7,
            KeyToken::Num8,
            KeyToken::Num9,
        ];
        for id in SHIPPED_IDS.into_iter().filter(|id| *id != "default") {
            let layout = v2_pro_layout(id);
            for key_id in V2_PRO_HOME.iter().chain(&V2_PRO_NEAR) {
                assert!(layout.contains_key(key_id), "{id} leaves {key_id:?} empty");
            }
            for key_id in V2_PRO_FAR {
                if let Some(key) = token(layout.get(&key_id)) {
                    assert!(
                        menus.contains(&key),
                        "{id} puts {key:?} on the far key {key_id:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn v2_pro_moves_with_the_stick_where_the_game_moves_with_wasd() {
        let stick = [
            (KeyId::AnalogUp, KeyToken::W),
            (KeyId::AnalogLeft, KeyToken::A),
            (KeyId::AnalogDown, KeyToken::S),
            (KeyId::AnalogRight, KeyToken::D),
        ];
        for id in SHIPPED_IDS {
            let p = shipped_profile(id).unwrap();
            let layout = v2_pro_layout(id);
            let moves_with_wasd = token(p.bindings.get(&KeyId::ThumbN)) == Some(KeyToken::W);
            for (key_id, key) in stick {
                if moves_with_wasd {
                    assert_eq!(token(layout.get(&key_id)), Some(key), "{id} {key_id:?}");
                } else {
                    assert_ne!(token(layout.get(&key_id)), Some(key), "{id} {key_id:?}");
                }
            }
        }
        // Click to move: the stick stays free for the game.
        assert!(!v2_pro_layout("path-of-exile").keys().any(|k| k.is_stick()));
        // Dota pans the camera with the arrows.
        assert_eq!(
            token(v2_pro_layout("dota-2").get(&KeyId::AnalogUp)),
            Some(KeyToken::Up)
        );
    }

    #[test]
    fn deadlock_bindings_match_spec() {
        let p = shipped_profile("deadlock").unwrap();
        assert_eq!(p.game, GameId::Deadlock);
        assert_eq!(p.setup_note, None, "works with the game's default keys");
        let mut expected = vec![
            (KeyId::Kp01, KeyToken::Escape),
            (KeyId::Kp05, KeyToken::B),
            (KeyId::Kp06, KeyToken::Tab),
            (KeyId::Kp07, KeyToken::Z),
            (KeyId::Kp08, KeyToken::X),
            (KeyId::Kp09, KeyToken::C),
            (KeyId::Kp10, KeyToken::V),
            (KeyId::Kp11, KeyToken::LeftShift),
            (KeyId::Kp12, KeyToken::Num1),
            (KeyId::Kp13, KeyToken::Num2),
            (KeyId::Kp14, KeyToken::Num3),
            (KeyId::Kp15, KeyToken::Num4),
            (KeyId::Kp16, KeyToken::LeftCtrl),
            (KeyId::Kp17, KeyToken::Q),
            (KeyId::Kp18, KeyToken::F),
            (KeyId::Kp19, KeyToken::R),
            (KeyId::Kp20, KeyToken::Space),
        ];
        expected.extend(WASD_THUMBS);
        assert_key_bindings(&p, &expected);

        let v2_pro = v2_pro_layout("deadlock");
        for (key_id, key) in [
            // Dash and the four abilities on the home row.
            (KeyId::Kp11, KeyToken::LeftShift),
            (KeyId::Kp12, KeyToken::Num1),
            (KeyId::Kp13, KeyToken::Num2),
            (KeyId::Kp14, KeyToken::Num3),
            (KeyId::Kp15, KeyToken::Num4),
            // Items above, melee, parry and reload below.
            (KeyId::Kp07, KeyToken::Z),
            (KeyId::Kp08, KeyToken::X),
            (KeyId::Kp09, KeyToken::C),
            (KeyId::Kp10, KeyToken::V),
            (KeyId::Kp17, KeyToken::Q),
            (KeyId::Kp18, KeyToken::F),
            (KeyId::Kp19, KeyToken::R),
            // Slide by pressing the stick while moving.
            (KeyId::StickClick, KeyToken::LeftCtrl),
            (KeyId::Kp20, KeyToken::Space),
        ] {
            assert_eq!(token(v2_pro.get(&key_id)), Some(key), "{key_id:?}");
        }
    }

    #[test]
    fn no_shipped_profile_automates_input() {
        // Most publishers allow rebinding but ban "one press, many actions":
        // macros and auto-repeat. What ships must never cross that line, so
        // a player who only uses the stock profiles is always inside the
        // rules. A player who builds a macro does so knowingly, and the
        // window warns them.
        for id in SHIPPED_IDS {
            let p = shipped_profile(id).unwrap();
            for (key_id, action) in &p.bindings {
                assert!(
                    matches!(
                        action,
                        Action::Key { .. }
                            | Action::Mouse { .. }
                            | Action::SwitchProfile { .. }
                            | Action::NextProfile
                    ),
                    "{id} {key_id:?} sends more than one action per press: {action:?}"
                );
            }
        }
    }

    #[test]
    fn every_new_binding_is_a_plain_key_on_every_model() {
        for id in NEW_GAME_IDS {
            let p = shipped_profile(id).unwrap();
            assert_eq!(
                p.device_models,
                DeviceModel::ALL.to_vec(),
                "{id} must support every pad"
            );
            assert!(p.lighting.color.is_some(), "{id} needs a sidebar colour");
            for (key_id, action) in &p.bindings {
                match action {
                    Action::Key { modifiers, .. } => assert!(
                        modifiers.is_empty(),
                        "{id} {key_id:?} must not carry a modifier"
                    ),
                    other => panic!("{id} {key_id:?} must be a plain key, found {other:?}"),
                }
            }
        }
    }
}
