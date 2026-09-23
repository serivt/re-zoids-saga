//! Tables of script strings: an array of 32-bit ROM pointers, one per string;
//! a null pointer marks an index with no string.

use formats::script_text::{Script, ScriptTextError};
use thiserror::Error;

const ROM_BASE: u32 = 0x0800_0000;
const POINTER_LEN: usize = 4;

/// Where a string table lives in a ROM and what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringTable {
    /// Identifier used to build string IDs, e.g. `dialogue`.
    pub name: &'static str,
    /// Offset of the pointer array from the start of the ROM.
    pub offset: usize,
    /// Number of pointers in the array.
    pub count: usize,
}

/// A string read from a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableString {
    /// Stable identifier: the table name and the zero-based index, e.g. `dialogue_00003`.
    pub id: String,
    /// Offset of the string's first byte from the start of the ROM; 0 for
    /// an index whose pointer is null.
    pub offset: usize,
    /// Decoded script, empty for a null pointer.
    pub script: Script,
}

impl TableString {
    /// Whether the table holds a string at this index.
    #[must_use]
    pub fn is_present(&self) -> bool {
        self.offset != 0
    }
}

/// Why a table could not be read.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum StringTableError {
    /// The pointer array does not fit in the ROM.
    #[error("table {name} at {offset:#x} with {count} entries does not fit in the ROM")]
    TableOutOfBounds {
        /// Table name.
        name: &'static str,
        /// Table offset.
        offset: usize,
        /// Entry count.
        count: usize,
    },
    /// A pointer does not point inside the ROM.
    #[error("{id} points to {pointer:#010x}, outside the ROM")]
    PointerOutOfBounds {
        /// String identifier.
        id: String,
        /// Pointer value as stored.
        pointer: u32,
    },
    /// A string could not be decoded.
    #[error("{id}: {source}")]
    Decode {
        /// String identifier.
        id: String,
        /// Decoding failure.
        source: ScriptTextError,
    },
}

impl StringTable {
    /// Reads and decodes every string of the table from `rom`.
    ///
    /// # Errors
    ///
    /// Returns [`StringTableError`] when the table or a pointer falls outside
    /// the ROM, or a string does not decode.
    pub fn read(&self, rom: &[u8]) -> Result<Vec<TableString>, StringTableError> {
        let end = self
            .offset
            .checked_add(self.count.saturating_mul(POINTER_LEN))
            .filter(|end| *end <= rom.len())
            .ok_or(StringTableError::TableOutOfBounds {
                name: self.name,
                offset: self.offset,
                count: self.count,
            })?;
        rom[self.offset..end]
            .chunks_exact(POINTER_LEN)
            .enumerate()
            .map(|(index, pointer)| self.read_entry(rom, index, pointer))
            .collect()
    }

    /// Reads only where each string starts (0 for a null pointer), without
    /// decoding, so tables holding undecodable strings can still be run.
    ///
    /// # Errors
    ///
    /// Returns [`StringTableError`] when the table or a pointer falls
    /// outside the ROM.
    pub fn offsets(&self, rom: &[u8]) -> Result<Vec<usize>, StringTableError> {
        let end = self
            .offset
            .checked_add(self.count.saturating_mul(POINTER_LEN))
            .filter(|end| *end <= rom.len())
            .ok_or(StringTableError::TableOutOfBounds {
                name: self.name,
                offset: self.offset,
                count: self.count,
            })?;
        rom[self.offset..end]
            .chunks_exact(POINTER_LEN)
            .enumerate()
            .map(|(index, bytes)| {
                let pointer = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                if pointer == 0 {
                    return Ok(0);
                }
                pointer
                    .checked_sub(ROM_BASE)
                    .map(|offset| offset as usize)
                    .filter(|offset| *offset < rom.len())
                    .ok_or_else(|| StringTableError::PointerOutOfBounds {
                        id: format!("{}_{index:05}", self.name),
                        pointer,
                    })
            })
            .collect()
    }

