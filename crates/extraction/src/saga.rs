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
        offset: 0x0074_FC54,
        count: 1017,
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
const PORTRAIT_COUNT: usize = 783;
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
    if entry[..4] == [0; 4] {
        return Err(PortraitError::NoSuchPortrait {
            character,
            expression,
        });
    }
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

pub(crate) fn rom_offset(pointer: &[u8]) -> Option<usize> {
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
    /// One entry per cell of `cell_tiles`×`cell_tiles` map tiles (16×16
    /// pixels in rooms and towns, 32×32 on Zoid maps), row-major; bit 15
    /// marks a cell the player cannot enter, bits 15–14 = `01` an exit
    /// whose index is the low byte.
    pub attributes: Vec<u16>,
    /// Map tiles per attribute cell side, from the map record.
    pub cell_tiles: usize,
}

impl Scene {
    /// Attribute columns.
    #[must_use]
    pub fn attribute_columns(&self) -> usize {
        self.map.width / self.cell_tiles.max(1)
    }

    /// Attribute rows.
    #[must_use]
    pub fn attribute_rows(&self) -> usize {
        self.map.height / self.cell_tiles.max(1)
    }

    /// The scene as a map with cells of `cell_tiles` map tiles uses it:
    /// the attribute grid is that coarse.
    #[must_use]
    pub fn with_cell_tiles(mut self, cell_tiles: usize) -> Self {
        self.cell_tiles = cell_tiles.max(1);
        let cells = self.attribute_columns() * self.attribute_rows();
        self.attributes.truncate(cells);
        self
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

/// Reads a field scene by its index in the scene table, with the attribute
/// grid of a room (cells of two map tiles); see [`Scene::with_cell_tiles`].
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
        cell_tiles: METATILE_TILES,
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
    /// The record's id; bit 15 is set on some records, and the game keeps
    /// the low byte in the save as the current area.
    pub id: u16,
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
        id: half(2),
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

const LOGO_PALETTE_OFFSET: usize = 0x0005_EFC0;
const LOGO_TILES_OFFSET: usize = 0x0005_EFE0;
const LOGO_MAP_OFFSET: usize = 0x0006_12A0;
const LOGO_MAP_COLUMNS: usize = 30;
const LOGO_MAP_ROWS: usize = 20;
const TILE_8BPP_LEN: usize = 64;
const TITLE_TILES_OFFSET: usize = 0x0006_1750;
const TITLE_PICTURE_OFFSET: usize = 0x0006_3414;
const TITLE_TEXT_TILES_OFFSET: usize = 0x0006_310C;
const TITLE_PALETTES_OFFSET: usize = 0x0006_78C0;
const TITLE_PALETTE_COUNT: usize = 11;
const TITLE_EXTRA_PALETTES: [usize; 4] = [0x0006_3394, 0x0006_33B4, 0x0006_33D4, 0x0006_33F4];
const NAME_ENTRY_PICTURE_OFFSET: usize = 0x0041_42B8;
const NAME_ENTRY_PICTURE_PALETTE_OFFSET: usize = 0x0042_5D58;
const NAME_ENTRY_SPRITES: [(usize, usize); 3] = [
    (0x004B_6788, 0x004B_6834),
    (0x004B_6A54, 0x004B_6A68),
    (0x004B_6940, 0x004B_69C0),
];
const KANA_TABLE_OFFSET: usize = 0x006D_4884;
const KANA_TABLE_ROWS: usize = 30;
/// Characters per kana table row.
pub const KANA_COLUMNS: usize = 13;
/// ROM offset of the title menu script (window, three choices, menu).
pub const TITLE_MENU_SCRIPT_OFFSET: usize = 0x006C_04FE;
/// The name entry's scripts: 0 refreshes the name field, 1 asks for
/// confirmation, 2 and 3 draw the special and symbol character pages.
pub const NAME_ENTRY_SCRIPTS: StringTable = StringTable {
    name: "name-entry",
    offset: 0x006D_08F8,
    count: 5,
};

/// The publisher logo shown at power-on: a 256-color tiled picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logo {
    /// 8bpp tiles, one palette index per pixel.
    pub tiles: Tileset,
    /// The 16 colors the tiles use, indices 0–15 of the 256-color palette.
    pub palette: [u16; 16],
    /// The 30×20 tilemap.
    pub map: TileMap,
}

/// Graphics of the title screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleGraphics {
    /// 4bpp tiles the game loads at VRAM tile 0x102: glow, subtitle and,
    /// from tile 175 on, the logo sprites.
    pub tiles: Tileset,
    /// 8bpp tiles of the background picture, 30 per row from VRAM tile 0x115.
    pub picture: Tileset,
    /// 4bpp tiles of the copyright sprites, at OBJ tile 112.
    pub text_tiles: Tileset,
    /// Background palettes 0–14.
    pub palettes: Vec<[u16; 16]>,
    /// Sprite palettes 0 and 1.
    pub sprite_palettes: [[u16; 16]; 2],
}

/// Graphics of the name entry screen besides the windows and the font.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameEntryGraphics {
    /// 8bpp tiles of the 128×128 picture behind the portrait, 16 per row.
    pub picture: Tileset,
    /// Its 64 colors, palette indices 64–127.
    pub picture_palette: Vec<u16>,
    /// The name field arrows: two 16×16 sprites, tiles 0–3 the right one
    /// (`R►`) and 4–7 the left one (`◄L`).
    pub arrows: SpriteBlock,
    /// The 8×8 mark under each name slot.
    pub slot_mark: SpriteBlock,
    /// The 8×16 grid cursor.
    pub cursor: SpriteBlock,
}

/// A block of 4bpp sprite tiles with its palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteBlock {
    /// The tiles.
    pub tiles: Tileset,
    /// The 16-color palette.
    pub palette: [u16; 16],
}

/// Why boot screen data could not be read.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BootError {
    /// The ROM is too short for a block.
    #[error("ROM of {len} bytes is too short for the {what}")]
    TooShort {
        /// ROM length.
        len: usize,
        /// Which block.
        what: &'static str,
    },
    /// A compressed block is not valid LZ77.
    #[error(transparent)]
    Lz77(#[from] Lz77Error),
}

/// Reads the publisher logo.
///
/// # Errors
///
/// Returns [`BootError`] when the ROM is too short.
pub fn logo(rom: &[u8]) -> Result<Logo, BootError> {
    let map_len = LOGO_MAP_COLUMNS * LOGO_MAP_ROWS * 2;
    let map_bytes = slice(rom, LOGO_MAP_OFFSET, map_len, "logo map")?;
    let map = TileMap::from_le_bytes(LOGO_MAP_COLUMNS, LOGO_MAP_ROWS, map_bytes).ok_or(
        BootError::TooShort {
            len: rom.len(),
            what: "logo map",
        },
    )?;
    let tile_count = map
        .entries
        .iter()
        .map(|entry| usize::from(entry & 0x3FF))
        .max()
        .unwrap_or(0)
        + 1;
    let tiles = tiles_8bpp(rom, LOGO_TILES_OFFSET, tile_count, "logo tiles")?;
    let palette = palette_at(rom, LOGO_PALETTE_OFFSET, "logo palette")?;
    Ok(Logo {
        tiles,
        palette,
        map,
    })
}

