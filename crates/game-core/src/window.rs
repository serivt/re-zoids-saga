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
const PROMPT: usize = 0x1C;
const LIGHT_PROMPT: usize = 0x1D;
const MORE_ABOVE: usize = 0x32;
const MORE_BELOW: usize = 0x34;
/// The page marks' tiles (`0x08040BA8`): the left one's top half, then the
/// right one's; each lower half is the next tile flipped vertically.
const PAGE_LEFT: usize = 0x36;
const PAGE_RIGHT: usize = 0x38;
const LIGHT_TOP_LEFT: usize = 0x12;
const LIGHT_TOP_RIGHT: usize = 0x13;
const LIGHT_BOTTOM_LEFT: usize = 0x14;
const LIGHT_BOTTOM_RIGHT: usize = 0x15;
const LIGHT_TOP: usize = 0x16;
const LIGHT_BOTTOM: usize = 0x17;
const LIGHT_LEFT: usize = 0x18;
const LIGHT_RIGHT: usize = 0x19;
const CURSOR_LEFT: [usize; 2] = [0x3B, 0x3C];
const CURSOR_RIGHT: [usize; 2] = [0x3D, 0x3E];

/// How a window's border is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStyle {
    /// The striped border of dialogue boxes.
    Standard,
    /// The thin border of menus and fields.
    Light,
    /// No border: only the fill.
    None,
}

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
        self.draw_framed(frame, column, row, columns, rows, FrameStyle::Standard);
    }

    /// Draws a window with the given border style.
    pub fn draw_framed(
        &self,
        frame: &mut Frame,
        column: usize,
        row: usize,
        columns: usize,
        rows: usize,
        style: FrameStyle,
    ) {
        for r in 0..rows {
            for c in 0..columns {
                let place = (edge(r, rows), edge(c, columns));
                let tile = match style {
                    FrameStyle::Standard => standard_tile(place),
                    FrameStyle::Light => light_tile(place),
                    FrameStyle::None => FILL,
                };
                self.draw_tile(frame, column + c, row + r, tile);
            }
        }
    }

    /// Draws the menu cursor brackets around the two tile rows at `row`,
    /// on columns `left` and `right`.
    pub fn draw_cursor(&self, frame: &mut Frame, left: usize, right: usize, row: usize) {
        for (i, (l, r)) in CURSOR_LEFT.iter().zip(CURSOR_RIGHT).enumerate() {
            self.draw_tile(frame, left, row + i, *l);
            self.draw_tile(frame, right, row + i, r);
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

    /// Draws the "more text" prompt on the border at tile `(column, row)`,
    /// the light border's own on a light window.
    pub fn draw_prompt(&self, frame: &mut Frame, column: usize, row: usize, style: FrameStyle) {
        let tile = if style == FrameStyle::Light {
            LIGHT_PROMPT
        } else {
            PROMPT
        };
        self.draw_tile(frame, column, row, tile);
    }

    /// Draws the marks a scrolled menu shows at the middle of its top and
    /// bottom borders when lines are hidden above or below.
    pub fn draw_scroll_marks(
        &self,
        frame: &mut Frame,
        column: usize,
        rows: (usize, usize),
        (above, below): (bool, bool),
    ) {
        if above {
            self.draw_tile(frame, column, rows.0, MORE_ABOVE);
        }
        if below {
            self.draw_tile(frame, column, rows.1, MORE_BELOW);
        }
    }

    /// Draws the marks a paged menu shows on its left and right borders,
    /// two tiles high around the middle row `row`, when pages lie before or
    /// after the shown one.
    pub fn draw_page_marks(
        &self,
        frame: &mut Frame,
        columns: (usize, usize),
        row: usize,
        (before, after): (bool, bool),
    ) {
        for (shown, column, tile) in [
            (before, columns.0, PAGE_LEFT),
            (after, columns.1, PAGE_RIGHT),
        ] {
            if shown {
                self.draw_tile(frame, column, row - 1, tile);
                self.draw_flipped_tile(frame, column, row, tile + 1);
            }
        }
    }

    fn draw_tile(&self, frame: &mut Frame, column: usize, row: usize, tile: usize) {
        let Some(pixels) = self.tiles.tile(tile) else {
            return;
        };
        self.draw_pixels(frame, column, row, pixels);
    }

    /// Draws a tile upside down.
    fn draw_flipped_tile(&self, frame: &mut Frame, column: usize, row: usize, tile: usize) {
        let Some(pixels) = self.tiles.tile(tile) else {
            return;
        };
        let mut flipped = *pixels;
        for (target, source) in flipped
            .chunks_exact_mut(TILE_SIZE)
            .zip(pixels.chunks_exact(TILE_SIZE).rev())
        {
            target.copy_from_slice(source);
        }
        self.draw_pixels(frame, column, row, &flipped);
    }

    fn draw_pixels(&self, frame: &mut Frame, column: usize, row: usize, pixels: &[u8]) {
        let image = IndexedImage {
            width: TILE_SIZE,
            height: TILE_SIZE,
            indices: pixels,
        };
        let position = (pixel_offset(column), pixel_offset(row));
        draw_indexed(frame, position, image, &self.palette, None);
    }
}

fn standard_tile(place: (Edge, Edge)) -> usize {
    match place {
        (Edge::First, Edge::First) => TOP_LEFT,
        (Edge::First, Edge::Last) => TOP_RIGHT,
        (Edge::Last, Edge::First) => BOTTOM_LEFT,
        (Edge::Last, Edge::Last) => BOTTOM_RIGHT,
        (Edge::First, Edge::Inside) => TOP,
        (Edge::Last, Edge::Inside) => BOTTOM,
        (Edge::Inside, Edge::First) => LEFT,
        (Edge::Inside, Edge::Last) => RIGHT,
        (Edge::Inside, Edge::Inside) => FILL,
    }
}

fn light_tile(place: (Edge, Edge)) -> usize {
    match place {
        (Edge::First, Edge::First) => LIGHT_TOP_LEFT,
        (Edge::First, Edge::Last) => LIGHT_TOP_RIGHT,
        (Edge::Last, Edge::First) => LIGHT_BOTTOM_LEFT,
        (Edge::Last, Edge::Last) => LIGHT_BOTTOM_RIGHT,
        (Edge::First, Edge::Inside) => LIGHT_TOP,
        (Edge::Last, Edge::Inside) => LIGHT_BOTTOM,
        (Edge::Inside, Edge::First) => LIGHT_LEFT,
        (Edge::Inside, Edge::Last) => LIGHT_RIGHT,
        (Edge::Inside, Edge::Inside) => FILL,
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
