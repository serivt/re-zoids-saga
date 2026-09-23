//! Drawing game text with the original 8×16 font.

use formats::font::{GLYPH_HEIGHT, GLYPH_WIDTH, Glyph, GlyphIndex, shift_jis_code};
use gba_runtime::ppu::{IndexedImage, Palette, draw_indexed};
use platform::Frame;

const BACKGROUND_INDEX: u8 = 1;
const FULL_WIDTH_OFFSET: u32 = 0xFEE0;
const IDEOGRAPHIC_SPACE: char = '\u{3000}';

/// The full-width form of a printable ASCII character, or the character
/// itself.
fn full_width(ch: char) -> char {
    match ch {
        ' ' => IDEOGRAPHIC_SPACE,
        '!'..='~' => char::from_u32(u32::from(ch) + FULL_WIDTH_OFFSET).unwrap_or(ch),
        other => other,
    }
}

/// A mark drawn above a letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accent {
    Acute,
    Grave,
    Circumflex,
    Diaeresis,
    Tilde,
}

fn accent(ch: char) -> Option<Accent> {
    Some(match ch {
        'á' | 'é' | 'í' | 'ó' | 'ú' | 'Á' | 'É' | 'Í' | 'Ó' | 'Ú' => Accent::Acute,
        'à' | 'è' | 'ì' | 'ò' | 'ù' | 'À' | 'È' | 'Ì' | 'Ò' | 'Ù' => Accent::Grave,
        'â' | 'ê' | 'î' | 'ô' | 'û' | 'Â' | 'Ê' | 'Î' | 'Ô' | 'Û' => Accent::Circumflex,
        'ä' | 'ë' | 'ï' | 'ö' | 'ü' | 'Ä' | 'Ë' | 'Ï' | 'Ö' | 'Ü' => Accent::Diaeresis,
        'ñ' | 'Ñ' | 'ã' | 'õ' | 'Ã' | 'Õ' => Accent::Tilde,
        _ => return None,
    })
}

/// The glyph turned upside down (and mirrored), as ¿ is to ?.
fn turned(glyph: &Glyph) -> Glyph {
    let mut pixels = glyph.pixels;
    pixels.reverse();
    Glyph { pixels }
}

/// The glyph with `mark` drawn in the two rows above its topmost pixel,
/// or over its top rows when nothing is above them.
fn accented(base: &Glyph, mark: Accent) -> Glyph {
    let mut glyph = base.clone();
    let is_ink = |index: u8| index != BACKGROUND_INDEX;
    let top = (0..GLYPH_HEIGHT)
        .find(|row| (0..GLYPH_WIDTH).any(|x| base.pixel(x, *row).is_some_and(is_ink)))
        .unwrap_or(GLYPH_HEIGHT);
    let ink = base
        .pixels
        .iter()
        .copied()
        .find(|p| is_ink(*p))
        .unwrap_or(0);
    let row = top.saturating_sub(3);
    let dots: &[(usize, usize)] = match mark {
        Accent::Acute => &[(4, 0), (3, 1)],
        Accent::Grave => &[(3, 0), (4, 1)],
        Accent::Circumflex => &[(3, 0), (2, 1), (4, 1)],
        Accent::Diaeresis => &[(2, 0), (5, 0), (2, 1), (5, 1)],
        Accent::Tilde => &[(2, 1), (3, 0), (4, 0), (5, 1)],
    };
    for (x, dy) in dots {
        glyph.pixels[(row + dy) * GLYPH_WIDTH + x] = ink;
    }
    glyph
}

/// A Latin letter without its accent, or the character itself.
fn plain_latin(ch: char) -> char {
    match ch {
        'á' | 'à' | 'â' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'Á' | 'À' | 'Â' | 'Ä' => 'A',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'Ó' | 'Ò' | 'Ô' | 'Ö' => 'O',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'Ñ' => 'N',
        'Ç' => 'C',
        '¿' => '?',
        '¡' => '!',
        other => other,
    }
}

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
    /// The font has no half-width Latin letters, so ASCII draws with the
    /// full-width forms, and accented Latin letters with their plain ones.
    #[must_use]
    pub fn glyph(&self, ch: char) -> Option<Glyph> {
        [ch, full_width(ch)]
            .into_iter()
            .filter_map(shift_jis_code)
            .find_map(|code| self.index.glyph(self.rom, code))
            .or_else(|| self.synthesized(ch))
            .or_else(|| self.font_glyph(full_width(plain_latin(ch))))
            .or_else(|| self.fallback.clone())
    }

    fn font_glyph(&self, ch: char) -> Option<Glyph> {
        shift_jis_code(ch).and_then(|code| self.index.glyph(self.rom, code))
    }

    /// Latin characters the font lacks, built from the glyphs it has: the
    /// inverted marks are the upright ones turned around, accented letters
    /// carry their mark above the plain letter.
    fn synthesized(&self, ch: char) -> Option<Glyph> {
        match ch {
            '¿' => self.font_glyph('？').map(|glyph| turned(&glyph)),
            '¡' => self.font_glyph('！').map(|glyph| turned(&glyph)),
            other => {
                let mark = accent(other)?;
                let base = self.font_glyph(full_width(plain_latin(other)))?;
                Some(accented(&base, mark))
            }
        }
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