/// Reads the title screen graphics.
///
/// # Errors
///
/// Returns [`BootError`] when the ROM is too short or a block does not
/// decompress.
pub fn title(rom: &[u8]) -> Result<TitleGraphics, BootError> {
    let tiles = Tileset::from_4bpp(&lz77_block(rom, TITLE_TILES_OFFSET, "title tiles")?);
    let picture_bytes = lz77_block(rom, TITLE_PICTURE_OFFSET, "title picture")?;
    let picture = Tileset::from_pixels(
        picture_bytes
            .chunks_exact(TILE_8BPP_LEN)
            .map(|tile| tile.try_into().unwrap_or([0; TILE_8BPP_LEN]))
            .collect(),
    );
    let text_tiles = Tileset::from_4bpp(&lz77_block(rom, TITLE_TEXT_TILES_OFFSET, "title text")?);
    let mut palettes = (0..TITLE_PALETTE_COUNT)
        .map(|index| {
            palette_at(
                rom,
                TITLE_PALETTES_OFFSET + index * PALETTE_LEN,
                "title palettes",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    for offset in TITLE_EXTRA_PALETTES {
        palettes.push(palette_at(rom, offset, "title palettes")?);
    }
    let sprite_palettes = [
        palette_at(rom, TITLE_EXTRA_PALETTES[2], "title palettes")?,
        palette_at(rom, TITLE_EXTRA_PALETTES[3], "title palettes")?,
    ];
    Ok(TitleGraphics {
        tiles,
        picture,
        text_tiles,
        palettes,
        sprite_palettes,
    })
}

/// Reads the name entry screen graphics.
///
/// # Errors
///
/// Returns [`BootError`] when the ROM is too short or a block does not
/// decompress.
pub fn name_entry_graphics(rom: &[u8]) -> Result<NameEntryGraphics, BootError> {
    let picture_bytes = lz77_block(rom, NAME_ENTRY_PICTURE_OFFSET, "name entry picture")?;
    let picture = Tileset::from_pixels(
        picture_bytes
            .chunks_exact(TILE_8BPP_LEN)
            .map(|tile| tile.try_into().unwrap_or([0; TILE_8BPP_LEN]))
            .collect(),
    );
    let picture_palette = lz77_block(rom, NAME_ENTRY_PICTURE_PALETTE_OFFSET, "name entry palette")?
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let mut blocks = NAME_ENTRY_SPRITES.iter().map(|(tiles, palette)| {
        let colors = lz77_block(rom, *palette, "name entry sprites")?;
        Ok::<SpriteBlock, BootError>(SpriteBlock {
            tiles: Tileset::from_4bpp(&lz77_block(rom, *tiles, "name entry sprites")?),
            palette: parse_palette(&colors).ok_or(BootError::TooShort {
                len: rom.len(),
                what: "name entry sprites",
            })?,
        })
    });
    let arrows = blocks.next().transpose()?.unwrap_or_else(empty_block);
    let slot_mark = blocks.next().transpose()?.unwrap_or_else(empty_block);
    let cursor = blocks.next().transpose()?.unwrap_or_else(empty_block);
    Ok(NameEntryGraphics {
        picture,
        picture_palette,
        arrows,
        slot_mark,
        cursor,
    })
}

fn empty_block() -> SpriteBlock {
    SpriteBlock {
        tiles: Tileset::from_4bpp(&[]),
        palette: [0; 16],
    }
}

/// Reads the character table of the name entry: 30 rows of 13 characters
/// (katakana, hiragana, alphanumerics, special, symbols, and the kanji
/// search's hiragana), full-width spaces where a cell is empty.
///
/// # Errors
///
/// Returns [`BootError`] when the ROM is too short.
pub fn kana_table(rom: &[u8]) -> Result<Vec<Vec<char>>, BootError> {
    let bytes = slice(
        rom,
        KANA_TABLE_OFFSET,
        KANA_TABLE_ROWS * KANA_COLUMNS * 2,
        "kana table",
    )?;
    Ok(bytes
        .chunks_exact(KANA_COLUMNS * 2)
        .map(|row| {
            row.chunks_exact(2)
                .map(|pair| {
                    let stored = [pair[1], pair[0]];
                    let (decoded, _) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(&stored);
                    decoded.chars().next().unwrap_or('\u{3000}')
                })
                .collect()
        })
        .collect())
}

fn slice<'a>(
    rom: &'a [u8],
    offset: usize,
    len: usize,
    what: &'static str,
) -> Result<&'a [u8], BootError> {
    rom.get(offset..offset + len).ok_or(BootError::TooShort {
        len: rom.len(),
        what,
    })
}

fn lz77_block(rom: &[u8], offset: usize, what: &'static str) -> Result<Vec<u8>, BootError> {
    let compressed = rom.get(offset..).ok_or(BootError::TooShort {
        len: rom.len(),
        what,
    })?;
    Ok(formats::lz77::decompress(compressed)?.0)
}

fn palette_at(rom: &[u8], offset: usize, what: &'static str) -> Result<[u16; 16], BootError> {
    slice(rom, offset, PALETTE_LEN, what).and_then(|bytes| {
        parse_palette(bytes).ok_or(BootError::TooShort {
            len: rom.len(),
            what,
        })
    })
}

fn tiles_8bpp(
    rom: &[u8],
    offset: usize,
    count: usize,
    what: &'static str,
) -> Result<Tileset, BootError> {
    let bytes = slice(rom, offset, count * TILE_8BPP_LEN, what)?;
    Ok(Tileset::from_pixels(
        bytes
            .chunks_exact(TILE_8BPP_LEN)
            .map(|tile| tile.try_into().unwrap_or([0; TILE_8BPP_LEN]))
            .collect(),
    ))
}

const PAUSE_WALLPAPER_TILES_OFFSET: usize = 0x0056_4748;
const PAUSE_WALLPAPER_PALETTE_OFFSET: usize = 0x0056_5E64;
const PAUSE_WALLPAPER_TEXTURE_OFFSET: usize = 0x0056_5F40;
const PAUSE_WALLPAPER_LOGO_OFFSET: usize = 0x0056_60C4;
const PAUSE_WALLPAPER_TILE_COUNT: usize = 256;
const PAUSE_WALLPAPER_MAP_SIDE: usize = 32;
/// The sound driver's song table: 134 entries of a header pointer and the
/// player number, at ROM `0x567768` (the code indexes it from there).
pub const SONG_TABLE: usize = 0x0056_7768;
/// Entries in the song table.
pub const SONG_COUNT: usize = 134;
/// The driver's master volume, read from its RAM.
pub const MASTER_VOLUME: u8 = 14;
/// Music the title screen plays.
pub const MUSIC_TITLE: usize = 1;
/// Music behind the name entry.
pub const MUSIC_NAME_ENTRY: usize = 3;
/// Music of the opening cutscene.
pub const MUSIC_OPENING: usize = 11;
/// Music of the first room once control begins.
pub const MUSIC_FIRST_ROOM: usize = 7;
/// Sound of START on the title.
pub const SOUND_TITLE_START: usize = 0x3D;
/// Sound of confirming a menu choice.
pub const SOUND_CONFIRM: usize = 0x47;
/// Sound of a door.
pub const SOUND_DOOR: usize = 0x82;
const MAP_MUSIC_FIELD: usize = 8;
const ROM_BASE: u32 = 0x0800_0000;

/// The song map `map` plays: the word at offset 8 of its record, which the
/// map loader starts unless it is already playing (the first room's
/// opening then switches to song 7 when control begins).
#[must_use]
pub fn map_music(rom: &[u8], map: usize) -> Option<usize> {
    if map >= MAP_COUNT {
        return None;
    }
    let at = MAP_TABLE_OFFSET + map * MAP_RECORD_LEN + MAP_MUSIC_FIELD;
    let field = rom.get(at..at + 2)?;
    Some(usize::from(u16::from_le_bytes([field[0], field[1]])))
}

/// The experience table: 99 words at ROM `0x66BB58`, entry `n` being the
/// experience a character needs to reach level `n + 1` (7·n³ from
/// level 3 on, 14 for level 2).
pub const EXPERIENCE_TABLE: usize = 0x0066_BB58;
/// Entries in the experience table.
pub const EXPERIENCE_LEVELS: usize = 99;

/// Experience needed to reach the level after `level`, or `None` past the
/// table.
#[must_use]
pub fn experience_to_next(rom: &[u8], level: usize) -> Option<u32> {
    if level >= EXPERIENCE_LEVELS {
        return None;
    }
    let at = EXPERIENCE_TABLE + level * 4;
    let bytes = rom.get(at..at + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// The battle system's menu scripts (the command list, the status
/// window's frames and the helpers that open the battle's windows), which
/// events also call.
pub const BATTLE_MENU_SCRIPTS: StringTable = StringTable {
    name: "battle-menu",
    offset: 0x0067_5D94,
    count: 28,
};

/// The battle system's messages ("the deck command 「…」 was taught" and
/// the like), which events also call.
pub const BATTLE_TEXT_SCRIPTS: StringTable = StringTable {
    name: "battle-text",
    offset: 0x0067_5E04,
    count: 134,
};

/// Where the deck commands' names start in the `item` table: command `n`
/// is its string `77 + n` (the game reads them from ROM `0x676530`).
pub const COMMAND_NAMES: usize = 77;

/// The scripts the pause menu is assembled from: window openers, the item
/// list (46), the help line and menu (47), the status submenu (48, 49),
/// the party panel pieces (64–67), the money box (44, 45) and notices.
pub const PAUSE_MENU_SCRIPTS: StringTable = StringTable {
    name: "pause-menu",
    offset: 0x0075_B1BC,
    count: 698,
};

/// The scrolling wallpaper behind the pause menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PauseWallpaper {
    /// 8bpp tiles; the game copies the first 256 of the block into the
    /// character block the two layers share.
    pub tiles: Tileset,
    /// Its 96 colors, palette indices 64–159.
    pub palette: Vec<u16>,
    /// The 32×32 map of the logo layer, scrolled one pixel per frame.
    pub logo: TileMap,
    /// The 32×32 map of the texture layer behind it.
    pub texture: TileMap,
}

/// Reads the pause menu wallpaper.
///
/// # Errors
///
/// Returns [`BootError`] when a block cannot be read.
pub fn pause_wallpaper(rom: &[u8]) -> Result<PauseWallpaper, BootError> {
    let tile_bytes = lz77_block(rom, PAUSE_WALLPAPER_TILES_OFFSET, "pause wallpaper tiles")?;
    let tiles = Tileset::from_pixels(
        tile_bytes
            .chunks_exact(TILE_8BPP_LEN)
            .take(PAUSE_WALLPAPER_TILE_COUNT)
            .map(|tile| tile.try_into().unwrap_or([0; TILE_8BPP_LEN]))
            .collect(),
    );
    let palette = lz77_block(
        rom,
        PAUSE_WALLPAPER_PALETTE_OFFSET,
        "pause wallpaper palette",
    )?
    .chunks_exact(2)
    .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
    .collect();
    let map = |offset: usize, what: &'static str| {
        let bytes = lz77_block(rom, offset, what)?;
        TileMap::from_le_bytes(PAUSE_WALLPAPER_MAP_SIDE, PAUSE_WALLPAPER_MAP_SIDE, &bytes).ok_or(
            BootError::TooShort {
                len: rom.len(),
                what,
            },
        )
    };
    Ok(PauseWallpaper {
        tiles,
        palette,
        logo: map(PAUSE_WALLPAPER_LOGO_OFFSET, "pause wallpaper logo map")?,
        texture: map(
            PAUSE_WALLPAPER_TEXTURE_OFFSET,
            "pause wallpaper texture map",
        )?,
    })
}

const SPRITE_TABLE_OFFSET: usize = 0x0031_8DFC;
const SPRITE_RECORD_LEN: usize = 32;
const SPRITE_COUNT: usize = 291;
const SPRITE_TAG_LEN: usize = 4;
const SPRITE_FRAME_LEN: usize = 24;
const SPRITE_FRAME_MIRROR: u16 = 1;
const SPRITE_ANIMATION_STEP_LEN: usize = 4;
const SPRITE_ANIMATION_END: u16 = 0x8000;
const SPRITE_ANIMATIONS_MAX: usize = 32;
const SPRITE_ANIMATION_STEPS_MAX: usize = 128;
/// Sprite id of the player's map sprite (`ch00`).
pub const PLAYER_SPRITE: usize = 0x98;
/// Map record of the first room after the opening.
pub const FIRST_ROOM_MAP: usize = 4;
/// Metatile the player stands on when control begins.
pub const PLAYER_START: (usize, usize) = (5, 2);
/// The flag a new game sets on entering the first room; continuing a save
/// without it plays the opening again.
pub const OPENING_SEEN_FLAG: u16 = 0x11F;
/// Animation ids of a walking sprite: idle animations are the facing
/// direction in sprite sheet order, walking ones follow them.
pub const WALK_ANIMATION_BASE: usize = 4;

/// One step of a sprite animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationStep {
    /// Frame record index.
    pub frame: usize,
    /// Ticks the frame stays; the game halves them for walking sprites.
    pub duration: u32,
}

