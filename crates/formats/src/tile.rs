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

/// A rectangle of tiles inside a composed image, in tile units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TilePiece {
    /// Column of the piece's top-left tile.
    pub column: usize,
    /// Row of the piece's top-left tile.
    pub row: usize,
    /// Width in tiles.
    pub columns: usize,
    /// Height in tiles.
    pub rows: usize,
}

/// An image assembled from tiles: one palette index per pixel, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileImage {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// `width * height` palette indices.
    pub indices: Vec<u8>,
}

impl TileImage {
    /// Composes an image from consecutive tiles laid out piece by piece, each
    /// piece filled row-major (the GBA's one-dimensional sprite mapping).
    /// Missing tiles are left at index 0.
    #[must_use]
    pub fn compose(tiles: &Tileset, pieces: &[TilePiece]) -> Self {
        let width = pieces
            .iter()
            .map(|p| p.column + p.columns)
            .max()
            .unwrap_or(0)
            * TILE_SIZE;
        let height = pieces.iter().map(|p| p.row + p.rows).max().unwrap_or(0) * TILE_SIZE;
        let mut indices = vec![0u8; width * height];
        let mut next = 0;
        for piece in pieces {
            for row in 0..piece.rows {
                for column in 0..piece.columns {
                    if let Some(tile) = tiles.tile(next) {
                        let origin = ((piece.row + row) * TILE_SIZE * width)
                            + (piece.column + column) * TILE_SIZE;
                        for (y, line) in tile.chunks_exact(TILE_SIZE).enumerate() {
                            let start = origin + y * width;
                            indices[start..start + TILE_SIZE].copy_from_slice(line);
                        }
                    }
                    next += 1;
                }
            }
        }
        Self {
            width,
            height,
            indices,
        }
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
    fn composes_pieces_in_one_dimensional_order() {
        let mut data = Vec::new();
        for i in 1..=6u8 {
            data.extend([i | i << 4; TILE_LEN]);
        }
        let tiles = Tileset::from_4bpp(&data);
        let pieces = [
            TilePiece {
                column: 0,
                row: 0,
                columns: 2,
                rows: 2,
            },
            TilePiece {
                column: 2,
                row: 0,
                columns: 1,
                rows: 2,
            },
        ];
        let image = TileImage::compose(&tiles, &pieces);
        assert_eq!((image.width, image.height), (24, 16));
        let at = |x: usize, y: usize| image.indices[y * image.width + x];
        assert_eq!(at(0, 0), 1);
        assert_eq!(at(8, 0), 2);
        assert_eq!(at(0, 8), 3);
        assert_eq!(at(8, 8), 4);
        assert_eq!(at(16, 0), 5);
        assert_eq!(at(16, 8), 6);
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
