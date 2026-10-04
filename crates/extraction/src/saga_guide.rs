//! The Zoid and character guides the title's options open: their script
//! tables and the pictures behind a Zoid's entry. See `docs/guide.md`.

use formats::tile::{TILE_PIXELS, TILE_SIZE, TileImage, TilePiece, Tileset};

use crate::revision::locate;
use crate::saga::{AnimationStep, BootError, read_animations};
use crate::string_table::StringTable;

/// Scripts the game's code runs by index; the guides' menus are 2 (Zoids)
/// and 4 (characters).
pub const SYSTEM_SCRIPTS: StringTable = StringTable {
    name: "system",
    offset: 0x006D_0880,
    count: 30,
};
/// The Zoid guide's menu: army, then type.
pub const ZOID_GUIDE_MENU: usize = 2;
/// The character guide's menu: series, then group.
pub const CHARACTER_GUIDE_MENU: usize = 4;

/// Entries of a guide row: the Zoids of one type, or the characters of
/// one group; absent ones are zero.
pub const GUIDE_ROW: usize = 20;

/// The Zoid guide's scripts: four helpers (open the three windows, clear
/// them, the next/back popup, the unknown entry), then one row of entries
/// per army and type.
pub const ZOID_GUIDE_SCRIPTS: StringTable = StringTable {
    name: "zoid-guide",
    offset: 0x0070_081C,
    count: ZOID_GUIDE_ENTRIES + ZOID_ARMIES * ZOID_TYPES * GUIDE_ROW,
};
/// Helper that opens the Zoid guide's windows.
pub const ZOID_GUIDE_OPEN: usize = 0;
/// Helper that clears them.
pub const ZOID_GUIDE_CLEAR: usize = 1;
/// Helper that asks next, previous or back.
pub const ZOID_GUIDE_POPUP: usize = 2;
/// Helper that prints an entry the player has not seen.
pub const ZOID_GUIDE_UNKNOWN: usize = 3;
/// Index of the first entry string.
pub const ZOID_GUIDE_ENTRIES: usize = 4;
/// Armies of the guide: republic, empire, kingdom, emperor.
pub const ZOID_ARMIES: usize = 4;
/// Type rows per army.
pub const ZOID_TYPES: usize = 20;

/// The character guide's scripts: three helpers (open the seven windows,
/// clear them, the next/back popup), then one row of entries per series
/// and group.
pub const CHARACTER_GUIDE_SCRIPTS: StringTable = StringTable {
    name: "character-guide",
    offset: 0x0070_56B8,
    count: CHARACTER_GUIDE_ENTRIES + CHARACTER_SERIES * CHARACTER_GROUPS * GUIDE_ROW,
};
/// Helper that opens the character guide's windows.
pub const CHARACTER_GUIDE_OPEN: usize = 0;
/// Helper that clears them.
pub const CHARACTER_GUIDE_CLEAR: usize = 1;
/// Helper that asks next, previous or back.
pub const CHARACTER_GUIDE_POPUP: usize = 2;
/// Index of the first entry string.
pub const CHARACTER_GUIDE_ENTRIES: usize = 3;
/// Series of the character guide.
pub const CHARACTER_SERIES: usize = 5;
/// Group rows per series.
pub const CHARACTER_GROUPS: usize = 10;
/// Every character's entry, in the order of the character table the save
/// keeps (`+0x34A4`, four bytes each); a character is in the guide once
/// bit `0x20` of its half-word is set.
pub const CHARACTER_ENTRIES: StringTable = StringTable {
    name: "characters",
    offset: 0x0070_6664,
    count: CHARACTER_COUNT,
};
/// Characters in the table.
pub const CHARACTER_COUNT: usize = 87;
/// The flag each series sets when one of its characters is known; its
/// groups use the flags after it.
pub const CHARACTER_SERIES_FLAGS: [u16; CHARACTER_SERIES] = [0, 5, 10, 15, 25];
/// The group flags each series' menu reads, first and last.
pub const CHARACTER_GROUP_FLAGS: [(u16, u16); CHARACTER_SERIES] =
    [(1, 4), (6, 9), (11, 14), (16, 24), (26, 29)];
/// Flags the character guide clears before it sets its own.
pub const CHARACTER_GUIDE_FLAGS: u16 = 30;
/// Sprite id of character 0's map sprite; character `n` uses the next ids.
pub const CHARACTER_SPRITE_BASE: usize = 0x98;

