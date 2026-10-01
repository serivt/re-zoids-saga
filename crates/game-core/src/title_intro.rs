//! The title's picture as the original builds it: the intro that opens
//! the plate, raises the picture line by line, lights the logo and the
//! subtitle, then PRESS START blinking under them; START during the intro
//! skips to the finished title, which fades in from black.
//!
//! The screen is kept the way the original's tasks leave it: a copy of the
//! four screen blocks, the scroll and brightness values the vertical blank
//! copies to the hardware, and the display registers the vertical-blank
//! callbacks set. Each frame runs the pending callback and those copies,
//! then the tasks' work for the frame; the picture is composed pixel by
//! pixel with the hardware's rules: layer priorities, the window, alpha
//! blending and the brightness decrease, per line where the original
//! changes it on each horizontal blank.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1). The
//! title's loader (`0x0800248C`) builds the plate, waits for the fade from
//! black, then starts the intro's tasks: `0x08002E48` (the strip's fade
//! through callbacks `0x08002DF8`, `0x08002E20`, `0x08002E34`),
//! `0x08002A30` (the plate's opening with the pieces at `0x663F60`, the
//! strip's columns, the copy `0x080029F0`, the rise, the subtitle
//! `0x080029A0` and its callbacks `0x08002D28`, `0x08002D9C`,
//! `0x08002DB8`, the sprites and `0x08002C3C`), `0x08002F38` (the
//! picture's map `0x08002E94`, its line tables and horizontal-blank
//! routine `0x080032AC` through `0x08003244`, `0x080032D0` and
//! `0x080032F8`, then the logo `0x08002ED8` with `0x08002C64`,
//! `0x08002CA4`, `0x08002CC0` and `0x08002CDC`) and `0x080027AC`, which
//! takes START: it stops the others, clears the effects (`0x0800292C`),
//! lays the finished maps with the strip at `0x6640D4` and fades in with
//! the fade task (`0x08004034`). Once the intro is over (bit 8 of
//! `0x0200E8A8`) the loader starts the blinking (`0x0800338C`) and reads
//! START for the menu; `0x0800331C` then stops the blinking and clears it.
//! Checked in a reference emulator frame by frame against the per-frame
//! registers, the map copy in IWRAM and screenshots of the intro, of START
//! during it and after it.

use extraction::saga::TitleGraphics;
use formats::tile::TILE_PIXELS;
use gba_runtime::ppu::Palette;
use platform::{Frame, Rgb};

const WIDTH: usize = 240;
const HEIGHT: usize = 160;
const MAP_COLUMNS: usize = 32;
const BLOCK: usize = 0x400;
const MAP_LEN: usize = 4 * BLOCK;
const BLANK: u16 = 0x100;
const TILE_SIZE: usize = 8;
const MAP_PIXELS: usize = MAP_COLUMNS * TILE_SIZE;
const TILE_BASE: u16 = 0x102;
const PICTURE_BASE: u16 = 0x115;
const PICTURE_BLANK: u16 = 0x80;
const PICTURE_COLUMNS: usize = 30;
const PICTURE_ROWS: (usize, usize) = (2, 18);
const PICTURE_MAP_ROWS: usize = 20;
const GLOW_LEFT: u16 = 0xC139;
const GLOW_RIGHT: u16 = 0xC175;
const GLOW_COLUMNS: usize = 10;
const GLOW_ROWS: usize = 6;
const STRIP_EDGE: u16 = 0xB502;
const STRIP_TILE: u16 = 0xB102;
const STRIP_BLOCK: u16 = 0xB10D;
const STRIP_BLOCK_STRIDE: usize = 11;
const STRIP_BLOCK_ROWS: usize = 4;
const STRIP_BOTTOM_ROW: usize = 5;
const LOGO_TILE: u16 = 0xD1B1;
const LOGO_COLUMNS: usize = 14;
const LOGO_ROWS: usize = 3;
const SUBTITLE_TILE: u16 = 0xE205;
const SUBTITLE_AT: (usize, usize) = (7, 4);
const SUBTITLE_SIZE: (usize, usize) = (14, 2);
const PRESS_START_TILE: u16 = 0xE221;
const PRESS_START_AT: (usize, usize) = (10, 9);
const PRESS_TILES: usize = 4;
const PRESS_START_TILES: usize = 9;
const HFLIP: u16 = 0x0400;
const VFLIP: u16 = 0x0800;

const BG0_ON: u16 = 0x0100;
const BG2_ON: u16 = 0x0400;
const BG3_ON: u16 = 0x0800;
const WINDOW_ON: u16 = 0x2000;
const LAYERS: u16 = 0x1F00;
const BG_8BPP: u16 = 0x0080;
const BG_CHARACTERS_AND_PRIORITY: u16 = 0xFF70;
const BLEND_ALPHA: u16 = 1;
const BLEND_DARKEN: u16 = 3;
const LEVELS: u16 = 16;
const LEVEL_MASK: u16 = 0x1F;
const BLACK: u16 = 0x1F;
/// The window's corners and its inside and outside layers (`0x08002D28`).
const WINDOW_X: (usize, usize) = (0, 0xEF);
const WINDOW_Y: (usize, usize) = (0x40, 0x58);
const WINDOW_IN: u16 = 0x1F3F;
const WINDOW_OUT: u16 = 0x1F1F;
const EFFECTS: u16 = 0x20;
const OBJ: usize = 4;
const BACKDROP: usize = 5;
const OBJ_PRIORITY: u16 = 1;

