//! The enemies that roam the Zoid maps: which formations an area's map
//! Zoids stand for, and how the game picks them.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! rebuild of the object states (`0x08006E4C`), the formation pick
//! (`0x080328FC`) and the battle's side setup (`0x0802B728`); checked
//! against the object states and formations a reference emulator held on
//! the world map.
//!
//! Each area has a table of formations per map-Zoid column: the words at
//! ROM `0x6838C8 + area × 0x30 + column × 4` (areas past 10 use area 0)
//! point at twelve 36-byte formations. A formation names its leader in
//! its first two bytes and up to six members in the four-byte groups from
//! `+4`, each a group and a record of the 28-byte enemy records (`0xFF`
//! for none); the leader stands in slot 4. Its rarity class is at `+0x1C`.
//! A leader's record is at ROM `0x67894C`, a member's at `0x67664C`, both
//! `group × 0x380 + record × 0x1C`; a record's first byte is the sprite a
//! map Zoid leading it shows.
//!
//! A battle lost on the field takes the party to its area's return point
//! (`0x08006E08`, read against the warp a reference emulator made after a
//! loss).
//!
//! The story's battles (`0x08008D28`, set up by `0x080329E4`) take their
//! enemies from the 36-byte records of ROM `0x67C0F4` instead: six
//! four-byte groups from `+4`, each a flag and a record of the battle's own
//! six enemy records (ROM `0x67C6DC + battle × 0xA8`, `0xFF` for none), the
//! terrain of both sides at `+0x1C`, the song at `+0x20` and a mode at
//! `+0x21` (read by `0x0802B728`, `0x08033D6C`, `0x0802E814` and the
//! battle's controller; checked against a battle a reference emulator set
//! up as the first of them).

const FORMATION_TABLES: usize = 0x0068_38C8;
const AREA_TABLE_LEN: usize = 0x30;
const LAST_AREA: usize = 10;
const FORMATIONS_PER_COLUMN: usize = 12;
/// Bytes of a formation.
pub const FORMATION_LEN: usize = 0x24;
const RARITY: usize = 0x1C;
const LEADER_RECORDS: usize = 0x0067_894C;
const MEMBER_RECORDS: usize = 0x0067_664C;
const RECORD_GROUP_LEN: usize = 0x380;
/// Bytes of an enemy record.
pub const ENEMY_RECORD_LEN: usize = 0x1C;
/// Slots on a side of a battle.
pub const SLOTS: usize = 6;
/// The slot a formation's leader takes.
pub const LEADER_SLOT: usize = 4;
const MEMBERS: usize = 4;
const MEMBER_LEN: usize = 4;
const NO_RECORD: u8 = 0xFF;
const RETURN_POINTS: usize = 0x0032_8D6C;
const RETURN_POINT_LEN: usize = 8;
const RETURN_POINT_COUNT: usize = 23;
const ROM_BASE: u32 = 0x0800_0000;
const STORY_BATTLES: usize = 0x0067_C0F4;
const STORY_ENEMIES: usize = 0x0067_C6DC;
const STORY_ENEMIES_LEN: usize = SLOTS * ENEMY_RECORD_LEN;
/// Story battles the table holds.
pub const STORY_BATTLE_COUNT: usize = 42;
const STORY_TERRAIN: usize = 0x1C;
const STORY_SONG: usize = 0x20;
const STORY_MODE: usize = 0x21;
/// A slot's flag that makes its enemy one criticals never hit
/// (`0x0802B728` sets its trait `0x200`).
const GUARDED: u8 = 1;
/// The modes: no items in the actor's menu (`battle-menu` 0x1A), and the
/// party's first three part slots emptied when the fight starts
/// (`0x080337F0`).
const MODE_NO_ITEMS: u8 = 1;
const MODE_UNARMED: u8 = 2;
/// Rolls below this (of 100) pick a formation of class 0.
const COMMON_ROLLS: u16 = 60;
/// Rolls below this pick class 1, the rest class 2.
const UNCOMMON_ROLLS: u16 = 95;

