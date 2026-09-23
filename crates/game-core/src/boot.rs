//! The screens before the game starts: publisher logo, title with its
//! menu, and name entry.
//!
//! Timings and layouts come from frame-by-frame captures of the original:
//! the logo fades in over 45 frames from frame 30, holds until frame 375,
//! fades out over 30 frames and leaves 30 frames of black; the title
//! fades in over 165 frames and waits for START, which opens the menu
//! script; the name entry lays out its windows as the original's window
//! records show and moves its cursor 16 pixels per character.

use extraction::saga::{self, BootError, KANA_COLUMNS, Logo, NameEntryGraphics, TitleGraphics};
use formats::tile::{TILE_PIXELS, TilePiece, Tileset};
use formats::tilemap::TileMap;
use gba_runtime::ppu::{
    FADE_STEPS, FullPalette, IndexedImage, Palette, PaletteBank, darken, draw_background,
    draw_background_256, draw_indexed,
};
use platform::{Button, Frame, Input, Rgb};

use crate::data::GameData;
use crate::script::{ScriptError, ScriptRunner};
use crate::text::CELL_WIDTH;
use crate::translation::{AlphabetPage, NAME_ENTRY_TABLE, TITLE_TABLE};
use crate::windows::{ScriptWindows, TextLayout};
use crate::{ScriptHost, TextPainter, WindowPainter};

const LOGO_BLACK_FRAMES: u32 = 30;
const LOGO_FADE_IN_FRAMES: u32 = 45;
const LOGO_HOLD_UNTIL: u32 = 375;
const LOGO_FADE_OUT_FRAMES: u32 = 30;
const LOGO_TAIL_FRAMES: u32 = 30;
const TITLE_FADE_IN_FRAMES: u32 = 165;
const SCREEN_TILES: usize = 32;
const TILE_PIXELS_I32: i32 = 8;
const TITLE_TILE_BASE: usize = 0x102;
const TITLE_PICTURE_BASE: usize = 0x115;
const TITLE_PICTURE_COLUMNS: usize = 30;
const TITLE_PICTURE_ROWS: (usize, usize) = (2, 18);
const TITLE_OBJ_TILE_BASE: usize = 175;
const TITLE_OBJ_TEXT_BASE: usize = 112;
const TITLE_GLOW_TILE: u16 = 0x139;
const TITLE_GLOW_RIGHT_TILE: u16 = 0x175;
const TITLE_STRIP_TILE: usize = 0x10D;
const TITLE_STRIP_STRIDE: usize = 11;
const TITLE_BG1_SCROLL: (usize, usize) = (252, 232);
const TITLE_BG3_SCROLL: (usize, usize) = (180, 224);
const GLOW_PALETTE: u16 = 12 << 12;
const STRIP_PALETTE: u16 = 11 << 12;
const HFLIP: u16 = 0x0400;
const NAME_SLOTS: usize = 8;
const NAME_GRID_ROWS_ROM: usize = 5;
/// Rows of characters a name-entry page shows.
pub(crate) const NAME_GRID_ROWS: usize = NAME_GRID_ROWS_ROM;
const NAME_PAGES: usize = 5;
const NAME_LABEL_WINDOW: (u8, u8, u8, u8) = (24, 4, 6, 4);
const NAME_FIELD_WINDOW: (u8, u8, u8, u8) = (8, 4, 16, 4);
const NAME_FIELD_MIN_WIDTH: u8 = 13;
const SCREEN_TILE_COLUMNS: u8 = 30;
/// Pixels the name entry's help line can take.
pub(crate) const NAME_HELP_PIXELS: usize = 160;
/// Pixels a name-entry page label can take once the name keeps its field.
pub(crate) const NAME_LABEL_PIXELS: usize = 56;
const NAME_GRID_ORIGIN: (i32, i32) = (8, 73);
const NAME_CELL: i32 = 16;
const NAME_SLOT_ORIGIN: (i32, i32) = (96, 50);
const NAME_ARROWS: [(i32, i32); 2] = [(72, 40), (168, 40)];
const NAME_RIGHT_ARROW_FROM_EDGE: u8 = 3;
const NAME_PICTURE_PALETTE_START: usize = 64;
const NAME_PICTURE_COLUMNS: usize = 16;
const FULL_WIDTH_SPACE: char = '\u{3000}';
/// Labels of the ROM's name-entry pages.
pub(crate) const NAME_PAGE_LABELS: [&str; NAME_PAGES] =
    ["カタカナ", "ひらがな", "英数文字", "特殊文字", "記号文字"];
