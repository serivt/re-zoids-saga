//! The lines a handheld's screen showed between its pixels, drawn over the
//! scaled picture: an LCD's grid, at the right and the bottom of each
//! pixel, marked, soft or fine, and a television's scan lines, at the
//! bottom of each row. The lines darken what they cover (they multiply it
//! by a shade) and are whole output pixels thick; they show only once a
//! pixel of the game spans enough output pixels. The backend draws them;
//! this module says where.
//!
//! Source of knowledge: this project's own design (see
//! `docs/launcher.md`, LCD grid).

/// A rectangle in the backend's units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Lines over the scaled picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    /// How dark they leave what they cover: the gray they multiply it by.
    pub shade: u8,
    /// The share of a pixel's output pixels a line takes, rounded to whole
    /// ones, at least one.
    pub share: f32,
    /// Whether there are lines between the columns too.
    pub columns: bool,
    /// The output pixels a pixel of the game must span for them to show.
    pub least: f32,
}

impl Grid {
    /// The original Game Boy Advance's marked grid.
    pub const LCD: Self = Self {
        shade: 0xB0,
        share: 0.2,
        columns: true,
        least: 3.0,
    };
    /// A softer grid, as the Game Boy Advance SP's.
    pub const LCD_SOFT: Self = Self {
        shade: 0xCC,
        share: 0.2,
        columns: true,
        least: 3.0,
    };
    /// A fine grid, as the Game Boy Micro's.
    pub const LCD_FINE: Self = Self {
        shade: 0xD8,
        share: 0.12,
        columns: true,
        least: 3.0,
    };
    /// A television's scan lines.
    pub const SCANLINES: Self = Self {
        shade: 0xA8,
        share: 0.4,
        columns: false,
        least: 2.0,
    };

    /// The grid the settings name `key` (`lcd`, `lcd-soft`, `lcd-fine`,
    /// `scanlines`, as the launcher's filters), if any.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "lcd" => Some(Self::LCD),
            "lcd-soft" => Some(Self::LCD_SOFT),
            "lcd-fine" => Some(Self::LCD_FINE),
            "scanlines" => Some(Self::SCANLINES),
            _ => None,
        }
    }

    /// The lines over a picture of `frame` pixels drawn at `area`, the
    /// backend's units being `scale` output pixels each: one at the bottom
    /// of each row and, with columns, at the right of each column; none
    /// when a pixel spans fewer output pixels than the grid needs.
    #[must_use]
    pub fn lines(self, area: Rect, frame: (usize, usize), scale: f32) -> Vec<Rect> {
        let (columns, rows) = (to_f32(frame.0), to_f32(frame.1));
        let cell = (area.width / columns, area.height / rows);
        let pixels = cell.0 * scale;
        if pixels < self.least {
            return Vec::new();
        }
        let thickness = (pixels * self.share).round().max(1.0) / scale;
        let column_count = if self.columns { frame.0 } else { 0 };
        let vertical = (1..=column_count).map(|column| Rect {
            x: area.x + to_f32(column) * cell.0 - thickness,
            y: area.y,
            width: thickness,
            height: area.height,
        });
        let horizontal = (1..=frame.1).map(|row| Rect {
            x: area.x,
            y: area.y + to_f32(row) * cell.1 - thickness,
            width: area.width,
            height: thickness,
        });
        vertical.chain(horizontal).collect()
    }
}

#[allow(clippy::cast_precision_loss)]
fn to_f32(value: usize) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: (usize, usize) = (240, 160);
    const LOGICAL: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 240.0,
        height: 160.0,
    };

    fn near(value: f32, expected: f32) -> bool {
        (value - expected).abs() < 0.001
    }

    #[test]
    fn the_lcd_grid_lines_each_pixel_right_and_below() {
        let lines = Grid::LCD.lines(LOGICAL, FRAME, 5.0);
        assert_eq!(lines.len(), 240 + 160);
        assert!(
            near(lines[0].x, 0.8) && near(lines[0].width, 0.2),
            "one output pixel of five"
        );
        assert!(near(lines[239].x, 239.8) && near(lines[239].height, 160.0));
        assert!(near(lines[240].y, 0.8) && near(lines[240].width, 240.0));
        let wide_area = Rect {
            x: 10.0,
            y: 0.0,
            width: 2400.0,
            height: 1600.0,
        };
        let wide = Grid::LCD.lines(wide_area, FRAME, 1.0);
        assert!(
            near(wide[0].x, 18.0) && near(wide[0].width, 2.0),
            "two pixels of ten"
        );
        assert!(
            Grid::LCD.lines(LOGICAL, FRAME, 2.0).is_empty(),
            "too small to show"
        );
    }

    #[test]
    fn scan_lines_darken_the_bottom_of_each_row_only() {
        let lines = Grid::SCANLINES.lines(LOGICAL, FRAME, 5.0);
        assert_eq!(lines.len(), 160);
        assert!(near(lines[0].y, 0.6) && near(lines[0].height, 0.4));
        assert!(near(lines[0].width, 240.0));
    }

    #[test]
    fn the_settings_name_the_grids() {
        assert_eq!(Grid::from_key("lcd-fine"), Some(Grid::LCD_FINE));
        assert_eq!(Grid::from_key("scanlines"), Some(Grid::SCANLINES));
        assert_eq!(Grid::from_key("sharp"), None);
    }
}