/// Frames from the loader's start (the 409th after power-on) to the fade
/// task's start, whose level reaches black's end three frames later.
const FADE_START: u32 = 4;
/// The loader's first frame shown by the port, when the logo is gone.
pub(crate) const TITLE_SHOWN: u32 = 26;
/// The intro's tasks' first frame, and the first START can skip it.
const TASKS: u32 = 69;
const SKIP_FROM: u32 = TASKS + 1;
/// The plate's opening: frames, and every how many a column of the strip
/// or a step of pieces goes down.
const OPENING_FRAMES: u32 = 36;
const STRIP_EVERY: u32 = 8;
const PIECES_EVERY: u32 = 4;
/// The strip's fade: levels from `STRIP_LEVEL` down to 1.
const STRIP_LEVEL: u16 = 15;
const COPY: u32 = TASKS + OPENING_FRAMES;
const HIDE_COPY: u32 = COPY + 15;
const PICTURE: u32 = HIDE_COPY + 1;
const RISE_STEPS: u32 = 32;
const RISE_EVERY: u32 = 4;
const SUBTITLE: u32 = PICTURE + RISE_STEPS * RISE_EVERY + 30;
const SUBTITLE_STEPS: u32 = 16;
const SPRITES: u32 = SUBTITLE + SUBTITLE_STEPS + 2;
/// The frame the intro is over.
const INTRO_END: u32 = SPRITES + 1;
/// The picture's lines: levels, and each line's wait by its distance from
/// the middle one.
const LINE_LEVEL: u16 = 15;
const MIDDLE_LINE: usize = 80;
/// PRESS START's cycle: a write every 15 frames, over 150.
const BLINK_EVERY: u32 = 15;
const BLINK_CYCLE: u32 = 150;
/// Frames after START is read that the skip turns the layers back on, the
/// fade's start, and when the title is ready.
const SKIP_LAYERS: u32 = 2;
const SKIP_READY: u32 = 36;
/// The frame the skip turns the layers back on, the loader is still busy:
/// the layers show from about this line down, and the brightness level
/// still holds what the emulator reads from the write-only register
/// (`0x0800292C` copies it). The line moves between 36 and 52 with the
/// frame's work.
const SKIP_FLASH_LINE: usize = 44;
const SKIP_FLASH_LEVEL: u16 = 13;

/// A sprite of the title: a row of consecutive OBJ tiles.
#[derive(Debug, Clone, Copy)]
struct TitleSprite {
    x: usize,
    y: usize,
    columns: usize,
    tile: usize,
    palette: usize,
}

const fn sprite(x: usize, y: usize, columns: usize, tile: usize, palette: usize) -> TitleSprite {
    TitleSprite {
        x,
        y,
        columns,
        tile,
        palette,
    }
}

/// The 46 sprites of the finished title as OAM lists them: the logo in
/// three rows of 32×8 blocks, the subtitle and two copyright lines.
const TITLE_SPRITES: [TitleSprite; 46] = [
    sprite(8, 36, 4, 0, 0),
    sprite(40, 36, 4, 4, 0),
    sprite(72, 36, 4, 8, 0),
    sprite(104, 36, 2, 12, 0),
    sprite(8, 44, 4, 14, 0),
    sprite(40, 44, 4, 18, 0),
    sprite(72, 44, 4, 22, 0),
    sprite(104, 44, 2, 26, 0),
    sprite(8, 52, 4, 28, 0),
    sprite(40, 52, 4, 32, 0),
    sprite(72, 52, 4, 36, 0),
    sprite(104, 52, 2, 40, 0),
    sprite(120, 36, 4, 42, 0),
    sprite(152, 36, 4, 46, 0),
    sprite(184, 36, 4, 50, 0),
    sprite(216, 36, 2, 54, 0),
    sprite(120, 44, 4, 56, 0),
    sprite(152, 44, 4, 60, 0),
    sprite(184, 44, 4, 64, 0),
    sprite(216, 44, 2, 68, 0),
    sprite(120, 52, 4, 70, 0),
    sprite(152, 52, 4, 74, 0),
    sprite(184, 52, 4, 78, 0),
    sprite(216, 52, 2, 82, 0),
    sprite(64, 68, 4, 84, 1),
    sprite(96, 68, 4, 88, 1),
    sprite(128, 68, 4, 92, 1),
    sprite(160, 68, 2, 96, 1),
    sprite(64, 76, 4, 98, 1),
    sprite(96, 76, 4, 102, 1),
    sprite(128, 76, 4, 106, 1),
    sprite(160, 76, 2, 110, 1),
    sprite(32, 144, 4, 112, 1),
    sprite(64, 144, 4, 116, 1),
    sprite(96, 144, 4, 120, 1),
    sprite(128, 144, 4, 124, 1),
    sprite(160, 144, 4, 128, 1),
    sprite(192, 144, 2, 132, 1),
    sprite(208, 144, 1, 134, 1),
    sprite(32, 152, 4, 135, 1),
    sprite(64, 152, 4, 139, 1),
    sprite(96, 152, 1, 143, 1),
    sprite(104, 152, 4, 144, 1),
    sprite(136, 152, 4, 148, 1),
    sprite(168, 152, 4, 152, 1),
    sprite(200, 152, 2, 156, 1),
];
/// OBJ tiles below this come from the title tiles from `OBJ_TILE_BASE`
/// on, the rest from the copyright's tiles.
const OBJ_TEXT_BASE: usize = 112;
const OBJ_TILE_BASE: usize = 175;

/// What the vertical blank copies: the map, the scroll of each layer, the
/// brightness level and whether the sprites show.
#[derive(Clone)]
struct Copied {
    map: Vec<u16>,
    scroll: [(i32, i32); 4],
    level: u16,
    sprites: bool,
}

/// The display registers.
#[derive(Clone, Copy)]
struct Registers {
    display: u16,
    backgrounds: [u16; 4],
    blend: u16,
    alpha: u16,
    window_in: u16,
    window_out: u16,
    /// Whether the horizontal blank sets each line's brightness level.
    lines: bool,
}