/// The ROM's name-entry help line.
pub(crate) const NAME_HELP: &str = "ＳＴＡＲＴ：終了　ＳＥＬＥＣＴ：文字変更";
const NAME_FIELD_PREFIX: &str = "\u{3000}\u{3000}\u{3000}";
const CONFIRM_SCRIPT: usize = 1;
const PLAYER_PORTRAIT: (u8, u8) = (0, 0);

/// A sprite of the title screen: a block of consecutive OBJ tiles.
#[derive(Debug, Clone, Copy)]
struct TitleSprite {
    x: i32,
    y: i32,
    columns: usize,
    rows: usize,
    tile: usize,
    palette: usize,
}

const fn sprite(
    x: i32,
    y: i32,
    columns: usize,
    rows: usize,
    tile: usize,
    palette: usize,
) -> TitleSprite {
    TitleSprite {
        x,
        y,
        columns,
        rows,
        tile,
        palette,
    }
}

/// The 50 sprites of the title as OAM lists them: the logo in three rows
/// of 32×8 blocks, the subtitle and two copyright lines.
const TITLE_SPRITES: [TitleSprite; 50] = [
    sprite(8, 36, 4, 1, 0, 0),
    sprite(40, 36, 4, 1, 4, 0),
    sprite(72, 36, 4, 1, 8, 0),
    sprite(104, 36, 2, 1, 12, 0),
    sprite(8, 44, 4, 1, 14, 0),
    sprite(40, 44, 4, 1, 18, 0),
    sprite(72, 44, 4, 1, 22, 0),
    sprite(104, 44, 2, 1, 26, 0),
    sprite(8, 52, 4, 1, 28, 0),
    sprite(40, 52, 4, 1, 32, 0),
    sprite(72, 52, 4, 1, 36, 0),
    sprite(104, 52, 2, 1, 40, 0),
    sprite(120, 36, 4, 1, 42, 0),
    sprite(152, 36, 4, 1, 46, 0),
    sprite(184, 36, 4, 1, 50, 0),
    sprite(216, 36, 2, 1, 54, 0),
    sprite(120, 44, 4, 1, 56, 0),
    sprite(152, 44, 4, 1, 60, 0),
    sprite(184, 44, 4, 1, 64, 0),
    sprite(216, 44, 2, 1, 68, 0),
    sprite(120, 52, 4, 1, 70, 0),
    sprite(152, 52, 4, 1, 74, 0),
    sprite(184, 52, 4, 1, 78, 0),
    sprite(216, 52, 2, 1, 82, 0),
    sprite(64, 68, 4, 1, 84, 1),
    sprite(96, 68, 4, 1, 88, 1),
    sprite(128, 68, 4, 1, 92, 1),
    sprite(160, 68, 2, 1, 96, 1),
    sprite(64, 76, 4, 1, 98, 1),
    sprite(96, 76, 4, 1, 102, 1),
    sprite(128, 76, 4, 1, 106, 1),
    sprite(160, 76, 2, 1, 110, 1),
    sprite(32, 144, 4, 1, 112, 1),
    sprite(64, 144, 4, 1, 116, 1),
    sprite(96, 144, 4, 1, 120, 1),
    sprite(128, 144, 4, 1, 124, 1),
    sprite(160, 144, 4, 1, 128, 1),
    sprite(192, 144, 2, 1, 132, 1),
    sprite(208, 144, 1, 1, 134, 1),
    sprite(32, 152, 4, 1, 135, 1),
    sprite(64, 152, 4, 1, 139, 1),
    sprite(96, 152, 1, 1, 143, 1),
    sprite(104, 152, 4, 1, 144, 1),
    sprite(136, 152, 4, 1, 148, 1),
    sprite(168, 152, 4, 1, 152, 1),
    sprite(200, 152, 2, 1, 156, 1),
    sprite(0, 0, 0, 0, 0, 0),
    sprite(0, 0, 0, 0, 0, 0),
    sprite(0, 0, 0, 0, 0, 0),
    sprite(0, 0, 0, 0, 0, 0),
];

