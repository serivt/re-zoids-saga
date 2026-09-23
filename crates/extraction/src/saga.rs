//! ROM layout of Zoids Saga (Japan, Rev 1).

use formats::font::{FontError, Glyph, GlyphIndex, RANGE_ENTRY_LEN, TILE_LEN};
use thiserror::Error;

use crate::string_table::StringTable;

/// The script string tables located so far, in the order they appear in the ROM.
pub const STRING_TABLES: &[StringTable] = &[
    StringTable {
        name: "name",
        offset: 0x0067_601C,
        count: 248,
    },
    StringTable {
        name: "item",
        offset: 0x0067_63FC,
        count: 148,
    },
    StringTable {
        name: "dialogue",
        offset: 0x0074_FCF4,
        count: 977,
    },
    StringTable {
        name: "battle",
        offset: 0x0075_5D30,
        count: 198,
    },
    StringTable {
        name: "menu",
        offset: 0x0075_B388,
        count: 156,
    },
];

/// Looks up a string table by name.
#[must_use]
pub fn string_table(name: &str) -> Option<&'static StringTable> {
    STRING_TABLES.iter().find(|table| table.name == name)
}

const FONT_RANGE_TABLE_OFFSET: usize = 0x006B_FAC8;
const FONT_RANGE_COUNT: usize = 111;
const FALLBACK_GLYPH_TOP: usize = 0x0068_3D58;
const FALLBACK_GLYPH_BOTTOM: usize = 0x0068_3D78;

/// Why the font could not be read from the ROM.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FontReadError {
    /// The ROM is too short to hold the font tables.
    #[error("ROM of {len} bytes is too short for the font tables")]
    TooShort {
        /// ROM length.
        len: usize,
    },
    /// The range table is malformed.
    #[error(transparent)]
    Font(#[from] FontError),
}

/// Reads the text font's glyph index and its fallback glyph (drawn for
/// characters the font lacks).
///
/// # Errors
///
/// Returns [`FontReadError`] when the ROM is too short or the table is malformed.
pub fn font(rom: &[u8]) -> Result<(GlyphIndex, Glyph), FontReadError> {
    let too_short = || FontReadError::TooShort { len: rom.len() };
    let table_end = FONT_RANGE_TABLE_OFFSET + FONT_RANGE_COUNT * RANGE_ENTRY_LEN;
    let table = rom
        .get(FONT_RANGE_TABLE_OFFSET..table_end)
        .ok_or_else(too_short)?;
    let index = GlyphIndex::parse(table)?;
    let top = tile(rom, FALLBACK_GLYPH_TOP).ok_or_else(too_short)?;
    let bottom = tile(rom, FALLBACK_GLYPH_BOTTOM).ok_or_else(too_short)?;
    Ok((index, Glyph::from_tiles(top, bottom)))
}

fn tile(rom: &[u8], offset: usize) -> Option<&[u8; TILE_LEN]> {
    rom.get(offset..offset + TILE_LEN)?.try_into().ok()
}
