//! Drawing text-mode backgrounds: a tilemap over 4bpp tiles with 16-color
//! palettes (GBATEK, "LCD VRAM BG Screen Data Format").

use platform::{Frame, Rgb};

use super::Palette;

/// Side of a tile in pixels.
pub const TILE_SIZE: usize = 8;
/// Pixels of one tile.
pub const TILE_PIXELS: usize = TILE_SIZE * TILE_SIZE;

/// The fields of a 16-bit tilemap entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TileMapEntry {
    /// Tile index within the character block.
    pub tile: u16,
    /// Mirrored left to right.
    pub flip_x: bool,
    /// Mirrored top to bottom.
    pub flip_y: bool,
    /// Palette bank, 0–15.
    pub palette: u8,
}

impl TileMapEntry {
    /// Decodes a raw entry.
    #[must_use]
    pub const fn from_u16(raw: u16) -> Self {
        Self {
            tile: raw & 0x03FF,
            flip_x: raw & 0x0400 != 0,
            flip_y: raw & 0x0800 != 0,
            palette: (raw >> 12) as u8,
        }
    }
}

/// The sixteen 16-color palettes of background palette RAM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteBank {
    palettes: Vec<Palette>,
}

impl PaletteBank {
    /// Builds a bank from BGR555 palettes; missing banks are black.
    #[must_use]
    pub fn from_bgr555(palettes: &[[u16; 16]]) -> Self {
        let mut all: Vec<Palette> = palettes
            .iter()
            .take(16)
            .map(|colors| Palette::new(colors.map(Palette::from_bgr555)))
            .collect();
        all.resize_with(16, || Palette::new([Rgb::default(); 16]));
        Self { palettes: all }
    }

    /// Palette `index` (0–15).
    #[must_use]
    pub fn palette(&self, index: u8) -> &Palette {
        &self.palettes[usize::from(index & 0x0F)]
    }
}

/// Fills the whole frame with a background whose top-left cell is map cell
/// `(scroll_x, scroll_y)`. `entry` returns the raw tilemap entry at a cell,
/// wrapping as the caller sees fit; `tile` returns a tile's palette indices.
/// With `transparent`, pixels of index 0 are left untouched, as for
/// backgrounds drawn above the backdrop.
pub fn draw_background<'t>(
    frame: &mut Frame,
    entry: impl Fn(usize, usize) -> u16,
    tile: impl Fn(usize) -> Option<&'t [u8; TILE_PIXELS]>,
    palettes: &PaletteBank,
    (scroll_x, scroll_y): (usize, usize),
    transparent: bool,
) {
    let columns = frame.width().div_ceil(TILE_SIZE);
    let rows = frame.height().div_ceil(TILE_SIZE);
    for row in 0..rows {
        for column in 0..columns {
            let cell = TileMapEntry::from_u16(entry(scroll_x + column, scroll_y + row));
            let Some(pixels) = tile(usize::from(cell.tile)) else {
                continue;
            };
            let palette = palettes.palette(cell.palette);
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let source_x = if cell.flip_x { TILE_SIZE - 1 - x } else { x };
                    let source_y = if cell.flip_y { TILE_SIZE - 1 - y } else { y };
                    let index = pixels[source_y * TILE_SIZE + source_x];
                    if transparent && index == 0 {
                        continue;
                    }
                    frame.set_pixel(
                        column * TILE_SIZE + x,
                        row * TILE_SIZE + y,
                        palette.color(index),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiles() -> [[u8; TILE_PIXELS]; 2] {
        let mut second = [0u8; TILE_PIXELS];
        second[0] = 1;
        second[1] = 2;
        [[0u8; TILE_PIXELS], second]
    }

    fn bank() -> PaletteBank {
        let mut palette = [0u16; 16];
        palette[1] = 0x001F;
        palette[2] = 0x03E0;
        PaletteBank::from_bgr555(&[palette])
    }

    #[test]
    fn decodes_entry_fields() {
        let entry = TileMapEntry::from_u16(0xC5F3);
        assert_eq!(
            entry,
            TileMapEntry {
                tile: 0x1F3,
                flip_x: true,
                flip_y: false,
                palette: 12
            }
        );
    }

    #[test]
    fn draws_flipped_tiles_with_their_palette() {
        let tiles = tiles();
        let mut frame = Frame::new(8, 8, Rgb::default());
        draw_background(
            &mut frame,
            |_, _| 0x0401,
            |i| tiles.get(i),
            &bank(),
            (0, 0),
            false,
        );
        assert_eq!(frame.pixel(7, 0), Some(Rgb::new(255, 0, 0)));
        assert_eq!(frame.pixel(6, 0), Some(Rgb::new(0, 255, 0)));
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(0, 0, 0)));
    }

    #[test]
    fn transparent_backgrounds_keep_the_frame_under_index_zero() {
        let tiles = tiles();
        let mut frame = Frame::new(8, 8, Rgb::new(9, 9, 9));
        draw_background(
            &mut frame,
            |_, _| 1,
            |i| tiles.get(i),
            &bank(),
            (0, 0),
            true,
        );
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(255, 0, 0)));
        assert_eq!(frame.pixel(2, 0), Some(Rgb::new(9, 9, 9)));
    }
}