/// A drawable frame: a block of the sheet's tiles placed relative to the
/// sprite's anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteFrame {
    /// First tile in the sheet.
    pub tile: usize,
    /// Horizontal offset of the top-left corner from the anchor.
    pub x: i16,
    /// Vertical offset of the top-left corner from the anchor.
    pub y: i16,
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Whether the image is drawn flipped left to right (right-facing
    /// frames of most characters reuse the left-facing images this way).
    pub mirrored: bool,
}

/// A sprite: same-sized images stored uncompressed, plus the frames and
/// animations that use them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteSheet {
    /// Four-character tag, e.g. `ch00` for the player or `mz10` for a map Zoid.
    pub tag: String,
    /// Number of images.
    pub images: usize,
    /// Tiles per image, laid out in rows of four (16 tiles = 32×32 pixels).
    pub tiles_per_image: usize,
    /// The 16-color BGR555 palette.
    pub palette: [u16; 16],
    /// Every image's tiles, image after image.
    pub tiles: Tileset,
    /// Frame records referenced by the animations.
    pub frames: Vec<SpriteFrame>,
    /// Animations as lists of steps.
    pub animations: Vec<Vec<AnimationStep>>,
}

impl SpriteSheet {
    /// Composes image `index`; `None` when it does not exist.
    #[must_use]
    pub fn image(&self, index: usize) -> Option<TileImage> {
        if index >= self.images || self.tiles_per_image % SPRITE_TILE_ROWS != 0 {
            return None;
        }
        let columns = self.tiles_per_image / SPRITE_TILE_ROWS;
        self.compose(index * self.tiles_per_image, columns, SPRITE_TILE_ROWS)
    }