/// The publisher logo with its fades.
pub struct LogoScreen {
    logo: Logo,
    palette: FullPalette,
    frame: u32,
}

impl LogoScreen {
    /// Reads the logo from `rom`.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when the ROM is too short.
    pub fn new(data: &GameData<'_>) -> Result<Self, BootError> {
        let logo = data.logo()?;
        let palette = FullPalette::from_bgr555(&logo.palette);
        Ok(Self {
            logo,
            palette,
            frame: 0,
        })
    }

    /// Advances one frame; returns `true` once the logo is over.
    pub fn update(&mut self) -> bool {
        self.frame += 1;
        self.frame >= LOGO_HOLD_UNTIL + LOGO_FADE_OUT_FRAMES + LOGO_TAIL_FRAMES
    }

    /// Darkness of the current frame, 0 (full) to [`FADE_STEPS`] (black).
    #[must_use]
    pub fn darkness(&self) -> u8 {
        let frame = self.frame;
        if frame < LOGO_BLACK_FRAMES {
            FADE_STEPS
        } else if frame < LOGO_BLACK_FRAMES + LOGO_FADE_IN_FRAMES {
            fade_level(
                LOGO_BLACK_FRAMES + LOGO_FADE_IN_FRAMES - frame,
                LOGO_FADE_IN_FRAMES,
            )
        } else if frame < LOGO_HOLD_UNTIL {
            0
        } else if frame < LOGO_HOLD_UNTIL + LOGO_FADE_OUT_FRAMES {
            fade_level(frame - LOGO_HOLD_UNTIL, LOGO_FADE_OUT_FRAMES)
        } else {
            FADE_STEPS
        }
    }

    /// Draws the logo darkened as the fade says.
    pub fn draw(&self, frame: &mut Frame) {
        frame.fill(Rgb::default());
        let map = &self.logo.map;
        draw_background_256(
            frame,
            |x, y| map.wrapping(x, y),
            |index| self.logo.tiles.tile(index),
            &self.palette,
            (0, 0),
            false,
        );
        darken(frame, self.darkness());
    }
}

/// Fade level after `elapsed` of `total` frames toward black.
fn fade_level(elapsed: u32, total: u32) -> u8 {
    u8::try_from(u32::from(FADE_STEPS) * elapsed / total.max(1)).unwrap_or(FADE_STEPS)
}

/// What the player picked on the title menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleChoice {
    /// はじめから: a new game.
    NewGame,
    /// つづきから: continue a saved game (not available yet).
    Continue,
    /// オプション: options (not available yet).
    Options,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TitleState {
    FadingIn,
    Waiting,
    Menu,
}

/// The title screen: fades in, waits for START, then runs the menu script.
pub struct TitleScreen {
    graphics: TitleGraphics,
    palettes: PaletteBank,
    picture_palette: FullPalette,
    glow: TileMap,
    strip: TileMap,
    picture: TileMap,
    frame: u32,
    state: TitleState,
    runner: ScriptRunner,
    previous: Input,
}

