//! The battle field the formation screen (部隊編成) shows the party on.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! screen's task at `0x08037B84`, the field loader at `0x080316FC` with the
//! map writer at `0x080011F0`, the unit sprites' loader at `0x080317E4`,
//! the cursor's at `0x08031B70` and its placement at `0x08038510`, with the
//! loads traced in a reference emulator (breakpoints on the `CpuSet`
//! wrapper at `0x0805D388`) and checked against the VRAM, palette RAM and
//! OAM the screen left.
//!
//! The field is two text backgrounds of raw 4bpp tiles, each with one
//! 16-color palette and a map the writer copies into the map shadow at a
//! tile position, adding a palette bank to each entry:
//!
//! | Layer | Tiles | Palette | Map | Size | At | Bank |
//! |---|---|---|---|---|---|---|
//! | BG3, the grid | `0x32B4E8` | `0x32D4E8` | `0x32D6E8` | 30×20 | (0, 0) | 0 |
//! | BG2, the platform | `0x35BF80` | `0x35DB80` | `0x35DDBC` | 15×18 | (0, 2) | 1 |
//!
//! The units stand at the positions of the six player slots of the table at
//! ROM `0x66B484` (16 bytes a slot: palette, first tile, entity, then x and
//! y in 16.16), less 120 pixels across on this screen. The slot cursor is
//! record 0 of the 44-byte table at ROM `0x66B5F8`: a raw palette and raw
//! tiles with their lengths, then an animation and a frame table in the
//! effect sprites' format (see [`crate::saga_battle`]).

use formats::bgr555::parse_palette;
use formats::tile::Tileset;

use crate::revision::locate;
use crate::saga::{read_steps, rom_offset};
use crate::saga_battle::{EffectSprite, read_pieces};

/// Formation slots, which are also the battle's player slots.
pub const SLOTS: usize = 6;

const TILE_LEN: usize = 32;
const LAYER_TILES: usize = 256;
const PALETTE_BYTES: usize = 32;
const GRID: LayerRecord = LayerRecord {
    tiles: 0x0032_B4E8,
    palette: 0x0032_D4E8,
    map: 0x0032_D6E8,
    columns: 30,
    rows: 20,
    origin: (0, 0),
    bank: 0,
};
const PLATFORM: LayerRecord = LayerRecord {
    tiles: 0x0035_BF80,
    palette: 0x0035_DB80,
    map: 0x0035_DDBC,
    columns: 15,
    rows: 18,
    origin: (0, 2),
    bank: 1,
};
const SLOT_TABLE: usize = 0x0066_B484;
const SLOT_LEN: usize = 16;
const SLOT_X: usize = 8;
const SLOT_Y: usize = 12;
/// How far left this screen draws the field's slots (`0x080317C0`).
const SCREEN_SHIFT: i32 = 120;
const CURSOR_RECORD: usize = 0x0066_B5F8;
const CURSOR_PALETTE: usize = 0;
const CURSOR_TILES: usize = 0xC;
const CURSOR_TILES_LEN: usize = 0x14;
const CURSOR_ANIMATIONS: usize = 0x18;
const CURSOR_FRAMES: usize = 0x1C;
const CURSOR_RECORD_LEN: usize = 0x2C;
const FRAMES_MAX: usize = 64;

struct LayerRecord {
    tiles: usize,
    palette: usize,
    map: usize,
    columns: usize,
    rows: usize,
    origin: (usize, usize),
    bank: u8,
}

/// One background of the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLayer {
    /// Its 4bpp tiles; tile 0 is clear.
    pub tiles: Tileset,
    /// Its 16 colors, loaded into palette bank [`FieldLayer::bank`].
    pub palette: [u16; 16],
    /// Map entries row after row, as the ROM stores them (palette 0).
    pub map: Vec<u16>,
    /// Columns of the map.
    pub columns: usize,
    /// Rows of the map.
    pub rows: usize,
    /// The tile the map's top-left corner is written to.
    pub origin: (usize, usize),
    /// The palette bank the writer adds to every entry.
    pub bank: u8,
}

