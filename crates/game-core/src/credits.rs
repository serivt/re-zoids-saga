//! The staff roll that closes chapter 9 (`0x0800C430`, the task
//! `0x08043BA4` in slot 4): over the attack scenes' plains a red Liger
//! runs in place while the staff's lines rise from the bottom of the
//! letterboxed screen, group after group, until the last one stays; then
//! the screen fades out and the story goes on.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! roll's task (`0x08043BA4`), its setup (`0x080436D4`, `0x08043818`), the
//! attack scenes' view loader it borrows (`0x08043EAC` with scenery `0x11`),
//! the scenery's task (`0x08043FF8`), the sprite loader (`0x08049154`), the
//! lines' rise (`0x080439E0`), the waits and fades (`0x08043908`, the fade
//! task `0x08004034`) and the window (`0x08042020`); checked against a
//! reference emulator's run of the roll frame by frame. See
//! `docs/events.md`.

use extraction::saga_battle::{self, BattleImage, EffectSprite, StaffGroup};
use formats::tile::TILE_PIXELS;
use gba_runtime::ppu::{FullPalette, Palette, darken};
use platform::{Frame, Rgb};

use crate::combat::scene::{run_line_program, wrap};
use crate::data::GameData;

const WIDTH: usize = 240;
const HEIGHT: usize = 160;
/// The scenery image the roll shows (`0x08043EAC` with scenery `0x11`:
/// image `0x11 × 3 + 2`), whose colors fill the palette from 0: 256 by
/// 128 pixels, its tiles row after row in the layer's map
/// (`0x0842B788`).
const SCENERY: u8 = 0x11 * 3 + 2;
const SCENERY_WIDTH: usize = 256;
const SCENERY_TILES_WIDE: usize = SCENERY_WIDTH / TILE;
const TILE: usize = 8;
/// The layer's vertical scroll: the picture 16 lines down (`0x080436D4`).
const SCENERY_TOP: usize = 16;
/// How far the scenery's scroll moves a frame (`0x08043FF8`, which clears
/// the view's flag when it starts: as the party's side), 16.16.
const SCENERY_SPEED: i32 = -0x1000;
/// The window the roll is seen through (`0x08042020`): the lines from 16
/// to 143; the rest is black.
const WINDOW: std::ops::Range<usize> = 16..144;
/// The shot sprites of the running Liger and of the staff's lines
/// (`0x08049154` with `0x7E` and `0x7F`).
const LIGER_SPRITE: u8 = 0x7E;
const LINES_SPRITE: u8 = 0x7F;
/// The lines' sprite's animations: one per line of the roll.
const LINE_ANIMATIONS: usize = 85;
/// Where the Liger stands.
const LIGER_AT: (i32, i32) = (0xA0, 0x90);
/// Where a group's lines start (`0x080439E0`): the first 8 pixels left of
/// the rest, one under another from the screen's bottom edge.
const FIRST_LINE_X: i32 = 0x88;
const LINE_X: i32 = 0x90;
const LINES_START: i32 = 0xA0;
const LINE_SPACING: i32 = 0x10;
/// A line shows once it is above the screen's bottom edge and goes at its
/// top; the last group stops when its first line is here.
const SHOWN_BELOW: i32 = 0xA0;
const LAST_STOP: i32 = 0x40;
/// Where each line's black shadow is drawn from it.
const SHADOW: (i32, i32) = (1, 1);

/// About the screen line the scenery's rewrite of its scroll table reaches
/// the drawing at: the lines above show the frame before's. The roll's
/// frames are busy, so it comes late, and how late moves with the load.
const TABLE_WRITE_LINE: usize = 100;

/// The roll's timing, in frames from the one it starts in: its song
/// starts, the scenery starts moving and the Liger running, the fade in
/// shows its first level (31, a level less a frame down to 0), and the
/// first group starts rising. A group starts two frames after the last line
/// of the one before has gone.
const MUSIC_AT: u32 = 2;
const SCENERY_AT: u32 = 10;
const LIGER_FROM: u32 = 10;
const FADE_IN_AT: u32 = 16;
const ROLL_AT: u32 = 50;
const NEXT_GROUP_AFTER: u32 = 2;
/// The song the roll plays, and the one that silences it at the end.
const ROLL_SONG: u16 = 2;
const SILENCE: u16 = 0;
/// Frames from the last group's stop: the fade out shows its first level
/// (0, a level more a frame up to 31) after ten seconds and the fade's own
/// three frames of setup; the song stops three seconds after the fade is
/// over, and the event that called the roll goes on a frame later.
const FADE_OUT_AT: u32 = 603;
const SILENCE_AT: u32 = 817;
const OVER_AT: u32 = 818;
/// The fade's levels: 31 is black; the screen shows up to 16.
const BLACK: u8 = 31;
const SHOWN_LEVELS: u8 = 16;