impl TitleScreen {
    /// Reads the title graphics from `rom`.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when a block cannot be read.
    pub fn new(data: &GameData<'_>) -> Result<Self, BootError> {
        let graphics = data.title()?;
        let palettes = PaletteBank::from_bgr555(&graphics.palettes);
        let flat: Vec<u16> = graphics.palettes.iter().flatten().copied().collect();
        Ok(Self {
            picture_palette: FullPalette::from_bgr555(&flat),
            palettes,
            glow: glow_map(),
            strip: strip_map(),
            picture: picture_map(),
            graphics,
            frame: 0,
            state: TitleState::FadingIn,
            runner: ScriptRunner::named(
                TITLE_TABLE,
                data.script_offsets(TITLE_TABLE)
                    .ok()
                    .flatten()
                    .unwrap_or_default(),
            ),
            previous: Input::default(),
        })
    }

    /// Advances one frame; the menu runs on `windows` once START was
    /// pressed. Returns the choice when the menu ends.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when the menu script cannot run.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<Option<TitleChoice>, ScriptError> {
        let start = input.is_held(Button::Start) && !self.previous.is_held(Button::Start);
        self.previous = input;
        self.frame += 1;
        match self.state {
            TitleState::FadingIn => {
                if start || self.frame >= TITLE_FADE_IN_FRAMES {
                    self.frame = TITLE_FADE_IN_FRAMES;
                    self.state = TitleState::Waiting;
                }
            }
            TitleState::Waiting => {
                if start {
                    self.runner.start(0)?;
                    self.state = TitleState::Menu;
                }
            }
            TitleState::Menu => {
                if self.runner.update(rom, input, windows)? {
                    self.state = TitleState::Waiting;
                    return Ok(Some(match self.runner.vars()[1] {
                        0 => TitleChoice::NewGame,
                        1 => TitleChoice::Continue,
                        _ => TitleChoice::Options,
                    }));
                }
            }
        }
        Ok(None)
    }

    /// Darkness of the current frame.
    #[must_use]
    pub fn darkness(&self) -> u8 {
        if self.frame >= TITLE_FADE_IN_FRAMES {
            0
        } else {
            fade_level(TITLE_FADE_IN_FRAMES - self.frame, TITLE_FADE_IN_FRAMES)
        }
    }

    /// Draws the title layers and sprites.
    pub fn draw(&self, frame: &mut Frame) {
        frame.fill(Rgb::default());
        let picture = |index: usize| {
            index
                .checked_sub(TITLE_PICTURE_BASE)
                .and_then(|tile| self.graphics.picture.tile(tile))
        };
        draw_background_256(
            frame,
            |x, y| self.picture.wrapping(x, y),
            picture,
            &self.picture_palette,
            (0, 0),
            true,
        );
        let tiles = |index: usize| {
            index
                .checked_sub(TITLE_TILE_BASE)
                .and_then(|tile| self.graphics.tiles.tile(tile))
        };
        draw_background(
            frame,
            |x, y| self.strip.wrapping(x, y),
            tiles,
            &self.palettes,
            TITLE_BG3_SCROLL,
            true,
        );
        draw_background(
            frame,
            |x, y| self.glow.wrapping(x, y),
            tiles,
            &self.palettes,
            TITLE_BG1_SCROLL,
            true,
        );
        for sprite in TITLE_SPRITES.iter().filter(|sprite| sprite.columns > 0) {
            self.draw_sprite(frame, sprite);
        }
        darken(frame, self.darkness());
    }

    fn obj_tile(&self, index: usize) -> Option<&[u8; TILE_PIXELS]> {
        if index < TITLE_OBJ_TEXT_BASE {
            self.graphics.tiles.tile(TITLE_OBJ_TILE_BASE + index)
        } else {
            self.graphics.text_tiles.tile(index - TITLE_OBJ_TEXT_BASE)
        }
    }

    fn draw_sprite(&self, frame: &mut Frame, sprite: &TitleSprite) {
        let count = sprite.columns * sprite.rows;
        let tiles = Tileset::from_pixels(
            (0..count)
                .map(|i| {
                    self.obj_tile(sprite.tile + i)
                        .copied()
                        .unwrap_or([0; TILE_PIXELS])
                })
                .collect(),
        );
        let image = formats::tile::TileImage::compose(
            &tiles,
            &[TilePiece {
                column: 0,
                row: 0,
                columns: sprite.columns,
                rows: sprite.rows,
            }],
        );
        let palette =
            Palette::new(self.graphics.sprite_palettes[sprite.palette].map(Palette::from_bgr555));
        draw_indexed(
            frame,
            (sprite.x, sprite.y),
            IndexedImage {
                width: image.width,
                height: image.height,
                indices: &image.indices,
            },
            &palette,
            Some(0),
        );
    }
}

