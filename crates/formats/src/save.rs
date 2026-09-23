//! The cartridge save memory as the game's save routine lays it out: a
//! header string, then the same run of blocks repeated once per copy, each
//! block followed by a 32-bit little-endian sum of its bytes. The layout
//! (header, block sizes, number of copies) comes from the ROM; this module
//! only encodes and checks it. See `docs/formats/save.md`.

use thiserror::Error;

/// Bytes of the sum that follows every block.
pub const CHECKSUM_LEN: usize = 4;
/// What an erased save memory reads as.
pub const ERASED: u8 = 0xFF;

/// Where the save routine puts everything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveLayout {
    /// The string written at the start of the memory once it holds data.
    pub magic: Vec<u8>,
    /// Size of each block of a copy, in order.
    pub blocks: Vec<usize>,
    /// How many copies of the blocks follow the header.
    pub copies: usize,
    /// Size of the save memory.
    pub memory_size: usize,
}

impl SaveLayout {
    /// Bytes one copy takes: its blocks and their sums.
    #[must_use]
    pub fn copy_len(&self) -> usize {
        self.blocks.iter().map(|size| size + CHECKSUM_LEN).sum()
    }

    /// Bytes the header and every copy take from the start of the memory.
    #[must_use]
    pub fn used_len(&self) -> usize {
        self.magic.len() + self.copies * self.copy_len()
    }

    fn block_offset(&self, copy: usize, block: usize) -> Option<usize> {
        if copy >= self.copies || block >= self.blocks.len() {
            return None;
        }
        let before: usize = self.blocks[..block]
            .iter()
            .map(|size| size + CHECKSUM_LEN)
            .sum();
        Some(self.magic.len() + copy * self.copy_len() + before)
    }
}

/// The sum the save routine stores after a block: every byte added into a
/// 32-bit value.
#[must_use]
pub fn checksum(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0u32, |sum, byte| sum.wrapping_add(u32::from(*byte)))
}

/// Why the save memory cannot be read or written as the layout says.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SaveMemoryError {
    /// The image is shorter than the layout.
    #[error("the save memory is {actual} bytes, the layout needs {needed}")]
    TooSmall {
        /// Bytes given.
        actual: usize,
        /// Bytes the layout needs.
        needed: usize,
    },
    /// The layout has no such copy or block.
    #[error("the save layout has no block {block} in copy {copy}")]
    NoSuchBlock {
        /// Copy asked for.
        copy: usize,
        /// Block asked for.
        block: usize,
    },
    /// Data of the wrong size for the block.
    #[error("block {block} holds {expected} bytes, not {actual}")]
    WrongLength {
        /// Block written.
        block: usize,
        /// Its size.
        expected: usize,
        /// Bytes given.
        actual: usize,
    },
}

/// An image of the save memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveMemory {
    bytes: Vec<u8>,
}

impl SaveMemory {
    /// A memory that was never written.
    #[must_use]
    pub fn erased(layout: &SaveLayout) -> Self {
        Self {
            bytes: vec![ERASED; layout.memory_size.max(layout.used_len())],
        }
    }

    /// A fresh memory ready for data: the header written and every block
    /// of every copy zero with a matching sum.
    #[must_use]
    pub fn formatted(layout: &SaveLayout) -> Self {
        let mut memory = Self::erased(layout);
        memory.format(layout);
        for copy in 0..layout.copies {
            for (block, size) in layout.blocks.iter().enumerate() {
                if let Some(offset) = layout.block_offset(copy, block) {
                    memory.write_block_at(offset, &vec![0; *size]);
                }
            }
        }
        memory
    }

    /// Reads an image, which must cover at least the layout.
    ///
    /// # Errors
    ///
    /// Returns [`SaveMemoryError::TooSmall`] when it does not.
    pub fn from_bytes(bytes: Vec<u8>, layout: &SaveLayout) -> Result<Self, SaveMemoryError> {
        if bytes.len() < layout.used_len() {
            return Err(SaveMemoryError::TooSmall {
                actual: bytes.len(),
                needed: layout.used_len(),
            });
        }
        Ok(Self { bytes })
    }

    /// The image.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Whether the header is there, which the game takes to mean the
    /// memory holds its data.
    #[must_use]
    pub fn is_formatted(&self, layout: &SaveLayout) -> bool {
        self.bytes.get(..layout.magic.len()) == Some(layout.magic.as_slice())
    }

    /// Writes the header.
    pub fn format(&mut self, layout: &SaveLayout) {
        self.write_at(0, &layout.magic);
    }

    /// Block `block` of copy `copy`, or `None` when its sum does not match.
    ///
    /// # Errors
    ///
    /// Returns [`SaveMemoryError::NoSuchBlock`] for a block outside the
    /// layout.
    pub fn block(
        &self,
        layout: &SaveLayout,
        copy: usize,
        block: usize,
    ) -> Result<Option<&[u8]>, SaveMemoryError> {
        let offset = layout
            .block_offset(copy, block)
            .ok_or(SaveMemoryError::NoSuchBlock { copy, block })?;
        let size = layout.blocks[block];
        let data = &self.bytes[offset..offset + size];
        let stored = &self.bytes[offset + size..offset + size + CHECKSUM_LEN];
        let stored = u32::from_le_bytes([stored[0], stored[1], stored[2], stored[3]]);
        Ok((checksum(data) == stored).then_some(data))
    }

