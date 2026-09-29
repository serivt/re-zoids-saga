//! The saves kept beside the ROM, copied to or from another place: on
//! Android the app's own folder is out of reach, so its launcher exports a
//! slot's `.sav` through the system's dialog (to an emulator, another
//! device or a backup) and imports one, which must read as a save of the
//! game before it replaces the slot; the save it replaces is kept beside
//! it as `.bak`.
//!
//! Source of knowledge: this project's own design.

use std::path::{Path, PathBuf};

use game_core::save::SaveFile;
use platform_sdl3::slot_path;

/// The extension of a save beside its ROM, and of the one an import
/// replaces.
pub const SAVE_EXTENSION: &str = "sav";
const BACKUP_EXTENSION: &str = "bak";

/// Where save slot `slot` (from 0) of the ROM at `rom` is kept.
#[must_use]
pub fn slot_file(rom: &Path, slot: usize) -> PathBuf {
    slot_path(&rom.with_extension(SAVE_EXTENSION), slot)
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

/// The name an exported slot is proposed under.
#[must_use]
pub fn export_name(slot: usize) -> String {
    format!("Zoids Saga (slot {}).{SAVE_EXTENSION}", slot + 1)
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
        assert_eq!(export_name(1), "Zoids Saga (slot 2).sav");
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
        let _ = std::fs::remove_dir_all(&folder);
    }
}
