//! A bitmap font written as text art: one `glyph <char>` header followed
//! by rows of `#` (ink) and `.` (empty), glyphs separated by blank lines.
//! Every glyph has the same number of rows; its width is its row length.

use std::collections::HashMap;

use thiserror::Error;

/// Rows every glyph has.
pub const PIXEL_FONT_ROWS: usize = 13;
const MAX_WIDTH: usize = 8;
const HEADER: &str = "glyph ";
const SPACE_NAME: &str = "space";

/// Why a font file could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PixelFontError {
    /// A line that is neither a header, a row, a comment nor blank.
    #[error("line {line}: expected a glyph header, found `{text}`")]
    BadHeader {
        /// Line number, from 1.
        line: usize,
        /// The line.
        text: String,
    },
    /// A row with characters other than `#` and `.`, or too wide.
    #[error("line {line}: bad row `{text}`")]
    BadRow {
        /// Line number, from 1.
        line: usize,
        /// The line.
        text: String,
    },
    /// A glyph whose rows differ in width or are not the fixed count.
    #[error("glyph `{glyph}` does not have {PIXEL_FONT_ROWS} rows of one width")]
    BadShape {
        /// The glyph's character.
        glyph: char,
    },
}

/// One glyph: its width and, per row, a bit per column (bit 0 leftmost).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelGlyph {
    /// Width in pixels, 1–8.
    pub width: u8,
    /// Rows, bit `x` set where column `x` has ink.
    pub rows: [u8; PIXEL_FONT_ROWS],
}

impl PixelGlyph {
    /// Whether the pixel at `(x, y)` has ink.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> bool {
        x < usize::from(self.width) && y < PIXEL_FONT_ROWS && self.rows[y] & (1 << x) != 0
    }

    /// The glyph turned upside down and mirrored.
    #[must_use]
    pub fn turned(&self) -> Self {
        let mut rows = self.rows;
        rows.reverse();
        for row in &mut rows {
            *row = row.reverse_bits() >> (8 - self.width.clamp(1, 8));
        }
        Self {
            width: self.width,
            rows,
        }
    }
}

/// The glyphs of a font.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PixelFont {
    glyphs: HashMap<char, PixelGlyph>,
}

impl PixelFont {
    /// Reads a font from its text-art source.
    ///
    /// # Errors
    ///
    /// Returns [`PixelFontError`] on a malformed line or glyph.
    pub fn parse(text: &str) -> Result<Self, PixelFontError> {
        let mut glyphs = HashMap::new();
        let mut current: Option<(char, Vec<u8>, usize)> = None;
        for (number, raw) in text.lines().enumerate() {
            let line = raw.trim_end();
            if line.is_empty() || line.starts_with('#') && current.is_none() {
                if let Some((ch, rows, width)) = current.take() {
                    glyphs.insert(ch, finish(ch, &rows, width)?);
                }
                continue;
            }
            if let Some(name) = line.strip_prefix(HEADER) {
                if let Some((ch, rows, width)) = current.take() {
                    glyphs.insert(ch, finish(ch, &rows, width)?);
                }
                let ch = if name == SPACE_NAME {
                    ' '
                } else {
                    let mut chars = name.chars();
                    match (chars.next(), chars.next()) {
                        (Some(ch), None) => ch,
                        _ => return Err(bad_header(number, raw)),
                    }
                };
                current = Some((ch, Vec::new(), 0));
                continue;
            }
            let Some((_, rows, width)) = current.as_mut() else {
                return Err(bad_header(number, raw));
            };
            if line.len() > MAX_WIDTH || line.chars().any(|c| c != '#' && c != '.') {
                return Err(PixelFontError::BadRow {
                    line: number + 1,
                    text: raw.to_owned(),
                });
            }
            let bits = line
                .chars()
                .enumerate()
                .filter(|(_, c)| *c == '#')
                .fold(0u8, |bits, (x, _)| bits | (1 << x));
            rows.push(bits);
            *width = (*width).max(line.len());
            if rows.len() > 1 && line.len() != *width {
                let ch = current.as_ref().map_or(' ', |(ch, ..)| *ch);
                return Err(PixelFontError::BadShape { glyph: ch });
            }
        }
        if let Some((ch, rows, width)) = current.take() {
            glyphs.insert(ch, finish(ch, &rows, width)?);
        }
        Ok(Self { glyphs })
    }

    /// The glyph of `ch`, if the font has it.
    #[must_use]
    pub fn glyph(&self, ch: char) -> Option<&PixelGlyph> {
        self.glyphs.get(&ch)
    }

    /// Number of glyphs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Whether the font has no glyphs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }
}

fn bad_header(number: usize, raw: &str) -> PixelFontError {
    PixelFontError::BadHeader {
        line: number + 1,
        text: raw.to_owned(),
    }
}

fn finish(ch: char, rows: &[u8], width: usize) -> Result<PixelGlyph, PixelFontError> {
    let shape: Result<[u8; PIXEL_FONT_ROWS], _> = rows.try_into();
    match (shape, u8::try_from(width)) {
        (Ok(rows), Ok(width)) if width >= 1 => Ok(PixelGlyph { width, rows }),
        _ => Err(PixelFontError::BadShape { glyph: ch }),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn source(rows: &[&str]) -> String {
        let mut text = String::from("# a comment\n\nglyph a\n");
        for row in rows {
            text.push_str(row);
            text.push('\n');
        }
        text
    }

    #[test]
    fn parses_glyphs_with_their_width_and_bits() {
        let mut rows = vec!["..."; PIXEL_FONT_ROWS];
        rows[2] = "#.#";
        rows[9] = ".#.";
        let mut text = source(&rows);
        text.push_str("\nglyph space\n");
        for _ in 0..PIXEL_FONT_ROWS {
            text.push_str("..\n");
        }
        let font = PixelFont::parse(&text).unwrap();
        assert_eq!(font.len(), 2);
        let a = font.glyph('a').unwrap();
        assert_eq!(a.width, 3);
        assert!(a.pixel(0, 2) && a.pixel(2, 2) && !a.pixel(1, 2));
        assert!(a.pixel(1, 9));
        assert_eq!(font.glyph(' ').unwrap().width, 2);
        let turned = a.turned();
        assert!(turned.pixel(1, 3) && turned.pixel(0, 10) && turned.pixel(2, 10));
    }

    #[test]
    fn rejects_bad_rows_headers_and_shapes() {
        assert!(matches!(
            PixelFont::parse("glyph ab\n"),
            Err(PixelFontError::BadHeader { line: 1, .. })
        ));
        assert!(matches!(
            PixelFont::parse("glyph a\n#x#\n"),
            Err(PixelFontError::BadRow { line: 2, .. })
        ));
        assert!(matches!(
            PixelFont::parse(&source(&["#.#"; 3])),
            Err(PixelFontError::BadShape { glyph: 'a' })
        ));
        assert!(matches!(
            PixelFont::parse("...\n"),
            Err(PixelFontError::BadHeader { line: 1, .. })
        ));
    }
}