/// A formation as the area's table holds it: 36 bytes, which the battle
/// reads again when it starts.
pub type Formation = [u8; FORMATION_LEN];

/// A story battle's record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoryBattle {
    /// Its number in the table.
    pub index: u8,
    record: [u8; FORMATION_LEN],
}

impl StoryBattle {
    /// The terrain both sides stand on.
    #[must_use]
    pub const fn terrain(&self) -> u8 {
        self.record[STORY_TERRAIN]
    }

    /// The song the fight plays.
    #[must_use]
    pub const fn song(&self) -> u8 {
        self.record[STORY_SONG]
    }

    /// Whether the actor's menu leaves the items out.
    #[must_use]
    pub const fn without_items(&self) -> bool {
        self.record[STORY_MODE] == MODE_NO_ITEMS
    }

    /// Whether the party fights without the parts of its first three
    /// slots.
    #[must_use]
    pub const fn unarmed(&self) -> bool {
        self.record[STORY_MODE] == MODE_UNARMED
    }
}

/// Story battle `index` (`0x080329E4` with bit 0 of the type), `None`
/// past the table or outside `rom`.
#[must_use]
pub fn story_battle(rom: &[u8], index: u8) -> Option<StoryBattle> {
    if usize::from(index) >= STORY_BATTLE_COUNT {
        return None;
    }
    let start = STORY_BATTLES + usize::from(index) * FORMATION_LEN;
    Some(StoryBattle {
        index,
        record: rom.get(start..start + FORMATION_LEN)?.try_into().ok()?,
    })
}

/// The enemies a battle brings: a roaming formation, or a story battle's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lineup {
    /// A formation a map Zoid stood for.
    Roaming(Formation),
    /// A story battle.
    Story(StoryBattle),
}

impl Lineup {
    /// The enemy record of slot `slot`, `None` for an empty slot or a
    /// record outside `rom` (`0x0802B728`).
    #[must_use]
    pub fn enemy_record(&self, rom: &[u8], slot: usize) -> Option<[u8; ENEMY_RECORD_LEN]> {
        match self {
            Self::Roaming(formation) => enemy_record(rom, formation, slot),
            Self::Story(battle) => {
                let at = MEMBERS + slot * MEMBER_LEN + 1;
                let record = *battle.record.get(at)?;
                if slot >= SLOTS || record == NO_RECORD {
                    return None;
                }
                let start = STORY_ENEMIES
                    + usize::from(battle.index) * STORY_ENEMIES_LEN
                    + usize::from(record) * ENEMY_RECORD_LEN;
                rom.get(start..start + ENEMY_RECORD_LEN)?.try_into().ok()
            }
        }
    }

    /// Whether the enemy of slot `slot` is one criticals never hit.
    #[must_use]
    pub fn guarded(&self, slot: usize) -> bool {
        match self {
            Self::Roaming(_) => false,
            Self::Story(battle) => battle.record.get(MEMBERS + slot * MEMBER_LEN) == Some(&GUARDED),
        }
    }

    /// The story battle, for one.
    #[must_use]
    pub const fn story(&self) -> Option<&StoryBattle> {
        match self {
            Self::Roaming(_) => None,
            Self::Story(battle) => Some(battle),
        }
    }
}

/// The twelve formations map Zoids of sprite column `column` may stand
/// for in area `area` (the map record id's low byte). `None` when the
/// table lies outside `rom`.
#[must_use]
pub fn formations(rom: &[u8], area: u8, column: u8) -> Option<Vec<Formation>> {
    let index = usize::from(area.wrapping_sub(1));
    let index = if index > LAST_AREA { 0 } else { index };
    let pointer = FORMATION_TABLES + index * AREA_TABLE_LEN + usize::from(column) * 4;
    let table = rom_pointer(rom, pointer)?;
    (0..FORMATIONS_PER_COLUMN)
        .map(|at| {
            let start = table + at * FORMATION_LEN;
            rom.get(start..start + FORMATION_LEN)
                .and_then(|bytes| bytes.try_into().ok())
        })
        .collect()
}

