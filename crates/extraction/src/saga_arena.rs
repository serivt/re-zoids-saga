//! The Zoid battles of chapter 5's colosseum: the fifteen matches of its
//! three domes, the regulations the desks check before each, and the arena
//! the party fights in.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! match task at `0x0801A3DC` with its table at ROM `0x668E7C` (12 bytes a
//! match: the desk's question, the judge's opening, the enemies' object
//! list and its length in bytes), the battle hook at `0x0801AE30` with its
//! table at ROM `0x669214` (8 bytes: the story battle, then the flag the
//! win sets), the judge's closing lines at ROM `0x6690AC`, the regulation
//! check at `0x08038654` with its routines listed at ROM `0x67E404`, the
//! formation count at `0x08038938` and the member list at `0x080389A4`.
//!
//! The arena is map 152 loaded with the match's list followed by an object
//! for each member of the formation, in slot order: the template at ROM
//! `0x668E68` showing the member's Zoid (sprite `0x98` plus the character,
//! bit 15 set), facing left, on the palette slot after the list's, at the
//! cell the table at ROM `0x32AED4` gives for that many members.

use crate::revision::{locate, locate_address};
use crate::saga::{MapError, MapObject, objects_at};
use crate::saga_party::{FORMATION_SLOTS, ZOID_RECORD_LEN, ZOID_RECORDS, formation, unit_record};

/// Matches in the colosseum.
pub const MATCHES: usize = 15;
/// The arena's map.
pub const ARENA_MAP: usize = 152;
/// Where the arena loads the player, off the map at once.
pub const ARENA_CELL: (usize, usize) = (8, 7);

const MATCH_TABLE: usize = 0x0066_8E7C;
const MATCH_LEN: usize = 12;
const BATTLE_TABLE: usize = 0x0066_9214;
const BATTLE_LEN: usize = 8;
const CLOSINGS: usize = 0x0066_90AC;
const MEMBER_TEMPLATE: u32 = 0x0866_8E68;
const MEMBER_CELLS: usize = 0x0032_AED4;
const OBJECT_LEN: usize = 20;
const PARTY_SPRITE: u16 = 0x8000 | 0x98;
const FACING_LEFT: usize = 2;
const CAT_ZOIDS: usize = 0x0067_E440;
const LIST_END: u8 = 0;
const FLYING: u16 = 2;
const SIZE: usize = 4;
const SMALL: u8 = 0;
const MEDIUM: u8 = 1;
const LARGE: u8 = 2;

/// A match of the colosseum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    /// The desk's question, whether to fight.
    pub question: u16,
    /// The judge's opening in the arena.
    pub opening: u16,
    /// The judge's closing once won; none for the final.
    pub closing: Option<u16>,
    /// The ROM address of the enemies' object list.
    pub enemies: u32,
    /// How many objects the list has.
    pub enemy_count: usize,
    /// The story battle fought.
    pub battle: u8,
    /// The flag the win sets.
    pub flag: u16,
}

/// What a match asks of the formation (the routines at ROM `0x67E404`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Regulation {
    /// Anything goes (`0x08038670`).
    Open,
    /// At most this many Zoids (`0x08038678`, `0x080388E4`, `0x08038900`).
    AtMost(usize),
    /// At most three, all flying (`0x08038694`).
    Flying,
    /// At most three, all of this size class (`0x08038708`, `0x0803879C`).
    AllSized(u8),
    /// At most three, one of them large (`0x0803880C`).
    OneLarge,
    /// At most three, all of the Liger, Tiger and Wolf types listed at ROM
    /// `0x67E440` (`0x08038880`).
    Cats,
}

const REGULATIONS: [Regulation; MATCHES] = [
    Regulation::Open,
    Regulation::Open,
    Regulation::AtMost(4),
    Regulation::Flying,
    Regulation::AllSized(SMALL),
    Regulation::Open,
    Regulation::Open,
    Regulation::AtMost(4),
    Regulation::AllSized(MEDIUM),
    Regulation::OneLarge,
    Regulation::Open,
    Regulation::Cats,
    Regulation::AtMost(3),
    Regulation::AtMost(2),
    Regulation::AtMost(3),
];

