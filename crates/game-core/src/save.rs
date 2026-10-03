//! Saving and continuing with the original's save memory, so a `.sav`
//! from the cartridge or an emulator continues here and the other way
//! round (see `docs/formats/save.md`).
//!
//! Only the game-state block is written, to both copies, as the pause
//! menu's save does; the other blocks of an existing image stay as they
//! were. Continuing reads the first copy and falls back to the second.
//! Because the original stores the name in Shift-JIS, a name with letters
//! it lacks is also kept, exactly, in a note of this port's own in the
//! bytes after the copies, which the original never touches; a second note
//! keeps the player's statistics (see [`crate::stats`]).

use formats::progress::{STATE_LEN, decode_name};
use formats::save::{ERASED, checksum};
use formats::{Progress, SaveLayout, SaveMemory};

use crate::stats::{STATS_LEN, Stats};

/// The block of a copy that holds the game state.
const STATE_BLOCK: usize = 0;
/// The dialogue strings the loader shows.
const NOTICE_MISSING: usize = 10;
const NOTICE_CORRUPT: usize = 11;
const NOTICE_RESTORED: usize = 12;
const NOTE_MAGIC: &[u8; 4] = b"RZSN";
const NOTE_SUM_LEN: usize = 4;
const NOTE_HEADER_LEN: usize = NOTE_MAGIC.len() + NOTE_SUM_LEN + 1;
/// The statistics' note, after the room the name's takes: a name of eight
/// characters of four bytes and its header fit well within it.
const STATS_MAGIC: &[u8; 4] = b"RZST";
const STATS_NOTE_AT: usize = 64;

/// A game-state block read back, with the name to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedGame {
    /// The block as stored.
    pub state: Vec<u8>,
    /// The player's name: the port's note when it belongs to this block,
    /// otherwise the block's own.
    pub player_name: String,
    /// The player's statistics: the port's note when it belongs to this
    /// block, otherwise none counted yet.
    pub stats: Stats,
}

impl SavedGame {
    /// The fields of the block.
    #[must_use]
    pub fn progress(&self) -> Option<Progress> {
        Progress::read(&self.state).ok()
    }
}

/// What continuing found in the save memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// The first copy is sound.
    Saved(SavedGame),
    /// The first copy is broken and the second is used.
    Restored(SavedGame),
    /// Nothing was ever saved.
    Missing,
    /// Both copies are broken.
    Corrupt,
}

impl Found {
    /// The dialogue string the original shows for this, if any.
    #[must_use]
    pub fn notice(&self) -> Option<usize> {
        match self {
            Self::Saved(_) => None,
            Self::Restored(_) => Some(NOTICE_RESTORED),
            Self::Missing => Some(NOTICE_MISSING),
            Self::Corrupt => Some(NOTICE_CORRUPT),
        }
    }

    /// The game to continue, if there is one.
    #[must_use]
    pub fn game(&self) -> Option<&SavedGame> {
        match self {
            Self::Saved(game) | Self::Restored(game) => Some(game),
            Self::Missing | Self::Corrupt => None,
        }
    }
}

/// Why a save image could not be put together.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SaveError {
    /// The layout has no room for the game-state block.
    #[error(transparent)]
    Memory(#[from] formats::SaveMemoryError),
}

/// Reads and writes save images with the layout the ROM describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFile {
    layout: SaveLayout,
}

impl SaveFile {
    /// Uses `layout`.
    #[must_use]
    pub fn new(layout: SaveLayout) -> Self {
        Self { layout }
    }

    /// What continuing finds in `image`, as the original's loader decides
    /// it.
    #[must_use]
    pub fn read(&self, image: Option<Vec<u8>>) -> Found {
        let Some(memory) = image.and_then(|bytes| SaveMemory::from_bytes(bytes, &self.layout).ok())
        else {
            return Found::Missing;
        };
        if !memory.is_formatted(&self.layout) {
            return Found::Missing;
        }
        let copy = |index: usize| {
            memory
                .block(&self.layout, index, STATE_BLOCK)
                .ok()
                .flatten()
                .filter(|state| state.len() == STATE_LEN)
                .map(|state| self.saved_game(&memory, state))
        };
        if let Some(game) = copy(0) {
            return Found::Saved(game);
        }
        (1..self.layout.copies)
            .find_map(copy)
            .map_or(Found::Corrupt, Found::Restored)
    }

    /// The image to store: `previous` (or a fresh memory when it holds no
    /// data) with `state` in every copy and the notes for `player_name` and
    /// `statistics`.
    ///
    /// # Errors
    ///
    /// Returns [`SaveError`] when the layout cannot hold the block.
    pub fn write(
        &self,
        previous: Option<Vec<u8>>,
        state: &[u8],
        player_name: &str,
        statistics: &Stats,
    ) -> Result<Vec<u8>, SaveError> {
        let mut memory = previous
            .and_then(|bytes| SaveMemory::from_bytes(bytes, &self.layout).ok())
            .filter(|memory| memory.is_formatted(&self.layout))
            .unwrap_or_else(|| SaveMemory::formatted(&self.layout));
        for copy in 0..self.layout.copies {
            memory.set_block(&self.layout, copy, STATE_BLOCK, state)?;
        }
        write_note(memory.spare_mut(&self.layout), state, player_name);
        write_stats(memory.spare_mut(&self.layout), state, statistics);
        Ok(memory.bytes().to_vec())
    }

