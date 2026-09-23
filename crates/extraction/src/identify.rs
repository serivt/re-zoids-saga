//! Recognizes which Zoids title a ROM image is, from its cartridge header and
//! the SHA-1 of the whole image.

use core::fmt;

use formats::{HeaderError, RomHeader};
use sha1::{Digest, Sha1};
use thiserror::Error;

/// A Zoids title this project can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Title {
    /// Zoids Saga (Japan, 2001).
    Saga,
    /// Zoids Saga II (Japan) / Zoids Legacy (USA).
    Legacy,
    /// Zoids Saga Fuzors (Japan, 2005).
    Fuzors,
}

impl fmt::Display for Title {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Saga => "Zoids Saga",
            Self::Legacy => "Zoids Legacy",
            Self::Fuzors => "Zoids Saga Fuzors",
        })
    }
}

/// A specific dump this project has been validated against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownRelease {
    /// Title the dump belongs to.
    pub title: Title,
    /// Product code in the cartridge header.
    pub game_code: &'static str,
    /// Region the cartridge was sold in.
    pub region: &'static str,
    /// Software revision in the cartridge header.
    pub version: u8,
    /// Lowercase hexadecimal SHA-1 of the whole image.
    pub sha1: &'static str,
}

const KNOWN_RELEASES: &[KnownRelease] = &[
    KnownRelease {
        title: Title::Saga,
        game_code: "ATZJ",
        region: "Japan",
        version: 1,
        sha1: "70bb546a7d00126d452c1d2c1ccddafb2cb91b37",
    },
    KnownRelease {
        title: Title::Fuzors,
        game_code: "BZFJ",
        region: "Japan",
        version: 0,
        sha1: "f5269aba2e5f587aa2f851f35e96c1cbc5e1a78a",
    },
];

/// Result of identifying a ROM image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identification {
    /// Title recognized from the product code.
    pub title: Title,
    /// Parsed cartridge header.
    pub header: RomHeader,
    /// Lowercase hexadecimal SHA-1 of the whole image.
    pub sha1: String,
    /// The validated dump this image matches, if any.
    pub known_release: Option<KnownRelease>,
}

impl Identification {
    /// Whether the image is byte-for-byte one of the validated dumps.
    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.known_release.is_some()
    }
}

/// Why an image could not be identified as a supported title.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdentifyError {
    /// The cartridge header is malformed.
    #[error(transparent)]
    Header(#[from] HeaderError),
    /// The header is valid but belongs to a game this project does not know.
    #[error("unsupported game code {game_code:?} (title {title:?})")]
    UnsupportedGame {
        /// Product code found in the header.
        game_code: String,
        /// Internal title found in the header.
        title: String,
    },
}

/// Identifies the title of a whole ROM image.
///
/// # Errors
///
/// Returns [`IdentifyError`] when the header is malformed or the product code
/// is not one of the supported titles.
pub fn identify(rom: &[u8]) -> Result<Identification, IdentifyError> {
    let header = RomHeader::parse(rom)?;
    let title =
        title_for_game_code(&header.game_code).ok_or_else(|| IdentifyError::UnsupportedGame {
            game_code: header.game_code.clone(),
            title: header.title.clone(),
        })?;
    let sha1 = sha1_hex(rom);
    let known_release = KNOWN_RELEASES
        .iter()
        .copied()
        .find(|release| release.sha1 == sha1);
    Ok(Identification {
        title,
        header,
        sha1,
        known_release,
    })
}

fn title_for_game_code(game_code: &str) -> Option<Title> {
    KNOWN_RELEASES
        .iter()
        .find(|release| release.game_code == game_code)
        .map(|release| release.title)
}

fn sha1_hex(bytes: &[u8]) -> String {
    use core::fmt::Write;

    Sha1::digest(bytes)
        .iter()
        .fold(String::with_capacity(40), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use formats::rom_header::{HEADER_LEN, complement};

    const TITLE_OFFSET: usize = 0xA0;
    const GAME_CODE_OFFSET: usize = 0xAC;
    const MAKER_CODE_OFFSET: usize = 0xB0;
    const FIXED_VALUE_OFFSET: usize = 0xB2;
    const VERSION_OFFSET: usize = 0xBC;
    const COMPLEMENT_OFFSET: usize = 0xBD;

    fn synthetic_rom(game_code: &str) -> Vec<u8> {
        let mut rom = vec![0u8; HEADER_LEN + 16];
        rom[TITLE_OFFSET..TITLE_OFFSET + 4].copy_from_slice(b"TEST");
        rom[GAME_CODE_OFFSET..GAME_CODE_OFFSET + 4].copy_from_slice(game_code.as_bytes());
        rom[MAKER_CODE_OFFSET..MAKER_CODE_OFFSET + 2].copy_from_slice(b"DA");
        rom[FIXED_VALUE_OFFSET] = 0x96;
        rom[VERSION_OFFSET] = 1;
        rom[COMPLEMENT_OFFSET] = complement(&rom);
        rom
    }

    #[test]
    fn recognizes_saga_by_game_code_without_a_known_hash() {
        let identification = identify(&synthetic_rom("ATZJ")).unwrap();
        assert_eq!(identification.title, Title::Saga);
        assert!(!identification.is_verified());
        assert_eq!(identification.sha1.len(), 40);
    }

    #[test]
    fn recognizes_fuzors_by_game_code() {
        let identification = identify(&synthetic_rom("BZFJ")).unwrap();
        assert_eq!(identification.title, Title::Fuzors);
    }

    #[test]
    fn rejects_an_unknown_game_code() {
        let error = identify(&synthetic_rom("AXYZ")).unwrap_err();
        assert_eq!(
            error,
            IdentifyError::UnsupportedGame {
                game_code: "AXYZ".to_owned(),
                title: "TEST".to_owned(),
            }
        );
    }

    #[test]
    fn propagates_header_errors() {
        let error = identify(&[0u8; 8]).unwrap_err();
        assert_eq!(
            error,
            IdentifyError::Header(HeaderError::TooShort { actual: 8 })
        );
    }

    #[test]
    fn known_releases_have_well_formed_hashes() {
        for release in KNOWN_RELEASES {
            assert_eq!(release.sha1.len(), 40);
            assert!(release.sha1.bytes().all(|b| b.is_ascii_hexdigit()));
            assert_eq!(release.game_code.len(), 4);
        }
    }
}