/// A line of the roll on its way up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Line {
    animation: u8,
    x: i32,
    /// Its height, 16.16.
    y: i32,
    shown: bool,
    rising: bool,
}

/// Where the roll is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Before the first group or between two, until the frame the next
    /// one starts in.
    Waiting(u32),
    /// A group's lines rise.
    Rising,
    /// The last group stopped in this frame.
    Stopped(u32),
}

/// What the screen shows: the registers and sprites as the frame before
/// left them.
#[derive(Debug, Clone, Default)]
struct Shown {
    level: u8,
    liger_frame: Option<usize>,
    lines: Vec<Line>,
}

/// The staff roll.
pub struct Credits {
    /// The frame being run, from 0.
    frame: u32,
    phase: Phase,
    scenery: Option<BattleImage>,
    palette: FullPalette,
    liger: Option<EffectSprite>,
    lines_sprite: Option<EffectSprite>,
    groups: Vec<StaffGroup>,
    group: usize,
    lines: Vec<Line>,
    /// The scenery's horizontal scroll, 16.16, and its lines' own: the
    /// table the lines are drawn off by, and the frame before's.
    scroll: i32,
    lines_scroll: [i32; HEIGHT],
    wave_phase: u8,
    table: [i32; HEIGHT],
    previous_table: [i32; HEIGHT],
    /// The Liger's animation: its step and the frames left of it.
    liger_step: (usize, u32),
    level: u8,
    music: Vec<u16>,
    shown: Shown,
}

impl Credits {
    /// The roll, its graphics read from the ROM, run through its first
    /// frame.
    #[must_use]
    pub fn new(data: &GameData<'_>) -> Self {
        let rom = data.bytes();
        let picture = saga_battle::scenery_picture(rom, SCENERY);
        let mut colors = vec![0u16; 256];
        if let Some((image, start)) = &picture {
            for (slot, color) in colors.iter_mut().skip(*start).zip(&image.palette) {
                *slot = *color;
            }
        }
        let mut credits = Self::with(
            picture.map(|(image, _)| image),
            FullPalette::from_bgr555(&colors),
            (
                saga_battle::shot_sprite(rom, LIGER_SPRITE),
                saga_battle::shot_sprite_with(rom, LINES_SPRITE, LINE_ANIMATIONS),
            ),
            saga_battle::staff_roll(rom).unwrap_or_default(),
        );
        credits.run(rom);
        credits
    }

    /// The roll before its first frame, of these graphics and groups.
    fn with(
        scenery: Option<BattleImage>,
        palette: FullPalette,
        (liger, lines_sprite): (Option<EffectSprite>, Option<EffectSprite>),
        groups: Vec<StaffGroup>,
    ) -> Self {
        let liger_step = (0, step_ticks(liger.as_ref(), 0));
        Self {
            frame: 0,
            phase: Phase::Waiting(ROLL_AT),
            scenery,
            palette,
            liger,
            lines_sprite,
            groups,
            group: 0,
            lines: Vec::new(),
            scroll: 0,
            lines_scroll: [0; HEIGHT],
            wave_phase: 0,
            table: [0; HEIGHT],
            previous_table: [0; HEIGHT],
            liger_step,
            level: BLACK,
            music: Vec::new(),
            shown: Shown {
                level: BLACK,
                ..Shown::default()
            },
        }
    }

