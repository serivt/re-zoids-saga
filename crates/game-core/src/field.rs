//! The field: a scene the player walks around, with the camera following.
//!
//! Movement matches the original: the player stands on a 16×16 metatile,
//! a held direction starts a step onto the next metatile when it is not
//! blocked, and a started step always completes at one pixel per frame.
//! Every actor has an anchor at the bottom center of its standing
//! metatile's lower neighbour, `(16 × column + 8, 16 × row + 16)`; frames
//! are drawn at the anchor plus their own offset, which puts a 32×32
//! sprite's top-left at `(16 × column − 8, 16 × row)`. The camera keeps the
//! player's anchor at screen (120, 80) within the map's bounds. Animations
//! come from the sprite sheet: idle animation = facing direction, walking
//! animation = facing + 4, each step lasting half its listed ticks.
//! Characters from the map's object list stand still on their metatile,
//! block the player and are drawn in back-to-front order, in front of the
//! player when level with it. Completing a step onto an exit metatile
//! reports the exit so the caller can warp; pressing A while standing and
//! facing a character turns it to the player (unless it is furniture) and
//! reports its dialogue so the caller can open a talk box. Wandering
//! characters (kind 2) wait a random `0..=127` frames, turn to a random
//! direction and, when the metatile ahead of their footing is free, walk
//! there at half the player's speed (32 frames); a blocked direction only
//! turns them and rolls a new wait.

use extraction::saga::{
    self, METATILE_TILES, MapError, PLAYER_SPRITE, Scene, SceneError, SpriteSheet,
    SpriteSheetError, WALK_ANIMATION_BASE, Warp,
};
use gba_runtime::ppu::{PaletteBank, SCREEN_HEIGHT, SCREEN_WIDTH, draw_background};
use platform::{Button, Frame, Input};
use thiserror::Error;

use crate::data::GameData;
use crate::draw_sprite;
use crate::rng::Rng;

const TILE_SIZE: usize = 8;
const METATILE_SIZE: usize = METATILE_TILES * TILE_SIZE;
const ANCHOR_OFFSET: (isize, isize) = (8, 16);
const CAMERA_ANCHOR: (isize, isize) = (120, 80);
const ANIMATION_SPEED_SHIFT: u32 = 1;
const FURNITURE_BEHAVIOR: u16 = 2;
const WANDER_KIND: u16 = 2;
const NPC_STEP_FRAMES: u32 = 32;
const IDLE_TIMER_MASK: u16 = 0x7F;

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

    const fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

/// Something the field reports after a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldEvent {
    /// The player finished a step onto this exit.
    Exit(usize),
    /// The player spoke to the character at this index in `npcs`, whose
    /// dialogue string this is.
    Talk {
        /// Index into the field's characters.
        npc: usize,
        /// Dialogue string index.
        dialogue: usize,
    },
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
    /// Anchor in map pixels: bottom center of the standing metatile's
    /// lower neighbour, moved along the step in progress.
    #[must_use]
    pub fn anchor(&self) -> (isize, isize) {
        let (dx, dy) = if self.walking {
            self.facing.delta()
        } else {
            (0, 0)
        };
        let step = isize::try_from(self.step).unwrap_or(0);
        let (x, y) = anchor(self.column, self.row);
        (x + dx * step, y + dy * step)
    }

    /// The metatile the collision box sits on: below the standing one.
    #[must_use]
    pub fn footing(&self) -> (usize, usize) {
        (self.column, self.row + 1)
    }

    /// Sheet animation to play: idle while standing, walking while stepping.
    #[must_use]
    pub fn animation_id(&self) -> usize {
        let base = if self.walking { WALK_ANIMATION_BASE } else { 0 };
        base + self.facing.index()
    }
}

/// An order given to a character by a cutscene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcCommand {
    /// Turn without moving.
    Face(Direction),
    /// Turn and walk one metatile, when the cell ahead is free.
    Step(Direction),
}

