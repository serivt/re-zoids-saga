//! The sound driver's tables in the ROM and the routines that turn a key
//! into a rate: the sampled voices' (`0x0805BC90`) and the programmable
//! channels' (`0x0805C744`).
//!
//! Source of knowledge: own reading of those routines in Zoids Saga
//! (Japan, Rev 1); the tables are read from the player's ROM.

/// Where the driver's settings and tables are in the ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverLayout {
    /// Offset of the song table.
    pub song_table: usize,
    /// Entries in the song table.
    pub song_count: usize,
    /// The driver's master volume, 0–15.
    pub master_volume: u8,
    /// Per key, a byte: the frequency table's entry (low nibble) and the
    /// shift that takes it to the key's octave (high nibble).
    pub key_table: usize,
    /// Twelve 32-bit frequencies of the top octave.
    pub frequency_table: usize,
    /// The same per key for the tone channels, from key 36.
    pub cgb_key_table: usize,
    /// Twelve signed 16-bit register values of the lowest octave.
    pub cgb_frequency_table: usize,
    /// Per key from 21, the noise channel's frequency byte (`NR43`).
    pub noise_table: usize,
    /// Per envelope level, the wave channel's volume byte (`NR32`).
    pub wave_volume_table: usize,
}

const KEYS: usize = 180;
const FREQUENCIES: usize = 12;
const CGB_KEYS: usize = 132;
const NOISE_KEYS: usize = 60;
const LEVELS: usize = 16;
const TOP_KEY: u8 = 178;
const CGB_LOW_KEY: u8 = 35;
const CGB_FIRST_KEY: u8 = 36;
const CGB_TOP_STEP: u8 = 130;
const NOISE_LOW_KEY: u8 = 20;
const NOISE_FIRST_KEY: u8 = 21;
const NOISE_TOP_STEP: u8 = 59;
const CGB_REGISTER_BASE: i32 = 2048;
const FINE_SHIFT: u32 = 24;

/// The tables, read once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tables {
    key: [u8; KEYS],
    frequency: [u32; FREQUENCIES],
    cgb_key: [u8; CGB_KEYS],
    cgb_frequency: [i16; FREQUENCIES],
    noise: [u8; NOISE_KEYS],
    wave_volume: [u8; LEVELS],
}

impl Tables {
    /// Reads the tables at `layout`'s offsets; bytes past the ROM read 0.
    #[must_use]
    pub fn read(rom: &[u8], layout: &DriverLayout) -> Self {
        let byte = |at: usize| rom.get(at).copied().unwrap_or(0);
        Self {
            key: std::array::from_fn(|index| byte(layout.key_table + index)),
            frequency: std::array::from_fn(|index| {
                let at = layout.frequency_table + index * 4;
                u32::from_le_bytes([byte(at), byte(at + 1), byte(at + 2), byte(at + 3)])
            }),
            cgb_key: std::array::from_fn(|index| byte(layout.cgb_key_table + index)),
            cgb_frequency: std::array::from_fn(|index| {
                let at = layout.cgb_frequency_table + index * 2;
                i16::from_le_bytes([byte(at), byte(at + 1)])
            }),
            noise: std::array::from_fn(|index| byte(layout.noise_table + index)),
            wave_volume: std::array::from_fn(|index| byte(layout.wave_volume_table + index)),
        }
    }

    /// The rate of a sampled voice (`0x0805BC90`): the sample's rate times
    /// the key's step, interpolated toward the next key by `fine` / 256,
    /// in the driver's 32-bit fixed point (about hertz). Keys above 178
    /// play 178 and all of its fine step.
    #[must_use]
    pub fn sample_frequency(&self, sample_rate: u32, key: u8, fine: u8) -> u32 {
        let (key, fine) = if key > TOP_KEY {
            (TOP_KEY, u8::MAX)
        } else {
            (key, fine)
        };
        let step = |key: u8| {
            let entry = self.key[usize::from(key)];
            self.frequency[usize::from(entry & 0x0F) % FREQUENCIES] >> (entry >> 4)
        };
        let low = step(key);
        let high = step(key + 1);
        let between = high_word(high.wrapping_sub(low), u32::from(fine) << FINE_SHIFT);
        high_word(sample_rate, low.wrapping_add(between))
    }

    /// The frequency value of a tone channel (`0x0805C744`): the register
    /// for the key from 36 on, interpolated toward the next key by `fine`
    /// / 256, plus 2048 (the register is its low 11 bits). Keys below 36
    /// play 36, keys above 166 play 166 and all of its fine step.
    #[must_use]
    pub fn tone_frequency(&self, key: u8, fine: u8) -> u32 {
        let (step, fine) = if key <= CGB_LOW_KEY {
            (0, 0)
        } else if key - CGB_FIRST_KEY > CGB_TOP_STEP {
            (CGB_TOP_STEP, u8::MAX)
        } else {
            (key - CGB_FIRST_KEY, fine)
        };
        let value = |step: u8| {
            let entry = self.cgb_key[usize::from(step)];
            i32::from(self.cgb_frequency[usize::from(entry & 0x0F) % FREQUENCIES]) >> (entry >> 4)
        };
        let low = value(step);
        let high = value(step + 1);
        let frequency = low + (((high - low) * i32::from(fine)) >> 8) + CGB_REGISTER_BASE;
        u32::try_from(frequency).unwrap_or(0)
    }

