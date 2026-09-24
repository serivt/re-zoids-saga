//! The battle scenes cutscenes stage (`0x08008E4C`): an enemy Zoid rolls
//! in over the battle scenery, its pilot's portrait and quote appear, and
//! it fires.
//!
//! Source of knowledge: per-frame traces of the two scenes of the opening
//! in a reference emulator (the scroll shadows at IWRAM `0x03004B9C`, the
//! per-scanline scroll table the `HBlank` handler at IWRAM `0x03005BB0`
//! reads from EWRAM `0x0200DAAC`, the brightness, OAM and the script calls
//! of the battle module) and the data described in
//! `extraction::saga_battle`; see `docs/battle.md`.
//!
//! The screen stacks BG2, the scenery (a 128×128 image repeated across a
//! 256×256 map, mirrored), BG1, the Zoid (the same kind of image on a
//! 512×256 map), BG0 with two light-framed windows, the portrait's at
//! tile (0, 14) 6×6 and the message's at (6, 16) 24×4, and the pilot's
//! 48×48 portrait as sprites at (0, 112) over the first. Both images are
//! drawn mirrored, as the map's entries flip every tile. The scenery
//! scrolls per scanline: the sky by 1/16 pixel a frame, the forest by 1/4,
//! and the ground faster on each line.
//!
//! Each shot spawns effect sprites (see `extraction::saga_battle`): a
//! muzzle flash, a ring and a round, or the scene's own kinds. Their
//! positions are on BG1, so they follow the Zoid's recoil; they are drawn
//! mirrored like the Zoid, semi-transparent (15/16 of the sprite over 8/16
//! of the layer below), under the windows, and like BG1 a frame late. Each
//! shot plays its sound effect in the frame its flash spawns, as the battle
//! module's calls to `0x080019EC` do.

use extraction::saga::Portrait;
use extraction::saga_battle::{BattleImage, BattleScene, EffectPiece, EffectSprite};
use formats::tile::TILE_PIXELS;
use gba_runtime::ppu::{FullPalette, Palette, darken};
use platform::{Frame, Input, Rgb};

use crate::data::GameData;
use crate::script::{ScriptError, ScriptRunner};
use crate::translation::BATTLE_TABLE;
use crate::windows::ScriptWindows;
use crate::{ScriptHost, TextPainter, WindowPainter, draw_sprite};

const WIDTH: usize = 240;
const HEIGHT: usize = 160;
const TILE: usize = 8;
const IMAGE_TILES_SIDE: usize = 16;
const SCENERY_MAP_WIDTH: usize = 256;
const ZOID_MAP_WIDTH: usize = 512;
const ZOID_COLORS: usize = 0;
const SCENERY_COLORS: usize = 64;
const PORTRAIT_AT: (i32, i32) = (0, 112);
const PORTRAIT_WINDOW: u8 = 0;
const PORTRAIT_RECT: (u8, u8, u8, u8) = (0, 14, 6, 6);
const MESSAGE_WINDOW: u8 = 1;
const MESSAGE_RECT: (u8, u8, u8, u8) = (6, 16, 24, 4);
const LIGHT_FRAME: u8 = 0x20;
const PLAIN: u8 = 0;
const TYPEWRITER: u8 = 1;
const BLACK: u8 = 16;
const FADE_STEP: u32 = 2;
const FADE_FRAMES: u32 = 8;
const FADE_IN_START: u32 = 20;
const SLIDE_START: u32 = 29;
const SLIDE_FROM: i32 = 176;
/// Frames the slide takes to reach the scroll's end.
const SLIDE_DONE: u32 = 64;
const PARALLAX_START: u32 = 11;
const PARALLAX_OFFSET: i32 = -0x2000;
/// The screen line the battle module's rewrite of the scroll table
/// reaches the drawing at, when the frame is not loaded.
const TABLE_WRITE_LINE: usize = 16;
const QUOTE_START: u32 = 159;
const RECOIL_STEPS: [(u32, i32); 3] = [(0, 4), (2, 2), (4, 1)];
const PLAIN_PIECE: u16 = 0xFF;
const DOUBLE_SIZE: u16 = 0x200;
const FLIP_X: u16 = 1;
const FLIP_Y: u16 = 2;
const ROTATION: u16 = 0xFF00;
const TILE_INDEX: u16 = 0x3FF;

