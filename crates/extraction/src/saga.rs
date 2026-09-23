//! ROM layout of Zoids Saga (Japan, Rev 1).

use formats::bgr555::{PALETTE_LEN, parse_palette, parse_palettes};
use formats::font::{FontError, Glyph, GlyphIndex, RANGE_ENTRY_LEN, TILE_LEN};
use formats::lz77::Lz77Error;
use formats::tile::{TileImage, TilePiece, Tileset};
use formats::tilemap::TileMap;
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

const SCENE_TABLE_OFFSET: usize = 0x001E_70AC;
const SCENE_RECORD_LEN: usize = 24;
const SCENE_COUNT: usize = 205;
const BACKDROP_SIDE: usize = 32;
/// Side of a map attribute cell in map tiles.
pub const METATILE_TILES: usize = 2;
const BLOCKED: u16 = 0x8000;
const EXIT_KIND_MASK: u16 = 0xC000;
const EXIT_WALK: u16 = 0x4000;
const EXIT_INDEX_MASK: u16 = 0x00FF;

/// A field scene: a scrolling map over a repeating backdrop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scene {
    /// The 4bpp tiles shared by the map and the backdrop.
    pub tiles: Tileset,
    /// Background palettes 0–14.
    pub palettes: Vec<[u16; 16]>,
    /// The scrolling map, drawn above the backdrop with index 0 transparent.
    pub map: TileMap,
    /// The 32×32 backdrop tiled behind the map.
    pub backdrop: TileMap,
    /// One entry per 16×16 metatile, row-major, `width / 2` per row;
    /// bit 15 marks a cell the player cannot enter, bits 15–14 = `01`
    /// an exit whose index is the low byte.
    pub attributes: Vec<u16>,
}

impl Scene {
    /// Attribute columns.
    #[must_use]
    pub fn attribute_columns(&self) -> usize {
        self.map.width / METATILE_TILES
    }

    /// Attribute rows.
    #[must_use]
    pub fn attribute_rows(&self) -> usize {
        self.map.height / METATILE_TILES
    }

    /// Whether metatile `(column, row)` blocks walking; cells outside the
    /// map block too.
    #[must_use]
    pub fn blocked(&self, column: usize, row: usize) -> bool {
        if column >= self.attribute_columns() || row >= self.attribute_rows() {
            return true;
        }
        self.attributes
            .get(row * self.attribute_columns() + column)
            .is_none_or(|attribute| attribute & BLOCKED != 0)
    }

    /// The exit index of metatile `(column, row)` when walking onto it
    /// warps the player.
    #[must_use]
    pub fn exit(&self, column: usize, row: usize) -> Option<usize> {
        if column >= self.attribute_columns() {
            return None;
        }
        self.attributes
            .get(row * self.attribute_columns() + column)
            .filter(|attribute| *attribute & EXIT_KIND_MASK == EXIT_WALK)
            .map(|attribute| usize::from(attribute & EXIT_INDEX_MASK))
    }
}

/// Why a scene could not be read.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SceneError {
    /// No such scene.
    #[error("no scene {index}; the table has {SCENE_COUNT}")]
    NoSuchScene {
        /// Requested index.
        index: usize,
    },
    /// The ROM is too short or a block is smaller than the record says.
    #[error("ROM of {len} bytes is too short for scene {index}")]
    TooShort {
        /// ROM length.
        len: usize,
        /// Scene index.
        index: usize,
    },
    /// A compressed block is not valid LZ77.
    #[error(transparent)]
    Lz77(#[from] Lz77Error),
}

/// Reads a field scene by its index in the scene table.
///
/// # Errors
///
/// Returns [`SceneError`] when the index is out of range, the ROM is too
/// short, or a block does not decompress.
pub fn scene(rom: &[u8], index: usize) -> Result<Scene, SceneError> {
    if index >= SCENE_COUNT {
        return Err(SceneError::NoSuchScene { index });
    }
    let too_short = || SceneError::TooShort {
        len: rom.len(),
        index,
    };
    let offset = SCENE_TABLE_OFFSET + index * SCENE_RECORD_LEN;
    let record = rom
        .get(offset..offset + SCENE_RECORD_LEN)
        .ok_or_else(too_short)?;
    let field = |i: usize| rom_offset(&record[4 * i..4 * i + 4]).ok_or_else(too_short);
    let width = usize::from(u16::from_le_bytes([record[20], record[21]]));
    let height = usize::from(u16::from_le_bytes([record[22], record[23]]));
    let backdrop_len = BACKDROP_SIDE * BACKDROP_SIDE * 2;
    let backdrop_bytes = rom
        .get(field(4)?..field(4)? + backdrop_len)
        .ok_or_else(too_short)?;
    let block = |offset: usize| -> Result<Vec<u8>, SceneError> {
        let (bytes, _) = formats::lz77::decompress(rom.get(offset..).ok_or_else(too_short)?)?;
        Ok(bytes)
    };
    let palettes = parse_palettes(&block(field(0)?)?);
    let tiles = Tileset::from_4bpp(&block(field(1)?)?);
    let map = TileMap::from_le_bytes(width, height, &block(field(2)?)?).ok_or_else(too_short)?;
    let backdrop = TileMap::from_le_bytes(BACKDROP_SIDE, BACKDROP_SIDE, backdrop_bytes)
        .ok_or_else(too_short)?;
    let attributes = block(field(3)?)?
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take(width / METATILE_TILES * (height / METATILE_TILES))
        .collect();
    Ok(Scene {
        tiles,
        palettes,
        map,
        backdrop,
        attributes,
    })
}

