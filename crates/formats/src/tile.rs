//! GBA 4-bit-per-pixel tiles: 8×8 pixels in 32 bytes, two pixels per byte with
//! the left pixel in the low nibble, rows top to bottom.

/// Side of a tile in pixels.
pub const TILE_SIZE: usize = 8;
/// Bytes of one 4bpp tile.
pub const TILE_LEN: usize = 32;
/// Pixels of one tile.
pub const TILE_PIXELS: usize = TILE_SIZE * TILE_SIZE;

/// Decodes a 4bpp tile into one palette index per pixel, row-major.
#[must_use]
pub fn decode_4bpp(tile: &[u8; TILE_LEN]) -> [u8; TILE_PIXELS] {
    let mut pixels = [0u8; TILE_PIXELS];
    for (index, byte) in tile.iter().enumerate() {
        pixels[2 * index] = byte & 0x0F;
        pixels[2 * index + 1] = byte >> 4;
    }
    pixels
}

/// A set of decoded 4bpp tiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tileset {
    tiles: Vec<[u8; TILE_PIXELS]>,
}

impl Tileset {
    /// Decodes every whole tile in `data`; trailing bytes are ignored.
    #[must_use]
    pub fn from_4bpp(data: &[u8]) -> Self {
        let tiles = data
            .chunks_exact(TILE_LEN)
            .filter_map(|chunk| chunk.try_into().ok())
            .map(|chunk: &[u8; TILE_LEN]| decode_4bpp(chunk))
            .collect();
        Self { tiles }
    }

    /// Number of tiles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Whether the set has no tiles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Pixels of tile `index`, if it exists.
    #[must_use]
    pub fn tile(&self, index: usize) -> Option<&[u8; TILE_PIXELS]> {
        self.tiles.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_low_nibble_as_the_left_pixel() {
        let mut tile = [0u8; TILE_LEN];
        tile[0] = 0xA1;
        tile[31] = 0x3F;
        let pixels = decode_4bpp(&tile);
        assert_eq!(&pixels[..2], &[1, 0xA]);
        assert_eq!(&pixels[62..], &[0xF, 3]);
    }

    #[test]
    fn splits_data_into_whole_tiles() {
        let mut data = vec![0x22u8; TILE_LEN * 2 + 5];
        data[TILE_LEN] = 0x01;
        let set = Tileset::from_4bpp(&data);
        assert_eq!(set.len(), 2);
        assert_eq!(set.tile(1).map(|t| t[0]), Some(1));
        assert_eq!(set.tile(2), None);
        assert!(Tileset::from_4bpp(&[]).is_empty());
    }
}
