//! The field: a scene with actors on it, the player among them, and the
//! camera following the player.
//!
//! Actors mirror the game's entity table: one per object of the map's list,
//! object 0 being the player, each standing on a 16×16 metatile with its
//! sprite's top-left at `(16 × column − 8, 16 × row)` in 16.16 fixed-point
//! pixels. Every frame each actor, in object order, either continues the
//! step it is taking or runs its command, which may start one: the
//! player's command reads the buttons, a wanderer picks a random direction
//! now and then, and a walk heads for a metatile one step at a time. A step
//! moves the metatile at once and the sprite over the following frames at
//! the step's speed (16 frames at one pixel per frame for the player, 32 at
//! half for characters); its first frame already moves. Steps are blocked
//! by the scene's attributes under the footing (the metatile below the
//! standing one) and by other visible actors' footings, both the one they
//! stand on and, while stepping, the one they left; a walk through
//! (command 11) ignores both. Animations come from the sprite sheet: idle
//! animation = facing, walking = facing + 4, restarted by every step and
//! turn, each tick shortened by the actor's animation shift (1 halves them:
//! the player's steps last one walking cycle). Actors are drawn in
//! back-to-front order of their anchor, the bottom center of the footing.
//! Completing a step onto an exit reports it; pressing A while standing and
//! facing an actor turns it toward the player (characters only) and reports
//! what it runs.

use extraction::saga::{
    MapError, MapObject, ObjectScript, PLAYER_SPRITE, Scene, SceneError, SpriteSheet,
    SpriteSheetError, WALK_ANIMATION_BASE, Warp,
};
use gba_runtime::ppu::{PaletteBank, SCREEN_HEIGHT, SCREEN_WIDTH, draw_background};
use platform::{Button, Frame, Input};
use thiserror::Error;

use crate::data::GameData;
use crate::draw_sprite;
use crate::rng::Rng;

const TILE_SIZE: usize = 8;
/// Side of a room's cell in pixels; Zoid maps use cells twice as large.
pub const ROOM_CELL: i32 = 16;
const SPRITE_SIDE: i32 = 32;
const FRACTION_BITS: u32 = 16;
/// One pixel in the actors' fixed-point positions.
pub const PIXEL: i32 = 1 << FRACTION_BITS;
/// The player's speed: a pixel a frame.
pub const PLAYER_SPEED: i32 = PIXEL;
/// A wandering character's speed: half a pixel a frame.
pub const WANDER_SPEED: i32 = PIXEL / 2;
const ANCHOR_FROM_ORIGIN: (isize, isize) = (16, 16);
const CAMERA_ANCHOR: (isize, isize) = (120, 80);
const PLAYER_ANIMATION_SHIFT: i8 = 1;
const CHARACTER_BEHAVIOR: u16 = 0;
const TURNING_BEHAVIORS: u16 = 2;
const PASSABLE_BEHAVIOR: u16 = 3;
const SILENT_BEHAVIOR: u16 = 5;
const PLAYER_KIND: u16 = 0;
const WANDER_KIND: u16 = 2;
const IDLE_TIMER_MASK: u16 = 0x7F;

/// Where an actor faces, in the order the sprite sheet uses.
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

    /// The sprite sheet's number for this direction.
    #[must_use]
    pub const fn index(self) -> usize {
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
    /// The player faced chest `chest` (actor `actor`) and pressed A.
    Chest {
        /// Index into the field's actors.
        actor: usize,
        /// The chest's number.
        chest: u16,
    },
    /// The player spoke to this actor, which runs `script`.
    Talk {
        /// Index into the field's actors (the object's index).
        actor: usize,
        /// What it runs.
        script: ObjectScript,
    },
}

/// A walk toward a metatile, as cutscenes give it (the game's commands 10
/// and 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Walk {
    /// Destination column.
    pub column: usize,
    /// Destination row.
    pub row: usize,
    /// Pixels a frame, in 16.16 fixed point.
    pub speed: i32,
    /// Shift applied to the animation's ticks while walking.
    pub animation_shift: i8,
    /// Whether it walks through walls and actors (command 11).
    pub through: bool,
}

/// What an actor does when it is not stepping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Moves with the buttons (command 0).
    Player,
    /// Stands still (command 1); a walk becomes this once it arrives.
    Idle,
    /// Walks around at random (command 2).
    Wander,
    /// Heads for a metatile, the longer axis first, horizontally on ties.
    WalkTo(Walk),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step {
    direction: Direction,
    frames: i32,
    speed: i32,
}