/// Records of `(source, destination, extra)` pointers the game decompresses
/// with the BIOS; the Zoid's tiles and palette are indexed by its picture
/// id.
const ZOID_TILES: usize = 0x006F_8974;
const ZOID_PALETTES: usize = 0x006F_9100;
/// The backdrop behind a Zoid: tiles and palette indexed by
/// `terrain × 3 + variant`.
const BACKDROP_TILES: usize = 0x006F_6934;
const BACKDROP_PALETTES: usize = 0x006F_6BBC;
/// A byte per picture id naming the Zoid's terrain.
const ZOID_TERRAIN: usize = 0x006D_3D94;
/// The 76-byte Zoid records; byte 4 is the backdrop variant.
const ZOID_RECORDS: usize = 0x0067_0210;
const ZOID_RECORD_LEN: usize = 0x4C;
const ZOID_VARIANT_FIELD: usize = 4;
const BACKDROP_VARIANTS: usize = 3;
/// The map both pictures use: 16×16 tiles in order.
const PICTURE_MAP: usize = 0x0056_4548;
const RECORD_LEN: usize = 12;
const ROM_BASE: u32 = 0x0800_0000;
const TILE_8BPP_LEN: usize = 64;
/// Side of a guide picture in tiles.
pub const PICTURE_TILES: usize = 16;
/// First palette color of a Zoid picture; its backdrop's follow it.
pub const ZOID_COLORS: usize = 0;
/// First palette color of the backdrop.
pub const BACKDROP_COLORS: usize = 64;

/// Part records by part id, 16 bytes each: LZ77 tiles, LZ77 palette, the
/// animation table and the frame table. The second part of a Zoid reads
/// the second table, the others the first.
const PART_RECORDS: [usize; PARTS_DRAWN] = [0x006F_6E44, 0x006F_77C4, 0x006F_6E44];
const PART_RECORD_LEN: usize = 16;
/// A Zoid record names its parts every four bytes from `+0x0A`; the guide
/// draws the first three.
const ZOID_PART_FIELD: usize = 0x0A;
const ZOID_PART_STRIDE: usize = 4;
const PARTS_DRAWN: usize = 3;
const NO_PART: u16 = 0xFFFF;
/// Where each part stands on the picture: seven x values then seven y
/// values per Zoid; `0xFFFF` means the seventh.
const PART_POSITIONS: usize = 0x006E_78EC;
const POSITIONS_PER_AXIS: usize = 7;
const POSITION_FALLBACK: usize = 6;
/// A frame is a list of 20-byte pieces ended by a tile of `0xFFFF`.
const PIECE_LEN: usize = 20;
const PIECE_END: u16 = 0xFFFF;
const PIECES_MAX: usize = 128;
const PIECE_MIRROR: u16 = 1;
const PART_PALETTE_COLORS: usize = 16;

/// One OBJ of a part's frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartPiece {
    /// First tile of the part's tiles.
    pub tile: usize,
    /// Whether the piece is drawn flipped left to right.
    pub mirrored: bool,
    /// Offset of its top-left corner from the part's anchor.
    pub x: i16,
    /// Vertical offset.
    pub y: i16,
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
}

/// A part the game draws over a Zoid's picture with sprites: a turret or
/// a weapon, in the battle sprites' format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoidPart {
    /// 4bpp tiles.
    pub tiles: Tileset,
    /// Its 16 colors.
    pub palette: [u16; PART_PALETTE_COLORS],
    /// Animations of frame indices and ticks.
    pub animations: Vec<Vec<AnimationStep>>,
    /// Frames, each a list of pieces.
    pub frames: Vec<Vec<PartPiece>>,
    /// Anchor on the picture, from its top-left corner.
    pub x: i16,
    /// Vertical anchor.
    pub y: i16,
}

impl ZoidPart {
    /// The pixels of `piece`.
    #[must_use]
    pub fn piece_image(&self, piece: &PartPiece) -> TileImage {
        let (columns, rows) = (piece.width / TILE_SIZE, piece.height / TILE_SIZE);
        let tiles = Tileset::from_pixels(
            (piece.tile..piece.tile + columns * rows)
                .map(|tile| self.tiles.tile(tile).copied().unwrap_or([0; TILE_PIXELS]))
                .collect(),
        );
        TileImage::compose(
            &tiles,
            &[TilePiece {
                column: 0,
                row: 0,
                columns,
                rows,
            }],
        )
    }
}

