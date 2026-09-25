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
/// Rolls below this (of 100) pick a formation of class 0.
const COMMON_ROLLS: u16 = 60;
/// Rolls below this pick class 1, the rest class 2.
const UNCOMMON_ROLLS: u16 = 95;

/// A formation as the area's table holds it: 36 bytes, which the battle
/// reads again when it starts.
pub type Formation = [u8; FORMATION_LEN];

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
