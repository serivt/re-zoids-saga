//! The battle screen of the real battles (`0x0800C3AC`): the grounds the
//! two sides stand on, the units' panels and the slots.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! screen's builder at `0x08031074` with the map writer at `0x080011F0`,
//! the panels (`0x08031314`, placed by `0x080312DC`) and their bars
//! (`0x08031534`), and the unit sprites' loader (`0x080317E4`); checked
//! against the VRAM, palette RAM and screenshots of a battle on the world
//! map in a reference emulator.
//!
//! BG3 is the grid the formation screen shows too (see
//! [`crate::saga_formation`]). BG2 holds the two grounds, each 15×18
//! tiles from a 24-byte record of ROM `0x66B72C` by terrain: palette,
//! tiles and map of the player's ground, then of the enemy's. A map's
//! entries start `0x3C` bytes into it. The player's ground is written at
//! tile (15, 2) with palette bank 1, the enemy's at (0, 2) with bank 2.
//!
//! BG1 holds a panel per formation slot filled, from the left: its frame
//! is the 6×4 map at ROM `0x66B882` over the LZ77-compressed tiles at ROM
//! `0x36BB40` (loaded from tile `0x30`), in palette bank 15; the name
//! fills the four middle cells of the top two rows, and the bars the
//! middle of the last two, four entries of the table at ROM `0x66B8B2` by
//! level (plus `0x13` for the second bar). A bar's level is 0 when full
//! and 24 when empty.
//!
//! The units stand at the positions of the 16-byte records of ROM
//! `0x66B484`: the player's six slots, then the enemy's six.

use formats::bgr555::parse_palette;
use formats::lz77;
use formats::tile::Tileset;

use crate::revision::locate;
use crate::saga::rom_offset;
use crate::saga_formation::FieldLayer;

const GROUNDS: usize = 0x0066_B72C;
const GROUND_RECORD_LEN: usize = 24;
const GROUND_MAP_HEADER: usize = 0x3C;
const GROUND_COLUMNS: usize = 15;
const GROUND_ROWS: usize = 18;
const GROUND_TILES: usize = 256;
const TILE_LEN: usize = 32;
const PALETTE_BYTES: usize = 32;
const PLAYER_GROUND_AT: (usize, usize) = (15, 2);
const ENEMY_GROUND_AT: (usize, usize) = (0, 2);
const PLAYER_GROUND_BANK: u8 = 1;
const ENEMY_GROUND_BANK: u8 = 2;
const PANEL_TILES: usize = 0x0036_BB40;
const PANEL_PALETTE: usize = 0x0036_BC5C;
const PANEL_MAP: usize = 0x0066_B882;
const BAR_TABLE: usize = 0x0066_B8B2;
/// The first tile the panel's frame tiles are loaded to.
pub const PANEL_FIRST_TILE: usize = 0x30;
/// Columns of a panel.
pub const PANEL_COLUMNS: usize = 6;
/// Rows of a panel.
pub const PANEL_ROWS: usize = 4;
/// A bar's levels: 0 full, [`BAR_EMPTY`] empty.
pub const BAR_EMPTY: u8 = 24;
const BAR_CELLS: usize = 4;
const SLOT_TABLE: usize = 0x0066_B484;
const SLOT_LEN: usize = 16;
const SLOT_X: usize = 8;
const SLOT_Y: usize = 12;
/// Slots of a side.
pub const SLOTS: usize = 6;

/// The two grounds of a terrain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grounds {
    /// The player's, on the right.
    pub player: FieldLayer,
    /// The enemy's, on the left.
    pub enemy: FieldLayer,
}

/// The grounds of terrain `terrain` (the low byte of the attribute the
/// side stands on in the field), if the ROM has them; each side can have
/// its own terrain, so both are read and the caller picks.
#[must_use]
pub fn grounds(rom: &[u8], terrain: u8) -> Option<Grounds> {
    let record = locate(rom, GROUNDS + usize::from(terrain) * GROUND_RECORD_LEN);
    let pointer = |at: usize| rom.get(record + at..record + at + 4).and_then(rom_offset);
    let layer = |first: usize, origin, bank| -> Option<FieldLayer> {
        let palette_at = pointer(first)?;
        let tiles_at = pointer(first + 4)?;
        let map_at = pointer(first + 8)? + GROUND_MAP_HEADER;
        let cells = GROUND_COLUMNS * GROUND_ROWS;
        Some(FieldLayer {
            tiles: Tileset::from_4bpp(rom.get(tiles_at..tiles_at + GROUND_TILES * TILE_LEN)?),
            palette: parse_palette(rom.get(palette_at..palette_at + PALETTE_BYTES)?)?,
            map: rom
                .get(map_at..map_at + cells * 2)?
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect(),
            columns: GROUND_COLUMNS,
            rows: GROUND_ROWS,
            origin,
            bank,
        })
    };
    Some(Grounds {
        player: layer(0, PLAYER_GROUND_AT, PLAYER_GROUND_BANK)?,
        enemy: layer(12, ENEMY_GROUND_AT, ENEMY_GROUND_BANK)?,
    })
}

/// A unit's panel: the frame, its tiles and palette, and the bars.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelGraphics {
    /// The frame's tiles, the first being tile [`PANEL_FIRST_TILE`].
    pub tiles: Tileset,
    /// Their colors (palette bank 14).
    pub palette: [u16; 16],
    /// The frame's entries, row after row.
    pub map: [u16; PANEL_COLUMNS * PANEL_ROWS],
    /// A bar's four entries by level.
    pub bars: Vec<[u16; BAR_CELLS]>,
}

