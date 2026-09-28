//! The system's own dialogs, through SDL3: choosing a file, the folder
//! where a program keeps its settings for the user, and opening a web page
//! in the user's browser.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use platform::PlatformError;
use sdl3::dialog::{DialogCallback, DialogError, DialogFileFilter, show_open_file_dialog};

use crate::{Sdl3Display, backend_error};

/// A file the player is choosing in the system's dialog. The dialog runs
/// on its own; the answer arrives while the window's events are polled.
pub struct FileChoice {
    answer: Receiver<Option<PathBuf>>,
}

impl FileChoice {
    /// The answer once the dialog has closed: the file, or `None` when the
    /// player canceled it. `None` while it is still open.
    #[must_use]
    pub fn answer(&self) -> Option<Option<PathBuf>> {
        self.answer.try_recv().ok()
    }
}

impl Sdl3Display {
    /// Opens the system's dialog to choose one file, offering `filters`
    /// (a name and its extensions separated by `;`, as `("PO files",
    /// "po")`) and starting in `location` when given.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] when the dialog cannot be opened.
    pub fn choose_file(
        &self,
        filters: &[(&str, &str)],
        location: Option<&Path>,
    ) -> Result<FileChoice, PlatformError> {
        let (sender, answer) = channel();
        let filters: Vec<DialogFileFilter<'_>> = filters
            .iter()
            .map(|&(name, pattern)| DialogFileFilter { name, pattern })
            .collect();
        let callback: DialogCallback = Box::new(
            move |result: Result<Vec<PathBuf>, DialogError>, _: Option<DialogFileFilter<'_>>| {
                let chosen = result.ok().and_then(|files| files.into_iter().next());
                let _ = sender.send(chosen);
            },
        );
        show_open_file_dialog(
            &filters,
            location,
            false,
            Some(self.canvas.window()),
            callback,
        )
        .map_err(backend_error)?;
        Ok(FileChoice { answer })
    }
}

/// The folder where the program named `app` keeps the user's settings,
/// created if needed: the system's usual place for them.
///
/// # Errors
///
/// Returns [`PlatformError`] when the system has none.
pub fn preferences_dir(organization: &str, app: &str) -> Result<PathBuf, PlatformError> {
    sdl3::filesystem::get_pref_path(organization, app).map_err(backend_error)
}

/// Reads the whole file at `path`, which may also be the `content://` URI
/// Android's dialog gives for a document.
///
/// # Errors
///
/// Returns [`PlatformError`] when the file cannot be opened or read.
pub fn read_file(path: &Path) -> Result<Vec<u8>, PlatformError> {
    let mut stream = sdl3::iostream::IOStream::from_file(path, "rb").map_err(backend_error)?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut bytes).map_err(backend_error)?;
    Ok(bytes)
}

/// Opens `url` in the user's web browser.
///
/// # Errors
///
/// Returns [`PlatformError`] when the system cannot open it.
pub fn open_url(url: &str) -> Result<(), PlatformError> {
    sdl3::url::open_url(url).map_err(backend_error)
}