/// The callbacks the tasks leave for the vertical blank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Callback {
    /// `0x08002DF8`: the strip's layer on, darkened.
    ShowStrip,
    /// `0x08002E20`: the strip's level.
    StripLevel(u16),
    /// `0x08002E34`: the strip's fade over.
    EndStrip,
    /// `0x08002C50`: BG0 and BG2 off once the glow was copied to BG1.
    HideCopy,
    /// `0x08003244`: the picture on, darkened line by line.
    ShowPicture,
    /// `0x080032D0`: the other line table.
    FlipLines,
    /// `0x080032F8`: the lines' fade over.
    EndLines,
    /// `0x08002C64`: the logo on, blended in.
    ShowLogo,
    /// `0x08002CA4`: the logo's weight.
    LogoIn(u16),
    /// `0x08002CC0`: the picture's weight under the logo.
    LogoSolid(u16),
    /// `0x08002CDC`: blending off.
    BlendOff,
    /// `0x08002D28`: the window on the subtitle, blended in.
    ShowSubtitle,
    /// `0x08002D9C`: the subtitle's weight.
    SubtitleIn(u16),
    /// `0x08002DB8`: window and blending off.
    EndSubtitle,
    /// `0x08002C3C`: the logo's layer off, its sprites in its place.
    HideLogo,
    /// `0x0800292C`: every layer and effect off for the skip.
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The intro plays.
    Intro,
    /// START skipped it on this frame; the finished title fades in.
    Skipped(u32),
    /// The intro is over since this frame.
    Ready(u32),
}

/// The title's picture, frame by frame.
pub(crate) struct TitleVideo {
    graphics: TitleGraphics,
    colors: [u16; 256],
    sprite_layer: Vec<Option<u16>>,
    frame: u32,
    stage: Stage,
    shadow: Copied,
    shown: Copied,
    registers: Registers,
    callback: Option<Callback>,
    lines: [[u16; HEIGHT]; 2],
    active: usize,
    lines_end: Option<u32>,
    fade_start: u32,
    flash_line: Option<usize>,
    blinking: bool,
    clear_at: Option<u32>,
}

impl TitleVideo {
    /// The title as its loader leaves it, run to the frame the port shows
    /// it at, [`TITLE_SHOWN`].
    pub(crate) fn new(graphics: TitleGraphics) -> Self {
        let mut colors = [0; 256];
        for (color, value) in colors.iter_mut().zip(graphics.palettes.iter().flatten()) {
            *color = *value;
        }
        let sprite_layer = sprite_layer(&graphics);
        let mut video = Self {
            graphics,
            colors,
            sprite_layer,
            frame: 0,
            stage: Stage::Intro,
            shadow: Copied {
                map: vec![BLANK; MAP_LEN],
                scroll: [(0, 0); 4],
                level: BLACK,
                sprites: false,
            },
            shown: Copied {
                map: Vec::new(),
                scroll: [(0, 0); 4],
                level: BLACK,
                sprites: false,
            },
            registers: Registers {
                display: 0x1740,
                backgrounds: [0x0002, 0x0101, 0x0201, 0x0301],
                blend: 0x3FFF,
                alpha: 0,
                window_in: 0,
                window_out: 0,
                lines: false,
            },
            callback: None,
            lines: [[0; HEIGHT]; 2],
            active: 0,
            lines_end: None,
            fade_start: FADE_START,
            flash_line: None,
            blinking: false,
            clear_at: None,
        };
        video.load();
        video.shown = video.shadow.clone();
        while video.frame < TITLE_SHOWN {
            video.update(false);
        }
        video
    }

    /// Whether the title controller counts this frame without START: from
    /// the frame after the intro ended or the skip faded in, as it runs
    /// before the task that marks it.
    pub(crate) fn counts_idle(&self) -> bool {
        matches!(self.stage, Stage::Ready(end) if self.frame > end)
    }

    /// Stops PRESS START's blinking and clears its line the next frame, as
    /// `0x0800331C` does when START opens the menu or the attract demo
    /// begins.
    pub(crate) fn stop_blinking(&mut self) {
        self.blinking = false;
        self.clear_at = Some(self.frame + 1);
    }

    /// Whether START on the next frame goes to the menu: the loader reads
    /// it from the second frame after the intro's end.
    pub(crate) fn takes_start(&self) -> bool {
        matches!(self.stage, Stage::Ready(end) if self.frame + 1 >= end + 2)
    }

    /// Whether PRESS START has tiles on its line, which the menu's START
    /// clears.
    pub(crate) fn shows_press_start(&self) -> bool {
        let (column, row) = PRESS_START_AT;
        let line = BLOCK + row * MAP_COLUMNS + column;
        self.shadow.map[line..line + PRESS_START_TILES]
            .iter()
            .any(|entry| *entry != BLANK)
    }

    /// Advances one frame; `start` says START was pressed on it. Returns
    /// whether the menu took it; during the intro it skips it instead.
    pub(crate) fn update(&mut self, start: bool) -> bool {
        let taken = start && self.takes_start();
        self.frame += 1;
        let frame = self.frame;
        self.flash_line = None;
        if let Some(callback) = self.callback.take() {
            self.run(callback);
        }
        self.shown.clone_from(&self.shadow);
        match self.stage {
            Stage::Intro => {
                self.intro(frame);
                if start && frame >= SKIP_FROM {
                    self.stage = Stage::Skipped(frame);
                    self.callback = Some(Callback::Skip);
                }
            }
            Stage::Skipped(at) => self.skip(frame - at),
            Stage::Ready(_) => {}
        }
        self.fade(frame);
        self.blink(frame);
        if taken {
            self.stop_blinking();
        }
        taken
    }

    /// The loader's maps and scroll.
    fn load(&mut self) {
        let map = &mut self.shadow.map;
        block(map, BLOCK, (0, 0), (GLOW_COLUMNS, GLOW_ROWS), GLOW_LEFT);
        block(
            map,
            2 * BLOCK,
            (5, 0),
            (GLOW_COLUMNS, GLOW_ROWS),
            GLOW_RIGHT,
        );
        map[2 * BLOCK + 4] = STRIP_EDGE;
        map[2 * BLOCK + STRIP_BOTTOM_ROW * MAP_COLUMNS + 4] = STRIP_EDGE + 5;
        strip_block(map, 4, 7);
        self.shadow.scroll = [(-40, -56), (-40, -56), (-80, -56), (-76, -64)];
    }

