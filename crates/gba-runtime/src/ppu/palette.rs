//! A 16-entry palette, the unit the GBA uses for 4-bit tiles.

use platform::Rgb;

/// Sixteen colors addressed by a 4-bit palette index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    colors: [Rgb; 16],
}

impl Palette {
    /// Builds a palette from its sixteen colors.
    #[must_use]
    pub const fn new(colors: [Rgb; 16]) -> Self {
        Self { colors }
    }

    /// Converts a GBA 15-bit BGR555 color to 8-bit channels, as the hardware
    /// maps 5-bit channels to its 5-bit DAC: each channel is replicated into
    /// the low bits so that 31 becomes 255.
    #[must_use]
    pub const fn from_bgr555(color: u16) -> Rgb {
        let r = (color & 0x1F) as u8;
        let g = ((color >> 5) & 0x1F) as u8;
        let b = ((color >> 10) & 0x1F) as u8;
        Rgb::new(r << 3 | r >> 2, g << 3 | g >> 2, b << 3 | b >> 2)
    }

    /// Color at a 4-bit index.
    #[must_use]
    pub fn color(&self, index: u8) -> Rgb {
        self.colors[usize::from(index & 0x0F)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_bgr555_channels_to_full_range() {
        assert_eq!(Palette::from_bgr555(0x7FFF), Rgb::new(255, 255, 255));
        assert_eq!(Palette::from_bgr555(0x001F), Rgb::new(255, 0, 0));
        assert_eq!(Palette::from_bgr555(0x03E0), Rgb::new(0, 255, 0));
        assert_eq!(Palette::from_bgr555(0x7C00), Rgb::new(0, 0, 255));
        assert_eq!(Palette::from_bgr555(0x0010), Rgb::new(132, 0, 0));
    }

    #[test]
    fn masks_the_index_to_four_bits() {
        let mut colors = [Rgb::default(); 16];
        colors[3] = Rgb::new(1, 2, 3);
        let palette = Palette::new(colors);
        assert_eq!(palette.color(3), Rgb::new(1, 2, 3));
        assert_eq!(palette.color(0x13), Rgb::new(1, 2, 3));
    }
}
