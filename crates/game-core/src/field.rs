//! The field: a scene the player walks around, with the camera following.
//!
//! Movement matches the original: the player stands on a 16×16 metatile,
//! a held direction starts a step onto the next metatile when it is not
//! blocked, and a started step always completes at one pixel per frame.
//! The sprite is 32×32 with its bottom-center 16×16 on the metatile below
//! the standing one, so the sprite's top-left is `(16 × column − 8,
//! 16 × row)`. The camera keeps the sprite at screen (104, 64) within the
//! map's bounds. Animation uses the sprite sheet's layout: idle images
//! `direction × 3 + [0, 1, 0, 2]`, walking images `12 + direction × 3 +
//! [0, 1, 0, 2]`, four frames each. Completing a step onto an exit
//! metatile reports the exit so the caller can warp.

use extraction::saga::{self, METATILE_TILES, MapError, Scene, SceneError, SpriteSheet, Warp};
use gba_runtime::ppu::{PaletteBank, SCREEN_HEIGHT, SCREEN_WIDTH, draw_background};
use platform::{Button, Frame, Input};
use thiserror::Error;

use crate::draw_sprite;

const TILE_SIZE: usize = 8;
const METATILE_SIZE: usize = METATILE_TILES * TILE_SIZE;
const SPRITE_OFFSET_X: isize = -8;
const CAMERA_ANCHOR: (isize, isize) = (104, 64);
const FRAMES_PER_IMAGE: u32 = 4;
const ANIMATION_CYCLE: [usize; 4] = [0, 1, 0, 2];
const IMAGES_PER_DIRECTION: usize = 3;
const WALK_BASE: usize = 12;

/// Where the player faces, in the order the sprite sheet uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Facing away from the camera.
    Up,
    /// Facing the camera.
    Down,
    /// Facing left.
    Left,
    /// Facing right.
    Right,
}

impl Direction {
    const ALL: [Self; 4] = [Self::Up, Self::Down, Self::Left, Self::Right];

    fn from_input(input: Input) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|direction| input.is_held(direction.button()))
    }

    /// The direction the sprite sheet numbers `index`.
    #[must_use]
    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    const fn button(self) -> Button {
        match self {
            Self::Up => Button::Up,
            Self::Down => Button::Down,
            Self::Left => Button::Left,
            Self::Right => Button::Right,
        }
    }

    const fn delta(self) -> (isize, isize) {
        match self {
            Self::Up => (0, -1),
            Self::Down => (0, 1),
            Self::Left => (-1, 0),
            Self::Right => (1, 0),
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Up => 0,
            Self::Down => 1,
            Self::Left => 2,
            Self::Right => 3,
        }
    }
}

/// The player's position and animation state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    /// Metatile column the player stands on.
    pub column: usize,
    /// Metatile row the player stands on.
    pub row: usize,
    /// Facing direction.
    pub facing: Direction,
    /// Pixels advanced into the step being taken, 0 when standing.
    pub step: usize,
    /// Frames spent in the current animation.
    pub animation: u32,
    /// Whether the player is stepping.
    pub walking: bool,
}

impl Player {
    /// Sprite top-left in map pixels.
    #[must_use]
    pub fn position(&self) -> (isize, isize) {
        let (dx, dy) = if self.walking {
            self.facing.delta()
        } else {
            (0, 0)
        };
        let step = isize::try_from(self.step).unwrap_or(0);
        (
            metatile_pixels(self.column) + SPRITE_OFFSET_X + dx * step,
            metatile_pixels(self.row) + dy * step,
        )
    }

    /// The metatile the collision box sits on: below the standing one.
    #[must_use]
    pub fn footing(&self) -> (usize, usize) {
        (self.column, self.row + 1)
    }
}

fn metatile_pixels(cell: usize) -> isize {
    isize::try_from(cell * METATILE_SIZE).unwrap_or(isize::MAX)
}