    /// The intro's tasks' work on `frame`.
    fn intro(&mut self, frame: u32) {
        self.fade_strip(frame);
        if let Some(step) = frame
            .checked_sub(TASKS)
            .filter(|step| *step < OPENING_FRAMES)
        {
            self.open(step);
        }
        match frame {
            COPY => {
                for row in 0..GLOW_ROWS {
                    for column in 0..15 {
                        self.shadow.map[BLOCK + row * MAP_COLUMNS + 14 + column] =
                            self.shadow.map[2 * BLOCK + row * MAP_COLUMNS + column];
                    }
                }
            }
            HIDE_COPY => self.callback = Some(Callback::HideCopy),
            SUBTITLE => {
                block(
                    &mut self.shadow.map,
                    0,
                    SUBTITLE_AT,
                    SUBTITLE_SIZE,
                    SUBTITLE_TILE,
                );
                self.callback = Some(Callback::ShowSubtitle);
            }
            SPRITES => {
                self.shadow.sprites = true;
                self.callback = Some(Callback::HideLogo);
            }
            INTRO_END => self.stage = Stage::Ready(frame),
            _ => {}
        }
        let rising = frame
            .checked_sub(PICTURE)
            .is_some_and(|step| step % RISE_EVERY == 0 && step / RISE_EVERY < RISE_STEPS);
        if rising {
            self.shadow.scroll[1].1 += 1;
            self.shadow.scroll[3].1 += 1;
        }
        if let Some(step) = frame
            .checked_sub(SUBTITLE + 1)
            .filter(|step| *step < SUBTITLE_STEPS)
        {
            self.callback = Some(Callback::SubtitleIn(to_u16(step + 1)));
        } else if frame == SUBTITLE + SUBTITLE_STEPS + 1 {
            self.callback = Some(Callback::EndSubtitle);
        }
        self.picture(frame);
    }

    /// Task `0x08002E48`: the strip's layer fades in.
    fn fade_strip(&mut self, frame: u32) {
        let Some(step) = frame.checked_sub(TASKS) else {
            return;
        };
        let step = to_u16(step);
        self.callback = match step {
            0 => Some(Callback::ShowStrip),
            _ if step < STRIP_LEVEL => Some(Callback::StripLevel(STRIP_LEVEL - step)),
            STRIP_LEVEL => Some(Callback::EndStrip),
            _ => self.callback,
        };
    }

    /// Task `0x08002A30`'s opening, step `step`: the halves slide apart,
    /// the strip grows by a column each side every eighth frame, and the
    /// pieces between the halves change every fourth.
    fn open(&mut self, step: u32) {
        self.shadow.scroll[0].0 += 1;
        self.shadow.scroll[1].0 += 1;
        self.shadow.scroll[2].0 -= 1;
        if step % STRIP_EVERY == 0 {
            let grown = to_i32(step / STRIP_EVERY);
            for column in [3 - grown, grown + 7] {
                for row in 0..STRIP_BLOCK_ROWS {
                    let row_i32 = to_i32_usize(row);
                    let tile = i32::from(STRIP_BLOCK)
                        + column
                        + row_i32 * to_i32_usize(STRIP_BLOCK_STRIDE);
                    let index = to_i32_usize(3 * BLOCK + row * MAP_COLUMNS) + column;
                    if let Ok(index) = usize::try_from(index) {
                        self.shadow.map[index % MAP_LEN] = u16::try_from(tile).unwrap_or(BLANK);
                    }
                }
            }
        }
        if step % PIECES_EVERY == 0 {
            let pieces = self
                .graphics
                .intro_steps
                .get(usize::try_from(step / PIECES_EVERY).unwrap_or(usize::MAX))
                .cloned()
                .unwrap_or_default();
            for [place, tiles] in pieces {
                let layer = usize::from(place & 3);
                let index = layer * BLOCK + usize::from(place >> 2);
                let flip = if layer == 2 { HFLIP } else { 0 };
                self.shadow.map[index] = (STRIP_TILE + u16::from(tiles & 0xF)) | flip;
                self.shadow.map[index + STRIP_BOTTOM_ROW * MAP_COLUMNS] =
                    (STRIP_TILE + u16::from(tiles >> 4)) | flip;
            }
        }
    }

    /// Task `0x08002F38`: the picture rises line by line from the middle,
    /// then the logo blends in over it.
    fn picture(&mut self, frame: u32) {
        if frame == PICTURE {
            picture_map(&mut self.shadow.map);
        } else if frame == PICTURE + 1 {
            self.shadow.scroll[2] = (0, 0);
            for line in 0..HEIGHT {
                let distance = to_u16_usize(line.abs_diff(MIDDLE_LINE));
                self.lines[0][line] = (distance << 4) | LINE_LEVEL;
                self.lines[1][line] = (distance << 4) | LINE_LEVEL;
            }
            self.active = 0;
            self.callback = Some(Callback::ShowPicture);
        } else if frame > PICTURE + 1 && self.lines_end.is_none() {
            if self.step_lines() {
                self.callback = Some(Callback::FlipLines);
            } else {
                self.lines_end = Some(frame);
                self.callback = Some(Callback::EndLines);
            }
        } else if let Some(end) = self.lines_end {
            let step = frame - end;
            if step == 1 {
                self.shadow.map[..BLOCK].fill(BLANK);
                let map = &mut self.shadow.map;
                block(map, 0, (0, 0), (LOGO_COLUMNS, LOGO_ROWS), LOGO_TILE);
                let right = LOGO_TILE + to_u16_usize(LOGO_COLUMNS * LOGO_ROWS);
                block(map, 0, (LOGO_COLUMNS, 0), (LOGO_COLUMNS, LOGO_ROWS), right);
                self.shadow.scroll[0] = (-8, -36);
                self.callback = Some(Callback::ShowLogo);
            } else if (2..18).contains(&step) {
                self.callback = Some(Callback::LogoIn(to_u16(step - 1)));
            } else if (18..34).contains(&step) {
                self.callback = Some(Callback::LogoSolid(to_u16(step - 17)));
            } else if step == 34 {
                self.callback = Some(Callback::BlendOff);
            }
        }
    }