/// The half-word Rev 1 keeps at `at`, read where `rom`'s release keeps it.
fn half(rom: &[u8], at: usize) -> Option<u16> {
    let at = locate(rom, at);
    Some(u16::from_le_bytes([*rom.get(at)?, *rom.get(at + 1)?]))
}

/// The word Rev 1 keeps at `at`, read where `rom`'s release keeps it.
fn word(rom: &[u8], at: usize) -> Option<u32> {
    let at = locate(rom, at);
    Some(u32::from_le_bytes(rom.get(at..at + 4)?.try_into().ok()?))
}

/// Match `n` (0 to 14): the South dome's are 0 to 4, the East dome's 5 to
/// 9, the Main dome's 10 to 14.
#[must_use]
pub fn colosseum_match(rom: &[u8], n: usize) -> Option<Match> {
    if n >= MATCHES {
        return None;
    }
    let at = MATCH_TABLE + n * MATCH_LEN;
    let bytes = usize::try_from(word(rom, at + 8)?).ok()?;
    let battle = BATTLE_TABLE + n * BATTLE_LEN;
    let closing = half(rom, CLOSINGS + n * 4)?;
    Some(Match {
        question: half(rom, at)?,
        opening: half(rom, at + 2)?,
        closing: (closing != 0).then_some(closing),
        enemies: word(rom, at + 4)?,
        enemy_count: bytes / OBJECT_LEN,
        battle: *rom.get(locate(rom, battle))?,
        flag: u16::try_from(word(rom, battle + 4)?).ok()?,
    })
}

/// The Zoids of the formation's filled slots, in slot order.
fn formation_zoids(state: &[u8]) -> Vec<u8> {
    formation(state)
        .iter()
        .flatten()
        .map(|&(unit, _)| unit_record(state, unit).map_or(0, |record| record[6]))
        .collect()
}

fn zoid_flags(rom: &[u8], zoid: u8) -> u16 {
    half(rom, ZOID_RECORDS + usize::from(zoid) * ZOID_RECORD_LEN).unwrap_or(0)
}

fn zoid_size(rom: &[u8], zoid: u8) -> u8 {
    rom.get(locate(
        rom,
        ZOID_RECORDS + usize::from(zoid) * ZOID_RECORD_LEN + SIZE,
    ))
    .copied()
    .unwrap_or(0)
}

fn cat(rom: &[u8], zoid: u8) -> bool {
    rom.get(locate(rom, CAT_ZOIDS)..)
        .unwrap_or_default()
        .iter()
        .take_while(|&&listed| listed != LIST_END)
        .any(|&listed| listed == zoid)
}

/// Whether the formation meets match `n`'s regulation (`0x08038654`).
#[must_use]
pub fn meets_regulation(rom: &[u8], state: &[u8], n: usize) -> bool {
    let zoids = formation_zoids(state);
    let three = zoids.len() <= 3;
    match REGULATIONS.get(n) {
        None | Some(Regulation::Open) => true,
        Some(Regulation::AtMost(most)) => zoids.len() <= *most,
        Some(Regulation::Flying) => {
            three
                && zoids
                    .iter()
                    .all(|&zoid| zoid_flags(rom, zoid) & FLYING != 0)
        }
        Some(Regulation::AllSized(size)) => {
            three && zoids.iter().all(|&zoid| zoid_size(rom, zoid) == *size)
        }
        Some(Regulation::OneLarge) => {
            three && zoids.iter().any(|&zoid| zoid_size(rom, zoid) == LARGE)
        }
        Some(Regulation::Cats) => three && zoids.iter().all(|&zoid| cat(rom, zoid)),
    }
}

