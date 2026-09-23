//! The game-state block Zoids Saga saves (RAM `0x02000B5C`, `0x3F10`
//! bytes): the fields this port reads and writes, at the offsets the
//! game's own code uses. The rest of the block is left as it is. See
//! `docs/formats/save.md`.

use thiserror::Error;

use crate::font::shift_jis_code;

/// Size of the block.
pub const STATE_LEN: usize = 0x3F10;
/// Half-words of flag bits; flag `n` is bit `15 - n % 16` of word `n / 16`.
pub const FLAG_WORDS: usize = 33;
/// Flags the block holds.
pub const FLAG_COUNT: usize = FLAG_WORDS * 16;
/// Characters of the player's name.
pub const NAME_CHARS: usize = 8;
/// The full-width space the new-game routine pads the default name with.
pub const NAME_PADDING: u16 = 0x8140;
/// The full-width question mark written for a character Shift-JIS lacks.
pub const NAME_UNKNOWN: u16 = 0x8148;

const AREA: usize = 0x02;
const MAP: usize = 0x04;
const COLUMN: usize = 0x06;
const ROW: usize = 0x08;
const FLAGS: usize = 0x0C;
const LEVEL: usize = 0xCD2;
const EXPERIENCE: usize = 0xCD4;
const NAME: usize = 0xD18;
const MONEY: usize = 0xD28;
const MESSAGE_SPEED: usize = 0x3618;
const SONG: usize = 0x3F0E;
const HALF_WIDTH_FIRST: char = '!';
const HALF_WIDTH_LAST: char = '~';
const FULL_WIDTH_OFFSET: u32 = 0xFF01 - 0x21;
const IDEOGRAPHIC_SPACE: char = '\u{3000}';

/// Why a block cannot be read or written.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProgressError {
    /// The block is not the size the game saves.
    #[error("the game-state block is {actual} bytes, not {STATE_LEN}")]
    WrongLength {
        /// Bytes given.
        actual: usize,
    },
}

/// What the block says about where the game is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// Low byte of the current map record's id, written on entering a map.
    pub area: u8,
    /// Map record the player stands in.
    pub map: u16,
    /// Metatile column of the player.
    pub column: u16,
    /// Metatile row of the player.
    pub row: u16,
    /// The game flags as stored.
    pub flags: [u16; FLAG_WORDS],
    /// The party's level.
    pub level: u8,
    /// The party's experience points.
    pub experience: u32,
    /// The player's name as Shift-JIS codes, zero after the last one.
    pub name: [u16; NAME_CHARS],
    /// Money in G.
    pub money: u32,
    /// Battle message speed, 1 (fast) to 5 (slow); stored as speed − 1.
    pub message_speed: u8,
    /// Song playing when the menu was opened, which a continued game
    /// plays again.
    pub song: u16,
}

impl Progress {
    /// Reads the fields from a block.
    ///
    /// # Errors
    ///
    /// Returns [`ProgressError`] when the block has the wrong size.
    pub fn read(state: &[u8]) -> Result<Self, ProgressError> {
        check_len(state)?;
        let half = |at: usize| u16::from_le_bytes([state[at], state[at + 1]]);
        let word = |at: usize| {
            u32::from_le_bytes([state[at], state[at + 1], state[at + 2], state[at + 3]])
        };
        Ok(Self {
            area: state[AREA],
            map: half(MAP),
            column: half(COLUMN),
            row: half(ROW),
            flags: std::array::from_fn(|index| half(FLAGS + index * 2)),
            level: state[LEVEL],
            experience: word(EXPERIENCE),
            name: std::array::from_fn(|index| half(NAME + index * 2)),
            money: word(MONEY),
            message_speed: state[MESSAGE_SPEED].saturating_add(1),
            song: half(SONG),
        })
    }

    /// Writes the fields into a block, leaving its other bytes alone.
    ///
    /// # Errors
    ///
    /// Returns [`ProgressError`] when the block has the wrong size.
    pub fn write(&self, state: &mut [u8]) -> Result<(), ProgressError> {
        check_len(state)?;
        let mut put = |at: usize, bytes: &[u8]| state[at..at + bytes.len()].copy_from_slice(bytes);
        put(AREA, &[self.area]);
        put(MAP, &self.map.to_le_bytes());
        put(COLUMN, &self.column.to_le_bytes());
        put(ROW, &self.row.to_le_bytes());
        for (index, word) in self.flags.iter().enumerate() {
            put(FLAGS + index * 2, &word.to_le_bytes());
        }
        put(LEVEL, &[self.level]);
        put(EXPERIENCE, &self.experience.to_le_bytes());
        for (index, code) in self.name.iter().enumerate() {
            put(NAME + index * 2, &code.to_le_bytes());
        }
        put(MONEY, &self.money.to_le_bytes());
        put(MESSAGE_SPEED, &[self.message_speed.saturating_sub(1)]);
        put(SONG, &self.song.to_le_bytes());
        Ok(())
    }

    /// Whether flag `flag` is set; flags past [`FLAG_COUNT`] never are.
    #[must_use]
    pub fn flag(&self, flag: u16) -> bool {
        flag_position(flag).is_some_and(|(word, mask)| self.flags[word] & mask != 0)
    }