/// The rarity class a roll of 0–99 asks for: most rolls ask for class 0,
/// one in three for class 1 and one in twenty for class 2 (`0x080328FC`).
#[must_use]
pub const fn rarity_for(roll: u16) -> u8 {
    if roll < COMMON_ROLLS {
        0
    } else if roll < UNCOMMON_ROLLS {
        1
    } else {
        2
    }
}

/// The formations of `candidates` of class `class`, by index; the first
/// alone when none is (`0x080328FC`).
#[must_use]
pub fn of_class(candidates: &[Formation], class: u8) -> Vec<usize> {
    let found: Vec<usize> = candidates
        .iter()
        .enumerate()
        .filter(|(_, formation)| formation[RARITY] == class)
        .map(|(index, _)| index)
        .collect();
    if found.is_empty() { vec![0] } else { found }
}

/// The enemy record of slot `slot` of `formation`: the leader's for slot
/// 4, a member's otherwise; `None` for an empty slot or a record outside
/// `rom`.
#[must_use]
pub fn enemy_record(
    rom: &[u8],
    formation: &Formation,
    slot: usize,
) -> Option<[u8; ENEMY_RECORD_LEN]> {
    let (table, group, record) = if slot == LEADER_SLOT {
        (LEADER_RECORDS, formation[0], formation[1])
    } else {
        let at = MEMBERS + slot * MEMBER_LEN;
        (MEMBER_RECORDS, *formation.get(at)?, *formation.get(at + 1)?)
    };
    if record == NO_RECORD {
        return None;
    }
    let start =
        table + usize::from(group) * RECORD_GROUP_LEN + usize::from(record) * ENEMY_RECORD_LEN;
    rom.get(start..start + ENEMY_RECORD_LEN)?.try_into().ok()
}

/// The record the battle's spoils come from (`0x0803666C`): the leader's
/// for `None`, else member `member`'s of the four-byte groups from `+4`,
/// which the spoils read for all six slots.
#[must_use]
pub fn spoils_record(
    rom: &[u8],
    formation: &Formation,
    member: Option<usize>,
) -> Option<[u8; ENEMY_RECORD_LEN]> {
    let (table, group, record) = match member {
        None => (LEADER_RECORDS, formation[0], formation[1]),
        Some(member) => {
            let at = MEMBERS + member * MEMBER_LEN;
            (MEMBER_RECORDS, *formation.get(at)?, *formation.get(at + 1)?)
        }
    };
    let start =
        table + usize::from(group) * RECORD_GROUP_LEN + usize::from(record) * ENEMY_RECORD_LEN;
    rom.get(start..start + ENEMY_RECORD_LEN)?.try_into().ok()
}

/// The bit of a map record's id that keeps its objects' states, which the
/// rebuild draws the map Zoids' formations for (`0x08006E4C`).
const PERSISTENT: u16 = 0x8000;
/// The behavior of a map Zoid among a map's objects.
const MAP_ZOID: u16 = 1;
const RARITY_CLASSES: u8 = 3;

/// The Zi data the roaming enemies can leave, each once in order: for
/// every map Zoid of the maps that keep their objects, the formations its
/// column may stand for in the map's area that a rarity class can pick
/// (`0x080328FC`), and of each the Zi data of its leader's and its
/// members' records (`0x0803666C`).
#[must_use]
pub fn roaming_zi_data(rom: &[u8]) -> Vec<u8> {
    let mut found = Vec::new();
    for map in 0..crate::saga::MAP_COUNT {
        let Ok(record) = crate::saga::map_record(rom, map) else {
            continue;
        };
        if record.id & PERSISTENT == 0 {
            continue;
        }
        let Ok(objects) = crate::saga::map_objects(rom, map) else {
            continue;
        };
        let area = record.id.to_le_bytes()[0];
        for object in objects.iter().skip(1) {
            if object.behavior != MAP_ZOID {
                continue;
            }
            let column = object.sprite.to_le_bytes()[0];
            let Some(candidates) = formations(rom, area, column) else {
                continue;
            };
            for class in 0..RARITY_CLASSES {
                for index in of_class(&candidates, class) {
                    found.extend(formation_zi_data(rom, &candidates[index]));
                }
            }
        }
    }
    found.sort_unstable();
    found.dedup();
    found
}

