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

const OPTIONS: usize = 0x00;
const BATTLE_LABELS: u16 = 0x1000;
const AREA: usize = 0x02;
const MAP: usize = 0x04;
const COLUMN: usize = 0x06;
const ROW: usize = 0x08;
const BATTLES: usize = 0x0A;
const FLAGS: usize = 0x0C;
const LEVEL: usize = 0xCD2;
const EXPERIENCE: usize = 0xCD4;
const NAME: usize = 0xD18;
const MONEY: usize = 0xD28;
const MESSAGE_SPEED: usize = 0x3618;
const SONG: usize = 0x3F0E;
const ZOIDS_SEEN: usize = 0x33E2;
const CHARACTERS: usize = 0x34A4;
const DECK_COMMANDS: usize = 0x347B;
const CHARACTER_LEN: usize = 4;
const OBJECT_STATES: usize = 0x50;
const OBJECT_STATE_LEN: usize = 16;
const END_OF_OBJECTS: u16 = 0xFFFF;
const OBJECT_PRESENT: u16 = 0x8000;
const CHARACTER_IN_GUIDE: u16 = 0x20;
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

/// Whether the player has seen the Zoid of picture `id`: its byte in the
/// table at `+0x33E2` is not zero.
#[must_use]
pub fn zoid_seen(state: &[u8], id: usize) -> bool {
    state.get(ZOIDS_SEEN + id).is_some_and(|seen| *seen != 0)
}

/// Marks the Zoid of picture `id` as seen, as `0x08037098` does.
pub fn see_zoid(state: &mut [u8], id: usize) {
    if let Some(seen) = state.get_mut(ZOIDS_SEEN + id) {
        *seen = 1;
    }
}

/// Whether character `index` is in the character guide: bit `0x20` of its
/// half-word in the four-byte records at `+0x34A4`.
#[must_use]
pub fn character_known(state: &[u8], index: usize) -> bool {
    let at = CHARACTERS + index * CHARACTER_LEN;
    state
        .get(at..at + 2)
        .is_some_and(|bits| u16::from_le_bytes([bits[0], bits[1]]) & CHARACTER_IN_GUIDE != 0)
}

/// Marks deck command `command` as learned: its byte in the table at
/// `+0x347B` becomes 1, as the routine at `0x080370C0` does.
pub fn learn_command(state: &mut [u8], command: usize) {
    if let Some(learned) = state.get_mut(DECK_COMMANDS + command) {
        *learned = 1;
    }
}

/// Whether deck command `command` has been learned.
#[must_use]
pub fn command_learned(state: &[u8], command: usize) -> bool {
    state
        .get(DECK_COMMANDS + command)
        .is_some_and(|learned| *learned != 0)
}

/// The most object states the block holds: the rebuild (`0x08006E4C`)
/// stops the game on an error screen past 200.
pub const OBJECT_STATE_LIMIT: usize = 200;

/// What the block keeps of an object of a map whose record id has bit 15:
/// the 16-byte records at `+0x50`, ended by a map of `0xFFFF`, which the
/// map loader (`0x08007188`) builds the objects from instead of the map's
/// own list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectState {
    /// The map record the object belongs to.
    pub map: u16,
    /// Whether it is still there (bit 15 of the map); a beaten enemy is
    /// not.
    pub present: bool,
    /// Metatile column where it last stood.
    pub column: u8,
    /// Metatile row.
    pub row: u8,
    /// Its sprite sheet.
    pub sprite: u16,
    /// For a map Zoid, its formation among those of the area.
    pub group: u16,
    /// The object's parameter (its record's half-word 7).
    pub parameter: u16,
    /// The object's command (its record's half-word 6).
    pub command: u8,
    /// Three bytes the loader copies into the entity (`+0x68`, `+0x6A`,
    /// `+0x6C`); zero when the table is built.
    pub extra: [u8; 3],
}

impl ObjectState {
    fn read(bytes: &[u8]) -> Option<Self> {
        let half = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        let map = half(0);
        (map != END_OF_OBJECTS).then(|| Self {
            map: map & !OBJECT_PRESENT,
            present: map & OBJECT_PRESENT != 0,
            column: bytes[2],
            row: bytes[3],
            sprite: half(4),
            group: half(6),
            parameter: half(8),
            command: bytes[10],
            extra: [bytes[11], bytes[12], bytes[13]],
        })
    }

    fn write(&self, bytes: &mut [u8]) {
        let map = self.map | if self.present { OBJECT_PRESENT } else { 0 };
        bytes[0..2].copy_from_slice(&map.to_le_bytes());
        bytes[2] = self.column;
        bytes[3] = self.row;
        bytes[4..6].copy_from_slice(&self.sprite.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.group.to_le_bytes());
        bytes[8..10].copy_from_slice(&self.parameter.to_le_bytes());
        bytes[10] = self.command;
        bytes[11..14].copy_from_slice(&self.extra);
    }
}