    /// Composes the picture of frame record `index`; `None` when it does
    /// not exist.
    #[must_use]
    pub fn frame_image(&self, index: usize) -> Option<TileImage> {
        let frame = self.frames.get(index)?;
        let (columns, rows) = (frame.width / TILE_SIDE, frame.height / TILE_SIDE);
        self.compose(frame.tile, columns, rows)
    }

    fn compose(&self, first: usize, columns: usize, rows: usize) -> Option<TileImage> {
        let count = columns * rows;
        if count == 0 || first + count > self.tiles.len() {
            return None;
        }
        let tiles = Tileset::from_pixels(
            (first..first + count)
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
            columns,
            rows,
        };
        Some(TileImage::compose(&tiles, &[piece]))
    }
}

const SPRITE_TILE_ROWS: usize = 4;
const TILE_SIDE: usize = 8;

/// Why a sprite sheet could not be read.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SpriteSheetError {
    /// No such record.
    #[error("no sprite {id}; ids run from 0 to {}", SPRITE_COUNT - 1)]
    NoSuchSprite {
        /// Requested id.
        id: usize,
    },
    /// No record carries the tag.
    #[error("no sprite tagged {tag:?}")]
    NoSuchTag {
        /// Requested tag.
        tag: String,
    },
    /// The ROM is too short for the record or its data.
    #[error("ROM of {len} bytes is too short for sprite {id}")]
    TooShort {
        /// ROM length.
        len: usize,
        /// Sprite id.
        id: usize,
    },
}

/// Reads sprite `id` (1-based, as map objects name them).
///
/// # Errors
///
/// Returns [`SpriteSheetError`] when the id is out of range or the ROM is too short.
pub fn sprite_sheet(rom: &[u8], id: usize) -> Result<SpriteSheet, SpriteSheetError> {
    if id >= SPRITE_COUNT {
        return Err(SpriteSheetError::NoSuchSprite { id });
    }
    let too_short = || SpriteSheetError::TooShort { len: rom.len(), id };
    let offset = SPRITE_TABLE_OFFSET + id * SPRITE_RECORD_LEN;
    let record = rom
        .get(offset..offset + SPRITE_RECORD_LEN)
        .ok_or_else(too_short)?;
    let pointer = |at: usize| rom_offset(&record[at..at + 4]).ok_or_else(too_short);
    let half = |at: usize| usize::from(u16::from_le_bytes([record[at], record[at + 1]]));
    let tag = String::from_utf8_lossy(&record[16..16 + SPRITE_TAG_LEN]).into_owned();
    let images = half(20);
    let tiles_per_image = half(28);
    let palette = rom
        .get(pointer(0)?..pointer(0)? + PALETTE_LEN)
        .and_then(parse_palette)
        .ok_or_else(too_short)?;
    let tile_bytes = rom
        .get(pointer(4)?..pointer(4)? + images * tiles_per_image * TILE_LEN)
        .ok_or_else(too_short)?;
    let animations = read_animations(rom, pointer(8)?).ok_or_else(too_short)?;
    let frame_count = animations
        .iter()
        .flatten()
        .map(|step| step.frame + 1)
        .max()
        .unwrap_or(0);
    let frames = (0..frame_count)
        .map(|index| read_frame(rom, pointer(12)? + index * 4).ok_or_else(too_short))
        .collect::<Result<_, _>>()?;
    Ok(SpriteSheet {
        tag,
        images,
        tiles_per_image,
        palette,
        tiles: Tileset::from_4bpp(tile_bytes),
        frames,
        animations,
    })
}

pub(crate) fn read_animations(rom: &[u8], table: usize) -> Option<Vec<Vec<AnimationStep>>> {
    let mut animations = Vec::new();
    for index in 0..SPRITE_ANIMATIONS_MAX {
        let at = table + index * 4;
        let Some(sequence) = rom.get(at..at + 4).and_then(rom_offset) else {
            break;
        };
        if sequence >= rom.len() {
            break;
        }
        animations.push(read_steps(rom, sequence)?);
    }
    Some(animations)
}

pub(crate) fn read_steps(rom: &[u8], mut at: usize) -> Option<Vec<AnimationStep>> {
    let mut steps = Vec::new();
    for _ in 0..SPRITE_ANIMATION_STEPS_MAX {
        let bytes = rom.get(at..at + SPRITE_ANIMATION_STEP_LEN)?;
        let frame = u16::from_le_bytes([bytes[0], bytes[1]]);
        if frame >= SPRITE_ANIMATION_END {
            return Some(steps);
        }
        steps.push(AnimationStep {
            frame: usize::from(frame),
            duration: u32::from(u16::from_le_bytes([bytes[2], bytes[3]])),
        });
        at += SPRITE_ANIMATION_STEP_LEN;
    }
    None
}

fn read_frame(rom: &[u8], pointer: usize) -> Option<SpriteFrame> {
    let at = rom_offset(rom.get(pointer..pointer + 4)?)?;
    let bytes = rom.get(at..at + SPRITE_FRAME_LEN)?;
    let half = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let signed = |i: usize| i16::from_le_bytes([bytes[i], bytes[i + 1]]);
    Some(SpriteFrame {
        tile: usize::from(half(0)),
        x: signed(4),
        y: signed(6),
        width: usize::from(half(8)),
        height: usize::from(half(10)),
        mirrored: half(2) & SPRITE_FRAME_MIRROR != 0,
    })
}