/// The Zi data the units of `formation` hold: its leader's, and its
/// members' with a record.
fn formation_zi_data(rom: &[u8], formation: &Formation) -> Vec<u8> {
    let members = (0..SLOTS)
        .filter(|member| formation.get(MEMBERS + member * MEMBER_LEN + 1) != Some(&NO_RECORD));
    std::iter::once(None)
        .chain(members.map(Some))
        .filter_map(|member| spoils_record(rom, formation, member))
        .map(|record| record[0])
        .filter(|&zoid| zoid != NO_RECORD)
        .collect()
}

/// The sprite a map Zoid standing for `formation` shows: the first byte of
/// its leader's record (`0x080328FC`); `0xFF` when it cannot be read.
#[must_use]
pub fn leader_sprite(rom: &[u8], formation: &Formation) -> u8 {
    enemy_record(rom, formation, LEADER_SLOT).map_or(NO_RECORD, |record| record[0])
}

/// Where the party is taken after losing a battle on the field
/// (`0x08006E08`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReturnPoint {
    /// The map.
    pub map: usize,
    /// The player's cell on it.
    pub cell: (usize, usize),
}

/// The return point of area index `index` (the area less one, at
/// `0x02000B5C + 3`): record `index` of the 23 at ROM `0x328D6C`, eight
/// bytes each, a map, a column and a row in halfwords.
#[must_use]
pub fn return_point(rom: &[u8], index: usize) -> Option<ReturnPoint> {
    if index >= RETURN_POINT_COUNT {
        return None;
    }
    let start = RETURN_POINTS + index * RETURN_POINT_LEN;
    let record = rom.get(start..start + RETURN_POINT_LEN)?;
    let half = |at: usize| usize::from(u16::from_le_bytes([record[at], record[at + 1]]));
    Some(ReturnPoint {
        map: half(0),
        cell: (half(2), half(4)),
    })
}

