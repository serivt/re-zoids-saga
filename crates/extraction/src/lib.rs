//! ROM identification and extraction into the intermediate game database, with caching keyed by ROM hash and extractor version.

pub mod identify;
pub mod saga;
pub mod saga_save;
pub mod string_table;

pub use identify::{Identification, IdentifyError, KnownRelease, Title, identify};
pub use string_table::{StringTable, StringTableError, TableString};
