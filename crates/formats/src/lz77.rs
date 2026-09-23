//! The LZ77 compression the GBA BIOS decodes (`LZ77UnCompWram`/`LZ77UnCompVram`).
//!
//! A block starts with a 32-bit little-endian header whose low byte is `0x10`
//! and whose upper 24 bits give the decompressed size. Then come groups of a
//! flag byte followed by eight items, most significant flag bit first: a clear
//! bit copies one literal byte; a set bit is a two-byte back-reference whose
//! first byte holds `length - 3` in its high nibble and the high bits of
//! `distance - 1` in its low nibble, and whose second byte holds the low bits
//! of `distance - 1`. Documented in GBATEK, "BIOS Decompression Functions".

use thiserror::Error;

const HEADER_LEN: usize = 4;
const MAGIC: u8 = 0x10;
const MIN_MATCH: usize = 3;

/// Why a byte slice could not be decompressed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Lz77Error {
    /// The header byte is not the LZ77 marker.
    #[error("block starts with {actual:#04x}, expected {MAGIC:#04x}")]
    BadMagic {
        /// Byte that was found.
        actual: u8,
    },
    /// The slice ends before the block is complete.
    #[error("compressed data ends at byte {offset} before the block is complete")]
    Truncated {
        /// Offset of the first missing byte.
        offset: usize,
    },
    /// A back-reference points before the start of the output.
    #[error("back-reference at byte {offset} reaches before the output start")]
    BadReference {
        /// Offset of the back-reference.
        offset: usize,
    },
}

/// Decompresses the block at the start of `data`, returning the output and the
/// number of compressed bytes consumed.
///
/// # Errors
///
/// Returns [`Lz77Error`] when the header is wrong, the data is truncated or a
/// back-reference is invalid.
pub fn decompress(data: &[u8]) -> Result<(Vec<u8>, usize), Lz77Error> {
    let header = data
        .get(..HEADER_LEN)
        .ok_or(Lz77Error::Truncated { offset: data.len() })?;
    if header[0] != MAGIC {
        return Err(Lz77Error::BadMagic { actual: header[0] });
    }
    let size = usize::from(header[1]) | usize::from(header[2]) << 8 | usize::from(header[3]) << 16;
    let mut out = Vec::with_capacity(size);
    let mut pos = HEADER_LEN;
    while out.len() < size {
        let flags = byte_at(data, pos)?;
        pos += 1;
        for bit in (0..8).rev() {
            if out.len() >= size {
                break;
            }
            if flags & (1 << bit) == 0 {
                out.push(byte_at(data, pos)?);
                pos += 1;
            } else {
                pos = copy_reference(data, pos, &mut out)?;
            }
        }
    }
    Ok((out, pos))
}

fn copy_reference(data: &[u8], pos: usize, out: &mut Vec<u8>) -> Result<usize, Lz77Error> {
    let first = byte_at(data, pos)?;
    let second = byte_at(data, pos + 1)?;
    let length = usize::from(first >> 4) + MIN_MATCH;
    let distance = (usize::from(first & 0x0F) << 8 | usize::from(second)) + 1;
    let start = out
        .len()
        .checked_sub(distance)
        .ok_or(Lz77Error::BadReference { offset: pos })?;
    for i in 0..length {
        out.push(out[start + i]);
    }
    Ok(pos + 2)
}

fn byte_at(data: &[u8], offset: usize) -> Result<u8, Lz77Error> {
    data.get(offset)
        .copied()
        .ok_or(Lz77Error::Truncated { offset })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn header(size: usize) -> [u8; 4] {
        let [low, mid, high, _] = u32::try_from(size).unwrap().to_le_bytes();
        [MAGIC, low, mid, high]
    }

    #[test]
    fn copies_literals() {
        let mut data = header(3).to_vec();
        data.extend([0x00, b'a', b'b', b'c']);
        assert_eq!(decompress(&data).unwrap(), (b"abc".to_vec(), 8));
    }

    #[test]
    fn expands_overlapping_back_references() {
        let mut data = header(8).to_vec();
        data.extend([0b0100_0000, b'x', 0x20, 0x00, b'y', b'z']);
        let (out, consumed) = decompress(&data).unwrap();
        assert_eq!(out, b"xxxxxxyz");
        assert_eq!(consumed, data.len());
    }

    #[test]
    fn stops_at_the_declared_size_across_groups() {
        let mut data = header(9).to_vec();
        data.extend([0x00, 1, 2, 3, 4, 5, 6, 7, 8, 0x00, 9, 0xFF]);
        let (out, consumed) = decompress(&data).unwrap();
        assert_eq!(out, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(consumed, data.len() - 1);
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(
            decompress(&[0x11, 0, 0, 0]),
            Err(Lz77Error::BadMagic { actual: 0x11 })
        );
        let mut truncated = header(4).to_vec();
        truncated.extend([0x00, 1]);
        assert_eq!(
            decompress(&truncated),
            Err(Lz77Error::Truncated { offset: 6 })
        );
        let mut bad_reference = header(4).to_vec();
        bad_reference.extend([0b1000_0000, 0x00, 0x05]);
        assert_eq!(
            decompress(&bad_reference),
            Err(Lz77Error::BadReference { offset: 5 })
        );
    }
}