    /// `image` without this port's notes (the player's name in full and the
    /// statistics): the bytes after the copies erased, as the original
    /// leaves them, so the memory is the one the cartridge would hold. An
    /// image that is not a save memory comes back as it is.
    #[must_use]
    pub fn without_port_notes(&self, image: Vec<u8>) -> Vec<u8> {
        match SaveMemory::from_bytes(image.clone(), &self.layout) {
            Ok(mut memory) => {
                memory.spare_mut(&self.layout).fill(ERASED);
                memory.bytes().to_vec()
            }
            Err(_) => image,
        }
    }

    fn saved_game(&self, memory: &SaveMemory, state: &[u8]) -> SavedGame {
        let player_name = read_note(memory.spare(&self.layout), state).unwrap_or_else(|| {
            Progress::read(state)
                .map(|progress| decode_name(&progress.name))
                .unwrap_or_default()
        });
        let statistics = read_stats(memory.spare(&self.layout), state).unwrap_or_default();
        SavedGame {
            state: state.to_vec(),
            player_name,
            stats: statistics,
        }
    }
}

fn write_note(spare: &mut [u8], state: &[u8], player_name: &str) {
    let name = player_name.as_bytes();
    let Ok(len) = u8::try_from(name.len()) else {
        return;
    };
    if NOTE_HEADER_LEN + name.len() > STATS_NOTE_AT {
        return;
    }
    let Some(note) = spare.get_mut(..NOTE_HEADER_LEN + name.len()) else {
        return;
    };
    note[..NOTE_MAGIC.len()].copy_from_slice(NOTE_MAGIC);
    note[NOTE_MAGIC.len()..NOTE_MAGIC.len() + NOTE_SUM_LEN]
        .copy_from_slice(&checksum(state).to_le_bytes());
    note[NOTE_HEADER_LEN - 1] = len;
    note[NOTE_HEADER_LEN..].copy_from_slice(name);
}

fn read_note(spare: &[u8], state: &[u8]) -> Option<String> {
    let header = spare.get(..NOTE_HEADER_LEN)?;
    if &header[..NOTE_MAGIC.len()] != NOTE_MAGIC {
        return None;
    }
    let sum = &header[NOTE_MAGIC.len()..NOTE_MAGIC.len() + NOTE_SUM_LEN];
    if u32::from_le_bytes([sum[0], sum[1], sum[2], sum[3]]) != checksum(state) {
        return None;
    }
    let len = usize::from(header[NOTE_HEADER_LEN - 1]);
    let name = spare.get(NOTE_HEADER_LEN..NOTE_HEADER_LEN + len)?;
    String::from_utf8(name.to_vec()).ok()
}

fn write_stats(spare: &mut [u8], state: &[u8], statistics: &Stats) {
    let Some(note) = spare.get_mut(STATS_NOTE_AT..STATS_NOTE_AT + NOTE_HEADER_LEN + STATS_LEN)
    else {
        return;
    };
    note[..STATS_MAGIC.len()].copy_from_slice(STATS_MAGIC);
    note[STATS_MAGIC.len()..STATS_MAGIC.len() + NOTE_SUM_LEN]
        .copy_from_slice(&checksum(state).to_le_bytes());
    note[NOTE_HEADER_LEN - 1] = u8::try_from(STATS_LEN).unwrap_or(u8::MAX);
    note[NOTE_HEADER_LEN..].copy_from_slice(&statistics.to_bytes());
}

