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
//! | `0x11` | Scenery + 1: index of the backgrounds below |
//! | `0x14` | The enemy's Zoid |
//! | `0x15` | Its pilot (a character) |
//! | `0x16` | Its quote: string `172 + n` of the `battle` table |
//!
//! Battle images are 128×128 pixels of 256 8bpp tiles with 64 colors,
//! LZ77-compressed, loaded by the 12-byte entries (source, destination,
//! length) of the loader at `0x08001A54`: the scenery's by the entries at ROM
//! `0x6F6934` (tiles) and `0x6F6BBC` (palette), loaded to palette entries
//! 64–127; a Zoid's by the entries at ROM `0x6F8974`
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
const SCENERY_TILES: usize = 0x006F_6934;
const SCENERY_PALETTES: usize = 0x006F_6BBC;
/// The loader's entries (`0x08001A54`): source, destination and length.
const LOAD_ENTRY_LEN: usize = 12;
const ZOID_TILES: usize = 0x006F_8974;
const ZOID_PALETTES: usize = 0x006F_9100;
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
/// The most pieces a frame can have: the drawer (`0x08000560`) reads them
/// until the end marker, into OAM's 128 entries.
const PIECES_MAX: usize = 128;

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
    /// Its animations, the first [`EFFECT_ANIMATIONS`] of its table that
    /// the ROM has.
    pub animations: Vec<Vec<AnimationStep>>,
}

/// Animations an effect sprite's table is read for: the shots pick one of
/// the first four (`0x080485C4`), the aiming grid up to its 20th.
pub const EFFECT_ANIMATIONS: usize = 32;

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

/// Rack `rack`'s own vertical place on Zoid `zoid`'s picture, `-1` when it
/// takes the default: the attack scenes leave off a part whose place is 0
/// (`0x08041AA8`).
#[must_use]
pub fn weapon_mount_raw(rom: &[u8], zoid: u16, rack: usize) -> Option<i16> {
    let at = WEAPON_MOUNTS + usize::from(zoid) * MOUNT_LEN + MOUNT_Y + rack.min(MOUNTS - 1) * 2;
    let bytes = rom.get(at..at + 2)?;
    Some(i16::from_le_bytes([bytes[0], bytes[1]]))
}

/// The Zoids' 76-byte records (ROM `0x670210`), whose sprite the status
/// screens and the aiming grid show: its 64 plain tiles, palette,
/// animations and frames at `+0x30`.
const ZOID_RECORDS: usize = 0x0067_0210;
const ZOID_RECORD_LEN: usize = 0x4C;
const ZOID_ICON: usize = 0x30;
const ZOID_ICON_TILE_BYTES: usize = 64 * 32;

/// Zoid `zoid`'s icon, which the aiming grid shows in its cells
/// (`0x0804502C`).
#[must_use]
pub fn zoid_icon(rom: &[u8], zoid: u16) -> Option<EffectSprite> {
    let at = ZOID_RECORDS + usize::from(zoid) * ZOID_RECORD_LEN + ZOID_ICON;
    sprite_record(rom, at, Packing::Plain(ZOID_ICON_TILE_BYTES))
}

/// The slots to go to from each slot, up, down, left and right, the first
/// with a unit taken (ROM `0x66BA90`, `0x08032390`: for each slot four
/// pointers to lists ended by `0xFF`).
const NEIGHBOURS: usize = 0x0066_BA90;
const NEIGHBOUR_WAYS: usize = 4;
const SIDE_SLOTS: usize = 6;
const LIST_END: u8 = 0xFF;

