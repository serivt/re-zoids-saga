//! Drawing game text: the ROM's 8×16 font for its own characters and
//! this project's Latin pixel font, proportional and narrower, for the
//! letters a translation needs.

use formats::font::{GLYPH_HEIGHT, GLYPH_WIDTH, Glyph, GlyphIndex, shift_jis_code};
use formats::pixel_font::{PIXEL_FONT_ROWS, PixelFont, PixelGlyph};
use gba_runtime::ppu::{IndexedImage, Palette, draw_indexed};
use platform::Frame;

/// The project's Latin font, an original asset.
pub const LATIN_FONT_SOURCE: &str = include_str!("../../../assets/fonts/latin/re-zoids-latin.txt");
const BACKGROUND_INDEX: u8 = 1;
const INK_INDEX: u8 = 15;
const LATIN_TOP: usize = 4;
const LATIN_SPACING: usize = 1;
const LATIN_INSET: usize = 3;
/// Pixels one cell of the ROM font takes.
pub const CELL_WIDTH: usize = GLYPH_WIDTH;
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
/// or over its top rows when nothing is above them; a dotted letter loses
/// its dot to the mark.
fn accented(base: &Glyph, mark: Accent, dotted: bool) -> Glyph {
    let mut glyph = base.clone();
    let is_ink = |index: u8| index != BACKGROUND_INDEX;
    let ink_row = |row: usize| (0..GLYPH_WIDTH).any(|x| base.pixel(x, row).is_some_and(is_ink));
    let mut top = (0..GLYPH_HEIGHT)
        .find(|row| ink_row(*row))
        .unwrap_or(GLYPH_HEIGHT);
    let ink = base
        .pixels
        .iter()
        .copied()
        .find(|p| is_ink(*p))
        .unwrap_or(0);
    if dotted {
        let gap = (top..GLYPH_HEIGHT)
            .find(|row| !ink_row(*row))
            .unwrap_or(top);
        for row in top..gap {
            for x in 0..GLYPH_WIDTH {
                glyph.pixels[row * GLYPH_WIDTH + x] = BACKGROUND_INDEX;
            }
        }
        top = (gap..GLYPH_HEIGHT).find(|row| ink_row(*row)).unwrap_or(top);
    }
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

/// How wide characters are, for laying text out without drawing it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextMetrics {
    latin: PixelFont,
}

impl TextMetrics {
    /// Metrics over the project's Latin font.
    #[must_use]
    pub fn standard() -> Self {
        Self {
            latin: PixelFont::parse(LATIN_FONT_SOURCE).unwrap_or_default(),
        }
    }

    /// The Latin glyph of `ch`, drawn or built from the font.
    fn latin_glyph(&self, ch: char) -> Option<PixelGlyph> {
        if let Some(glyph) = self.latin.glyph(ch) {
            return Some(*glyph);
        }
        match ch {
            '¿' => self.latin.glyph('?').map(PixelGlyph::turned),
            '¡' => self.latin.glyph('!').map(PixelGlyph::turned),
            'ç' | 'Ç' => self.latin.glyph(plain_latin(ch)).map(|base| cedilla(*base)),
            other => {
                let mark = accent(other)?;
                let base = self.latin.glyph(plain_latin(other))?;
                Some(latin_accented(
                    *base,
                    mark,
                    matches!(plain_latin(other), 'i' | 'j'),
                ))
            }
        }
    }

    /// Whether `ch` draws with the Latin font.
    #[must_use]
    pub fn is_latin(&self, ch: char) -> bool {
        self.latin_glyph(ch).is_some()
    }

    /// Pixels the pen moves after `ch`: its Latin width plus a space, or a
    /// full cell for the ROM font.
    #[must_use]
    pub fn advance(&self, ch: char) -> usize {
        self.latin_glyph(ch)
            .map_or(CELL_WIDTH, |glyph| usize::from(glyph.width) + LATIN_SPACING)
    }

    /// Pixels a line starting with `first` is set in from the window's
    /// edge: Latin letters get a little room, the ROM's glyphs carry their
    /// own.
    #[must_use]
    pub fn inset(&self, first: char) -> usize {
        if self.is_latin(first) { LATIN_INSET } else { 0 }
    }

    /// Pixels `text` takes on one line, its inset included.
    #[must_use]
    pub fn width(&self, text: &str) -> usize {
        let inset = text.chars().next().map_or(0, |first| self.inset(first));
        inset + text.chars().map(|ch| self.advance(ch)).sum::<usize>()
    }
}

