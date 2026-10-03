//! The saves in the browser's own storage (`localStorage`): each one under
//! a key of its own, its bytes as Base64 text, with the time it was stored.
//! The storage answers at once, as the game's saving expects; a browser
//! keeps a few megabytes a site, room for every slot (32 KiB each) many
//! times over.

use std::time::{Duration, SystemTime};

use platform::{SaveStorage, StorageError};

/// Every key the port stores starts with this.
const PREFIX: &str = "re-zoids-saga/";
const TIME_SUFFIX: &str = "#stored";
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const PAD: u8 = b'=';

/// A save kept under a key of the browser's storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalStorage {
    key: String,
}

impl LocalStorage {
    /// The save kept under `name` (as `re-zoids-saga/name`).
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            key: format!("{PREFIX}{name}"),
        }
    }

    fn storage() -> Result<web_sys::Storage, String> {
        web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
            .ok_or_else(|| "the browser keeps no storage for the page".to_owned())
    }
}

impl SaveStorage for LocalStorage {
    fn load(&self) -> Result<Option<Vec<u8>>, StorageError> {
        let storage = Self::storage().map_err(StorageError::Read)?;
        match storage.get_item(&self.key) {
            Ok(Some(text)) => decode(&text)
                .map(Some)
                .ok_or_else(|| StorageError::Read(format!("{} is not a save", self.key))),
            Ok(None) => Ok(None),
            Err(error) => Err(StorageError::Read(format!("{error:?}"))),
        }
    }

    fn store(&mut self, bytes: &[u8]) -> Result<(), StorageError> {
        let storage = Self::storage().map_err(StorageError::Write)?;
        storage
            .set_item(&self.key, &encode(bytes))
            .map_err(|error| StorageError::Write(format!("{error:?}")))?;
        let now = js_sys::Date::now().to_string();
        let _ = storage.set_item(&format!("{}{TIME_SUFFIX}", self.key), &now);
        Ok(())
    }

    fn modified(&self) -> Option<SystemTime> {
        let storage = Self::storage().ok()?;
        let stored = storage
            .get_item(&format!("{}{TIME_SUFFIX}", self.key))
            .ok()
            .flatten()?;
        let millis: u64 = stored.split('.').next()?.parse().ok()?;
        Some(SystemTime::UNIX_EPOCH + Duration::from_millis(millis))
    }
}

/// `bytes` as Base64 text.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let word = chunk.iter().enumerate().fold(0u32, |word, (at, &byte)| {
            word | u32::from(byte) << (16 - 8 * at)
        });
        for place in 0..4 {
            if place <= chunk.len() {
                let index = usize::try_from(word >> (18 - 6 * place) & 0x3F).unwrap_or(0);
                text.push(char::from(ALPHABET[index]));
            } else {
                text.push(char::from(PAD));
            }
        }
    }
    text
}

/// The bytes Base64 `text` holds, or `None` when it is not Base64.
#[must_use]
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let text = text.trim_end_matches(char::from(PAD)).as_bytes();
    let mut bytes = Vec::with_capacity(text.len() * 3 / 4);
    for chunk in text.chunks(4) {
        if chunk.len() == 1 {
            return None;
        }
        let mut word = 0u32;
        for (place, &symbol) in chunk.iter().enumerate() {
            let value = ALPHABET.iter().position(|&letter| letter == symbol)?;
            word |= u32::try_from(value).ok()? << (18 - 6 * place);
        }
        for at in 0..chunk.len() - 1 {
            bytes.push(u8::try_from(word >> (16 - 8 * at) & 0xFF).ok()?);
        }
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_gives_the_bytes_back() {
        assert_eq!(encode(b"Man"), "TWFu");
        assert_eq!(encode(b"Ma"), "TWE=");
        assert_eq!(encode(b"M"), "TQ==");
        for len in 0..10 {
            let bytes: Vec<u8> = (0..len)
                .map(|n: u8| n.wrapping_mul(37).wrapping_add(200))
                .collect();
            assert_eq!(decode(&encode(&bytes)), Some(bytes));
        }
        assert_eq!(decode("T!=="), None);
        assert_eq!(decode("TWFuT"), None);
    }
}