/// A character standing on the map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Npc {
    /// Metatile column it stands on.
    pub column: usize,
    /// Metatile row it stands on.
    pub row: usize,
    /// Its sprite.
    pub sheet: SpriteSheet,
    /// Sheet animation it plays.
    pub animation_id: usize,
    /// Frames spent in the animation.
    pub animation: u32,
    /// Dialogue string it says when spoken to.
    pub dialogue: Option<usize>,
    /// Whether it turns toward the player when spoken to.
    pub turns: bool,
    /// Whether it walks around on its own.
    pub wanders: bool,
    /// Where it faces.
    pub facing: Direction,
    /// Frames left before it acts again while standing.
    pub timer: u16,
    /// Frames into the step being taken, 0 when standing; the standing
    /// metatile is already the step's destination.
    pub step: u32,
}

impl Npc {
    /// The metatile it blocks: below the standing one.
    #[must_use]
    pub fn footing(&self) -> (usize, usize) {
        (self.column, self.row + 1)
    }

    /// Anchor in map pixels, behind the destination by what is left of
    /// the step.
    #[must_use]
    pub fn anchor(&self) -> (isize, isize) {
        let (x, y) = anchor(self.column, self.row);
        if self.step == 0 {
            return (x, y);
        }
        let (dx, dy) = self.facing.delta();
        let left =
            isize::try_from((NPC_STEP_FRAMES - self.step.min(NPC_STEP_FRAMES)) / 2).unwrap_or(0);
        (x - dx * left, y - dy * left)
    }

    fn face(&mut self, direction: Direction) {
        self.facing = direction;
        self.animation_id = direction.index();
        self.animation = 0;
    }
}

fn random_direction(value: u16) -> Direction {
    match value >> 14 {
        0 => Direction::Up,
        1 => Direction::Down,
        2 => Direction::Left,
        _ => Direction::Right,
    }
}

fn anchor(column: usize, row: usize) -> (isize, isize) {
    let pixels = |cell: usize| isize::try_from(cell * METATILE_SIZE).unwrap_or(isize::MAX);
    (
        pixels(column) + ANCHOR_OFFSET.0,
        pixels(row) + ANCHOR_OFFSET.1,
    )
}

/// Frame record shown by `sheet`'s animation `id` after `elapsed` frames,
/// with steps lasting half their ticks (at least one frame).
#[must_use]
pub fn current_frame(sheet: &SpriteSheet, id: usize, elapsed: u32) -> Option<usize> {
    let steps = sheet.animations.get(id)?;
    let length = |ticks: u32| (ticks >> ANIMATION_SPEED_SHIFT).max(1);
    let cycle: u32 = steps.iter().map(|step| length(step.duration)).sum();
    if cycle == 0 {
        return None;
    }
    let mut remaining = elapsed % cycle;
    steps
        .iter()
        .find(|step| {
            let frames = length(step.duration);
            if remaining < frames {
                true
            } else {
                remaining -= frames;
                false
            }
        })
        .map(|step| step.frame)
}