/// Draws strings with a font read from a ROM image.
pub struct TextPainter<'rom> {
    rom: &'rom [u8],
    index: GlyphIndex,
    fallback: Option<Glyph>,
    metrics: TextMetrics,
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
            metrics: TextMetrics::standard(),
        }
    }

    /// The painter's character widths.
    #[must_use]
    pub fn metrics(&self) -> &TextMetrics {
        &self.metrics
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
                let plain = plain_latin(other);
                let base = self.font_glyph(full_width(plain))?;
                Some(accented(&base, mark, matches!(plain, 'i' | 'j')))
            }
        }
    }

    /// Draws `text` with its top-left corner at `(x, y)`, one glyph row per
    /// line, Latin letters at their own widths; the background index is
    /// transparent. Returns the height drawn in pixels.
    pub fn draw(&self, frame: &mut Frame, x: i32, y: i32, text: &str, palette: &Palette) -> i32 {
        self.draw_lines(frame, x, y, text, palette, false)
    }

    /// Draws `text` like [`Self::draw`] but one cell per character, Latin
    /// letters centered in theirs, for grids that align with sprites.
    pub fn draw_cells(
        &self,
        frame: &mut Frame,
        x: i32,
        y: i32,
        text: &str,
        palette: &Palette,
    ) -> i32 {
        self.draw_lines(frame, x, y, text, palette, true)
    }

    fn draw_lines(
        &self,
        frame: &mut Frame,
        x: i32,
        y: i32,
        text: &str,
        palette: &Palette,
        cells: bool,
    ) -> i32 {
        let mut lines = 0;
        for (line, content) in text.lines().enumerate() {
            lines = line + 1;
            let line_y = y + cell_offset(line, GLYPH_HEIGHT);
            let mut pen = if cells {
                0
            } else {
                content
                    .chars()
                    .next()
                    .map_or(0, |first| self.metrics.inset(first))
            };
            for ch in content.chars() {
                if let Some(glyph) = self.metrics.latin_glyph(ch) {
                    let inset = if cells {
                        (CELL_WIDTH - usize::from(glyph.width).min(CELL_WIDTH)) / 2
                    } else {
                        0
                    };
                    draw_latin(
                        frame,
                        x + cell_offset(pen + inset, 1),
                        line_y,
                        &glyph,
                        palette,
                    );
                    pen += if cells {
                        CELL_WIDTH
                    } else {
                        usize::from(glyph.width) + LATIN_SPACING
                    };
                    continue;
                }
                if let Some(glyph) = self.glyph(ch) {
                    let image = IndexedImage {
                        width: GLYPH_WIDTH,
                        height: GLYPH_HEIGHT,
                        indices: &glyph.pixels,
                    };
                    let position = (x + cell_offset(pen, 1), line_y);
                    draw_indexed(frame, position, image, palette, Some(BACKGROUND_INDEX));
                }
                pen += CELL_WIDTH;
            }
        }
        cell_offset(lines, GLYPH_HEIGHT)
    }
}

/// Draws a Latin glyph with its box top at `LATIN_TOP` rows below `y`.
fn draw_latin(frame: &mut Frame, x: i32, y: i32, glyph: &PixelGlyph, palette: &Palette) {
    let color = palette.color(INK_INDEX);
    for row in 0..PIXEL_FONT_ROWS {
        for column in 0..usize::from(glyph.width) {
            if glyph.pixel(column, row) {
                let px = x + cell_offset(column, 1);
                let py = y + cell_offset(LATIN_TOP + row, 1);
                if let (Ok(px), Ok(py)) = (usize::try_from(px), usize::try_from(py)) {
                    frame.set_pixel(px, py, color);
                }
            }
        }
    }
}

/// A Latin glyph with `mark` in the two rows above its topmost ink (the
/// two rows at the top when nothing is above it); a dotted letter loses
/// its dot to the mark.
fn latin_accented(base: PixelGlyph, mark: Accent, dotted: bool) -> PixelGlyph {
    let mut glyph = base;
    let mut top = glyph
        .rows
        .iter()
        .position(|row| *row != 0)
        .unwrap_or(PIXEL_FONT_ROWS);
    if dotted {
        let gap = (top..PIXEL_FONT_ROWS)
            .find(|row| glyph.rows[*row] == 0)
            .unwrap_or(top);
        for row in top..gap {
            glyph.rows[row] = 0;
        }
        top = (gap..PIXEL_FONT_ROWS)
            .find(|row| glyph.rows[*row] != 0)
            .unwrap_or(top);
    }
    let row = top.saturating_sub(3);
    let dots: &[(usize, usize)] = match mark {
        Accent::Acute => &[(3, 0), (2, 1)],
        Accent::Grave => &[(1, 0), (2, 1)],
        Accent::Circumflex => &[(2, 0), (1, 1), (3, 1)],
        Accent::Diaeresis => &[(1, 1), (3, 1)],
        Accent::Tilde => &[(1, 0), (3, 0), (0, 1), (2, 1)],
    };
    let width = usize::from(glyph.width);
    for (x, dy) in dots {
        let x = (*x).min(width.saturating_sub(1));
        glyph.rows[row + dy] |= 1 << x;
    }
    glyph
}

/// A Latin glyph with a cedilla hanging from its bottom.
fn cedilla(base: PixelGlyph) -> PixelGlyph {
    let mut glyph = base;
    let bottom = glyph.rows.iter().rposition(|row| *row != 0).unwrap_or(0);
    let width = usize::from(glyph.width);
    let x = width / 2;
    if bottom + 2 < PIXEL_FONT_ROWS {
        glyph.rows[bottom + 1] |= 1 << x;
        glyph.rows[bottom + 2] |= 1 << x.saturating_sub(1) | 1 << x;
    }
    glyph
}

fn cell_offset(cells: usize, cell_size: usize) -> i32 {
    i32::try_from(cells * cell_size).unwrap_or(i32::MAX)
}