    fn read_entry(
        &self,
        rom: &[u8],
        index: usize,
        pointer: &[u8],
    ) -> Result<TableString, StringTableError> {
        let id = format!("{}_{index:05}", self.name);
        let pointer = u32::from_le_bytes([pointer[0], pointer[1], pointer[2], pointer[3]]);
        if pointer == 0 {
            return Ok(TableString {
                id,
                offset: 0,
                script: Script::default(),
            });
        }
        let offset = pointer
            .checked_sub(ROM_BASE)
            .map(|offset| offset as usize)
            .filter(|offset| *offset < rom.len())
            .ok_or_else(|| StringTableError::PointerOutOfBounds {
                id: id.clone(),
                pointer,
            })?;
        let (script, _) =
            Script::decode(&rom[offset..]).map_err(|source| StringTableError::Decode {
                id: id.clone(),
                source,
            })?;
        Ok(TableString { id, offset, script })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use formats::script_text::{Element, Piece};

    fn rom_with_table(strings: &[&[u8]]) -> (Vec<u8>, StringTable) {
        let table_offset = 0x100;
        let mut rom = vec![0u8; table_offset + strings.len() * POINTER_LEN];
        for (index, bytes) in strings.iter().enumerate() {
            let pointer = ROM_BASE + u32::try_from(rom.len()).unwrap();
            rom[table_offset + index * POINTER_LEN..][..POINTER_LEN]
                .copy_from_slice(&pointer.to_le_bytes());
            rom.extend_from_slice(bytes);
        }
        let table = StringTable {
            name: "test",
            offset: table_offset,
            count: strings.len(),
        };
        (rom, table)
    }

    #[test]
    fn reads_every_entry_with_stable_ids() {
        let (rom, table) = rom_with_table(&[&[0x20, 0x5D, 0x83, 0x1D, 0x22], &[0x0B, 0x22]]);
        let strings = table.read(&rom).unwrap();
        assert_eq!(strings.len(), 2);
        assert_eq!(strings[0].id, "test_00000");
        assert_eq!(
            strings[0].script.elements,
            vec![Element::Message(vec![Piece::Text("ゾ".to_owned())])]
        );
        assert_eq!(strings[1].id, "test_00001");
        assert_eq!(strings[1].offset, 0x108 + 5);
    }

    #[test]
    fn rejects_a_table_past_the_end_of_the_rom() {
        let table = StringTable {
            name: "test",
            offset: 0x10,
            count: 4,
        };
        assert_eq!(
            table.read(&[0u8; 0x18]),
            Err(StringTableError::TableOutOfBounds {
                name: "test",
                offset: 0x10,
                count: 4
            })
        );
    }

    #[test]
    fn rejects_a_pointer_outside_the_rom() {
        let (mut rom, table) = rom_with_table(&[&[0x22]]);
        rom[0x100..0x104].copy_from_slice(&0x0900_0000u32.to_le_bytes());
        assert_eq!(
            table.read(&rom),
            Err(StringTableError::PointerOutOfBounds {
                id: "test_00000".to_owned(),
                pointer: 0x0900_0000
            })
        );
    }

    #[test]
    fn lists_offsets_without_decoding() {
        let (mut rom, table) = rom_with_table(&[&[0x22], &[0x20, 0x5D]]);
        let offsets = table.offsets(&rom).unwrap();
        assert_eq!(offsets, [0x108, 0x109]);
        rom[0x104..0x108].copy_from_slice(&[0; 4]);
        assert_eq!(table.offsets(&rom).unwrap(), [0x108, 0]);
    }

    #[test]
    fn keeps_null_pointers_as_absent_entries() {
        let (mut rom, table) = rom_with_table(&[&[0x22], &[0x22]]);
        rom[0x100..0x104].copy_from_slice(&[0; 4]);
        let strings = table.read(&rom).unwrap();
        assert!(!strings[0].is_present());
        assert_eq!(strings[0].id, "test_00000");
        assert_eq!(strings[0].script, Script::default());
        assert!(strings[1].is_present());
    }

    #[test]
    fn reports_which_string_failed_to_decode() {
        let (rom, table) = rom_with_table(&[&[0x22], &[0x20, 0x5D]]);
        let error = table.read(&rom).unwrap_err();
        assert!(matches!(error, StringTableError::Decode { ref id, .. } if id == "test_00001"));
    }
}