/// The parts drawn over Zoid `id`'s picture.
///
/// # Errors
///
/// Returns [`BootError`] when a part cannot be read.
pub fn zoid_parts(rom: &[u8], id: usize) -> Result<Vec<ZoidPart>, BootError> {
    let too_short = |what: &'static str| BootError::TooShort {
        len: rom.len(),
        what,
    };
    let half = |at: usize| {
        let at = locate(rom, at);
        rom.get(at..at + 2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
    };
    let mut parts = Vec::new();
    for (slot, table) in PART_RECORDS.iter().enumerate() {
        let field = ZOID_RECORDS + id * ZOID_RECORD_LEN + ZOID_PART_FIELD + slot * ZOID_PART_STRIDE;
        let part = half(field).ok_or_else(|| too_short("Zoid record"))?;
        if part == 0 || part == NO_PART {
            continue;
        }
        let record = locate(rom, table + usize::from(part) * PART_RECORD_LEN);
        let pointer = |index: usize| record_pointer(rom, record + index * 4);
        let (Some(tiles), Some(palette), Some(animations), Some(frames)) =
            (pointer(0), pointer(1), pointer(2), pointer(3))
        else {
            return Err(too_short("Zoid part"));
        };
        let animations = read_animations(rom, animations).ok_or_else(|| too_short("part"))?;
        let frame_count = animations
            .iter()
            .flatten()
            .map(|step| step.frame + 1)
            .max()
            .unwrap_or(0);
        let frames = (0..frame_count)
            .map(|index| {
                record_pointer(rom, frames + index * 4)
                    .and_then(|at| read_pieces(rom, at))
                    .ok_or_else(|| too_short("part frame"))
            })
            .collect::<Result<_, _>>()?;
        let colors = lz77(rom, palette, "part palette")?;
        let position = |axis: usize| {
            let row = PART_POSITIONS + id * POSITIONS_PER_AXIS * 4 + axis * POSITIONS_PER_AXIS * 2;
            let value = half(row + slot * 2)
                .filter(|value| *value != NO_PART)
                .or_else(|| half(row + POSITION_FALLBACK * 2))
                .unwrap_or(0);
            i16::try_from(value).unwrap_or(0)
        };
        parts.push(ZoidPart {
            tiles: Tileset::from_4bpp(&lz77(rom, tiles, "part tiles")?),
            palette: std::array::from_fn(|index| {
                colors
                    .get(index * 2..index * 2 + 2)
                    .map_or(0, |color| u16::from_le_bytes([color[0], color[1]]))
            }),
            animations,
            frames,
            x: position(0),
            y: position(1),
        });
    }
    Ok(parts)
}

fn read_pieces(rom: &[u8], mut at: usize) -> Option<Vec<PartPiece>> {
    let mut pieces = Vec::new();
    for _ in 0..PIECES_MAX {
        let bytes = rom.get(at..at + PIECE_LEN)?;
        let half = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        if half(0) == PIECE_END {
            return Some(pieces);
        }
        pieces.push(PartPiece {
            tile: usize::from(half(0)),
            mirrored: half(2) & PIECE_MIRROR != 0,
            x: i16::from_le_bytes([bytes[4], bytes[5]]),
            y: i16::from_le_bytes([bytes[6], bytes[7]]),
            width: usize::from(half(8)),
            height: usize::from(half(10)),
        });
        at += PIECE_LEN;
    }
    None
}

fn record_pointer(rom: &[u8], at: usize) -> Option<usize> {
    let bytes = rom.get(at..at + 4)?;
    let address = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let offset = usize::try_from(address.checked_sub(ROM_BASE)?).ok()?;
    (offset < rom.len()).then_some(offset)
}

/// A 256-color picture of the guide: tiles in map order and its colors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuidePicture {
    /// 8bpp tiles.
    pub tiles: Tileset,
    /// The map, 16×16 entries.
    pub map: Vec<u16>,
    /// BGR555 colors, loaded from [`ZOID_COLORS`] or [`BACKDROP_COLORS`].
    pub palette: Vec<u16>,
}

