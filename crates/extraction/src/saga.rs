//! ROM layout of Zoids Saga (Japan, Rev 1).

use formats::bgr555::{PALETTE_LEN, parse_palette};
use formats::font::{FontError, Glyph, GlyphIndex, RANGE_ENTRY_LEN, TILE_LEN};
use formats::lz77::Lz77Error;
use formats::tile::{TileImage, TilePiece, Tileset};
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

const WINDOW_TILESET_OFFSET: usize = 0x003B_9428;
const WINDOW_PALETTE_OFFSET: usize = 0x006B_F9F8;

/// Tiles and palette of the text window frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSkin {
    /// The 64 frame tiles, in the order the game loads them into VRAM.
    pub tiles: Tileset,
    /// The 16-color BGR555 palette the frame and the text use.
    pub palette: [u16; 16],
}

/// Why the window skin could not be read.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WindowSkinError {
    /// The ROM is too short to hold the skin data.
    #[error("ROM of {len} bytes is too short for the window skin")]
    TooShort {
        /// ROM length.
        len: usize,
    },
    /// The tileset block is not valid LZ77.
    #[error(transparent)]
    Tiles(#[from] Lz77Error),
}

/// Reads the text window's frame tileset (LZ77-compressed in the ROM) and its palette.
///
/// # Errors
///
/// Returns [`WindowSkinError`] when the ROM is too short or the tileset does not decompress.
pub fn window_skin(rom: &[u8]) -> Result<WindowSkin, WindowSkinError> {
    let too_short = || WindowSkinError::TooShort { len: rom.len() };
    let compressed = rom.get(WINDOW_TILESET_OFFSET..).ok_or_else(too_short)?;
    let (tile_bytes, _) = formats::lz77::decompress(compressed)?;
    let palette = rom
        .get(WINDOW_PALETTE_OFFSET..WINDOW_PALETTE_OFFSET + PALETTE_LEN)
        .and_then(parse_palette)
        .ok_or_else(too_short)?;
    Ok(WindowSkin {
        tiles: Tileset::from_4bpp(&tile_bytes),
        palette,
    })
}

const PORTRAIT_TABLE_OFFSET: usize = 0x006D_0A64;
const PORTRAIT_RECORD_LEN: usize = 16;
const PORTRAIT_COUNT: usize = 468;
/// Portraits per character in the table: one per facial expression.
pub const PORTRAIT_EXPRESSIONS: usize = 9;
const PORTRAIT_PIECES: [TilePiece; 4] = [
    TilePiece {
        column: 0,
        row: 0,
        columns: 4,
        rows: 4,
    },
    TilePiece {
        column: 0,
        row: 4,
        columns: 4,
        rows: 2,
    },
    TilePiece {
        column: 4,
        row: 0,
        columns: 2,
        rows: 4,
    },
    TilePiece {
        column: 4,
        row: 4,
        columns: 2,
        rows: 2,
    },
];

/// A character's 48×48 dialogue portrait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Portrait {
    /// Palette indices of the portrait; index 0 is transparent.
    pub image: TileImage,
    /// The 16-color BGR555 palette.
    pub palette: [u16; 16],
}

/// Why a portrait could not be read.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PortraitError {
    /// No such character or expression.
    #[error("no portrait for character {character} expression {expression}")]
    NoSuchPortrait {
        /// Character index as the script numbers it.
        character: usize,
        /// Expression index.
        expression: usize,
    },
    /// The ROM is too short to hold the record or its data.
    #[error("ROM of {len} bytes is too short for the portrait table")]
    TooShort {
        /// ROM length.
        len: usize,
    },
    /// The tiles or palette block is not valid LZ77.
    #[error(transparent)]
    Lz77(#[from] Lz77Error),
}

/// Reads the portrait of a character (as the script numbers it) with the
/// given expression. Both are LZ77 blocks referenced by a record table.
///
/// # Errors
///
/// Returns [`PortraitError`] when the indices are out of range, the ROM is
/// too short or a block does not decompress.
pub fn portrait(
    rom: &[u8],
    character: usize,
    expression: usize,
) -> Result<Portrait, PortraitError> {
    let record = character
        .checked_mul(PORTRAIT_EXPRESSIONS)
        .and_then(|base| base.checked_add(expression))
        .filter(|record| expression < PORTRAIT_EXPRESSIONS && *record < PORTRAIT_COUNT)
        .ok_or(PortraitError::NoSuchPortrait {
            character,
            expression,
        })?;
    let too_short = || PortraitError::TooShort { len: rom.len() };
    let offset = PORTRAIT_TABLE_OFFSET + record * PORTRAIT_RECORD_LEN;
    let entry = rom.get(offset..offset + 8).ok_or_else(too_short)?;
    let tiles_offset = rom_offset(&entry[..4]).ok_or_else(too_short)?;
    let palette_offset = rom_offset(&entry[4..8]).ok_or_else(too_short)?;
    let (tile_bytes, _) =
        formats::lz77::decompress(rom.get(tiles_offset..).ok_or_else(too_short)?)?;
    let (palette_bytes, _) =
        formats::lz77::decompress(rom.get(palette_offset..).ok_or_else(too_short)?)?;
    let palette = parse_palette(&palette_bytes).ok_or_else(too_short)?;
    Ok(Portrait {
        image: TileImage::compose(&Tileset::from_4bpp(&tile_bytes), &PORTRAIT_PIECES),
        palette,
    })
}

fn rom_offset(pointer: &[u8]) -> Option<usize> {
    let address = u32::from_le_bytes([pointer[0], pointer[1], pointer[2], pointer[3]]);
    address
        .checked_sub(0x0800_0000)
        .map(|offset| offset as usize)
}