    /// One step of the line tables: the table not shown takes the shown
    /// one with each line's wait or, once it is over, its level lowered.
    /// Returns `false` once the last line's level is 0.
    fn step_lines(&mut self) -> bool {
        let next = self.active ^ 1;
        if line_level(self.lines[next][HEIGHT - 1]) == 0 {
            return false;
        }
        for line in 0..HEIGHT {
            let shown = self.lines[self.active][line];
            let entry = &mut self.lines[next][line];
            if shown & 0xFF0 == 0 {
                *entry = (*entry & 0xF00F) | (shown & 0xFF0);
                if shown & 0xF != 0 {
                    let high = if shown & 0x3000 == 0 {
                        *entry = (*entry & 0xFFF0) | ((shown & 0xF) - 1);
                        *entry >> 14
                    } else {
                        ((shown & 0x3FFF) >> 12).wrapping_sub(1)
                    };
                    *entry = (*entry & 0xCFFF) | ((high & 3) << 12);
                }
            } else {
                let wait = ((shown >> 4) & 0xFF).wrapping_sub(1) & 0xFF;
                *entry = (*entry & 0xF00F) | (wait << 4);
            }
        }
        true
    }

    /// Task `0x080027AC` after START, `step` frames on: the finished maps,
    /// then the layers back on.
    fn skip(&mut self, step: u32) {
        if step == 1 {
            let map = &mut self.shadow.map;
            block(map, BLOCK, (19, 0), (GLOW_COLUMNS, GLOW_ROWS), GLOW_RIGHT);
            for (row, entries) in [0, STRIP_BOTTOM_ROW]
                .into_iter()
                .zip(self.graphics.skip_strip)
            {
                for (column, entry) in entries.into_iter().enumerate() {
                    map[BLOCK + row * MAP_COLUMNS + 10 + column] = STRIP_TILE.wrapping_add(entry);
                }
            }
            strip_block(map, 0, 11);
            picture_map(map);
            self.shadow.scroll[1] = (-4, -24);
            self.shadow.scroll[3] = (-76, -32);
            self.shadow.scroll[2] = (0, 0);
            self.shadow.sprites = true;
            let background = &mut self.registers.backgrounds[2];
            *background = (*background & BG_CHARACTERS_AND_PRIORITY) | BG_8BPP | 2;
            self.fade_start = self.frame;
            self.registers.blend = 0x3FFF;
        } else if step == SKIP_LAYERS {
            self.registers.display |= 0x1E00;
            self.flash_line = Some(SKIP_FLASH_LINE);
        } else if step == SKIP_READY {
            self.stage = Stage::Ready(self.frame);
        }
    }

    /// The fade task (`0x08004034`) from black: the level holds black two
    /// frames, then drops by one a frame.
    fn fade(&mut self, frame: u32) {
        if let Some(step) = frame
            .checked_sub(self.fade_start + 1)
            .filter(|step| *step <= u32::from(BLACK) + 1)
        {
            self.shadow.level = BLACK - to_u16(step.saturating_sub(1));
        }
    }

    /// The loader's blinking: PRESS, then START, then nothing, twice; then
    /// both and nothing, twice; every 15 frames. START for the menu stops
    /// it and clears the line the frame after.
    fn blink(&mut self, frame: u32) {
        if let Stage::Ready(end) = self.stage {
            if frame == end + 1 {
                self.blinking = true;
            }
            let step = frame.saturating_sub(end + 1);
            if self.blinking && step % BLINK_EVERY == 0 {
                match (step % BLINK_CYCLE) / BLINK_EVERY {
                    0 | 3 => self.press_start(0, PRESS_TILES),
                    1 | 4 => self.press_start(PRESS_TILES, PRESS_START_TILES),
                    6 | 8 => self.press_start(0, PRESS_START_TILES),
                    _ => self.press_start(0, 0),
                }
            }
        }
        if self.clear_at == Some(frame) {
            self.press_start(0, 0);
        }
    }

    /// Writes PRESS START's tiles `from..to` on its line, or clears it.
    fn press_start(&mut self, from: usize, to: usize) {
        let (column, row) = PRESS_START_AT;
        let line = BLOCK + row * MAP_COLUMNS + column;
        if to == 0 {
            self.shadow.map[line..line + PRESS_START_TILES].fill(BLANK);
        }
        for tile in from..to {
            self.shadow.map[line + tile] = PRESS_START_TILE + to_u16_usize(tile);
        }
    }