/// The picture the status screens show of Zoid `zoid` (`0x0804D6E8`):
/// its record at ROM `0x670210` (76 bytes each) carries a sprite of its
/// own, the images at `+0x30` (64 tiles), the palette at `+0x34`, and the
/// animation and frame tables at `+0x38` and `+0x3C`.
///
/// # Errors
///
/// Returns [`SpriteSheetError`] when the record or its data is outside the
/// ROM.
pub fn zoid_status_sprite(rom: &[u8], zoid: usize) -> Result<SpriteSheet, SpriteSheetError> {
    let too_short = || SpriteSheetError::TooShort {
        len: rom.len(),
        id: zoid,
    };
    let offset = ZOID_RECORDS + zoid * ZOID_RECORD_LEN;
    let record = rom
        .get(offset..offset + ZOID_RECORD_LEN)
        .ok_or_else(too_short)?;
    let pointer = |at: usize| rom_offset(&record[at..at + 4]).ok_or_else(too_short);
    let palette = rom
        .get(pointer(ZOID_SPRITE_PALETTE)?..pointer(ZOID_SPRITE_PALETTE)? + PALETTE_LEN)
        .and_then(parse_palette)
        .ok_or_else(too_short)?;
    let images = pointer(ZOID_SPRITE_IMAGES)?;
    let tile_bytes = rom
        .get(images..images + ZOID_SPRITE_TILES * TILE_LEN)
        .ok_or_else(too_short)?;
    let animations =
        read_animations(rom, pointer(ZOID_SPRITE_ANIMATIONS)?).ok_or_else(too_short)?;
    let frame_count = animations
        .iter()
        .flatten()
        .map(|step| step.frame + 1)
        .max()
        .unwrap_or(0);
    let frames = (0..frame_count)
        .map(|index| {
            read_frame(rom, pointer(ZOID_SPRITE_FRAMES)? + index * 4).ok_or_else(too_short)
        })
        .collect::<Result<_, _>>()?;
    Ok(SpriteSheet {
        tag: String::new(),
        images: 1,
        tiles_per_image: ZOID_SPRITE_TILES,
        palette,
        tiles: Tileset::from_4bpp(tile_bytes),
        frames,
        animations,
    })
}

const ZOID_RECORDS: usize = 0x0067_0210;
const ZOID_RECORD_LEN: usize = 0x4C;
const ZOID_SPRITE_IMAGES: usize = 0x30;
const ZOID_SPRITE_PALETTE: usize = 0x34;
const ZOID_SPRITE_ANIMATIONS: usize = 0x38;
const ZOID_SPRITE_FRAMES: usize = 0x3C;
const ZOID_SPRITE_TILES: usize = 64;

/// Reads the first sprite carrying `tag`.
///
/// # Errors
///
/// Returns [`SpriteSheetError`] when no record has the tag or the ROM is too short.
pub fn sprite_sheet_by_tag(rom: &[u8], tag: &str) -> Result<SpriteSheet, SpriteSheetError> {
    (1..SPRITE_COUNT)
        .find(|id| {
            let offset = SPRITE_TABLE_OFFSET + id * SPRITE_RECORD_LEN + 16;
            rom.get(offset..offset + SPRITE_TAG_LEN) == Some(tag.as_bytes())
        })
        .map_or_else(
            || {
                Err(SpriteSheetError::NoSuchTag {
                    tag: tag.to_owned(),
                })
            },
            |id| sprite_sheet(rom, id),
        )
}

const OBJECT_TABLE_OFFSET: usize = 0x0032_82B4;
const OBJECT_TABLE_ENTRY_LEN: usize = 8;
const OBJECT_LEN: usize = 20;
const OBJECT_SPRITE_LOOKUP: u16 = 0x8000;
const OBJECT_NO_SCRIPT: u32 = 0x8000_0000;
const OBJECT_EVENT_FLAG: u32 = 0x8000_0000;
const CHEST_BEHAVIOR: u16 = 4;

/// An object placed on a map: the player (object 0) or a character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapObject {
    /// Sprite id (0 is the carrier, `mz10`), bit 15 set to use the party's
    /// Zoid.
    pub sprite: u16,
    /// OBJ palette slot the game reserves for it.
    pub palette_slot: usize,
    /// Metatile column the object stands on.
    pub column: usize,
    /// Metatile row the object stands on.
    pub row: usize,
    /// Script reference, `None` when the object has none: bit 31 set names a
    /// dialogue string by index, otherwise it points at code.
    pub script: Option<u32>,
    /// Kind: 0 for the player, 1 or 2 for characters, 4 for invisible triggers.
    pub kind: u16,
    /// Extra parameter of the kind; meaning not modeled.
    pub parameter: u16,
    /// Animation the object starts with.
    pub animation: usize,
    /// Behavior: 0 for characters, 1 for map Zoids, 2 for furniture-like sprites.
    pub behavior: u16,
}

impl MapObject {
    /// The sprite sheet to draw, when the object names a fixed one.
    #[must_use]
    pub fn sprite_sheet_id(&self) -> Option<usize> {
        (self.sprite & OBJECT_SPRITE_LOOKUP == 0).then_some(usize::from(self.sprite))
    }

    /// The dialogue string index the object says when spoken to: a script
    /// reference with bit 31 set names it in the low half-word.
    #[must_use]
    pub fn event_id(&self) -> Option<u16> {
        self.script
            .filter(|script| script & OBJECT_EVENT_FLAG != 0)
            .and_then(|script| u16::try_from(script & u32::from(u16::MAX)).ok())
    }

    /// The chest number of an object of behavior 4 (the low half-word of its
    /// script reference, 0 when it has none).
    #[must_use]
    pub fn chest(&self) -> Option<u16> {
        (self.behavior == CHEST_BEHAVIOR)
            .then(|| u16::try_from(self.script.unwrap_or(0) & u32::from(u16::MAX)).unwrap_or(0))
    }

    /// What the object runs when spoken to.
    #[must_use]
    pub fn script_kind(&self) -> Option<ObjectScript> {
        let script = self.script?;
        Some(match self.event_id() {
            Some(index) => ObjectScript::Dialogue(index),
            None => ObjectScript::Code(script & !1),
        })
    }
}

/// What a chest holds (the 12-byte records at ROM `0x66BCE4`, read by the
/// routine at `0x080376A8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Treasure {
    /// Money in G.
    pub money: u32,
    /// A Zoid (a picture id), when the chest holds one.
    pub zoid: Option<u8>,
    /// An item, when it holds one.
    pub item: Option<u16>,
    /// Two more kinds of reward the port does not model yet (bytes 8 and 9).
    pub other: [Option<u8>; 2],
}

