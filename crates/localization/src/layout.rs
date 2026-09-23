//! Dynamic text layout: breaking a message into the lines a text box shows.

/// Width and height of a text area, in glyph cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextArea {
    /// Cells per line.
    pub columns: usize,
    /// Visible lines.
    pub rows: usize,
}

/// A message laid out for a text area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// Lines in reading order, none wider than the area.
    pub lines: Vec<String>,
    /// Lines the area can show.
    pub rows: usize,
}

impl Layout {
    /// Whether the message needs more lines than the area shows.
    #[must_use]
    pub fn overflows(&self) -> bool {
        self.lines.len() > self.rows
    }

    /// The lines that fit in the area.
    #[must_use]
    pub fn visible_lines(&self) -> &[String] {
        &self.lines[..self.lines.len().min(self.rows)]
    }
}

impl TextArea {
    /// Lays out `text`: explicit line breaks are kept and every line is
    /// wrapped at `columns` cells, breaking between any two characters
    /// (the original game wraps this way, without kinsoku rules). Cell
    /// widths come from `cells`, which returns how many cells a character
    /// occupies.
    #[must_use]
    pub fn layout(&self, text: &str, cells: impl Fn(char) -> usize) -> Layout {
        let lines = text
            .split('\n')
            .flat_map(|line| self.wrap_line(line, &cells))
            .collect();
        Layout {
            lines,
            rows: self.rows,
        }
    }

    fn wrap_line(&self, line: &str, cells: &impl Fn(char) -> usize) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current = String::new();
        let mut used = 0;
        for ch in line.chars() {
            let width = cells(ch);
            if used + width > self.columns && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                used = 0;
            }
            current.push(ch);
            used += width;
        }
        lines.push(current);
        lines
    }
}

/// Cell width of a character in a font where every glyph is one cell.
#[must_use]
pub const fn monospace(_: char) -> usize {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: TextArea = TextArea {
        columns: 4,
        rows: 2,
    };

    #[test]
    fn keeps_explicit_breaks_and_wraps_long_lines() {
        let layout = AREA.layout("ab\ncdefghi", monospace);
        assert_eq!(layout.lines, ["ab", "cdef", "ghi"]);
        assert!(layout.overflows());
        assert_eq!(layout.visible_lines(), ["ab", "cdef"]);
    }

    #[test]
    fn a_line_exactly_as_wide_as_the_area_does_not_wrap() {
        let layout = AREA.layout("abcd", monospace);
        assert_eq!(layout.lines, ["abcd"]);
        assert!(!layout.overflows());
    }

    #[test]
    fn empty_text_is_one_empty_line() {
        let layout = AREA.layout("", monospace);
        assert_eq!(layout.lines, [""]);
    }

    #[test]
    fn honors_wide_characters() {
        let wide = |ch: char| if ch == 'W' { 2 } else { 1 };
        let layout = AREA.layout("aWWb", wide);
        assert_eq!(layout.lines, ["aW", "Wb"]);
    }
}
