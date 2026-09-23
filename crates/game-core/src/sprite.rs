//! Drawing composed tile images such as portraits.

use formats::tile::TileImage;
use gba_runtime::ppu::{IndexedImage, Palette, draw_indexed};
use platform::Frame;

const TRANSPARENT_INDEX: u8 = 0;

/// Draws `image` with its top-left corner at `(x, y)` using a BGR555
/// palette, flipped left to right when `mirrored`; palette index 0 is
/// transparent, as for GBA sprites.
pub fn draw_sprite(
    frame: &mut Frame,
    x: i32,
    y: i32,
    image: &TileImage,
    bgr555: &[u16; 16],
    mirrored: bool,
) {
    let palette = Palette::new(bgr555.map(Palette::from_bgr555));
    let flipped;
    let indices = if mirrored {
        flipped = image
            .indices
            .chunks(image.width.max(1))
            .flat_map(|row| row.iter().rev().copied())
            .collect::<Vec<u8>>();
        &flipped
    } else {
        &image.indices
    };
    let indexed = IndexedImage {
        width: image.width,
        height: image.height,
        indices,
    };
    draw_indexed(frame, (x, y), indexed, &palette, Some(TRANSPARENT_INDEX));
}

#[cfg(test)]
mod tests {
    use platform::Rgb;

    use super::*;

    #[test]
    fn skips_index_zero_and_uses_the_palette() {
        let image = TileImage {
            width: 2,
            height: 1,
            indices: vec![0, 1],
        };
        let mut palette = [0u16; 16];
        palette[1] = 0x7FFF;
        let mut frame = Frame::new(2, 1, Rgb::new(9, 9, 9));
        draw_sprite(&mut frame, 0, 0, &image, &palette, false);
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(9, 9, 9)));
        assert_eq!(frame.pixel(1, 0), Some(Rgb::new(255, 255, 255)));
        let mut frame = Frame::new(2, 1, Rgb::new(9, 9, 9));
        draw_sprite(&mut frame, 0, 0, &image, &palette, true);
        assert_eq!(frame.pixel(0, 0), Some(Rgb::new(255, 255, 255)));
        assert_eq!(frame.pixel(1, 0), Some(Rgb::new(9, 9, 9)));
    }
}
