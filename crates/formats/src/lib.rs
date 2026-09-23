//! Codecs for the reverse-engineered ROM formats: compression, tilesets, palettes, text encoding and script bytecode. Pure decoding, no gameplay knowledge.

pub mod font;
pub mod rom_header;
pub mod script_text;

pub use font::{FontError, Glyph, GlyphIndex};
pub use rom_header::{HeaderError, RomHeader};
pub use script_text::{Element, Piece, Script, ScriptTextError};
