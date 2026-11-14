//! Ready-made frames for the custom lighting effect. Each preset is drawn
//! from a few colours; `LightPreset::frame` turns that into a colour per key
//! on the V2 Pro's grid.

use crate::lighting::{FRAME_COLUMNS, FRAME_ROWS, HOME_ROW};
use crate::types::LightFrame;
use serde::{Deserialize, Serialize};

/// Longest name a saved preset can have, in characters.
pub const SAVED_NAME_MAX_LEN: usize = 32;

/// A frame the user saved under a name of their own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPreset {
    pub name: String,
    pub frame: LightFrame,
}

impl SavedPreset {
    pub fn is_valid(&self) -> bool {
        let name = self.name.trim();
        !name.is_empty()
            && name.chars().count() <= SAVED_NAME_MAX_LEN
            && self.frame.len() <= FRAME_ROWS
            && self.frame.iter().all(|row| row.len() <= FRAME_COLUMNS)
    }
}

/// `name` tidied for a saved preset: trimmed, and cut to the length limit.
/// `None` if nothing is left.
pub fn saved_name(name: &str) -> Option<String> {
    let name: String = name.trim().chars().take(SAVED_NAME_MAX_LEN).collect();
    let name = name.trim_end().to_string();
    (!name.is_empty()).then_some(name)
}

