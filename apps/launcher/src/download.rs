//! Translations downloaded from the project's translations repository: the
//! languages it offers (`po/languages.json`) and each one's PO file, fetched
//! over HTTPS on a thread of their own only when the player asks, checked,
//! and kept in the launcher's folder as `<code>.po`, where they work
//! offline from then on.
//!
//! Source of knowledge: this project's own design.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use game_core::Translation;
use serde::Deserialize;

/// Where the repository's files are served from, as they are on its main
/// branch.
const BASE_URL: &str = "https://raw.githubusercontent.com/serivt/re-zoids-saga-translations/main/";
const INDEX: &str = "po/languages.json";
/// How long a download may take, and how large its files may be.
const TIMEOUT: Duration = Duration::from_secs(30);
const INDEX_LIMIT: u64 = 64 * 1024;
const TRANSLATION_LIMIT: u64 = 16 * 1024 * 1024;
const TRANSLATION_EXTENSION: &str = "po";

/// A language the repository offers.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Language {
    /// Its code, which names the file it is kept in: `es` for `es.po`.
    pub code: String,
    /// Its name in itself: `Español`.
    pub name: String,
    /// Its PO file in the repository: `po/es.po`.
    pub file: String,
}

impl Language {
    /// Where the downloaded translation is kept in `folder`.
    #[must_use]
    pub fn kept_in(&self, folder: &Path) -> PathBuf {
        folder.join(format!("{}.{TRANSLATION_EXTENSION}", self.code))
    }
}

#[derive(Deserialize)]
struct Index {
    languages: Vec<Language>,
}

/// The languages `text`, the repository's index, lists. A language whose
/// code is not letters, digits, `-` and `_`, or whose file is not
/// `po/<code>.po`, is left out, so no name from the network reaches a path
/// but those.
///
/// # Errors
///
/// Returns the reason when `text` is not an index.
pub fn parse_index(text: &str) -> Result<Vec<Language>, String> {
    let index: Index = serde_json::from_str(text).map_err(|error| error.to_string())?;
    Ok(index
        .languages
        .into_iter()
        .filter(|language| {
            !language.code.is_empty()
                && language
                    .code
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
                && language.file == format!("po/{}.{TRANSLATION_EXTENSION}", language.code)
                && !language.name.trim().is_empty()
        })
        .collect())
}

/// Checks that `bytes` read as a translation, then writes them where
/// `language` is kept in `folder`, replacing what was there only once the
/// whole file is written.
///
/// # Errors
///
/// Returns the reason when `bytes` are not a translation or cannot be
/// written.
pub fn keep(bytes: &[u8], language: &Language, folder: &Path) -> Result<PathBuf, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let translation = Translation::from_po(text).map_err(|error| error.to_string())?;
    if translation.is_empty() {
        return Err("the file has no translated messages".to_owned());
    }
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let kept = language.kept_in(folder);
    let temporary = kept.with_extension("download");
    std::fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, &kept).map_err(|error| error.to_string())?;
    Ok(kept)
}

/// What a download brought back.
#[derive(Debug)]
pub enum Answer {
    /// The languages the repository offers, or why they could not be read.
    Languages(Result<Vec<Language>, String>),
    /// Where a language's translation is now kept, or why it could not be.
    Kept(Result<PathBuf, String>),
}

/// A download running on its own thread; its answer arrives while the
/// screen goes on.
pub struct Download {
    answer: Receiver<Answer>,
}

impl Download {
    /// Starts reading the languages the repository offers.
    #[must_use]
    pub fn languages() -> Self {
        Self::start(|| {
            Answer::Languages(
                fetch(INDEX, INDEX_LIMIT)
                    .and_then(|bytes| parse_index(&String::from_utf8_lossy(&bytes))),
            )
        })
    }

    /// Starts downloading `language`'s translation into `folder`.
    #[must_use]
    pub fn translation(language: Language, folder: PathBuf) -> Self {
        Self::start(move || {
            Answer::Kept(
                fetch(&language.file, TRANSLATION_LIMIT)
                    .and_then(|bytes| keep(&bytes, &language, &folder)),
            )
        })
    }

    /// The answer once the download has finished, `None` while it runs.
    #[must_use]
    pub fn answer(&self) -> Option<Answer> {
        self.answer.try_recv().ok()
    }

    fn start(work: impl FnOnce() -> Answer + Send + 'static) -> Self {
        let (sender, answer) = channel();
        std::thread::spawn(move || {
            let _ = sender.send(work());
        });
        Self { answer }
    }
}

/// The file at `path` of the repository, of at most `limit` bytes.
fn fetch(path: &str, limit: u64) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .into();
    let mut response = agent
        .get(&format!("{BASE_URL}{path}"))
        .call()
        .map_err(|error| error.to_string())?;
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const INDEX_TEXT: &str = r#"{ "languages": [
        { "code": "en", "name": "English", "file": "po/en.po" },
        { "code": "pt_BR", "name": "Português", "file": "po/pt_BR.po" },
        { "code": "../x", "name": "Bad", "file": "po/../x.po" },
        { "code": "fr", "name": "Français", "file": "elsewhere/fr.po" },
        { "code": "de", "name": " ", "file": "po/de.po" }
    ] }"#;

    fn language(code: &str) -> Language {
        Language {
            code: code.to_owned(),
            name: code.to_uppercase(),
            file: format!("po/{code}.po"),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("re-zoids-download-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_index_keeps_only_well_formed_languages() {
        let languages = parse_index(INDEX_TEXT).unwrap();
        let codes: Vec<&str> = languages.iter().map(|l| l.code.as_str()).collect();
        assert_eq!(codes, ["en", "pt_BR"]);
        assert_eq!(languages[1].name, "Português");
        assert!(parse_index("not json").is_err());
        assert!(parse_index("{}").is_err());
    }

    #[test]
    fn a_language_is_kept_under_its_code() {
        assert_eq!(
            language("es").kept_in(Path::new("/prefs")),
            PathBuf::from("/prefs/es.po")
        );
    }

    /// Needs the network: `cargo test -p launcher -- --ignored`.
    #[test]
    #[ignore = "downloads from the translations' repository"]
    fn downloads_a_translation_from_the_repository() {
        let folder = scratch("network");
        let download = Download::translation(language("en"), folder.clone());
        let answer = (0..300).find_map(|_| {
            std::thread::sleep(Duration::from_millis(100));
            download.answer()
        });
        let Some(Answer::Kept(Ok(kept))) = answer else {
            panic!("no translation: {answer:?}");
        };
        let text = std::fs::read_to_string(&kept).unwrap();
        assert!(Translation::from_po(&text).unwrap().len() > 1000);
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_translation_is_kept_only_when_it_reads() {
        let folder = scratch("keep");
        let po = "msgid \"\"\nmsgstr \"\"\n\"Language: es\\n\"\n\nmsgctxt \"port/launcher/play\"\nmsgid \"port/launcher/play\"\nmsgstr \"Jugar\"\n";
        let kept = keep(po.as_bytes(), &language("es"), &folder).unwrap();
        assert_eq!(kept, folder.join("es.po"));
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), po);
        assert!(keep(b"msgid \"\"\nmsgstr \"\"\n", &language("fr"), &folder).is_err());
        assert!(keep(b"nonsense", &language("fr"), &folder).is_err());
        assert!(keep(&[0xFF, 0xFE], &language("fr"), &folder).is_err());
        assert!(!folder.join("fr.po").exists());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
