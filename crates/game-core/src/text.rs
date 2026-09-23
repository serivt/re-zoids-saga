//! Drawing game text with the original 8×16 font.

use formats::font::{GLYPH_HEIGHT, GLYPH_WIDTH, Glyph, GlyphIndex, shift_jis_code};
use gba_runtime::ppu::{IndexedImage, Palette, draw_indexed};
use platform::Frame;

const BACKGROUND_INDEX: u8 = 1;

/// Draws strings with a font read from a ROM image.
pub struct TextPainter<'rom> {
    rom: &'rom [u8],
    index: GlyphIndex,
    fallback: Option<Glyph>,
}

impl<'rom> TextPainter<'rom> {
    /// Creates a painter over `rom` using `index` to find glyphs and
    /// `fallback` for characters the font lacks.
    #[must_use]
    pub fn new(rom: &'rom [u8], index: GlyphIndex, fallback: Option<Glyph>) -> Self {
        Self {
            rom,
            index,
            fallback,
        }
    }

    /// Glyph for a character, or the fallback glyph when the font lacks it.
    #[must_use]
    pub fn glyph(&self, ch: char) -> Option<Glyph> {
        shift_jis_code(ch)
            .and_then(|code| self.index.glyph(self.rom, code))
            .or_else(|| self.fallback.clone())
    }

    /// Draws `text` with its top-left corner at `(x, y)`, one glyph cell per
    /// character and one glyph row per line; the background index is
    /// transparent. Returns the height drawn in pixels.
    pub fn draw(&self, frame: &mut Frame, x: i32, y: i32, text: &str, palette: &Palette) -> i32 {
        let mut lines = 0;
        for (line, content) in text.lines().enumerate() {
            lines = line + 1;
            let line_y = y + cell_offset(line, GLYPH_HEIGHT);
            for (column, ch) in content.chars().enumerate() {
                let Some(glyph) = self.glyph(ch) else {
                    continue;
                };
                let image = IndexedImage {
                    width: GLYPH_WIDTH,
                    height: GLYPH_HEIGHT,
                    indices: &glyph.pixels,
                };
                let position = (x + cell_offset(column, GLYPH_WIDTH), line_y);
                draw_indexed(frame, position, image, palette, Some(BACKGROUND_INDEX));
            }
        }
        cell_offset(lines, GLYPH_HEIGHT)
    }
}

fn cell_offset(cells: usize, cell_size: usize) -> i32 {
    i32::try_from(cells * cell_size).unwrap_or(i32::MAX)
}