impl FieldLayer {
    /// The map entry on screen tile `(column, row)`, with the writer's
    /// palette bank, or 0 (a clear tile) outside the map.
    #[must_use]
    pub fn entry(&self, column: usize, row: usize) -> u16 {
        let (Some(column), Some(row)) = (
            column.checked_sub(self.origin.0),
            row.checked_sub(self.origin.1),
        ) else {
            return 0;
        };
        if column >= self.columns || row >= self.rows {
            return 0;
        }
        let entry = self.map[row * self.columns + column];
        entry.wrapping_add(u16::from(self.bank) << 12)
    }
}

/// The field behind the formation: the grid (BG3) and the platform (BG2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleField {
    /// The grid, the farthest layer.
    pub grid: FieldLayer,
    /// The platform the units stand on.
    pub platform: FieldLayer,
}

/// The field the formation screen loads, if the ROM has it.
#[must_use]
pub fn battle_field(rom: &[u8]) -> Option<BattleField> {
    Some(BattleField {
        grid: field_layer(rom, &GRID)?,
        platform: field_layer(rom, &PLATFORM)?,
    })
}

/// The layer whose tiles, palette and map Rev 1 keeps where `record`
/// says, read where `rom`'s release keeps them.
fn field_layer(rom: &[u8], record: &LayerRecord) -> Option<FieldLayer> {
    let (tiles_at, palette_at, map_at) = (
        locate(rom, record.tiles),
        locate(rom, record.palette),
        locate(rom, record.map),
    );
    let tiles = rom.get(tiles_at..tiles_at + LAYER_TILES * TILE_LEN)?;
    let palette = parse_palette(rom.get(palette_at..palette_at + PALETTE_BYTES)?)?;
    let cells = record.columns * record.rows;
    let map = rom
        .get(map_at..map_at + cells * 2)?
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Some(FieldLayer {
        tiles: Tileset::from_4bpp(tiles),
        palette,
        map,
        columns: record.columns,
        rows: record.rows,
        origin: record.origin,
        bank: record.bank,
    })
}

/// Where the unit in formation slot `slot` stands on the formation screen:
/// its sprite's anchor.
#[must_use]
pub fn slot_anchor(rom: &[u8], slot: usize) -> Option<(i32, i32)> {
    if slot >= SLOTS {
        return None;
    }
    let at = locate(rom, SLOT_TABLE + slot * SLOT_LEN);
    let record = rom.get(at..at + SLOT_LEN)?;
    let whole = |at: usize| {
        let value =
            i32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]]);
        value >> 16
    };
    Some((whole(SLOT_X) - SCREEN_SHIFT, whole(SLOT_Y)))
}