/// How one of the scenes unfolds, as measured frame by frame: the frames
/// are counted from the one the scene first shows.
struct Timeline {
    /// Frames the scene's own work runs behind the first scene's (the
    /// battle module's CPU load).
    lag: u32,
    /// The frames each shot pushes the Zoid back.
    recoils: &'static [u32],
    /// Frames the battle module ran long and left the scenery's scroll
    /// table as it was (it loads the shot's graphics).
    stalls: &'static [u32],
    /// The effect sprites the shots spawn.
    spawns: &'static [Spawn],
    /// The frame the fade to black starts.
    fade_out: u32,
    /// The frame the scene hands the screen back.
    end: u32,
}

/// An effect sprite a shot spawns, as the entity table showed it.
struct Spawn {
    /// The frame it appears in the entity table.
    at: u32,
    /// Its sprite in the effects table.
    sprite: u16,
    /// Its anchor on BG1.
    position: (i32, i32),
    /// How a round flies; `None` for a sprite that stays until its
    /// animation ends.
    flight: Option<Flight>,
    /// The sound effect the shot plays as the sprite appears.
    sound: Option<u8>,
}

/// A round flying right along its line.
struct Flight {
    /// Frames it waits before moving.
    delay: u32,
    /// Frames it exists.
    lifetime: u32,
}

/// Pixels a round flies a frame.
const ROUND_SPEED: i32 = 24;

const fn spawn(at: u32, sprite: u16, position: (i32, i32)) -> Spawn {
    Spawn {
        at,
        sprite,
        position,
        flight: None,
        sound: None,
    }
}

/// A muzzle flash, which plays the shot's sound.
const fn flash(at: u32, sprite: u16, position: (i32, i32), sound: u8) -> Spawn {
    Spawn {
        at,
        sprite,
        position,
        flight: None,
        sound: Some(sound),
    }
}

const fn round(at: u32, sprite: u16, position: (i32, i32), delay: u32, lifetime: u32) -> Spawn {
    Spawn {
        at,
        sprite,
        position,
        flight: Some(Flight { delay, lifetime }),
        sound: None,
    }
}

const MUZZLE: u16 = 188;
const RING: u16 = 189;
const BULLET: u16 = 167;
const HORN_MUZZLE: u16 = 183;
const HORN_ROUND: u16 = 156;
const WOLF_SHOT_SOUND: u8 = 123;
const HORN_SHOT_SOUND: u8 = 89;

/// The Command Wolf's four shots: a flash with sound effect 123, a ring a
/// frame later and a bullet a frame after that, from two barrels.
const WOLF_SHOTS: [Spawn; 12] = [
    flash(180, MUZZLE, (91, 55), WOLF_SHOT_SOUND),
    spawn(181, RING, (91, 55)),
    round(182, BULLET, (91, 55), 0, 9),
    flash(191, MUZZLE, (91, 66), WOLF_SHOT_SOUND),
    spawn(192, RING, (91, 66)),
    round(193, BULLET, (91, 66), 0, 9),
    flash(216, MUZZLE, (69, 54), WOLF_SHOT_SOUND),
    spawn(217, RING, (69, 54)),
    round(218, BULLET, (69, 54), 0, 9),
    flash(227, MUZZLE, (69, 65), WOLF_SHOT_SOUND),
    spawn(228, RING, (69, 65)),
    round(229, BULLET, (69, 65), 0, 9),
];

