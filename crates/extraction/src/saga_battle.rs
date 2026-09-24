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
//!
//! The shots' effects are sprites of the 282 16-byte records at ROM
//! `0x6F77D4`: LZ77-compressed 4bpp tiles, an LZ77-compressed palette, a
//! table of animations and a table of frames. A frame is a list of 20-byte
//! pieces ended by a tile of `0xFFFF`, which the sprite drawer at
//! `0x08000560` turns into OAM entries: the first tile, the flips (low two
//! bits) and a rotation (high byte), the offset from the anchor, the size,
//! the horizontal and vertical scale in 8.8 and, unless `0xFF`, an affine
//! slot with `0x200` for the double-size box.

use formats::bgr555::parse_palette;
use formats::lz77;
use formats::tile::{TILE_PIXELS, Tileset};

use crate::saga::{AnimationStep, read_steps, rom_offset};

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
const EFFECTS: usize = 0x006F_77D4;
const EFFECT_LEN: usize = 16;
const EFFECT_COUNT: usize = 282;
const WEAPON_SPRITES: usize = 0x006F_6E44;
const BACK_WEAPON_SPRITES: usize = 0x006F_77C4;
const WEAPON_MOUNTS: usize = 0x006E_78EC;
const MOUNT_LEN: usize = 28;
const MOUNTS: usize = 6;
const MOUNT_Y: usize = 0xE;
const PIECE_LEN: usize = 20;
const PIECES_END: u16 = 0xFFFF;
const PIECES_MAX: usize = 16;

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

/// One piece of an effect's frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectPiece {
    /// First tile, relative to the sprite's; the top four bits add to the
    /// palette.
    pub tile: u16,
    /// Flips in the low two bits, a rotation in the high byte.
    pub attributes: u16,
    /// Offset of the piece's top-left corner from the anchor.
    pub x: i16,
    /// See [`EffectPiece::x`].
    pub y: i16,
    /// Size in pixels.
    pub width: u16,
    /// See [`EffectPiece::width`].
    pub height: u16,
    /// Horizontal scale, 8.8 (`0x100` is the image's size).
    pub scale_x: i16,
    /// Vertical scale, 8.8.
    pub scale_y: i16,
    /// `0xFF` for a plain piece; otherwise an affine one, drawn in a box
    /// twice its size when bit `0x200` is set.
    pub affine: u16,
}

/// A shot effect's sprite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectSprite {
    /// Its 4bpp tiles.
    pub tiles: Tileset,
    /// Its 16 colors.
    pub palette: [u16; 16],
    /// Its frames, each a list of pieces.
    pub frames: Vec<Vec<EffectPiece>>,
    /// Its first animation.
    pub animation: Vec<AnimationStep>,
}

/// Effect sprite `id` of the table at ROM `0x6F77D4`.
#[must_use]
pub fn effect_sprite(rom: &[u8], id: usize) -> Option<EffectSprite> {
    if id >= EFFECT_COUNT {
        return None;
    }
    sprite_at(rom, EFFECTS + id * EFFECT_LEN)
}

/// The picture of weapon `part` mounted on rack `rack` of a Zoid, which
/// the equipment screen draws over the Zoid (`0x0804D768`): the second
/// rack's come from ROM `0x6F77C4`, the others' from `0x6F6E44`; `None`
/// for a part without one.
#[must_use]
pub fn weapon_sprite(rom: &[u8], part: u16, rack: usize) -> Option<EffectSprite> {
    let table = if rack == 1 {
        BACK_WEAPON_SPRITES
    } else {
        WEAPON_SPRITES
    };
    sprite_at(rom, table + usize::from(part) * EFFECT_LEN)
}

/// Where rack `rack`'s weapon sits on Zoid `zoid`'s picture
/// (`0x08045818`, `0x08045850`): the Zoid's 28-byte record at ROM
/// `0x6E78EC` gives six x then six y, `0xFFFF` for a default after each
/// six.
#[must_use]
pub fn weapon_mount(rom: &[u8], zoid: u16, rack: usize) -> Option<(i16, i16)> {
    let at = WEAPON_MOUNTS + usize::from(zoid) * MOUNT_LEN;
    let record = rom.get(at..at + MOUNT_LEN)?;
    let value = |first: usize| {
        let read = |index: usize| i16::from_le_bytes([record[index], record[index + 1]]);
        let own = read(first + rack.min(MOUNTS - 1) * 2);
        if own == -1 {
            read(first + MOUNTS * 2)
        } else {
            own
        }
    };
    Some((value(0), value(MOUNT_Y)))
}