    /// The noise channel's frequency byte (`NR43`) for a key: keys up to 20
    /// give the first entry, keys past 80 the last.
    #[must_use]
    pub fn noise_frequency(&self, key: u8) -> u8 {
        let step = if key <= NOISE_LOW_KEY {
            0
        } else {
            (key - NOISE_FIRST_KEY).min(NOISE_TOP_STEP)
        };
        self.noise[usize::from(step)]
    }

    /// The wave channel's volume byte (`NR32`) for an envelope level.
    #[must_use]
    pub fn wave_volume(&self, level: u8) -> u8 {
        self.wave_volume[usize::from(level) % LEVELS]
    }
}

/// The high word of an unsigned 32 × 32-bit product (`0x0805B0F8`).
#[must_use]
pub fn high_word(a: u32, b: u32) -> u32 {
    u32::try_from((u64::from(a) * u64::from(b)) >> 32).unwrap_or(u32::MAX)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A synthetic layout: every table after `base`, the sampled key table
    /// mapping key `k` to entry `k % 12` shifted by `14 - k / 12` (key 60
    /// at the entry's value >> 9), the frequencies an octave's worth of
    /// doublings, the tone table one entry per key.
    pub(crate) fn tables_in(rom: &mut Vec<u8>, base: usize) -> DriverLayout {
        let layout = DriverLayout {
            song_table: 0,
            song_count: 0,
            master_volume: 14,
            key_table: base,
            frequency_table: base + KEYS,
            cgb_key_table: base + KEYS + FREQUENCIES * 4,
            cgb_frequency_table: base + KEYS + FREQUENCIES * 4 + CGB_KEYS,
            noise_table: base + KEYS + FREQUENCIES * 6 + CGB_KEYS,
            wave_volume_table: base + KEYS + FREQUENCIES * 6 + CGB_KEYS + NOISE_KEYS,
        };
        rom.resize(layout.wave_volume_table + LEVELS, 0);
        for key in 0..KEYS {
            let octave = u8::try_from(14usize.saturating_sub(key / 12)).unwrap_or(0);
            rom[layout.key_table + key] = (octave << 4) | u8::try_from(key % 12).unwrap_or(0);
        }
        for index in 0..FREQUENCIES {
            let value: u32 = 0x8000_0000 + u32::try_from(index).unwrap_or(0) * 0x0800_0000;
            rom[layout.frequency_table + index * 4..layout.frequency_table + index * 4 + 4]
                .copy_from_slice(&value.to_le_bytes());
            let tone = -2004i16 + i16::try_from(index).unwrap_or(0) * 80;
            rom[layout.cgb_frequency_table + index * 2..layout.cgb_frequency_table + index * 2 + 2]
                .copy_from_slice(&tone.to_le_bytes());
        }
        for step in 0..CGB_KEYS {
            let octave = u8::try_from(step / 12).unwrap_or(0);
            rom[layout.cgb_key_table + step] = (octave << 4) | u8::try_from(step % 12).unwrap_or(0);
        }
        for step in 0..NOISE_KEYS {
            rom[layout.noise_table + step] = u8::try_from(step).unwrap_or(0);
        }
        for level in 0..LEVELS {
            rom[layout.wave_volume_table + level] = u8::try_from(level).unwrap_or(0);
        }
        layout
    }

    #[test]
    fn a_sampled_key_scales_the_rate_and_fine_steps_interpolate() {
        let mut rom = Vec::new();
        let layout = tables_in(&mut rom, 0);
        let tables = Tables::read(&rom, &layout);
        let rate = 13_379 * 1024;
        assert_eq!(tables.sample_frequency(rate, 60, 0), 13_379);
        let half = tables.sample_frequency(rate, 60, 128);
        let next = tables.sample_frequency(rate, 61, 0);
        assert!(half > 13_379 && half < next);
        assert_eq!(
            tables.sample_frequency(rate, 200, 0),
            tables.sample_frequency(rate, 178, 255)
        );
    }

    #[test]
    fn tone_keys_clamp_and_noise_keys_index_their_table() {
        let mut rom = Vec::new();
        let layout = tables_in(&mut rom, 0);
        let tables = Tables::read(&rom, &layout);
        assert_eq!(tables.tone_frequency(36, 0), 2048 - 2004);
        assert_eq!(tables.tone_frequency(10, 200), 2048 - 2004);
        assert_eq!(tables.tone_frequency(37, 0), 2048 - 1924);
        assert_eq!(tables.tone_frequency(36, 128), 2048 - 1964);
        assert_eq!(tables.noise_frequency(0), 0);
        assert_eq!(tables.noise_frequency(30), 9);
        assert_eq!(tables.noise_frequency(127), 59);
        assert_eq!(tables.wave_volume(5), 5);
        assert_eq!(high_word(0x8000_0000, 4), 2);
    }
}