/// The Red Horn's ten shots: a flash with sound effect 89 and, a frame
/// later, a round that waits a frame before flying, the barrel moving
/// between them.
const HORN_SHOTS: [Spawn; 20] = [
    flash(186, HORN_MUZZLE, (107, 55), HORN_SHOT_SOUND),
    round(187, HORN_ROUND, (109, 50), 1, 10),
    flash(198, HORN_MUZZLE, (104, 55), HORN_SHOT_SOUND),
    round(199, HORN_ROUND, (106, 50), 1, 10),
    flash(210, HORN_MUZZLE, (102, 58), HORN_SHOT_SOUND),
    round(211, HORN_ROUND, (104, 53), 1, 10),
    flash(221, HORN_MUZZLE, (104, 61), HORN_SHOT_SOUND),
    round(222, HORN_ROUND, (106, 56), 1, 10),
    flash(232, HORN_MUZZLE, (107, 61), HORN_SHOT_SOUND),
    round(233, HORN_ROUND, (109, 56), 1, 10),
    flash(243, HORN_MUZZLE, (109, 58), HORN_SHOT_SOUND),
    round(244, HORN_ROUND, (111, 53), 1, 10),
    flash(255, HORN_MUZZLE, (107, 55), HORN_SHOT_SOUND),
    round(256, HORN_ROUND, (109, 50), 1, 10),
    flash(267, HORN_MUZZLE, (104, 55), HORN_SHOT_SOUND),
    round(268, HORN_ROUND, (106, 50), 1, 10),
    flash(279, HORN_MUZZLE, (102, 58), HORN_SHOT_SOUND),
    round(280, HORN_ROUND, (104, 53), 1, 10),
    flash(290, HORN_MUZZLE, (104, 61), HORN_SHOT_SOUND),
    round(291, HORN_ROUND, (106, 56), 1, 10),
];

/// Frames the game spends reloading the map after a scene.
const RELOAD_FRAMES: u32 = 9;

const TIMELINES: [Timeline; 2] = [
    Timeline {
        lag: 0,
        recoils: &[181, 192, 217, 228],
        stalls: &[164],
        spawns: &WOLF_SHOTS,
        fade_out: 298,
        end: 310,
    },
    Timeline {
        lag: 1,
        recoils: &[],
        stalls: &[],
        spawns: &HORN_SHOTS,
        fade_out: 288,
        end: 300,
    },
];

/// A battle scene being shown.
pub struct BattleStage {
    scenery: BattleImage,
    zoid: BattleImage,
    portrait: Portrait,
    palette: FullPalette,
    quote: usize,
    timeline: &'static Timeline,
    runner: ScriptRunner,
    effects: Vec<(u16, EffectSprite)>,
    frame: u32,
    started: bool,
}

/// Why a scene could not be staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingScene(pub u8);

impl BattleStage {
    /// Reads scene `index` and its graphics.
    ///
    /// # Errors
    ///
    /// Returns [`MissingScene`] when the scene, its images or its timeline
    /// are missing.
    pub fn new(data: &GameData<'_>, index: u8) -> Result<Self, MissingScene> {
        let missing = MissingScene(index);
        let timeline = TIMELINES.get(usize::from(index)).ok_or(missing)?;
        let BattleScene {
            scenery,
            zoid,
            pilot,
            quote,
        } = data.battle_scene(usize::from(index)).ok_or(missing)?;
        let scenery = data.scenery_image(scenery).ok_or(missing)?;
        let zoid = data.zoid_image(zoid).ok_or(missing)?;
        let portrait = data.portrait(usize::from(pilot), 0).map_err(|_| missing)?;
        let mut palette = FullPalette::from_bgr555(&[0]);
        palette.write(ZOID_COLORS, &zoid.palette);
        palette.write(SCENERY_COLORS, &scenery.palette);
        let mut effects: Vec<(u16, EffectSprite)> = Vec::new();
        for spawn in timeline.spawns {
            if effects.iter().all(|(id, _)| *id != spawn.sprite) {
                let sprite = data.effect_sprite(spawn.sprite).ok_or(missing)?;
                effects.push((spawn.sprite, sprite));
            }
        }
        let strings = data
            .script_offsets(BATTLE_TABLE)
            .ok()
            .flatten()
            .unwrap_or_default();
        Ok(Self {
            scenery,
            zoid,
            portrait,
            palette,
            quote,
            timeline,
            runner: ScriptRunner::named(BATTLE_TABLE, strings),
            effects,
            frame: 0,
            started: false,
        })
    }

    /// Frames the event waits in all: the scene and the map's reload.
    #[must_use]
    pub fn frames(&self) -> u32 {
        self.timeline.end + RELOAD_FRAMES
    }

