//! Forming the party in the game-state block, as the hangar's choice does.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! choice routine at `0x08037644`, the unit allocator at `0x08036A30` and
//! its initializer at `0x08036B2C`, the pilot assignment at `0x08036BE0`
//! with the unit statistics at `0x08036CB0` and the percentage routine at
//! `0x080346C0`, the starting units at `0x080374B8` / `0x080372D0` and the
//! formation slots at `0x08037AB4`.
//!
//! Units are 0x38-byte records at `+0xD2C` of the game state, 0x99
//! ordinary slots and 0x14 special ones after them, counted at `+0x3304`:
//!
//! | Offset | Content |
//! |---|---|
//! | 0 | The Zoid record's first half-word |
//! | 2 | Flags: 1 in use, 2 in the formation, 4 piloted, 8 special |
//! | 4 | The unit's slot |
//! | 6 | Zoid (index of the 76-byte records at ROM `0x670210`) |
//! | 8, 12 | Current values of the first two statistics |
//! | 16 | Six four-byte part entries, the part id at +2 (`0xFFFF` none) |
//! | 40 | Statistics: two words and two half-words |
//! | 52 | Training level: half of it is added in percent to every statistic |
//! | 53 | The Zoid record's byte 4 |

use formats::progress::STATE_LEN;

const UNITS: usize = 0xD2C;
const UNIT_LEN: usize = 0x38;
const ORDINARY_UNITS: usize = 0x99;
const ALL_UNITS: usize = 0xAD;
const UNIT_COUNT: usize = 0x3304;
const IN_USE: u16 = 1;
const IN_FORMATION: u16 = 2;
const PILOTED: u16 = 4;
const SPECIAL: u16 = 8;
const SPECIAL_TRAINING: u8 = 100;
const SPECIAL_UNIT: u16 = 4;
const ZOID_RECORDS: usize = 0x0067_0210;
const ZOID_RECORD_LEN: usize = 0x4C;
const ZOID_PARTS: usize = 8;
const ZOID_STATS: usize = 0x40;
const PARTS_LEN: usize = 24;
const STATS_LEN: usize = 12;
const UNIT_PARTS: usize = 0x10;
const UNIT_STATS: usize = 0x28;
const UNIT_TRAINING: usize = 0x34;
const UNIT_VARIANT: usize = 0x35;
const PART_RECORDS: usize = 0x0066_C8F8;
const PART_RECORD_LEN: usize = 24;
const PART_SLOTS: usize = 6;
const NO_PART: u16 = 0xFFFF;
const PART_ACTIVE: u32 = 0x2000_0000;
const PART_BONUSES: [(u32, usize); 6] = [
    (0x4000, 10),
    (0x8000, 10),
    (0x1_0000, 8),
    (0x2_0000, 8),
    (0x80_0000, 0),
    (0x100_0000, 4),
];
const PART_BONUS: usize = 0xC;
const CHARACTERS: usize = 0x34A4;
const CHARACTER_LEN: usize = 4;
const CHARACTER_UNIT: usize = 2;
const CHARACTER_IN_FORMATION: u16 = 0x10;
const NO_UNIT: u8 = 0xFF;
const FORMATION: usize = 0x3600;
const MEMBER_RECORDS: usize = 0xCD8;
const MEMBER_RECORD_LEN: usize = 16;
const MEMBERS: usize = 4;
const PILOT_TABLE: usize = 0x0067_B35C;
const PILOT_CHAPTERS: usize = 10;
const AREA: usize = 2;
const STARTING_LISTS: usize = 0x0067_E380;
const LIST_END: u8 = 0xFF;
const SHIELD_LIGER: u16 = 0x39;
const SABER_TIGER: u16 = 0x0F;
const RAYNOS: u16 = 0x5C;
const WARRIOR_SLOTS: [(u8, u8); 3] = [(1, 4), (2, 0), (3, 2)];
const MAX_WORD: [i32; 2] = [9999, 999];
const MAX_HALF: [i32; 2] = [9999, 999];
const ROM_BASE: u32 = 0x0800_0000;

/// Forms the party around the Zoid picked in the hangar (0 the Shield
/// Liger and any other value, 1 the Saber Tiger, 2 the Raynos): the
/// prince pilots it in formation slot 1, the warriors get their starting
/// Zoids in slots 4, 0 and 2. Returns `None` when `state` is not a
/// game-state block, no unit slot is free or a table is outside `rom`.
pub fn form_party(rom: &[u8], state: &mut [u8], choice: usize) -> Option<()> {
    if state.len() != STATE_LEN {
        return None;
    }
    let zoid = match choice {
        1 => SABER_TIGER,
        2 => RAYNOS,
        _ => SHIELD_LIGER,
    };
    let unit = add_unit(rom, state, zoid, false)?;
    assign(rom, state, 0, unit)?;
    place(state, 0, 1);
    for (character, bits, zoid) in starting_list(rom, 0)? {
        add_character(rom, state, character, bits, zoid)?;
    }
    for (character, slot) in WARRIOR_SLOTS {
        place(state, character, slot);
    }
    Some(())
}