/// Someone or something standing on the map: the player or an object of
/// the map's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    /// Metatile column it stands on (a step's destination from its start).
    pub column: usize,
    /// Metatile row it stands on.
    pub row: usize,
    previous: (usize, usize),
    x: i32,
    y: i32,
    size: i32,
    /// Its sprite; `None` for invisible objects.
    pub sheet: Option<SpriteSheet>,
    /// Whether it is drawn and blocks others.
    pub visible: bool,
    /// Where it faces.
    pub facing: Direction,
    /// Sheet animation it plays.
    pub animation_id: usize,
    /// Frames into the animation, the frame that started it counting one.
    pub animation: u32,
    /// Shift applied to the animation's ticks (1 halves them).
    pub animation_shift: i8,
    /// What it does when standing.
    pub command: Command,
    step: Option<Step>,
    /// What it runs when spoken to.
    pub script: Option<ObjectScript>,
    /// The object's behavior: 0 characters, 1 map Zoids, 2 furniture, 3
    /// passable, 4 chests, 5 silent.
    pub behavior: u16,
    /// Frames left before a wanderer acts again.
    pub timer: u16,
    /// The chest's number, for a chest.
    pub chest: Option<u16>,
    /// Whether the animation stops on its last step instead of looping.
    pub once: bool,
    /// A step of the animation shown without animating.
    pub pose: Option<usize>,
}

impl Actor {
    /// An actor standing on `(column, row)` facing `facing`.
    #[must_use]
    pub fn new(
        sheet: Option<SpriteSheet>,
        (column, row): (usize, usize),
        facing: Direction,
    ) -> Self {
        let (x, y) = origin(column, row, ROOM_CELL);
        Self {
            column,
            row,
            previous: (column, row),
            x,
            y,
            size: ROOM_CELL,
            sheet,
            visible: true,
            facing,
            animation_id: facing.index(),
            animation: 0,
            animation_shift: PLAYER_ANIMATION_SHIFT,
            command: Command::Idle,
            step: None,
            script: None,
            behavior: CHARACTER_BEHAVIOR,
            timer: 0,
            chest: None,
            once: false,
            pose: None,
        }
    }

    fn from_object(sheet: Option<SpriteSheet>, object: &MapObject) -> Self {
        let facing = Direction::from_index(object.animation).unwrap_or(Direction::Down);
        let mut actor = Self::new(sheet, (object.column, object.row), facing);
        actor.animation_id = object.animation;
        actor.script = object.script_kind();
        actor.chest = object.chest();
        actor.behavior = object.behavior;
        actor.command = match object.kind {
            PLAYER_KIND => Command::Player,
            WANDER_KIND => Command::Wander,
            _ => Command::Idle,
        };
        actor
    }

    /// The cell its collision box sits on: the bottom half of its 32×32
    /// sprite, below the standing cell in rooms, the standing cell itself
    /// on Zoid maps.
    #[must_use]
    pub fn footing(&self) -> (usize, usize) {
        (self.column, self.row + self.footing_offset())
    }

    fn previous_footing(&self) -> (usize, usize) {
        (self.previous.0, self.previous.1 + self.footing_offset())
    }

    fn footing_offset(&self) -> usize {
        usize::try_from(SPRITE_SIDE / self.size.max(1) - 1).unwrap_or(0)
    }

    /// Uses cells of `size` pixels, as its map does, standing where it is.
    pub fn set_cell_size(&mut self, size: i32) {
        self.size = size;
        self.place((self.column, self.row));
    }

    /// Whether it is taking a step.
    #[must_use]
    pub fn walking(&self) -> bool {
        self.step.is_some()
    }

    /// Top-left of its 32×32 box in map pixels.
    #[must_use]
    pub fn position(&self) -> (isize, isize) {
        let pixels = |value: i32| isize::try_from(value >> FRACTION_BITS).unwrap_or(0);
        (pixels(self.x), pixels(self.y))
    }

    /// Anchor in map pixels: the bottom center of its footing, moved along
    /// the step in progress.
    #[must_use]
    pub fn anchor(&self) -> (isize, isize) {
        let (x, y) = self.position();
        (x + ANCHOR_FROM_ORIGIN.0, y + ANCHOR_FROM_ORIGIN.1)
    }

    /// Turns toward `direction` and plays its idle animation from the start.
    pub fn face(&mut self, direction: Direction) {
        self.facing = direction;
        self.play(direction.index());
    }

    /// Plays animation `id` from the start.
    pub fn play(&mut self, id: usize) {
        self.animation_id = id;
        self.animation = 0;
    }

    /// Stands it on `(column, row)`, ending any step.
    pub fn place(&mut self, (column, row): (usize, usize)) {
        self.column = column;
        self.row = row;
        self.previous = (column, row);
        (self.x, self.y) = origin(column, row, self.size);
        self.step = None;
    }

    /// Moves its sprite by `(dx, dy)` pixels without changing its metatile.
    pub fn nudge(&mut self, (dx, dy): (i32, i32)) {
        self.x += dx * PIXEL;
        self.y += dy * PIXEL;
    }

    /// Puts its sprite's top-left at map pixel `(x, y)` without changing its
    /// metatile.
    pub fn set_position(&mut self, (x, y): (i32, i32)) {
        self.x = x * PIXEL;
        self.y = y * PIXEL;
    }

    /// Plays animation `id` once from the start, stopping on its last step.
    pub fn play_once(&mut self, id: usize) {
        self.once = true;
        self.pose = None;
        self.play(id);
    }