fn blank_map() -> TileMap {
    TileMap {
        width: SCREEN_TILES,
        height: SCREEN_TILES,
        entries: vec![0x100; SCREEN_TILES * SCREEN_TILES],
    }
}

/// BG1 as the loader builds it: two 10×6 glow blocks and a mirrored
/// strip between them on the first and last rows.
fn glow_map() -> TileMap {
    let mut map = blank_map();
    let mut set = |x: usize, y: usize, entry: u16| map.entries[y * SCREEN_TILES + x] = entry;
    for row in 0..6 {
        for column in 0..10 {
            let index = u16::try_from(row * 10 + column).unwrap_or(0);
            set(column, row, GLOW_PALETTE | (TITLE_GLOW_TILE + index));
            set(
                column + 19,
                row,
                GLOW_PALETTE | (TITLE_GLOW_RIGHT_TILE + index),
            );
        }
    }
    for (row, base) in [(0usize, 0x102u16), (5, 0x107)] {
        for i in 0..4u16 {
            set(10 + usize::from(i), row, STRIP_PALETTE | (base + i));
        }
        for i in 0..5u16 {
            set(
                14 + usize::from(i),
                row,
                STRIP_PALETTE | HFLIP | (base + 4 - i),
            );
        }
    }
    map
}

/// BG3 as the loader builds it: a 12×4 block of consecutive tiles with an
/// 11-tile stride, so each row starts where the previous one ended.
fn strip_map() -> TileMap {
    let mut map = blank_map();
    for row in 0..4 {
        for column in 0..12 {
            let tile =
                u16::try_from(TITLE_STRIP_TILE + row * TITLE_STRIP_STRIDE + column).unwrap_or(0);
            map.entries[row * SCREEN_TILES + column] = STRIP_PALETTE | tile;
        }
    }
    map
}