const MAP_TABLE_OFFSET: usize = 0x0031_B27C;
const MAP_RECORD_LEN: usize = 28;
const MAP_COUNT: usize = 343;
const MAP_NAME_LEN: usize = 12;
const WARP_TABLE_OFFSET: usize = 0x0031_FD84;
const WARP_LEN: usize = 12;
const KEEP_FACING: u16 = 0xFFFF;

/// A map: a scene plus the data the game attaches to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapRecord {
    /// Index in the scene table.
    pub scene: usize,
    /// Map tiles per attribute cell side (2 for rooms, 4 for the world map).
    pub metatile_tiles: usize,
    /// ASCII name, e.g. `md0153`.
    pub name: String,
}

/// Where an exit leads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Warp {
    /// Destination map record.
    pub map: usize,
    /// Arrival metatile column.
    pub column: usize,
    /// Arrival metatile row.
    pub row: usize,
    /// Facing on arrival in sprite sheet order; `None` keeps the current one.
    pub facing: Option<usize>,
    /// Sound id: 0 for the default door sound, `0x44` for none.
    pub sound: u16,
}

/// Why a map record or warp could not be read.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MapError {
    /// No such map.
    #[error("no map {index}; the table has {MAP_COUNT}")]
    NoSuchMap {
        /// Requested index.
        index: usize,
    },
    /// The ROM is too short for the record or warp.
    #[error("ROM of {len} bytes is too short for map {index}")]
    TooShort {
        /// ROM length.
        len: usize,
        /// Map index.
        index: usize,
    },
}

/// Reads map record `index`.
///
/// # Errors
///
/// Returns [`MapError`] when the index is out of range or the ROM is too
/// short.
pub fn map_record(rom: &[u8], index: usize) -> Result<MapRecord, MapError> {
    if index >= MAP_COUNT {
        return Err(MapError::NoSuchMap { index });
    }
    let offset = MAP_TABLE_OFFSET + index * MAP_RECORD_LEN;
    let record = rom
        .get(offset..offset + MAP_RECORD_LEN)
        .ok_or(MapError::TooShort {
            len: rom.len(),
            index,
        })?;
    let half = |at: usize| u16::from_le_bytes([record[at], record[at + 1]]);
    let name = &record[MAP_RECORD_LEN - MAP_NAME_LEN..];
    let name = name.split(|byte| *byte == 0).next().unwrap_or_default();
    Ok(MapRecord {
        scene: usize::from(half(0)),
        metatile_tiles: usize::from(half(4)),
        name: String::from_utf8_lossy(name).into_owned(),
    })
}

/// Reads exit `exit` of map `map`.
///
/// # Errors
///
/// Returns [`MapError`] when the map is out of range or the ROM is too
/// short for its warp table.
pub fn warp(rom: &[u8], map: usize, exit: usize) -> Result<Warp, MapError> {
    if map >= MAP_COUNT {
        return Err(MapError::NoSuchMap { index: map });
    }
    let too_short = || MapError::TooShort {
        len: rom.len(),
        index: map,
    };
    let pointer = WARP_TABLE_OFFSET + map * 4;
    let table =
        rom_offset(rom.get(pointer..pointer + 4).ok_or_else(too_short)?).ok_or_else(too_short)?;
    let offset = table + exit * WARP_LEN;
    let entry = rom.get(offset..offset + WARP_LEN).ok_or_else(too_short)?;
    let half = |at: usize| u16::from_le_bytes([entry[at], entry[at + 1]]);
    let facing = half(10);
    Ok(Warp {
        map: usize::from(half(2)),
        column: usize::from(half(4)),
        row: usize::from(half(6)),
        facing: (facing != KEEP_FACING).then_some(usize::from(facing)),
        sound: half(8),
    })
}