/// Why a field could not be loaded from the ROM.
#[derive(Debug, Error)]
pub enum FieldError {
    /// The map record or warp is unreadable.
    #[error(transparent)]
    Map(#[from] MapError),
    /// The scene is unreadable.
    #[error(transparent)]
    Scene(#[from] SceneError),
}

/// A scene with the player in it.
pub struct Field {
    map: usize,
    scene: Scene,
    sheet: SpriteSheet,
    palettes: PaletteBank,
    /// The player.
    pub player: Player,
}

impl Field {
    /// Stands the player on metatile `(column, row)` of `scene`.
    #[must_use]
    pub fn new(scene: Scene, sheet: SpriteSheet, (column, row): (usize, usize)) -> Self {
        let palettes = PaletteBank::from_bgr555(&scene.palettes);
        Self {
            map: 0,
            scene,
            sheet,
            palettes,
            player: Player {
                column,
                row,
                facing: Direction::Down,
                step: 0,
                animation: 0,
                walking: false,
            },
        }
    }

    /// Loads map `map` from `rom` and stands the player on `(column, row)`.
    ///
    /// # Errors
    ///
    /// Returns [`FieldError`] when the map record or its scene cannot be read.
    pub fn load(
        rom: &[u8],
        map: usize,
        sheet: SpriteSheet,
        (column, row): (usize, usize),
    ) -> Result<Self, FieldError> {
        let scene = saga::scene(rom, saga::map_record(rom, map)?.scene)?;
        let mut field = Self::new(scene, sheet, (column, row));
        field.map = map;
        Ok(field)
    }

    /// The map record being walked.
    #[must_use]
    pub fn map(&self) -> usize {
        self.map
    }

    /// The scene being walked.
    #[must_use]
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Advances one frame with the buttons held; returns the exit whose
    /// metatile the player finished stepping onto, if any.
    pub fn update(&mut self, input: Input) -> Option<usize> {
        self.player.animation += 1;
        if self.player.walking {
            return self.advance_step();
        }
        let direction = Direction::from_input(input)?;
        if self.player.facing != direction {
            self.player.facing = direction;
            self.player.animation = 0;
        }
        let (dx, dy) = direction.delta();
        let (column, row) = self.player.footing();
        let target = (column.checked_add_signed(dx), row.checked_add_signed(dy));
        if let (Some(column), Some(row)) = target
            && !self.scene.blocked(column, row)
        {
            self.player.walking = true;
            self.player.animation = 0;
            self.advance_step();
        }
        None
    }

    fn advance_step(&mut self) -> Option<usize> {
        self.player.step += 1;
        if self.player.step < METATILE_SIZE {
            return None;
        }
        let (dx, dy) = self.player.facing.delta();
        self.player.column = self.player.column.saturating_add_signed(dx);
        self.player.row = self.player.row.saturating_add_signed(dy);
        self.player.step = 0;
        self.player.walking = false;
        let (column, row) = self.player.footing();
        self.scene.exit(column, row)
    }

    /// Follows `exit` of the current map: loads the destination and stands
    /// the player where the warp says.
    ///
    /// # Errors
    ///
    /// Returns [`FieldError`] when the warp, the map record or the scene
    /// cannot be read.
    pub fn warp(&mut self, rom: &[u8], exit: usize) -> Result<Warp, FieldError> {
        let warp = saga::warp(rom, self.map, exit)?;
        let scene = saga::scene(rom, saga::map_record(rom, warp.map)?.scene)?;
        self.enter(scene, warp.map, &warp);
        Ok(warp)
    }

    /// Replaces the scene and places the player as `warp` says.
    pub fn enter(&mut self, scene: Scene, map: usize, warp: &Warp) {
        self.palettes = PaletteBank::from_bgr555(&scene.palettes);
        self.scene = scene;
        self.map = map;
        self.player.column = warp.column;
        self.player.row = warp.row;
        self.player.step = 0;
        self.player.walking = false;
        self.player.animation = 0;
        if let Some(facing) = warp.facing.and_then(Direction::from_index) {
            self.player.facing = facing;
        }
    }

    /// Camera scroll in map pixels: the sprite sits at the anchor unless the
    /// map edge is closer.
    #[must_use]
    pub fn camera(&self) -> (usize, usize) {
        let map_width = self.scene.map.width * TILE_SIZE;
        let map_height = self.scene.map.height * TILE_SIZE;
        let max_x = map_width.saturating_sub(SCREEN_WIDTH);
        let max_y = map_height.saturating_sub(SCREEN_HEIGHT);
        let (x, y) = self.player.position();
        let scroll = |position: isize, anchor: isize, max: usize| {
            usize::try_from(position - anchor).unwrap_or(0).min(max)
        };
        (
            scroll(x, CAMERA_ANCHOR.0, max_x),
            scroll(y, CAMERA_ANCHOR.1, max_y),
        )
    }

    /// Index of the sprite sheet image for the current animation frame.
    #[must_use]
    pub fn image_index(&self) -> usize {
        let step = (self.player.animation / FRAMES_PER_IMAGE) as usize % ANIMATION_CYCLE.len();
        let base = if self.player.walking { WALK_BASE } else { 0 };
        base + self.player.facing.index() * IMAGES_PER_DIRECTION + ANIMATION_CYCLE[step]
    }

    /// Draws the scene and the player for the current frame.
    pub fn draw(&self, frame: &mut Frame) {
        let scroll = self.camera();
        let tile = |index: usize| self.scene.tiles.tile(index);
        let backdrop = |x: usize, y: usize| self.scene.backdrop.wrapping(x, y);
        draw_background(frame, backdrop, tile, &self.palettes, scroll, false);
        let map = |x: usize, y: usize| self.scene.map.wrapping(x, y);
        draw_background(frame, map, tile, &self.palettes, scroll, true);
        if let Some(image) = self.sheet.frame(self.image_index()) {
            let (x, y) = self.player.position();
            let x = i32::try_from(x).unwrap_or(i32::MAX) - i32::try_from(scroll.0).unwrap_or(0);
            let y = i32::try_from(y).unwrap_or(i32::MAX) - i32::try_from(scroll.1).unwrap_or(0);
            draw_sprite(frame, x, y, &image, &self.sheet.palette);
        }
    }
}

/// Draws `scene` with the map's top-left visible pixel at `scroll`.
pub fn draw_scene(frame: &mut Frame, scene: &Scene, scroll: (usize, usize)) {
    let bank = PaletteBank::from_bgr555(&scene.palettes);
    let tile = |index: usize| scene.tiles.tile(index);
    draw_background(
        frame,
        |x, y| scene.backdrop.wrapping(x, y),
        tile,
        &bank,
        scroll,
        false,
    );
    draw_background(
        frame,
        |x, y| scene.map.wrapping(x, y),
        tile,
        &bank,
        scroll,
        true,
    );
}

#[cfg(test)]
mod tests {
    use formats::tile::Tileset;
    use formats::tilemap::TileMap;

    use super::*;

    /// A scene of `columns`×`rows` metatiles whose outer ring is blocked,
    /// with exit 1 at `(1, 2)`.
    fn scene(columns: usize, rows: usize) -> Scene {
        let attributes = (0..columns * rows)
            .map(|i| {
                let (c, r) = (i % columns, i / columns);
                if c == 0 || r == 0 || c + 1 == columns || r + 1 == rows {
                    0x8000
                } else if (c, r) == (1, 2) {
                    0x4001
                } else {
                    0
                }
            })
            .collect();
        Scene {
            tiles: Tileset::from_4bpp(&[]),
            palettes: vec![],
            map: TileMap {
                width: columns * METATILE_TILES,
                height: rows * METATILE_TILES,
                entries: vec![0; columns * rows * 4],
            },
            backdrop: TileMap {
                width: 32,
                height: 32,
                entries: vec![0; 1024],
            },
            attributes,
        }
    }

    /// The player standing on `(3, 1)` of a `columns`×`rows` scene.
    fn field(columns: usize, rows: usize) -> Field {
        let sheet = SpriteSheet {
            tag: "mz25".to_owned(),
            frames: 0,
            tiles_per_frame: 16,
            palette: [0; 16],
            tiles: Tileset::from_4bpp(&[]),
        };
        Field::new(scene(columns, rows), sheet, (3, 1))
    }

    fn held(direction: Direction) -> Input {
        Input::default().with(direction.button())
    }

    #[test]
    fn steps_a_whole_metatile_once_started() {
        let mut field = field(6, 5);
        assert_eq!(field.player.position(), (40, 16));
        field.update(held(Direction::Right));
        assert_eq!(field.player.position(), (41, 16));
        assert!(field.player.walking);
        for _ in 0..14 {
            field.update(Input::default());
        }
        assert_eq!(field.player.position(), (55, 16));
        assert!(field.player.walking);
        field.update(Input::default());
        assert_eq!(field.player.position(), (56, 16));
        assert_eq!((field.player.column, field.player.row), (4, 1));
        assert!(!field.player.walking);
        field.update(Input::default());
        assert_eq!(field.player.position(), (56, 16));
    }

    #[test]
    fn keeps_stepping_while_held_and_stops_at_walls() {
        let mut field = field(6, 4);
        for _ in 0..40 {
            field.update(held(Direction::Right));
        }
        assert_eq!((field.player.column, field.player.row), (4, 1));
        assert_eq!(field.player.position(), (56, 16));
        assert_eq!(field.player.facing, Direction::Right);
        assert!(!field.player.walking);
        field.update(held(Direction::Down));
        assert_eq!(field.player.facing, Direction::Down);
        assert_eq!(field.player.position(), (56, 16));
        assert!(!field.player.walking);
    }

    #[test]
    fn reports_the_exit_when_the_step_onto_it_completes() {
        let mut field = field(6, 5);
        let mut exits = Vec::new();
        for _ in 0..32 {
            exits.extend(field.update(held(Direction::Left)));
        }
        assert_eq!(exits, [1]);
        assert_eq!(field.player.footing(), (1, 2));
        assert_eq!(field.update(Input::default()), None);
    }

    #[test]
    fn entering_a_scene_places_and_turns_the_player() {
        let mut field = field(6, 5);
        field.update(held(Direction::Left));
        let warp = Warp {
            map: 7,
            column: 2,
            row: 2,
            facing: Some(3),
            sound: 0,
        };
        field.enter(scene(8, 8), 7, &warp);
        assert_eq!(field.map(), 7);
        assert_eq!(field.player.position(), (24, 32));
        assert_eq!(field.player.facing, Direction::Right);
        assert!(!field.player.walking);
        let keep = Warp {
            facing: None,
            ..warp
        };
        field.enter(scene(8, 8), 7, &keep);
        assert_eq!(field.player.facing, Direction::Right);
    }

    #[test]
    fn animation_cycles_through_the_sheet_layout() {
        let mut field = field(6, 5);
        assert_eq!(field.image_index(), 3);
        field.update(held(Direction::Right));
        assert_eq!(field.image_index(), 21);
        for _ in 0..4 {
            field.update(held(Direction::Right));
        }
        assert_eq!(field.image_index(), 22);
        for _ in 0..8 {
            field.update(held(Direction::Right));
        }
        assert_eq!(field.image_index(), 23);
    }

    #[test]
    fn camera_keeps_the_sprite_at_the_anchor_within_bounds() {
        let mut large = field(40, 20);
        assert_eq!(large.camera(), (0, 0));
        large.player.column = 13;
        large.player.row = 7;
        assert_eq!(large.player.position(), (200, 112));
        assert_eq!(large.camera(), (96, 48));
        large.player.column = 1000;
        large.player.row = 1000;
        assert_eq!(large.camera(), (640 - 240, 320 - 160));
        assert_eq!(field(6, 5).camera(), (0, 0));
    }
}