    /// Whether the roll is over and the event that called it goes on.
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self.phase, Phase::Stopped(at) if self.frame >= at + OVER_AT)
    }

    /// The songs asked for since the last call.
    pub fn take_music(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.music)
    }

    /// Keeps what the screen shows this frame.
    pub fn latch(&mut self) {
        self.shown = Shown {
            level: self.level,
            liger_frame: self.liger_frame(),
            lines: self.lines.clone(),
        };
    }

    /// Advances one frame.
    pub fn update(&mut self, rom: &[u8]) {
        self.frame += 1;
        self.run(rom);
    }

    fn run(&mut self, rom: &[u8]) {
        let frame = self.frame;
        self.previous_table = self.table;
        if frame == MUSIC_AT {
            self.music.push(ROLL_SONG);
        }
        if frame >= SCENERY_AT {
            self.move_scenery(rom);
        }
        if frame >= FADE_IN_AT {
            let into = u8::try_from(frame - FADE_IN_AT).unwrap_or(BLACK);
            self.level = BLACK.saturating_sub(into);
        }
        match self.phase {
            Phase::Waiting(at) if frame >= at => self.start_group(),
            Phase::Waiting(_) => {}
            Phase::Rising => self.rise(),
            Phase::Stopped(at) => self.end(frame - at),
        }
        if frame >= LIGER_FROM {
            self.animate_liger();
        }
    }

    /// A frame of the scenery's task (`0x08043FF8`): the layer's scroll,
    /// then the scenery's line routine and the table from both.
    fn move_scenery(&mut self, rom: &[u8]) {
        self.scroll = self.scroll.wrapping_add(SCENERY_SPEED);
        run_line_program(rom, SCENERY, &mut self.lines_scroll, &mut self.wave_phase);
        for (entry, line) in self.table.iter_mut().zip(&self.lines_scroll) {
            *entry = self.scroll.wrapping_sub(*line);
        }
    }

    /// Sets up the current group's lines below the screen and moves them
    /// a first time (`0x080439E0`).
    fn start_group(&mut self) {
        let Some(group) = self.groups.get(self.group) else {
            return;
        };
        self.lines = group
            .lines
            .iter()
            .zip(0..)
            .map(|(&animation, index)| Line {
                animation,
                x: if index == 0 { FIRST_LINE_X } else { LINE_X },
                y: (LINES_START + LINE_SPACING * index) << 16,
                shown: false,
                rising: true,
            })
            .collect();
        self.phase = Phase::Rising;
        self.rise();
    }

    /// A frame of the lines' rise: each moves up by the group's speed,
    /// shows once above the bottom edge and goes at the top. The last group
    /// stops once its first line is at 64; the others end when their last
    /// line has gone.
    fn rise(&mut self) {
        let speed = self.groups.get(self.group).map_or(0, |group| group.speed);
        for line in self.lines.iter_mut().filter(|line| line.rising) {
            line.y -= speed;
            if line.y >> 16 < SHOWN_BELOW {
                line.shown = true;
            }
            if line.y >> 16 == 0 {
                line.shown = false;
                line.rising = false;
            }
        }
        let last = self.group + 1 >= self.groups.len();
        if last
            && self
                .lines
                .first()
                .is_some_and(|line| line.y == LAST_STOP << 16)
        {
            self.phase = Phase::Stopped(self.frame);
        } else if self.lines.iter().all(|line| !line.rising) {
            self.group += 1;
            self.phase = Phase::Waiting(self.frame + NEXT_GROUP_AFTER);
        }
    }

    /// The end, `since` frames after the last group stopped: the fade out,
    /// then the song silenced.
    fn end(&mut self, since: u32) {
        if since >= FADE_OUT_AT {
            let into = u8::try_from(since - FADE_OUT_AT).unwrap_or(BLACK);
            self.level = into.min(BLACK);
        }
        if since == SILENCE_AT {
            self.music.push(SILENCE);
        }
    }

    /// A frame of the Liger's looping animation (`0x08000AF8`).
    fn animate_liger(&mut self) {
        let Some(steps) = self.liger.as_ref().map(|sprite| sprite.animation.len()) else {
            return;
        };
        let (step, left) = self.liger_step;
        self.liger_step = if left > 1 {
            (step, left - 1)
        } else {
            let next = (step + 1) % steps.max(1);
            (next, step_ticks(self.liger.as_ref(), next))
        };
    }

    fn liger_frame(&self) -> Option<usize> {
        let sprite = self.liger.as_ref()?;
        sprite
            .animation
            .get(self.liger_step.0)
            .map(|step| step.frame)
    }

    /// Draws the roll: black but for the window, where the lines with
    /// their shadows and the Liger go over the scenery, all darkened by the
    /// fade.
    pub fn draw(&self, frame: &mut Frame) {
        frame.fill(Rgb::default());
        let shown = &self.shown;
        if shown.level >= SHOWN_LEVELS {
            return;
        }
        let sprites = self.sprite_layer();
        for y in WINDOW {
            let table = if y < TABLE_WRITE_LINE {
                &self.previous_table
            } else {
                &self.table
            };
            let scroll = table[y - 1] >> 16;
            for x in 0..WIDTH.min(frame.width()) {
                let color = sprites[y * WIDTH + x].unwrap_or_else(|| {
                    self.scenery.as_ref().map_or(Rgb::default(), |image| {
                        let index = scenery_pixel(
                            &image.tiles,
                            wrap(x, scroll, SCENERY_WIDTH),
                            y - SCENERY_TOP,
                        );
                        self.palette.color(index)
                    })
                });
                frame.set_pixel(x, y, color);
            }
        }
        darken(frame, shown.level);
    }

    /// The sprites' pixels, in the order of their slots: the lines, their
    /// shadows, the Liger.
    fn sprite_layer(&self) -> Vec<Option<Rgb>> {
        let shown = &self.shown;
        let mut layer = vec![None; WIDTH * HEIGHT];
        if let Some(sprite) = &self.lines_sprite {
            let text = Palette::new(sprite.palette.map(Palette::from_bgr555));
            let shadow = Palette::new([Rgb::default(); 16]);
            let visible = || shown.lines.iter().filter(|line| line.shown);
            let passes = visible()
                .map(|line| (line, &text, (0, 0)))
                .chain(visible().map(|line| (line, &shadow, SHADOW)));
            for (line, palette, (dx, dy)) in passes {
                let pieces = sprite
                    .animations
                    .get(usize::from(line.animation))
                    .and_then(|steps| steps.first())
                    .and_then(|step| sprite.frames.get(step.frame));
                for piece in pieces.into_iter().flatten() {
                    let anchor = (line.x + dx, (line.y >> 16) + dy);
                    crate::battle::draw_piece(&mut layer, sprite, palette, piece, anchor, false);
                }
            }
        }
        if let (Some(sprite), Some(index)) = (&self.liger, shown.liger_frame) {
            let palette = Palette::new(sprite.palette.map(Palette::from_bgr555));
            for piece in sprite.frames.get(index).into_iter().flatten() {
                crate::battle::draw_piece(&mut layer, sprite, &palette, piece, LIGER_AT, false);
            }
        }
        layer
    }
}

