//! Codecs for the reverse-engineered ROM formats: compression, tilesets, palettes, text encoding and script bytecode. Pure decoding, no gameplay knowledge.

pub mod rom_header;

pub use rom_header::{HeaderError, RomHeader};
