//! The battle scenes cutscenes stage, and the battle graphics they show.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the scene
//! call at `0x08008E4C`, the battle module's entry at `0x08003590` and the
//! quote selection at `0x08042916`, with the loads the scene makes traced
//! in a reference emulator (breakpoints on the game's LZ77 and `CpuSet`
//! wrappers at `0x0805D3A0` and `0x0805D388`) and checked byte for byte
//! against the VRAM and palette RAM it left.
//!
//! A scene is a 44-byte record of the table at ROM `0x66429C`; the cutscene
//! scenes of the opening use these fields:
//!
//! | Offset | Content |
//! |---|---|
//! | `0x11` | Scenery: index of the backgrounds below |
//! | `0x14` | The enemy's Zoid |
//! | `0x15` | Its pilot (a character) |
//! | `0x16` | Its quote: string `172 + n` of the `battle` table |
//!
//! Battle images are 128×128 pixels of 256 8bpp tiles with 64 colors,
//! LZ77-compressed: the scenery's at the pointers of ROM `0x6F6968` (tiles)
//! and `0x6F6BF0` (palette) by scenery, loaded to palette entries 64–127;
//! a Zoid's at the first of three pointers per Zoid of ROM `0x6F8974`
//! (tiles) and `0x6F9100` (palette), loaded to entries 0–63.

use formats::lz77;
use formats::tile::TILE_PIXELS;

const SCENES: usize = 0x0066_429C;
const SCENE_LEN: usize = 44;
const SCENERY_FIELD: usize = 0x11;
const ENEMY_FIELD: usize = 0x14;
const SCENERY_TILES: usize = 0x006F_6968;
const SCENERY_PALETTES: usize = 0x006F_6BF0;
const ZOID_TILES: usize = 0x006F_8974;
const ZOID_PALETTES: usize = 0x006F_9100;
const ZOID_IMAGES: usize = 3;
const IMAGE_TILES: usize = 256;
const IMAGE_COLORS: usize = 64;
/// First `battle` string of the quotes the scenes' enemies speak.
pub const QUOTE_BASE: usize = 172;
const ROM_BASE: u32 = 0x0800_0000;

/// What a staged battle scene shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleScene {
    /// The scenery behind the Zoids.
    pub scenery: u8,
    /// The enemy's Zoid.
    pub zoid: u8,
    /// The enemy's pilot.
    pub pilot: u8,
    /// The `battle` string the pilot speaks.
    pub quote: usize,
}

/// A 128×128 battle image: 256 tiles of palette indices and its colors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleImage {
    /// 8bpp tiles, row after row of 16.
    pub tiles: Vec<[u8; TILE_PIXELS]>,
    /// BGR555 colors, to be loaded where the image's indices expect them.
    pub palette: Vec<u16>,
}

/// Scene `index` of the table, if the ROM has it.
#[must_use]
pub fn battle_scene(rom: &[u8], index: usize) -> Option<BattleScene> {
    let at = SCENES + index * SCENE_LEN;
    let record = rom.get(at..at + SCENE_LEN)?;
    Some(BattleScene {
        scenery: record[SCENERY_FIELD],
        zoid: record[ENEMY_FIELD],
        pilot: record[ENEMY_FIELD + 1],
        quote: QUOTE_BASE + usize::from(record[ENEMY_FIELD + 2]),
    })
}

/// The scenery image `scenery`.
#[must_use]
pub fn scenery_image(rom: &[u8], scenery: u8) -> Option<BattleImage> {
    let slot = usize::from(scenery) * 4;
    image(rom, SCENERY_TILES + slot, SCENERY_PALETTES + slot)
}

/// Zoid `zoid`'s battle image.
#[must_use]
pub fn zoid_image(rom: &[u8], zoid: u8) -> Option<BattleImage> {
    let slot = usize::from(zoid) * ZOID_IMAGES * 4;
    image(rom, ZOID_TILES + slot, ZOID_PALETTES + slot)
}

fn image(rom: &[u8], tiles: usize, palette: usize) -> Option<BattleImage> {
    let tile_bytes = decompress_at(rom, tiles)?;
    let palette_bytes = decompress_at(rom, palette)?;
    let tiles = tile_bytes
        .chunks_exact(TILE_PIXELS)
        .take(IMAGE_TILES)
        .map(|chunk| {
            let mut tile = [0; TILE_PIXELS];
            tile.copy_from_slice(chunk);
            tile
        })
        .collect::<Vec<_>>();
    let palette = palette_bytes
        .chunks_exact(2)
        .take(IMAGE_COLORS)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    (tiles.len() == IMAGE_TILES && palette.len() == IMAGE_COLORS)
        .then_some(BattleImage { tiles, palette })
}

fn decompress_at(rom: &[u8], pointer_at: usize) -> Option<Vec<u8>> {
    let pointer = rom.get(pointer_at..pointer_at + 4)?;
    let address = u32::from_le_bytes([pointer[0], pointer[1], pointer[2], pointer[3]]);
    let offset = usize::try_from(address.checked_sub(ROM_BASE)?).ok()?;
    lz77::decompress(rom.get(offset..)?)
        .ok()
        .map(|(bytes, _)| bytes)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn put_pointer(rom: &mut [u8], at: usize, target: usize) {
        let address = ROM_BASE + u32::try_from(target).expect("offset");
        rom[at..at + 4].copy_from_slice(&address.to_le_bytes());
    }

    /// An LZ77 block of `len` literal bytes all equal to `value`.
    fn literal_block(len: usize, value: u8) -> Vec<u8> {
        let mut block = vec![0x10];
        block.extend_from_slice(&u32::try_from(len).expect("len").to_le_bytes()[..3]);
        for chunk in (0..len).collect::<Vec<_>>().chunks(8) {
            block.push(0);
            block.extend(chunk.iter().map(|_| value));
        }
        block
    }

    #[test]
    fn reads_a_scene_and_its_images() {
        let data = 0x0070_0000;
        let mut rom = vec![0; data + 0x6000];
        let scene = SCENES + 2 * SCENE_LEN;
        rom[scene + SCENERY_FIELD] = 8;
        rom[scene + ENEMY_FIELD..scene + ENEMY_FIELD + 3].copy_from_slice(&[0x49, 0x3A, 1]);
        let tiles = literal_block(IMAGE_TILES * TILE_PIXELS, 5);
        let palette = literal_block(IMAGE_COLORS * 2, 0x11);
        rom[data..data + tiles.len()].copy_from_slice(&tiles);
        let palette_at = data + tiles.len();
        rom[palette_at..palette_at + palette.len()].copy_from_slice(&palette);
        put_pointer(&mut rom, SCENERY_TILES + 8 * 4, data);
        put_pointer(&mut rom, SCENERY_PALETTES + 8 * 4, palette_at);
        put_pointer(&mut rom, ZOID_TILES + 0x49 * 12, data);
        put_pointer(&mut rom, ZOID_PALETTES + 0x49 * 12, palette_at);
        let read = battle_scene(&rom, 2).expect("scene");
        assert_eq!(
            read,
            BattleScene {
                scenery: 8,
                zoid: 0x49,
                pilot: 0x3A,
                quote: 173
            }
        );
        let scenery = scenery_image(&rom, 8).expect("scenery");
        assert_eq!(scenery.tiles.len(), IMAGE_TILES);
        assert_eq!(scenery.tiles[255][63], 5);
        assert_eq!(scenery.palette[63], 0x1111);
        assert_eq!(zoid_image(&rom, 0x49), Some(scenery));
        assert_eq!(zoid_image(&rom, 0x4A), None);
    }
}