    /// Sets or clears flag `flag`; returns `false` for a flag the block
    /// cannot hold.
    pub fn set_flag(&mut self, flag: u16, set: bool) -> bool {
        let Some((word, mask)) = flag_position(flag) else {
            return false;
        };
        if set {
            self.flags[word] |= mask;
        } else {
            self.flags[word] &= !mask;
        }
        true
    }

    /// The flags that are set, in order.
    pub fn set_flags(&self) -> impl Iterator<Item = u16> + '_ {
        (0..FLAG_COUNT)
            .filter_map(|flag| u16::try_from(flag).ok())
            .filter(|flag| self.flag(*flag))
    }
}

fn check_len(state: &[u8]) -> Result<(), ProgressError> {
    if state.len() == STATE_LEN {
        Ok(())
    } else {
        Err(ProgressError::WrongLength {
            actual: state.len(),
        })
    }
}

fn flag_position(flag: u16) -> Option<(usize, u16)> {
    let flag = usize::from(flag);
    (flag < FLAG_COUNT).then(|| (flag / 16, 0x8000 >> (flag % 16)))
}

/// Encodes a name the way the name entry stores it: one Shift-JIS code per
/// character, half-width ASCII as its full-width form, zero after the last
/// one. Characters past [`NAME_CHARS`] are dropped and characters
/// Shift-JIS lacks become [`NAME_UNKNOWN`]; the flag says whether the
/// name survived intact.
#[must_use]
pub fn encode_name(name: &str) -> ([u16; NAME_CHARS], bool) {
    let mut codes = [0; NAME_CHARS];
    let mut intact = name.chars().count() <= NAME_CHARS;
    for (slot, ch) in codes.iter_mut().zip(name.chars()) {
        let code = shift_jis_code(full_width(ch));
        intact &= code.is_some() && full_width(ch) == ch;
        *slot = code.unwrap_or(NAME_UNKNOWN);
    }
    (codes, intact)
}

/// Decodes a stored name, stopping at a zero and dropping trailing
/// full-width spaces.
#[must_use]
pub fn decode_name(codes: &[u16]) -> String {
    let bytes: Vec<u8> = codes
        .iter()
        .take_while(|code| **code != 0)
        .flat_map(|code| code.to_be_bytes())
        .collect();
    let (text, _) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(&bytes);
    text.trim_end_matches(IDEOGRAPHIC_SPACE).to_owned()
}

fn full_width(ch: char) -> char {
    match ch {
        ' ' => IDEOGRAPHIC_SPACE,
        HALF_WIDTH_FIRST..=HALF_WIDTH_LAST => {
            char::from_u32(u32::from(ch) + FULL_WIDTH_OFFSET).unwrap_or(ch)
        }
        _ => ch,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn block() -> Vec<u8> {
        let mut state = vec![0; STATE_LEN];
        state[0x04] = 4;
        state[0x06] = 5;
        state[0x08] = 2;
        state[0x2E] = 1;
        state[0xCD2] = 1;
        state[0x3618] = 2;
        state[0x3F0E] = 7;
        state[0x100] = 0xAB;
        state
    }

    #[test]
    fn reads_the_fields_at_the_games_offsets() {
        let progress = Progress::read(&block()).expect("right size");
        assert_eq!((progress.map, progress.column, progress.row), (4, 5, 2));
        assert_eq!(progress.level, 1);
        assert_eq!(progress.message_speed, 3);
        assert_eq!(progress.song, 7);
        assert!(progress.flag(0x11F));
        assert_eq!(progress.set_flags().collect::<Vec<_>>(), vec![0x11F]);
    }

    #[test]
    fn writes_the_fields_and_keeps_the_rest() {
        let mut state = block();
        let mut progress = Progress::read(&state).expect("right size");
        progress.column = 8;
        progress.money = 0x0102_0304;
        progress.message_speed = 5;
        assert!(progress.set_flag(0, true));
        assert!(!progress.set_flag(600, true));
        progress.write(&mut state).expect("right size");
        assert_eq!(state[0x06], 8);
        assert_eq!(&state[0xD28..0xD2C], &[4, 3, 2, 1]);
        assert_eq!(state[0x3618], 4);
        assert_eq!(state[0x0D], 0x80);
        assert_eq!(state[0x100], 0xAB);
    }

    #[test]
    fn rejects_blocks_of_another_size() {
        assert_eq!(
            Progress::read(&[0; 4]),
            Err(ProgressError::WrongLength { actual: 4 })
        );
    }

    #[test]
    fn stores_names_as_full_width_shift_jis_codes() {
        let (codes, intact) = encode_name("アトレー");
        assert!(intact);
        assert_eq!(codes, [0x8341, 0x8367, 0x838C, 0x815B, 0, 0, 0, 0]);
        assert_eq!(decode_name(&codes), "アトレー");
        let (codes, intact) = encode_name("Ana");
        assert!(!intact);
        assert_eq!(&codes[..3], &[0x8260, 0x828E, 0x8281]);
        assert_eq!(decode_name(&codes), "Ａｎａ");
        let (codes, _) = encode_name("Iñigo");
        assert_eq!(codes[1], NAME_UNKNOWN);
    }

    #[test]
    fn stops_a_name_at_a_zero_code_and_drops_padding() {
        assert_eq!(decode_name(&[0x8341, 0, 0x8367]), "ア");
        assert_eq!(decode_name(&[0x8341, NAME_PADDING, NAME_PADDING]), "ア");
    }
}