/// The arena's objects for match `n`: the match's list, then the
/// formation's members.
///
/// # Errors
///
/// Returns [`MapError`] when a list is outside the ROM.
pub fn arena_objects(rom: &[u8], state: &[u8], n: usize) -> Result<Vec<MapObject>, MapError> {
    let too_short = || MapError::TooShort {
        len: rom.len(),
        index: ARENA_MAP,
    };
    let game = colosseum_match(rom, n).ok_or_else(too_short)?;
    let mut objects = objects_at(rom, game.enemies, game.enemy_count)?;
    let template = objects_at(rom, locate_address(rom, MEMBER_TEMPLATE), 1)?
        .pop()
        .ok_or_else(too_short)?;
    let members: Vec<u8> = formation(state)
        .iter()
        .flatten()
        .map(|&(_, character)| character)
        .take(FORMATION_SLOTS)
        .collect();
    let count = members.len();
    let first = count * count.saturating_sub(1) / 2;
    for (index, character) in members.into_iter().enumerate() {
        let at = MEMBER_CELLS + (first + index) * 4;
        let (column, row) = (
            half(rom, at).ok_or_else(too_short)?,
            half(rom, at + 2).ok_or_else(too_short)?,
        );
        objects.push(MapObject {
            sprite: PARTY_SPRITE + u16::from(character),
            palette_slot: game.enemy_count + index,
            column: usize::from(column),
            row: usize::from(row),
            animation: FACING_LEFT,
            ..template.clone()
        });
    }
    Ok(objects)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x0070_0000];
        let put_half = |rom: &mut Vec<u8>, at: usize, value: u16| {
            rom[at..at + 2].copy_from_slice(&value.to_le_bytes());
        };
        let put_word = |rom: &mut Vec<u8>, at: usize, value: u32| {
            rom[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        put_half(&mut rom, MATCH_TABLE, 0x134);
        put_half(&mut rom, MATCH_TABLE + 2, 0x135);
        put_word(&mut rom, MATCH_TABLE + 4, 0x0860_0000);
        put_word(&mut rom, MATCH_TABLE + 8, 40);
        rom[BATTLE_TABLE] = 10;
        put_word(&mut rom, BATTLE_TABLE + 4, 0x17D);
        put_half(&mut rom, CLOSINGS, 0x136);
        put_word(&mut rom, 0x0060_0000 + 8, 0x8000_0000);
        put_word(&mut rom, 0x0060_0000 + 28, 0x8000_0000);
        put_half(&mut rom, 0x0066_8E68 + 12, 1);
        put_word(&mut rom, 0x0066_8E68 + 8, 0x8000_0000);
        for (i, (x, y)) in [(10, 7), (10, 6), (10, 8)].into_iter().enumerate() {
            put_half(&mut rom, MEMBER_CELLS + i * 4, x);
            put_half(&mut rom, MEMBER_CELLS + i * 4 + 2, y);
        }
        rom
    }

    fn state(members: &[(u8, u8)]) -> Vec<u8> {
        let mut state = vec![0u8; formats::progress::STATE_LEN];
        for slot in 0..FORMATION_SLOTS {
            let (unit, character) = members.get(slot).copied().unwrap_or((0xFF, 0xFF));
            state[0x3600 + slot * 4] = unit;
            state[0x3600 + slot * 4 + 1] = character;
        }
        state
    }

    #[test]
    fn a_match_reads_its_lines_list_battle_and_flag() {
        let game = colosseum_match(&rom(), 0);
        assert_eq!(
            game,
            Some(Match {
                question: 0x134,
                opening: 0x135,
                closing: Some(0x136),
                enemies: 0x0860_0000,
                enemy_count: 2,
                battle: 10,
                flag: 0x17D,
            })
        );
    }

    #[test]
    fn the_arena_adds_the_members_after_the_list() {
        let objects = arena_objects(&rom(), &state(&[(0xFF, 0), (3, 9), (4, 12)]), 0);
        let objects = objects.unwrap_or_default();
        assert_eq!(objects.len(), 4);
        assert_eq!(
            (
                objects[2].sprite,
                objects[2].palette_slot,
                objects[2].column,
                objects[2].row
            ),
            (0x80A1, 2, 10, 6)
        );
        assert_eq!(
            (objects[3].sprite, objects[3].column, objects[3].row),
            (0x80A4, 10, 8)
        );
        assert_eq!(objects[3].animation, FACING_LEFT);
    }

    #[test]
    fn a_regulation_counts_the_formation() {
        let rom = rom();
        let four = state(&[(1, 0), (2, 1), (3, 2), (4, 3)]);
        assert!(meets_regulation(&rom, &four, 2));
        assert!(!meets_regulation(&rom, &four, 12));
        assert!(!meets_regulation(
            &rom,
            &state(&[(1, 0), (2, 1), (3, 2)]),
            13
        ));
    }
}
