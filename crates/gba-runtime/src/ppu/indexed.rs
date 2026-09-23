//! Blitting of palette-indexed pixels into a frame.

use platform::Frame;

use super::Palette;

/// A block of palette indices, row-major.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexedImage<'a> {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// One palette index per pixel, `width * height` of them.
    pub indices: &'a [u8],
}

/// Draws `image` with its top-left corner at `(x, y)`. Pixels whose index
/// equals `transparent` are skipped, and pixels outside the frame are clipped.
pub fn draw_indexed(
    frame: &mut Frame,
    (x, y): (i32, i32),
    image: IndexedImage<'_>,
    palette: &Palette,
    transparent: Option<u8>,
) {
    let rows = image.indices.chunks_exact(image.width).take(image.height);
    for (row, row_indices) in rows.enumerate() {
        let Some(frame_y) = offset(y, row) else {
            continue;
        };
        for (column, index) in row_indices.iter().enumerate() {
            if transparent == Some(*index) {
                continue;
            }
            if let Some(frame_x) = offset(x, column) {
                frame.set_pixel(frame_x, frame_y, palette.color(*index));
            }
        }
    }
}

fn offset(origin: i32, delta: usize) -> Option<usize> {
    let delta = i32::try_from(delta).ok()?;
    usize::try_from(origin.checked_add(delta)?).ok()
}

#[cfg(test)]
mod tests {
    use platform::Rgb;

    use super::*;

    fn palette() -> Palette {
        let mut colors = [Rgb::default(); 16];
        colors[1] = Rgb::new(1, 1, 1);
        colors[2] = Rgb::new(2, 2, 2);
        Palette::new(colors)
    }

    #[test]
    fn draws_clipped_and_skips_transparent_pixels() {
        let mut frame = Frame::new(3, 2, Rgb::new(9, 9, 9));
        let image = IndexedImage {
            width: 2,
            height: 2,
            indices: &[1, 2, 2, 1],
        };
        draw_indexed(&mut frame, (-1, 1), image, &palette(), Some(1));
        assert_eq!(frame.pixel(0, 1), Some(Rgb::new(2, 2, 2)));
        assert_eq!(frame.pixel(1, 1), Some(Rgb::new(9, 9, 9)));
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(9, 9, 9)));
    }

    #[test]
    fn draws_opaque_blocks_when_no_index_is_transparent() {
        let mut frame = Frame::new(2, 1, Rgb::new(9, 9, 9));
        let image = IndexedImage {
            width: 2,
            height: 1,
            indices: &[1, 2],
        };
        draw_indexed(&mut frame, (0, 0), image, &palette(), None);
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(1, 1, 1)));
        assert_eq!(frame.pixel(1, 0), Some(Rgb::new(2, 2, 2)));
    }
}
