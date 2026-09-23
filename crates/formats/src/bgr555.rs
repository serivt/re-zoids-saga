//! GBA palette entries: 15-bit colors stored as little-endian `u16`, five bits
//! each of red (low), green and blue.

/// Bytes of one 16-color palette.
pub const PALETTE_LEN: usize = 32;

/// Parses a 16-entry palette; `None` when `data` is shorter than a palette.
#[must_use]
pub fn parse_palette(data: &[u8]) -> Option<[u16; 16]> {
    let data = data.get(..PALETTE_LEN)?;
    let mut colors = [0u16; 16];
    for (color, pair) in colors.iter_mut().zip(data.chunks_exact(2)) {
        *color = u16::from_le_bytes([pair[0], pair[1]]);
    }
    Some(colors)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn reads_little_endian_entries() {
        let mut data = vec![0u8; PALETTE_LEN];
        data[0] = 0xFF;
        data[1] = 0x7F;
        data[30] = 0x1F;
        let palette = parse_palette(&data).expect("palette");
        assert_eq!(palette[0], 0x7FFF);
        assert_eq!(palette[15], 0x001F);
        assert_eq!(parse_palette(&data[..31]), None);
    }
}