    /// Whether the scene and the reload after it are over.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.frame + 1 >= self.frames()
    }

    /// Whether the scene still shows, rather than the reload's black.
    #[must_use]
    pub fn is_showing(&self) -> bool {
        self.frame < self.timeline.end
    }

    /// Advances one frame: opens the message window, types the quote, and
    /// closes the window when the scene hands the screen back.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when the quote cannot run.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        if self.started {
            self.frame += 1;
        }
        self.started = true;
        if self.frame == 0 {
            windows.close_window(None);
            windows.open_window(PORTRAIT_WINDOW, LIGHT_FRAME, PORTRAIT_RECT, PLAIN);
            windows.open_window(MESSAGE_WINDOW, LIGHT_FRAME, MESSAGE_RECT, TYPEWRITER);
            windows.present(None);
        }
        if self.frame == QUOTE_START + self.timeline.lag {
            self.runner.select_window(MESSAGE_WINDOW);
            self.runner.start(self.quote)?;
        }
        if self.frame > QUOTE_START + self.timeline.lag && self.frame < self.timeline.end {
            self.runner.update(rom, input, windows)?;
        }
        for spawn in self
            .timeline
            .spawns
            .iter()
            .filter(|spawn| spawn.at == self.frame)
        {
            if let Some(sound) = spawn.sound {
                windows.play_sound(sound);
            }
        }
        if self.frame + 1 == self.timeline.end {
            windows.close_window(None);
        }
        Ok(())
    }

    /// The brightness shown: black, the fade in, normal, the fade out,
    /// black. Like BG1's scroll, the screen shows the level the module set
    /// the frame before, which the next `VBlank` applies.
    #[must_use]
    pub fn brightness(&self) -> u8 {
        brightness_at(self.frame.saturating_sub(1), self.timeline)
    }

    /// BG1's horizontal scroll shown: the Zoid slides in from the left,
    /// slowing as a parabola, then each shot pushes it back 4, 2 and 1
    /// pixels; the `VBlank` copy shows the frame before's value.
    #[must_use]
    pub fn zoid_scroll(&self) -> i32 {
        zoid_scroll_at(self.frame.saturating_sub(1), self.timeline)
    }

    /// BG2's horizontal scroll on screen line `line`: the `HBlank` handler
    /// writes the per-line table's entry for the line just drawn, so a line
    /// shows the entry of the line above it (line 0 shows entry 1, which
    /// the handler writes during `VBlank`). The module rewrites the table
    /// while the frame is drawn, about 16 lines in, so the lines above show
    /// the frame before's values.
    #[must_use]
    pub fn scenery_scroll(&self, line: usize) -> i32 {
        let entry = if line == 0 { 1 } else { line - 1 };
        let frame = if line < TABLE_WRITE_LINE {
            self.frame.saturating_sub(1)
        } else {
            self.frame
        };
        scenery_scroll_at(frame, self.timeline, entry)
    }

    /// Draws the scene: scenery, Zoid, message window and portrait, with
    /// the brightness; black once the scene has handed the screen back.
    pub fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        let zoid_scroll = self.zoid_scroll();
        for y in 0..HEIGHT.min(frame.height()) {
            let scenery_scroll = self.scenery_scroll(y);
            for x in 0..WIDTH.min(frame.width()) {
                let zoid = wrap(x, zoid_scroll, ZOID_MAP_WIDTH);
                let scenery = wrap(x, scenery_scroll, SCENERY_MAP_WIDTH);
                let index = match image_pixel(&self.zoid.tiles, zoid, y, false) {
                    0 => image_pixel(&self.scenery.tiles, scenery, y, true),
                    index => index,
                };
                frame.set_pixel(x, y, self.palette.color(index));
            }
        }
        self.draw_effects(frame);
        windows.draw(frame, skin, painter);
        draw_sprite(
            frame,
            PORTRAIT_AT.0,
            PORTRAIT_AT.1,
            &self.portrait.image,
            &self.portrait.palette,
            false,
        );
        darken(frame, self.brightness());
    }
}

fn brightness_at(frame: u32, timeline: &Timeline) -> u8 {
    let fade_in = FADE_IN_START + timeline.lag;
    let level = if frame < fade_in || frame >= timeline.end {
        u32::from(BLACK)
    } else if frame < fade_in + FADE_FRAMES {
        u32::from(BLACK) - FADE_STEP * (frame - fade_in + 1)
    } else if frame < timeline.fade_out {
        0
    } else {
        (FADE_STEP * (frame - timeline.fade_out + 1)).min(u32::from(BLACK))
    };
    u8::try_from(level).unwrap_or(BLACK)
}