const TREASURE_TABLE: usize = 0x0066_BCE4;
const TREASURE_LEN: usize = 12;
const NO_TREASURE_BYTE: u8 = 0xFF;
const NO_TREASURE_ITEM: u16 = 0xFFFF;
/// Flag `CHEST_FLAG_BASE + n` marks chest `n` as opened.
pub const CHEST_FLAG_BASE: u16 = 0x1E;

/// Reads what chest `chest` holds.
#[must_use]
pub fn treasure(rom: &[u8], chest: usize) -> Option<Treasure> {
    let at = TREASURE_TABLE + chest * TREASURE_LEN;
    let bytes = rom.get(at..at + TREASURE_LEN)?;
    let byte = |value: u8| (value != NO_TREASURE_BYTE).then_some(value);
    let item = u16::from_le_bytes([bytes[6], bytes[7]]);
    Some(Treasure {
        money: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        zoid: (bytes[4] != 0).then_some(bytes[4]),
        item: (item != NO_TREASURE_ITEM).then_some(item),
        other: [byte(bytes[8]), byte(bytes[9])],
    })
}

/// What an object runs when the player speaks to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectScript {
    /// A string of the `dialogue` table; for a chest, the chest's number.
    Dialogue(u16),
    /// Thumb code at this ROM address.
    Code(u32),
}

/// Reads `count` objects in the map objects' format from the list at ROM
/// address `address` (`0x08xxxxxx`), as cutscenes pass to the scene loader.
///
/// # Errors
///
/// Returns [`MapError::TooShort`] when the list is outside the ROM.
pub fn objects_at(rom: &[u8], address: u32, count: usize) -> Result<Vec<MapObject>, MapError> {
    let too_short = || MapError::TooShort {
        len: rom.len(),
        index: 0,
    };
    let list = address
        .checked_sub(ROM_BASE)
        .and_then(|offset| usize::try_from(offset).ok())
        .ok_or_else(too_short)?;
    (0..count)
        .map(|index| read_object(rom, list + index * OBJECT_LEN).ok_or_else(too_short))
        .collect()
}

fn read_object(rom: &[u8], at: usize) -> Option<MapObject> {
    let bytes = rom.get(at..at + OBJECT_LEN)?;
    let half = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let script = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    Some(MapObject {
        sprite: half(0),
        palette_slot: usize::from(half(2)),
        column: usize::from(half(4)),
        row: usize::from(half(6)),
        script: (script != OBJECT_NO_SCRIPT).then_some(script),
        kind: half(12),
        parameter: half(14),
        animation: usize::from(half(16)),
        behavior: half(18),
    })
}

