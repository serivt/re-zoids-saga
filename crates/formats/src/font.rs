//! The 8×16 pixel text font of the Zoids GBA games.
//!
//! Glyphs are 4-bit-per-pixel GBA tiles: a glyph is one 8×8 tile for its top
//! half and one for its bottom half, each 32 bytes with two pixels per byte,
//! low nibble first. The sheet stores 32 glyphs per row: the top tiles of a
//! row are contiguous (`0x400` bytes) and the bottom tiles follow them, so a
//! glyph's halves sit `0x400` bytes apart and consecutive rows `0x800` apart.
//! Palette index 1 is the background; 2, 3 and 4 are anti-aliasing shades
//! and 15 is the solid stroke.
//!
//! Which glyph a Shift-JIS code maps to is given by a *range table*: entries
//! of `(first_code, count, sheet_address)` covering consecutive codes.
//! See `docs/formats/font.md`.

use thiserror::Error;

use crate::tile::{TILE_LEN as TILE_BYTES, decode_4bpp};

/// Glyph width in pixels.
pub const GLYPH_WIDTH: usize = 8;
/// Glyph height in pixels.
pub const GLYPH_HEIGHT: usize = 16;
/// Bytes of one 8×8 4bpp tile.
pub const TILE_LEN: usize = TILE_BYTES;
/// Bytes of one range table entry.
pub const RANGE_ENTRY_LEN: usize = 8;

const GLYPHS_PER_ROW: usize = 32;
const HALF_ROW_LEN: usize = GLYPHS_PER_ROW * TILE_LEN;
const ROW_LEN: usize = 2 * HALF_ROW_LEN;
const ROM_BASE: u32 = 0x0800_0000;

/// A decoded glyph: 16 rows of 8 palette indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    /// Palette index of each pixel, row-major.
    pub pixels: [u8; GLYPH_WIDTH * GLYPH_HEIGHT],
}

impl Glyph {
    /// Decodes a glyph from its top and bottom tiles.
    #[must_use]
    pub fn from_tiles(top: &[u8; TILE_LEN], bottom: &[u8; TILE_LEN]) -> Self {
        let mut pixels = [0u8; GLYPH_WIDTH * GLYPH_HEIGHT];
        let (upper, lower) = pixels.split_at_mut(GLYPH_WIDTH * GLYPH_HEIGHT / 2);
        upper.copy_from_slice(&decode_4bpp(top));
        lower.copy_from_slice(&decode_4bpp(bottom));
        Self { pixels }
    }

    /// Palette index at `(x, y)`; `None` outside the glyph.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Option<u8> {
        (x < GLYPH_WIDTH && y < GLYPH_HEIGHT).then(|| self.pixels[y * GLYPH_WIDTH + x])
    }
}

/// A run of consecutive Shift-JIS codes stored in one glyph sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphRange {
    /// First Shift-JIS code of the run.
    pub first_code: u16,
    /// Number of codes in the run.
    pub count: u16,
    /// ROM offset of the sheet holding the run's first glyph.
    pub sheet_offset: usize,
}

/// Where each glyph of the font lives in the ROM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphIndex {
    ranges: Vec<GlyphRange>,
}

/// Why a range table could not be parsed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FontError {
    /// The table bytes are not a whole number of entries.
    #[error("range table of {len} bytes is not a multiple of {RANGE_ENTRY_LEN}")]
    BadTableLength {
        /// Length that was given.
        len: usize,
    },
    /// A sheet address does not point into the ROM.
    #[error("range {index} points to {address:#010x}, outside the ROM")]
    BadSheetAddress {
        /// Entry index.
        index: usize,
        /// Address as stored.
        address: u32,
    },
}

impl GlyphIndex {
    /// Parses a range table.
    ///
    /// # Errors
    ///
    /// Returns [`FontError`] when the length is not a multiple of the entry
    /// size or an entry's sheet address is not a ROM address.
    pub fn parse(table: &[u8]) -> Result<Self, FontError> {
        if table.len() % RANGE_ENTRY_LEN != 0 {
            return Err(FontError::BadTableLength { len: table.len() });
        }
        let ranges = table
            .chunks_exact(RANGE_ENTRY_LEN)
            .enumerate()
            .map(|(index, entry)| parse_range(index, entry))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { ranges })
    }

    /// Number of glyphs covered by the table.
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.ranges
            .iter()
            .map(|range| usize::from(range.count))
            .sum()
    }

    /// ROM offsets of the top and bottom tiles of the glyph for a Shift-JIS
    /// code; `None` when the font has no glyph for it.
    #[must_use]
    pub fn tile_offsets(&self, code: u16) -> Option<(usize, usize)> {
        let range = self
            .ranges
            .iter()
            .find(|range| (range.first_code..range.first_code + range.count).contains(&code))?;
        let position = usize::from(code - range.first_code);
        let top = range.sheet_offset
            + (position / GLYPHS_PER_ROW) * ROW_LEN
            + (position % GLYPHS_PER_ROW) * TILE_LEN;
        Some((top, top + HALF_ROW_LEN))
    }

    /// Decodes the glyph for a Shift-JIS code from `rom`; `None` when the
    /// font has no glyph for it or its tiles fall outside `rom`.
    #[must_use]
    pub fn glyph(&self, rom: &[u8], code: u16) -> Option<Glyph> {
        let (top, bottom) = self.tile_offsets(code)?;
        Some(Glyph::from_tiles(tile_at(rom, top)?, tile_at(rom, bottom)?))
    }
}