fn scenery_scroll_at(frame: u32, timeline: &Timeline, entry: usize) -> i32 {
    let stalled = timeline
        .stalls
        .iter()
        .filter(|stall| **stall <= frame)
        .count();
    let advanced = frame
        .saturating_sub(PARALLAX_START + timeline.lag)
        .saturating_sub(u32::try_from(stalled).unwrap_or(0));
    let advanced = i32::try_from(advanced).unwrap_or(i32::MAX / 0x1_0000);
    (parallax_rate(entry).saturating_mul(advanced) + PARALLAX_OFFSET) >> 16
}

fn zoid_scroll_at(frame: u32, timeline: &Timeline) -> i32 {
    let slide = SLIDE_START + timeline.lag;
    let base = match frame.checked_sub(slide) {
        None => SLIDE_FROM,
        Some(t) => {
            let t = i32::try_from(t.min(SLIDE_DONE)).unwrap_or(0);
            (SLIDE_FROM * 8 - t * t).max(0) / 8
        }
    };
    let recoil: i32 = timeline
        .recoils
        .iter()
        .flat_map(|shot| {
            RECOIL_STEPS
                .iter()
                .map(move |(after, push)| (shot + after, *push))
        })
        .filter(|(at, _)| frame >= *at)
        .map(|(_, push)| push)
        .sum();
    base + recoil
}

impl BattleStage {
    /// Draws the effects the entity table held the frame before, blended
    /// over the layers: the first sprite drawn at a pixel covers the later
    /// ones, as lower OAM entries do.
    fn draw_effects(&self, frame: &mut Frame) {
        let Some(shown) = self.frame.checked_sub(1) else {
            return;
        };
        let scroll = zoid_scroll_at(shown, self.timeline);
        let mut layer: Vec<Option<Rgb>> = vec![None; WIDTH * HEIGHT];
        for spawn in self.timeline.spawns {
            let Some(age) = shown.checked_sub(spawn.at) else {
                continue;
            };
            let Some((_, sprite)) = self.effects.iter().find(|(id, _)| *id == spawn.sprite) else {
                continue;
            };
            let Some((x, frame_index)) = effect_state(spawn, sprite, age) else {
                continue;
            };
            let anchor = (x - scroll, spawn.position.1);
            if hidden(anchor) {
                continue;
            }
            let palette = Palette::new(sprite.palette.map(Palette::from_bgr555));
            for piece in sprite.frames.get(frame_index).into_iter().flatten() {
                draw_piece(&mut layer, sprite, &palette, piece, anchor);
            }
        }
        for (index, color) in layer.into_iter().enumerate() {
            let (x, y) = (index % WIDTH, index / WIDTH);
            if let (Some(color), Some(below)) = (color, frame.pixel(x, y)) {
                frame.set_pixel(x, y, blend(color, below));
            }
        }
    }
}

/// Where an effect is and which frame it shows `age` frames after it
/// spawned; `None` once it is gone. A sprite that stays shows its
/// animation, which starts a step in, and goes with its end; a round
/// keeps its last frame while it flies.
fn effect_state(spawn: &Spawn, sprite: &EffectSprite, age: u32) -> Option<(i32, usize)> {
    let elapsed = age + 1;
    let mut start = 0;
    let mut step = None;
    for candidate in &sprite.animation {
        if elapsed < start + candidate.duration {
            step = Some(candidate.frame);
            break;
        }
        start += candidate.duration;
    }
    match &spawn.flight {
        None => step.map(|frame| (spawn.position.0, frame)),
        Some(flight) if age < flight.lifetime => {
            let frame = step.or_else(|| sprite.animation.last().map(|last| last.frame))?;
            let flown = i32::try_from(age.saturating_sub(flight.delay)).unwrap_or(0);
            Some((spawn.position.0 + ROUND_SPEED * flown, frame))
        }
        Some(_) => None,
    }
}

/// Whether the sprite drawer culls an anchor this far off screen
/// (`0x080005CA`).
fn hidden((x, y): (i32, i32)) -> bool {
    const RIGHT: i32 = 0x140;
    const BOTTOM: i32 = 0xC0;
    (x + 0x38).rem_euclid(0x1_0000) > RIGHT || (y + 0x20).rem_euclid(0x1_0000) > BOTTOM
}

