//! 256-color backgrounds and whole-frame brightness (GBATEK, "LCD I/O BG
//! Control" bit 7 and "Color Special Effects" brightness decrease).

use platform::{Frame, Rgb};

use super::Palette;
use super::background::{TILE_PIXELS, TILE_SIZE, TileMapEntry};

const COLORS: usize = 256;
/// Brightness steps of a full fade, as `BLDY` counts them.
pub const FADE_STEPS: u8 = 16;

/// The 256 colors of a palette RAM half.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullPalette {
    colors: Vec<Rgb>,
}

impl FullPalette {
    /// Builds the palette from BGR555 colors; missing entries are black.
    #[must_use]
    pub fn from_bgr555(colors: &[u16]) -> Self {
        let mut all: Vec<Rgb> = colors
            .iter()
            .take(COLORS)
            .map(|color| Palette::from_bgr555(*color))
            .collect();
        all.resize(COLORS, Rgb::default());
        Self { colors: all }
    }

    /// Replaces colors from `start` with `colors`.
    pub fn write(&mut self, start: usize, colors: &[u16]) {
        for (slot, color) in self.colors.iter_mut().skip(start).zip(colors) {
            *slot = Palette::from_bgr555(*color);
        }
    }

    /// Color `index`.
    #[must_use]
    pub fn color(&self, index: u8) -> Rgb {
        self.colors[usize::from(index)]
    }
}

/// Fills the frame with a 256-color background scrolled by
/// `(scroll_x, scroll_y)` pixels; tiles hold one palette index per pixel.
/// With `transparent`, index 0 is left untouched.
pub fn draw_background_256<'t>(
    frame: &mut Frame,
    entry: impl Fn(usize, usize) -> u16,
    tile: impl Fn(usize) -> Option<&'t [u8; TILE_PIXELS]>,
    palette: &FullPalette,
    (scroll_x, scroll_y): (usize, usize),
    transparent: bool,
) {
    let (first_column, offset_x) = (scroll_x / TILE_SIZE, scroll_x % TILE_SIZE);
    let (first_row, offset_y) = (scroll_y / TILE_SIZE, scroll_y % TILE_SIZE);
    let columns = frame.width().div_ceil(TILE_SIZE) + 1;
    let rows = frame.height().div_ceil(TILE_SIZE) + 1;
    for row in 0..rows {
        for column in 0..columns {
            let cell = TileMapEntry::from_u16(entry(first_column + column, first_row + row));
            let Some(pixels) = tile(usize::from(cell.tile)) else {
                continue;
            };
            for y in 0..TILE_SIZE {
                let Some(frame_y) = (row * TILE_SIZE + y).checked_sub(offset_y) else {
                    continue;
                };
                for x in 0..TILE_SIZE {
                    let Some(frame_x) = (column * TILE_SIZE + x).checked_sub(offset_x) else {
                        continue;
                    };
                    let source_x = if cell.flip_x { TILE_SIZE - 1 - x } else { x };
                    let source_y = if cell.flip_y { TILE_SIZE - 1 - y } else { y };
                    let index = pixels[source_y * TILE_SIZE + source_x];
                    if transparent && index == 0 {
                        continue;
                    }
                    frame.set_pixel(frame_x, frame_y, palette.color(index));
                }
            }
        }
    }
}

/// Darkens the whole frame by `level` of [`FADE_STEPS`] toward black, as
/// the brightness-decrease effect does.
pub fn darken(frame: &mut Frame, level: u8) {
    let keep = u32::from(FADE_STEPS.saturating_sub(level.min(FADE_STEPS)));
    if keep == u32::from(FADE_STEPS) {
        return;
    }
    let scale =
        |value: u8| u8::try_from(u32::from(value) * keep / u32::from(FADE_STEPS)).unwrap_or(0);
    for y in 0..frame.height() {
        for x in 0..frame.width() {
            if let Some(color) = frame.pixel(x, y) {
                frame.set_pixel(
                    x,
                    y,
                    Rgb::new(scale(color.r), scale(color.g), scale(color.b)),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_256_color_tiles_and_darkens() {
        let mut tile = [0u8; TILE_PIXELS];
        tile[0] = 200;
        let palette =
            FullPalette::from_bgr555(&[0; 200].iter().copied().chain([0x7FFF]).collect::<Vec<_>>());
        let mut frame = Frame::new(8, 8, Rgb::new(3, 3, 3));
        draw_background_256(
            &mut frame,
            |_, _| 0,
            |_| Some(&tile),
            &palette,
            (0, 0),
            true,
        );
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(255, 255, 255)));
        assert_eq!(frame.pixel(1, 0), Some(Rgb::new(3, 3, 3)));
        darken(&mut frame, 8);
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(127, 127, 127)));
        darken(&mut frame, 16);
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(0, 0, 0)));
        let mut written = FullPalette::from_bgr555(&[]);
        written.write(64, &[0x001F]);
        assert_eq!(written.color(64), Rgb::new(255, 0, 0));
    }
}