const SPRITE_TABLE_OFFSET: usize = 0x0031_8E04;
const SPRITE_RECORD_LEN: usize = 32;
const SPRITE_COUNT: usize = 248;
const SPRITE_TILE_ROWS: usize = 4;
/// Record of the player's map sprite sheet (tagged `mz25`; tags are not unique).
pub const PLAYER_SPRITE_SHEET: usize = 151;

/// A sheet of same-sized sprite frames stored uncompressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteSheet {
    /// Four-character tag, e.g. `ch00` for the player or `mz25` for a map Zoid.
    pub tag: String,
    /// Number of frames.
    pub frames: usize,
    /// Tiles per frame, laid out in rows of four (16 tiles = 32×32 pixels).
    pub tiles_per_frame: usize,
    /// The 16-color BGR555 palette.
    pub palette: [u16; 16],
    /// Every frame's tiles, frame after frame.
    pub tiles: Tileset,
}

impl SpriteSheet {
    /// Composes frame `index`; `None` when it does not exist.
    #[must_use]
    pub fn frame(&self, index: usize) -> Option<TileImage> {
        if index >= self.frames || self.tiles_per_frame % SPRITE_TILE_ROWS != 0 {
            return None;
        }
        let first = index * self.tiles_per_frame;
        let frame_tiles = Tileset::from_pixels(
            (first..first + self.tiles_per_frame)
                .map(|tile| {
                    self.tiles
                        .tile(tile)
                        .copied()
                        .unwrap_or([0; formats::tile::TILE_PIXELS])
                })
                .collect(),
        );
        let piece = TilePiece {
            column: 0,
            row: 0,
            columns: self.tiles_per_frame / SPRITE_TILE_ROWS,
            rows: SPRITE_TILE_ROWS,
        };
        Some(TileImage::compose(&frame_tiles, &[piece]))
    }
}

/// Why a sprite sheet could not be read.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SpriteSheetError {
    /// No such record.
    #[error("no sprite sheet {index}; the table has {SPRITE_COUNT}")]
    NoSuchSheet {
        /// Requested index.
        index: usize,
    },
    /// No record carries the tag.
    #[error("no sprite sheet tagged {tag:?}")]
    NoSuchTag {
        /// Requested tag.
        tag: String,
    },
    /// The ROM is too short for the record or its data.
    #[error("ROM of {len} bytes is too short for sprite sheet {index}")]
    TooShort {
        /// ROM length.
        len: usize,
        /// Sheet index.
        index: usize,
    },
}

/// Reads sprite sheet `index` of the table at ROM `0x318E04`.
///
/// # Errors
///
/// Returns [`SpriteSheetError`] when the index is out of range or the ROM is too short.
pub fn sprite_sheet(rom: &[u8], index: usize) -> Result<SpriteSheet, SpriteSheetError> {
    if index >= SPRITE_COUNT {
        return Err(SpriteSheetError::NoSuchSheet { index });
    }
    let too_short = || SpriteSheetError::TooShort {
        len: rom.len(),
        index,
    };
    let offset = SPRITE_TABLE_OFFSET + index * SPRITE_RECORD_LEN;
    let record = rom
        .get(offset..offset + SPRITE_RECORD_LEN)
        .ok_or_else(too_short)?;
    let word = |i: usize| {
        u32::from_le_bytes([
            record[4 * i],
            record[4 * i + 1],
            record[4 * i + 2],
            record[4 * i + 3],
        ])
    };
    let tag = String::from_utf8_lossy(&record[8..12]).into_owned();
    let frames = word(3) as usize;
    let tiles_per_frame = word(5) as usize;
    let palette_offset = rom_offset(&record[24..28]).ok_or_else(too_short)?;
    let tiles_offset = rom_offset(&record[28..32]).ok_or_else(too_short)?;
    let palette = rom
        .get(palette_offset..palette_offset + PALETTE_LEN)
        .and_then(parse_palette)
        .ok_or_else(too_short)?;
    let tile_bytes = rom
        .get(tiles_offset..tiles_offset + frames * tiles_per_frame * TILE_LEN)
        .ok_or_else(too_short)?;
    Ok(SpriteSheet {
        tag,
        frames,
        tiles_per_frame,
        palette,
        tiles: Tileset::from_4bpp(tile_bytes),
    })
}