/// The lead byte of the codes whose glyphs the game draws itself (the NEC
/// special characters' row of Shift-JIS), such as the letters Ａ to Ｆ that
/// tell same Zoids apart in battle (`0x8791` on).
pub const GAME_GLYPH_LEAD: u8 = 0x87;
/// Where those codes live among the characters: the supplementary private
/// use area, so they keep their own glyphs.
const GAME_GLYPH_BASE: u32 = 0xF_0000;

/// The character standing for a code of the game's own glyphs.
#[must_use]
pub fn game_glyph(code: u16) -> Option<char> {
    char::from_u32(GAME_GLYPH_BASE + u32::from(code))
}

/// The Shift-JIS code of a character, as the font indexes it; `None` for
/// characters Shift-JIS cannot encode or single-byte ones.
#[must_use]
pub fn shift_jis_code(ch: char) -> Option<u16> {
    if let Some(code) = u32::from(ch)
        .checked_sub(GAME_GLYPH_BASE)
        .and_then(|code| u16::try_from(code).ok())
    {
        return Some(code);
    }
    let mut buffer = [0u8; 4];
    let (encoded, _, had_errors) = encoding_rs::SHIFT_JIS.encode(ch.encode_utf8(&mut buffer));
    match (had_errors, encoded.as_ref()) {
        (false, [lead, trail]) => Some(u16::from_be_bytes([*lead, *trail])),
        _ => None,
    }
}

fn parse_range(index: usize, entry: &[u8]) -> Result<GlyphRange, FontError> {
    let first_code = u16::from_le_bytes([entry[0], entry[1]]);
    let count = u16::from_le_bytes([entry[2], entry[3]]);
    let address = u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]);
    let sheet_offset = address
        .checked_sub(ROM_BASE)
        .map(|offset| offset as usize)
        .ok_or(FontError::BadSheetAddress { index, address })?;
    Ok(GlyphRange {
        first_code,
        count,
        sheet_offset,
    })
}

fn tile_at(rom: &[u8], offset: usize) -> Option<&[u8; TILE_LEN]> {
    rom.get(offset..offset + TILE_LEN)?.try_into().ok()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn entry(first: u16, count: u16, address: u32) -> Vec<u8> {
        let mut bytes = first.to_le_bytes().to_vec();
        bytes.extend(count.to_le_bytes());
        bytes.extend(address.to_le_bytes());
        bytes
    }

    #[test]
    fn decodes_nibbles_low_pixel_first() {
        let mut top = [0x11u8; TILE_LEN];
        top[0] = 0xF2;
        let bottom = [0x11u8; TILE_LEN];
        let glyph = Glyph::from_tiles(&top, &bottom);
        assert_eq!(glyph.pixel(0, 0), Some(2));
        assert_eq!(glyph.pixel(1, 0), Some(15));
        assert_eq!(glyph.pixel(2, 0), Some(1));
        assert_eq!(glyph.pixel(0, 8), Some(1));
        assert_eq!(glyph.pixel(8, 0), None);
        assert_eq!(glyph.pixel(0, 16), None);
    }

    #[test]
    fn locates_tiles_by_row_and_column() {
        let table = [
            entry(0x8340, 64, ROM_BASE + 0x1000),
            entry(0x9000, 2, ROM_BASE + 0x8000),
        ]
        .concat();
        let index = GlyphIndex::parse(&table).unwrap();
        assert_eq!(index.glyph_count(), 66);
        assert_eq!(index.tile_offsets(0x8340), Some((0x1000, 0x1400)));
        assert_eq!(index.tile_offsets(0x8341), Some((0x1020, 0x1420)));
        assert_eq!(index.tile_offsets(0x8340 + 32), Some((0x1800, 0x1C00)));
        assert_eq!(index.tile_offsets(0x9001), Some((0x8020, 0x8420)));
        assert_eq!(index.tile_offsets(0x8380), None);
        assert_eq!(index.tile_offsets(0x0000), None);
    }

    #[test]
    fn decodes_a_glyph_from_the_rom() {
        let table = entry(0x8140, 1, ROM_BASE + 0x100);
        let index = GlyphIndex::parse(&table).unwrap();
        let mut rom = vec![0x11u8; 0x100 + HALF_ROW_LEN + TILE_LEN];
        rom[0x100] = 0xFF;
        rom[0x100 + HALF_ROW_LEN + 31] = 0x4F;
        let glyph = index.glyph(&rom, 0x8140).unwrap();
        assert_eq!(glyph.pixel(0, 0), Some(15));
        assert_eq!(glyph.pixel(6, 15), Some(15));
        assert_eq!(glyph.pixel(7, 15), Some(4));
        assert_eq!(index.glyph(&rom[..0x110], 0x8140), None);
    }

    #[test]
    fn maps_characters_to_shift_jis_codes() {
        assert_eq!(shift_jis_code('ゾ'), Some(0x835D));
        assert_eq!(shift_jis_code('Ａ'), Some(0x8260));
        assert_eq!(shift_jis_code('A'), None);
        assert_eq!(shift_jis_code('🦖'), None);
    }

    #[test]
    fn rejects_malformed_tables() {
        assert_eq!(
            GlyphIndex::parse(&[0u8; 12]),
            Err(FontError::BadTableLength { len: 12 })
        );
        assert_eq!(
            GlyphIndex::parse(&entry(0x8140, 1, 0x0200_0000)),
            Err(FontError::BadSheetAddress {
                index: 0,
                address: 0x0200_0000
            })
        );
    }
}