/// The palette index at `(x, y)` of the scenery's picture.
fn scenery_pixel(tiles: &[[u8; TILE_PIXELS]], x: usize, y: usize) -> u8 {
    let tile = (y / TILE) * SCENERY_TILES_WIDE + x / TILE;
    tiles
        .get(tile)
        .map_or(0, |pixels| pixels[(y % TILE) * TILE + x % TILE])
}

/// The frames step `step` of `sprite`'s first animation lasts.
fn step_ticks(sprite: Option<&EffectSprite>, step: usize) -> u32 {
    sprite
        .and_then(|sprite| sprite.animation.get(step))
        .map_or(1, |step| step.duration.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(speed: i32, lines: &[u8]) -> StaffGroup {
        StaffGroup {
            speed,
            lines: lines.to_vec(),
        }
    }

    /// Two groups rising 16 pixels a frame: the first starts at frame 50
    /// and is gone at 60 (its second line from 176 down to 0), the second
    /// starts at 62 and stops at 67 with its first line at 64.
    fn run_through() -> (Credits, Vec<(u32, u16)>) {
        let mut credits = Credits::with(
            None,
            FullPalette::from_bgr555(&[0]),
            (None, None),
            vec![group(0x10_0000, &[1, 2]), group(0x10_0000, &[3])],
        );
        credits.run(&[]);
        let mut songs = Vec::new();
        while !credits.is_done() {
            credits.update(&[]);
            songs.extend(
                credits
                    .take_music()
                    .into_iter()
                    .map(|song| (credits.frame, song)),
            );
        }
        (credits, songs)
    }

    #[test]
    fn the_last_group_stops_and_the_roll_ends_after_the_fade_out() {
        let (credits, songs) = run_through();
        assert_eq!(credits.phase, Phase::Stopped(67));
        assert_eq!(
            songs,
            vec![(MUSIC_AT, ROLL_SONG), (67 + SILENCE_AT, SILENCE)]
        );
        assert_eq!(credits.frame, 67 + OVER_AT);
        assert_eq!(credits.level, BLACK);
    }

    #[test]
    fn a_group_starts_two_frames_after_the_last_one_is_gone() {
        let mut credits = Credits::with(
            None,
            FullPalette::from_bgr555(&[0]),
            (None, None),
            vec![group(0x10_0000, &[1, 2]), group(0x10_0000, &[3])],
        );
        credits.run(&[]);
        while credits.frame < 61 {
            credits.update(&[]);
        }
        assert_eq!(credits.phase, Phase::Waiting(62));
        credits.update(&[]);
        assert_eq!(credits.phase, Phase::Rising);
        assert_eq!(credits.lines.len(), 1);
        assert_eq!(credits.lines[0].y, (LINES_START - 16) << 16);
        assert!(credits.lines[0].shown);
    }
}