/// The slots the item's marker tries from each slot, by way.
#[must_use]
pub fn neighbours(rom: &[u8]) -> Vec<Vec<Vec<u8>>> {
    (0..SIDE_SLOTS)
        .map(|slot| {
            (0..NEIGHBOUR_WAYS)
                .map(|way| {
                    let at = NEIGHBOURS + (slot * NEIGHBOUR_WAYS + way) * 4;
                    rom.get(at..at + 4)
                        .and_then(rom_offset)
                        .and_then(|start| rom.get(start..))
                        .map(|bytes| {
                            bytes
                                .iter()
                                .take_while(|&&slot| slot != LIST_END)
                                .take(SIDE_SLOTS)
                                .copied()
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect()
}

/// The figures the battle screen shows over the party's units with L
/// (`0x08031FB4`): 21 plain 4bpp tiles at ROM `0x3664EC`, the hit points'
/// digits 0–9, the energy's 10–19 and a slash, with the palette at ROM
/// `0x366238`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitLabels {
    /// The tiles.
    pub tiles: Tileset,
    /// The 16-color BGR555 palette.
    pub palette: [u16; 16],
}

const LABEL_TILES: usize = 0x0036_64EC;
const LABEL_TILE_COUNT: usize = 21;
const LABEL_PALETTE: usize = 0x0036_6238;
const TILE_BYTES: usize = 32;

/// The figures' tiles and palette.
#[must_use]
pub fn unit_labels(rom: &[u8]) -> Option<UnitLabels> {
    let tiles = rom.get(LABEL_TILES..LABEL_TILES + LABEL_TILE_COUNT * TILE_BYTES)?;
    let palette = parse_palette(rom.get(LABEL_PALETTE..LABEL_PALETTE + TILE_BYTES)?)?;
    Some(UnitLabels {
        tiles: Tileset::from_4bpp(tiles),
        palette,
    })
}

/// The battle screen's effects (ROM `0x66B99C`, 7 records like the
/// shots' but with a plain palette): the spark a hit unit shows is 6.
const SCREEN_EFFECTS: usize = 0x0066_B99C;
const SCREEN_EFFECT_COUNT: usize = 7;

/// Effect `id` of the battle screen's table (`0x08032208`).
#[must_use]
pub fn screen_effect(rom: &[u8], id: usize) -> Option<EffectSprite> {
    if id >= SCREEN_EFFECT_COUNT {
        return None;
    }
    sprite_record(rom, SCREEN_EFFECTS + id * EFFECT_LEN, Packing::Screen)
}

fn sprite_at(rom: &[u8], at: usize) -> Option<EffectSprite> {
    sprite_record(rom, at, Packing::Effect)
}

/// How a sprite record's tiles and palette are stored.
#[derive(Clone, Copy)]
enum Packing {
    /// Both LZ77-compressed: the effects and the weapons.
    Effect,
    /// Compressed tiles and a plain palette: the battle screen's effects.
    Screen,
    /// This many plain tile bytes and a plain palette: the Zoids' icons.
    Plain(usize),
}

fn sprite_record(rom: &[u8], at: usize, packing: Packing) -> Option<EffectSprite> {
    sprite_record_with(rom, at, packing, EFFECT_ANIMATIONS)
}

fn sprite_record_with(
    rom: &[u8],
    at: usize,
    packing: Packing,
    animations: usize,
) -> Option<EffectSprite> {
    let record = rom.get(at..at + EFFECT_LEN)?;
    let pointer = |at: usize| rom_offset(&record[at..at + 4]);
    let tile_bytes = match packing {
        Packing::Effect | Packing::Screen => lz77::decompress(rom.get(pointer(0)?..)?).ok()?.0,
        Packing::Plain(len) => rom.get(pointer(0)?..pointer(0)? + len)?.to_vec(),
    };
    let palette = if let Packing::Effect = packing {
        let (palette_bytes, _) = lz77::decompress(rom.get(pointer(4)?..)?).ok()?;
        parse_palette(&palette_bytes)?
    } else {
        parse_palette(rom.get(pointer(4)?..pointer(4)? + 32)?)?
    };
    sprite_parts(
        rom,
        &tile_bytes,
        palette,
        (pointer(8)?, animations),
        pointer(12)?,
    )
}

/// The battle screen's own sprites (0x2C-byte records at ROM `0x66B5F8`,
/// `0x08031B70`): record 0 the item's target marker, 1–6 the party slots'
/// figures. A record gives the palette and its size, the plain tiles and
/// their size, and the animation and frame tables.
const SCREEN_SPRITES: usize = 0x0066_B5F8;
const SCREEN_SPRITE_LEN: usize = 0x2C;
const SCREEN_SPRITE_COUNT: usize = 7;

/// Record `id` of the battle screen's own sprites.
#[must_use]
pub fn screen_sprite(rom: &[u8], id: usize) -> Option<EffectSprite> {
    if id >= SCREEN_SPRITE_COUNT {
        return None;
    }
    let at = SCREEN_SPRITES + id * SCREEN_SPRITE_LEN;
    let record = rom.get(at..at + SCREEN_SPRITE_LEN)?;
    let pointer = |at: usize| rom_offset(&record[at..at + 4]);
    let size =
        |at: usize| usize::try_from(u32::from_le_bytes(record[at..at + 4].try_into().ok()?)).ok();
    let palette = parse_palette(rom.get(pointer(0)?..pointer(0)? + 32)?)?;
    let tiles = rom.get(pointer(0xC)?..pointer(0xC)? + size(0x14)?)?;
    sprite_parts(
        rom,
        tiles,
        palette,
        (pointer(0x18)?, EFFECT_ANIMATIONS),
        pointer(0x1C)?,
    )
}

/// A sprite from its tiles, palette, and animation and frame tables.
fn sprite_parts(
    rom: &[u8],
    tile_bytes: &[u8],
    palette: [u16; 16],
    (table, count): (usize, usize),
    frame_table: usize,
) -> Option<EffectSprite> {
    let first_animation = rom_offset(rom.get(table..table + 4)?)?;
    let animation = read_steps(rom, first_animation)?;
    let mut animations = vec![animation.clone()];
    for index in 1..count {
        let at = table + index * 4;
        let Some(steps) = rom
            .get(at..at + 4)
            .and_then(rom_offset)
            .and_then(|start| read_steps(rom, start))
            .filter(|steps| !steps.is_empty())
        else {
            break;
        };
        animations.push(steps);
    }
    let frame_count = animation
        .iter()
        .map(|step| step.frame + 1)
        .max()
        .unwrap_or(0);
    let mut frames = (0..frame_count)
        .map(|index| {
            let at = frame_table + index * 4;
            read_pieces(rom, rom_offset(rom.get(at..at + 4)?)?)
        })
        .collect::<Option<Vec<_>>>()?;
    let wanted = animations
        .iter()
        .flatten()
        .map(|step| step.frame + 1)
        .max()
        .unwrap_or(0);
    for index in frame_count..wanted {
        let at = frame_table + index * 4;
        let pieces = rom
            .get(at..at + 4)
            .and_then(rom_offset)
            .and_then(|start| read_pieces(rom, start))
            .unwrap_or_default();
        frames.push(pieces);
    }
    Some(EffectSprite {
        tiles: Tileset::from_4bpp(tile_bytes),
        palette,
        frames,
        animation,
        animations,
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
        scenery: record[SCENERY_FIELD].wrapping_sub(1),
        zoid: record[ENEMY_FIELD],
        pilot: record[ENEMY_FIELD + 1],
        quote: QUOTE_BASE + usize::from(record[ENEMY_FIELD + 2]),
    })
}

/// One side of a staged scene's record, as the scene builder
/// (`0x0803DD54`) reads it: 20 bytes, the party's at `+0` and the enemy's
/// at `+0x14`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedSide {
    /// The Zoid, 0 for none.
    pub zoid: u8,
    /// The pilot.
    pub pilot: u8,
    /// The line it speaks: string `172 + n` of the `battle` table.
    pub quote: Option<usize>,
    /// The parts of its first three racks (four-byte entries at `+4`, the
    /// part at `+2`), `None` for an empty one.
    pub racks: [Option<u16>; 3],
    /// The part slot it fires (`+0x10`), `None` for a side that does not.
    pub weapon: Option<usize>,
    /// Its terrain (`+0x11`), which picks the scenery.
    pub terrain: u8,
}

/// A staged scene played as an attack: the side that fires and the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedAttack {
    /// The party's side.
    pub party: StagedSide,
    /// The enemy's side.
    pub enemy: StagedSide,
    /// Whether the other side stands to be shot at (`+0x29` is 0).
    pub target_stands: bool,
}

const STAGED_SIDE_LEN: usize = 0x14;
const STAGED_ALONE: usize = 0x29;
const NONE: u8 = 0xFF;

fn staged_side(bytes: &[u8]) -> StagedSide {
    let half = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    StagedSide {
        zoid: bytes[0],
        pilot: bytes[1],
        quote: (bytes[2] != NONE).then(|| QUOTE_BASE + usize::from(bytes[2])),
        racks: std::array::from_fn(|rack| {
            let part = half(4 + rack * 4 + 2);
            (part != 0xFFFF).then_some(part)
        }),
        weapon: (bytes[0x10] != NONE).then_some(usize::from(bytes[0x10])),
        terrain: bytes[0x11],
    }
}

/// Staged scene `index` read as an attack (`0x0803DD54`).
#[must_use]
pub fn staged_attack(rom: &[u8], index: usize) -> Option<StagedAttack> {
    let at = SCENES + index * SCENE_LEN;
    let record = rom.get(at..at + SCENE_LEN)?;
    Some(StagedAttack {
        party: staged_side(&record[..STAGED_SIDE_LEN]),
        enemy: staged_side(&record[STAGED_SIDE_LEN..2 * STAGED_SIDE_LEN]),
        target_stands: record[STAGED_ALONE] == 0,
    })
}

/// The scenery image `scenery`: kind × 3 + the Zoid's size class
/// (`0x08043D6C`).
#[must_use]
pub fn scenery_image(rom: &[u8], scenery: u8) -> Option<BattleImage> {
    let slot = usize::from(scenery) * LOAD_ENTRY_LEN;
    image(rom, SCENERY_TILES + slot, SCENERY_PALETTES + slot)
}

/// Zoid `zoid`'s battle image (`0x08044E98`).
#[must_use]
pub fn zoid_image(rom: &[u8], zoid: u8) -> Option<BattleImage> {
    let slot = usize::from(zoid) * LOAD_ENTRY_LEN;
    image(rom, ZOID_TILES + slot, ZOID_PALETTES + slot)
}

/// Scenery image `scenery` whole, as its loader entries give it
/// (`0x08043CE4`): all its tiles, and all its colors with the palette
/// index the first one goes to. The attack scenes' sceneries are 256
/// tiles with 64 colors from 64; the few that fill the palette from 0 are
/// 512 tiles, the staff roll's plains.
#[must_use]
pub fn scenery_picture(rom: &[u8], scenery: u8) -> Option<(BattleImage, usize)> {
    let slot = usize::from(scenery) * LOAD_ENTRY_LEN;
    let tiles = decompress_at(rom, SCENERY_TILES + slot)?
        .chunks_exact(TILE_PIXELS)
        .map(|chunk| {
            let mut tile = [0; TILE_PIXELS];
            tile.copy_from_slice(chunk);
            tile
        })
        .collect();
    let entry = SCENERY_PALETTES + slot;
    let destination = rom.get(entry + 4..entry + 8)?;
    let destination = u32::from_le_bytes(destination.try_into().ok()?);
    let start = usize::try_from(destination.checked_sub(PALETTE_RAM)? / 2).ok()?;
    let palette = decompress_at(rom, entry)?
        .chunks_exact(2)
        .take(PALETTE_COLORS.checked_sub(start)?)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Some((BattleImage { tiles, palette }, start))
}

const PALETTE_RAM: u32 = 0x0500_0000;
const PALETTE_COLORS: usize = 256;

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

/// Entry of the effects table the attack scenes' shot sprites start at:
/// their table at ROM `0x6F8174` is the effects table's tail.
pub const SHOT_SPRITES: usize = 154;
const FIRE_PARTS: usize = 0x006E_51BC;
const FIRE_PART_LEN: usize = 16;
const SHOT_ANIMATIONS: usize = 0x006D_718C;
const MISSED_ANIMATIONS: usize = 0x006E_8F64;
const SHOT_ANIMATION_LEN: usize = 0x110;
/// Sprites a shot animation spawns at most.
pub const SHOT_SPAWNS: usize = 18;
const SPAWN_X: usize = 0x12;
const SPAWN_Y: usize = 0x36;
const SPAWN_FLAGS: usize = 0x5C;
const SPAWN_PARAMETER: usize = 0xA4;
const SPAWN_BEHAVIOR: usize = 0xC8;
const SPAWN_TIMING: usize = 0xDA;
const SPAWN_SOUND: usize = 0xFE;
const SPREADS: usize = 0x006E_8A88;
const WAVE: usize = 0x006D_3B94;

/// Shot sprite `id` with its first `animations` animations read, for the
/// sprites that have more than [`EFFECT_ANIMATIONS`]: the staff roll's
/// lines are the animations of one.
#[must_use]
pub fn shot_sprite_with(rom: &[u8], id: u8, animations: usize) -> Option<EffectSprite> {
    let id = SHOT_SPRITES + usize::from(id);
    if id >= EFFECT_COUNT {
        return None;
    }
    sprite_record_with(rom, EFFECTS + id * EFFECT_LEN, Packing::Effect, animations)
}

/// A group of the staff roll's lines (the 0x1C-byte records at ROM
/// `0x6D3E58`, read by `0x080439E0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaffGroup {
    /// How far the lines rise a frame, 16.16.
    pub speed: i32,
    /// The lines: animations of the lines' sprite, top to bottom.
    pub lines: Vec<u8>,
}

const STAFF_ROLL: usize = 0x006D_3E58;
const STAFF_GROUP_LEN: usize = 0x1C;
const STAFF_LINES: usize = 8;
/// Lines a group holds at most.
pub const STAFF_GROUP_LINES: usize = 20;
const STAFF_END: u8 = 0xFF;

/// The staff roll's groups, up to the record whose first byte is `0xFF`.
#[must_use]
pub fn staff_roll(rom: &[u8]) -> Option<Vec<StaffGroup>> {
    let mut groups = Vec::new();
    for index in 0.. {
        let at = STAFF_ROLL + index * STAFF_GROUP_LEN;
        let record = rom.get(at..at + STAFF_GROUP_LEN)?;
        if record[0] == STAFF_END {
            return Some(groups);
        }
        let lines = record[STAFF_LINES..STAFF_LINES + STAFF_GROUP_LINES]
            .iter()
            .copied()
            .take_while(|&line| line != STAFF_END)
            .collect();
        groups.push(StaffGroup {
            speed: i32::from_le_bytes(record[4..8].try_into().ok()?),
            lines,
        });
    }
    None
}

/// The shot sprite `id` of an animation (`0x080483B4`).
#[must_use]
pub fn shot_sprite(rom: &[u8], id: u8) -> Option<EffectSprite> {
    effect_sprite(rom, SHOT_SPRITES + usize::from(id))
}

/// How a weapon part fires in the attack scenes: its 16-byte record at
/// ROM `0x6E51BC` (`0x080485C4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirePart {
    /// The shot animation the attacker's view plays.
    pub attacker: u8,
    /// The one the target's view plays.
    pub target: u8,
    /// The attacker's in the sky (scenery kind 7), when not 0.
    pub sky_attacker: u8,
    /// The target's in the sky, when not 0.
    pub sky_target: u8,
    /// The attacker's when the part sits on the back rack, when not 0.
    pub back: u8,
    /// Added to the rack's mount for the sprites placed on it.
    pub mount_offset: (u16, u16),
    /// Added again on the back rack.
    pub back_offset: (u16, u16),
}

/// Part `part`'s fire record.
#[must_use]
pub fn fire_part(rom: &[u8], part: u16) -> Option<FirePart> {
    let at = FIRE_PARTS + usize::from(part) * FIRE_PART_LEN;
    let record = rom.get(at..at + FIRE_PART_LEN)?;
    let half = |i: usize| u16::from_le_bytes([record[i], record[i + 1]]);
    Some(FirePart {
        attacker: record[0],
        target: record[1],
        sky_attacker: record[2],
        sky_target: record[3],
        back: record[4],
        mount_offset: (half(6), half(8)),
        back_offset: (half(10), half(12)),
    })
}

/// One sprite a shot animation spawns: an entry of the 0x110-byte records
/// at ROM `0x6D718C` (`0x6E8F64` for the target's view of a miss).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShotSpawn {
    /// The shot sprite; `b'.'` places it where the record says even on a
    /// rack.
    pub sprite: u8,
    /// Where it starts.
    pub x: u16,
    /// See [`ShotSpawn::x`].
    pub y: u16,
    /// How it is drawn and set off: bit 0 animates it, bit 1 loops the
    /// animation, bit 2 places it on the weapon's rack, bit 3 makes it
    /// semi-transparent, bit 4 starts it at once, bits 8–12 a step the
    /// next one waits for, bits 14 and 15 spread x and y, bits 16–19 a
    /// shake, bit 20 puts it behind the Zoid, and bit 31 shares the tiles
    /// of the sprite bits 26–30 count back to.
    pub flags: u32,
    /// A value its behavior reads.
    pub parameter: u16,
    /// Its behavior (the routines at ROM `0x6D4784`).
    pub behavior: u8,
    /// Frames it waits once set off (high byte), and its animation (low).
    pub timing: u16,
    /// The sound effect it plays as it appears, when not 0.
    pub sound: u8,
}

