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
//! stand on and, for the first half of a step, the one they left (the
//! stepping command `0x0800B764` moves the previous cell to the new one
//! halfway); a walk through
//! (command 11) ignores both. Animations come from the sprite sheet: idle
//! animation = facing, walking = facing + 4, restarted by every step and
//! turn, each tick shortened by the actor's animation shift (1 halves them:
//! the player's steps last one walking cycle). Actors are drawn nearer over
//! farther, in the order the game's own sort leaves them, and the screen
//! shows the positions of the frame before with the current pictures.
//! Completing a step onto an exit reports it; pressing A while standing and
//! facing an actor turns it toward the player (characters only) and reports
//! what it runs.

use extraction::saga::{
    MapError, MapObject, ObjectScript, PLAYER_SPRITE, Scene, SceneError, SpriteFrame, SpriteSheet,
    SpriteSheetError, WALK_ANIMATION_BASE, Warp,
};
use formats::progress::ObjectState;
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
/// The attribute bits that stop a guard's line of sight.
const SIGHT_BLOCKERS: u16 = 0xE000;
/// The OBJ palette banks sprites choose from.
const OBJECT_PALETTES: usize = 16;
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
/// A step with B held (`0x0800B282`): twice the speed, so half the frames,
/// and the walk animation's ticks shifted once more.
const RUN_SPEED_FACTOR: i32 = 2;
const RUN_ANIMATION_SHIFT: i8 = 2;
const CHARACTER_BEHAVIOR: u16 = 0;
const TURNING_BEHAVIORS: u16 = 2;
const PASSABLE_BEHAVIOR: u16 = 3;
const SILENT_BEHAVIOR: u16 = 5;
const PLAYER_KIND: u16 = 0;
const WANDER_KIND: u16 = 2;
const SHY_KIND: u16 = 3;
const CHASE_KIND: u16 = 4;
/// A sprite field with this bit names its sheet through a lookup the port
/// keeps from the object's own list.
const SPRITE_LOOKUP: u16 = 0x8000;
/// Objects of this behavior are map Zoids: the player's carrier and the
/// enemies that roam the Zoid maps meet in battle.
const ZOID_BEHAVIOR: u16 = 1;
/// How near, in cells along each axis, a roaming enemy heads for the
/// player (`0x0800AA98`).
const CHASE_RADIUS: usize = 2;
/// The attribute bit that sets a cell on another level; Zoids on different
/// levels pass without meeting (`0x0800AE7C`).
const LEVEL_BIT: u16 = 0x1000;
/// The space-time portal's sprite, and how many entities `0x0800960C`
/// looks through for it.
const PORTAL_SPRITE: u16 = 0xF7;
const PORTAL_SEARCH: usize = 29;
/// What a beaten Zoid turns into (`0x080089A0` with sprite `0xFD`), and
/// the sound it goes off with.
pub const EXPLOSION_SPRITE: usize = 0xFD;
const EXPLOSION_SOUND: u16 = 0x5A;
/// The attribute bit of an exit's cell; an enemy does not reach the player
/// standing on one.
const EXIT_BIT: u16 = 0x4000;
const IDLE_TIMER_MASK: u16 = 0x7F;
/// A sprite off the screen by more than this (its top-left 56 pixels left
/// or 32 above) or whose top-left is past the span from there is skipped
/// by the OAM builder (`0x080005CA`) and left out of the sort.
const CULL_MARGIN: (isize, isize) = (0x38, 0x20);
const CULL_SPAN: (isize, isize) = (320, 192);
/// How near, in cells along each axis, a running player makes a shy
/// character step away (`0x0800AA60`).
const SHY_RADIUS: usize = 3;
/// Draws up to this make a shy character step a random way instead.
const SHY_RANDOM_LIMIT: u16 = 0x0FFF;
/// The draw's bit that picks the vertical axis when both are as far.
const SHY_VERTICAL_BIT: u16 = 0x8000;

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

    /// The cell step one way: columns and rows.
    #[must_use]
    pub const fn delta(self) -> (isize, isize) {
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

    /// The way back.
    #[must_use]
    pub const fn opposite(self) -> Self {
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
    /// The player pushed against this door (a `0xC000` exit).
    Door(usize),
    /// The player pushed toward the portal on this exit (a `0x4800` exit).
    Portal(usize),
    /// The player and a roaming enemy ran into each other.
    Encounter {
        /// Index into the field's actors of the enemy.
        enemy: usize,
    },
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
    /// A wrecked actor's explosion ended and it is gone.
    Wrecked {
        /// Index into the field's actors.
        actor: usize,
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
    /// Walks around at random, and steps away from the player while B is
    /// held within three cells (command 3).
    Shy,
    /// Walks around at random, and heads for the player within two cells
    /// (command 4): the enemies that roam the Zoid maps.
    Chase,
    /// Heads for a metatile, the longer axis first, horizontally on ties.
    WalkTo(Walk),
    /// Beaten in battle (entity state 5, `0x0800BC8C`): plays its animation
    /// once more, then explodes (`exploding`) and is gone.
    Wrecked {
        /// Whether it already shows the explosion.
        exploding: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step {
    direction: Direction,
    frames: i32,
    speed: i32,
    /// Frames left when the cell left stops blocking others.
    halfway: i32,
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
    /// The object state it was built from, on a map whose objects the
    /// game-state block keeps.
    pub slot: Option<usize>,
    /// For a roaming enemy, its formation among the area's.
    pub group: u16,
    /// Frames it stands still before its command runs again, as an enemy
    /// does after the party retreated from it.
    pub pause: u16,
    /// The sprite it shows (entity `+0x5A`), as its object or state lists
    /// it.
    pub sprite: u16,
    /// The OBJ palette bank it is drawn with (entity `+6` bits 12–15), the
    /// slot its object reserves.
    palette_slot: usize,
    /// The sheet a wrecked actor explodes with.
    explosion: Option<Box<SpriteSheet>>,
    /// For an object showing a party member's Zoid, its sprite as listed
    /// (bit 15 set), resolved once the game state is known.
    party_sprite: Option<u16>,
    /// The shift the animation's first step was timed with, when not the
    /// current one: the shift in effect when it started (`0x08000BD8`), a
    /// walk's step setting its own before. The player's own step starts its
    /// walking animation before it sets the walking shift, and a finished
    /// step starts the standing one before the standing shift comes back.
    start_shift: Option<i8>,
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
            slot: None,
            group: 0,
            pause: 0,
            sprite: 0,
            palette_slot: 0,
            explosion: None,
            party_sprite: None,
            start_shift: None,
        }
    }

    fn from_object(sheet: Option<SpriteSheet>, object: &MapObject) -> Self {
        let facing = Direction::from_index(object.animation).unwrap_or(Direction::Down);
        let mut actor = Self::new(sheet, (object.column, object.row), facing);
        actor.animation_id = object.animation;
        actor.script = object.script_kind();
        actor.chest = object.chest();
        actor.behavior = object.behavior;
        actor.command = command_for(object.kind);
        actor.party_sprite = (object.sprite_sheet_id().is_none()).then_some(object.sprite);
        actor.sprite = object.sprite;
        actor.palette_slot = object.palette_slot % OBJECT_PALETTES;
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

    /// Its sprite's position in 16.16 map pixels (entity `+0x08`, `+0x0C`).
    #[must_use]
    pub fn fixed_position(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    /// Moves the sprite by `(dx, dy)` 16.16 pixels, its cell unchanged.
    pub fn move_by(&mut self, dx: i32, dy: i32) {
        self.x = self.x.wrapping_add(dx);
        self.y = self.y.wrapping_add(dy);
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

    /// Plays animation `id` from the start, its first step timed with the
    /// shift in effect now (`0x08000BD8`).
    pub fn play(&mut self, id: usize) {
        self.animation_id = id;
        self.animation = 0;
        self.start_shift = Some(self.animation_shift);
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
        let first = sheet
            .animations
            .get(self.animation_id)
            .and_then(|steps| steps.first())
            .map_or(0, |step| step.duration);
        let now = tick_length(first, self.animation_shift);
        let started = self
            .start_shift
            .map_or(now, |shift| tick_length(first, shift));
        self.once
            && animation_length(sheet, self.animation_id, self.animation_shift)
                .is_some_and(|length| self.animation + now >= length + started)
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
        self.start_shift = None;
        let frames = self.size * PIXEL / speed.max(1);
        self.step = Some(Step {
            direction,
            frames,
            speed,
            halfway: frames / 2 - 1,
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
        if step.frames == step.halfway {
            self.previous = (self.column, self.row);
        }
        if step.frames > 0 {
            return false;
        }
        self.step = None;
        self.previous = (self.column, self.row);
        (self.x, self.y) = origin(self.column, self.row, self.size);
        let walking = self.animation_shift;
        self.face(self.facing);
        self.start_shift = Some(walking);
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
            self.elapsed(steps)?,
            self.animation_shift,
        )
    }

    /// The frames into the animation as its steps at the current shift
    /// count them, its first step having taken the length it started with.
    fn elapsed(&self, steps: &[extraction::saga::AnimationStep]) -> Option<u32> {
        let first = steps.first()?.duration;
        let now = tick_length(first, self.animation_shift);
        let started = self
            .start_shift
            .map_or(now, |shift| tick_length(first, shift));
        Some(if self.animation < started {
            0
        } else {
            self.animation - started + now
        })
    }

    /// Whether the animation shows the last frame of its step `step` (the
    /// step's countdown at 1).
    #[must_use]
    pub fn ends_step(&self, step: usize) -> bool {
        let Some(sheet) = self.sheet.as_ref() else {
            return false;
        };
        let Some(steps) = sheet.animations.get(self.animation_id) else {
            return false;
        };
        let Some(elapsed) = self.elapsed(steps) else {
            return false;
        };
        let at = |elapsed| step_at(sheet, self.animation_id, elapsed, self.animation_shift);
        at(elapsed) == Some(step) && at(elapsed + 1) != Some(step)
    }
}

/// What an object of command `kind` does.
fn command_for(kind: u16) -> Command {
    match kind {
        PLAYER_KIND => Command::Player,
        WANDER_KIND => Command::Wander,
        SHY_KIND => Command::Shy,
        CHASE_KIND => Command::Chase,
        _ => Command::Idle,
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
    let step = step_at(sheet, id, elapsed, shift)?;
    sheet.animations.get(id)?.get(step).map(|step| step.frame)
}

/// The step of `sheet`'s animation `id` shown after `elapsed` frames, the
/// animation looping (see [`frame_at`]).
fn step_at(sheet: &SpriteSheet, id: usize, elapsed: u32, shift: i8) -> Option<usize> {
    let steps = sheet.animations.get(id)?;
    let length = |ticks: u32| tick_length(ticks, shift);
    let cycle: u32 = steps.iter().map(|step| length(step.duration)).sum();
    if cycle == 0 {
        return None;
    }
    let mut remaining = elapsed % cycle;
    steps.iter().position(|step| {
        let frames = length(step.duration);
        if remaining < frames {
            true
        } else {
            remaining -= frames;
            false
        }
    })
}

/// The animation shift of an actor standing still: characters' steps last
/// half their ticks, the rest's their whole.
fn standing_shift(behavior: u16) -> i8 {
    if behavior == CHARACTER_BEHAVIOR {
        PLAYER_ANIMATION_SHIFT
    } else {
        0
    }
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
    /// The actors from front to back, the order the game gives OAM.
    order: Vec<usize>,
    /// The actors the last frame's sprites left out as off the screen.
    culled: Vec<bool>,
    /// What the screen shows, kept by [`Field::latch`].
    shown: Option<Shown>,
    /// Sound effects the actors asked for, not yet played.
    sounds: Vec<u16>,
    /// A debugging aid the original has not: the roaming enemies neither
    /// block the player nor meet it.
    intangible: bool,
    /// No enemy meets the player (`0x02000008` bit 1), which some events
    /// set while they run.
    calm: bool,
    /// The OBJ palette banks: the palette last copied into each, which
    /// every actor reserving that slot is drawn with.
    object_palettes: [Option<[u16; 16]>; OBJECT_PALETTES],
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
            order: vec![0],
            culled: vec![false],
            shown: None,
            sounds: Vec::new(),
            intangible: false,
            calm: false,
            object_palettes: [None; OBJECT_PALETTES],
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
            order: Vec::new(),
            culled: Vec::new(),
            shown: None,
            sounds: Vec::new(),
            intangible: false,
            calm: false,
            object_palettes: [None; OBJECT_PALETTES],
        };
        field.player_mut().place((column, row));
        field.load_object_palettes();
        field.sort_actors();
        Ok(field)
    }

    /// Builds the objects of a map whose record id has bit 15 from the
    /// game-state block's object states, as the map loader does
    /// (`0x08007188`): object `n` (from 1) takes state `first + n − 1`,
    /// its sprite, cell, command and parameter; an object whose state is
    /// gone is not loaded: its actor stays hidden and idle, so a beaten
    /// roaming enemy neither shows nor goes after the player when its map
    /// loads again. `first` is `None` when the table has no state for the
    /// map, which loads none of its objects.
    ///
    /// # Errors
    ///
    /// Returns [`FieldError`] when a sprite cannot be read.
    pub fn apply_object_states(
        &mut self,
        data: &GameData<'_>,
        states: &[ObjectState],
        first: Option<usize>,
    ) -> Result<(), FieldError> {
        let size = cell_pixels(&self.scene);
        for index in 1..self.actors.len() {
            let slot = first.map(|first| first + index - 1);
            let state = slot
                .and_then(|slot| states.get(slot))
                .filter(|state| state.present);
            let actor = &mut self.actors[index];
            let Some(state) = state else {
                actor.visible = false;
                actor.command = Command::Idle;
                actor.slot = None;
                continue;
            };
            if state.sprite & SPRITE_LOOKUP == 0 {
                actor.sheet = Some(data.sprite_sheet(usize::from(state.sprite))?);
            }
            actor.sprite = state.sprite;
            actor.command = command_for(u16::from(state.command));
            actor.slot = slot;
            actor.group = state.group;
            actor.set_cell_size(size);
            actor.place((usize::from(state.column), usize::from(state.row)));
        }
        self.load_object_palettes();
        self.sort_actors();
        Ok(())
    }

    /// The cells the actors built from object states stand on, by state,
    /// for those taking a step: what the stepping command writes back
    /// halfway through each step (`0x0800B764`). An object an event places
    /// keeps its state.
    pub fn object_cells(&self) -> impl Iterator<Item = (usize, (usize, usize))> + '_ {
        self.actors
            .iter()
            .filter(|actor| actor.visible && actor.walking())
            .filter_map(|actor| actor.slot.map(|slot| (slot, actor.previous)))
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

    /// Wrecks actor `index`, beaten in battle (`0x0800B9CC`): its facing's
    /// animation plays once from the start, then it turns into `explosion`
    /// facing up, with a sound, and once that has played it is gone
    /// (`0x0800BC8C`). It keeps its animation shift.
    pub fn wreck(&mut self, index: usize, explosion: SpriteSheet) {
        if let Some(actor) = self.actors.get_mut(index) {
            actor.command = Command::Wrecked { exploding: false };
            actor.explosion = Some(Box::new(explosion));
            actor.play_once(actor.facing.index());
        }
    }

    /// The sound effects asked for since the last call.
    pub fn take_sounds(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.sounds)
    }

    /// Restarts every actor's animation `elapsed` frames in, as reloading
    /// the map does when the pause menu closes.
    pub fn restart_animations(&mut self, elapsed: u32) {
        for actor in &mut self.actors {
            actor.animation = elapsed;
        }
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
        self.tick_animations();
        self.sort_actors();
        event
    }

    /// Advances every actor's animation by a frame.
    pub fn tick_animations(&mut self) {
        for actor in &mut self.actors {
            actor.animation = actor.animation.saturating_add(1);
        }
    }

    /// Orders the actors front to back as the game orders its sprites
    /// (`0x08000468`, before each frame's OAM): the list starts in object
    /// order when a scene loads and is sorted in place by a selection sort,
    /// each visible actor swapping places with a later visible, on-screen
    /// one lower on the map. Actors level with each other are not swapped,
    /// so their order is whatever earlier swaps left: in the first room the
    /// maid, lower down, sends the prince behind the chair at his desk.
    fn sort_actors(&mut self) {
        if self.order.len() != self.actors.len() {
            self.order = (0..self.actors.len()).collect();
            self.culled = vec![false; self.actors.len()];
        }
        for first in 0..self.order.len() {
            if !self.actors[self.order[first]].visible {
                continue;
            }
            for later in first + 1..self.order.len() {
                let (front, back) = (self.order[first], self.order[later]);
                let (front_y, back) = (self.actors[front].y, &self.actors[back]);
                if back.visible && !self.culled[self.order[later]] && front_y < back.y {
                    self.order.swap(first, later);
                }
            }
        }
        let camera = self.camera();
        for (culled, actor) in self.culled.iter_mut().zip(&self.actors) {
            let (x, y) = actor.position();
            let x = x - isize::try_from(camera.0).unwrap_or(0) + CULL_MARGIN.0;
            let y = y - isize::try_from(camera.1).unwrap_or(0) + CULL_MARGIN.1;
            *culled = !((0..=CULL_SPAN.0).contains(&x) && (0..=CULL_SPAN.1).contains(&y));
        }
    }

    fn update_actor(&mut self, index: usize, input: Input, pressed_a: bool) -> Option<FieldEvent> {
        let actor = &mut self.actors[index];
        if let Command::Wrecked { exploding } = actor.command {
            return self.update_wreck(index, exploding);
        }
        if actor.step.is_some() {
            let finished = actor.advance_step();
            if finished
                && matches!(
                    actor.command,
                    Command::Wander | Command::Shy | Command::Chase
                )
            {
                actor.timer = self.rng.next(self.frame) & IDLE_TIMER_MASK;
            }
            if finished && index == 0 && actor.command == Command::Player {
                let (column, row) = actor.footing();
                return self.scene.exit(column, row).map(FieldEvent::Exit);
            }
            if let (true, 0, Command::WalkTo(walk)) = (finished, index, actor.command) {
                return self.walk_door(walk);
            }
            return None;
        }
        if actor.pause > 0 {
            actor.pause -= 1;
            return None;
        }
        let event = match actor.command {
            Command::Player => self.update_player(input, pressed_a),
            Command::Idle | Command::Wrecked { .. } => None,
            Command::Wander => {
                self.wander(index);
                None
            }
            Command::Shy => {
                if input.is_held(Button::B) {
                    self.pursue(index, SHY_RADIUS, true)
                } else {
                    self.wander(index);
                    None
                }
            }
            Command::Chase => self.pursue(index, CHASE_RADIUS, false),
            Command::WalkTo(walk) => self.walk(index, walk),
        };
        let actor = &mut self.actors[index];
        if actor.step.is_none() {
            actor.animation_shift = standing_shift(actor.behavior);
        }
        event
    }

    /// A frame of a wrecked actor (`0x0800BC8C`): once its animation has
    /// played, it turns into the explosion, and once that has, it is gone.
    fn update_wreck(&mut self, index: usize, exploding: bool) -> Option<FieldEvent> {
        let actor = &mut self.actors[index];
        if !actor.animation_done() {
            return None;
        }
        if exploding {
            actor.visible = false;
            actor.command = Command::Idle;
            return Some(FieldEvent::Wrecked { actor: index });
        }
        if let Some(explosion) = actor.explosion.take() {
            self.object_palettes[actor.palette_slot] = Some(explosion.palette);
            actor.sheet = Some(*explosion);
        }
        actor.facing = Direction::Up;
        actor.play_once(Direction::Up.index());
        actor.command = Command::Wrecked { exploding: true };
        self.sounds.push(EXPLOSION_SOUND);
        None
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
        if let Some(exit) = self
            .ahead(direction)
            .and_then(|(column, row)| self.scene.portal(column, row))
        {
            return Some(FieldEvent::Portal(exit));
        }
        if self.free(0, direction, false) {
            let running = input.is_held(Button::B);
            let speed = if running {
                PLAYER_SPEED * RUN_SPEED_FACTOR
            } else {
                PLAYER_SPEED
            };
            let player = &mut self.actors[0];
            let standing = player.animation_shift;
            player.start_step(direction, speed);
            player.start_shift = Some(standing);
            if running {
                player.animation_shift = RUN_ANIMATION_SHIFT;
            }
            return None;
        }
        if let Some(enemy) = self.meeting(0, direction) {
            self.actors[enemy].face(direction.opposite());
            return Some(FieldEvent::Encounter { enemy });
        }
        let (dx, dy) = direction.delta();
        let (column, row) = self.actors[0].footing();
        let column = column.checked_add_signed(dx)?;
        let row = row.checked_add_signed(dy)?;
        self.scene.door(column, row).map(FieldEvent::Door)
    }

    /// The player's footing one cell toward `direction`.
    fn ahead(&self, direction: Direction) -> Option<(usize, usize)> {
        let (dx, dy) = direction.delta();
        let (column, row) = self.actors[0].footing();
        Some((column.checked_add_signed(dx)?, row.checked_add_signed(dy)?))
    }

    /// The first object showing the space-time portal (sprite `0xF7`), and
    /// its cell, as `0x0800960C` looks for it among the entities.
    #[must_use]
    pub fn portal(&self) -> Option<(usize, (usize, usize))> {
        self.actors
            .iter()
            .enumerate()
            .take(PORTAL_SEARCH)
            .find(|(_, actor)| actor.visible && actor.sprite == PORTAL_SPRITE)
            .map(|(index, actor)| (index, (actor.column, actor.row)))
    }

    /// The Zoid actor `index`, stepping toward `direction`, runs into, when
    /// the two meet in battle (`0x0800AE7C`): the player's carrier and an
    /// enemy Zoid, roaming or standing guard, on the same level, the player
    /// in control; an enemy does not reach a player on an exit's cell, and
    /// one beaten and exploding meets no one.
    fn meeting(&self, index: usize, direction: Direction) -> Option<usize> {
        if self.intangible || self.calm {
            return None;
        }
        let (dx, dy) = direction.delta();
        let (column, row) = self.actors[index].footing();
        let ahead = (column.checked_add_signed(dx)?, row.checked_add_signed(dy)?);
        let other = (0..self.actors.len()).find(|&other| {
            let actor = &self.actors[other];
            other != index
                && actor.visible
                && (actor.footing() == ahead || actor.previous_footing() == ahead)
        })?;
        let (enemy, player) = if index == 0 {
            (other, 0)
        } else {
            (index, other)
        };
        if player != 0 || self.actors[0].command != Command::Player {
            return None;
        }
        let level = |actor: &Actor| {
            let (column, row) = actor.footing();
            self.scene.attribute(column, row).unwrap_or(0) & LEVEL_BIT
        };
        let (carrier, zoid) = (&self.actors[0], &self.actors[enemy]);
        let exit_cell = {
            let (column, row) = carrier.footing();
            self.scene.attribute(column, row).unwrap_or(0) & EXIT_BIT != 0
        };
        let meets = carrier.behavior == ZOID_BEHAVIOR
            && zoid.behavior == ZOID_BEHAVIOR
            && !matches!(zoid.command, Command::Wrecked { .. })
            && level(carrier) == level(zoid)
            && (index == 0 || !exit_cell);
        meets.then_some(enemy)
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

    /// The flight routine (`0x0800A5E4`) for actor `index` within `radius`
    /// cells of the player along both axes: a shy character with B held
    /// steps away (`away`), a roaming enemy toward. Mostly along the axis
    /// the player is farther on, a random draw picking the axis on ties,
    /// and one draw in sixteen a random way; at a pixel a frame, and every
    /// frame it can. Farther away it wanders. An enemy stepping into the
    /// player meets it.
    fn pursue(&mut self, index: usize, radius: usize, away: bool) -> Option<FieldEvent> {
        let (player, actor) = (&self.actors[0], &self.actors[index]);
        let across = player.column.abs_diff(actor.column);
        let along = player.row.abs_diff(actor.row);
        if across > radius || along > radius {
            self.wander(index);
            return None;
        }
        let player_left = player.column < actor.column;
        let player_above = player.row < actor.row;
        let horizontal = if player_left == away {
            Direction::Right
        } else {
            Direction::Left
        };
        let vertical = if player_above == away {
            Direction::Down
        } else {
            Direction::Up
        };
        let draw = self.rng.next(self.frame);
        let direction = if draw <= SHY_RANDOM_LIMIT {
            self.rng.seed(self.frame);
            random_direction(self.rng.next(self.frame))
        } else if across == along {
            if draw & SHY_VERTICAL_BIT == 0 {
                horizontal
            } else {
                vertical
            }
        } else if across > along {
            horizontal
        } else {
            vertical
        };
        let free = self.free(index, direction, true);
        if !free && let Some(enemy) = self.meeting(index, direction) {
            self.actors[enemy].face(direction);
            self.actors[0].face(direction.opposite());
            return Some(FieldEvent::Encounter { enemy });
        }
        let actor = &mut self.actors[index];
        actor.face(direction);
        if free {
            actor.start_step(direction, PIXEL);
        } else {
            actor.timer = self.rng.next(self.frame) & IDLE_TIMER_MASK;
        }
        None
    }

    /// The door the player's walk to a cell runs into at the end of a step,
    /// which the original takes in that frame.
    fn walk_door(&self, walk: Walk) -> Option<FieldEvent> {
        let player = &self.actors[0];
        let direction = toward((player.column, player.row), (walk.column, walk.row))?;
        if walk.through || self.free(0, direction, false) {
            return None;
        }
        let (dx, dy) = direction.delta();
        let (column, row) = player.footing();
        let column = column.checked_add_signed(dx)?;
        let row = row.checked_add_signed(dy)?;
        self.scene.door(column, row).map(FieldEvent::Door)
    }

    /// A step of a walk to a cell. The player walking against a door
    /// takes it, as pushing against it does (`0x0800AE7C`): the world
    /// map's drive to Arcana ends in the town this way.
    fn walk(&mut self, index: usize, walk: Walk) -> Option<FieldEvent> {
        let actor = &self.actors[index];
        let Some(direction) = toward((actor.column, actor.row), (walk.column, walk.row)) else {
            self.actors[index].command = Command::Idle;
            return None;
        };
        let free = walk.through || self.free(index, direction, false);
        let actor = &mut self.actors[index];
        if free {
            actor.start_step(direction, walk.speed);
            actor.animation_shift = walk.animation_shift;
            return None;
        }
        if actor.facing != direction {
            actor.face(direction);
        }
        if index != 0 {
            return None;
        }
        let (dx, dy) = direction.delta();
        let (column, row) = actor.footing();
        let column = column.checked_add_signed(dx)?;
        let row = row.checked_add_signed(dy)?;
        self.scene.door(column, row).map(FieldEvent::Door)
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
                && !(self.intangible && index == 0 && Self::roaming_enemy(actor))
                && (actor.footing() == (column, row) || actor.previous_footing() == (column, row))
        })
    }

    /// Whether `actor` is an enemy on a Zoid map, which meets the player
    /// in battle whether it roams or stands guard.
    fn roaming_enemy(actor: &Actor) -> bool {
        actor.behavior == ZOID_BEHAVIOR
    }

    /// Makes the roaming enemies intangible, or solid again: when
    /// intangible the player walks through them and none meets it in
    /// battle. A debugging aid; the original has no such thing.
    pub fn set_intangible(&mut self, intangible: bool) {
        self.intangible = intangible;
    }

    /// Keeps the enemies from meeting the player, or lets them again
    /// (`0x02000008` bit 1): they still block its way.
    pub fn set_calm(&mut self, calm: bool) {
        self.calm = calm;
    }

    /// Whether actor `index` was left out of the last frame's sprites as
    /// off the screen (entity flag `0x10`).
    #[must_use]
    pub fn culled(&self, index: usize) -> bool {
        self.culled.get(index).copied().unwrap_or(false)
    }

    /// The first of `guards` that sees the player (`0x0801BC2C`): an actor
    /// on the screen, standing (the routine reads its animation, and a
    /// walking one is none of the four standing ones), whose line of sight
    /// from its cell, the way it faces, reaches the player's cell before a cell whose attribute has any of
    /// bits `0xE000` (read at the footing, as `0x08008498` does): five
    /// cells up or down, seven across.
    #[must_use]
    pub fn seen_by(&self, guards: &[usize]) -> Option<usize> {
        let player = self.actors.first()?;
        let target = (player.column, player.row);
        let footing = self.actors.first().map_or(0, Actor::footing_offset);
        guards.iter().copied().find(|&guard| {
            let Some(actor) = self.actors.get(guard) else {
                return false;
            };
            if !actor.visible || self.culled(guard) || actor.step.is_some() {
                return false;
            }
            let (dx, dy, reach) = match actor.facing {
                Direction::Up => (0, -1, 5),
                Direction::Down => (0, 1, 5),
                Direction::Left => (-1, 0, 7),
                Direction::Right => (1, 0, 7),
            };
            (1..=reach)
                .map_while(|k| {
                    let column = actor.column.checked_add_signed(dx * k)?;
                    let row = actor.row.checked_add_signed(dy * k)?;
                    let blocked = self
                        .scene
                        .attribute(column, row + footing)
                        .is_some_and(|attribute| attribute & SIGHT_BLOCKERS != 0);
                    (!blocked).then_some((column, row))
                })
                .any(|cell| cell == target)
        })
    }

    fn talk(&mut self) -> Option<FieldEvent> {
        let player = &self.actors[0];
        let (dx, dy) = player.facing.delta();
        let (column, row) = player.footing();
        let mut ahead = (column.checked_add_signed(dx)?, row.checked_add_signed(dy)?);
        while self.scene.counter(ahead.0, ahead.1) {
            ahead = (
                ahead.0.checked_add_signed(dx)?,
                ahead.1.checked_add_signed(dy)?,
            );
        }
        let facing = player.facing;
        let index = (1..self.actors.len()).find(|&index| {
            let actor = &self.actors[index];
            actor.visible
                && actor.step.is_none()
                && actor.footing() == ahead
                && !matches!(actor.command, Command::Wrecked { .. })
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
        self.order.clear();
        self.culled.clear();
        self.shown = None;
        self.enter(scene, warp.map, &warp);
        self.load_object_palettes();
        self.sort_actors();
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
        let ((base_x, max_x), (base_y, max_y)) = self.camera_base();
        let panned = |base: usize, pan: i32, max: usize| {
            let offset = isize::try_from(pan >> FRACTION_BITS).unwrap_or(0);
            base.saturating_add_signed(offset).min(max)
        };
        (
            panned(base_x, self.pan.0, max_x),
            panned(base_y, self.pan.1, max_y),
        )
    }

    /// Gives the objects that show a party member's Zoid its sprite
    /// ([`extraction::saga_party::object_sprite`]).
    ///
    /// # Errors
    ///
    /// Returns [`FieldError`] when a sprite cannot be read.
    pub fn show_party_zoids(
        &mut self,
        data: &GameData<'_>,
        state: &[u8],
    ) -> Result<(), FieldError> {
        for actor in &mut self.actors {
            if let Some(sprite) = actor.party_sprite.take() {
                let sprite = extraction::saga_party::object_sprite(state, sprite);
                actor.sheet = Some(data.sprite_sheet(usize::from(sprite))?);
            }
        }
        self.load_object_palettes();
        Ok(())
    }

    /// Copies each actor's palette into the bank its object reserves, in
    /// the list's order, as the map loader does: an object sharing a slot
    /// with an earlier one shows its palette on both (Regina wears Fiene's
    /// in chapter 4's Ark base).
    fn load_object_palettes(&mut self) {
        self.object_palettes = [None; OBJECT_PALETTES];
        for actor in &self.actors {
            if let Some(sheet) = &actor.sheet {
                self.object_palettes[actor.palette_slot] = Some(sheet.palette);
            }
        }
    }

    /// Gives actor `index` the sprite `sheet` (`0x080089A0`), copying its
    /// palette into the actor's bank.
    pub fn set_sheet(&mut self, index: usize, sheet: SpriteSheet) {
        let Some(actor) = self.actors.get_mut(index) else {
            return;
        };
        self.object_palettes[actor.palette_slot] = Some(sheet.palette);
        actor.sheet = Some(sheet);
        actor.play(actor.animation_id & 3);
    }

    /// Whether the map is one of the Zoid's, whose cells are twice a room's.
    #[must_use]
    pub fn zoid_map(&self) -> bool {
        cell_pixels(&self.scene) > ROOM_CELL
    }

    /// How many actors the last frame drew: visible and on the screen.
    #[must_use]
    pub fn drawn_actors(&self) -> usize {
        self.actors
            .iter()
            .zip(&self.culled)
            .filter(|(actor, culled)| actor.visible && !**culled)
            .count()
    }

    /// Stands actor `actor` on `cell` (`0x08008B70`). Only the player's own
    /// steps move the camera, so placing the player leaves it where it is.
    pub fn place_actor(&mut self, actor: usize, cell: (usize, usize)) {
        let before = self.camera();
        let Some(placed) = self.actors.get_mut(actor) else {
            return;
        };
        placed.place(cell);
        if actor == 0 {
            self.hold_camera(before);
        }
    }

    /// Moves the player's sprite by `(dx, dy)` 16.16 pixels as a scene's
    /// glide does (`0x08011E40`, `0x08011EAC`): the camera does not follow
    /// the sprite, but scrolls by the same amount when `camera`
    /// (`0x08008324`).
    pub fn glide_player(&mut self, dx: i32, dy: i32, camera: bool) {
        let before = self.camera();
        self.player_mut().move_by(dx, dy);
        self.hold_camera(before);
        if camera {
            self.pan_by(dx, dy);
        }
    }

    /// Sets the pan so the camera stays at `before` whatever the player's
    /// anchor now asks for.
    fn hold_camera(&mut self, before: (usize, usize)) {
        let ((base_x, _), (base_y, _)) = self.camera_base();
        let offset = |camera: usize, base: usize| {
            (i32::try_from(camera).unwrap_or(0) - i32::try_from(base).unwrap_or(0)) << FRACTION_BITS
        };
        self.pan = (offset(before.0, base_x), offset(before.1, base_y));
    }

    /// Scrolls the camera by `(dx, dy)` 16.16 pixels (`0x08008324`): a move
    /// past the map's edge is refused, so the pan stops there and comes back
    /// from there.
    pub fn pan_by(&mut self, dx: i32, dy: i32) {
        let ((base_x, max_x), (base_y, max_y)) = self.camera_base();
        let bounded = |pan: i32, delta: i32, base: usize, max: usize| {
            let low = -(i32::try_from(base).unwrap_or(0) << FRACTION_BITS);
            let high = i32::try_from(max.saturating_sub(base)).unwrap_or(0) << FRACTION_BITS;
            let moved = pan.saturating_add(delta);
            if (low..=high).contains(&moved) {
                moved
            } else {
                pan
            }
        };
        self.pan = (
            bounded(self.pan.0, dx, base_x, max_x),
            bounded(self.pan.1, dy, base_y, max_y),
        );
    }

    /// The camera's scroll before the pan and its largest, for each axis.
    fn camera_base(&self) -> ((usize, usize), (usize, usize)) {
        let map_width = self.scene.map.width * TILE_SIZE;
        let map_height = self.scene.map.height * TILE_SIZE;
        let max_x = map_width.saturating_sub(SCREEN_WIDTH);
        let max_y = map_height.saturating_sub(SCREEN_HEIGHT);
        let (x, y) = self.player().anchor();
        let scroll = |position: isize, anchor: isize, max: usize| {
            usize::try_from(position - anchor).unwrap_or(0).min(max)
        };
        (
            (scroll(x, CAMERA_ANCHOR.0, max_x), max_x),
            (scroll(y, CAMERA_ANCHOR.1, max_y), max_y),
        )
    }

    /// Frame record of the player's sheet for the current animation frame.
    #[must_use]
    pub fn player_frame(&self) -> Option<usize> {
        self.player().current_frame()
    }

    /// Draws the scene and the visible actors for the current frame.
    pub fn draw(&self, frame: &mut Frame) {
        let current;
        let shown = if let Some(shown) = &self.shown {
            shown
        } else {
            current = self.showing();
            &current
        };
        let scroll = shown.camera;
        let tile = |index: usize| self.scene.tiles.tile(index);
        let backdrop = |x: usize, y: usize| self.scene.backdrop.wrapping(x, y);
        draw_background(frame, backdrop, tile, &self.palettes, scroll, false);
        let map = |x: usize, y: usize| self.scene.map.wrapping(x, y);
        draw_background(frame, map, tile, &self.palettes, scroll, true);
        for sprite in shown.sprites.iter().rev() {
            let Some(actor) = self.actors.get(sprite.actor) else {
                continue;
            };
            let Some(sheet) = actor.sheet.as_ref() else {
                continue;
            };
            let record = &sprite.frame;
            let copied = actor
                .current_frame()
                .and_then(|index| sheet.frames.get(index))
                .unwrap_or(record);
            let Some(image) = sheet.streamed_image(record, copied) else {
                continue;
            };
            let (x, y) = sprite.position;
            let (anchor_x, anchor_y) = sprite.anchor;
            let anchor_x = if record.mirrored {
                2 * i16::try_from(ANCHOR_FROM_ORIGIN.0).unwrap_or(0) - anchor_x
            } else {
                anchor_x
            };
            let screen = |position: isize, offset: i16, anchor: i16, scroll: usize| {
                i32::try_from(position + isize::from(offset) + isize::from(anchor))
                    .unwrap_or(i32::MAX)
                    - i32::try_from(scroll).unwrap_or(0)
            };
            let palette = self.object_palettes[actor.palette_slot]
                .as_ref()
                .unwrap_or(&sheet.palette);
            draw_sprite(
                frame,
                screen(x, record.x, anchor_x, scroll.0),
                screen(y, record.y, anchor_y, scroll.1),
                &image,
                palette,
                record.mirrored,
            );
        }
    }

    /// Keeps what the screen will show until the next call: the game copies
    /// its sprite table and scroll registers at the vertical blank, so a
    /// frame shows the positions, order, flips and camera of the frame
    /// before, while each sprite's picture, copied straight into video
    /// memory, is already the current one. Called once a frame, before the
    /// frame's update.
    pub fn latch(&mut self) {
        self.shown = Some(self.showing());
    }

    fn showing(&self) -> Shown {
        let sprites = self
            .order
            .iter()
            .filter_map(|&index| {
                let actor = self.actors.get(index)?;
                if !actor.visible || actor.sheet.is_none() {
                    return None;
                }
                let sheet = actor.sheet.as_ref()?;
                Some(ShownSprite {
                    actor: index,
                    position: actor.position(),
                    frame: *sheet.frames.get(actor.current_frame()?)?,
                    anchor: sheet.anchor,
                })
            })
            .collect();
        Shown {
            camera: self.camera(),
            sprites,
        }
    }
}

/// The sprites and camera a frame shows.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
    camera: (usize, usize),
    /// Front to back.
    sprites: Vec<ShownSprite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShownSprite {
    actor: usize,
    position: (isize, isize),
    /// The frame the sprite table names, and the anchor it was placed from.
    frame: SpriteFrame,
    anchor: (i16, i16),
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
    for actor in &mut actors {
        actor.animation_shift = standing_shift(actor.behavior);
    }
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
            anchor: (16, 16),
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

    fn tinted(color: u16) -> SpriteSheet {
        let mut sheet = sheet();
        sheet.palette = [color; 16];
        sheet
    }

    #[test]
    fn objects_sharing_a_palette_slot_show_the_last_ones_palette() {
        let mut field = field(8, 8);
        let mut first = Actor::new(Some(tinted(1)), (4, 4), Direction::Down);
        first.palette_slot = 1;
        let mut second = Actor::new(Some(tinted(2)), (5, 4), Direction::Down);
        second.palette_slot = 1;
        field.actors.extend([first, second]);
        field.load_object_palettes();
        assert_eq!(field.object_palettes[1], Some([2; 16]));
        field.set_sheet(1, tinted(3));
        assert_eq!(field.object_palettes[1], Some([3; 16]));
        assert_eq!(field.object_palettes[0], Some([0; 16]));
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

    fn zoid_field() -> Field {
        let mut field = field(8, 5);
        field.player_mut().behavior = ZOID_BEHAVIOR;
        field
    }

    fn enemy(column: usize, row: usize) -> Actor {
        let mut actor = character(column, row, None, ZOID_BEHAVIOR);
        actor.command = Command::Chase;
        actor
    }

    /// An explosion of two steps, the second a smaller frame: three ticks,
    /// then one.
    fn explosion() -> SpriteSheet {
        let mut sheet = sheet();
        sheet.tag = "ma14".to_owned();
        sheet.frames[1].width = 8;
        sheet.frames[1].height = 8;
        sheet.animations[0] = vec![
            AnimationStep {
                frame: 0,
                duration: 3,
            },
            AnimationStep {
                frame: 1,
                duration: 1,
            },
        ];
        sheet
    }

    #[test]
    fn a_wrecked_enemy_plays_its_animation_explodes_and_is_gone() {
        let mut field = zoid_field();
        field.actors.push(enemy(4, 1));
        field.actors[1].face(Direction::Left);
        field.wreck(1, explosion());
        let mut frames = 0;
        while field.actors[1]
            .sheet
            .as_ref()
            .map(|sheet| sheet.tag.as_str())
            == Some("ch00")
        {
            assert_eq!(field.update(Input::default()), None);
            frames += 1;
        }
        assert_eq!(frames, 17);
        assert_eq!(field.take_sounds(), [EXPLOSION_SOUND]);
        assert_eq!(field.actors[1].facing, Direction::Up);
        assert!(field.actors[1].visible);
        let mut gone = None;
        for frame in 0..4 {
            if let Some(event) = field.update(Input::default()) {
                gone = Some((frame, event));
                break;
            }
        }
        assert_eq!(gone, Some((1, FieldEvent::Wrecked { actor: 1 })));
        assert!(!field.actors[1].visible);
        assert!(field.take_sounds().is_empty());
    }

    #[test]
    fn a_wrecked_enemy_is_not_spoken_to() {
        let mut field = zoid_field();
        field.actors.push(enemy(4, 1));
        field.player_mut().face(Direction::Right);
        field.wreck(1, explosion());
        field.update(Input::default().with(Button::A));
        assert_eq!(field.actors[1].facing, Direction::Down);
    }

    #[test]
    fn the_carrier_meets_a_roaming_enemy_it_steps_into() {
        let mut field = zoid_field();
        field.actors.push(enemy(4, 1));
        assert_eq!(
            field.update(held(Direction::Right)),
            Some(FieldEvent::Encounter { enemy: 1 })
        );
        assert_eq!(field.actors[1].facing, Direction::Left);
        assert!(!field.player().walking());
    }

    #[test]
    fn intangible_enemies_let_the_carrier_through_without_a_battle() {
        let mut field = zoid_field();
        field.actors.push(enemy(4, 1));
        field.set_intangible(true);
        assert_eq!(field.update(held(Direction::Right)), None);
        assert!(field.player().walking());
    }

    #[test]
    fn an_enemy_nearby_heads_for_the_carrier_and_meets_it() {
        let mut field = zoid_field();
        field.actors.push(enemy(5, 1));
        let mut met = None;
        for _ in 0..200 {
            if let Some(event) = field.update(Input::default()) {
                met = Some(event);
                break;
            }
        }
        assert_eq!(met, Some(FieldEvent::Encounter { enemy: 1 }));
    }

    #[test]
    fn an_enemy_its_state_leaves_out_neither_shows_nor_meets_the_carrier() {
        let mut field = zoid_field();
        field.actors.push(enemy(5, 1));
        let Ok(()) = field.apply_object_states(&GameData::new(&[]), &[], None) else {
            panic!("no sprite is read for a state left out");
        };
        assert!(!field.actors[1].visible);
        let footing = field.actors[1].footing();
        for _ in 0..200 {
            assert_eq!(field.update(Input::default()), None);
        }
        assert_eq!(field.actors[1].footing(), footing);
    }

    #[test]
    fn townsfolk_and_paused_enemies_do_not_meet_the_player() {
        let mut field = zoid_field();
        field.actors.push(character(4, 1, None, 0));
        assert_eq!(field.update(held(Direction::Right)), None);
        let mut field = zoid_field();
        let mut resting = enemy(5, 1);
        resting.pause = 3;
        field.actors.push(resting);
        for _ in 0..3 {
            assert_eq!(field.update(Input::default()), None);
            assert!(!field.actors[1].walking());
        }
    }

    #[test]
    fn pushing_against_a_door_takes_it_without_a_step() {
        let mut field = field(6, 5);
        field.scene.attributes[2 * 6 + 4] = 0xC003;
        assert_eq!(
            field.update(held(Direction::Right)),
            Some(FieldEvent::Door(3))
        );
        assert_eq!((field.player().column, field.player().row), (3, 1));
        assert!(!field.player().walking());
        assert_eq!(field.player().facing, Direction::Right);
    }

    #[test]
    fn pushing_toward_a_portal_starts_the_trip_and_the_portal_is_found() {
        let mut field = field(6, 5);
        field.scene.attributes[2 * 6 + 4] = 0x4802;
        assert_eq!(field.portal(), None);
        let mut portal = character(4, 1, None, 0);
        portal.sprite = PORTAL_SPRITE;
        field.actors.push(portal);
        assert_eq!(
            field.update(held(Direction::Right)),
            Some(FieldEvent::Portal(2))
        );
        assert!(!field.player().walking());
        assert_eq!(field.player().facing, Direction::Right);
        assert_eq!(field.portal(), Some((1, (4, 1))));
    }

    #[test]
    fn a_walk_against_a_door_takes_it_as_its_step_ends() {
        let mut field = field(7, 5);
        field.scene.attributes[2 * 7 + 5] = 0xC001;
        field.player_mut().command = Command::WalkTo(Walk {
            column: 5,
            row: 1,
            speed: PIXEL,
            animation_shift: 1,
            through: false,
        });
        let events: Vec<_> = (0..16).map(|_| field.update(Input::default())).collect();
        assert_eq!(events[..15], [None; 15]);
        assert_eq!(events[15], Some(FieldEvent::Door(1)));
        assert_eq!((field.player().column, field.player().row), (4, 1));
    }

    #[test]
    fn b_held_runs_a_step_in_half_the_frames() {
        let mut field = field(6, 5);
        let running = held(Direction::Right).with(Button::B);
        field.update(running);
        assert_eq!(field.player().position(), (42, 16));
        assert_eq!(field.player().animation_shift, RUN_ANIMATION_SHIFT);
        for _ in 0..7 {
            field.update(Input::default());
        }
        assert_eq!(field.player().position(), (56, 16));
        assert!(!field.player().walking());
    }

    #[test]
    fn level_sprites_keep_the_order_earlier_swaps_left() {
        let mut field = field(8, 20);
        field.player_mut().place((5, 2));
        for cell in [(2, 4), (5, 2), (7, 2), (4, 16)] {
            field.actors.push(character(cell.0, cell.1, None, 0));
        }
        field.sort_actors();
        assert_eq!(field.order, [4, 1, 2, 3, 0]);
    }

    #[test]
    fn the_screen_shows_the_positions_of_the_frame_before() {
        let mut field = field(6, 5);
        field.latch();
        field.update(held(Direction::Right));
        assert_eq!(field.player().position(), (41, 16));
        field.latch();
        let shown = field.shown.as_ref().map(|shown| shown.sprites[0].position);
        field.update(Input::default());
        assert_eq!(shown, Some((41, 16)));
        assert_eq!(field.player().position(), (42, 16));
    }

    #[test]
    fn the_cell_left_stops_blocking_halfway_through_a_step() {
        let mut field = field(6, 5);
        field.update(held(Direction::Right));
        for _ in 0..7 {
            field.update(Input::default());
        }
        assert_eq!(field.player().previous, (3, 1));
        field.update(Input::default());
        assert_eq!(field.player().previous, (4, 1));
        assert!(field.player().walking());
    }

    #[test]
    fn a_shy_character_steps_away_from_a_running_player() {
        let mut field = field(8, 8);
        field.player_mut().place((5, 3));
        let mut shy = character(3, 3, None, 0);
        shy.command = Command::Shy;
        shy.timer = 50;
        field.actors.push(shy);
        field.update(Input::default().with(Button::B));
        let actor = &field.actors[1];
        assert!(actor.walking());
        assert_eq!(
            (actor.column, actor.row, actor.facing),
            (2, 3, Direction::Left)
        );
        for _ in 0..15 {
            field.update(Input::default().with(Button::B));
        }
        assert!(!field.actors[1].walking());
    }

    #[test]
    fn a_shy_character_only_wanders_while_b_is_up() {
        let mut field = field(8, 8);
        field.player_mut().place((5, 3));
        let mut shy = character(3, 3, None, 0);
        shy.command = Command::Shy;
        shy.timer = 50;
        field.actors.push(shy);
        field.update(Input::default());
        assert!(!field.actors[1].walking());
        assert_eq!(field.actors[1].timer, 49);
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
    fn a_pan_stops_at_the_map_edge_and_comes_back_from_it() {
        let mut field = field(20, 15);
        let step = 2 << FRACTION_BITS;
        for _ in 0..10 {
            field.pan_by(0, -step);
        }
        assert_eq!(field.camera().1, 0);
        for _ in 0..50 {
            field.pan_by(0, step);
        }
        assert_eq!(field.camera().1, 80);
        field.pan_by(0, -step);
        assert_eq!(field.camera().1, 78);
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
    fn the_player_speaks_across_a_counter() {
        let mut field = field(7, 5);
        let columns = field.scene.attribute_columns();
        field.scene.attributes[2 * columns + 4] = 0xA000;
        field
            .actors
            .push(character(5, 1, Some(ObjectScript::Code(0x0800_90F0)), 0));
        field.update(held(Direction::Right));
        assert_eq!(
            field.update(Input::default().with(Button::A)),
            Some(FieldEvent::Talk {
                actor: 1,
                script: ObjectScript::Code(0x0800_90F0)
            })
        );
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

    #[test]
    fn a_gliding_player_takes_the_camera_along_only_when_asked() {
        let mut large = field(40, 20);
        large.player_mut().place((13, 7));
        let pixel = 1 << FRACTION_BITS;
        for _ in 0..8 {
            large.glide_player(0, -pixel, false);
        }
        assert_eq!(large.player().position(), (200, 104));
        assert_eq!(large.camera(), (96, 48));
        for _ in 0..8 {
            large.glide_player(0, -pixel, true);
        }
        assert_eq!(large.player().position(), (200, 96));
        assert_eq!(large.camera(), (96, 40));
    }
}