/// Why a field could not be loaded from the ROM.
#[derive(Debug, Error)]
pub enum FieldError {
    /// The map record, object list or warp is unreadable.
    #[error(transparent)]
    Map(#[from] MapError),
    /// The scene is unreadable.
    #[error(transparent)]
    Scene(#[from] SceneError),
    /// A sprite is unreadable.
    #[error(transparent)]
    Sprite(#[from] SpriteSheetError),
}

/// A scene with the player and the map's characters in it.
pub struct Field {
    map: usize,
    scene: Scene,
    sheet: SpriteSheet,
    palettes: PaletteBank,
    /// The player.
    pub player: Player,
    /// The characters standing on the map.
    pub npcs: Vec<Npc>,
    previous: Input,
    frame: u16,
    rng: Rng,
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
            npcs: Vec::new(),
            previous: Input::default(),
            frame: 0,
            rng: Rng::default(),
        }
    }

    /// Loads map `map` from `rom` with its characters and stands the player
    /// on `(column, row)`.
    ///
    /// # Errors
    ///
    /// Returns [`FieldError`] when the map record, its scene, its objects or
    /// a sprite cannot be read.
    pub fn load(
        data: &GameData<'_>,
        map: usize,
        (column, row): (usize, usize),
    ) -> Result<Self, FieldError> {
        let (scene, player_sheet, npcs) = load_map(data, map)?;
        let mut field = Self::new(scene, player_sheet, (column, row));
        field.map = map;
        field.npcs = npcs;
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

    /// Advances one frame with the buttons held; reports an exit reached or
    /// a character spoken to.
    pub fn update(&mut self, input: Input) -> Option<FieldEvent> {
        let pressed_a = input.is_held(Button::A) && !self.previous.is_held(Button::A);
        self.previous = input;
        self.frame = self.frame.wrapping_add(1);
        self.player.animation += 1;
        for index in 0..self.npcs.len() {
            self.npcs[index].animation += 1;
            self.wander(index);
        }
        if self.player.walking {
            return self.advance_step().map(FieldEvent::Exit);
        }
        if pressed_a {
            return self.talk();
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
            && !self.blocked(column, row)
        {
            self.player.walking = true;
            self.player.animation = 0;
            self.advance_step();
        }
        None
    }

    fn wander(&mut self, index: usize) {
        let npc = &self.npcs[index];
        if npc.step > 0 {
            let npc = &mut self.npcs[index];
            npc.step += 1;
            if npc.step >= NPC_STEP_FRAMES {
                npc.step = 0;
                npc.animation_id = npc.facing.index();
                npc.timer = self.rng.next(self.frame) & IDLE_TIMER_MASK;
            }
            return;
        }
        if !npc.wanders {
            return;
        }
        if npc.timer > 0 {
            self.npcs[index].timer -= 1;
            return;
        }
        self.rng.seed(self.frame);
        let direction = random_direction(self.rng.next(self.frame));
        if !self.start_npc_step(index, direction) {
            self.npcs[index].timer = self.rng.next(self.frame) & IDLE_TIMER_MASK;
        }
    }

    /// Turns character `index` toward `direction` and starts a step when
    /// the cell ahead is free; returns whether it stepped.
    fn start_npc_step(&mut self, index: usize, direction: Direction) -> bool {
        let (dx, dy) = direction.delta();
        let (column, row) = self.npcs[index].footing();
        let target = (column.checked_add_signed(dx), row.checked_add_signed(dy));
        let free = match target {
            (Some(column), Some(row)) => !self.blocked_for_npc(index, column, row),
            _ => false,
        };
        let npc = &mut self.npcs[index];
        npc.face(direction);
        if free {
            npc.column = npc.column.saturating_add_signed(dx);
            npc.row = npc.row.saturating_add_signed(dy);
            npc.step = 1;
            npc.animation_id = WALK_ANIMATION_BASE + direction.index();
        }
        free
    }

    /// Gives character `index` a cutscene order; returns whether a step
    /// started (or, for a turn, whether the character exists).
    pub fn command_npc(&mut self, index: usize, command: NpcCommand) -> bool {
        if index >= self.npcs.len() {
            return false;
        }
        match command {
            NpcCommand::Face(direction) => {
                self.npcs[index].face(direction);
                true
            }
            NpcCommand::Step(direction) => self.start_npc_step(index, direction),
        }
    }

    /// Whether character `index` is standing still (or absent).
    #[must_use]
    pub fn npc_idle(&self, index: usize) -> bool {
        self.npcs.get(index).is_none_or(|npc| npc.step == 0)
    }

    /// Adds a character loaded from `rom` at `(column, row)`; returns its
    /// index.
    ///
    /// # Errors
    ///
    /// Returns [`FieldError`] when the sprite cannot be read.
    pub fn spawn_npc(
        &mut self,
        data: &GameData<'_>,
        sprite: usize,
        (column, row): (usize, usize),
        facing: Direction,
    ) -> Result<usize, FieldError> {
        let sheet = data.sprite_sheet(sprite)?;
        self.npcs.push(Npc {
            column,
            row,
            sheet,
            animation_id: facing.index(),
            animation: 0,
            dialogue: None,
            turns: true,
            wanders: false,
            facing,
            timer: 0,
            step: 0,
        });
        Ok(self.npcs.len() - 1)
    }

    fn blocked_for_npc(&self, index: usize, column: usize, row: usize) -> bool {
        let (dx, dy) = if self.player.walking {
            self.player.facing.delta()
        } else {
            (0, 0)
        };
        let (fc, fr) = self.player.footing();
        let player_cells = [
            (fc, fr),
            (fc.wrapping_add_signed(dx), fr.wrapping_add_signed(dy)),
        ];
        self.scene.blocked(column, row)
            || self.scene.exit(column, row).is_some()
            || player_cells.contains(&(column, row))
            || self
                .npcs
                .iter()
                .enumerate()
                .any(|(other, npc)| other != index && npc.footing() == (column, row))
    }

    fn talk(&mut self) -> Option<FieldEvent> {
        let (dx, dy) = self.player.facing.delta();
        let (column, row) = self.player.footing();
        let ahead = (column.checked_add_signed(dx)?, row.checked_add_signed(dy)?);
        let index = self
            .npcs
            .iter()
            .position(|npc| npc.step == 0 && npc.footing() == ahead)?;
        let npc = &mut self.npcs[index];
        if npc.turns {
            npc.face(self.player.facing.opposite());
        }
        npc.dialogue.map(|dialogue| FieldEvent::Talk {
            npc: index,
            dialogue,
        })
    }

    fn blocked(&self, column: usize, row: usize) -> bool {
        self.scene.blocked(column, row)
            || self.npcs.iter().any(|npc| npc.footing() == (column, row))
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

    /// Follows `exit` of the current map: loads the destination with its
    /// characters and stands the player where the warp says.
    ///
    /// # Errors
    ///
    /// Returns [`FieldError`] when the warp, the map or a sprite cannot be read.
    pub fn warp(&mut self, data: &GameData<'_>, exit: usize) -> Result<Warp, FieldError> {
        let warp = data.warp(self.map, exit)?;
        let (scene, player_sheet, npcs) = load_map(data, warp.map)?;
        self.enter(scene, warp.map, &warp);
        self.sheet = player_sheet;
        self.npcs = npcs;
        Ok(warp)
    }

    /// Replaces the scene, drops the characters and places the player as
    /// `warp` says.
    pub fn enter(&mut self, scene: Scene, map: usize, warp: &Warp) {
        self.palettes = PaletteBank::from_bgr555(&scene.palettes);
        self.scene = scene;
        self.map = map;
        self.npcs.clear();
        self.player.column = warp.column;
        self.player.row = warp.row;
        self.player.step = 0;
        self.player.walking = false;
        self.player.animation = 0;
        if let Some(facing) = warp.facing.and_then(Direction::from_index) {
            self.player.facing = facing;
        }
    }

    /// Camera scroll in map pixels: the player's anchor sits at the camera
    /// anchor unless the map edge is closer.
    #[must_use]
    pub fn camera(&self) -> (usize, usize) {
        let map_width = self.scene.map.width * TILE_SIZE;
        let map_height = self.scene.map.height * TILE_SIZE;
        let max_x = map_width.saturating_sub(SCREEN_WIDTH);
        let max_y = map_height.saturating_sub(SCREEN_HEIGHT);
        let (x, y) = self.player.anchor();
        let scroll = |position: isize, anchor: isize, max: usize| {
            usize::try_from(position - anchor).unwrap_or(0).min(max)
        };
        (
            scroll(x, CAMERA_ANCHOR.0, max_x),
            scroll(y, CAMERA_ANCHOR.1, max_y),
        )
    }

    /// Frame record of the player's sheet for the current animation frame.
    #[must_use]
    pub fn player_frame(&self) -> Option<usize> {
        current_frame(
            &self.sheet,
            self.player.animation_id(),
            self.player.animation,
        )
    }

    /// Draws the scene, the characters and the player for the current frame.
    pub fn draw(&self, frame: &mut Frame) {
        let scroll = self.camera();
        let tile = |index: usize| self.scene.tiles.tile(index);
        let backdrop = |x: usize, y: usize| self.scene.backdrop.wrapping(x, y);
        draw_background(frame, backdrop, tile, &self.palettes, scroll, false);
        let map = |x: usize, y: usize| self.scene.map.wrapping(x, y);
        draw_background(frame, map, tile, &self.palettes, scroll, true);
        let mut actors = vec![(&self.sheet, self.player_frame(), self.player.anchor())];
        actors.extend(self.npcs.iter().map(|npc| {
            let frame = current_frame(&npc.sheet, npc.animation_id, npc.animation);
            (&npc.sheet, frame, npc.anchor())
        }));
        actors.sort_by_key(|(_, _, (_, y))| *y);
        for (sheet, current, (x, y)) in actors {
            let Some((record, image)) = current
                .and_then(|index| Some((sheet.frames.get(index)?, sheet.frame_image(index)?)))
            else {
                continue;
            };
            let screen = |position: isize, offset: i16, scroll: usize| {
                i32::try_from(position + isize::from(offset)).unwrap_or(i32::MAX)
                    - i32::try_from(scroll).unwrap_or(0)
            };
            draw_sprite(
                frame,
                screen(x, record.x, scroll.0),
                screen(y, record.y, scroll.1),
                &image,
                &sheet.palette,
                record.mirrored,
            );
        }
    }
}

fn load_map(data: &GameData<'_>, map: usize) -> Result<(Scene, SpriteSheet, Vec<Npc>), FieldError> {
    let scene = data.scene(data.map_record(map)?.scene)?;
    let objects = data.map_objects(map)?;
    let player_sprite = objects
        .first()
        .and_then(saga::MapObject::sprite_sheet_id)
        .unwrap_or(PLAYER_SPRITE);
    let player_sheet = data.sprite_sheet(player_sprite)?;
    let npcs = objects
        .iter()
        .skip(1)
        .filter_map(|object| {
            let sprite = object.sprite_sheet_id()?;
            Some(data.sprite_sheet(sprite).map(|sheet| Npc {
                column: object.column,
                row: object.row,
                sheet,
                animation_id: object.animation,
                animation: 0,
                dialogue: object.event_id().map(usize::from),
                turns: object.behavior < FURNITURE_BEHAVIOR,
                wanders: object.kind == WANDER_KIND,
                facing: Direction::from_index(object.animation).unwrap_or(Direction::Down),
                timer: 0,
                step: 0,
            }))
        })
        .collect::<Result<_, _>>()?;
    Ok((scene, player_sheet, npcs))
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
    use extraction::saga::{AnimationStep, SpriteFrame};
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

    /// A walking sheet laid out like the game's characters: idle
    /// animations show images `direction × 3 + [0, 1, 0, 2]`, walking ones
    /// `12 + direction × 3 + [0, 1, 0, 2]`, eight ticks each.
    fn sheet() -> SpriteSheet {
        let step = |frame: usize| AnimationStep { frame, duration: 8 };
        let animations = (0..8)
            .map(|id| [0, 1, 0, 2].map(|k| step(id * 3 + k)).to_vec())
            .collect();
        let frames = (0..24)
            .map(|index| SpriteFrame {
                tile: (index + index / 12 * 3) * 16,
                x: -16,
                y: -16,
                width: 32,
                height: 32,
                mirrored: false,
            })
            .collect();
        SpriteSheet {
            tag: "ch00".to_owned(),
            images: 0,
            tiles_per_image: 16,
            palette: [0; 16],
            tiles: Tileset::from_4bpp(&[]),
            frames,
            animations,
        }
    }

    /// The player standing on `(3, 1)` of a `columns`×`rows` scene.
    fn field(columns: usize, rows: usize) -> Field {
        Field::new(scene(columns, rows), sheet(), (3, 1))
    }

    fn held(direction: Direction) -> Input {
        Input::default().with(direction.button())
    }

    fn sprite_top_left(player: &Player) -> (isize, isize) {
        let (x, y) = player.anchor();
        (x - 16, y - 16)
    }

    #[test]
    fn steps_a_whole_metatile_once_started() {
        let mut field = field(6, 5);
        assert_eq!(sprite_top_left(&field.player), (40, 16));
        field.update(held(Direction::Right));
        assert_eq!(sprite_top_left(&field.player), (41, 16));
        assert!(field.player.walking);
        for _ in 0..14 {
            field.update(Input::default());
        }
        assert_eq!(sprite_top_left(&field.player), (55, 16));
        assert!(field.player.walking);
        field.update(Input::default());
        assert_eq!(sprite_top_left(&field.player), (56, 16));
        assert_eq!((field.player.column, field.player.row), (4, 1));
        assert!(!field.player.walking);
        field.update(Input::default());
        assert_eq!(sprite_top_left(&field.player), (56, 16));
    }

    #[test]
    fn keeps_stepping_while_held_and_stops_at_walls() {
        let mut field = field(6, 4);
        for _ in 0..40 {
            field.update(held(Direction::Right));
        }
        assert_eq!((field.player.column, field.player.row), (4, 1));
        assert_eq!(field.player.facing, Direction::Right);
        assert!(!field.player.walking);
        field.update(held(Direction::Down));
        assert_eq!(field.player.facing, Direction::Down);
        assert!(!field.player.walking);
    }

    #[test]
    fn characters_block_the_player() {
        let mut field = field(6, 5);
        field.npcs.push(npc(4, 1, Some(7), true));
        field.update(held(Direction::Right));
        assert!(!field.player.walking);
        assert_eq!(field.player.facing, Direction::Right);
        field.update(held(Direction::Down));
        assert!(field.player.walking);
    }

    fn npc(column: usize, row: usize, dialogue: Option<usize>, turns: bool) -> Npc {
        Npc {
            column,
            row,
            sheet: sheet(),
            animation_id: 1,
            animation: 9,
            dialogue,
            turns,
            wanders: false,
            facing: Direction::Down,
            timer: 0,
            step: 0,
        }
    }

    fn wanderer(column: usize, row: usize) -> Npc {
        Npc {
            wanders: true,
            ..npc(column, row, None, true)
        }
    }

    #[test]
    fn wandering_characters_step_when_the_cell_ahead_is_free() {
        let mut field = field(8, 8);
        field.player.column = 6;
        field.player.row = 5;
        let mut walker = wanderer(3, 3);
        walker.timer = 2;
        field.npcs.push(walker);
        field.update(Input::default());
        field.update(Input::default());
        assert_eq!(field.npcs[0].timer, 0);
        assert_eq!(field.npcs[0].step, 0);
        field.update(Input::default());
        let npc = &field.npcs[0];
        assert!(npc.step == 1 || npc.timer > 0);
        let stepped = npc.step == 1;
        let facing = npc.facing;
        if stepped {
            assert_eq!(npc.animation_id, WALK_ANIMATION_BASE + facing.index());
            let (dx, dy) = facing.delta();
            assert_eq!(
                (npc.column, npc.row),
                (
                    3usize.wrapping_add_signed(dx),
                    3usize.wrapping_add_signed(dy)
                )
            );
            let (ax, ay) = npc.anchor();
            let (tx, ty) = anchor(npc.column, npc.row);
            assert_eq!((ax, ay), (tx - dx * 15, ty - dy * 15));
            for _ in 0..31 {
                field.update(Input::default());
            }
            let npc = &field.npcs[0];
            assert_eq!(npc.step, 0);
            assert_eq!(npc.animation_id, facing.index());
            assert_eq!(npc.anchor(), anchor(npc.column, npc.row));
            assert!(npc.timer <= IDLE_TIMER_MASK);
        } else {
            assert_eq!(npc.animation_id, facing.index());
        }
    }

    #[test]
    fn wandering_characters_only_turn_when_walled_in() {
        let mut field = field(3, 4);
        field.player.column = 1;
        field.player.row = 2;
        field.npcs.push(wanderer(1, 0));
        for _ in 0..400 {
            field.update(Input::default());
            let npc = &field.npcs[0];
            assert_eq!((npc.column, npc.row, npc.step), (1, 0, 0));
        }
        let mut furniture = wanderer(1, 0);
        furniture.wanders = false;
        field.npcs[0] = furniture;
        for _ in 0..200 {
            field.update(Input::default());
        }
        assert_eq!(field.npcs[0].facing, Direction::Down);
        assert_eq!(field.npcs[0].timer, 0);
    }

    #[test]
    fn cutscene_commands_turn_and_step_characters() {
        let mut field = field(8, 8);
        field.player.column = 6;
        field.player.row = 5;
        field.npcs.push(npc(3, 3, None, true));
        assert!(field.command_npc(0, NpcCommand::Face(Direction::Left)));
        assert_eq!(field.npcs[0].facing, Direction::Left);
        assert!(!field.command_npc(0, NpcCommand::Step(Direction::Up)) || field.npcs[0].step == 1);
        assert!(field.command_npc(0, NpcCommand::Step(Direction::Right)) || field.npcs[0].step > 0);
        assert!(!field.npc_idle(0));
        for _ in 0..NPC_STEP_FRAMES {
            field.update(Input::default());
        }
        assert!(field.npc_idle(0));
        assert!(!field.command_npc(5, NpcCommand::Face(Direction::Up)));
        assert!(field.npc_idle(5));
    }

    #[test]
    fn a_stepping_character_blocks_its_destination_and_cannot_be_talked_to() {
        let mut field = field(6, 5);
        let mut walker = wanderer(4, 1);
        walker.step = 5;
        walker.facing = Direction::Down;
        field.npcs.push(walker);
        field.update(held(Direction::Right));
        assert!(!field.player.walking);
        assert_eq!(field.update(Input::default().with(Button::A)), None);
    }

    #[test]
    fn pressing_a_toward_a_character_turns_it_and_reports_its_dialogue() {
        let mut field = field(6, 5);
        field.npcs.push(npc(4, 1, Some(738), true));
        field.update(held(Direction::Right));
        let a = Input::default().with(Button::A);
        assert_eq!(
            field.update(a),
            Some(FieldEvent::Talk {
                npc: 0,
                dialogue: 738
            })
        );
        assert_eq!(field.npcs[0].animation_id, 2);
        assert_eq!(field.npcs[0].animation, 0);
        assert_eq!(field.update(a), None);
        assert_eq!(field.update(Input::default()), None);
        field.update(held(Direction::Up));
        assert!(field.player.walking);
        for _ in 0..16 {
            field.update(Input::default());
        }
        assert_eq!(field.update(a), None);
    }

    #[test]
    fn furniture_and_silent_characters_do_not_talk() {
        let mut field = field(6, 5);
        field.npcs.push(npc(4, 1, None, true));
        field.update(held(Direction::Right));
        assert_eq!(field.update(Input::default().with(Button::A)), None);
        assert_eq!(field.npcs[0].animation_id, 2);
        field.npcs[0] = npc(4, 1, Some(1), false);
        field.update(Input::default());
        assert_eq!(
            field.update(Input::default().with(Button::A)),
            Some(FieldEvent::Talk {
                npc: 0,
                dialogue: 1
            })
        );
        assert_eq!(field.npcs[0].animation_id, 1);
    }

    #[test]
    fn reports_the_exit_when_the_step_onto_it_completes() {
        let mut field = field(6, 5);
        let mut exits = Vec::new();
        for _ in 0..32 {
            exits.extend(field.update(held(Direction::Left)));
        }
        assert_eq!(exits, [FieldEvent::Exit(1)]);
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
        assert_eq!(sprite_top_left(&field.player), (24, 32));
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
    fn plays_the_sheet_animations_at_half_speed() {
        let mut field = field(6, 5);
        assert_eq!(field.player_frame(), Some(3));
        field.update(held(Direction::Right));
        assert_eq!(field.player_frame(), Some(21));
        for _ in 0..4 {
            field.update(held(Direction::Right));
        }
        assert_eq!(field.player_frame(), Some(22));
        for _ in 0..8 {
            field.update(held(Direction::Right));
        }
        assert_eq!(field.player_frame(), Some(23));
        let sheet = sheet();
        assert_eq!(current_frame(&sheet, 2, 16), Some(6));
        assert_eq!(current_frame(&sheet, 8, 0), None);
    }

    #[test]
    fn camera_keeps_the_sprite_at_the_anchor_within_bounds() {
        let mut large = field(40, 20);
        assert_eq!(large.camera(), (0, 0));
        large.player.column = 13;
        large.player.row = 7;
        assert_eq!(sprite_top_left(&large.player), (200, 112));
        assert_eq!(large.camera(), (96, 48));
        large.player.column = 1000;
        large.player.row = 1000;
        assert_eq!(large.camera(), (640 - 240, 320 - 160));
        assert_eq!(field(6, 5).camera(), (0, 0));
    }
}