fn read_stats(spare: &[u8], state: &[u8]) -> Option<Stats> {
    let note = spare.get(STATS_NOTE_AT..)?;
    let header = note.get(..NOTE_HEADER_LEN)?;
    if &header[..STATS_MAGIC.len()] != STATS_MAGIC {
        return None;
    }
    let sum = &header[STATS_MAGIC.len()..STATS_MAGIC.len() + NOTE_SUM_LEN];
    if u32::from_le_bytes([sum[0], sum[1], sum[2], sum[3]]) != checksum(state) {
        return None;
    }
    let len = usize::from(header[NOTE_HEADER_LEN - 1]);
    Stats::from_bytes(note.get(NOTE_HEADER_LEN..NOTE_HEADER_LEN + len)?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use formats::progress::encode_name;

    fn layout() -> SaveLayout {
        SaveLayout {
            magic: b"TEST\0".to_vec(),
            blocks: vec![STATE_LEN, 4],
            copies: 2,
            memory_size: 2 * (STATE_LEN + 12) + 5 + 128,
        }
    }

    fn state(name: &str) -> Vec<u8> {
        let mut state = vec![0; STATE_LEN];
        let mut progress = Progress::read(&state).expect("block");
        progress.map = 4;
        progress.name = encode_name(name).0;
        progress.write(&mut state).expect("block");
        state
    }

    fn corrupt(image: &mut [u8], copy: usize) {
        image[5 + copy * (STATE_LEN + 12) + 1] ^= 0x55;
    }

    #[test]
    fn nothing_saved_or_an_unformatted_memory_is_missing() {
        let file = SaveFile::new(layout());
        assert_eq!(file.read(None), Found::Missing);
        assert_eq!(file.read(Some(vec![0xFF; 100])), Found::Missing);
        let erased = SaveMemory::erased(&layout()).bytes().to_vec();
        assert_eq!(file.read(Some(erased)), Found::Missing);
    }

    #[test]
    fn writes_both_copies_and_reads_the_first() {
        let file = SaveFile::new(layout());
        let image = file
            .write(None, &state("アトレー"), "アトレー", &Stats::default())
            .expect("fits");
        let found = file.read(Some(image));
        assert_eq!(found.notice(), None);
        let game = found.game().expect("saved");
        assert_eq!(game.player_name, "アトレー");
        assert_eq!(game.progress().expect("block").map, 4);
    }

    #[test]
    fn falls_back_to_the_second_copy_then_gives_up() {
        let file = SaveFile::new(layout());
        let mut image = file
            .write(None, &state("ア"), "ア", &Stats::default())
            .expect("fits");
        corrupt(&mut image, 0);
        let found = file.read(Some(image.clone()));
        assert!(matches!(found, Found::Restored(_)));
        assert_eq!(found.notice(), Some(NOTICE_RESTORED));
        corrupt(&mut image, 1);
        assert_eq!(file.read(Some(image)), Found::Corrupt);
    }

    #[test]
    fn keeps_the_other_blocks_of_an_existing_image() {
        let file = SaveFile::new(layout());
        let mut memory = SaveMemory::formatted(&layout());
        memory
            .set_block(&layout(), 1, 1, &[9, 9, 9, 9])
            .expect("fits");
        let image = file
            .write(
                Some(memory.bytes().to_vec()),
                &state("ア"),
                "ア",
                &Stats::default(),
            )
            .expect("fits");
        let memory = SaveMemory::from_bytes(image, &layout()).expect("image");
        assert_eq!(
            memory.block(&layout(), 1, 1),
            Ok(Some([9, 9, 9, 9].as_slice()))
        );
    }

    #[test]
    fn the_note_keeps_a_name_shift_jis_cannot_hold() {
        let file = SaveFile::new(layout());
        let image = file
            .write(None, &state("Iñigo"), "Iñigo", &Stats::default())
            .expect("fits");
        let found = file.read(Some(image));
        assert_eq!(found.game().expect("saved").player_name, "Iñigo");
    }

    #[test]
    fn a_note_left_from_another_save_is_ignored() {
        let file = SaveFile::new(layout());
        let image = file
            .write(None, &state("Iñigo"), "Iñigo", &Stats::default())
            .expect("fits");
        let mut memory = SaveMemory::from_bytes(image, &layout()).expect("image");
        memory
            .set_block(&layout(), 0, STATE_BLOCK, &state("アトレー"))
            .expect("fits");
        let found = file.read(Some(memory.bytes().to_vec()));
        assert_eq!(found.game().expect("saved").player_name, "アトレー");
    }

    #[test]
    fn the_statistics_travel_with_the_save_they_belong_to() {
        let file = SaveFile::new(layout());
        let stats = Stats {
            battles_won: 12,
            best_hit: 345,
            story_won: 1 << 41,
            ..Stats::default()
        };
        let image = file
            .write(None, &state("アトレー"), "アトレー", &stats)
            .expect("fits");
        let found = file.read(Some(image.clone()));
        assert_eq!(found.game().expect("saved").stats, stats);
        let mut memory = SaveMemory::from_bytes(image, &layout()).expect("image");
        memory
            .set_block(&layout(), 0, STATE_BLOCK, &state("ア"))
            .expect("fits");
        let found = file.read(Some(memory.bytes().to_vec()));
        assert_eq!(found.game().expect("saved").stats, Stats::default());
    }

    #[test]
    fn the_notes_can_be_left_out_and_the_game_stays() {
        let file = SaveFile::new(layout());
        let stats = Stats {
            battles_won: 3,
            ..Stats::default()
        };
        let image = file
            .write(None, &state("ア"), "Atory", &stats)
            .expect("fits");
        let strict = file.without_port_notes(image.clone());
        assert_eq!(strict.len(), image.len());
        let game = file
            .read(Some(strict.clone()))
            .game()
            .cloned()
            .expect("saved");
        assert_eq!(
            (game.player_name.as_str(), game.stats),
            ("ア", Stats::default())
        );
        let memory = SaveMemory::from_bytes(strict, &layout()).expect("image");
        assert!(memory.spare(&layout()).iter().all(|&byte| byte == ERASED));
        assert_eq!(file.without_port_notes(vec![1, 2]), vec![1, 2]);
    }
}
