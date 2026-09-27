//! ROM identification and extraction into the intermediate game database, with caching keyed by ROM hash and extractor version.

pub mod identify;
pub mod saga;
pub mod saga_arena;
pub mod saga_battle;
pub mod saga_combat;
pub mod saga_encounter;
pub mod saga_formation;
pub mod saga_guide;
pub mod saga_party;
pub mod saga_save;
pub mod saga_shop;
pub mod string_table;

pub use identify::{Identification, IdentifyError, KnownRelease, Title, identify};
pub use string_table::{StringTable, StringTableError, TableString};