    /// Whether an animation played once has reached its end.
    #[must_use]
    pub fn animation_done(&self) -> bool {
        let Some(sheet) = self.sheet.as_ref() else {
            return true;
        };
        self.once
            && animation_length(sheet, self.animation_id, self.animation_shift)
                .is_some_and(|length| self.animation >= length)
    }

    /// Whether a walk has arrived or it otherwise stands idle.
    #[must_use]
    pub fn idle(&self) -> bool {
        self.command == Command::Idle && self.step.is_none()
    }

    fn start_step(&mut self, direction: Direction, speed: i32) {
        let (dx, dy) = direction.delta();
        self.previous = (self.column, self.row);
        self.column = self.column.saturating_add_signed(dx);
        self.row = self.row.saturating_add_signed(dy);
        self.facing = direction;
        self.play(WALK_ANIMATION_BASE + direction.index());
        self.animation_shift = PLAYER_ANIMATION_SHIFT;
        self.step = Some(Step {
            direction,
            frames: self.size * PIXEL / speed.max(1),
            speed,
        });
        self.advance_step();
    }

    fn advance_step(&mut self) -> bool {
        let Some(step) = self.step.as_mut() else {
            return false;
        };
        let (dx, dy) = step.direction.delta();
        let delta = |d: isize| i32::try_from(d).unwrap_or(0) * step.speed;
        self.x += delta(dx);
        self.y += delta(dy);
        step.frames -= 1;
        if step.frames > 0 {
            return false;
        }
        self.step = None;
        self.previous = (self.column, self.row);
        (self.x, self.y) = origin(self.column, self.row, self.size);
        self.face(self.facing);
        true
    }

    fn current_frame(&self) -> Option<usize> {
        let sheet = self.sheet.as_ref()?;
        let steps = sheet.animations.get(self.animation_id)?;
        if let Some(step) = self.pose {
            return steps
                .get(step)
                .or_else(|| steps.last())
                .map(|step| step.frame);
        }
        if self.animation_done() {
            return steps.last().map(|step| step.frame);
        }
        frame_at(
            sheet,
            self.animation_id,
            self.animation,
            self.animation_shift,
        )
    }
}

/// The sprite's top-left for cell `(column, row)` of `size` pixels, in
/// fixed point: centered on the cell, its top on the cell's top. The game
/// keeps cells in 16 bits, so `0xFFFE` stands two rows above the map (where
/// cutscenes park the player).
fn origin(column: usize, row: usize, size: i32) -> (i32, i32) {
    let signed = |cell: usize| {
        let cell = u16::try_from(cell & usize::from(u16::MAX)).unwrap_or(0);
        i32::from(i16::from_ne_bytes(cell.to_ne_bytes()))
    };
    (
        (signed(column) * size + size / 2 - SPRITE_SIDE / 2) * PIXEL,
        signed(row) * size * PIXEL,
    )
}

fn random_direction(value: u16) -> Direction {
    match value >> 14 {
        0 => Direction::Up,
        1 => Direction::Down,
        2 => Direction::Left,
        _ => Direction::Right,
    }
}

/// The direction a walk from `from` to `to` steps in: along the longer
/// axis, horizontally when both are as long, as the game's routine at
/// `0x0800A864` decides.
fn toward(from: (usize, usize), to: (usize, usize)) -> Option<Direction> {
    let signed = |value: usize| isize::try_from(value).unwrap_or(isize::MAX);
    let dx = signed(to.0) - signed(from.0);
    let dy = signed(to.1) - signed(from.1);
    if dx == 0 && dy == 0 {
        return None;
    }
    Some(if dx.abs() >= dy.abs() {
        if dx < 0 {
            Direction::Left
        } else {
            Direction::Right
        }
    } else if dy < 0 {
        Direction::Up
    } else {
        Direction::Down
    })
}

/// Frame record shown by `sheet`'s animation `id` after `elapsed` frames,
/// with steps lasting half their ticks (at least one frame).
#[must_use]
pub fn current_frame(sheet: &SpriteSheet, id: usize, elapsed: u32) -> Option<usize> {
    frame_at(sheet, id, elapsed, PLAYER_ANIMATION_SHIFT)
}