fn sprite_at(rom: &[u8], at: usize) -> Option<EffectSprite> {
    let record = rom.get(at..at + EFFECT_LEN)?;
    let pointer = |at: usize| rom_offset(&record[at..at + 4]);
    let (tile_bytes, _) = lz77::decompress(rom.get(pointer(0)?..)?).ok()?;
    let (palette_bytes, _) = lz77::decompress(rom.get(pointer(4)?..)?).ok()?;
    let palette = parse_palette(&palette_bytes)?;
    let first_animation = rom_offset(rom.get(pointer(8)?..pointer(8)? + 4)?)?;
    let animation = read_steps(rom, first_animation)?;
    let frame_table = pointer(12)?;
    let frame_count = animation
        .iter()
        .map(|step| step.frame + 1)
        .max()
        .unwrap_or(0);
    let frames = (0..frame_count)
        .map(|index| {
            let at = frame_table + index * 4;
            read_pieces(rom, rom_offset(rom.get(at..at + 4)?)?)
        })
        .collect::<Option<Vec<_>>>()?;
    Some(EffectSprite {
        tiles: Tileset::from_4bpp(&tile_bytes),
        palette,
        frames,
        animation,
    })
}

pub(crate) fn read_pieces(rom: &[u8], mut at: usize) -> Option<Vec<EffectPiece>> {
    let mut pieces = Vec::new();
    for _ in 0..PIECES_MAX {
        let bytes = rom.get(at..at + PIECE_LEN)?;
        let half = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        let signed = |i: usize| i16::from_le_bytes([bytes[i], bytes[i + 1]]);
        if half(0) == PIECES_END {
            return Some(pieces);
        }
        pieces.push(EffectPiece {
            tile: half(0),
            attributes: half(2),
            x: signed(4),
            y: signed(6),
            width: half(8),
            height: half(10),
            scale_x: signed(12),
            scale_y: signed(14),
            affine: half(16),
        });
        at += PIECE_LEN;
    }
    None
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

    #[test]
    fn a_rack_without_its_own_mount_takes_the_default() {
        let mut rom = vec![0; WEAPON_MOUNTS + 3 * MOUNT_LEN];
        let at = WEAPON_MOUNTS + 2 * MOUNT_LEN;
        for (index, value) in [87i16, 64, -1, 13, 38, -1, 74, 80, 62, -1, 75, 72, -1, 73]
            .iter()
            .enumerate()
        {
            rom[at + index * 2..at + index * 2 + 2].copy_from_slice(&value.to_le_bytes());
        }
        assert_eq!(weapon_mount(&rom, 2, 0), Some((87, 80)));
        assert_eq!(weapon_mount(&rom, 2, 1), Some((64, 62)));
        assert_eq!(weapon_mount(&rom, 2, 2), Some((74, 73)));
        assert_eq!(weapon_mount(&rom, 3, 0), None);
        assert!(weapon_sprite(&rom, 9, 0).is_none());
    }

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
    fn reads_an_effect_sprite_and_its_multi_piece_frames() {
        let data = 0x0070_0000;
        let mut rom = vec![0; data + 0x400];
        let record = EFFECTS + 5 * EFFECT_LEN;
        let tiles = literal_block(2 * 32, 0x21);
        let palette = literal_block(32, 0x7F);
        let (tiles_at, palette_at) = (data, data + 0x100);
        let (animations, steps, frames, frame) =
            (data + 0x200, data + 0x210, data + 0x220, data + 0x240);
        rom[tiles_at..tiles_at + tiles.len()].copy_from_slice(&tiles);
        rom[palette_at..palette_at + palette.len()].copy_from_slice(&palette);
        put_pointer(&mut rom, record, tiles_at);
        put_pointer(&mut rom, record + 4, palette_at);
        put_pointer(&mut rom, record + 8, animations);
        put_pointer(&mut rom, record + 12, frames);
        put_pointer(&mut rom, animations, steps);
        rom[steps..steps + 8].copy_from_slice(&[0, 0, 2, 0, 0xFF, 0xFF, 0, 0]);
        put_pointer(&mut rom, frames, frame);
        let piece = |tile: u16, x: i16, affine: u16| {
            let mut bytes = Vec::new();
            for half in [
                tile,
                0,
                u16::from_ne_bytes(x.to_ne_bytes()),
                0xFFFC,
                32,
                8,
                0x100,
                0x120,
                affine,
                0,
            ] {
                bytes.extend_from_slice(&half.to_le_bytes());
            }
            bytes
        };
        let mut pieces = piece(0, -60, 0xFF);
        pieces.extend(piece(4, -28, 0x300));
        pieces.extend_from_slice(&0xFFFFu16.to_le_bytes());
        rom[frame..frame + pieces.len()].copy_from_slice(&pieces);
        let sprite = effect_sprite(&rom, 5).expect("effect");
        assert_eq!(sprite.palette[0], 0x7F7F);
        assert_eq!(sprite.animation.len(), 1);
        assert_eq!(sprite.frames.len(), 1);
        assert_eq!(sprite.frames[0].len(), 2);
        assert_eq!(sprite.frames[0][1].tile, 4);
        assert_eq!(sprite.frames[0][1].x, -28);
        assert_eq!(sprite.frames[0][1].scale_y, 0x120);
        assert_eq!(sprite.frames[0][1].affine, 0x300);
        assert_eq!(effect_sprite(&rom, EFFECT_COUNT), None);
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
