//! Codecs for the reverse-engineered ROM formats: compression, tilesets, palettes, text encoding and script bytecode. Pure decoding, no gameplay knowledge.

pub mod bgr555;
pub mod font;
pub mod lz77;
pub mod m4a;
pub mod pixel_font;
pub mod rom_header;
pub mod script_ops;
pub mod script_text;
pub mod tile;
pub mod tilemap;

pub use font::{FontError, Glyph, GlyphIndex};
pub use lz77::Lz77Error;
pub use m4a::{Command, Envelope, M4aError, Running, Sample, SongHeader, Voice};
pub use pixel_font::{PIXEL_FONT_ROWS, PixelFont, PixelFontError, PixelGlyph};
pub use rom_header::{HeaderError, RomHeader};
pub use script_ops::{
    Instruction, MessageStep, ScriptOpError, decode_instruction, decode_message_step,
};
pub use script_text::{Element, Piece, Script, ScriptTextError};
pub use tile::{TileImage, TilePiece, Tileset};
pub use tilemap::TileMap;