/// Frame record shown by `sheet`'s animation `id` after `elapsed` frames,
/// each step lasting its ticks shifted right by `shift` (left when
/// negative), at least one frame. The frame that starts an animation
/// already counts one, so its first step shows a frame shorter.
#[must_use]
pub fn frame_at(sheet: &SpriteSheet, id: usize, elapsed: u32, shift: i8) -> Option<usize> {
    let steps = sheet.animations.get(id)?;
    let length = |ticks: u32| tick_length(ticks, shift);
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

/// Frames `sheet`'s animation `id` lasts with ticks shifted by `shift`.
fn animation_length(sheet: &SpriteSheet, id: usize, shift: i8) -> Option<u32> {
    let steps = sheet.animations.get(id)?;
    Some(
        steps
            .iter()
            .map(|step| tick_length(step.duration, shift))
            .sum(),
    )
}

fn tick_length(ticks: u32, shift: i8) -> u32 {
    let scaled = if shift >= 0 {
        ticks >> shift.unsigned_abs()
    } else {
        ticks << shift.unsigned_abs()
    };
    scaled.max(1)
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

/// A scene with the player and the map's objects on it.
pub struct Field {
    map: usize,
    scene: Scene,
    palettes: PaletteBank,
    /// The actors, by object index; actor 0 is the player.
    pub actors: Vec<Actor>,
    /// How far cutscenes have scrolled the camera from the player, in
    /// 16.16 fixed-point pixels.
    pub pan: (i32, i32),
    previous: Input,
    frame: u16,
    rng: Rng,
}

impl Field {
    /// Stands the player, drawn with `sheet`, on metatile `(column, row)` of
    /// `scene`.
    #[must_use]
    pub fn new(scene: Scene, sheet: SpriteSheet, (column, row): (usize, usize)) -> Self {
        let palettes = PaletteBank::from_bgr555(&scene.palettes);
        let mut player = Actor::new(Some(sheet), (column, row), Direction::Down);
        player.command = Command::Player;
        player.set_cell_size(cell_pixels(&scene));
        Self {
            map: 0,
            scene,
            palettes,
            actors: vec![player],
            pan: (0, 0),
            previous: Input::default(),
            frame: 0,
            rng: Rng::default(),
        }
    }

    /// Loads map `map` with its objects and stands the player on
    /// `(column, row)`.
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
        let objects = data.map_objects(map)?;
        Self::load_with(data, map, (column, row), &objects)
    }

    /// Loads map `map` with the objects `objects` instead of its own list,
    /// as cutscenes do, and stands the player (object 0) on `(column, row)`.
    ///
    /// # Errors
    ///
    /// Returns [`FieldError`] when the map record, its scene or a sprite
    /// cannot be read.
    pub fn load_with(
        data: &GameData<'_>,
        map: usize,
        (column, row): (usize, usize),
        objects: &[MapObject],
    ) -> Result<Self, FieldError> {
        let scene = map_scene(data, map)?;
        let mut actors = load_actors(data, objects)?;
        for actor in &mut actors {
            actor.set_cell_size(cell_pixels(&scene));
        }
        let mut field = Self {
            map,
            palettes: PaletteBank::from_bgr555(&scene.palettes),
            scene,
            actors,
            pan: (0, 0),
            previous: Input::default(),
            frame: 0,
            rng: Rng::default(),
        };
        field.player_mut().place((column, row));
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

    /// The player.
    #[must_use]
    pub fn player(&self) -> &Actor {
        &self.actors[0]
    }

    /// The player, to move or turn.
    pub fn player_mut(&mut self) -> &mut Actor {
        &mut self.actors[0]
    }

    /// Actor `index`, if there is one.
    #[must_use]
    pub fn actor(&self, index: usize) -> Option<&Actor> {
        self.actors.get(index)
    }

    /// Actor `index`, to command.
    pub fn actor_mut(&mut self, index: usize) -> Option<&mut Actor> {
        self.actors.get_mut(index)
    }

    /// Advances one frame with the buttons held; reports an exit reached or
    /// an actor spoken to.
    pub fn update(&mut self, input: Input) -> Option<FieldEvent> {
        let pressed_a = input.is_held(Button::A) && !self.previous.is_held(Button::A);
        self.previous = input;
        self.frame = self.frame.wrapping_add(1);
        let mut event = None;
        for index in 0..self.actors.len() {
            if let Some(found) = self.update_actor(index, input, pressed_a) {
                event = Some(found);
            }
        }
        for actor in &mut self.actors {
            actor.animation = actor.animation.saturating_add(1);
        }
        event
    }

    fn update_actor(&mut self, index: usize, input: Input, pressed_a: bool) -> Option<FieldEvent> {
        let actor = &mut self.actors[index];
        if actor.step.is_some() {
            let finished = actor.advance_step();
            if finished && actor.command == Command::Wander {
                actor.timer = self.rng.next(self.frame) & IDLE_TIMER_MASK;
            }
            if finished && index == 0 && actor.command == Command::Player {
                let (column, row) = actor.footing();
                return self.scene.exit(column, row).map(FieldEvent::Exit);
            }
            return None;
        }
        let event = match actor.command {
            Command::Player => self.update_player(input, pressed_a),
            Command::Idle => None,
            Command::Wander => {
                self.wander(index);
                None
            }
            Command::WalkTo(walk) => {
                self.walk(index, walk);
                None
            }
        };
        let actor = &mut self.actors[index];
        if actor.step.is_none() {
            actor.animation_shift = if actor.behavior == CHARACTER_BEHAVIOR {
                PLAYER_ANIMATION_SHIFT
            } else {
                0
            };
        }
        event
    }

    fn update_player(&mut self, input: Input, pressed_a: bool) -> Option<FieldEvent> {
        if pressed_a {
            return self.talk();
        }
        let direction = Direction::from_input(input)?;
        let player = &mut self.actors[0];
        if player.facing != direction {
            player.face(direction);
        }
        if self.free(0, direction, false) {
            self.actors[0].start_step(direction, PLAYER_SPEED);
        }
        None
    }

    fn wander(&mut self, index: usize) {
        if self.actors[index].timer > 0 {
            self.actors[index].timer -= 1;
            return;
        }
        self.rng.seed(self.frame);
        let direction = random_direction(self.rng.next(self.frame));
        let free = self.free(index, direction, true);
        let actor = &mut self.actors[index];
        actor.face(direction);
        if free {
            actor.start_step(direction, WANDER_SPEED);
        } else {
            actor.timer = self.rng.next(self.frame) & IDLE_TIMER_MASK;
        }
    }

    fn walk(&mut self, index: usize, walk: Walk) {
        let actor = &self.actors[index];
        let Some(direction) = toward((actor.column, actor.row), (walk.column, walk.row)) else {
            self.actors[index].command = Command::Idle;
            return;
        };
        let free = walk.through || self.free(index, direction, false);
        let actor = &mut self.actors[index];
        if free {
            actor.start_step(direction, walk.speed);
            actor.animation_shift = walk.animation_shift;
        } else if actor.facing != direction {
            actor.face(direction);
        }
    }

    /// Whether actor `index` may step toward `direction`: the footing ahead
    /// is inside the scene and not blocked, no other visible actor stands
    /// or is stepping out of it, and, for wanderers, it is not an exit.
    fn free(&self, index: usize, direction: Direction, avoid_exits: bool) -> bool {
        let (dx, dy) = direction.delta();
        let (column, row) = self.actors[index].footing();
        let (Some(column), Some(row)) = (column.checked_add_signed(dx), row.checked_add_signed(dy))
        else {
            return false;
        };
        if self.scene.blocked(column, row) || avoid_exits && self.scene.exit(column, row).is_some()
        {
            return false;
        }
        !self.actors.iter().enumerate().any(|(other, actor)| {
            other != index
                && actor.visible
                && actor.behavior != PASSABLE_BEHAVIOR
                && (actor.footing() == (column, row) || actor.previous_footing() == (column, row))
        })
    }

    fn talk(&mut self) -> Option<FieldEvent> {
        let player = &self.actors[0];
        let (dx, dy) = player.facing.delta();
        let (column, row) = player.footing();
        let ahead = (column.checked_add_signed(dx)?, row.checked_add_signed(dy)?);
        let facing = player.facing;
        let index = (1..self.actors.len()).find(|&index| {
            let actor = &self.actors[index];
            actor.visible && actor.step.is_none() && actor.footing() == ahead
        })?;
        let actor = &mut self.actors[index];
        if let Some(chest) = actor.chest {
            return Some(FieldEvent::Chest {
                actor: index,
                chest,
            });
        }
        if actor.behavior == SILENT_BEHAVIOR {
            return None;
        }
        if actor.behavior < TURNING_BEHAVIORS {
            actor.face(facing.opposite());
        }
        actor.script.map(|script| FieldEvent::Talk {
            actor: index,
            script,
        })
    }

    /// Follows `exit` of the current map: loads the destination with its
    /// objects and stands the player where the warp says.
    ///
    /// # Errors
    ///
    /// Returns [`FieldError`] when the warp, the map or a sprite cannot be read.
    pub fn warp(&mut self, data: &GameData<'_>, exit: usize) -> Result<Warp, FieldError> {
        let warp = data.warp(self.map, exit)?;
        let scene = map_scene(data, warp.map)?;
        let objects = data.map_objects(warp.map)?;
        let mut actors = load_actors(data, &objects)?;
        for actor in &mut actors {
            actor.set_cell_size(cell_pixels(&scene));
        }
        if let (Some(arriving), Some(player)) = (actors.first_mut(), self.actors.first()) {
            arriving.facing = player.facing;
        }
        self.actors = actors;
        self.enter(scene, warp.map, &warp);
        Ok(warp)
    }

    /// Replaces the scene and places the player as `warp` says, keeping the
    /// actors.
    pub fn enter(&mut self, scene: Scene, map: usize, warp: &Warp) {
        self.palettes = PaletteBank::from_bgr555(&scene.palettes);
        let size = cell_pixels(&scene);
        self.scene = scene;
        self.map = map;
        for actor in &mut self.actors {
            actor.set_cell_size(size);
        }
        let player = self.player_mut();
        player.place((warp.column, warp.row));
        player.command = Command::Player;
        let facing = warp
            .facing
            .and_then(Direction::from_index)
            .unwrap_or(player.facing);
        player.face(facing);
    }

    /// Camera scroll in map pixels: the player's anchor sits at the camera
    /// anchor unless the map edge is closer, moved by the cutscene pan.
    #[must_use]
    pub fn camera(&self) -> (usize, usize) {
        let map_width = self.scene.map.width * TILE_SIZE;
        let map_height = self.scene.map.height * TILE_SIZE;
        let max_x = map_width.saturating_sub(SCREEN_WIDTH);
        let max_y = map_height.saturating_sub(SCREEN_HEIGHT);
        let (x, y) = self.player().anchor();
        let scroll = |position: isize, anchor: isize, max: usize| {
            usize::try_from(position - anchor).unwrap_or(0).min(max)
        };
        let panned = |base: usize, pan: i32, max: usize| {
            let offset = isize::try_from(pan >> FRACTION_BITS).unwrap_or(0);
            base.saturating_add_signed(offset).min(max)
        };
        (
            panned(scroll(x, CAMERA_ANCHOR.0, max_x), self.pan.0, max_x),
            panned(scroll(y, CAMERA_ANCHOR.1, max_y), self.pan.1, max_y),
        )
    }

    /// Frame record of the player's sheet for the current animation frame.
    #[must_use]
    pub fn player_frame(&self) -> Option<usize> {
        self.player().current_frame()
    }

    /// Draws the scene and the visible actors for the current frame.
    pub fn draw(&self, frame: &mut Frame) {
        let scroll = self.camera();
        let tile = |index: usize| self.scene.tiles.tile(index);
        let backdrop = |x: usize, y: usize| self.scene.backdrop.wrapping(x, y);
        draw_background(frame, backdrop, tile, &self.palettes, scroll, false);
        let map = |x: usize, y: usize| self.scene.map.wrapping(x, y);
        draw_background(frame, map, tile, &self.palettes, scroll, true);
        let mut actors: Vec<(usize, &Actor)> = self
            .actors
            .iter()
            .enumerate()
            .filter(|(_, actor)| actor.visible && actor.sheet.is_some())
            .collect();
        actors.sort_by_key(|(index, actor)| (actor.anchor().1, usize::from(*index == 0)));
        for (_, actor) in actors {
            let Some(sheet) = actor.sheet.as_ref() else {
                continue;
            };
            let Some((record, image)) = actor
                .current_frame()
                .and_then(|index| Some((sheet.frames.get(index)?, sheet.frame_image(index)?)))
            else {
                continue;
            };
            let (x, y) = actor.anchor();
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

/// Map `map`'s scene with the attribute grid its record sets.
fn map_scene(data: &GameData<'_>, map: usize) -> Result<Scene, FieldError> {
    let record = data.map_record(map)?;
    Ok(data
        .scene(record.scene)?
        .with_cell_tiles(record.metatile_tiles))
}

fn cell_pixels(scene: &Scene) -> i32 {
    i32::try_from(scene.cell_tiles * TILE_SIZE).unwrap_or(ROOM_CELL)
}

fn load_actors(data: &GameData<'_>, objects: &[MapObject]) -> Result<Vec<Actor>, FieldError> {
    let mut actors = objects
        .iter()
        .map(|object| {
            let sheet = object
                .sprite_sheet_id()
                .map(|sprite| data.sprite_sheet(sprite))
                .transpose()?;
            Ok(Actor::from_object(sheet, object))
        })
        .collect::<Result<Vec<_>, FieldError>>()?;
    if actors.is_empty() {
        let sheet = data.sprite_sheet(PLAYER_SPRITE)?;
        let mut player = Actor::new(Some(sheet), (0, 0), Direction::Down);
        player.command = Command::Player;
        actors.push(player);
    }
    let player = &mut actors[0];
    if player.sheet.is_none() {
        player.sheet = Some(data.sprite_sheet(PLAYER_SPRITE)?);
    }
    player.script = None;
    Ok(actors)
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
    use extraction::saga::{AnimationStep, METATILE_TILES, SpriteFrame};
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
            cell_tiles: METATILE_TILES,
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

    fn character(column: usize, row: usize, script: Option<ObjectScript>, behavior: u16) -> Actor {
        let mut actor = Actor::new(Some(sheet()), (column, row), Direction::Down);
        actor.script = script;
        actor.behavior = behavior;
        actor
    }

    fn wanderer(column: usize, row: usize) -> Actor {
        let mut actor = character(column, row, None, 0);
        actor.command = Command::Wander;
        actor
    }

    #[test]
    fn steps_a_whole_metatile_once_started() {
        let mut field = field(6, 5);
        assert_eq!(field.player().position(), (40, 16));
        field.update(held(Direction::Right));
        assert_eq!(field.player().position(), (41, 16));
        assert!(field.player().walking());
        for _ in 0..14 {
            field.update(Input::default());
        }
        assert_eq!(field.player().position(), (55, 16));
        assert!(field.player().walking());
        field.update(Input::default());
        assert_eq!(field.player().position(), (56, 16));
        assert_eq!((field.player().column, field.player().row), (4, 1));
        assert!(!field.player().walking());
        field.update(Input::default());
        assert_eq!(field.player().position(), (56, 16));
    }

    #[test]
    fn keeps_stepping_while_held_and_stops_at_walls() {
        let mut field = field(6, 4);
        for _ in 0..40 {
            field.update(held(Direction::Right));
        }
        assert_eq!((field.player().column, field.player().row), (4, 1));
        assert_eq!(field.player().facing, Direction::Right);
        assert!(!field.player().walking());
        field.update(held(Direction::Down));
        assert_eq!(field.player().facing, Direction::Down);
        assert!(!field.player().walking());
    }

    #[test]
    fn actors_block_the_player_unless_hidden_or_passable() {
        let mut field = field(6, 5);
        field.actors.push(character(4, 1, None, 0));
        field.update(held(Direction::Right));
        assert!(!field.player().walking());
        assert_eq!(field.player().facing, Direction::Right);
        field.actors[1].visible = false;
        field.update(held(Direction::Right));
        assert!(field.player().walking());
        let mut field = self::field(6, 5);
        field.actors.push(character(4, 1, None, PASSABLE_BEHAVIOR));
        field.update(held(Direction::Right));
        assert!(field.player().walking());
    }

    #[test]
    fn half_speed_steps_take_32_frames_and_floor_their_pixels() {
        let mut field = field(8, 8);
        field.player_mut().place((6, 5));
        let mut walker = character(3, 3, None, 0);
        walker.command = Command::WalkTo(Walk {
            column: 2,
            row: 3,
            speed: WANDER_SPEED,
            animation_shift: 0,
            through: false,
        });
        field.actors.push(walker);
        field.update(Input::default());
        assert_eq!(field.actors[1].position(), (39, 48));
        assert_eq!(field.actors[1].column, 2);
        assert_eq!(field.actors[1].animation_id, WALK_ANIMATION_BASE + 2);
        for _ in 0..30 {
            field.update(Input::default());
        }
        assert_eq!(field.actors[1].position(), (24, 48));
        assert!(field.actors[1].walking());
        field.update(Input::default());
        assert_eq!(field.actors[1].position(), (24, 48));
        assert!(!field.actors[1].walking());
        assert_eq!(field.actors[1].animation_id, 2);
        assert_eq!(
            field.actors[1].command,
            Command::WalkTo(Walk {
                column: 2,
                row: 3,
                speed: WANDER_SPEED,
                animation_shift: 0,
                through: false,
            })
        );
        field.update(Input::default());
        assert!(field.actors[1].idle());
        let mut right = self::field(8, 8);
        right.player_mut().place((6, 5));
        let mut walker = character(2, 3, None, 0);
        walker.command = Command::WalkTo(Walk {
            column: 3,
            row: 3,
            speed: WANDER_SPEED,
            animation_shift: 0,
            through: false,
        });
        right.actors.push(walker);
        right.update(Input::default());
        assert_eq!(right.actors[1].position(), (24, 48));
        right.update(Input::default());
        assert_eq!(right.actors[1].position(), (25, 48));
    }

    #[test]
    fn walks_take_the_longer_axis_first_and_horizontal_on_ties() {
        assert_eq!(toward((5, 4), (2, 8)), Some(Direction::Down));
        assert_eq!(toward((5, 5), (2, 8)), Some(Direction::Left));
        assert_eq!(toward((2, 2), (2, 2)), None);
        assert_eq!(toward((2, 2), (2, 0)), Some(Direction::Up));
        assert_eq!(toward((2, 2), (4, 2)), Some(Direction::Right));
    }

    #[test]
    fn walks_through_ignore_walls() {
        let mut field = field(6, 5);
        let mut walker = character(3, 2, None, 0);
        walker.command = Command::WalkTo(Walk {
            column: 3,
            row: 0,
            speed: PLAYER_SPEED,
            animation_shift: 1,
            through: true,
        });
        field.player_mut().place((1, 1));
        field.actors.push(walker);
        for _ in 0..40 {
            field.update(Input::default());
        }
        assert_eq!((field.actors[1].column, field.actors[1].row), (3, 0));
        assert!(field.actors[1].idle());
    }

    #[test]
    fn wandering_characters_step_when_the_cell_ahead_is_free() {
        let mut field = field(8, 8);
        field.player_mut().place((6, 5));
        let mut walker = wanderer(3, 3);
        walker.timer = 2;
        field.actors.push(walker);
        field.update(Input::default());
        field.update(Input::default());
        assert_eq!(field.actors[1].timer, 0);
        assert!(!field.actors[1].walking());
        field.update(Input::default());
        let actor = &field.actors[1];
        assert!(actor.walking() || actor.timer > 0);
        if actor.walking() {
            let facing = actor.facing;
            assert_eq!(actor.animation_id, WALK_ANIMATION_BASE + facing.index());
            let (dx, dy) = facing.delta();
            assert_eq!(
                (actor.column, actor.row),
                (
                    3usize.wrapping_add_signed(dx),
                    3usize.wrapping_add_signed(dy)
                )
            );
            for _ in 0..31 {
                field.update(Input::default());
            }
            let actor = &field.actors[1];
            assert!(!actor.walking());
            assert_eq!(actor.animation_id, facing.index());
            assert!(actor.timer <= IDLE_TIMER_MASK);
        } else {
            assert_eq!(actor.animation_id, actor.facing.index());
        }
    }

    #[test]
    fn wandering_characters_only_turn_when_walled_in() {
        let mut field = field(3, 4);
        field.player_mut().place((1, 2));
        field.actors.push(wanderer(1, 0));
        for _ in 0..400 {
            field.update(Input::default());
            let actor = &field.actors[1];
            assert_eq!((actor.column, actor.row), (1, 0));
            assert!(!actor.walking());
        }
    }

    #[test]
    fn a_stepping_actor_blocks_both_cells_and_cannot_be_talked_to() {
        let mut field = field(6, 5);
        let mut walker = character(4, 1, Some(ObjectScript::Dialogue(1)), 0);
        walker.command = Command::WalkTo(Walk {
            column: 4,
            row: 2,
            speed: WANDER_SPEED,
            animation_shift: 0,
            through: false,
        });
        field.actors.push(walker);
        field.update(Input::default());
        assert!(field.actors[1].walking());
        field.update(held(Direction::Right));
        assert!(!field.player().walking());
        assert_eq!(field.update(Input::default().with(Button::A)), None);
    }

    #[test]
    fn pressing_a_toward_an_actor_turns_it_and_reports_its_script() {
        let mut field = field(6, 5);
        field
            .actors
            .push(character(4, 1, Some(ObjectScript::Dialogue(738)), 0));
        field.update(held(Direction::Right));
        let a = Input::default().with(Button::A);
        assert_eq!(
            field.update(a),
            Some(FieldEvent::Talk {
                actor: 1,
                script: ObjectScript::Dialogue(738)
            })
        );
        assert_eq!(field.actors[1].animation_id, 2);
        assert_eq!(field.update(a), None);
        assert_eq!(field.update(Input::default()), None);
        field.update(held(Direction::Up));
        assert_eq!(field.player().facing, Direction::Up);
        for _ in 0..16 {
            field.update(Input::default());
        }
        assert_eq!(field.update(a), None);
    }

    #[test]
    fn furniture_does_not_turn_and_silent_or_scriptless_actors_do_not_talk() {
        let mut field = field(6, 5);
        field.actors.push(character(4, 1, None, 0));
        field.update(held(Direction::Right));
        assert_eq!(field.update(Input::default().with(Button::A)), None);
        assert_eq!(field.actors[1].animation_id, 2);
        field.actors[1] = character(4, 1, Some(ObjectScript::Code(0x0800_C73C)), 2);
        field.update(Input::default());
        assert_eq!(
            field.update(Input::default().with(Button::A)),
            Some(FieldEvent::Talk {
                actor: 1,
                script: ObjectScript::Code(0x0800_C73C)
            })
        );
        assert_eq!(field.actors[1].animation_id, 1);
        field.actors[1] = character(4, 1, Some(ObjectScript::Dialogue(1)), SILENT_BEHAVIOR);
        field.update(Input::default());
        assert_eq!(field.update(Input::default().with(Button::A)), None);
    }

    #[test]
    fn reports_the_exit_when_the_step_onto_it_completes() {
        let mut field = field(6, 5);
        let mut exits = Vec::new();
        for _ in 0..32 {
            exits.extend(field.update(held(Direction::Left)));
        }
        assert_eq!(exits, [FieldEvent::Exit(1)]);
        assert_eq!(field.player().footing(), (1, 2));
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
        assert_eq!(field.player().position(), (24, 32));
        assert_eq!(field.player().facing, Direction::Right);
        assert!(!field.player().walking());
        let keep = Warp {
            facing: None,
            ..warp
        };
        field.enter(scene(8, 8), 7, &keep);
        assert_eq!(field.player().facing, Direction::Right);
    }

    #[test]
    fn plays_the_sheet_animations_counting_the_first_frame() {
        let mut field = field(6, 5);
        assert_eq!(field.player_frame(), Some(3));
        field.update(held(Direction::Right));
        assert_eq!(field.player_frame(), Some(21));
        for _ in 0..3 {
            field.update(held(Direction::Right));
        }
        assert_eq!(field.player_frame(), Some(22));
        for _ in 0..8 {
            field.update(held(Direction::Right));
        }
        assert_eq!(field.player_frame(), Some(23));
        let sheet = sheet();
        assert_eq!(current_frame(&sheet, 2, 17), Some(6));
        assert_eq!(current_frame(&sheet, 8, 0), None);
        assert_eq!(frame_at(&sheet, 2, 7, 0), Some(6));
        assert_eq!(frame_at(&sheet, 2, 8, 0), Some(7));
        assert_eq!(frame_at(&sheet, 2, 8, -1), Some(6));
    }

    #[test]
    fn camera_keeps_the_sprite_at_the_anchor_within_bounds() {
        let mut large = field(40, 20);
        assert_eq!(large.camera(), (0, 0));
        large.player_mut().place((13, 7));
        assert_eq!(large.player().position(), (200, 112));
        assert_eq!(large.camera(), (96, 48));
        large.player_mut().place((1000, 1000));
        assert_eq!(large.camera(), (640 - 240, 320 - 160));
        assert_eq!(field(6, 5).camera(), (0, 0));
    }
}