/// The sprites of shot animation `index`; `missed` reads the table of
/// the misses.
#[must_use]
pub fn shot_animation(rom: &[u8], index: u8, missed: bool) -> Option<Vec<Option<ShotSpawn>>> {
    let table = if missed {
        MISSED_ANIMATIONS
    } else {
        SHOT_ANIMATIONS
    };
    let at = table + usize::from(index) * SHOT_ANIMATION_LEN;
    let record = rom.get(at..at + SHOT_ANIMATION_LEN)?;
    let half = |i: usize| u16::from_le_bytes([record[i], record[i + 1]]);
    Some(
        (0..SHOT_SPAWNS)
            .map(|slot| {
                let sprite = record[slot];
                (sprite != 0).then(|| ShotSpawn {
                    sprite,
                    x: half(SPAWN_X + slot * 2),
                    y: half(SPAWN_Y + slot * 2),
                    flags: u32::from_le_bytes([
                        record[SPAWN_FLAGS + slot * 4],
                        record[SPAWN_FLAGS + slot * 4 + 1],
                        record[SPAWN_FLAGS + slot * 4 + 2],
                        record[SPAWN_FLAGS + slot * 4 + 3],
                    ]),
                    parameter: half(SPAWN_PARAMETER + slot * 2),
                    behavior: record[SPAWN_BEHAVIOR + slot],
                    timing: half(SPAWN_TIMING + slot * 2),
                    sound: record[SPAWN_SOUND + slot],
                })
            })
            .collect(),
    )
}