    fn run(&mut self, callback: Callback) {
        let registers = &mut self.registers;
        match callback {
            Callback::ShowStrip => {
                registers.display |= BG3_ON;
                registers.blend = 0xC8;
                self.shadow.level = STRIP_LEVEL;
            }
            Callback::StripLevel(level) => self.shadow.level = level,
            Callback::EndStrip => {
                registers.blend = 0;
                self.shadow.level = 0;
            }
            Callback::HideCopy => registers.display &= !(BG0_ON | BG2_ON),
            Callback::ShowPicture => {
                let background = &mut registers.backgrounds[2];
                *background = (*background & BG_CHARACTERS_AND_PRIORITY) | BG_8BPP | 2;
                registers.display |= BG2_ON;
                registers.blend = 0xC4;
                self.shadow.level = LINE_LEVEL;
                registers.lines = true;
            }
            Callback::FlipLines => self.active ^= 1,
            Callback::EndLines => {
                registers.blend = 0;
                self.shadow.level = 0;
                registers.lines = false;
            }
            Callback::ShowLogo => {
                registers.blend = 0x3F41;
                registers.alpha = 0x1000;
                registers.backgrounds[0] &= BG_CHARACTERS_AND_PRIORITY;
                registers.display |= BG0_ON;
            }
            Callback::LogoIn(weight) => registers.alpha = weight | 0x1000,
            Callback::LogoSolid(step) => registers.alpha = ((LEVELS - step) << 8) | 0x10,
            Callback::BlendOff => {
                registers.alpha = 0;
                registers.blend = 0;
            }
            Callback::ShowSubtitle => {
                registers.window_in = WINDOW_IN;
                registers.window_out = WINDOW_OUT;
                registers.display |= WINDOW_ON;
                registers.blend = 0x3F41;
                registers.alpha = 0x1000;
            }
            Callback::SubtitleIn(weight) => registers.alpha = ((LEVELS - weight) << 8) | weight,
            Callback::EndSubtitle => {
                registers.alpha = 0;
                registers.blend = 0;
                registers.display &= !WINDOW_ON;
                registers.window_in = 0;
                registers.window_out = 0;
            }
            Callback::HideLogo => registers.display &= !BG0_ON,
            Callback::Skip => {
                registers.display &= !WINDOW_ON & !LAYERS;
                registers.window_in = 0;
                registers.window_out = 0;
                registers.blend = 0;
                registers.alpha = 0;
                registers.lines = false;
                self.shadow.level = SKIP_FLASH_LEVEL;
            }
        }
    }

    /// Composes the picture.
    pub(crate) fn draw(&self, frame: &mut Frame) {
        let registers = self.registers;
        let mut order = Vec::with_capacity(5);
        for priority in 0..4 {
            if registers.display & 0x1000 != 0 && priority == OBJ_PRIORITY && self.shown.sprites {
                order.push(OBJ);
            }
            for layer in 0..4 {
                if registers.display & (BG0_ON << layer) != 0
                    && registers.backgrounds[layer] & 3 == priority
                {
                    order.push(layer);
                }
            }
        }
        let mode = (registers.blend >> 6) & 3;
        let alpha = (
            (registers.alpha & LEVEL_MASK).min(LEVELS),
            ((registers.alpha >> 8) & LEVEL_MASK).min(LEVELS),
        );
        let backdrop = self.colors[0];
        for y in 0..HEIGHT.min(frame.height()) {
            let hidden = self.flash_line.is_some_and(|line| y < line);
            let level = if registers.lines {
                y.checked_sub(1)
                    .map_or(self.shown.level, |line| self.lines[self.active][line] & 0xF)
            } else {
                self.shown.level
            };
            let level = (level & LEVEL_MASK).min(LEVELS);
            for x in 0..WIDTH.min(frame.width()) {
                let mask = if registers.display & WINDOW_ON == 0 {
                    0x3F
                } else if (WINDOW_X.0..WINDOW_X.1).contains(&x)
                    && (WINDOW_Y.0..WINDOW_Y.1).contains(&y)
                {
                    registers.window_in & 0x3F
                } else {
                    registers.window_out & 0x3F
                };
                let mut found = [(BACKDROP, backdrop); 2];
                let mut count = 0;
                for &layer in order.iter().filter(|_| !hidden) {
                    if mask & (1 << layer) == 0 {
                        continue;
                    }
                    if let Some(color) = self.pixel(layer, x, y) {
                        found[count] = (layer, color);
                        count += 1;
                        if count == 2 {
                            break;
                        }
                    }
                }
                let (top, color) = found[0];
                let color = Palette::from_bgr555(color);
                let first = registers.blend & (1 << top) != 0 && mask & EFFECTS != 0;
                let color = match mode {
                    BLEND_ALPHA if first && registers.blend & (0x100 << found[1].0) != 0 => {
                        mix(color, Palette::from_bgr555(found[1].1), alpha)
                    }
                    BLEND_DARKEN if first => darken(color, level),
                    _ => color,
                };
                frame.set_pixel(x, y, color);
            }
        }
    }

    /// Layer `layer`'s color at `(x, y)`, or `None` where it is clear.
    fn pixel(&self, layer: usize, x: usize, y: usize) -> Option<u16> {
        if layer == OBJ {
            return self.sprite_layer[y * WIDTH + x];
        }
        let control = self.registers.backgrounds[layer];
        let (scroll_x, scroll_y) = self.shown.scroll[layer];
        let map_x = wrap(x, scroll_x);
        let map_y = wrap(y, scroll_y);
        let block = usize::from((control >> 8) & 3) * BLOCK;
        let entry = self.shown.map[block + (map_y / TILE_SIZE) * MAP_COLUMNS + map_x / TILE_SIZE];
        let mut column = map_x % TILE_SIZE;
        let mut row = map_y % TILE_SIZE;
        if entry & HFLIP != 0 {
            column = TILE_SIZE - 1 - column;
        }
        if entry & VFLIP != 0 {
            row = TILE_SIZE - 1 - row;
        }
        let tile = entry & 0x3FF;
        let index = if control & BG_8BPP == 0 {
            let pixels = tile
                .checked_sub(TILE_BASE)
                .and_then(|tile| self.graphics.tiles.tile(usize::from(tile)))?;
            let index = pixels[row * TILE_SIZE + column];
            if index == 0 {
                return None;
            }
            usize::from(entry >> 12) * 16 + usize::from(index)
        } else {
            let pixels = tile
                .checked_sub(PICTURE_BASE)
                .filter(|_| tile != PICTURE_BLANK)
                .and_then(|tile| self.graphics.picture.tile(usize::from(tile)))?;
            usize::from(pixels[row * TILE_SIZE + column])
        };
        (index != 0).then(|| self.colors[index])
    }
}

