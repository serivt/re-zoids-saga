//! ROM identification and extraction into the intermediate game database, with caching keyed by ROM hash and extractor version.

pub mod identify;

pub use identify::{Identification, IdentifyError, KnownRelease, Title, identify};
