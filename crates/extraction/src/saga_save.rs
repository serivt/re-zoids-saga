//! What Zoids Saga's save needs from the ROM: the layout its save routine
//! reads from a descriptor table, and the game-state block as the new-game
//! routine (`0x08003948`) leaves it. See `docs/formats/save.md`.

use formats::progress::STATE_LEN;
use formats::{Progress, SaveLayout};
use thiserror::Error;

/// The save descriptor: the bytes one copy takes, then address and size
/// pairs ending at a zero address; the first pair is the header string in
/// ROM, the others are the RAM blocks of a copy.
const SAVE_DESCRIPTOR: usize = 0x0066_6E54;
const DESCRIPTOR_ENTRY_LEN: usize = 8;
const DESCRIPTOR_MAX_ENTRIES: usize = 16;
/// The save routine writes the same blocks twice, the second copy being
/// the one the loader falls back to.
const SAVE_COPIES: usize = 2;
/// The cartridge's 32 KiB SRAM (`SRAM_F_V102`).
const SAVE_MEMORY_SIZE: usize = 0x8000;
const ROM_BASE: u32 = 0x0800_0000;
/// The four member records the new-game routine copies to `+0xCD8`.
const ROSTER: usize = 0x0067_AC4C;
const ROSTER_RECORDS: usize = 4;
const ROSTER_RECORD_LEN: usize = 16;
const ROSTER_FIELD: usize = 0xCD8;
/// Pointers to `0xFF`-terminated lists of characters; the new-game
/// routine puts the characters of list 0 in the character guide.
const CHARACTER_LISTS: usize = 0x0066_C8D0;
/// The character table: four bytes per character, a flag half-word (bit
/// `0x20`: in the character guide) and a byte that is `0xFF` when empty.
const CHARACTER_TABLE: usize = 0x34A4;
const CHARACTER_LEN: usize = 4;
const CHARACTERS: usize = 0x57;
const IN_GUIDE: u16 = 0x20;
const FIRST_CHARACTER_BITS: u16 = 0x03;
const LIST_END: u8 = 0xFF;
const EMPTY: u8 = 0xFF;
const CHARACTER_EMPTY_FIELD: usize = 2;
const EMPTY_RUN: std::ops::Range<usize> = 0x349C..0x34A2;
const PAIR_TABLE: usize = 0x3600;
const PAIR_ENTRIES: usize = 6;
const NEW_GAME_LEVEL: u8 = 1;
const NEW_GAME_MESSAGE_SPEED: u8 = 3;

/// Why the save data cannot be read from the ROM.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SaveDataError {
    /// The ROM is too short for a table.
    #[error("ROM of {len} bytes is too short for the {what}")]
    TooShort {
        /// ROM length.
        len: usize,
        /// Which table.
        what: &'static str,
    },
    /// The descriptor does not describe a layout the port understands.
    #[error("the save descriptor is not understood: {0}")]
    Descriptor(&'static str),
}

/// The layout the save routine writes, read from its descriptor.
///
/// # Errors
///
/// Returns [`SaveDataError`] when the descriptor cannot be read or does not
/// add up.
pub fn save_layout(rom: &[u8]) -> Result<SaveLayout, SaveDataError> {
    let word = |at: usize, what: &'static str| {
        rom.get(at..at + 4)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            .ok_or(SaveDataError::TooShort {
                len: rom.len(),
                what,
            })
    };
    let copy_len = to_usize(word(SAVE_DESCRIPTOR, "save descriptor")?);
    let mut entries = Vec::new();
    for index in 0..DESCRIPTOR_MAX_ENTRIES {
        let at = SAVE_DESCRIPTOR + 4 + index * DESCRIPTOR_ENTRY_LEN;
        let address = word(at, "save descriptor")?;
        if address == 0 {
            break;
        }
        entries.push((address, to_usize(word(at + 4, "save descriptor")?)));
    }
    let Some((&(magic_address, magic_len), blocks)) = entries.split_first() else {
        return Err(SaveDataError::Descriptor("no entries"));
    };
    let magic_at = magic_address
        .checked_sub(ROM_BASE)
        .map(to_usize)
        .ok_or(SaveDataError::Descriptor("the header is not in ROM"))?;
    let magic = rom
        .get(magic_at..magic_at + magic_len)
        .ok_or(SaveDataError::TooShort {
            len: rom.len(),
            what: "save header",
        })?
        .to_vec();
    let layout = SaveLayout {
        magic,
        blocks: blocks.iter().map(|(_, size)| *size).collect(),
        copies: SAVE_COPIES,
        memory_size: SAVE_MEMORY_SIZE,
    };
    if layout.copy_len() != copy_len {
        return Err(SaveDataError::Descriptor("the copy size does not match"));
    }
    if layout.used_len() > layout.memory_size {
        return Err(SaveDataError::Descriptor("the copies overflow the memory"));
    }
    Ok(layout)
}

