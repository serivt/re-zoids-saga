//! What the launcher remembers between runs: the ROM and the translation
//! last played and the keys chosen for the buttons, one `key=value` line
//! each, in the user's settings folder.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use platform::Button;

const ROM_KEY: &str = "rom";
const TRANSLATION_KEY: &str = "translation";
/// A button's key: `key.<button>=<key name>`.
const BUTTON_PREFIX: &str = "key.";
/// The settings folder's organization and program names, and the file in
/// it.
pub const ORGANIZATION: &str = "re-zoids-saga";
/// See [`ORGANIZATION`].
pub const APP: &str = "launcher";
/// See [`ORGANIZATION`].
pub const FILE_NAME: &str = "launcher.cfg";

/// The launcher's remembered choices.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    /// The ROM last played.
    pub rom: Option<PathBuf>,
    /// The translation last played with, if any.
    pub translation: Option<PathBuf>,
    /// The keys the player chose for buttons, by the backend's names; the
    /// others keep their defaults.
    pub keys: Vec<(Button, String)>,
}

impl Settings {
    /// Reads `text`; unknown keys and malformed lines are ignored.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if let Some(button) = key.strip_prefix(BUTTON_PREFIX).and_then(Button::from_name) {
                if !value.is_empty() {
                    settings.keys.retain(|(bound, _)| *bound != button);
                    settings.keys.push((button, value.to_owned()));
                }
                continue;
            }
            let value = (!value.is_empty()).then(|| PathBuf::from(value));
            match key {
                ROM_KEY => settings.rom = value,
                TRANSLATION_KEY => settings.translation = value,
                _ => {}
            }
        }
        settings
    }

    /// The settings as the file holds them.
    #[must_use]
    pub fn to_text(&self) -> String {
        let path = |path: &Option<PathBuf>| {
            path.as_deref()
                .map(Path::to_string_lossy)
                .unwrap_or_default()
                .into_owned()
        };
        let mut text = format!(
            "{ROM_KEY}={}\n{TRANSLATION_KEY}={}\n",
            path(&self.rom),
            path(&self.translation)
        );
        for (button, key) in &self.keys {
            let _ = writeln!(text, "{BUTTON_PREFIX}{}={key}", button.name());
        }
        text
    }

    /// Reads the file at `path`; nothing is remembered when it is missing.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    /// Writes the file at `path`.
    ///
    /// # Errors
    ///
    /// Returns the error of the write.
    pub fn store(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, self.to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_the_rom_and_the_translation() {
        let settings = Settings {
            rom: Some(PathBuf::from("/games/Zoids Saga.gba")),
            translation: None,
            keys: vec![
                (Button::A, "Space".to_owned()),
                (Button::Up, "W".to_owned()),
            ],
        };
        assert_eq!(Settings::parse(&settings.to_text()), settings);
        let read = Settings::parse("# notes\ntranslation=es.po\nrom=a=b.gba\nother=1\n");
        assert_eq!(read.rom, Some(PathBuf::from("a=b.gba")));
        assert_eq!(read.translation, Some(PathBuf::from("es.po")));
        assert_eq!(Settings::parse(""), Settings::default());
        let keys = Settings::parse("key.b=Q\nkey.b=E\nkey.turbo=T\nkey.a=\n").keys;
        assert_eq!(keys, [(Button::B, "E".to_owned())]);
    }
}