fn half(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn set_half(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn set_word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn unit_at(unit: u8) -> usize {
    UNITS + usize::from(unit) * UNIT_LEN
}

fn zoid_record(rom: &[u8], zoid: u16) -> Option<&[u8]> {
    let at = ZOID_RECORDS + usize::from(zoid) * ZOID_RECORD_LEN;
    rom.get(at..at + ZOID_RECORD_LEN)
}

/// Allocates a unit of `zoid` in the first free slot, among the special
/// ones when `special` (`0x08036A30`), recounting the ordinary units in
/// use; `None` when none is free or the Zoid is unknown.
fn add_unit(rom: &[u8], state: &mut [u8], zoid: u16, special: bool) -> Option<u8> {
    let in_use = |state: &[u8], slot: usize| half(state, UNITS + slot * UNIT_LEN + 2) & IN_USE != 0;
    let slots = if special {
        ORDINARY_UNITS..ALL_UNITS
    } else {
        state[UNIT_COUNT] = u8::try_from(
            (0..ORDINARY_UNITS)
                .filter(|&slot| in_use(state, slot))
                .count(),
        )
        .unwrap_or(u8::MAX);
        0..ORDINARY_UNITS
    };
    let slot = slots.clone().find(|&slot| !in_use(state, slot))?;
    let unit = u8::try_from(slot).ok()?;
    let record = zoid_record(rom, zoid)?;
    let at = unit_at(unit);
    state[at..at + UNIT_LEN].fill(0);
    set_half(state, at + 4, u16::from(unit));
    set_half(state, at + 6, zoid);
    set_half(state, at + 2, IN_USE);
    state[UNIT_COUNT] = state[UNIT_COUNT].wrapping_add(1);
    set_half(state, at, half(record, 0));
    state[at + UNIT_TRAINING] = 0;
    state[at + UNIT_STATS..at + UNIT_STATS + STATS_LEN]
        .copy_from_slice(&record[ZOID_STATS..ZOID_STATS + STATS_LEN]);
    state[at + UNIT_VARIANT] = record[4];
    state[at + UNIT_PARTS..at + UNIT_PARTS + PARTS_LEN]
        .copy_from_slice(&record[ZOID_PARTS..ZOID_PARTS + PARTS_LEN]);
    let (current, energy) = (
        word(state, at + UNIT_STATS),
        word(state, at + UNIT_STATS + 4),
    );
    set_word(state, at + 8, current);
    set_word(state, at + 12, energy);
    Some(unit)
}

/// Makes `character` the pilot of `unit` and computes the unit's
/// statistics (`0x08036BE0`).
fn assign(rom: &[u8], state: &mut [u8], character: u8, unit: u8) -> Option<()> {
    state[CHARACTERS + usize::from(character) * CHARACTER_LEN + CHARACTER_UNIT] = unit;
    let at = unit_at(unit);
    let flags = half(state, at + 2) | PILOTED;
    set_half(state, at + 2, flags);
    compute_stats(rom, state, character, unit)
}

/// The unit's statistics from its Zoid, training, parts and pilot, capped
/// (`0x08036CB0`). The half-words are stored after every step, so they
/// wrap to 16 bits and read back signed.
fn compute_stats(rom: &[u8], state: &mut [u8], character: u8, unit: u8) -> Option<()> {
    let at = unit_at(unit);
    let record = zoid_record(rom, half(state, at + 6))?;
    let signed = |value: u16| i32::from(i16::from_ne_bytes(value.to_ne_bytes()));
    let as_half = |value: i32| {
        let bytes = value.to_le_bytes();
        u16::from_le_bytes([bytes[0], bytes[1]])
    };
    let mut words = [word(record, ZOID_STATS), word(record, ZOID_STATS + 4)]
        .map(|value| i32::from_ne_bytes(value.to_ne_bytes()));
    let mut halves = [half(record, ZOID_STATS + 8), half(record, ZOID_STATS + 10)];
    let training = i32::from(state[at + UNIT_TRAINING] >> 1);
    for value in &mut words {
        *value = value.wrapping_add(percent(*value, training));
    }
    for value in &mut halves {
        *value = as_half(signed(*value) + percent(signed(*value), training));
    }
    for slot in 0..PART_SLOTS {
        let part = half(state, at + UNIT_PARTS + slot * 4 + 2);
        if part == NO_PART {
            continue;
        }
        let part_at = PART_RECORDS + usize::from(part) * PART_RECORD_LEN;
        let record = rom.get(part_at..part_at + PART_RECORD_LEN)?;
        let flags = word(record, 0);
        if flags & PART_ACTIVE == 0 {
            continue;
        }
        let bonus = word(record, PART_BONUS);
        for (bit, field) in PART_BONUSES {
            if flags & bit == 0 {
                continue;
            }
            match field {
                0 | 4 => {
                    let value = &mut words[field / 4];
                    *value = value.wrapping_add(i32::from_ne_bytes(bonus.to_ne_bytes()));
                }
                _ => {
                    let value = &mut halves[(field - 8) / 2];
                    *value = value.wrapping_add(u16::try_from(bonus & 0xFFFF).unwrap_or(0));
                }
            }
        }
    }
    if character != NO_UNIT {
        let pilot = pilot(rom, state, character)?;
        words[0] = words[0].wrapping_add(percent(words[0], pilot[0]));
        for (value, bonus) in halves.iter_mut().zip([pilot[1], pilot[2]]) {
            *value = as_half(signed(*value) + percent(signed(*value), bonus));
        }
    }
    for (value, max) in words.iter_mut().zip(MAX_WORD) {
        *value = (*value).min(max);
    }
    for (value, max) in halves.iter_mut().zip(MAX_HALF) {
        if signed(*value) > max {
            *value = as_half(max);
        }
    }
    let base = at + UNIT_STATS;
    for (index, value) in words.iter().enumerate() {
        set_word(
            state,
            base + index * 4,
            u32::from_ne_bytes(value.to_ne_bytes()),
        );
    }
    for (index, value) in halves.iter().enumerate() {
        set_half(state, base + 8 + index * 2, *value);
    }
    Some(())
}

/// `percent`% of `value`, rounded to nearest for values below 0x10000, as
/// the routine at `0x080346C0` computes it.
fn percent(value: i32, percent: i32) -> i32 {
    if value >> 16 == 0 {
        let scaled = (value << 16) / 100;
        let scaled = scaled.wrapping_mul(percent);
        (scaled >> 16) + i32::from(scaled & 0xFFFF > 0x7FFF)
    } else {
        (value / 100).wrapping_mul(percent)
    }
}

/// The pilot's bonuses in percent to the first statistic and the two
/// half-words: the member records for the first four characters, the
/// table at ROM `0x67B35C` by chapter for the others (`0x080334F8`).
fn pilot(rom: &[u8], state: &[u8], character: u8) -> Option<[i32; 3]> {
    let character = usize::from(character);
    let record = if character < MEMBERS {
        let at = MEMBER_RECORDS + character * MEMBER_RECORD_LEN;
        state.get(at..at + MEMBER_RECORD_LEN)?.to_vec()
    } else {
        let chapter = usize::from(state[AREA].wrapping_sub(1));
        let chapter = if chapter > PILOT_CHAPTERS { 0 } else { chapter };
        let at = PILOT_TABLE + (character * PILOT_CHAPTERS + chapter) * 4;
        let pointer = word(rom.get(at..at + 4)?, 0);
        let start = usize::try_from(pointer.checked_sub(ROM_BASE)?).ok()?;
        rom.get(start..start + MEMBER_RECORD_LEN)?.to_vec()
    };
    let signed = |at: usize| i32::from(i16::from_ne_bytes(half(&record, at).to_ne_bytes()));
    Some([signed(4), signed(6), signed(8)])
}

/// Puts `character`'s unit in formation slot `slot` (`0x08037AB4`).
fn place(state: &mut [u8], character: u8, slot: u8) {
    let entry = CHARACTERS + usize::from(character) * CHARACTER_LEN;
    let unit = state[entry + CHARACTER_UNIT];
    if unit == NO_UNIT {
        return;
    }
    let formation = FORMATION + usize::from(slot) * 4;
    state[formation + 1] = character;
    state[formation] = unit;
    let at = unit_at(unit);
    let flags = half(state, at + 2) | IN_FORMATION;
    set_half(state, at + 2, flags);
    let bits = half(state, entry) | CHARACTER_IN_FORMATION;
    set_half(state, entry, bits);
}

/// The entries of starting list `list` (ROM `0x67E380`): the character's
/// flag bits, the character and its Zoid.
fn starting_list(rom: &[u8], list: usize) -> Option<Vec<(u8, u16, u16)>> {
    let at = STARTING_LISTS + list * 4;
    let pointer = word(rom.get(at..at + 4)?, 0);
    let mut at = usize::try_from(pointer.checked_sub(ROM_BASE)?).ok()?;
    let mut entries = Vec::new();
    loop {
        let entry = rom.get(at..at + 4)?;
        if entry[2] == LIST_END {
            return Some(entries);
        }
        entries.push((entry[2], half(entry, 0), u16::from(entry[3])));
        at += 4;
    }
}

/// Marks `character` with `bits` and, when `zoid` is not 0, gives them a
/// unit of it with full values (`0x080372D0`).
fn add_character(rom: &[u8], state: &mut [u8], character: u8, bits: u16, zoid: u16) -> Option<()> {
    let entry = CHARACTERS + usize::from(character) * CHARACTER_LEN;
    let flags = half(state, entry) | bits | 1;
    set_half(state, entry, flags);
    if zoid == 0 {
        return Some(());
    }
    let special = bits & SPECIAL_UNIT != 0;
    let Some(unit) = add_unit(rom, state, zoid, special) else {
        return Some(());
    };
    let at = unit_at(unit);
    if special {
        let flags = half(state, at + 2) | SPECIAL;
        set_half(state, at + 2, flags);
        state[at + UNIT_TRAINING] = SPECIAL_TRAINING;
    }
    assign(rom, state, character, unit)?;
    let (full, energy) = (
        word(state, at + UNIT_STATS),
        word(state, at + UNIT_STATS + 4),
    );
    set_word(state, at + 8, full);
    set_word(state, at + 12, energy);
    Some(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn percentages_round_small_values_and_truncate_large_ones() {
        assert_eq!(percent(250, 10), 25);
        assert_eq!(percent(25, 50), 13);
        assert_eq!(percent(15, 50), 7);
        assert_eq!(percent(0x2_0000, 50), 0x2_0000 / 100 * 50);
        assert_eq!(percent(100, 0), 0);
    }

    fn rom() -> Vec<u8> {
        let mut rom = vec![0; 0x0068_0000];
        for zoid in [0x39usize, 0x46] {
            let at = ZOID_RECORDS + zoid * ZOID_RECORD_LEN;
            rom[at] = 0x20;
            rom[at + 4] = 7;
            for slot in 0..PART_SLOTS {
                rom[at + ZOID_PARTS + slot * 4 + 2..at + ZOID_PARTS + slot * 4 + 4]
                    .copy_from_slice(&NO_PART.to_le_bytes());
            }
            rom[at + ZOID_STATS] = 100;
            rom[at + ZOID_STATS + 4] = 20;
            rom[at + ZOID_STATS + 8] = 250;
            rom[at + ZOID_STATS + 10] = 10;
        }
        let list = 0x0067_E000u32;
        rom[STARTING_LISTS..STARTING_LISTS + 4].copy_from_slice(&(ROM_BASE + list).to_le_bytes());
        let list = list as usize;
        rom[list..list + 8].copy_from_slice(&[2, 0, 1, 0x46, 0, 0, 0xFF, 0xFF]);
        rom
    }

    fn state() -> Vec<u8> {
        let mut state = vec![0; STATE_LEN];
        for character in 0..87 {
            state[CHARACTERS + character * CHARACTER_LEN + CHARACTER_UNIT] = NO_UNIT;
        }
        state
    }

    #[test]
    fn the_prince_pilots_the_picked_zoid_and_the_warriors_theirs() {
        let rom = rom();
        let mut state = state();
        form_party(&rom, &mut state, 0).expect("party");
        assert_eq!(state[UNIT_COUNT], 2);
        assert_eq!(half(&state, UNITS + 6), 0x39);
        assert_eq!(half(&state, UNITS + 2), IN_USE | PILOTED | IN_FORMATION);
        assert_eq!(state[UNITS + UNIT_VARIANT], 7);
        assert_eq!(word(&state, UNITS + UNIT_STATS), 100);
        assert_eq!(state[CHARACTERS + CHARACTER_UNIT], 0);
        assert_eq!(state[FORMATION + 4..FORMATION + 6], [0, 0]);
        let second = UNITS + UNIT_LEN;
        assert_eq!(half(&state, second + 6), 0x46);
        assert_eq!(word(&state, second + 8), 100);
        assert_eq!(state[CHARACTERS + CHARACTER_LEN + CHARACTER_UNIT], 1);
        assert_eq!(state[FORMATION + 16..FORMATION + 18], [1, 1]);
        assert_eq!(half(&state, CHARACTERS + CHARACTER_LEN) & 0x13, 0x13);
    }

    #[test]
    fn refuses_a_short_state_and_defaults_to_the_shield_liger() {
        let rom = rom();
        assert_eq!(form_party(&rom, &mut [0; 16], 0), None);
        let mut state = state();
        form_party(&rom, &mut state, 3).expect("party");
        assert_eq!(half(&state, UNITS + 6), SHIELD_LIGER);
    }
}