/// The picture of Zoid `id`, or `None` when the guide has none.
///
/// # Errors
///
/// Returns [`BootError`] when a block cannot be read.
pub fn zoid_picture(rom: &[u8], id: usize) -> Result<Option<GuidePicture>, BootError> {
    picture(rom, ZOID_TILES, ZOID_PALETTES, id)
}

/// The backdrop behind Zoid `id`, or `None` when the guide has none.
///
/// # Errors
///
/// Returns [`BootError`] when a block cannot be read.
pub fn zoid_backdrop(rom: &[u8], id: usize) -> Result<Option<GuidePicture>, BootError> {
    let Some(terrain) = rom.get(locate(rom, ZOID_TERRAIN + id)) else {
        return Ok(None);
    };
    let Some(variant) = rom.get(locate(
        rom,
        ZOID_RECORDS + id * ZOID_RECORD_LEN + ZOID_VARIANT_FIELD,
    )) else {
        return Ok(None);
    };
    let index = usize::from(*terrain) * BACKDROP_VARIANTS + usize::from(*variant);
    picture(rom, BACKDROP_TILES, BACKDROP_PALETTES, index)
}

fn picture(
    rom: &[u8],
    tiles: usize,
    palettes: usize,
    index: usize,
) -> Result<Option<GuidePicture>, BootError> {
    let (Some(tiles), Some(palette)) = (record(rom, tiles, index), record(rom, palettes, index))
    else {
        return Ok(None);
    };
    let tile_bytes = lz77(rom, tiles, "guide picture tiles")?;
    let palette_bytes = lz77(rom, palette, "guide picture palette")?;
    let map_at = locate(rom, PICTURE_MAP);
    let map = rom
        .get(map_at..map_at + PICTURE_TILES * PICTURE_TILES * 2)
        .ok_or(BootError::TooShort {
            len: rom.len(),
            what: "guide picture map",
        })?
        .chunks_exact(2)
        .map(|entry| u16::from_le_bytes([entry[0], entry[1]]))
        .collect();
    Ok(Some(GuidePicture {
        tiles: Tileset::from_pixels(
            tile_bytes
                .chunks_exact(TILE_8BPP_LEN)
                .map(|tile| tile.try_into().unwrap_or([0; TILE_8BPP_LEN]))
                .collect(),
        ),
        map,
        palette: palette_bytes
            .chunks_exact(2)
            .map(|color| u16::from_le_bytes([color[0], color[1]]))
            .collect(),
    }))
}

/// The pointer of record `index` of the table Rev 1 keeps at `table`,
/// read where `rom`'s release keeps it.
fn record(rom: &[u8], table: usize, index: usize) -> Option<usize> {
    record_pointer(rom, locate(rom, table + index * RECORD_LEN))
}

