//! Drawing text windows with the original frame tiles.

use formats::tile::{TILE_SIZE, Tileset};
use gba_runtime::ppu::{IndexedImage, Palette, draw_indexed};
use localization::TextArea;
use platform::Frame;

const FILL: usize = 0x01;
const TOP_LEFT: usize = 0x0A;
const TOP_RIGHT: usize = 0x0B;
const BOTTOM_LEFT: usize = 0x0C;
const BOTTOM_RIGHT: usize = 0x0D;
const TOP: usize = 0x0E;
const BOTTOM: usize = 0x0F;
const LEFT: usize = 0x10;
const RIGHT: usize = 0x11;
const DIVIDER_TOP: usize = 0x20;
const DIVIDER_BOTTOM: usize = 0x21;

/// Text area of the story dialogue box: the cells right of the portrait
/// divider, below the speaker's name.
pub const DIALOGUE_TEXT_AREA: TextArea = TextArea {
    columns: 22,
    rows: 2,
};

/// Draws windows from a frame tileset and its palette.
pub struct WindowPainter {
    tiles: Tileset,
    palette: Palette,
}

impl WindowPainter {
    /// Creates a painter from the frame tiles and their BGR555 palette.
    #[must_use]
    pub fn new(tiles: Tileset, bgr555: &[u16; 16]) -> Self {
        Self {
            tiles,
            palette: Palette::new(bgr555.map(Palette::from_bgr555)),
        }
    }

    /// The palette shared by the frame and the text drawn inside it.
    #[must_use]
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Draws a window whose top-left tile is at `(column, row)`, spanning
    /// `columns`×`rows` tiles including the border.
    pub fn draw_window(
        &self,
        frame: &mut Frame,
        column: usize,
        row: usize,
        columns: usize,
        rows: usize,
    ) {
        for r in 0..rows {
            for c in 0..columns {
                let tile = match (edge(r, rows), edge(c, columns)) {
                    (Edge::First, Edge::First) => TOP_LEFT,
                    (Edge::First, Edge::Last) => TOP_RIGHT,
                    (Edge::Last, Edge::First) => BOTTOM_LEFT,
                    (Edge::Last, Edge::Last) => BOTTOM_RIGHT,
                    (Edge::First, Edge::Inside) => TOP,
                    (Edge::Last, Edge::Inside) => BOTTOM,
                    (Edge::Inside, Edge::First) => LEFT,
                    (Edge::Inside, Edge::Last) => RIGHT,
                    (Edge::Inside, Edge::Inside) => FILL,
                };
                self.draw_tile(frame, column + c, row + r, tile);
            }
        }
    }

    /// Draws a vertical divider inside a window at tile `column`, from the
    /// window's top border row `row` down through `rows` tiles.
    pub fn draw_divider(&self, frame: &mut Frame, column: usize, row: usize, rows: usize) {
        for r in 0..rows {
            let tile = match edge(r, rows) {
                Edge::First => DIVIDER_TOP,
                Edge::Last => DIVIDER_BOTTOM,
                Edge::Inside => LEFT,
            };
            self.draw_tile(frame, column, row + r, tile);
        }
    }

    fn draw_tile(&self, frame: &mut Frame, column: usize, row: usize, tile: usize) {
        let Some(pixels) = self.tiles.tile(tile) else {
            return;
        };
        let image = IndexedImage {
            width: TILE_SIZE,
            height: TILE_SIZE,
            indices: pixels,
        };
        let position = (pixel_offset(column), pixel_offset(row));
        draw_indexed(frame, position, image, &self.palette, None);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    First,
    Inside,
    Last,
}

fn edge(index: usize, count: usize) -> Edge {
    if index == 0 {
        Edge::First
    } else if index + 1 == count {
        Edge::Last
    } else {
        Edge::Inside
    }
}

fn pixel_offset(tiles: usize) -> i32 {
    i32::try_from(tiles * TILE_SIZE).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::tile::TILE_LEN;
    use platform::Rgb;

    /// A tileset where tile `i` is filled with palette index `i & 0xF`.
    fn numbered_tiles() -> Tileset {
        let mut data = Vec::new();
        for i in 0..0x22u8 {
            let nibble = i & 0x0F;
            data.extend([nibble | nibble << 4; TILE_LEN]);
        }
        Tileset::from_4bpp(&data)
    }

    fn painter() -> WindowPainter {
        let mut palette = [0u16; 16];
        for (i, color) in palette.iter_mut().enumerate() {
            *color = u16::try_from(i).unwrap_or(0);
        }
        WindowPainter::new(numbered_tiles(), &palette)
    }

    fn index_at(frame: &Frame, x: usize, y: usize) -> usize {
        frame
            .pixel(x, y)
            .map_or(0xFF, |color| usize::from(color.r >> 3))
    }

    #[test]
    fn places_corners_edges_and_fill() {
        let mut frame = Frame::new(32, 24, Rgb::default());
        painter().draw_window(&mut frame, 0, 0, 4, 3);
        assert_eq!(index_at(&frame, 0, 0), TOP_LEFT & 0xF);
        assert_eq!(index_at(&frame, 31, 0), TOP_RIGHT & 0xF);
        assert_eq!(index_at(&frame, 0, 23), BOTTOM_LEFT & 0xF);
        assert_eq!(index_at(&frame, 31, 23), BOTTOM_RIGHT & 0xF);
        assert_eq!(index_at(&frame, 12, 0), TOP & 0xF);
        assert_eq!(index_at(&frame, 12, 23), BOTTOM & 0xF);
        assert_eq!(index_at(&frame, 0, 12), LEFT & 0xF);
        assert_eq!(index_at(&frame, 31, 12), RIGHT & 0xF);
        assert_eq!(index_at(&frame, 12, 12), FILL);
    }

    #[test]
    fn draws_a_divider_with_junctions() {
        let mut frame = Frame::new(16, 24, Rgb::default());
        painter().draw_divider(&mut frame, 1, 0, 3);
        assert_eq!(index_at(&frame, 8, 0), DIVIDER_TOP & 0xF);
        assert_eq!(index_at(&frame, 8, 12), LEFT & 0xF);
        assert_eq!(index_at(&frame, 8, 23), DIVIDER_BOTTOM & 0xF);
    }
}