/// BG2 as the loader builds it: the 30×16 picture, one tile after another.
fn picture_map() -> TileMap {
    let mut map = TileMap {
        width: SCREEN_TILES,
        height: SCREEN_TILES,
        entries: vec![0x80; SCREEN_TILES * SCREEN_TILES],
    };
    for row in TITLE_PICTURE_ROWS.0..TITLE_PICTURE_ROWS.1 {
        for column in 0..TITLE_PICTURE_COLUMNS {
            let index = (row - TITLE_PICTURE_ROWS.0) * TITLE_PICTURE_COLUMNS + column;
            map.entries[row * SCREEN_TILES + column] =
                u16::try_from(TITLE_PICTURE_BASE + index).unwrap_or(0);
        }
    }
    map
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameState {
    Editing,
    Confirming,
}

/// The name entry screen: a character grid, the name field and its
/// confirmation dialog.
pub struct NameEntry {
    graphics: NameEntryGraphics,
    picture_palette: FullPalette,
    picture: TileMap,
    pages: Vec<AlphabetPage>,
    help: String,
    arrows: [(i32, i32); 2],
    page: usize,
    cursor: (usize, usize),
    name: Vec<char>,
    state: NameState,
    runner: ScriptRunner,
    previous: Input,
}

impl NameEntry {
    /// Reads the screen's data from `rom`; `name` is the default name and
    /// `input` what is held as the screen opens, so a button still down
    /// from the title does not count as pressed here.
    ///
    /// # Errors
    ///
    /// Returns [`BootError`] when a block cannot be read.
    pub fn new(data: &GameData<'_>, name: &str, input: Input) -> Result<Self, BootError> {
        let graphics = data.name_entry_graphics()?;
        let mut picture_palette = FullPalette::from_bgr555(&[]);
        picture_palette.write(NAME_PICTURE_PALETTE_START, &graphics.picture_palette);
        let scripts = data
            .script_offsets(NAME_ENTRY_TABLE)
            .ok()
            .flatten()
            .unwrap_or_default();
        let mut picture = blank_map();
        picture.entries.fill(0);
        for row in 0..NAME_PICTURE_COLUMNS {
            for column in 0..NAME_PICTURE_COLUMNS {
                picture.entries[row * SCREEN_TILES + column] =
                    u16::try_from(row * NAME_PICTURE_COLUMNS + column).unwrap_or(0);
            }
        }
        let table = data.kana_table()?;
        let pages = NAME_PAGE_LABELS
            .iter()
            .enumerate()
            .map(|(number, label)| AlphabetPage {
                label: (*label).to_owned(),
                rows: table
                    .iter()
                    .skip(number * NAME_GRID_ROWS)
                    .take(NAME_GRID_ROWS)
                    .cloned()
                    .collect(),
            })
            .collect();
        Ok(Self {
            graphics,
            picture_palette,
            picture,
            pages,
            help: NAME_HELP.to_owned(),
            arrows: NAME_ARROWS,
            page: 0,
            cursor: (0, 0),
            name: name.chars().take(NAME_SLOTS).collect(),
            state: NameState::Editing,
            runner: ScriptRunner::named(NAME_ENTRY_TABLE, scripts),
            previous: input,
        })
    }

    /// The name being entered.
    #[must_use]
    pub fn name(&self) -> String {
        self.name.iter().collect()
    }

    /// Opens the screen's windows on `windows`, taking the character pages
    /// and help line of its translation when it has them; the label window
    /// grows to the left for longer labels, leaving the name its field.
    pub fn open(&mut self, windows: &mut ScriptWindows<'_>) {
        let extensions = windows.extensions().clone();
        if let Some(pages) = extensions.borrow().alphabet_pages() {
            self.pages = pages;
            self.page = 0;
        }
        if let Some(help) = extensions.borrow().name_entry_help() {
            self.help = help;
        }
        let label_cells = self
            .pages
            .iter()
            .map(|page| windows.metrics().width(&page.label).div_ceil(CELL_WIDTH))
            .max()
            .unwrap_or(0);
        let label_width = u8::try_from(label_cells + 2)
            .unwrap_or(u8::MAX)
            .max(NAME_LABEL_WINDOW.2)
            .min(SCREEN_TILE_COLUMNS - NAME_FIELD_WINDOW.0 - NAME_FIELD_MIN_WIDTH);
        let label_x = SCREEN_TILE_COLUMNS - label_width;
        self.arrows[1].0 = i32::from(label_x - NAME_RIGHT_ARROW_FROM_EDGE) * TILE_PIXELS_I32;
        windows.close_window(None);
        windows.open_window(0, 0x10, (0, 0, 8, 8), 2);
        windows.portrait(0, PLAYER_PORTRAIT.0, PLAYER_PORTRAIT.1);
        windows.open_window(1, 0x40, (8, 0, 22, 4), 0);
        for ch in self.help.chars() {
            windows.put_char(1, ch);
        }
        windows.open_window(
            4,
            0x20,
            (
                label_x,
                NAME_LABEL_WINDOW.1,
                label_width,
                NAME_LABEL_WINDOW.3,
            ),
            0,
        );
        windows.open_window(
            5,
            0x20,
            (
                NAME_FIELD_WINDOW.0,
                NAME_FIELD_WINDOW.1,
                label_x - NAME_FIELD_WINDOW.0,
                NAME_FIELD_WINDOW.3,
            ),
            0,
        );
        windows.open_window(6, 0x21, (0, 8, 30, 12), 0);
        windows.set_layout(5, TextLayout::Cells);
        windows.set_layout(6, TextLayout::Cells);
        self.refresh(windows);
        windows.present(None);
    }

    fn refresh(&self, windows: &mut ScriptWindows<'_>) {
        windows.clear_window(4);
        for ch in self.pages[self.page].label.chars() {
            windows.put_char(4, ch);
        }
        windows.clear_window(6);
        for (row, line) in self.page_rows().iter().enumerate() {
            if row > 0 {
                windows.line_break(6);
            }
            for (column, ch) in line.iter().enumerate() {
                if column > 0 {
                    windows.put_char(6, FULL_WIDTH_SPACE);
                }
                windows.put_char(6, *ch);
            }
        }
        self.refresh_name(windows);
    }

    fn refresh_name(&self, windows: &mut ScriptWindows<'_>) {
        windows.clear_window(5);
        for ch in NAME_FIELD_PREFIX.chars().chain(self.name.iter().copied()) {
            windows.put_char(5, ch);
        }
        windows.set_player_name(&self.name());
    }

    fn page_rows(&self) -> &[Vec<char>] {
        self.pages.get(self.page).map_or(&[], |page| &page.rows)
    }

    /// Advances one frame; returns `true` when the name was confirmed.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when the confirmation script cannot run.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<bool, ScriptError> {
        let previous = self.previous;
        self.previous = input;
        let pressed = |button: Button| input.is_held(button) && !previous.is_held(button);
        match self.state {
            NameState::Confirming => {
                if self.runner.update(rom, input, windows)? {
                    if self.runner.vars()[0] == 0 {
                        return Ok(true);
                    }
                    self.state = NameState::Editing;
                    windows.close_window(Some(2));
                    windows.close_window(Some(3));
                }
                return Ok(false);
            }
            NameState::Editing => {}
        }
        let (column, row) = self.cursor;
        let columns = KANA_COLUMNS;
        if pressed(Button::Left) {
            self.cursor.0 = (column + columns - 1) % columns;
        } else if pressed(Button::Right) {
            self.cursor.0 = (column + 1) % columns;
        } else if pressed(Button::Up) {
            self.cursor.1 = (row + NAME_GRID_ROWS - 1) % NAME_GRID_ROWS;
        } else if pressed(Button::Down) {
            self.cursor.1 = (row + 1) % NAME_GRID_ROWS;
        }
        if pressed(Button::A) {
            let ch = self
                .page_rows()
                .get(self.cursor.1)
                .and_then(|line| line.get(self.cursor.0))
                .copied()
                .unwrap_or(FULL_WIDTH_SPACE);
            if ch != FULL_WIDTH_SPACE && self.name.len() < NAME_SLOTS {
                self.name.push(ch);
                self.refresh_name(windows);
            }
        } else if pressed(Button::B) {
            if self.name.pop().is_some() {
                self.refresh_name(windows);
            }
        } else if pressed(Button::Select) {
            self.page = (self.page + 1) % self.pages.len().max(1);
            self.cursor = (0, 0);
            self.refresh(windows);
        } else if pressed(Button::Start) && !self.name.is_empty() {
            self.runner.start(CONFIRM_SCRIPT)?;
            self.state = NameState::Confirming;
        }
        Ok(false)
    }

    /// Draws the picture, the windows and the sprites.
    pub fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        frame.fill(Rgb::default());
        draw_background_256(
            frame,
            |x, y| self.picture.wrapping(x, y),
            |index| self.graphics.picture.tile(index),
            &self.picture_palette,
            (0, 0),
            false,
        );
        windows.draw(frame, skin, painter);
        if self.state == NameState::Editing {
            for (i, (x, y)) in self.arrows.iter().enumerate() {
                draw_block(frame, &self.graphics.arrows, (1 - i) * 4, 2, 2, *x, *y);
            }
        }
        for slot in 0..NAME_SLOTS {
            let x = NAME_SLOT_ORIGIN.0 + i32::try_from(slot).unwrap_or(0) * TILE_PIXELS_I32;
            draw_block(
                frame,
                &self.graphics.slot_mark,
                0,
                1,
                1,
                x,
                NAME_SLOT_ORIGIN.1,
            );
        }
        if self.state == NameState::Editing {
            let x = NAME_GRID_ORIGIN.0 + i32::try_from(self.cursor.0).unwrap_or(0) * NAME_CELL;
            let y = NAME_GRID_ORIGIN.1 + i32::try_from(self.cursor.1).unwrap_or(0) * NAME_CELL;
            draw_block(frame, &self.graphics.cursor, 0, 1, 2, x, y);
        }
    }
}