/// The sprites' layer: each pixel's color, or `None` where no sprite is.
fn sprite_layer(graphics: &TitleGraphics) -> Vec<Option<u16>> {
    let mut layer = vec![None; WIDTH * HEIGHT];
    let tile = |index: usize| -> Option<&[u8; TILE_PIXELS]> {
        if index < OBJ_TEXT_BASE {
            graphics.tiles.tile(OBJ_TILE_BASE + index)
        } else {
            graphics.text_tiles.tile(index - OBJ_TEXT_BASE)
        }
    };
    for sprite in TITLE_SPRITES.iter().rev() {
        let palette = graphics.sprite_palettes[sprite.palette];
        for column in 0..sprite.columns {
            let Some(pixels) = tile(sprite.tile + column) else {
                continue;
            };
            for (offset, index) in pixels.iter().enumerate() {
                let x = sprite.x + column * TILE_SIZE + offset % TILE_SIZE;
                let y = sprite.y + offset / TILE_SIZE;
                if *index != 0 && x < WIDTH && y < HEIGHT {
                    layer[y * WIDTH + x] = Some(palette[usize::from(*index & 0xF)]);
                }
            }
        }
    }
    layer
}

/// Writes `size` cells of consecutive tiles from `first` at `at` in the
/// screen block starting at `block`, row by row.
fn block(map: &mut [u16], block: usize, at: (usize, usize), size: (usize, usize), first: u16) {
    let mut tile = first;
    for row in 0..size.1 {
        for column in 0..size.0 {
            map[block + (at.1 + row) * MAP_COLUMNS + at.0 + column] = tile;
            tile = tile.wrapping_add(1);
        }
    }
}

/// `0x08002404`: the strip's block in BG3, columns `from..to` of its four
/// rows, 11 tiles apart.
fn strip_block(map: &mut [u16], from: usize, to: usize) {
    for row in 0..STRIP_BLOCK_ROWS {
        for column in from..to {
            map[3 * BLOCK + row * MAP_COLUMNS + column] =
                STRIP_BLOCK + to_u16_usize(column + row * STRIP_BLOCK_STRIDE);
        }
    }
}

/// `0x08002E94`: the 8bpp picture in BG2, its 30×16 tiles one after
/// another from row 2, blank above and below.
fn picture_map(map: &mut [u16]) {
    for row in 0..PICTURE_MAP_ROWS {
        for column in 0..PICTURE_COLUMNS {
            map[2 * BLOCK + row * MAP_COLUMNS + column] = row
                .checked_sub(PICTURE_ROWS.0)
                .filter(|_| row < PICTURE_ROWS.1)
                .map_or(PICTURE_BLANK, |row| {
                    PICTURE_BASE + to_u16_usize(row * PICTURE_COLUMNS + column)
                });
        }
    }
}

/// The map coordinate the screen's `position` shows with `scroll`.
fn wrap(position: usize, scroll: i32) -> usize {
    let wrapped = (to_i32_usize(position) + scroll).rem_euclid(to_i32_usize(MAP_PIXELS));
    usize::try_from(wrapped).unwrap_or(0)
}

/// A line table entry's brightness level, its low nibble.
fn line_level(entry: u16) -> u16 {
    entry & 0xF
}

/// Alpha blending of two colors with weights out of 16. The reference
/// emulator blends the 8-bit channels, not the hardware's 5-bit ones, and
/// the port follows it so the pictures compare.
fn mix(top: Rgb, below: Rgb, (first, second): (u16, u16)) -> Rgb {
    channels(top, below, |a, b| ((a * first + b * second) >> 4).min(255))
}

/// The brightness decrease of a color by `level` of 16, on 8-bit channels
/// like [`mix`].
fn darken(color: Rgb, level: u16) -> Rgb {
    channels(color, color, |a, _| a - ((a * level) >> 4))
}

fn channels(a: Rgb, b: Rgb, each: impl Fn(u16, u16) -> u16) -> Rgb {
    let channel = |x: u8, y: u8| u8::try_from(each(u16::from(x), u16::from(y))).unwrap_or(u8::MAX);
    Rgb::new(channel(a.r, b.r), channel(a.g, b.g), channel(a.b, b.b))
}

fn to_u16(value: u32) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

