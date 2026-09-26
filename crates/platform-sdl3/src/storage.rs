//! The save memory as a file on disk, the way emulators keep it next to
//! the ROM, so the same `.sav` works in both.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use platform::{SaveStorage, StorageError};

const TEMPORARY_SUFFIX: &str = "tmp";

/// Where save slot `slot` (from 0) of the save at `base` is kept: the
/// first slot is `base` itself, so the usual `.sav` stays where emulators
/// look for it; slot `n` adds `.n+1` before the extension, as
/// `game.2.sav`.
#[must_use]
pub fn slot_path(base: &Path, slot: usize) -> PathBuf {
    if slot == 0 {
        return base.to_path_buf();
    }
    let mut name = base.file_stem().unwrap_or_default().to_os_string();
    name.push(format!(".{}", slot + 1));
    if let Some(extension) = base.extension() {
        name.push(".");
        name.push(extension);
    }
    base.with_file_name(name)
}

/// A save file; writes go to a temporary file first and replace the save
/// only once complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStorage {
    path: PathBuf,
}

impl FileStorage {
    /// Keeps the save at `path`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Where the save is kept.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn temporary(&self) -> PathBuf {
        let mut name = self.path.clone().into_os_string();
        name.push(".");
        name.push(TEMPORARY_SUFFIX);
        PathBuf::from(name)
    }
}

impl SaveStorage for FileStorage {
    fn load(&self) -> Result<Option<Vec<u8>>, StorageError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StorageError::Read(format!(
                "{}: {error}",
                self.path.display()
            ))),
        }
    }

    fn store(&mut self, bytes: &[u8]) -> Result<(), StorageError> {
        let temporary = self.temporary();
        let failed = |error: std::io::Error| {
            StorageError::Write(format!("{}: {error}", self.path.display()))
        };
        std::fs::write(&temporary, bytes).map_err(failed)?;
        std::fs::rename(&temporary, &self.path).map_err(failed)
    }

    fn modified(&self) -> Option<std::time::SystemTime> {
        std::fs::metadata(&self.path).ok()?.modified().ok()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("re-zoids-storage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temporary directory");
        dir.join(name)
    }

    #[test]
    fn a_missing_file_holds_nothing() {
        let storage = FileStorage::new(scratch("missing.sav"));
        assert!(storage.load().expect("readable").is_none());
    }

    #[test]
    fn the_first_slot_is_the_save_itself_and_the_others_are_numbered() {
        let base = Path::new("roms/game.sav");
        assert_eq!(slot_path(base, 0), PathBuf::from("roms/game.sav"));
        assert_eq!(slot_path(base, 1), PathBuf::from("roms/game.2.sav"));
        assert_eq!(slot_path(base, 3), PathBuf::from("roms/game.4.sav"));
        assert_eq!(slot_path(Path::new("save"), 2), PathBuf::from("save.3"));
    }

    #[test]
    fn stores_and_loads_the_image() {
        let path = scratch("round-trip.sav");
        let mut storage = FileStorage::new(&path);
        storage.store(&[1, 2, 3]).expect("writable");
        storage.store(&[4, 5]).expect("writable");
        assert_eq!(storage.load().expect("readable"), Some(vec![4, 5]));
        assert!(storage.modified().is_some());
        assert!(!storage.temporary().exists());
        std::fs::remove_file(path).expect("removable");
    }
}