fn draw_block(
    frame: &mut Frame,
    block: &saga::SpriteBlock,
    first: usize,
    columns: usize,
    rows: usize,
    x: i32,
    y: i32,
) {
    let tiles = Tileset::from_pixels(
        (0..columns * rows)
            .map(|i| {
                block
                    .tiles
                    .tile(first + i)
                    .copied()
                    .unwrap_or([0; TILE_PIXELS])
            })
            .collect(),
    );
    let image = formats::tile::TileImage::compose(
        &tiles,
        &[TilePiece {
            column: 0,
            row: 0,
            columns,
            rows,
        }],
    );
    let palette = Palette::new(block.palette.map(Palette::from_bgr555));
    draw_indexed(
        frame,
        (x, y),
        IndexedImage {
            width: image.width,
            height: image.height,
            indices: &image.indices,
        },
        &palette,
        Some(0),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_fades_in_holds_and_fades_out() {
        let mut screen = LogoScreen {
            logo: Logo {
                tiles: Tileset::from_4bpp(&[]),
                palette: [0; 16],
                map: TileMap {
                    width: 30,
                    height: 20,
                    entries: vec![0; 600],
                },
            },
            palette: FullPalette::from_bgr555(&[]),
            frame: 0,
        };
        assert_eq!(screen.darkness(), FADE_STEPS);
        for _ in 0..LOGO_BLACK_FRAMES + LOGO_FADE_IN_FRAMES {
            screen.update();
        }
        assert_eq!(screen.darkness(), 0);
        while screen.frame < LOGO_HOLD_UNTIL + LOGO_FADE_OUT_FRAMES / 2 {
            screen.update();
        }
        assert_eq!(screen.darkness(), FADE_STEPS / 2);
        let mut done = false;
        for _ in 0..LOGO_FADE_OUT_FRAMES + LOGO_TAIL_FRAMES {
            done = screen.update();
        }
        assert!(done);
        assert_eq!(fade_level(3, 6), FADE_STEPS / 2);
    }

    #[test]
    fn title_maps_follow_the_loader_layout() {
        let glow = glow_map();
        assert_eq!(glow.entries[0], GLOW_PALETTE | 0x139);
        assert_eq!(glow.entries[9], GLOW_PALETTE | 0x142);
        assert_eq!(glow.entries[SCREEN_TILES + 19], GLOW_PALETTE | 0x17F);
        assert_eq!(glow.entries[10], STRIP_PALETTE | 0x102);
        assert_eq!(glow.entries[14], STRIP_PALETTE | HFLIP | 0x106);
        assert_eq!(glow.entries[SCREEN_TILES * 6], 0x100);
        let strip = strip_map();
        assert_eq!(strip.entries[11], STRIP_PALETTE | 0x118);
        assert_eq!(strip.entries[SCREEN_TILES], STRIP_PALETTE | 0x118);
        let picture = picture_map();
        assert_eq!(picture.entries[SCREEN_TILES * 2], 0x115);
        assert_eq!(picture.entries[SCREEN_TILES * 3], 0x115 + 30);
        assert_eq!(picture.entries[0], 0x80);
    }
}