fn to_u16_usize(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

fn to_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn to_i32_usize(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::tile::Tileset;

    const RED: u16 = 0x001F;
    const GREEN: u16 = 0x03E0;
    const BLUE: u16 = 0x7C00;

    /// Title graphics of solid tiles: the 4bpp ones all color 1, the
    /// picture's all color 2; the strip's palette red, the glow's green and
    /// the picture's color blue.
    fn graphics() -> TitleGraphics {
        let mut palettes = vec![[0; 16]; 15];
        palettes[11][1] = RED;
        palettes[12][1] = GREEN;
        palettes[0][2] = BLUE;
        TitleGraphics {
            tiles: Tileset::from_pixels(vec![[1; TILE_PIXELS]; 300]),
            picture: Tileset::from_pixels(vec![[2; TILE_PIXELS]; 480]),
            text_tiles: Tileset::from_pixels(Vec::new()),
            palettes,
            sprite_palettes: [[0; 16]; 2],
            intro_steps: vec![vec![[0x31, 0x21]], vec![[0x0A, 0x42]]],
            skip_strip: [[1; 9], [2; 9]],
        }
    }

    fn run_to(video: &mut TitleVideo, frame: u32) {
        while video.frame < frame {
            video.update(false);
        }
    }

    fn cell(video: &TitleVideo, block: usize, column: usize, row: usize) -> u16 {
        video.shadow.map[block * BLOCK + row * MAP_COLUMNS + column]
    }

    fn pixel(video: &TitleVideo, x: usize, y: usize) -> Option<Rgb> {
        let mut frame = Frame::new(WIDTH, HEIGHT, Rgb::default());
        video.draw(&mut frame);
        frame.pixel(x, y)
    }

    #[test]
    fn the_intro_lays_the_finished_title_and_ends_on_its_frame() {
        let mut video = TitleVideo::new(graphics());
        assert_eq!(video.frame, TITLE_SHOWN);
        run_to(&mut video, INTRO_END - 1);
        assert_eq!(video.stage, Stage::Intro);
        video.update(false);
        assert_eq!(video.stage, Stage::Ready(INTRO_END));
        assert!(!video.counts_idle());
        video.update(false);
        assert!(video.counts_idle());
        assert_eq!(cell(&video, 1, 0, 0), GLOW_LEFT);
        assert_eq!(cell(&video, 1, 19, 5), GLOW_RIGHT + 50);
        assert_eq!(cell(&video, 1, 12, 0), STRIP_TILE + 1);
        assert_eq!(cell(&video, 1, 12, 5), STRIP_TILE + 2);
        assert_eq!(cell(&video, 1, 16, 0), (STRIP_TILE + 2) | HFLIP);
        assert_eq!(cell(&video, 1, 16, 5), (STRIP_TILE + 4) | HFLIP);
        assert_eq!(cell(&video, 2, 0, 1), PICTURE_BLANK);
        assert_eq!(cell(&video, 2, 1, 3), PICTURE_BASE + 31);
        assert_eq!(cell(&video, 3, 11, 1), STRIP_BLOCK + 22);
        assert_eq!(cell(&video, 3, 31, 0), STRIP_BLOCK + 10);
        assert_eq!(cell(&video, 0, 14, 0), LOGO_TILE + 42);
        assert_eq!(cell(&video, 0, 7, 4), SUBTITLE_TILE);
        assert_eq!(
            video.shadow.scroll,
            [(-8, -36), (-4, -24), (0, 0), (-76, -32)]
        );
        assert_eq!(video.registers.display, 0x1E40);
        assert_eq!(video.registers.blend, 0);
        assert!(video.shown.sprites);
        // The last line's level reaches 0 on frame 627 from power-on.
        assert_eq!(video.lines_end, Some(218));
    }

    #[test]
    fn the_layers_show_by_priority_darkened_by_the_fade() {
        let video = TitleVideo::new(graphics());
        assert_eq!(pixel(&video, 40, 56), Some(Rgb::new(0, 64, 0)));
        let mut video = TitleVideo::new(graphics());
        run_to(&mut video, INTRO_END + 1);
        assert_eq!(pixel(&video, 0, 100), Some(Rgb::new(0, 0, 255)));
        assert_eq!(pixel(&video, 4, 24), Some(Rgb::new(0, 255, 0)));
        assert_eq!(pixel(&video, 8, 36), Some(Rgb::new(0, 0, 0)));
    }

    #[test]
    fn start_skips_the_intro_and_the_title_fades_in() {
        let mut video = TitleVideo::new(graphics());
        video.update(true);
        assert_eq!(video.stage, Stage::Intro);
        run_to(&mut video, 200);
        video.update(true);
        assert_eq!(video.stage, Stage::Skipped(201));
        video.update(false);
        assert_eq!(video.registers.display & LAYERS, 0);
        video.update(false);
        assert_eq!(video.registers.display & 0x1E00, 0x1E00);
        assert_eq!(video.flash_line, Some(SKIP_FLASH_LINE));
        assert_eq!(cell(&video, 1, 10, 0), STRIP_TILE + 1);
        assert_eq!(cell(&video, 1, 18, 5), STRIP_TILE + 2);
        assert_eq!(cell(&video, 1, 19, 0), GLOW_RIGHT);
        assert_eq!(cell(&video, 3, 10, 3), STRIP_BLOCK + 43);
        assert_eq!(video.registers.backgrounds[2] & 0xFF, 0x82);
        video.update(false);
        assert_eq!(pixel(&video, 0, 100), Some(Rgb::new(0, 0, 0)));
        run_to(&mut video, 201 + SKIP_READY);
        assert_eq!(video.stage, Stage::Ready(201 + SKIP_READY));
        assert_eq!(pixel(&video, 0, 100), Some(Rgb::new(0, 0, 255)));
        assert!(!video.takes_start());
        video.update(false);
        assert!(video.takes_start());
    }

    #[test]
    fn press_start_blinks_until_start_opens_the_menu() {
        let mut video = TitleVideo::new(graphics());
        run_to(&mut video, INTRO_END);
        assert!(!video.shows_press_start());
        let line = |video: &TitleVideo| {
            (0..PRESS_START_TILES)
                .map(|tile| cell(video, 1, PRESS_START_AT.0 + tile, PRESS_START_AT.1) != BLANK)
                .collect::<Vec<_>>()
        };
        let press: Vec<bool> = (0..PRESS_START_TILES).map(|tile| tile < 4).collect();
        let first = INTRO_END + 1;
        for (after, shown) in [
            (0, press.clone()),
            (15, vec![true; 9]),
            (30, vec![false; 9]),
            (45, press),
            (90, vec![true; 9]),
            (105, vec![false; 9]),
            (150, (0..9).map(|tile| tile < 4).collect()),
        ] {
            run_to(&mut video, first + after);
            assert_eq!(line(&video), shown, "{after}");
        }
        assert!(video.update(true));
        assert!(video.shows_press_start());
        video.update(false);
        assert!(!video.shows_press_start());
        run_to(&mut video, first + 400);
        assert!(!video.shows_press_start());
    }

    #[test]
    fn blends_on_eight_bit_channels() {
        assert_eq!(darken(Rgb::new(222, 148, 24), 11), Rgb::new(70, 47, 8));
        assert_eq!(darken(Rgb::new(222, 148, 24), 16), Rgb::new(0, 0, 0));
        assert_eq!(
            mix(Rgb::new(200, 100, 0), Rgb::new(100, 100, 100), (8, 8)),
            Rgb::new(150, 100, 50)
        );
        assert_eq!(
            mix(Rgb::new(255, 0, 0), Rgb::new(255, 0, 0), (16, 16)),
            Rgb::new(255, 0, 0)
        );
        assert_eq!(wrap(0, -4), 252);
        assert_eq!(wrap(10, 250), 4);
    }
}