/// The game-state block as a new game starts it: the member records and
/// the character table seeded from the ROM, the empty markers set, level 1
/// and message speed 3. The name is left to the name entry.
///
/// # Errors
///
/// Returns [`SaveDataError`] when a table cannot be read.
pub fn new_game_state(rom: &[u8]) -> Result<Vec<u8>, SaveDataError> {
    let too_short = |what: &'static str| SaveDataError::TooShort {
        len: rom.len(),
        what,
    };
    let mut state = vec![0; STATE_LEN];
    let roster = rom
        .get(ROSTER..ROSTER + ROSTER_RECORDS * ROSTER_RECORD_LEN)
        .ok_or_else(|| too_short("member records"))?;
    state[ROSTER_FIELD..ROSTER_FIELD + roster.len()].copy_from_slice(roster);
    for entry in 0..CHARACTERS {
        state[CHARACTER_TABLE + entry * CHARACTER_LEN + CHARACTER_EMPTY_FIELD] = EMPTY;
    }
    for entry in 0..PAIR_ENTRIES {
        state[PAIR_TABLE + entry * 4] = EMPTY;
        state[PAIR_TABLE + entry * 4 + 1] = EMPTY;
    }
    state[EMPTY_RUN].fill(EMPTY);
    mark_character(&mut state, 0, FIRST_CHARACTER_BITS);
    for entry in starting_list(rom).ok_or_else(|| too_short("starting characters"))? {
        mark_character(&mut state, usize::from(entry), IN_GUIDE);
    }
    let mut progress = Progress::read(&state).map_err(|_| too_short("game state"))?;
    progress.level = NEW_GAME_LEVEL;
    progress.message_speed = NEW_GAME_MESSAGE_SPEED;
    progress
        .write(&mut state)
        .map_err(|_| too_short("game state"))?;
    Ok(state)
}

fn starting_list(rom: &[u8]) -> Option<Vec<u8>> {
    let pointer = rom.get(CHARACTER_LISTS..CHARACTER_LISTS + 4)?;
    let address = u32::from_le_bytes([pointer[0], pointer[1], pointer[2], pointer[3]]);
    let start = to_usize(address.checked_sub(ROM_BASE)?);
    let list = rom.get(start..)?;
    let end = list.iter().position(|entry| *entry == LIST_END)?;
    Some(list[..end].to_vec())
}

fn mark_character(state: &mut [u8], entry: usize, bits: u16) {
    let at = CHARACTER_TABLE + entry * CHARACTER_LEN;
    if let Some(field) = state.get_mut(at..at + 2) {
        let value = u16::from_le_bytes([field[0], field[1]]) | bits;
        field.copy_from_slice(&value.to_le_bytes());
    }
}

fn to_usize(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn put(rom: &mut [u8], at: usize, words: &[u32]) {
        for (index, word) in words.iter().enumerate() {
            rom[at + index * 4..at + index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
    }

    fn synthetic_rom() -> Vec<u8> {
        let mut rom = vec![0; 0x0070_0000];
        let magic = 0x0066_6E48;
        rom[magic..magic + 5].copy_from_slice(b"TEST\0");
        put(
            &mut rom,
            SAVE_DESCRIPTOR,
            &[
                0x10 + 4 + 2 + 4,
                ROM_BASE + 0x0066_6E48,
                5,
                0x0200_0000,
                0x10,
                0x0200_1000,
                2,
                0,
                0x1902_1901,
            ],
        );
        rom[ROSTER + 2] = 0x42;
        let list = 0x0066_C000;
        put(&mut rom, CHARACTER_LISTS, &[ROM_BASE + 0x0066_C000]);
        rom[list..list + 3].copy_from_slice(&[1, 2, LIST_END]);
        rom
    }

    #[test]
    fn reads_the_layout_from_the_descriptor() {
        let layout = save_layout(&synthetic_rom()).expect("layout");
        assert_eq!(layout.magic, b"TEST\0");
        assert_eq!(layout.blocks, vec![0x10, 2]);
        assert_eq!(layout.copies, 2);
        assert_eq!(layout.memory_size, SAVE_MEMORY_SIZE);
    }

    #[test]
    fn refuses_a_descriptor_whose_sizes_disagree() {
        let mut rom = synthetic_rom();
        put(&mut rom, SAVE_DESCRIPTOR, &[0x99]);
        assert_eq!(
            save_layout(&rom),
            Err(SaveDataError::Descriptor("the copy size does not match"))
        );
    }

    #[test]
    fn builds_the_new_game_block() {
        let state = new_game_state(&synthetic_rom()).expect("state");
        let progress = Progress::read(&state).expect("block");
        assert_eq!(progress.level, 1);
        assert_eq!(progress.message_speed, 3);
        assert_eq!(state[ROSTER_FIELD + 2], 0x42);
        assert_eq!(
            &state[CHARACTER_TABLE..CHARACTER_TABLE + 12],
            &[0x03, 0, EMPTY, 0, 0x20, 0, EMPTY, 0, 0x20, 0, EMPTY, 0]
        );
        assert_eq!(state[CHARACTER_TABLE + 0x56 * 4 + 2], EMPTY);
        assert_eq!(state[CHARACTER_TABLE + 0x57 * 4 + 2], 0);
        assert_eq!(&state[0x3600..0x3602], &[EMPTY, EMPTY]);
        assert_eq!(&state[0x349C..0x34A2], &[EMPTY; 6]);
    }
}