/// Reads the first sprite sheet carrying `tag`; tags repeat, so callers that
/// need a specific record use [`sprite_sheet`].
///
/// # Errors
///
/// Returns [`SpriteSheetError`] when no record has the tag or the ROM is too short.
pub fn sprite_sheet_by_tag(rom: &[u8], tag: &str) -> Result<SpriteSheet, SpriteSheetError> {
    (0..SPRITE_COUNT)
        .find(|index| {
            let offset = SPRITE_TABLE_OFFSET + index * SPRITE_RECORD_LEN + 8;
            rom.get(offset..offset + 4) == Some(tag.as_bytes())
        })
        .map_or_else(
            || {
                Err(SpriteSheetError::NoSuchTag {
                    tag: tag.to_owned(),
                })
            },
            |index| sprite_sheet(rom, index),
        )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn sheet(frames: usize, tiles_per_frame: usize) -> SpriteSheet {
        let tiles = (0..frames * tiles_per_frame)
            .map(|index| [u8::try_from(index % 16).unwrap_or(0); formats::tile::TILE_PIXELS])
            .collect();
        SpriteSheet {
            tag: "ch00".to_owned(),
            frames,
            tiles_per_frame,
            palette: [0; 16],
            tiles: Tileset::from_pixels(tiles),
        }
    }

    #[test]
    fn composes_square_frames_in_tile_rows_of_four() {
        let sheet = sheet(2, 16);
        let frame = sheet.frame(1).unwrap();
        assert_eq!((frame.width, frame.height), (32, 32));
        assert_eq!(frame.indices[0], 0);
        assert_eq!(frame.indices[8], 1);
        assert_eq!(frame.indices[8 * 32], 4);
        assert_eq!(sheet.frame(2), None);
    }

    #[test]
    fn rejects_frames_that_do_not_form_whole_rows() {
        assert_eq!(sheet(1, 6).frame(0), None);
    }

    /// A ROM holding map record 4 and its two warps.
    fn rom_with_map_4() -> Vec<u8> {
        let table = WARP_TABLE_OFFSET + MAP_COUNT * 4;
        let mut rom = vec![0; table + 2 * WARP_LEN];
        let record = MAP_TABLE_OFFSET + 4 * MAP_RECORD_LEN;
        rom[record..record + 6].copy_from_slice(&[3, 0, 1, 0x80, 2, 0]);
        rom[record + 16..record + 22].copy_from_slice(b"md0153");
        let pointer = 0x0800_0000u32 + u32::try_from(table).unwrap();
        rom[WARP_TABLE_OFFSET + 16..WARP_TABLE_OFFSET + 20].copy_from_slice(&pointer.to_le_bytes());
        let warps: [u16; 12] = [0xFFFF, 5, 8, 16, 0, 0xFFFF, 0xFFFF, 3, 23, 5, 0x44, 1];
        for (i, half) in warps.iter().enumerate() {
            rom[table + 2 * i..table + 2 * i + 2].copy_from_slice(&half.to_le_bytes());
        }
        rom
    }

    #[test]
    fn reads_map_records() {
        let rom = rom_with_map_4();
        assert_eq!(
            map_record(&rom, 4).unwrap(),
            MapRecord {
                scene: 3,
                metatile_tiles: 2,
                name: "md0153".to_owned(),
            }
        );
        assert_eq!(
            map_record(&rom, MAP_COUNT),
            Err(MapError::NoSuchMap { index: MAP_COUNT })
        );
        assert!(matches!(
            map_record(&[0; 16], 4),
            Err(MapError::TooShort { len: 16, index: 4 })
        ));
    }

    #[test]
    fn reads_warps() {
        let rom = rom_with_map_4();
        assert_eq!(
            warp(&rom, 4, 0).unwrap(),
            Warp {
                map: 5,
                column: 8,
                row: 16,
                facing: None,
                sound: 0,
            }
        );
        assert_eq!(
            warp(&rom, 4, 1).unwrap(),
            Warp {
                map: 3,
                column: 23,
                row: 5,
                facing: Some(1),
                sound: 0x44,
            }
        );
        assert!(matches!(warp(&rom, 4, 2), Err(MapError::TooShort { .. })));
        assert!(matches!(warp(&rom, 3, 0), Err(MapError::TooShort { .. })));
    }

    #[test]
    fn finds_exits_in_attributes() {
        let scene = Scene {
            tiles: Tileset::from_4bpp(&[]),
            palettes: vec![],
            map: TileMap {
                width: 4,
                height: 4,
                entries: vec![0; 16],
            },
            backdrop: TileMap {
                width: 32,
                height: 32,
                entries: vec![0; 1024],
            },
            attributes: vec![0x8000, 0x4001, 0xC002, 0x0001],
        };
        assert_eq!(scene.exit(0, 0), None);
        assert_eq!(scene.exit(1, 0), Some(1));
        assert_eq!(scene.exit(0, 1), None);
        assert_eq!(scene.exit(1, 1), None);
        assert_eq!(scene.exit(2, 0), None);
        assert_eq!(scene.exit(1, 5), None);
    }
}