/// Rasterizes one piece of an effect, mirrored as the enemy's sprites are,
/// into `layer`, where the first sprite at a pixel wins.
fn draw_piece(
    layer: &mut [Option<Rgb>],
    sprite: &EffectSprite,
    palette: &Palette,
    piece: &EffectPiece,
    (anchor_x, anchor_y): (i32, i32),
) {
    let (width, height) = (i32::from(piece.width), i32::from(piece.height));
    let affine = piece.affine != PLAIN_PIECE;
    let double = affine && piece.affine & DOUBLE_SIZE != 0;
    let (box_width, box_height) = if double {
        (width * 2, height * 2)
    } else {
        (width, height)
    };
    let left = anchor_x - i32::from(piece.x) - width - if double { width } else { 0 };
    let top = anchor_y + i32::from(piece.y);
    let matrix = affine.then(|| piece_matrix(piece));
    for row in 0..box_height {
        for column in 0..box_width {
            let (x, y) = (left + column, top + row);
            let (Ok(sx), Ok(sy)) = (usize::try_from(x), usize::try_from(y)) else {
                continue;
            };
            if sx >= WIDTH || sy >= HEIGHT || layer[sy * WIDTH + sx].is_some() {
                continue;
            }
            let source = if let Some((pa, pb, pc, pd)) = matrix {
                let (dx, dy) = (column - box_width / 2, row - box_height / 2);
                (
                    ((pa * dx + pb * dy) >> 8) + width / 2,
                    ((pc * dx + pd * dy) >> 8) + height / 2,
                )
            } else {
                let mirrored = piece.attributes & FLIP_X == 0;
                let tx = if mirrored { width - 1 - column } else { column };
                let ty = if piece.attributes & FLIP_Y != 0 {
                    height - 1 - row
                } else {
                    row
                };
                (tx, ty)
            };
            if let Some(index) = piece_pixel(sprite, piece, source)
                && index != 0
            {
                layer[sy * WIDTH + sx] = Some(palette.color(index));
            }
        }
    }
}

/// The affine matrix the drawer sets for a piece (`ObjAffineSet` with the
/// reciprocal of each scale and the piece's rotation), in 8.8, with the
/// first entry negated for the enemy's mirroring.
fn piece_matrix(piece: &EffectPiece) -> (i32, i32, i32, i32) {
    let reciprocal = |scale: i16| 0x1_0000 / i32::from(scale.unsigned_abs()).max(1);
    let (sx, sy) = (reciprocal(piece.scale_x), reciprocal(piece.scale_y));
    let sy = if piece.scale_y < 0 { -sy } else { sy };
    let turn = (0x1_0000 - i32::from(piece.attributes & ROTATION)) & 0xFFFF;
    let angle = f64::from(turn) * std::f64::consts::TAU / 65536.0;
    let (cos, sin) = (angle.cos(), angle.sin());
    let mut pa = to_fixed(f64::from(sx) * cos);
    let pb = to_fixed(-f64::from(sx) * sin);
    let pc = to_fixed(f64::from(sy) * sin);
    let mut pd = to_fixed(f64::from(sy) * cos);
    pa = -pa;
    if piece.scale_x < 0 {
        pa = -pa;
    }
    if piece.scale_y < 0 {
        pd = -pd;
    }
    (pa, pb, pc, pd)
}

/// Rounds a matrix entry, bounded by the reciprocal of the smallest scale,
/// to an integer.
#[allow(clippy::cast_possible_truncation)]
fn to_fixed(value: f64) -> i32 {
    value
        .round()
        .clamp(-f64::from(0x1_0000), f64::from(0x1_0000)) as i32
}

/// The palette index at `(x, y)` of a piece's image: its tiles in rows of
/// `width / 8`, one-dimensional OBJ mapping.
fn piece_pixel(sprite: &EffectSprite, piece: &EffectPiece, (x, y): (i32, i32)) -> Option<u8> {
    let (width, height) = (i32::from(piece.width), i32::from(piece.height));
    if !(0..width).contains(&x) || !(0..height).contains(&y) {
        return None;
    }
    let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
    let columns = usize::from(piece.width) / TILE;
    let tile = usize::from(piece.tile & TILE_INDEX) + (y / TILE) * columns + x / TILE;
    sprite
        .tiles
        .tile(tile)
        .map(|pixels| pixels[(y % TILE) * TILE + x % TILE])
}