/// How far Zoid `zoid`'s shots spread (ROM `0x6E8A88`, 4 bytes a Zoid):
/// the horizontal and the vertical mode.
#[must_use]
pub fn shot_spread(rom: &[u8], zoid: u16) -> Option<(u8, u8)> {
    let at = SPREADS + usize::from(zoid) * 4;
    let record = rom.get(at..at + 2)?;
    Some((record[0], record[1]))
}

/// Entry `index` of the wave table at ROM `0x6D3B94` (256 signed
/// halves) the flying Zoids bob by (`0x08043038`).
#[must_use]
pub fn wave(rom: &[u8], index: u8) -> Option<i16> {
    let at = WAVE + usize::from(index) * 2;
    let bytes = rom.get(at..at + 2)?;
    Some(i16::from_le_bytes([bytes[0], bytes[1]]))
}

/// The screen shakes' tables (`0x08044B34` and the four after it): the
/// pixels the Zoid's layer moves each frame, by shake kind 1 to 5.
const SHAKES: [(usize, usize); 5] = [
    (0x006D_4168, 16),
    (0x006D_41E8, 16),
    (0x006D_4218, 0x98),
    (0x006D_4188, 48),
    (0x006D_4208, 8),
];

/// Shake kind `kind`'s steps, one a frame.
#[must_use]
pub fn shake_steps(rom: &[u8], kind: u8) -> Option<Vec<i16>> {
    let (at, count) = *SHAKES.get(usize::from(kind).checked_sub(1)?)?;
    let bytes = rom.get(at..at + count * 2)?;
    Some(
        bytes
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect(),
    )
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
        rom[scene + SCENERY_FIELD] = 9;
        rom[scene + ENEMY_FIELD..scene + ENEMY_FIELD + 3].copy_from_slice(&[0x49, 0x3A, 1]);
        let tiles = literal_block(IMAGE_TILES * TILE_PIXELS, 5);
        let palette = literal_block(IMAGE_COLORS * 2, 0x11);
        rom[data..data + tiles.len()].copy_from_slice(&tiles);
        let palette_at = data + tiles.len();
        rom[palette_at..palette_at + palette.len()].copy_from_slice(&palette);
        put_pointer(&mut rom, SCENERY_TILES + 8 * LOAD_ENTRY_LEN, data);
        put_pointer(&mut rom, SCENERY_PALETTES + 8 * LOAD_ENTRY_LEN, palette_at);
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