    /// Writes block `block` of copy `copy` and its sum.
    ///
    /// # Errors
    ///
    /// Returns [`SaveMemoryError`] for a block outside the layout or data
    /// of another size.
    pub fn set_block(
        &mut self,
        layout: &SaveLayout,
        copy: usize,
        block: usize,
        data: &[u8],
    ) -> Result<(), SaveMemoryError> {
        let offset = layout
            .block_offset(copy, block)
            .ok_or(SaveMemoryError::NoSuchBlock { copy, block })?;
        let expected = layout.blocks[block];
        if data.len() != expected {
            return Err(SaveMemoryError::WrongLength {
                block,
                expected,
                actual: data.len(),
            });
        }
        self.write_block_at(offset, data);
        Ok(())
    }

    /// The bytes after the last copy, which the game never touches.
    #[must_use]
    pub fn spare(&self, layout: &SaveLayout) -> &[u8] {
        &self.bytes[layout.used_len()..]
    }

    /// The bytes after the last copy, for writing.
    pub fn spare_mut(&mut self, layout: &SaveLayout) -> &mut [u8] {
        &mut self.bytes[layout.used_len()..]
    }

    fn write_at(&mut self, offset: usize, data: &[u8]) {
        self.bytes[offset..offset + data.len()].copy_from_slice(data);
    }

    fn write_block_at(&mut self, offset: usize, data: &[u8]) {
        self.write_at(offset, data);
        self.write_at(offset + data.len(), &checksum(data).to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn layout() -> SaveLayout {
        SaveLayout {
            magic: b"TEST\0".to_vec(),
            blocks: vec![6, 2],
            copies: 2,
            memory_size: 64,
        }
    }

    #[test]
    fn places_the_copies_after_the_header() {
        let layout = layout();
        assert_eq!(layout.copy_len(), 16);
        assert_eq!(layout.used_len(), 37);
        assert_eq!(layout.block_offset(0, 0), Some(5));
        assert_eq!(layout.block_offset(0, 1), Some(15));
        assert_eq!(layout.block_offset(1, 0), Some(21));
        assert_eq!(layout.block_offset(2, 0), None);
    }

    #[test]
    fn sums_every_byte_into_32_bits() {
        assert_eq!(checksum(&[1, 2, 0xFF]), 0x102);
        assert_eq!(checksum(&[]), 0);
    }

    #[test]
    fn writes_blocks_with_their_sum_and_reads_them_back() {
        let layout = layout();
        let mut memory = SaveMemory::erased(&layout);
        assert!(!memory.is_formatted(&layout));
        memory.format(&layout);
        memory
            .set_block(&layout, 1, 0, &[1, 2, 3, 4, 5, 6])
            .expect("block fits");
        assert!(memory.is_formatted(&layout));
        assert_eq!(&memory.bytes()[21..31], &[1, 2, 3, 4, 5, 6, 21, 0, 0, 0]);
        assert_eq!(
            memory.block(&layout, 1, 0),
            Ok(Some([1, 2, 3, 4, 5, 6].as_slice()))
        );
        assert_eq!(memory.block(&layout, 0, 0), Ok(None));
    }

    #[test]
    fn a_changed_byte_breaks_the_sum() {
        let layout = layout();
        let mut memory = SaveMemory::formatted(&layout);
        assert_eq!(memory.block(&layout, 0, 1), Ok(Some([0, 0].as_slice())));
        let mut bytes = memory.bytes().to_vec();
        bytes[15] = 9;
        memory = SaveMemory::from_bytes(bytes, &layout).expect("large enough");
        assert_eq!(memory.block(&layout, 0, 1), Ok(None));
    }

    #[test]
    fn rejects_short_images_and_wrong_sizes() {
        let layout = layout();
        assert_eq!(
            SaveMemory::from_bytes(vec![0; 20], &layout),
            Err(SaveMemoryError::TooSmall {
                actual: 20,
                needed: 37
            })
        );
        let mut memory = SaveMemory::erased(&layout);
        assert_eq!(
            memory.set_block(&layout, 0, 1, &[1]),
            Err(SaveMemoryError::WrongLength {
                block: 1,
                expected: 2,
                actual: 1
            })
        );
        assert_eq!(
            memory.set_block(&layout, 0, 2, &[1]),
            Err(SaveMemoryError::NoSuchBlock { copy: 0, block: 2 })
        );
    }

    #[test]
    fn leaves_the_spare_bytes_to_the_caller() {
        let layout = layout();
        let mut memory = SaveMemory::formatted(&layout);
        assert_eq!(memory.spare(&layout), &[ERASED; 27]);
        memory.spare_mut(&layout)[0] = 7;
        assert_eq!(memory.bytes()[37], 7);
    }
}