/// Reads the objects of map `map`; object 0 is the player's entry.
///
/// # Errors
///
/// Returns [`MapError`] when the map is out of range or the ROM is too
/// short for its object list.
pub fn map_objects(rom: &[u8], map: usize) -> Result<Vec<MapObject>, MapError> {
    if map >= MAP_COUNT {
        return Err(MapError::NoSuchMap { index: map });
    }
    let too_short = || MapError::TooShort {
        len: rom.len(),
        index: map,
    };
    let entry = OBJECT_TABLE_OFFSET + map * OBJECT_TABLE_ENTRY_LEN;
    let entry = rom
        .get(entry..entry + OBJECT_TABLE_ENTRY_LEN)
        .ok_or_else(too_short)?;
    let count = usize::from(u16::from_le_bytes([entry[0], entry[1]]));
    let list = rom_offset(&entry[4..8]).ok_or_else(too_short)?;
    (0..count)
        .map(|index| read_object(rom, list + index * OBJECT_LEN).ok_or_else(too_short))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn sheet(images: usize, tiles_per_image: usize) -> SpriteSheet {
        let tiles = (0..images * tiles_per_image)
            .map(|index| [u8::try_from(index % 16).unwrap_or(0); formats::tile::TILE_PIXELS])
            .collect();
        SpriteSheet {
            tag: "ch00".to_owned(),
            images,
            tiles_per_image,
            palette: [0; 16],
            tiles: Tileset::from_pixels(tiles),
            frames: vec![
                SpriteFrame {
                    tile: 16,
                    x: -16,
                    y: -16,
                    width: 32,
                    height: 32,
                    mirrored: false,
                },
                SpriteFrame {
                    tile: 4,
                    x: 0,
                    y: 0,
                    width: 16,
                    height: 8,
                    mirrored: false,
                },
            ],
            animations: vec![],
        }
    }

    #[test]
    fn composes_square_images_in_tile_rows_of_four() {
        let sheet = sheet(2, 16);
        let image = sheet.image(1).unwrap();
        assert_eq!((image.width, image.height), (32, 32));
        assert_eq!(image.indices[0], 0);
        assert_eq!(image.indices[8], 1);
        assert_eq!(image.indices[8 * 32], 4);
        assert_eq!(sheet.image(2), None);
    }

    #[test]
    fn composes_frames_from_their_tile_block() {
        assert_eq!(sheet(1, 16).frame_image(0), None);
        let sheet = sheet(2, 16);
        let image = sheet.frame_image(0).unwrap();
        assert_eq!((image.width, image.height), (32, 32));
        assert_eq!(image.indices[0], 0);
        let small = sheet.frame_image(1).unwrap();
        assert_eq!((small.width, small.height), (16, 8));
        assert_eq!(small.indices[0], 4);
        assert_eq!(small.indices[8], 5);
        assert_eq!(sheet.frame_image(2), None);
    }

    #[test]
    fn rejects_images_that_do_not_form_whole_rows() {
        assert_eq!(sheet(1, 6).image(0), None);
    }

    fn put(rom: &mut [u8], at: usize, bytes: &[u8]) {
        rom[at..at + bytes.len()].copy_from_slice(bytes);
    }

    fn pointer(offset: usize) -> [u8; 4] {
        (0x0800_0000 + u32::try_from(offset).unwrap()).to_le_bytes()
    }

    /// A ROM holding sprite 2 with two idle-style animations.
    fn rom_with_sprite_2() -> Vec<u8> {
        let data = SPRITE_TABLE_OFFSET + SPRITE_COUNT * SPRITE_RECORD_LEN;
        let (palette, tiles, frames, animations, sequences) = (
            data,
            data + 32,
            data + 32 + 3 * 512,
            data + 2000,
            data + 2100,
        );
        let mut rom = vec![0; data + 2200];
        let record = SPRITE_TABLE_OFFSET + 2 * SPRITE_RECORD_LEN;
        put(&mut rom, record, &pointer(palette));
        put(&mut rom, record + 4, &pointer(tiles));
        put(&mut rom, record + 8, &pointer(animations));
        put(&mut rom, record + 12, &pointer(frames + 100));
        put(&mut rom, record + 16, b"ch01");
        put(&mut rom, record + 20, &3u16.to_le_bytes());
        put(&mut rom, record + 28, &16u32.to_le_bytes());
        rom[palette + 2] = 0x7F;
        rom[tiles + 512] = 0x21;
        rom[tiles + 1024] = 0x02;
        for (index, tile) in [0u16, 16, 32].iter().enumerate() {
            put(
                &mut rom,
                frames + 100 + index * 4,
                &pointer(frames + index * 24),
            );
            put(&mut rom, frames + index * 24, &tile.to_le_bytes());
            put(
                &mut rom,
                frames + index * 24 + 2,
                &u16::from(index == 2).to_le_bytes(),
            );
            put(&mut rom, frames + index * 24 + 4, &(-16i16).to_le_bytes());
            put(&mut rom, frames + index * 24 + 6, &(-8i16).to_le_bytes());
            put(&mut rom, frames + index * 24 + 8, &32u16.to_le_bytes());
            put(&mut rom, frames + index * 24 + 10, &32u16.to_le_bytes());
        }
        put(&mut rom, animations, &pointer(sequences));
        put(&mut rom, animations + 4, &pointer(sequences + 12));
        let steps: [u16; 8] = [0, 8, 1, 8, 0xFFFF, 0, 2, 4];
        for (i, half) in steps.iter().enumerate() {
            put(&mut rom, sequences + 2 * i, &half.to_le_bytes());
        }
        put(&mut rom, sequences + 16, &0xFFFFu16.to_le_bytes());
        rom
    }

    #[test]
    fn reads_sprite_records_with_frames_and_animations() {
        let rom = rom_with_sprite_2();
        let sheet = sprite_sheet(&rom, 2).unwrap();
        assert_eq!(sheet.tag, "ch01");
        assert_eq!((sheet.images, sheet.tiles_per_image), (3, 16));
        assert_eq!(sheet.palette[1], 0x7F);
        assert_eq!(sheet.image(1).unwrap().indices[0], 1);
        assert_eq!(
            sheet.animations,
            vec![
                vec![
                    AnimationStep {
                        frame: 0,
                        duration: 8
                    },
                    AnimationStep {
                        frame: 1,
                        duration: 8
                    },
                ],
                vec![AnimationStep {
                    frame: 2,
                    duration: 4
                }],
            ]
        );
        assert_eq!(sheet.frames.len(), 3);
        assert_eq!(
            sheet.frames[2],
            SpriteFrame {
                tile: 32,
                x: -16,
                y: -8,
                width: 32,
                height: 32,
                mirrored: true,
            }
        );
        assert_eq!(sheet.frame_image(2).unwrap().indices[0], 2);
        assert_eq!(
            sprite_sheet(&rom, SPRITE_COUNT),
            Err(SpriteSheetError::NoSuchSprite { id: SPRITE_COUNT })
        );
        assert_eq!(sprite_sheet_by_tag(&rom, "ch01").unwrap().tag, "ch01");
        assert!(matches!(
            sprite_sheet(&rom, 3),
            Err(SpriteSheetError::TooShort { id: 3, .. })
        ));
    }

    #[test]
    fn reads_the_picture_a_zoid_record_carries() {
        let data = ZOID_RECORDS + 0x40 * ZOID_RECORD_LEN;
        let (palette, tiles, frame, frames, animations, steps) = (
            data,
            data + 32,
            data + 2100,
            data + 2130,
            data + 2140,
            data + 2150,
        );
        let mut rom = vec![0; data + 2200];
        let record = ZOID_RECORDS + 3 * ZOID_RECORD_LEN;
        put(&mut rom, record + ZOID_SPRITE_IMAGES, &pointer(tiles));
        put(&mut rom, record + ZOID_SPRITE_PALETTE, &pointer(palette));
        put(
            &mut rom,
            record + ZOID_SPRITE_ANIMATIONS,
            &pointer(animations),
        );
        put(&mut rom, record + ZOID_SPRITE_FRAMES, &pointer(frames));
        rom[palette + 2] = 0x1F;
        rom[tiles + 63 * TILE_LEN] = 0x03;
        put(&mut rom, frames, &pointer(frame));
        for (i, half) in [0u16, 0, 0xFFE0, 0xFFC0, 64, 64].iter().enumerate() {
            put(&mut rom, frame + 2 * i, &half.to_le_bytes());
        }
        put(&mut rom, animations, &pointer(steps));
        for (i, half) in [0u16, 1, 0xFFFF, 0].iter().enumerate() {
            put(&mut rom, steps + 2 * i, &half.to_le_bytes());
        }
        let sheet = zoid_status_sprite(&rom, 3).unwrap();
        assert_eq!(sheet.palette[1], 0x1F);
        assert_eq!((sheet.frames[0].x, sheet.frames[0].y), (-32, -64));
        let image = sheet.frame_image(0).unwrap();
        assert_eq!((image.width, image.height), (64, 64));
        assert_eq!(image.indices[56 * 64 + 56], 3);
        assert!(matches!(
            zoid_status_sprite(&rom, 4),
            Err(SpriteSheetError::TooShort { id: 4, .. })
        ));
    }

    #[test]
    fn reads_map_objects() {
        let list = OBJECT_TABLE_OFFSET + MAP_COUNT * OBJECT_TABLE_ENTRY_LEN;
        let mut rom = vec![0; list + 2 * OBJECT_LEN];
        let entry = OBJECT_TABLE_OFFSET + 4 * OBJECT_TABLE_ENTRY_LEN;
        put(&mut rom, entry, &2u16.to_le_bytes());
        put(&mut rom, entry + 4, &pointer(list));
        let halves: [u16; 20] = [
            0x98, 0, 0, 0, 0, 0x8000, 0, 0, 0, 0, 0xF6, 4, 6, 2, 0x02E2, 0x8000, 1, 0, 0, 2,
        ];
        for (i, half) in halves.iter().enumerate() {
            put(&mut rom, list + 2 * i, &half.to_le_bytes());
        }
        let objects = map_objects(&rom, 4).unwrap();
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].sprite_sheet_id(), Some(0x98));
        assert_eq!(objects[0].script, None);
        assert_eq!(objects[0].event_id(), None);
        assert_eq!(objects[1].event_id(), Some(0x2E2));
        assert_eq!(
            MapObject {
                script: Some(0x0800_C73D),
                ..objects[1].clone()
            }
            .event_id(),
            None
        );
        assert_eq!(
            objects[1],
            MapObject {
                sprite: 0xF6,
                palette_slot: 4,
                column: 6,
                row: 2,
                script: Some(0x8000_02E2),
                kind: 1,
                parameter: 0,
                animation: 0,
                behavior: 2,
            }
        );
        assert_eq!(
            MapObject {
                sprite: 0x8001,
                ..objects[1].clone()
            }
            .sprite_sheet_id(),
            None
        );
        assert!(matches!(
            map_objects(&rom, 5),
            Err(MapError::TooShort { .. })
        ));
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
                id: 0x8001,
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

    /// Wraps `data` as an LZ77 block of literal runs.
    fn stored_lz77(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0x10];
        out.extend(&u32::try_from(data.len()).unwrap().to_le_bytes()[..3]);
        for chunk in data.chunks(8) {
            out.push(0);
            out.extend(chunk);
        }
        out
    }

    #[test]
    fn reads_the_logo_the_kana_table_and_the_name_entry_scripts() {
        let mut rom = vec![0; KANA_TABLE_OFFSET + KANA_TABLE_ROWS * KANA_COLUMNS * 2];
        put(&mut rom, LOGO_MAP_OFFSET, &2u16.to_le_bytes());
        put(&mut rom, LOGO_MAP_OFFSET + 2, &1u16.to_le_bytes());
        rom[LOGO_TILES_OFFSET + 2 * TILE_8BPP_LEN] = 7;
        put(&mut rom, LOGO_PALETTE_OFFSET + 14, &0x7FFFu16.to_le_bytes());
        let read = logo(&rom).unwrap();
        assert_eq!((read.map.width, read.map.height), (30, 20));
        assert_eq!(read.tiles.len(), 3);
        assert_eq!(read.tiles.tile(2).map(|tile| tile[0]), Some(7));
        assert_eq!(read.palette[7], 0x7FFF);
        put(&mut rom, KANA_TABLE_OFFSET, &[0x41, 0x83, 0x40, 0x81]);
        let table = kana_table(&rom).unwrap();
        assert_eq!(table.len(), KANA_TABLE_ROWS);
        assert_eq!(&table[0][..2], &['ア', '\u{3000}']);
        assert_eq!(NAME_ENTRY_SCRIPTS.count, 5);
        assert!(matches!(
            logo(&rom[..LOGO_MAP_OFFSET]),
            Err(BootError::TooShort {
                what: "logo map",
                ..
            })
        ));
    }

    #[test]
    fn reads_the_title_and_name_entry_graphics() {
        let mut rom = vec![0; NAME_ENTRY_SPRITES[1].0 + 0x1000];
        let mut tile = vec![0u8; 32];
        tile[0] = 0x21;
        put(&mut rom, TITLE_TILES_OFFSET, &stored_lz77(&tile));
        let mut picture = vec![0u8; TILE_8BPP_LEN * 2];
        picture[TILE_8BPP_LEN] = 9;
        put(&mut rom, TITLE_PICTURE_OFFSET, &stored_lz77(&picture));
        put(&mut rom, TITLE_TEXT_TILES_OFFSET, &stored_lz77(&tile));
        put(
            &mut rom,
            TITLE_EXTRA_PALETTES[3] + 2,
            &0x001Fu16.to_le_bytes(),
        );
        let read = title(&rom).unwrap();
        assert_eq!(read.tiles.len(), 1);
        assert_eq!(
            read.tiles.tile(0).map(|tile| [tile[0], tile[1]]),
            Some([1, 2])
        );
        assert_eq!(read.picture.tile(1).map(|tile| tile[0]), Some(9));
        assert_eq!(read.palettes.len(), 15);
        assert_eq!(read.palettes[14][1], 0x001F);
        assert_eq!(read.sprite_palettes[1][1], 0x001F);
        put(&mut rom, NAME_ENTRY_PICTURE_OFFSET, &stored_lz77(&picture));
        put(
            &mut rom,
            NAME_ENTRY_PICTURE_PALETTE_OFFSET,
            &stored_lz77(&[0x1F, 0x00, 0xE0, 0x03]),
        );
        for (tiles, palette) in NAME_ENTRY_SPRITES {
            put(&mut rom, tiles, &stored_lz77(&tile));
            let mut colors = [0u8; PALETTE_LEN];
            colors[..2].copy_from_slice(&[0x1F, 0x7C]);
            put(&mut rom, palette, &stored_lz77(&colors));
        }
        let graphics = name_entry_graphics(&rom).unwrap();
        assert_eq!(graphics.picture.len(), 2);
        assert_eq!(graphics.picture_palette, [0x001F, 0x03E0]);
        assert_eq!(graphics.cursor.tiles.len(), 1);
        assert_eq!(graphics.cursor.palette[0], 0x7C1F);
        assert!(title(&rom[..TITLE_TILES_OFFSET]).is_err());
    }

    #[test]
    fn reads_the_pause_wallpaper() {
        let mut rom = vec![0; PAUSE_WALLPAPER_LOGO_OFFSET + 0x1000];
        let mut tile = vec![0u8; TILE_8BPP_LEN];
        tile[0] = 5;
        put(&mut rom, PAUSE_WALLPAPER_TILES_OFFSET, &stored_lz77(&tile));
        put(
            &mut rom,
            PAUSE_WALLPAPER_PALETTE_OFFSET,
            &stored_lz77(&[0x1F, 0x00]),
        );
        let mut map = vec![0u8; PAUSE_WALLPAPER_MAP_SIDE * PAUSE_WALLPAPER_MAP_SIDE * 2];
        map[2] = 1;
        put(&mut rom, PAUSE_WALLPAPER_TEXTURE_OFFSET, &stored_lz77(&map));
        put(&mut rom, PAUSE_WALLPAPER_LOGO_OFFSET, &stored_lz77(&map));
        let wallpaper = pause_wallpaper(&rom).unwrap();
        assert_eq!(wallpaper.tiles.len(), 1);
        assert_eq!(wallpaper.palette, [0x001F]);
        assert_eq!(wallpaper.logo.wrapping(1, 0), 1);
        assert_eq!(wallpaper.texture.width, 32);
        assert_eq!(PAUSE_MENU_SCRIPTS.count, 698);
    }

    #[test]
    fn reads_the_experience_table() {
        let mut rom = vec![0; EXPERIENCE_TABLE + 12];
        rom[EXPERIENCE_TABLE + 4..EXPERIENCE_TABLE + 8].copy_from_slice(&14u32.to_le_bytes());
        rom[EXPERIENCE_TABLE + 8..EXPERIENCE_TABLE + 12].copy_from_slice(&56u32.to_le_bytes());
        assert_eq!(experience_to_next(&rom, 1), Some(14));
        assert_eq!(experience_to_next(&rom, 2), Some(56));
        assert_eq!(experience_to_next(&rom, 3), None);
        assert_eq!(experience_to_next(&rom, EXPERIENCE_LEVELS), None);
    }

    #[test]
    fn reads_a_map_song_from_its_record() {
        let mut rom = vec![0; MAP_TABLE_OFFSET + 3 * MAP_RECORD_LEN];
        rom[MAP_TABLE_OFFSET + 2 * MAP_RECORD_LEN + 8] = 0x0B;
        assert_eq!(map_music(&rom, 2), Some(11));
        assert_eq!(map_music(&rom, 1), Some(0));
        assert_eq!(map_music(&[0; 16], 0), None);
        assert_eq!(map_music(&[0; 16], MAP_COUNT), None);
        assert_eq!(SONG_TABLE + SONG_COUNT * 8, 0x0056_7B98);
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
            cell_tiles: METATILE_TILES,
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
