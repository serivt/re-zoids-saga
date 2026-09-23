//! The field: a scene the player walks around, with the camera following.
//!
//! Movement matches the original: one pixel per frame while a direction is
//! held, a 16×16 collision box at the bottom center of the 32×32 sprite
//! tested against the scene's metatile attributes, and a camera that keeps
//! the sprite at screen (104, 64) within the map's bounds. Animation uses
//! the sprite sheet's layout: idle images `direction × 3 + [0, 1, 0, 2]`,
//! walking images `12 + direction × 3 + [0, 1, 0, 2]`, four frames each.

use extraction::saga::{METATILE_TILES, Scene, SpriteSheet};
use gba_runtime::ppu::{PaletteBank, SCREEN_HEIGHT, SCREEN_WIDTH, draw_background};
use platform::{Button, Frame, Input};

use crate::draw_sprite;

const TILE_SIZE: usize = 8;
const METATILE_SIZE: usize = METATILE_TILES * TILE_SIZE;
const COLLISION_BOX: (usize, usize, usize, usize) = (8, 16, 16, 16);
const CAMERA_ANCHOR: (usize, usize) = (104, 64);
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
    fn from_input(input: Input) -> Option<Self> {
        [Self::Up, Self::Down, Self::Left, Self::Right]
            .into_iter()
            .find(|direction| input.is_held(direction.button()))
    }

    const fn button(self) -> Button {
        match self {
            Self::Up => Button::Up,
            Self::Down => Button::Down,
            Self::Left => Button::Left,
            Self::Right => Button::Right,
        }
    }

    fn delta(self) -> (isize, isize) {
        match self {
            Self::Up => (0, -1),
            Self::Down => (0, 1),
            Self::Left => (-1, 0),
            Self::Right => (1, 0),
        }
    }

    fn index(self) -> usize {
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
    /// Sprite top-left in map pixels.
    pub x: usize,
    /// Sprite top-left in map pixels.
    pub y: usize,
    /// Facing direction.
    pub facing: Direction,
    /// Frames spent in the current animation.
    pub animation: u32,
    /// Whether the last update moved the player.
    pub walking: bool,
}

/// A scene with the player in it.
pub struct Field {
    scene: Scene,
    sheet: SpriteSheet,
    palettes: PaletteBank,
    /// The player.
    pub player: Player,
}

impl Field {
    /// Places the player at sprite position `(x, y)` in `scene`.
    #[must_use]
    pub fn new(scene: Scene, sheet: SpriteSheet, (x, y): (usize, usize)) -> Self {
        let palettes = PaletteBank::from_bgr555(&scene.palettes);
        Self {
            scene,
            sheet,
            palettes,
            player: Player {
                x,
                y,
                facing: Direction::Down,
                animation: 0,
                walking: false,
            },
        }
    }

    /// The scene being walked.
    #[must_use]
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Advances one frame with the buttons held.
    pub fn update(&mut self, input: Input) {
        let Some(direction) = Direction::from_input(input) else {
            self.set_walking(false);
            self.player.animation += 1;
            return;
        };
        if self.player.facing != direction {
            self.player.facing = direction;
            self.player.animation = 0;
        }
        let (dx, dy) = direction.delta();
        let moved = self.try_step(dx, dy);
        self.set_walking(moved);
        self.player.animation += 1;
    }

    fn set_walking(&mut self, walking: bool) {
        if self.player.walking != walking {
            self.player.walking = walking;
            self.player.animation = 0;
        }
    }

    fn try_step(&mut self, dx: isize, dy: isize) -> bool {
        let Some(x) = self.player.x.checked_add_signed(dx) else {
            return false;
        };
        let Some(y) = self.player.y.checked_add_signed(dy) else {
            return false;
        };
        if self.box_blocked(x, y) {
            return false;
        }
        self.player.x = x;
        self.player.y = y;
        true
    }

    fn box_blocked(&self, x: usize, y: usize) -> bool {
        let (left, top, width, height) = COLLISION_BOX;
        let (x0, y0) = (x + left, y + top);
        let (x1, y1) = (x0 + width - 1, y0 + height - 1);
        [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
            .iter()
            .any(|(px, py)| self.scene.blocked(px / METATILE_SIZE, py / METATILE_SIZE))
    }

    /// Camera scroll in map pixels: the sprite sits at the anchor unless the
    /// map edge is closer.
    #[must_use]
    pub fn camera(&self) -> (usize, usize) {
        let map_width = self.scene.map.width * TILE_SIZE;
        let map_height = self.scene.map.height * TILE_SIZE;
        let max_x = map_width.saturating_sub(SCREEN_WIDTH);
        let max_y = map_height.saturating_sub(SCREEN_HEIGHT);
        (
            self.player.x.saturating_sub(CAMERA_ANCHOR.0).min(max_x),
            self.player.y.saturating_sub(CAMERA_ANCHOR.1).min(max_y),
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
            let x = i32::try_from(self.player.x).unwrap_or(i32::MAX)
                - i32::try_from(scroll.0).unwrap_or(0);
            let y = i32::try_from(self.player.y).unwrap_or(i32::MAX)
                - i32::try_from(scroll.1).unwrap_or(0);
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

    /// A scene of `columns`×`rows` metatiles whose outer ring is blocked.
    fn field(columns: usize, rows: usize) -> Field {
        let attributes = (0..columns * rows)
            .map(|i| {
                let (c, r) = (i % columns, i / columns);
                u16::from(c == 0 || r == 0 || c + 1 == columns || r + 1 == rows) << 15
            })
            .collect();
        let scene = Scene {
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
        };
        let sheet = SpriteSheet {
            tag: "mz25".to_owned(),
            frames: 0,
            tiles_per_frame: 16,
            palette: [0; 16],
            tiles: Tileset::from_4bpp(&[]),
        };
        Field::new(scene, sheet, (16, 0))
    }

    fn held(direction: Direction) -> Input {
        Input::default().with(direction.button())
    }

    #[test]
    fn walks_one_pixel_per_frame_until_blocked() {
        let mut field = field(6, 4);
        for _ in 0..40 {
            field.update(held(Direction::Left));
        }
        assert_eq!(field.player.x, 8);
        assert_eq!(field.player.facing, Direction::Left);
        assert!(!field.player.walking);
        for _ in 0..3 {
            field.update(held(Direction::Right));
        }
        assert_eq!(field.player.x, 11);
        assert!(field.player.walking);
    }

    #[test]
    fn animation_cycles_through_the_sheet_layout() {
        let mut field = field(6, 4);
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
        large.player.x = 200;
        large.player.y = 100;
        assert_eq!(large.camera(), (96, 36));
        large.player.x = 10_000;
        large.player.y = 10_000;
        assert_eq!(large.camera(), (640 - 240, 320 - 160));
        assert_eq!(field(6, 4).camera(), (0, 0));
    }
}