fn lz77(rom: &[u8], offset: usize, what: &'static str) -> Result<Vec<u8>, BootError> {
    let compressed = rom.get(offset..).ok_or(BootError::TooShort {
        len: rom.len(),
        what,
    })?;
    Ok(formats::lz77::decompress(compressed)?.0)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn lz77_literal(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0x10];
        out.extend_from_slice(&u32::try_from(data.len()).expect("small").to_le_bytes()[..3]);
        for chunk in data.chunks(8) {
            out.push(0);
            out.extend_from_slice(chunk);
        }
        out
    }

    fn put_record(rom: &mut [u8], table: usize, index: usize, source: usize) {
        let pointer = ROM_BASE + u32::try_from(source).expect("small");
        let at = table + index * RECORD_LEN;
        rom[at..at + 4].copy_from_slice(&pointer.to_le_bytes());
    }

    fn put(rom: &mut [u8], at: usize, bytes: &[u8]) {
        rom[at..at + bytes.len()].copy_from_slice(bytes);
    }

    fn synthetic_rom() -> Vec<u8> {
        let mut rom = vec![0; 0x0070_0000];
        let tiles = lz77_literal(&[7; 2 * TILE_8BPP_LEN]);
        let palette = lz77_literal(&[0x1F, 0, 0xE0, 0x03]);
        put(&mut rom, 0x0010_0000, &tiles);
        put(&mut rom, 0x0010_1000, &palette);
        put_record(&mut rom, ZOID_TILES, 2, 0x0010_0000);
        put_record(&mut rom, ZOID_PALETTES, 2, 0x0010_1000);
        rom[ZOID_TERRAIN + 2] = 6;
        rom[ZOID_RECORDS + 2 * ZOID_RECORD_LEN + ZOID_VARIANT_FIELD] = 1;
        put_record(&mut rom, BACKDROP_TILES, 19, 0x0010_0000);
        put_record(&mut rom, BACKDROP_PALETTES, 19, 0x0010_1000);
        for index in 0..PICTURE_TILES * PICTURE_TILES {
            let entry = u16::try_from(index).expect("small").to_le_bytes();
            put(&mut rom, PICTURE_MAP + index * 2, &entry);
        }
        rom
    }

    #[test]
    fn reads_a_zoid_picture_and_its_backdrop() {
        let rom = synthetic_rom();
        let picture = zoid_picture(&rom, 2).expect("readable").expect("present");
        assert_eq!(picture.tiles.len(), 2);
        assert_eq!(picture.palette, vec![0x001F, 0x03E0]);
        assert_eq!(picture.map[17], 17);
        let backdrop = zoid_backdrop(&rom, 2).expect("readable").expect("present");
        assert_eq!(backdrop.tiles.len(), 2);
    }

    fn put_word(rom: &mut [u8], at: usize, value: usize) {
        let pointer = ROM_BASE + u32::try_from(value).expect("small");
        rom[at..at + 4].copy_from_slice(&pointer.to_le_bytes());
    }

    fn put_half(rom: &mut [u8], at: usize, value: u16) {
        rom[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn reads_a_part_with_its_pieces_and_place() {
        let mut rom = synthetic_rom();
        let id = 2;
        let field = ZOID_RECORDS + id * ZOID_RECORD_LEN + ZOID_PART_FIELD;
        put_half(&mut rom, field, NO_PART);
        put_half(&mut rom, field + ZOID_PART_STRIDE, 5);
        put_half(&mut rom, field + 2 * ZOID_PART_STRIDE, NO_PART);
        let record = PART_RECORDS[1] + 5 * PART_RECORD_LEN;
        let (tiles, palette, animations, frames) =
            (0x0011_0000, 0x0011_1000, 0x0011_2000, 0x0011_3000);
        put(&mut rom, tiles, &lz77_literal(&[0x21; 3 * 32]));
        put(&mut rom, palette, &lz77_literal(&[0x1F, 0]));
        put_word(&mut rom, record, tiles);
        put_word(&mut rom, record + 4, palette);
        put_word(&mut rom, record + 8, animations);
        put_word(&mut rom, record + 12, frames);
        put_word(&mut rom, animations, 0x0011_2100);
        put(&mut rom, 0x0011_2100, &[0, 0, 4, 0, 0xFF, 0xFF, 0, 0]);
        put_word(&mut rom, frames, 0x0011_3100);
        let piece = [1u8, 0, 1, 0, 0xF8, 0xFF, 0xF0, 0xFF, 16, 0, 8, 0];
        put(&mut rom, 0x0011_3100, &piece);
        put(&mut rom, 0x0011_3100 + PIECE_LEN, &[0xFF, 0xFF]);
        let row = PART_POSITIONS + id * POSITIONS_PER_AXIS * 4;
        put_half(&mut rom, row + 2, NO_PART);
        put_half(&mut rom, row + POSITION_FALLBACK * 2, 30);
        put_half(&mut rom, row + POSITIONS_PER_AXIS * 2 + 2, 40);
        let parts = zoid_parts(&rom, id).expect("readable");
        assert_eq!(parts.len(), 1);
        let part = &parts[0];
        assert_eq!((part.x, part.y), (30, 40));
        assert_eq!(part.palette[0], 0x001F);
        assert_eq!(part.animations[0][0].duration, 4);
        assert_eq!(
            part.frames,
            vec![vec![PartPiece {
                tile: 1,
                mirrored: true,
                x: -8,
                y: -16,
                width: 16,
                height: 8,
            }]]
        );
        let image = part.piece_image(&part.frames[0][0]);
        assert_eq!((image.width, image.height), (16, 8));
        assert_eq!(image.indices[0], 1);
    }

    #[test]
    fn a_zoid_without_records_has_no_picture() {
        let rom = synthetic_rom();
        assert_eq!(zoid_picture(&rom, 3), Ok(None));
        assert_eq!(zoid_backdrop(&rom, 3), Ok(None));
    }
}
