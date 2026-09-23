//! GBA text-mode tilemaps as stored: little-endian 16-bit entries, row-major.
//! Decoding an entry's fields is the PPU's business.

/// A tilemap of `width`×`height` raw entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileMap {
    /// Width in cells.
    pub width: usize,
    /// Height in cells.
    pub height: usize,
    /// `width * height` entries, row-major.
    pub entries: Vec<u16>,
}

impl TileMap {
    /// Decodes little-endian entries; `None` when `data` is shorter than
    /// `width * height` entries.
    #[must_use]
    pub fn from_le_bytes(width: usize, height: usize, data: &[u8]) -> Option<Self> {
        let data = data.get(..width.checked_mul(height)?.checked_mul(2)?)?;
        let entries = data
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        Some(Self {
            width,
            height,
            entries,
        })
    }

    /// Entry at `(x, y)`, wrapping around both edges as the hardware does.
    #[must_use]
    pub fn wrapping(&self, x: usize, y: usize) -> u16 {
        self.entries[(y % self.height) * self.width + (x % self.width)]
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn builds_maps_and_wraps() {
        let data = [1, 0, 2, 0, 3, 0, 4, 0];
        let map = TileMap::from_le_bytes(2, 2, &data).unwrap();
        assert_eq!(map.wrapping(0, 0), 1);
        assert_eq!(map.wrapping(3, 1), 4);
        assert_eq!(map.wrapping(2, 3), 3);
        assert_eq!(TileMap::from_le_bytes(2, 3, &data), None);
    }
}