/// The panels' graphics, if the ROM has them.
#[must_use]
pub fn panel_graphics(rom: &[u8]) -> Option<PanelGraphics> {
    let (tiles, _) = lz77::decompress(rom.get(locate(rom, PANEL_TILES)..)?).ok()?;
    let palette_at = locate(rom, PANEL_PALETTE);
    let palette = parse_palette(rom.get(palette_at..palette_at + PALETTE_BYTES)?)?;
    let half = |at: usize| {
        let at = locate(rom, at);
        rom.get(at..at + 2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
    };
    let map = (0..PANEL_COLUMNS * PANEL_ROWS)
        .map(|cell| half(PANEL_MAP + cell * 2))
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()?;
    let bars = (0..=usize::from(BAR_EMPTY))
        .map(|level| {
            let row = (0..BAR_CELLS)
                .map(|cell| half(BAR_TABLE + level * BAR_CELLS * 2 + cell * 2))
                .collect::<Option<Vec<_>>>()?;
            row.try_into().ok()
        })
        .collect::<Option<Vec<_>>>()?;
    Some(PanelGraphics {
        tiles: Tileset::from_4bpp(&tiles),
        palette,
        map,
        bars,
    })
}

/// A bar's level for `value` of `most` (`0x08031534`): 24 cells, full at
/// 0; empty only at 0, and a sliver shows while anything is left.
#[must_use]
pub fn bar_level(value: u32, most: u32) -> u8 {
    if value == 0 {
        return BAR_EMPTY;
    }
    if value > most {
        return 0;
    }
    let filled = u64::from(value) * u64::from(BAR_EMPTY) / u64::from(most.max(1));
    match u8::try_from(filled).unwrap_or(BAR_EMPTY) {
        0 => BAR_EMPTY - 1,
        filled => BAR_EMPTY - filled,
    }
}

/// The tile column panel `index` of `count` starts at (`0x080312DC`):
/// five apart when all six slots are filled, six otherwise.
#[must_use]
pub const fn panel_column(index: usize, count: usize) -> usize {
    if count == SLOTS { index * 5 } else { index * 6 }
}

/// Where the unit of slot `slot` of the player's side (`enemy` false) or
/// the enemy's stands: its sprite's anchor.
#[must_use]
pub fn slot_anchor(rom: &[u8], enemy: bool, slot: usize) -> Option<(i32, i32)> {
    if slot >= SLOTS {
        return None;
    }
    let at = locate(
        rom,
        SLOT_TABLE + (usize::from(enemy) * SLOTS + slot) * SLOT_LEN,
    );
    let record = rom.get(at..at + SLOT_LEN)?;
    let whole = |at: usize| {
        i32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]]) >> 16
    };
    Some((whole(SLOT_X), whole(SLOT_Y)))
}

/// What the battle takes from a Zoid's 76-byte record at ROM `0x670210`
/// for an enemy unit (`0x0802B9DC`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoidBattleRecord {
    /// The record's first half-word: the unit's traits.
    pub traits: u16,
    /// The size class, 0 S, 1 M, 2 L.
    pub size: u8,
    /// The six part slots, each the slot's flags then the part id
    /// (`0xFFFF` for none) in the upper half.
    pub parts: [u32; SLOTS],
    /// Hit points, energy, SP and DF before parts and pilot.
    pub stats: (i32, i32, i16, i16),
}

const ZOID_SIZE: usize = 4;
const ZOID_PARTS: usize = 8;
const ZOID_STATS: usize = 0x40;

/// The battle's view of Zoid `zoid`'s record, if the ROM has it.
#[must_use]
pub fn zoid_battle_record(rom: &[u8], zoid: u16) -> Option<ZoidBattleRecord> {
    let at = locate(
        rom,
        crate::saga_party::ZOID_RECORDS + usize::from(zoid) * crate::saga_party::ZOID_RECORD_LEN,
    );
    let record = rom.get(at..at + crate::saga_party::ZOID_RECORD_LEN)?;
    let word = |at: usize| {
        u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]])
    };
    let half = |at: usize| u16::from_le_bytes([record[at], record[at + 1]]);
    let signed = |value: u32| i32::from_ne_bytes(value.to_ne_bytes());
    let short = |value: u16| i16::from_ne_bytes(value.to_ne_bytes());
    Some(ZoidBattleRecord {
        traits: half(0),
        size: record[ZOID_SIZE],
        parts: std::array::from_fn(|slot| word(ZOID_PARTS + slot * 4)),
        stats: (
            signed(word(ZOID_STATS)),
            signed(word(ZOID_STATS + 4)),
            short(half(ZOID_STATS + 8)),
            short(half(ZOID_STATS + 10)),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bar_empties_in_24_levels_and_keeps_a_sliver() {
        assert_eq!(bar_level(100, 100), 0);
        assert_eq!(bar_level(50, 100), 12);
        assert_eq!(bar_level(1, 100), 23);
        assert_eq!(bar_level(0, 100), BAR_EMPTY);
        assert_eq!(bar_level(120, 100), 0);
    }

    #[test]
    fn panels_sit_six_columns_apart_unless_there_are_six() {
        assert_eq!(panel_column(3, 4), 18);
        assert_eq!(panel_column(3, 6), 15);
    }

    #[test]
    fn enemy_slots_follow_the_players() {
        let mut rom = vec![0; SLOT_TABLE + 2 * SLOTS * SLOT_LEN];
        let at = SLOT_TABLE + (SLOTS + 1) * SLOT_LEN;
        rom[at + SLOT_X..at + SLOT_X + 4].copy_from_slice(&0x0053_0000i32.to_le_bytes());
        rom[at + SLOT_Y..at + SLOT_Y + 4].copy_from_slice(&0x005E_0000i32.to_le_bytes());
        assert_eq!(slot_anchor(&rom, true, 1), Some((0x53, 0x5E)));
        assert_eq!(slot_anchor(&rom, false, SLOTS), None);
    }
}