/// `presets` with `preset` added. A preset of the same name, ignoring case,
/// is replaced in place, so saving again under a name updates it.
pub fn with_saved(mut presets: Vec<SavedPreset>, preset: SavedPreset) -> Vec<SavedPreset> {
    let key = preset.name.to_lowercase();
    match presets.iter_mut().find(|p| p.name.to_lowercase() == key) {
        Some(existing) => *existing = preset,
        None => presets.push(preset),
    }
    presets
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetGroup {
    WorldOfWarcraft,
    Games,
    Aesthetic,
    Functional,
}

impl PresetGroup {
    pub const ALL: [PresetGroup; 4] = [
        PresetGroup::WorldOfWarcraft,
        PresetGroup::Games,
        PresetGroup::Aesthetic,
        PresetGroup::Functional,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::WorldOfWarcraft => "World of Warcraft",
            Self::Games => "Games",
            Self::Aesthetic => "Aesthetic",
            Self::Functional => "Functional",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    /// One colour per row, top to bottom.
    Rows([[u8; 3]; FRAME_ROWS]),
    /// Blends from the top-left key to the bottom-right one through the stops.
    Diagonal(&'static [[u8; 3]]),
    /// Blends from the left column to the right one through the stops.
    Columns(&'static [[u8; 3]]),
    /// Colours keys by what the layout rules put on them: `[home, near, far,
    /// thumb, rest]`. Home is keys 11 to 15; near is the keys straight above
    /// and below it (6 to 10, 16 to 19, 24); far is the top row and the 22/23
    /// column; thumb is the bottom row.
    Roles([[u8; 3]; 5]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub group: PresetGroup,
    pub fill: Fill,
}

/// Keys per row on the V2 Pro, top to bottom.
const ROW_KEYS: [usize; FRAME_ROWS] = [6, 6, 6, 5, 4];
const UNLIT: [u8; 3] = [0, 0, 0];

impl LightPreset {
    pub fn frame(&self) -> LightFrame {
        (0..FRAME_ROWS)
            .map(|row| {
                (0..FRAME_COLUMNS)
                    .map(|column| {
                        if column >= ROW_KEYS[row] {
                            UNLIT
                        } else {
                            self.cell(row, column)
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// One colour per row, for small previews: each row's first key.
    pub fn row_colors(&self) -> [[u8; 3]; FRAME_ROWS] {
        let mut rows = [UNLIT; FRAME_ROWS];
        for (row, color) in rows.iter_mut().enumerate() {
            *color = self.cell(row, 0);
        }
        rows
    }

    fn cell(&self, row: usize, column: usize) -> [u8; 3] {
        match self.fill {
            Fill::Rows(rows) => rows[row],
            Fill::Diagonal(stops) => {
                let span = (FRAME_ROWS - 1 + FRAME_COLUMNS - 1) as f32;
                blend(stops, (row + column) as f32 / span)
            }
            Fill::Columns(stops) => blend(stops, column as f32 / (FRAME_COLUMNS - 1) as f32),
            Fill::Roles([home, near, far, thumb, rest]) => match role(row, column) {
                Role::Home => home,
                Role::Near => near,
                Role::Far => far,
                Role::Thumb => thumb,
                Role::Rest => rest,
            },
        }
    }
}

enum Role {
    Home,
    Near,
    Far,
    Thumb,
    Rest,
}

fn role(row: usize, column: usize) -> Role {
    const RIGHT_COLUMN: usize = FRAME_COLUMNS - 1;
    match (row, column) {
        (0, _) | (1..=2, RIGHT_COLUMN) => Role::Far,
        (HOME_ROW, _) => Role::Home,
        (1 | 3, _) => Role::Near,
        (4, _) => Role::Thumb,
        _ => Role::Rest,
    }
}

/// The colour `t` of the way along evenly spaced stops, `t` in 0..=1.
fn blend(stops: &[[u8; 3]], t: f32) -> [u8; 3] {
    match stops {
        [] => UNLIT,
        [only] => *only,
        _ => {
            let scaled = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
            let index = (scaled.floor() as usize).min(stops.len() - 2);
            let local = scaled - index as f32;
            let (a, b) = (stops[index], stops[index + 1]);
            let mut out = UNLIT;
            for channel in 0..3 {
                let mixed =
                    f32::from(a[channel]) + (f32::from(b[channel]) - f32::from(a[channel])) * local;
                out[channel] = mixed.round().clamp(0.0, 255.0) as u8;
            }
            out
        }
    }
}

const fn rgb(hex: u32) -> [u8; 3] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8]
}

const fn rows(hex: [u32; FRAME_ROWS]) -> Fill {
    Fill::Rows([
        rgb(hex[0]),
        rgb(hex[1]),
        rgb(hex[2]),
        rgb(hex[3]),
        rgb(hex[4]),
    ])
}

const fn preset(
    id: &'static str,
    name: &'static str,
    group: PresetGroup,
    fill: Fill,
) -> LightPreset {
    LightPreset {
        id,
        name,
        group,
        fill,
    }
}

use PresetGroup::{Aesthetic, Functional, Games, WorldOfWarcraft};

/// Every class gets five different colours, top row to bottom, with the
/// class colour on the rest row.
pub const PRESETS: &[LightPreset] = &[
    // World of Warcraft: the thirteen classes, then the factions.
    preset(
        "wow-death-knight",
        "Death Knight",
        WorldOfWarcraft,
        // Frost, rime, the class colour, blood, unholy.
        rows([0x40a0ff, 0xb0e8ff, 0xc41e3a, 0x700010, 0x30ff40]),
    ),
    preset(
        "wow-demon-hunter",
        "Demon Hunter",
        WorldOfWarcraft,
        // Fel fire fading into the void.
        rows([0x30ff30, 0xb0ff00, 0xa330c9, 0x5a00d0, 0x200060]),
    ),
    preset(
        "wow-druid",
        "Druid",
        WorldOfWarcraft,
        // Lunar and solar eclipse, the class colour, the wilds.
        rows([0x6070ff, 0xffd000, 0xff7c0a, 0x30c030, 0x00906a]),
    ),
    preset(
        "wow-evoker",
        "Evoker",
        WorldOfWarcraft,
        // Azure and bronze above, the class colour, red and green below.
        rows([0x3070ff, 0xffb020, 0x33937f, 0xff3020, 0x50ff30]),
    ),
    preset(
        "wow-hunter",
        "Hunter",
        WorldOfWarcraft,
        // Open sky down to the forest floor.
        rows([0x80c8ff, 0xffb000, 0xaad372, 0x40a020, 0xa04000]),
    ),
    preset(
        "wow-mage",
        "Mage",
        WorldOfWarcraft,
        // Arcane, the class colour, frost, fire.
        rows([0x8030ff, 0xff50e0, 0x3fc7eb, 0xd0f4ff, 0xff5000]),
    ),
    preset(
        "wow-monk",
        "Monk",
        WorldOfWarcraft,
        // Gold and mist, jade, a brew.
        rows([0xffd040, 0x80ffc0, 0x00ff98, 0x00a060, 0xff9000]),
    ),
    preset(
        "wow-paladin",
        "Paladin",
        WorldOfWarcraft,
        // The Light, the class colour, retribution's fire.
        rows([0xfff4d0, 0xffc020, 0xf48cba, 0xff3060, 0xff8000]),
    ),
    preset(
        "wow-priest",
        "Priest",
        WorldOfWarcraft,
        // Holy above the class colour, shadow and void below.
        rows([0xffd060, 0xfff2c0, 0xffffff, 0x9040ff, 0x3a00b0]),
    ),
    preset(
        "wow-rogue",
        "Rogue",
        WorldOfWarcraft,
        // Shadow, poison, the class colour, gold, blood.
        rows([0x6020ff, 0x40ff20, 0xfff468, 0xff9000, 0xff2020]),
    ),
    preset(
        "wow-shaman",
        "Shaman",
        WorldOfWarcraft,
        // Air, lightning, water in the class colour, earth, fire.
        rows([0xc0f0ff, 0x8060ff, 0x0070dd, 0xd07000, 0xff3000]),
    ),
    preset(
        "wow-warlock",
        "Warlock",
        WorldOfWarcraft,
        // Hellfire, chaos, the class colour, shadow, fel.
        rows([0xff3000, 0xd000a0, 0x8788ee, 0x7000ff, 0x40ff00]),
    ),
    preset(
        "wow-warrior",
        "Warrior",
        WorldOfWarcraft,
        // Steel, bronze, the class colour, rage.
        rows([0xd8e0e8, 0xffb060, 0xc69b6d, 0xff2000, 0x800000]),
    ),
    preset(
        "wow-horde",
        "Horde",
        WorldOfWarcraft,
        rows([0xffb000, 0xff2000, 0xc00000, 0x800000, 0x300000]),
    ),
    preset(
        "wow-alliance",
        "Alliance",
        WorldOfWarcraft,
        rows([0xffd040, 0x4090ff, 0x1060ff, 0x0030c0, 0x001880]),
    ),
    // Games, one look for each shipped profile's game.
    preset(
        "game-hellfire",
        "Hellfire",
        Games,
        rows([0xffa000, 0xff6000, 0xff2000, 0xc00000, 0x600000]),
    ),
    preset(
        "game-grace",
        "Grace",
        Games,
        Fill::Diagonal(&[rgb(0xffe9a0), rgb(0xffb000), rgb(0xff7a00)]),
    ),
    preset(
        "game-overworld",
        "Overworld",
        Games,
        // Sky, grass, dirt, stone, lava.
        rows([0x60b8ff, 0x40d020, 0x9a5020, 0x808080, 0xff4000]),
    ),
    preset(
        "game-radiant-dire",
        "Radiant & Dire",
        Games,
        Fill::Diagonal(&[rgb(0x20ff50), rgb(0x202020), rgb(0xff2010)]),
    ),
    preset(
        "game-ct-t",
        "CT & T",
        Games,
        rows([0x5aa0ff, 0x3070e0, 0xa0a0a0, 0xe08020, 0xffb040]),
    ),
    preset(
        "game-payload",
        "Payload",
        Games,
        rows([0xff9c00, 0xffb84d, 0xffffff, 0x7fc8ff, 0x2f9cff]),
    ),
    preset(
        "game-wraeclast",
        "Wraeclast",
        Games,
        rows([0xffd27a, 0xe0a030, 0xc03010, 0x700000, 0x400008]),
    ),
    preset(
        "game-crystal",
        "Crystal",
        Games,
        Fill::Diagonal(&[rgb(0xffffff), rgb(0x80d8ff), rgb(0x2060ff), rgb(0x8040ff)]),
    ),
    preset(
        "game-patrons",
        "Amber & Sapphire",
        Games,
        Fill::Diagonal(&[rgb(0xffa020), rgb(0x2050ff)]),
    ),
    preset(
        "game-tyria",
        "Tyria",
        Games,
        rows([0xffffff, 0xff9090, 0xff3030, 0xc00000, 0x600000]),
    ),
    preset(
        "game-tamriel",
        "Tamriel",
        Games,
        rows([0xfff0c0, 0xe0b060, 0xc08030, 0x806020, 0x403010]),
    ),
    // Aesthetic: gradients that only have to look good.
    preset(
        "sunset",
        "Sunset",
        Aesthetic,
        rows([0xffd000, 0xff8a00, 0xff3d5a, 0xc02080, 0x5020a0]),
    ),
    preset(
        "synthwave",
        "Synthwave",
        Aesthetic,
        Fill::Diagonal(&[rgb(0xff00c8), rgb(0x8000ff), rgb(0x00e5ff)]),
    ),
    preset(
        "ocean",
        "Ocean",
        Aesthetic,
        rows([0xa0fff0, 0x00e0e0, 0x00a0ff, 0x0050e0, 0x001890]),
    ),
    preset(
        "aurora",
        "Aurora",
        Aesthetic,
        Fill::Diagonal(&[rgb(0x20ff80), rgb(0x00d0c0), rgb(0x4060ff), rgb(0xb040ff)]),
    ),
    preset(
        "sakura",
        "Sakura",
        Aesthetic,
        rows([0xffffff, 0xffd0e8, 0xff9ccc, 0xff60a8, 0xd02070]),
    ),
    preset(
        "ember",
        "Ember",
        Aesthetic,
        Fill::Diagonal(&[rgb(0xffd060), rgb(0xff7000), rgb(0xff1000), rgb(0x600000)]),
    ),
    preset(
        "glacier",
        "Glacier",
        Aesthetic,
        rows([0xffffff, 0xd8f4ff, 0x90dcff, 0x40a8ff, 0x1060e0]),
    ),
    preset(
        "forest",
        "Forest",
        Aesthetic,
        rows([0xd0ff60, 0x80e040, 0x30b030, 0x108040, 0x004020]),
    ),
    preset(
        "twilight",
        "Twilight",
        Aesthetic,
        Fill::Columns(&[rgb(0xff6080), rgb(0xa040ff), rgb(0x2040ff)]),
    ),
    preset(
        "prism",
        "Prism",
        Aesthetic,
        Fill::Diagonal(&[
            rgb(0xff0000),
            rgb(0xff8000),
            rgb(0xffff00),
            rgb(0x00ff00),
            rgb(0x0080ff),
            rgb(0x8000ff),
        ]),
    ),
    // Functional: light follows what the layout rules put on each key.
    preset(
        "roles",
        "Key roles",
        Functional,
        // Home green, near yellow, menus purple, thumb row orange.
        Fill::Roles([
            rgb(0x30ff60),
            rgb(0xffc000),
            rgb(0x8040ff),
            rgb(0xff6000),
            UNLIT,
        ]),
    ),
    preset(
        "focus",
        "Focus",
        Functional,
        // The keys a hand reaches without moving stand out, the rest stay dim.
        Fill::Roles([
            rgb(0xffffff),
            rgb(0x4090ff),
            rgb(0x101830),
            rgb(0x203060),
            UNLIT,
        ]),
    ),
    preset(
        "home-row",
        "Home row only",
        Functional,
        // Find the rest row by eye, everything else off.
        Fill::Roles([rgb(0xffffff), UNLIT, UNLIT, UNLIT, UNLIT]),
    ),
    preset(
        "night",
        "Night",
        Functional,
        // Dim red, kind to eyes in a dark room.
        rows([0x300000, 0x500000, 0x901000, 0x500000, 0x300000]),
    ),
];

pub fn preset_by_id(id: &str) -> Option<&'static LightPreset> {
    PRESETS.iter().find(|preset| preset.id == id)
}

/// The preset that draws exactly `frame`, if any.
pub fn preset_for_frame(frame: &LightFrame) -> Option<&'static LightPreset> {
    PRESETS.iter().find(|preset| preset.frame() == *frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WOW_CLASSES: [&str; 13] = [
        "Death Knight",
        "Demon Hunter",
        "Druid",
        "Evoker",
        "Hunter",
        "Mage",
        "Monk",
        "Paladin",
        "Priest",
        "Rogue",
        "Shaman",
        "Warlock",
        "Warrior",
    ];

    #[test]
    fn ids_and_names_are_unique() {
        for (i, a) in PRESETS.iter().enumerate() {
            for b in &PRESETS[i + 1..] {
                assert_ne!(a.id, b.id);
                assert_ne!(a.name, b.name);
            }
        }
    }

    #[test]
    fn every_wow_class_has_a_preset_with_five_different_rows() {
        for class in WOW_CLASSES {
            let preset = PRESETS
                .iter()
                .find(|p| p.name == class && p.group == PresetGroup::WorldOfWarcraft)
                .unwrap_or_else(|| panic!("no preset for {class}"));
            let rows = preset.row_colors();
            for (i, a) in rows.iter().enumerate() {
                for b in &rows[i + 1..] {
                    assert_ne!(a, b, "{class} repeats a row colour");
                }
            }
        }
    }

    #[test]
    fn every_group_has_presets() {
        for group in PresetGroup::ALL {
            assert!(PRESETS.iter().any(|p| p.group == group), "{}", group.name());
        }
    }

    #[test]
    fn frames_fit_the_grid_and_leave_cells_past_a_row_dark() {
        for preset in PRESETS {
            let frame = preset.frame();
            assert_eq!(frame.len(), FRAME_ROWS);
            for (row, cells) in frame.iter().enumerate() {
                assert_eq!(cells.len(), FRAME_COLUMNS);
                assert!(cells[ROW_KEYS[row]..].iter().all(|c| *c == UNLIT));
            }
        }
    }

    #[test]
    fn every_preset_lights_something() {
        for preset in PRESETS {
            assert!(
                preset.frame().iter().flatten().any(|c| *c != UNLIT),
                "{} is all dark",
                preset.id
            );
        }
    }

    #[test]
    fn blend_hits_the_end_stops() {
        let stops = [rgb(0x000000), rgb(0xff8000)];
        assert_eq!(blend(&stops, 0.0), [0, 0, 0]);
        assert_eq!(blend(&stops, 1.0), [0xff, 0x80, 0]);
        assert_eq!(blend(&stops, 0.5), [128, 64, 0]);
    }

    #[test]
    fn diagonal_runs_from_top_left_to_bottom_right() {
        let preset = preset_by_id("synthwave").unwrap();
        let frame = preset.frame();
        assert_eq!(frame[0][0], rgb(0xff00c8));
        assert_ne!(frame[0][0], frame[3][4]);
    }

    #[test]
    fn roles_follow_the_layout_rules() {
        let frame = preset_by_id("roles").unwrap().frame();
        let home = rgb(0x30ff60);
        let near = rgb(0xffc000);
        let far = rgb(0x8040ff);
        assert!(frame[HOME_ROW][..5].iter().all(|c| *c == home));
        assert_eq!(frame[1][5], far, "key 22 is a menu key");
        assert_eq!(frame[HOME_ROW][5], far, "key 23 is a menu key");
        assert!(frame[0].iter().all(|c| *c == far));
        assert!(frame[1][..5].iter().all(|c| *c == near));
        assert!(frame[3][..5].iter().all(|c| *c == near));
        assert!(frame[4][..4].iter().all(|c| *c == rgb(0xff6000)));
    }

    #[test]
    fn saved_names_are_trimmed_cut_and_never_empty() {
        assert_eq!(saved_name("  Raid night "), Some("Raid night".into()));
        assert_eq!(saved_name("   "), None);
        let long = "x".repeat(SAVED_NAME_MAX_LEN + 10);
        assert_eq!(saved_name(&long).unwrap().len(), SAVED_NAME_MAX_LEN);
    }

    #[test]
    fn saving_under_an_existing_name_replaces_it() {
        let a = SavedPreset {
            name: "Raid".into(),
            frame: vec![vec![[1, 1, 1]]],
        };
        let b = SavedPreset {
            name: "Arena".into(),
            frame: vec![vec![[2, 2, 2]]],
        };
        let a2 = SavedPreset {
            name: "raid".into(),
            frame: vec![vec![[3, 3, 3]]],
        };
        let list = with_saved(with_saved(Vec::new(), a), b.clone());
        let list = with_saved(list, a2.clone());
        assert_eq!(list, vec![a2, b]);
    }

    #[test]
    fn saved_presets_must_fit_the_grid_and_have_a_name() {
        let ok = SavedPreset {
            name: "Mine".into(),
            frame: preset_by_id("ocean").unwrap().frame(),
        };
        assert!(ok.is_valid());
        let unnamed = SavedPreset {
            name: " ".into(),
            ..ok.clone()
        };
        assert!(!unnamed.is_valid());
        let too_wide = SavedPreset {
            frame: vec![vec![[0, 0, 0]; FRAME_COLUMNS + 1]],
            ..ok
        };
        assert!(!too_wide.is_valid());
    }

    #[test]
    fn a_preset_frame_is_recognised() {
        let preset = preset_by_id("wow-mage").unwrap();
        assert_eq!(preset_for_frame(&preset.frame()), Some(preset));
        assert_eq!(preset_for_frame(&vec![vec![[1, 2, 3]]]), None);
    }
}