/// A semi-transparent sprite's pixel over the layer below it: 15/16 of the
/// sprite and 8/16 of the layer (`BLDALPHA` `0x080F`), per 5-bit channel.
fn blend(sprite: Rgb, below: Rgb) -> Rgb {
    let channel = |top: u8, bottom: u8| {
        let mixed = ((u16::from(top >> 3) * 15 + u16::from(bottom >> 3) * 8) >> 4).min(31);
        let five = u8::try_from(mixed).unwrap_or(31);
        five << 3 | five >> 2
    };
    Rgb::new(
        channel(sprite.r, below.r),
        channel(sprite.g, below.g),
        channel(sprite.b, below.b),
    )
}

/// The table's step per frame for entry `line`: the sky 1/16 pixel, the
/// forest 1/4, the ground from 13/32 on entry 113 up by 1/32 an entry to
/// 127, and the entries under the scenery like the sky.
fn parallax_rate(line: usize) -> i32 {
    const FOREST: usize = 72;
    const GROUND: usize = 113;
    const BELOW: usize = 128;
    match line {
        FOREST..GROUND => 0x4000,
        GROUND..BELOW => 0x6800 + i32::try_from(line - GROUND).unwrap_or(0) * 0x800,
        _ => 0x1000,
    }
}

fn wrap(x: usize, scroll: i32, width: usize) -> usize {
    let width = i64::try_from(width).unwrap_or(i64::MAX);
    let position = i64::try_from(x).unwrap_or(0) + i64::from(scroll);
    usize::try_from(position.rem_euclid(width)).unwrap_or(0)
}

