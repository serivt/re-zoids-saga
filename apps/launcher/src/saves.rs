//! The saves kept beside the ROM, copied to or from another place through
//! the system's dialog: a slot's save, or the enhanced mode's autosave, is
//! exported for an emulator or a flash cart (`.sav`), for `RetroArch`
//! (`.srm`, the same bytes) or without the port's notes (the memory the
//! cartridge itself would hold); a save chosen to import must read as a
//! save of the game, and the player sees what it holds against what the
//! slot holds before it replaces the slot, whose save is kept beside it as
//! `.bak`. On Android the app's own folder is out of reach, so this is the
//! only way in or out. The enhanced mode's achievements, which every slot
//! shares, are kept beside them in a file of their own.
//!
//! Source of knowledge: this project's own design.

use std::path::{Path, PathBuf};

use game_core::save::{Found, SaveFile};
use game_core::slots::{Slot, SlotSummary};
use platform_sdl3::{autosave_path, slot_path};

/// The extension of a save beside its ROM, and of the one an import
/// replaces.
pub const SAVE_EXTENSION: &str = "sav";
/// The extension `RetroArch` gives a game's save memory.
const RETROARCH_EXTENSION: &str = "srm";
const BACKUP_EXTENSION: &str = "bak";
/// The extension of the file beside the saves that keeps the enhanced
/// mode's achievements, which every slot shares.
const ACHIEVEMENTS_EXTENSION: &str = "achievements";

/// Where save slot `slot` (from 0) of the ROM at `rom` is kept.
#[must_use]
pub fn slot_file(rom: &Path, slot: usize) -> PathBuf {
    slot_path(&rom.with_extension(SAVE_EXTENSION), slot)
}

/// Where the enhanced mode's autosave of the ROM at `rom` is kept.
#[must_use]
pub fn autosave_file(rom: &Path) -> PathBuf {
    autosave_path(&rom.with_extension(SAVE_EXTENSION))
}

/// What an export writes, and for what.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    /// The save as it is, for an emulator or a flash cart: `.sav`.
    Emulator,
    /// The same bytes under `RetroArch`'s extension: `.srm`.
    RetroArch,
    /// The save without the port's notes, the memory the cartridge itself
    /// would hold: `.sav`.
    Strict,
}

impl ExportKind {
    /// The extension of the file it writes.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Emulator | Self::Strict => SAVE_EXTENSION,
            Self::RetroArch => RETROARCH_EXTENSION,
        }
    }

    /// The bytes to write for the save `bytes` of the game whose ROM is
    /// `rom_bytes`.
    #[must_use]
    pub fn bytes(self, rom_bytes: &[u8], bytes: Vec<u8>) -> Vec<u8> {
        match (self, extraction::saga_save::save_layout(rom_bytes)) {
            (Self::Strict, Ok(layout)) => SaveFile::new(layout).without_port_notes(bytes),
            _ => bytes,
        }
    }
}

/// What a save holds, for the player to see before an import: its game's
/// level, area and money, and the time played when the port counted it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveSummary {
    /// The game's fields.
    pub game: SlotSummary,
    /// Hours and minutes played, when the save keeps the port's
    /// statistics.
    pub played: Option<(u32, u32)>,
}

/// What `bytes`, a save of the game whose ROM is `rom_bytes`, holds; `None`
/// when they hold no game to continue.
#[must_use]
pub fn summary(rom_bytes: &[u8], bytes: &[u8]) -> Option<SaveSummary> {
    let layout = extraction::saga_save::save_layout(rom_bytes).ok()?;
    let found = SaveFile::new(layout).read(Some(bytes.to_vec()));
    let Slot::Game(game) = Slot::from_found(&found) else {
        return None;
    };
    let played = match &found {
        Found::Saved(saved) | Found::Restored(saved) if saved.stats.play_frames > 0 => {
            let (hours, minutes, _) = saved.stats.play_time();
            Some((hours, minutes))
        }
        _ => None,
    };
    Some(SaveSummary { game, played })
}

/// Where the achievements of the games saved at `save` are kept: beside
/// it, with its name.
#[must_use]
pub fn achievements_file(save: &Path) -> PathBuf {
    save.with_extension(ACHIEVEMENTS_EXTENSION)
}

/// Whether `bytes` read as a save of the game whose ROM is `rom_bytes`,
/// with a game to continue.
#[must_use]
pub fn is_save(rom_bytes: &[u8], bytes: &[u8]) -> bool {
    let Ok(layout) = extraction::saga_save::save_layout(rom_bytes) else {
        return false;
    };
    SaveFile::new(layout)
        .read(Some(bytes.to_vec()))
        .game()
        .is_some()
}

/// Puts `bytes`, a save, in slot `slot` of the ROM at `rom`, keeping the
/// one it replaces as `.bak` beside it.
///
/// # Errors
///
/// Returns the reason when the ROM cannot be read, `bytes` are not a save
/// of it, or the slot cannot be written.
pub fn import(rom: &Path, slot: usize, bytes: &[u8]) -> Result<(), String> {
    let rom_bytes = std::fs::read(rom).map_err(|error| error.to_string())?;
    if !is_save(&rom_bytes, bytes) {
        return Err("not a save of this game".to_owned());
    }
    let kept = slot_file(rom, slot);
    if kept.is_file() {
        std::fs::copy(&kept, kept.with_extension(BACKUP_EXTENSION))
            .map_err(|error| error.to_string())?;
    }
    let temporary = kept.with_extension("import");
    std::fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, &kept).map_err(|error| error.to_string())
}

/// The name an export is proposed under: the ROM's, so an emulator or
/// `RetroArch` loads it beside the ROM by itself, with the kind's extension.
#[must_use]
pub fn export_name(rom: &Path, kind: ExportKind) -> String {
    let stem = rom.file_stem().map_or_else(
        || "Zoids Saga".to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    format!("{stem}.{}", kind.extension())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn the_slots_sit_beside_the_rom() {
        let rom = Path::new("/data/rom.gba");
        assert_eq!(slot_file(rom, 0), PathBuf::from("/data/rom.sav"));
        assert_eq!(slot_file(rom, 2), PathBuf::from("/data/rom.3.sav"));
        assert_eq!(export_name(rom, ExportKind::Emulator), "rom.sav");
        assert_eq!(export_name(rom, ExportKind::RetroArch), "rom.srm");
        assert_eq!(autosave_file(rom), PathBuf::from("/data/rom.auto.sav"));
        assert_eq!(
            achievements_file(&slot_file(rom, 0)),
            PathBuf::from("/data/rom.achievements")
        );
    }

    #[test]
    fn only_a_save_of_the_game_is_imported() {
        let folder = std::env::temp_dir().join(format!("re-zoids-saves-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let rom = folder.join("rom.gba");
        std::fs::write(&rom, [0u8; 64]).unwrap();
        std::fs::write(slot_file(&rom, 0), b"kept").unwrap();
        let blank = vec![0u8; 32 * 1024];
        assert!(!is_save(&[0u8; 64], &blank));
        assert!(import(&rom, 0, &blank).is_err());
        assert_eq!(std::fs::read(slot_file(&rom, 0)).unwrap(), b"kept");
        assert!(import(&folder.join("missing.gba"), 0, b"x").is_err());
        assert_eq!(summary(&[0u8; 64], &blank), None);
        assert_eq!(ExportKind::Strict.bytes(&[0u8; 64], vec![7]), vec![7]);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
