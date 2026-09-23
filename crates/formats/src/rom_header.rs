//! Cartridge header of a GBA ROM, as laid out in the first `0xC0` bytes of the
//! image (GBATEK, "GBA Cartridge Header").

use core::ops::Range;

use thiserror::Error;

/// Size in bytes of the cartridge header at the start of every ROM image.
pub const HEADER_LEN: usize = 0xC0;

const TITLE: Range<usize> = 0xA0..0xAC;
const GAME_CODE: Range<usize> = 0xAC..0xB0;
const MAKER_CODE: Range<usize> = 0xB0..0xB2;
const FIXED_VALUE_OFFSET: usize = 0xB2;
const FIXED_VALUE: u8 = 0x96;
const VERSION_OFFSET: usize = 0xBC;
const COMPLEMENT_OFFSET: usize = 0xBD;
const COMPLEMENT_INPUT: Range<usize> = 0xA0..0xBD;
const COMPLEMENT_BIAS: u8 = 0x19;

/// Identifying fields of a cartridge header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomHeader {
    /// Internal title, up to 12 ASCII characters.
    pub title: String,
    /// Four-character product code, e.g. `ATZJ`.
    pub game_code: String,
    /// Two-character publisher code, e.g. `DA` for Tomy.
    pub maker_code: String,
    /// Software revision, `0` for the first release.
    pub version: u8,
}

/// Why a byte slice is not a valid cartridge header.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HeaderError {
    /// The image ends before the header does.
    #[error("ROM is {actual} bytes, shorter than the {HEADER_LEN}-byte header")]
    TooShort {
        /// Length of the image that was given.
        actual: usize,
    },
    /// The byte the hardware requires to be `0x96` holds another value.
    #[error("fixed header byte is {actual:#04x}, expected {FIXED_VALUE:#04x}")]
    BadFixedValue {
        /// Value found at the fixed-byte offset.
        actual: u8,
    },
    /// The header complement does not match its contents.
    #[error("header complement is {actual:#04x}, expected {expected:#04x}")]
    BadComplement {
        /// Complement stored in the header.
        actual: u8,
        /// Complement computed from the header contents.
        expected: u8,
    },
}

impl RomHeader {
    /// Parses and validates the header at the start of `rom`.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError`] when the image is too short, the fixed byte is
    /// wrong, or the complement does not match the header contents.
    pub fn parse(rom: &[u8]) -> Result<Self, HeaderError> {
        let header = rom
            .get(..HEADER_LEN)
            .ok_or(HeaderError::TooShort { actual: rom.len() })?;
        validate_fixed_value(header)?;
        validate_complement(header)?;
        Ok(Self {
            title: ascii_field(&header[TITLE]),
            game_code: ascii_field(&header[GAME_CODE]),
            maker_code: ascii_field(&header[MAKER_CODE]),
            version: header[VERSION_OFFSET],
        })
    }
}

/// Computes the complement byte the hardware expects at offset `0xBD` for a
/// header of at least `0xBD` bytes.
#[must_use]
pub fn complement(header: &[u8]) -> u8 {
    let sum = header[COMPLEMENT_INPUT]
        .iter()
        .fold(0u8, |acc, byte| acc.wrapping_add(*byte));
    sum.wrapping_add(COMPLEMENT_BIAS).wrapping_neg()
}

fn validate_fixed_value(header: &[u8]) -> Result<(), HeaderError> {
    let actual = header[FIXED_VALUE_OFFSET];
    if actual == FIXED_VALUE {
        Ok(())
    } else {
        Err(HeaderError::BadFixedValue { actual })
    }
}

fn validate_complement(header: &[u8]) -> Result<(), HeaderError> {
    let expected = complement(header);
    let actual = header[COMPLEMENT_OFFSET];
    if actual == expected {
        Ok(())
    } else {
        Err(HeaderError::BadComplement { actual, expected })
    }
}

fn ascii_field(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches(['\0', ' '])
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_header(title: &str, game_code: &str, maker: &str, version: u8) -> Vec<u8> {
        let mut header = vec![0u8; HEADER_LEN];
        header[TITLE][..title.len()].copy_from_slice(title.as_bytes());
        header[GAME_CODE].copy_from_slice(game_code.as_bytes());
        header[MAKER_CODE].copy_from_slice(maker.as_bytes());
        header[FIXED_VALUE_OFFSET] = FIXED_VALUE;
        header[VERSION_OFFSET] = version;
        header[COMPLEMENT_OFFSET] = complement(&header);
        header
    }

    #[test]
    fn parses_a_valid_header() {
        let rom = synthetic_header("TESTTITLE", "AXYZ", "01", 2);
        let parsed = RomHeader::parse(&rom);
        assert_eq!(
            parsed,
            Ok(RomHeader {
                title: "TESTTITLE".to_owned(),
                game_code: "AXYZ".to_owned(),
                maker_code: "01".to_owned(),
                version: 2,
            })
        );
    }

    #[test]
    fn rejects_a_truncated_image() {
        let rom = vec![0u8; HEADER_LEN - 1];
        assert_eq!(
            RomHeader::parse(&rom),
            Err(HeaderError::TooShort {
                actual: HEADER_LEN - 1
            })
        );
    }

    #[test]
    fn rejects_a_wrong_fixed_byte() {
        let mut rom = synthetic_header("T", "AAAA", "00", 0);
        rom[FIXED_VALUE_OFFSET] = 0;
        assert_eq!(
            RomHeader::parse(&rom),
            Err(HeaderError::BadFixedValue { actual: 0 })
        );
    }

    #[test]
    fn rejects_a_wrong_complement() {
        let mut rom = synthetic_header("T", "AAAA", "00", 0);
        let expected = rom[COMPLEMENT_OFFSET];
        rom[COMPLEMENT_OFFSET] = expected.wrapping_add(1);
        assert_eq!(
            RomHeader::parse(&rom),
            Err(HeaderError::BadComplement {
                actual: expected.wrapping_add(1),
                expected,
            })
        );
    }

    #[test]
    fn complement_changes_with_any_header_byte() {
        let base = synthetic_header("T", "AAAA", "00", 0);
        let mut changed = base.clone();
        changed[VERSION_OFFSET] = 1;
        assert_ne!(complement(&base), complement(&changed));
    }
}