/// The palette index at map pixel `(x, y)` of a battle image's map: its
/// 16×16 tiles in the top-left corner, every one mirrored and the columns
/// in reverse, and again beside it when the map `repeats` (the scenery's);
/// the rest of the map is tile 0.
fn image_pixel(tiles: &[[u8; TILE_PIXELS]], x: usize, y: usize, repeats: bool) -> u8 {
    let (column, row) = (x / TILE, y / TILE);
    let (tile, pixel_x) = if row < IMAGE_TILES_SIDE && (column < IMAGE_TILES_SIDE || repeats) {
        (
            row * IMAGE_TILES_SIDE + IMAGE_TILES_SIDE - 1 - column % IMAGE_TILES_SIDE,
            TILE - 1 - x % TILE,
        )
    } else {
        (0, x % TILE)
    };
    tiles
        .get(tile)
        .map_or(0, |pixels| pixels[(y % TILE) * TILE + pixel_x])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_scene_follows_the_measured_timeline() {
        let first = &TIMELINES[0];
        assert_eq!(brightness_at(19, first), 16);
        assert_eq!(brightness_at(20, first), 14);
        assert_eq!(brightness_at(27, first), 0);
        assert_eq!(brightness_at(298, first), 2);
        assert_eq!(brightness_at(305, first), 16);
        let scrolls: Vec<i32> = [29, 30, 34, 39, 49, 59, 66, 67]
            .iter()
            .map(|frame| zoid_scroll_at(*frame, first))
            .collect();
        assert_eq!(scrolls, [176, 175, 172, 163, 126, 63, 4, 0]);
        assert_eq!(zoid_scroll_at(181, first), 4);
        assert_eq!(zoid_scroll_at(185, first), 7);
        assert_eq!(zoid_scroll_at(240, first), 28);
        assert_eq!(zoid_scroll_at(240, &TIMELINES[1]), 0);
        assert_eq!(brightness_at(20, &TIMELINES[1]), 16);
    }

    #[test]
    fn the_scenery_scrolls_per_line_band() {
        assert_eq!(parallax_rate(0), 0x1000);
        assert_eq!(parallax_rate(72), 0x4000);
        assert_eq!(parallax_rate(113), 0x6800);
        assert_eq!(parallax_rate(127), 0xD800);
        assert_eq!(parallax_rate(150), 0x1000);
        assert_eq!(wrap(10, -20, 256), 246);
        let first = &TIMELINES[0];
        assert_eq!(scenery_scroll_at(11, first, 72), -1);
        assert_eq!(scenery_scroll_at(19, first, 72), 1);
        assert_eq!(scenery_scroll_at(163, first, 72), 37);
        assert_eq!(scenery_scroll_at(164, first, 72), 37);
        assert_eq!(scenery_scroll_at(165, first, 72), 38);
    }

    fn effect(durations: &[u32]) -> EffectSprite {
        use extraction::saga::AnimationStep;
        use formats::tile::Tileset;
        EffectSprite {
            tiles: Tileset::from_pixels(vec![[1; TILE_PIXELS]; 4]),
            palette: [0x7FFF; 16],
            frames: vec![Vec::new(); durations.len()],
            animation: durations
                .iter()
                .enumerate()
                .map(|(frame, duration)| AnimationStep {
                    frame,
                    duration: *duration,
                })
                .collect(),
        }
    }

    #[test]
    fn effects_play_their_animation_or_fly_for_their_lifetime() {
        let muzzle = effect(&[2, 2, 16]);
        let stay = flash(10, MUZZLE, (91, 55), WOLF_SHOT_SOUND);
        assert_eq!(effect_state(&stay, &muzzle, 0), Some((91, 0)));
        assert_eq!(effect_state(&stay, &muzzle, 1), Some((91, 1)));
        assert_eq!(effect_state(&stay, &muzzle, 3), Some((91, 2)));
        assert_eq!(effect_state(&stay, &muzzle, 18), Some((91, 2)));
        assert_eq!(effect_state(&stay, &muzzle, 19), None);
        let bullet = effect(&[1]);
        let flying = round(10, BULLET, (91, 55), 1, 10);
        assert_eq!(effect_state(&flying, &bullet, 0), Some((91, 0)));
        assert_eq!(effect_state(&flying, &bullet, 1), Some((91, 0)));
        assert_eq!(effect_state(&flying, &bullet, 3), Some((91 + 48, 0)));
        assert_eq!(effect_state(&flying, &bullet, 10), None);
        let shots = TIMELINES[0]
            .spawns
            .iter()
            .filter_map(|spawn| spawn.sound.map(|sound| (spawn.at, sound)));
        assert_eq!(
            shots.collect::<Vec<_>>(),
            [(180, 123), (191, 123), (216, 123), (227, 123)]
        );
        assert_eq!(
            TIMELINES[1]
                .spawns
                .iter()
                .filter(|spawn| spawn.sound == Some(89))
                .count(),
            10
        );
    }

    #[test]
    fn effects_are_culled_blended_and_scaled_like_the_hardware_draws_them() {
        assert!(!hidden((91, 55)));
        assert!(hidden((283, 55)));
        assert!(hidden((91, 170)));
        let white = Rgb::new(0xFF, 0xFF, 0xFF);
        let black = Rgb::new(0, 0, 0);
        assert_eq!(blend(white, black).r, 239);
        assert_eq!(blend(black, white).r, 123);
        let piece = EffectPiece {
            tile: 0,
            attributes: 0,
            x: -16,
            y: 0,
            width: 16,
            height: 16,
            scale_x: 0x200,
            scale_y: 0x100,
            affine: 0x300,
        };
        assert_eq!(piece_matrix(&piece), (-128, 0, 0, 256));
        let sprite = effect(&[1]);
        assert_eq!(piece_pixel(&sprite, &piece, (15, 15)), Some(1));
        assert_eq!(piece_pixel(&sprite, &piece, (16, 0)), None);
    }

    #[test]
    fn battle_images_are_drawn_mirrored() {
        let mut tiles = vec![[0u8; TILE_PIXELS]; IMAGE_TILES_SIDE * IMAGE_TILES_SIDE];
        tiles[15][7] = 9;
        tiles[0][0] = 3;
        assert_eq!(image_pixel(&tiles, 0, 0, false), 9);
        assert_eq!(image_pixel(&tiles, 7, 0, false), 0);
        assert_eq!(image_pixel(&tiles, 128, 0, true), 9);
        assert_eq!(image_pixel(&tiles, 128, 0, false), 3);
        assert_eq!(image_pixel(&tiles, 0, 128, true), 3);
    }
}
