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
//! 48×48 portrait as sprites at (0, 112) over the first. Both images are drawn mirrored, as the
//! map's entries flip every tile. The scenery scrolls per scanline: the
//! sky by 1/16 pixel a frame, the forest by 1/4, and the ground faster on
//! each line. The shots, their effects and the sounds are not modeled yet.

use extraction::saga::Portrait;
use extraction::saga_battle::{BattleImage, BattleScene};
use formats::tile::TILE_PIXELS;
use gba_runtime::ppu::{FullPalette, darken};
use platform::{Frame, Input};

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
const QUOTE_START: u32 = 159;
const RECOIL_STEPS: [(u32, i32); 3] = [(0, 4), (2, 2), (4, 1)];

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
    /// The frame the fade to black starts.
    fade_out: u32,
    /// The frame the scene hands the screen back.
    end: u32,
}

/// Frames the game spends reloading the map after a scene.
const RELOAD_FRAMES: u32 = 9;

const TIMELINES: [Timeline; 2] = [
    Timeline {
        lag: 0,
        recoils: &[181, 192, 217, 228],
        stalls: &[164],
        fade_out: 298,
        end: 310,
    },
    Timeline {
        lag: 1,
        recoils: &[],
        stalls: &[],
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
    /// the handler writes during `VBlank`).
    #[must_use]
    pub fn scenery_scroll(&self, line: usize) -> i32 {
        let entry = if line == 0 { 1 } else { line - 1 };
        scenery_scroll_at(self.frame, self.timeline, entry)
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