fn rom_pointer(rom: &[u8], at: usize) -> Option<usize> {
    let bytes = rom.get(at..at + 4)?;
    let address = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    usize::try_from(address.checked_sub(ROM_BASE)?).ok()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn a_return_point_is_a_map_and_a_cell_by_area() {
        let mut rom = vec![0; RETURN_POINTS + RETURN_POINT_COUNT * RETURN_POINT_LEN];
        let at = RETURN_POINTS + 2 * RETURN_POINT_LEN;
        rom[at..at + 6].copy_from_slice(&[0x2C, 0x01, 7, 0, 5, 0]);
        assert_eq!(
            return_point(&rom, 2),
            Some(ReturnPoint {
                map: 300,
                cell: (7, 5)
            })
        );
        assert_eq!(return_point(&rom, RETURN_POINT_COUNT), None);
    }

    fn rom() -> Vec<u8> {
        let table = 0x0070_0000;
        let mut rom = vec![0; 0x0070_0000 + FORMATION_LEN * FORMATIONS_PER_COLUMN];
        let pointer = FORMATION_TABLES + AREA_TABLE_LEN + 4;
        rom[pointer..pointer + 4]
            .copy_from_slice(&(ROM_BASE + u32::try_from(table).unwrap()).to_le_bytes());
        for index in 0..FORMATIONS_PER_COLUMN {
            let at = table + index * FORMATION_LEN;
            rom[at + 1] = 2;
            rom[at + RARITY] = u8::from(index == 5);
            for slot in 0..SLOTS {
                rom[at + MEMBERS + slot * MEMBER_LEN + 1] = NO_RECORD;
            }
        }
        rom[table + MEMBERS + 1] = 3;
        rom[LEADER_RECORDS + 2 * ENEMY_RECORD_LEN] = 0x27;
        rom
    }

    #[test]
    fn the_roaming_zi_data_are_those_the_map_zoids_formations_hold() {
        let mut rom = rom();
        let table = 0x0070_0000;
        rom[table + MEMBERS + 1] = 3;
        rom[MEMBER_RECORDS + 3 * ENEMY_RECORD_LEN] = 0x31;
        rom[table + 7 * FORMATION_LEN + 1] = 4;
        rom[table + 7 * FORMATION_LEN + RARITY] = RARITY_CLASSES;
        rom[LEADER_RECORDS + 4 * ENEMY_RECORD_LEN] = 0x55;
        crate::saga::put_test_map(&mut rom, 4, 0x8002, 0x0060_0000, &[(1, MAP_ZOID, 0)]);
        crate::saga::put_test_map(&mut rom, 6, 2, 0x0060_1000, &[(1, MAP_ZOID, 0)]);
        assert_eq!(
            roaming_zi_data(&rom),
            vec![0x27, 0x31],
            "no class picks formation 7; map 6 keeps no objects"
        );
        rom[table + 7 * FORMATION_LEN + RARITY] = 2;
        assert_eq!(roaming_zi_data(&rom), vec![0x27, 0x31, 0x55]);
    }

    #[test]
    fn an_areas_column_has_twelve_formations() {
        let rom = rom();
        let found = formations(&rom, 2, 1).unwrap();
        assert_eq!(found.len(), FORMATIONS_PER_COLUMN);
        assert_eq!(leader_sprite(&rom, &found[0]), 0x27);
        assert!(enemy_record(&rom, &found[0], 0).is_some());
        assert!(enemy_record(&rom, &found[0], 1).is_none());
        assert!(enemy_record(&rom, &found[1], 0).is_none());
    }

    #[test]
    fn a_story_battle_brings_its_own_enemy_records() {
        let mut rom = vec![0; STORY_ENEMIES + STORY_BATTLE_COUNT * STORY_ENEMIES_LEN];
        let at = STORY_BATTLES + 3 * FORMATION_LEN;
        for slot in 0..SLOTS {
            rom[at + MEMBERS + slot * MEMBER_LEN + 1] = NO_RECORD;
        }
        rom[at + MEMBERS + 2 * MEMBER_LEN] = GUARDED;
        rom[at + MEMBERS + 2 * MEMBER_LEN + 1] = 1;
        rom[at + STORY_TERRAIN] = 7;
        rom[at + STORY_SONG] = 0x1B;
        rom[at + STORY_MODE] = MODE_NO_ITEMS;
        rom[STORY_ENEMIES + 3 * STORY_ENEMIES_LEN + ENEMY_RECORD_LEN] = 0x46;
        let battle = story_battle(&rom, 3).unwrap();
        assert_eq!((battle.terrain(), battle.song()), (7, 0x1B));
        assert!(battle.without_items() && !battle.unarmed());
        let lineup = Lineup::Story(battle);
        assert_eq!(
            lineup.enemy_record(&rom, 2).map(|record| record[0]),
            Some(0x46)
        );
        assert!(lineup.enemy_record(&rom, 0).is_none());
        assert!(lineup.guarded(2) && !lineup.guarded(0));
        assert!(story_battle(&rom, 42).is_none());
    }

    #[test]
    fn rolls_pick_a_class_and_a_class_its_formations() {
        assert_eq!(
            [
                rarity_for(59),
                rarity_for(60),
                rarity_for(94),
                rarity_for(95)
            ],
            [0, 1, 1, 2]
        );
        let rom = rom();
        let found = formations(&rom, 2, 1).unwrap();
        assert_eq!(of_class(&found, 1), vec![5]);
        assert_eq!(of_class(&found, 2), vec![0]);
    }
}
