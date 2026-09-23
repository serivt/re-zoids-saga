//! Where the game keeps its save memory between runs: one image, read at
//! start and replaced whole on every save.

use thiserror::Error;

/// Why the save memory could not be read or written.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Reading failed for a reason other than there being nothing saved.
    #[error("cannot read the save: {0}")]
    Read(String),
    /// Writing failed; what was stored before is still there.
    #[error("cannot write the save: {0}")]
    Write(String),
}

/// A place for the save memory image.
pub trait SaveStorage {
    /// The stored image, or `None` when nothing was ever saved.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Read`] when the image exists but cannot be
    /// read.
    fn load(&self) -> Result<Option<Vec<u8>>, StorageError>;

    /// Replaces the stored image with `bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Write`] when the image cannot be written.
    fn store(&mut self, bytes: &[u8]) -> Result<(), StorageError>;
}