/// The cursor that marks a slot.
#[must_use]
pub fn slot_cursor(rom: &[u8]) -> Option<EffectSprite> {
    let record_at = locate(rom, CURSOR_RECORD);
    let record = rom.get(record_at..record_at + CURSOR_RECORD_LEN)?;
    let pointer = |at: usize| rom_offset(&record[at..at + 4]);
    let palette_at = pointer(CURSOR_PALETTE)?;
    let palette = parse_palette(rom.get(palette_at..palette_at + PALETTE_BYTES)?)?;
    let tiles_at = pointer(CURSOR_TILES)?;
    let tiles_len = usize::from(u16::from_le_bytes([
        record[CURSOR_TILES_LEN],
        record[CURSOR_TILES_LEN + 1],
    ]));
    let tiles = rom.get(tiles_at..tiles_at + tiles_len)?;
    let first_animation = rom_offset(rom.get(pointer(CURSOR_ANIMATIONS)?..)?.get(..4)?)?;
    let animation = read_steps(rom, first_animation)?;
    let frame_table = pointer(CURSOR_FRAMES)?;
    let frame_count = animation
        .iter()
        .map(|step| step.frame + 1)
        .max()
        .unwrap_or(0)
        .min(FRAMES_MAX);
    let frames = (0..frame_count)
        .map(|index| {
            let at = frame_table + index * 4;
            read_pieces(rom, rom_offset(rom.get(at..at + 4)?)?)
        })
        .collect::<Option<Vec<_>>>()?;
    Some(EffectSprite {
        tiles: Tileset::from_4bpp(tiles),
        palette,
        frames,
        animations: vec![animation.clone()],
        animation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROM_BASE: u32 = 0x0800_0000;

    fn put_pointer(rom: &mut [u8], at: usize, target: usize) {
        let address = ROM_BASE + u32::try_from(target).unwrap_or(0);
        rom[at..at + 4].copy_from_slice(&address.to_le_bytes());
    }

    fn put_half(rom: &mut [u8], at: usize, value: u16) {
        rom[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn a_layer_places_its_map_and_adds_its_bank() {
        let mut rom = vec![0; PLATFORM.map + 15 * 18 * 2];
        put_half(&mut rom, PLATFORM.map, 0x0405);
        put_half(&mut rom, PLATFORM.map + 2, 7);
        let field = battle_field(&rom);
        let platform = field.map(|field| field.platform);
        let entry = |column, row| platform.as_ref().map(|layer| layer.entry(column, row));
        assert_eq!(entry(0, 2), Some(0x1405));
        assert_eq!(entry(1, 2), Some(0x1007));
        assert_eq!(entry(0, 1), Some(0));
        assert_eq!(entry(15, 2), Some(0));
    }

    #[test]
    fn slots_stand_shifted_left() {
        let mut rom = vec![0; SLOT_TABLE + SLOTS * SLOT_LEN];
        let at = SLOT_TABLE + 4 * SLOT_LEN;
        rom[at + SLOT_X..at + SLOT_X + 4].copy_from_slice(&0x00C7_0000i32.to_le_bytes());
        rom[at + SLOT_Y..at + SLOT_Y + 4].copy_from_slice(&0x0057_8000i32.to_le_bytes());
        assert_eq!(slot_anchor(&rom, 4), Some((79, 87)));
        assert_eq!(slot_anchor(&rom, SLOTS), None);
    }

    #[test]
    fn reads_the_cursor_and_its_frames() {
        let palette = 0x100;
        let tiles = 0x200;
        let steps = 0x300;
        let animations = 0x320;
        let frames = 0x340;
        let piece = 0x360;
        let mut rom = vec![0; CURSOR_RECORD + CURSOR_RECORD_LEN];
        put_half(&mut rom, palette + 2, 0x7FFF);
        rom[tiles] = 0x21;
        put_half(&mut rom, steps, 0);
        put_half(&mut rom, steps + 2, 4);
        put_half(&mut rom, steps + 4, 0xFFFF);
        put_pointer(&mut rom, animations, steps);
        put_pointer(&mut rom, frames, piece);
        put_half(&mut rom, piece + 4, 0xFFF0);
        put_half(&mut rom, piece + 8, 32);
        put_half(&mut rom, piece + 20, 0xFFFF);
        put_pointer(&mut rom, CURSOR_RECORD + CURSOR_PALETTE, palette);
        put_pointer(&mut rom, CURSOR_RECORD + CURSOR_TILES, tiles);
        put_half(&mut rom, CURSOR_RECORD + CURSOR_TILES_LEN, 64);
        put_pointer(&mut rom, CURSOR_RECORD + CURSOR_ANIMATIONS, animations);
        put_pointer(&mut rom, CURSOR_RECORD + CURSOR_FRAMES, frames);
        let cursor = slot_cursor(&rom);
        assert_eq!(cursor.as_ref().map(|c| c.palette[1]), Some(0x7FFF));
        assert_eq!(cursor.as_ref().map(|c| c.tiles.len()), Some(2));
        assert_eq!(
            cursor
                .as_ref()
                .and_then(|c| c.tiles.tile(0))
                .map(|t| t[..2].to_vec()),
            Some(vec![1, 2])
        );
        assert_eq!(cursor.as_ref().map(|c| c.animation.len()), Some(1));
        assert_eq!(
            cursor
                .as_ref()
                .map(|c| (c.frames.len(), c.frames[0][0].x, c.frames[0][0].width)),
            Some((1, -16, 32))
        );
    }
}