/// The object states the block holds, up to the end marker.
#[must_use]
pub fn object_states(state: &[u8]) -> Vec<ObjectState> {
    (0..=OBJECT_STATE_LIMIT)
        .map_while(|index| {
            let at = OBJECT_STATES + index * OBJECT_STATE_LEN;
            state
                .get(at..at + OBJECT_STATE_LEN)
                .and_then(ObjectState::read)
        })
        .collect()
}

/// Replaces the object states with `objects`, ended by the marker; states
/// past [`OBJECT_STATE_LIMIT`] are dropped.
pub fn write_object_states(state: &mut [u8], objects: &[ObjectState]) {
    let count = objects.len().min(OBJECT_STATE_LIMIT);
    for (index, object) in objects.iter().take(count).enumerate() {
        let at = OBJECT_STATES + index * OBJECT_STATE_LEN;
        if let Some(bytes) = state.get_mut(at..at + OBJECT_STATE_LEN) {
            object.write(bytes);
        }
    }
    let end = OBJECT_STATES + count * OBJECT_STATE_LEN;
    if let Some(bytes) = state.get_mut(end..end + 2) {
        bytes.copy_from_slice(&END_OF_OBJECTS.to_le_bytes());
    }
}

/// Records where object state `index` stands, as a step does halfway
/// through (`0x0800B764`).
pub fn set_object_cell(state: &mut [u8], index: usize, (column, row): (u8, u8)) {
    let at = OBJECT_STATES + index * OBJECT_STATE_LEN;
    if let Some(bytes) = state.get_mut(at + 2..at + 4) {
        bytes.copy_from_slice(&[column, row]);
    }
}

/// Marks object state `index` as gone, as a won battle does to the enemy
/// (`0x0800B9CC`).
pub fn remove_object(state: &mut [u8], index: usize) {
    let at = OBJECT_STATES + index * OBJECT_STATE_LEN;
    if let Some(byte) = state.get_mut(at + 1) {
        *byte &= !OBJECT_PRESENT.to_le_bytes()[1];
    }
}

/// Whether the battle screen shows the party's hit points and energy over
/// its units (bit `0x1000` of the half-word at `+0`, which L toggles,
/// `0x0802F8F8`).
#[must_use]
pub fn battle_labels(state: &[u8]) -> bool {
    state
        .get(OPTIONS..OPTIONS + 2)
        .is_some_and(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) & BATTLE_LABELS != 0)
}

/// Sets or clears [`battle_labels`].
pub fn set_battle_labels(state: &mut [u8], on: bool) {
    if let Some(bytes) = state.get_mut(OPTIONS..OPTIONS + 2) {
        let word = u16::from_le_bytes([bytes[0], bytes[1]]);
        let word = if on {
            word | BATTLE_LABELS
        } else {
            word & !BATTLE_LABELS
        };
        bytes.copy_from_slice(&word.to_le_bytes());
    }
}

/// Counts a battle won against a roaming enemy: the half-word at `+0x0A`
/// (`0x0800B9CC`).
pub fn count_battle(state: &mut [u8]) {
    if let Some(bytes) = state.get_mut(BATTLES..BATTLES + 2) {
        let count = u16::from_le_bytes([bytes[0], bytes[1]]).wrapping_add(1);
        bytes.copy_from_slice(&count.to_le_bytes());
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
    fn the_battle_labels_bit_keeps_the_rest_of_its_word() {
        let mut state = vec![0; STATE_LEN];
        state[0] = 0x05;
        assert!(!battle_labels(&state));
        set_battle_labels(&mut state, true);
        assert!(battle_labels(&state));
        assert_eq!(&state[..2], &[0x05, 0x10]);
        set_battle_labels(&mut state, false);
        assert_eq!(&state[..2], &[0x05, 0x00]);
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
    fn tells_seen_zoids_and_known_characters() {
        let mut state = block();
        state[0x33E2 + 5] = 1;
        state[0x34A4 + 4 * 2] = 0x23;
        assert!(zoid_seen(&state, 5) && !zoid_seen(&state, 6));
        assert!(character_known(&state, 2) && !character_known(&state, 3));
        assert!(!zoid_seen(&state, STATE_LEN));
    }

    #[test]
    fn keeps_object_states_up_to_the_end_marker() {
        let mut state = block();
        let enemy = ObjectState {
            map: 1,
            present: true,
            column: 8,
            row: 9,
            sprite: 0x27,
            group: 10,
            parameter: 0,
            command: 4,
            extra: [0; 3],
        };
        write_object_states(&mut state, &[enemy]);
        assert_eq!(
            &state[0x50..0x5B],
            &[1, 0x80, 8, 9, 0x27, 0, 10, 0, 0, 0, 4]
        );
        assert_eq!(&state[0x60..0x62], &[0xFF, 0xFF]);
        set_object_cell(&mut state, 0, (7, 9));
        remove_object(&mut state, 0);
        let read = object_states(&state);
        assert_eq!(read.len(), 1);
        assert_eq!((read[0].column, read[0].present), (7, false));
        count_battle(&mut state);
        assert_eq!(&state[0x0A..0x0C], &[1, 0]);
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
