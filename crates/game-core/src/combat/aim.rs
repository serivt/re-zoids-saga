//! The player's aim in an attack scene (task `0x08045BF8` in slot 8): a
//! cursor on the chosen weapon with a window of its figures, then a grid
//! of the side it aims at, where the weapon's reach picks the groups of
//! targets a set of cursors moves between.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! task (`0x08045BF8`) and the scene's states that start and end it
//! (`0x080426D4`, `0x08042750`); the keys' repeat (`0x0800175C`), the
//! weapon cycle (`0x08047DAC`, `0x08047DD8`), the energy check
//! (`0x08045BA4`), the weapon's shape on the grid (`0x0804593C`), the first
//! group it takes (`0x080470D8`, `0x08047310`, ROM `0x6D43BC`), the moves
//! between groups (`0x08047754`, `0x08047184`), the cursors each group
//! sends (`0x080469D4`), the grid's animation for it (`0x080459F4`), its
//! opening and closing (`0x08045358`, `0x080455FC`, `0x080455B0`, ROM
//! `0x6D437C`), the icons (`0x08045238`, `0x0804502C`, ROM `0x6D4358`), the
//! figures (`0x08046784`, `0x08048F88`, ROM `0x6D4724`, `0x6D4778`), the
//! cursors' behaviors (`0x080492DC`, `0x08049560`) and the fade of the Zoid
//! for the weapon behind it (`0x080481D4`); checked against an attack
//! chosen in a battle on the world map in a reference emulator (the task's
//! states, its calls and the entity table frame by frame). See
//! `docs/combat.md`.
//!
//! The task runs the window scripts to their end before it goes on, and
//! the grid opens and closes in loops of their own: the port queues those
//! as steps that take their frames.

use std::collections::VecDeque;

use extraction::saga_battle::{self, EffectPiece};
use platform::{Button, Input};

use super::effects::{ANIMATES, ENDED, Entities, Entity, HOLDS_END, MIRRORED, VISIBLE};
use super::scene::SceneUnit;
use crate::ScriptHost;
use crate::data::GameData;
use crate::script::{ScriptError, ScriptRunner};
use crate::translation::PART_TABLE;
use crate::windows::ScriptWindows;
use extraction::saga_combat::SLOTS;

/// Frames a key must be held before it repeats, then between repeats
/// (`0x0800175C(0x10, 6)`).
const REPEAT_DELAY: u8 = 0x10;
const REPEAT_EVERY: u8 = 6;
const KEYS: usize = 10;
const KEY_A: u16 = 1;
const KEY_B: u16 = 2;
const KEY_RIGHT: u16 = 0x10;
const KEY_LEFT: u16 = 0x20;
const KEY_UP: u16 = 0x40;
const KEY_DOWN: u16 = 0x80;
const KEYS_SIDEWAYS: u16 = KEY_RIGHT | KEY_LEFT;
const KEYS_UPRIGHT: u16 = KEY_UP | KEY_DOWN;

// The entities of the aim.
const CURSOR: usize = 1;
const FIRST_LOCK: usize = 2;
const LOCKS: usize = 6;
const FIRST_PERCENT: usize = 8;
const FIRST_ICON: usize = 14;
const GRID: usize = 23;
const FIRST_DIGITS: usize = 24;
/// The entities the task's frame runs, from the last down (`0x08000DF4`
/// on 7 to 1), and those the grid's opening runs.
const FRAME_ENTITIES: usize = 7;
const OPENING_ENTITIES: usize = 19;

// Their sprites, of the effects' table.
const GRID_SPRITE: usize = 151;
const PERCENT_SPRITE: usize = 152;
const DIGITS_SPRITE: usize = 153;
const CURSOR_SPRITE: usize = 154;
const CURSOR_ANIMATION: usize = 0;
const LOCK_ANIMATION: usize = 1;
const LOCKED_ANIMATION: usize = 2;
// OBJ priorities (the entities' `+6`).
const PRIORITY_FIGURES: u8 = 0;
const PRIORITY_CURSORS: u8 = 1;
const PRIORITY_GRID: u8 = 2;

/// Where the grid and the figures start (`0x08045174`, `0x080490E4`).
const GRID_AT: (i32, i32) = (16, 8);
const FIGURES_AT: (i32, i32) = (30, 30);
/// The cells' icons (ROM `0x6D4358`): the front column at x 64, the back
/// one at 32, from the bottom up.
const ICON_X: [i32; SLOTS] = [64, 64, 64, 32, 32, 32];
const ICON_Y: [i32; SLOTS] = [112, 80, 48, 112, 80, 48];
/// The slot in each cell (ROM `0x6D4370`): the party's grid shows its
/// side mirrored.
const CELL_SLOTS: [[usize; SLOTS]; 2] = [[5, 4, 3, 2, 1, 0], [0, 1, 2, 3, 4, 5]];
/// The party side's Zoid sits 112 pixels right on the screen; the back
/// rack's cursor 16 pixels above its mount (`0x080492DC`).
const PARTY_OFFSET: i32 = 0x70;
const BACK_RACK_RAISE: i32 = 16;
const BACK_RACK: usize = 1;
/// The racks with an entity of their own; the others' places come from
/// the Zoid's record alone.
const RACKS: usize = 3;
/// A locking cursor stops 25 pixels above its icon; the percent shows 15
/// right of it, the hit points 10 right and 8 up (`0x08049560`).
const LOCK_ABOVE: i32 = 25;
const PERCENT_OFFSET: (i32, i32) = (15, 0);
const DIGITS_OFFSET: (i32, i32) = (10, -8);
/// The frame the weapon's window's closing costs the scene: the other
/// windows' redraw runs past it.
const CLOSE_STALL: u8 = 1;
/// The frames the weapon's figures take to draw: the scene's tasks lose
/// two, the task itself one before it shows the window.
const FIGURES_STALL: u8 = 2;
/// Steps of a pixel the cursors take a frame.
const CURSOR_STEPS: usize = 3;
const LOCK_STEPS: usize = 10;
/// The percent's count, 8 a frame.
const PERCENT_COUNT: usize = 8;
/// The hit points the grid shows as `????`: an enemy's with trait `0x200`.
pub const UNKNOWN_HP: u16 = 10_000;
/// The digits' pieces (ROM `0x6D4724`): four 8×8 cells, and the tile of
/// each digit, of a blank and of `?` (ROM `0x6D4778`).
const DIGIT_X: [i16; 4] = [-13, -6, 1, 8];
const DIGIT_Y: i16 = -8;
const DIGIT_TILES: [u16; 12] = [3, 2, 1, 0, 11, 10, 9, 8, 7, 6, 4, 5];
const BLANK_DIGIT: usize = 10;
const UNKNOWN_DIGIT: usize = 11;
/// The grid's and the icons' ratios as they open (ROM `0x6D439C` and
/// `0x6D437C`), from the smallest.
const GRID_RATIO_FROM: i16 = 0x4C0;
const ICON_RATIO_FROM: i16 = 0x560;
const RATIO_STEP: i16 = 0x40;
const OPENING_STEPS: u16 = 16;
/// The opening's frames: 15 of the grid, then 16 of the icons.
const GRID_FRAMES: u16 = OPENING_STEPS - 1;
const OPENING_FRAMES: u16 = GRID_FRAMES + OPENING_STEPS;
const ANGLE_STEP: u16 = 0x400;

// Sounds.
const SOUND_OPEN: u16 = 0x64;
const SOUND_MOVE: u16 = 0x40;
const SOUND_WEAPON: u16 = 0x43;
const SOUND_TARGET: u16 = 0x42;
const SOUND_GROUP: u16 = 0x48;
const SOUND_CANCEL: u16 = 0x3F;
const SOUND_REFUSED: u16 = 0x4F;

// Scripts of the `system` table (`0x0803E4E8`).
/// The message window's prompt, 選択して下さい。
const SCRIPT_PROMPT: usize = 0xC;
/// The weapon's window: opened with the prompt (0x10), cleared and drawn
/// (0xD), its lines printed (0x11) and shown (0xE); it closes for the grid
/// (0xF).
const INFO_WINDOW: u8 = 2;
const SCRIPT_SHOW_INFO: usize = 0xE;
const SCRIPT_OPEN_INFO: usize = 0x10;
const SCRIPT_FIGURES: usize = 0x11;
/// 距離が近すぎます or 距離が遠すぎます by variable 0, and ＥＰが足りません.
const SCRIPT_OUT_OF_REACH: usize = 0x13;
const SCRIPT_NO_ENERGY: usize = 0x14;
// Its variables (EWRAM `0x02007574`).
const VAR_REACH: usize = 0;
const VAR_POWER: usize = 2;
const VAR_ACCURACY: usize = 3;
const VAR_CODE: usize = 4;
const VAR_VALUE: usize = 6;
const VAR_SLOT: usize = 7;
const MAX_POWER: u16 = 999;
const MAX_ACCURACY: u16 = 99;

// The weapons' flags (`+0x58`).
const OFFENSIVE: u32 = 1;
const OWN_SIDE: u32 = 2;
const SELF_ONLY: u32 = 4;
const EVERYONE: u32 = 8;
/// A part the grid doesn't remember as the unit's last weapon.
const UNREMEMBERED_PART: u16 = 0x70;
/// The first of the fixed weapons' slots, which some reaches take
/// differently.
const FIXED_SLOTS: usize = 3;
/// The slot of the rack behind the Zoid, which fades the Zoid to show it.
const BEHIND_SLOT: usize = 2;

const PARTY: usize = 0;
const ENEMY: usize = 1;

/// A weapon the unit can aim, as the task reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimWeapon {
    /// Its part.
    pub part: u16,
    /// Its power as the window shows it: its turn's power, rounded, at most
    /// 999, and 0 for a weapon of less than 1.
    pub power: u16,
    /// Its accuracy, at most 99, and 0 for one aimed at its own side.
    pub accuracy: u16,
    /// Its reach's code (`+0xF`), which picks its shape on the grid.
    pub code: u8,
    /// The energy it costs.
    pub cost: u16,
    /// Its flags (`+0x58`).
    pub flags: u32,
    /// Its chance to hit each unit, by side and slot.
    pub chances: [[u16; SLOTS]; 2],
}

/// A unit the grid can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimUnit {
    /// Its Zoid, whose icon shows.
    pub zoid: u16,
    /// Its hit points, [`UNKNOWN_HP`] for those the grid hides.
    pub hp: u16,
    /// Its scene record, for the target's view.
    pub view: SceneUnit,
}

/// The weapon a unit last aimed (EWRAM `0x0200E24E`, `0x0200E254`,
/// `0x0200E260`), which the next aim starts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimMemory {
    /// The unit's Zoid then.
    pub zoid: u16,
    /// The weapon's slot.
    pub slot: usize,
    /// Its part.
    pub part: u16,
}

/// What the aim starts from.
#[derive(Debug, Clone)]
pub struct AimSetup {
    /// The attacker's Zoid.
    pub zoid: u16,
    /// Its slot on the party's side.
    pub slot: usize,
    /// Its energy.
    pub ep: i32,
    /// Its scene record's parts: the weapons the cursor stops on.
    pub parts: [u16; SLOTS],
    /// Its weapons.
    pub weapons: [Option<AimWeapon>; SLOTS],
    /// The units still fighting, by side and slot.
    pub units: [[Option<AimUnit>; SLOTS]; 2],
    /// The weapon it last aimed.
    pub memory: Option<AimMemory>,
}

/// How the aim ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AimOutcome {
    /// A weapon and its targets.
    Chosen {
        /// The weapon's slot.
        weapon: usize,
        /// The side it aims at.
        side: usize,
        /// The targets' slots, in order.
        targets: Vec<usize>,
    },
    /// The player went back to the battle's menu.
    Cancelled,
}

/// The task's states, by the original's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// 0.
    Start,
    /// 9: the prompt, once the cursor stands on the weapon.
    Prompt,
    /// 1: picking the weapon.
    Weapon,
    /// 100 and 0x65: the Zoid's fade back, then 2 or 7.
    Leaving {
        cancel: bool,
    },
    /// 2: the weapon's targets.
    Grid,
    /// 3: picking the targets.
    Targets,
    /// 4: the figures hide; 5 once the cursors have settled.
    Confirm,
    Close,
    /// 6 and 7: the result.
    Chosen,
    Cancelled,
    /// 200 and 10: the weapon reaches nothing.
    Refused,
    Back,
    /// 0xE: a weapon without a grid, once the cursors have settled.
    NoGrid,
}

/// A step of the task that takes frames of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    /// A `system` script, run to its end (`0x0803E4E8`).
    Script(usize),
    /// A part's name (`0x0803E51C` on the part table).
    PartName(u16),
    Sound(u16),
    /// The task's frame: its cursors' behaviors, then the wait
    /// (`0x08000DF4` on 7 to 1, `0x0805EF90(1)`).
    Frame,
    /// Frame `frame` of the grid's opening (`0x08045358(…, 0)`): 15 of the
    /// grid growing, then 16 of the icons.
    Open(u16),
    /// Frame `frame` of the closing, `wait` frames a step (`0x08045358(…,
    /// 1 or 2)`).
    Shut {
        frame: u16,
        wait: u16,
    },
    /// Waits for the weapon's cursor to end its animation.
    AwaitCursor,
    /// Script 0xD as the trace showed it: the window cleared and drawn at
    /// once, then the flush's frame.
    ClearInfo,
    /// Script 0xF as the trace showed it: the window goes at the end of
    /// the first of its two frames, as the others are redrawn.
    CloseInfo(u8),
    /// Frames the task waits.
    Pause(u8),
    /// Frames the scene's own tasks lose while the task draws the window's
    /// text, which runs past the frame.
    Stall(u8),
    /// The task's own changes between them.
    Enter(State),
    Group(u8),
    Animate(usize, usize),
    Figures,
    HideAim,
}

/// How a step of the task leaves the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Progress {
    /// Done; the task goes on in this frame.
    Next,
    /// Done, and the frame is over.
    Yield,
    /// Not done: it goes on in the next frame.
    Wait,
}

/// What a cursor's behavior keeps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Flight {
    /// Its behavior's state (`+0x50`) and sub-state (`+0x56`).
    phase: u16,
    sub: u16,
    /// Whether it is still moving (`+0x4C`).
    busy: bool,
    /// The cell it goes to (`+0x54`), and the last it counted (`+0x58`).
    cell: Option<usize>,
    counted: Option<usize>,
    /// Its line: where it goes, the steps, the error and the major axis.
    target: (i32, i32),
    step: (i32, i32),
    delta: (u32, u32),
    error: u32,
    along_y: bool,
}

/// The fade of the Zoid that shows the weapon behind it (`0x080481D4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blend {
    /// The Zoid's layer blends over the scenery (`BLDCNT` first target
    /// BG1).
    pub zoid: bool,
    /// `BLDALPHA`: the first target's and the second's sixteenths.
    pub alpha: (u8, u8),
}

/// The aim being shown.
pub struct Aim {
    setup: AimSetup,
    state: State,
    ops: VecDeque<Op>,
    running: bool,
    /// Frames the scene's own tasks are to lose.
    stall: u8,
    parts: ScriptRunner,
    vars: [u16; 8],
    repeat: Repeat,
    keys: u16,
    /// The weapon's slot (variable 7) and whether the cursor stands on it
    /// (variable 5).
    slot: usize,
    arrived: bool,
    /// The fade's counter (`[sp + 0x24]`).
    pulse: u8,
    /// A message replaced the prompt (`[sp + 0x30]`).
    notice: bool,
    /// The weapon the grid last opened for, and its group.
    grid_weapon: Option<usize>,
    group: u8,
    shape: u8,
    side: usize,
    /// The targets (EWRAM `0x0200DA80`), by side and slot.
    chosen: [[bool; SLOTS]; 2],
    /// Each cell's chance and hit points (`0x0200E270`, `0x0200E276`), and
    /// the hit points its count shows (`0x0200E4C6`).
    chances: [u16; SLOTS],
    hp: [u16; SLOTS],
    counted_hp: [u16; SLOTS],
    flights: [Flight; FRAME_ENTITIES],
    /// Where the cursor points for each weapon.
    places: [(i32, i32); SLOTS],
    icons: [bool; SLOTS],
    digit_sprites: [usize; LOCKS],
    sounds: Vec<u16>,
    outcome: Option<AimOutcome>,
    memory: Option<AimMemory>,
    blend: Option<Blend>,
}

impl Aim {
    /// The aim's sprites (the scene's state `0x1052`: the grid, the
    /// figures and the cursors on the first weapon), then its task.
    pub fn new(data: &GameData<'_>, setup: AimSetup, entities: &mut Entities) -> Self {
        let rom = data.bytes();
        let remembered = setup
            .memory
            .filter(|memory| memory.zoid == setup.zoid)
            .map_or(0, |memory| memory.slot);
        let slot = next_weapon(&setup.parts, remembered);
        let parts = ScriptRunner::named(
            PART_TABLE,
            data.script_offsets(PART_TABLE)
                .ok()
                .flatten()
                .unwrap_or_default(),
        );
        let mut aim = Self {
            setup,
            state: State::Start,
            ops: VecDeque::new(),
            running: false,
            stall: 0,
            parts,
            vars: [0; 8],
            repeat: Repeat::default(),
            keys: 0,
            slot,
            arrived: false,
            pulse: 0,
            notice: false,
            grid_weapon: None,
            group: 0,
            shape: 0,
            side: ENEMY,
            chosen: [[false; SLOTS]; 2],
            chances: [0; SLOTS],
            hp: [0; SLOTS],
            counted_hp: [0; SLOTS],
            flights: [Flight::default(); FRAME_ENTITIES],
            places: [(0, 0); SLOTS],
            icons: [false; SLOTS],
            digit_sprites: [0; LOCKS],
            sounds: Vec::new(),
            outcome: None,
            memory: None,
            blend: None,
        };
        aim.places = std::array::from_fn(|slot| aim.weapon_place(rom, slot));
        aim.spawn(rom, entities);
        aim
    }

    fn spawn(&mut self, rom: &[u8], entities: &mut Entities) {
        if let Some(sprite) = saga_battle::effect_sprite(rom, GRID_SPRITE) {
            let sprite = entities.sprite_index(sprite);
            let mut grid = Entity::new(sprite, GRID);
            grid.priority = PRIORITY_GRID;
            (grid.x, grid.y) = (GRID_AT.0 << 16, GRID_AT.1 << 16);
            entities.put(GRID, Some(grid));
            entities.animate(GRID, 0);
        }
        let percent = saga_battle::effect_sprite(rom, PERCENT_SPRITE);
        if let Some(sprite) = percent.clone() {
            let sprite = entities.sprite_index(sprite);
            for k in 0..LOCKS {
                let mut entity = Entity::new(sprite, FIRST_PERCENT + k);
                entity.priority = PRIORITY_FIGURES;
                if k == 0 {
                    (entity.x, entity.y) = (FIGURES_AT.0 << 16, FIGURES_AT.1 << 16);
                }
                entities.put(FIRST_PERCENT + k, Some(entity));
                entities.animate(FIRST_PERCENT + k, 0);
            }
        }
        // The hit points' frames are built in memory, four digits each, in
        // the percent's colors.
        if let Some(mut digits) = saga_battle::effect_sprite(rom, DIGITS_SPRITE) {
            if let Some(percent) = &percent {
                digits.palette = percent.palette;
            }
            for k in 0..LOCKS {
                let mut sprite = digits.clone();
                sprite.frames = vec![digit_pieces(0)];
                sprite.animation = vec![extraction::saga::AnimationStep {
                    frame: 0,
                    duration: 1,
                }];
                sprite.animations = vec![sprite.animation.clone()];
                let sprite = entities.sprite_index(sprite);
                self.digit_sprites[k] = sprite;
                let mut entity = Entity::new(sprite, FIRST_DIGITS + k);
                entity.priority = PRIORITY_FIGURES;
                if k == 0 {
                    (entity.x, entity.y) = (FIGURES_AT.0 << 16, FIGURES_AT.1 << 16);
                }
                entities.put(FIRST_DIGITS + k, Some(entity));
                entities.animate(FIRST_DIGITS + k, 0);
            }
        }
        if let Some(sprite) = saga_battle::effect_sprite(rom, CURSOR_SPRITE) {
            let sprite = entities.sprite_index(sprite);
            let at = self.places[self.slot];
            for index in CURSOR..FIRST_LOCK + LOCKS {
                let mut cursor = Entity::new(sprite, index);
                cursor.priority = PRIORITY_CURSORS;
                cursor.set(VISIBLE, true);
                cursor.set(ANIMATES, true);
                (cursor.x, cursor.y) = (at.0 << 16, at.1 << 16);
                entities.put(index, Some(cursor));
                entities.animate(index, CURSOR_ANIMATION);
            }
        }
        self.flights = [Flight::default(); FRAME_ENTITIES];
        for flight in &mut self.flights {
            flight.busy = true;
        }
    }

    /// Where the cursor points for weapon slot `slot`: the weapon's place
    /// on the Zoid, 112 pixels right, the back rack's 16 higher.
    fn weapon_place(&self, rom: &[u8], slot: usize) -> (i32, i32) {
        let (x, y) = saga_battle::weapon_mount(rom, self.setup.zoid, slot).unwrap_or((0, 0));
        let raise = if slot == BACK_RACK && slot < RACKS {
            BACK_RACK_RAISE
        } else {
            0
        };
        (i32::from(x) + PARTY_OFFSET, i32::from(y) - raise)
    }

    /// How the aim ended, once it has.
    #[must_use]
    pub fn outcome(&self) -> Option<&AimOutcome> {
        self.outcome.as_ref()
    }

    /// The weapon the unit starts on next time, once the grid has opened
    /// for it; taken once.
    pub fn take_memory(&mut self) -> Option<AimMemory> {
        self.memory.take()
    }

    /// The sounds requested since the last call.
    pub fn take_sounds(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.sounds)
    }

    /// The frames the scene's own tasks lose from the next, taken once.
    pub fn take_stall(&mut self) -> u8 {
        std::mem::take(&mut self.stall)
    }

    /// The fade of the Zoid, while it shows the weapon behind it.
    #[must_use]
    pub fn blend(&self) -> Option<Blend> {
        self.blend
    }

    /// The aim's state by the original's numbers, for comparing runs.
    #[must_use]
    pub fn state(&self) -> u16 {
        match self.state {
            State::Start => 0,
            State::Prompt => 9,
            State::Weapon => 1,
            State::Leaving { cancel: false } => 100,
            State::Leaving { cancel: true } => 0x65,
            State::Grid => 2,
            State::Targets => 3,
            State::Confirm => 4,
            State::Close => 5,
            State::Chosen => 6,
            State::Cancelled => 7,
            State::Refused => 200,
            State::Back => 10,
            State::NoGrid => 0xE,
        }
    }

    /// One frame of the task.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub fn update(
        &mut self,
        rom: &[u8],
        input: Input,
        system: &mut ScriptRunner,
        windows: &mut ScriptWindows<'_>,
        entities: &mut Entities,
    ) -> Result<(), ScriptError> {
        if self.ops.is_empty() {
            self.keys = self.repeat.update(held_keys(input));
            self.step(rom, entities);
            self.ops.push_back(Op::Frame);
        }
        while let Some(op) = self.ops.front().copied() {
            let progress = self.run(rom, op, input, system, windows, entities)?;
            if progress != Progress::Wait {
                self.ops.pop_front();
            }
            if progress != Progress::Next {
                break;
            }
        }
        Ok(())
    }

    /// Runs `op` for this frame.
    fn run(
        &mut self,
        rom: &[u8],
        op: Op,
        input: Input,
        system: &mut ScriptRunner,
        windows: &mut ScriptWindows<'_>,
        entities: &mut Entities,
    ) -> Result<Progress, ScriptError> {
        let progress = match op {
            Op::Script(_) | Op::PartName(_) | Op::ClearInfo | Op::CloseInfo(_) => {
                return self.run_window_op(rom, op, input, system, windows);
            }
            Op::Sound(sound) => {
                self.sounds.push(sound);
                Progress::Next
            }
            Op::Frame => {
                self.behave(entities, FRAME_ENTITIES);
                Progress::Yield
            }
            Op::Open(frame) => {
                if frame >= OPENING_FRAMES {
                    return Ok(Progress::Next);
                }
                self.open_step(entities, frame);
                self.advance_op(Op::Open(frame + 1));
                Progress::Wait
            }
            Op::Shut { frame, wait } => {
                if self.shut_step(entities, frame, wait) {
                    return Ok(Progress::Next);
                }
                self.advance_op(Op::Shut {
                    frame: frame + 1,
                    wait,
                });
                Progress::Wait
            }
            Op::Pause(frames) => {
                if frames == 0 {
                    Progress::Next
                } else {
                    self.advance_op(Op::Pause(frames - 1));
                    Progress::Wait
                }
            }
            Op::Stall(frames) => {
                self.stall = frames;
                Progress::Next
            }
            Op::AwaitCursor => {
                if entities.get(CURSOR).is_none_or(|cursor| cursor.is(ENDED)) {
                    Progress::Next
                } else {
                    Progress::Wait
                }
            }
            Op::Enter(state) => {
                self.state = state;
                Progress::Next
            }
            Op::Group(group) => {
                self.set_group(group);
                Progress::Next
            }
            Op::Animate(index, animation) => {
                entities.animate(index, animation);
                Progress::Next
            }
            Op::Figures => {
                self.figures();
                Progress::Next
            }
            Op::HideAim => {
                for index in
                    (CURSOR..FIRST_ICON + LOCKS - 1).chain(FIRST_DIGITS..FIRST_DIGITS + LOCKS)
                {
                    if let Some(entity) = entities.get_mut(index) {
                        entity.flags = 0;
                    }
                }
                // Their behaviors go on; only their flags and `+0x4C` go.
                for flight in &mut self.flights {
                    flight.busy = false;
                }
                Progress::Next
            }
        };
        Ok(progress)
    }

    /// Runs a step of the windows: a script, or one of those the port
    /// times as the trace showed them.
    fn run_window_op(
        &mut self,
        rom: &[u8],
        op: Op,
        input: Input,
        system: &mut ScriptRunner,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<Progress, ScriptError> {
        let progress = match op {
            Op::Script(index) => {
                if !self.running {
                    system.start(index)?;
                    system.set_vars(self.vars);
                    self.running = true;
                }
                if system.update(rom, input, windows)? {
                    self.running = false;
                    Progress::Next
                } else {
                    Progress::Wait
                }
            }
            Op::PartName(part) => {
                if !self.running {
                    self.parts.resume(system.context());
                    self.parts.start(usize::from(part))?;
                    self.parts.set_vars(self.vars);
                    self.running = true;
                }
                if self.parts.update(rom, input, windows)? {
                    self.running = false;
                    system.resume(self.parts.context());
                    Progress::Next
                } else {
                    Progress::Wait
                }
            }
            Op::ClearInfo => {
                windows.clear_window(INFO_WINDOW);
                windows.draw_window(INFO_WINDOW);
                system.select_window(INFO_WINDOW);
                self.ops[0] = Op::Pause(1);
                Progress::Wait
            }
            Op::CloseInfo(frame) => match frame {
                0 => {
                    self.advance_op(Op::CloseInfo(1));
                    Progress::Wait
                }
                1 => {
                    windows.close_window(Some(INFO_WINDOW));
                    self.advance_op(Op::CloseInfo(2));
                    Progress::Wait
                }
                _ => Progress::Next,
            },
            _ => Progress::Next,
        };
        Ok(progress)
    }

    /// Replaces the op at the queue's front, for its next frame.
    fn advance_op(&mut self, op: Op) {
        if let Some(front) = self.ops.front_mut() {
            *front = op;
        }
    }

    /// The task's state this frame, queuing what takes frames.
    fn step(&mut self, rom: &[u8], entities: &mut Entities) {
        let keys = self.keys;
        match self.state {
            State::Start => {
                self.notice = false;
                self.arrived = false;
                self.vars[VAR_SLOT] = u16::try_from(self.slot).unwrap_or(0);
                self.ops.push_back(Op::Sound(SOUND_OPEN));
                self.state = State::Prompt;
            }
            State::Prompt => {
                if self.arrived {
                    self.ops.push_back(Op::Script(SCRIPT_PROMPT));
                    self.ops.push_back(Op::Script(SCRIPT_OPEN_INFO));
                    self.state = State::Weapon;
                }
            }
            State::Weapon => self.step_weapon(entities, keys),
            State::Leaving { cancel } => {
                self.fade_back(entities);
                if self.pulse == 0 {
                    self.state = if cancel {
                        State::Cancelled
                    } else {
                        State::Grid
                    };
                }
            }
            State::Grid => self.step_grid(rom, entities),
            State::Targets => self.step_targets(entities, keys),
            State::Confirm => {
                for k in 0..LOCKS {
                    for index in [FIRST_PERCENT + k, FIRST_DIGITS + k] {
                        if let Some(entity) = entities.get_mut(index) {
                            entity.set(VISIBLE, false);
                        }
                    }
                }
                if self.settled() {
                    self.state = State::Close;
                }
            }
            State::Close => {
                for (k, flight) in self.flights.iter().enumerate().skip(1) {
                    if flight.phase == 1 {
                        entities.animate(CURSOR + k, LOCKED_ANIMATION);
                    }
                }
                for index in CURSOR..FIRST_LOCK + LOCKS {
                    if let Some(entity) = entities.get_mut(index) {
                        entity.set(ANIMATES, true);
                        entity.set(HOLDS_END, true);
                    }
                }
                self.queue_shut(2);
                self.ops.push_back(Op::AwaitCursor);
                self.ops.push_back(Op::HideAim);
                self.ops.push_back(Op::Enter(State::Chosen));
            }
            State::Chosen => {
                self.ops.push_back(Op::HideAim);
                self.outcome = Some(AimOutcome::Chosen {
                    weapon: self.slot,
                    side: self.side,
                    targets: (0..SLOTS)
                        .filter(|&slot| self.chosen[self.side][slot])
                        .collect(),
                });
            }
            State::Cancelled => self.outcome = Some(AimOutcome::Cancelled),
            State::Refused => {
                self.ops.push_back(Op::Script(SCRIPT_OUT_OF_REACH));
                self.ops.push_back(Op::Enter(State::Back));
            }
            State::Back => {
                self.pulse = 0;
                self.state = State::Weapon;
            }
            State::NoGrid => {
                if self.settled() {
                    self.state = State::Chosen;
                }
            }
        }
    }

    /// State 1: up and down move between the weapons, A takes one the
    /// unit has the energy for, B goes back; the window shows the weapon
    /// once the cursor stands on it.
    fn step_weapon(&mut self, entities: &mut Entities, keys: u16) {
        for (key, forward) in [(KEY_UP, false), (KEY_DOWN, true)] {
            if keys & key == 0 {
                continue;
            }
            if self.notice {
                self.ops.push_back(Op::Script(SCRIPT_PROMPT));
                self.notice = false;
            }
            self.ops.push_back(Op::Sound(SOUND_MOVE));
            let next = if forward {
                next_weapon(&self.setup.parts, (self.slot + 1) % SLOTS)
            } else {
                previous_weapon(&self.setup.parts, self.slot.checked_sub(1))
            };
            if next != self.slot {
                self.slot = next;
                self.vars[VAR_SLOT] = u16::try_from(next).unwrap_or(0);
                self.flights[0] = Flight::default();
                self.pulse = 0;
                if let Some(cursor) = entities.get_mut(CURSOR) {
                    cursor.set(VISIBLE, true);
                }
            }
        }
        if self.slot == BEHIND_SLOT {
            if self.pulse != 0xFF {
                self.fade(entities);
                if self.pulse == 0 {
                    self.pulse = 0xFF;
                }
            }
        } else {
            self.fade_back(entities);
        }
        if keys & KEY_A != 0 {
            let weapon = self.setup.weapons[self.slot];
            let cost = weapon.map_or(0, |weapon| i32::from(weapon.cost));
            if self.setup.ep < cost {
                self.ops.push_back(Op::Sound(SOUND_REFUSED));
                self.vars[VAR_VALUE] = u16::try_from(self.setup.ep.max(0)).unwrap_or(0);
                self.ops.push_back(Op::Script(SCRIPT_NO_ENERGY));
                self.notice = true;
            } else {
                if self.notice && self.grid_weapon != Some(self.slot) {
                    self.ops.push_back(Op::Script(SCRIPT_PROMPT));
                    self.notice = false;
                }
                self.arrived = false;
                self.pulse = 0;
                self.state = State::Leaving { cancel: false };
            }
        } else if keys & KEY_B != 0 {
            self.arrived = false;
            self.ops.push_back(Op::Sound(SOUND_CANCEL));
            self.pulse = 0;
            self.state = State::Leaving { cancel: true };
        } else if self.arrived {
            self.show_weapon();
            self.arrived = false;
        }
    }

    /// The weapon's window: its name, power, accuracy, reach and cost.
    fn show_weapon(&mut self) {
        let weapon = self.setup.weapons[self.slot];
        let part = self.setup.parts[self.slot];
        self.ops.push_back(Op::ClearInfo);
        self.vars[VAR_VALUE] = part;
        self.ops
            .push_back(Op::PartName(weapon.map_or(part, |weapon| weapon.part)));
        if let Some(weapon) = weapon {
            self.vars[VAR_POWER] = weapon.power.min(MAX_POWER);
            self.vars[VAR_ACCURACY] = if weapon.flags & (OWN_SIDE | SELF_ONLY) == 0 {
                weapon.accuracy.min(MAX_ACCURACY)
            } else {
                0
            };
            self.vars[VAR_CODE] = u16::from(weapon.code);
            self.vars[VAR_VALUE] = weapon.cost;
        }
        self.ops.push_back(Op::Script(SCRIPT_FIGURES));
        self.ops.push_back(Op::Stall(FIGURES_STALL));
        self.ops.push_back(Op::Pause(FIGURES_STALL - 1));
        self.ops.push_back(Op::Script(SCRIPT_SHOW_INFO));
    }

    /// State 2: the weapon's side and grid, or its targets at once for
    /// one without a grid.
    fn step_grid(&mut self, rom: &[u8], entities: &mut Entities) {
        self.chosen = [[false; SLOTS]; 2];
        let Some(weapon) = self.setup.weapons[self.slot] else {
            self.state = State::Back;
            return;
        };
        let actor = self.setup.slot;
        let remember = |part: u16| AimMemory {
            zoid: self.setup.zoid,
            slot: self.slot,
            part,
        };
        if weapon.flags & SELF_ONLY != 0 {
            self.ops.push_back(Op::CloseInfo(0));
            self.ops.push_back(Op::Sound(SOUND_TARGET));
            self.chosen[PARTY][actor] = true;
            self.side = PARTY;
            self.grid_weapon = Some(self.slot);
            self.state = State::NoGrid;
            return;
        }
        if weapon.flags & OFFENSIVE != 0 {
            self.side = ENEMY;
            if weapon.part != UNREMEMBERED_PART {
                self.memory = Some(remember(weapon.part));
            }
        } else if weapon.flags & OWN_SIDE != 0 {
            self.side = PARTY;
        } else if weapon.flags & EVERYONE != 0 {
            self.ops.push_back(Op::CloseInfo(0));
            self.ops.push_back(Op::Sound(SOUND_TARGET));
            self.chosen = [[true; SLOTS]; 2];
            self.side = ENEMY;
            self.memory = Some(remember(weapon.part));
            self.grid_weapon = Some(self.slot);
            self.state = State::NoGrid;
            return;
        }
        let shape = shape(weapon.code, self.slot >= FIXED_SLOTS);
        self.show_icons(rom, entities);
        if self.grid_weapon != Some(self.slot) {
            self.group = first_group(shape, self.icons);
        }
        self.grid_weapon = Some(self.slot);
        if self.group >= 0xFE {
            self.ops.push_back(Op::Sound(SOUND_REFUSED));
            self.vars[VAR_REACH] = u16::from(self.group == 0xFF);
            self.notice = true;
            self.state = State::Refused;
            return;
        }
        self.shape = shape;
        self.ops.push_back(Op::Stall(CLOSE_STALL));
        self.ops.push_back(Op::CloseInfo(0));
        self.ops.push_back(Op::Sound(SOUND_WEAPON));
        self.ops.push_back(Op::Group(self.group));
        self.ops.push_back(Op::Figures);
        self.ops.push_back(Op::Open(0));
        self.ops
            .push_back(Op::Animate(GRID, grid_animation(self.shape, self.group)));
        self.ops.push_back(Op::Enter(State::Targets));
    }

    /// The icons of the side the weapon aims at (`0x08045238`), hidden
    /// until the grid opens.
    fn show_icons(&mut self, rom: &[u8], entities: &mut Entities) {
        let side = self.side;
        for cell in 0..SLOTS {
            let slot = CELL_SLOTS[side][cell];
            let index = FIRST_ICON + cell;
            self.icons[cell] = false;
            entities.put(index, None);
            let Some(unit) = self.setup.units[side][slot] else {
                continue;
            };
            let Some(sprite) = saga_battle::zoid_icon(rom, unit.zoid) else {
                continue;
            };
            let sprite = entities.sprite_index(sprite);
            let mut icon = Entity::new(sprite, index);
            icon.priority = PRIORITY_GRID;
            icon.set(MIRRORED, side == PARTY);
            icon.affine = Some((0, 0, 0));
            (icon.x, icon.y) = (ICON_X[cell] << 16, ICON_Y[cell] << 16);
            entities.put(index, Some(icon));
            entities.animate(index, 0);
            self.icons[cell] = true;
        }
    }

    /// State 3: the arrows move between the shape's groups, A takes the
    /// group, B goes back to the weapons with the grid closing.
    fn step_targets(&mut self, entities: &mut Entities, keys: u16) {
        for key in [KEY_UP, KEY_DOWN, KEY_LEFT, KEY_RIGHT] {
            if keys & key == 0 {
                continue;
            }
            let next = navigate(self.shape, self.group, keys, self.icons);
            if next != self.group && valid(next, self.icons) {
                self.sounds.push(SOUND_GROUP);
                self.set_group(next);
                self.figures();
                entities.animate(GRID, grid_animation(self.shape, self.group));
            }
        }
        if keys & KEY_B != 0 {
            self.ops.push_back(Op::Sound(SOUND_CANCEL));
            for flight in self.flights.iter_mut().skip(1) {
                flight.phase = 0;
                flight.sub = 0;
            }
            self.queue_shut(1);
            self.ops.push_back(Op::Script(SCRIPT_OPEN_INFO));
            self.arrived = true;
            self.ops.push_back(Op::Enter(State::Back));
            self.ops.push_back(Op::Animate(GRID, 0));
        } else if keys & KEY_A != 0 {
            self.ops.push_back(Op::Sound(SOUND_TARGET));
            self.state = State::Confirm;
        }
    }

    /// Whether every cursor has stopped (`+0x4C` of 1 to 7).
    fn settled(&self) -> bool {
        self.flights.iter().all(|flight| !flight.busy)
    }

    /// The group `group` of the shape (`0x080469D4`): its cells' targets,
    /// and a cursor sent to each cell with a unit, from the seventh down.
    fn set_group(&mut self, group: u8) {
        self.group = group;
        for flight in self.flights.iter_mut().skip(1) {
            flight.phase = 2;
            flight.sub = 0;
        }
        self.chosen = [[false; SLOTS]; 2];
        let single = matches!(self.shape, 0 | 5 | 7);
        for (order, cell) in group_cells(group).into_iter().enumerate() {
            if !single && !self.icons[cell] {
                continue;
            }
            let lock = FRAME_ENTITIES - 1 - order;
            self.chosen[self.side][CELL_SLOTS[self.side][cell]] = true;
            self.flights[lock].phase = 1;
            self.flights[lock].sub = 0;
            self.flights[lock].cell = Some(cell);
        }
    }

    /// Each cell's chance and hit points, for the targets (`0x08046784`).
    fn figures(&mut self) {
        let weapon = self.setup.weapons[self.slot];
        for (cell, &slot) in CELL_SLOTS[self.side].iter().enumerate() {
            if !self.chosen[self.side][slot] {
                self.chances[cell] = 0;
                continue;
            }
            self.chances[cell] = weapon.map_or(0, |weapon| weapon.chances[self.side][slot]);
            self.hp[cell] = self.setup.units[self.side][slot].map_or(0, |unit| unit.hp);
        }
    }

    /// Queues the grid's closing: the icons shrink back, `wait` frames a
    /// step, then it and they hide.
    fn queue_shut(&mut self, wait: u16) {
        self.ops.push_back(Op::Shut { frame: 0, wait });
    }

    /// A frame of the grid's opening: the grid grows and turns, then the
    /// icons grow; the cursors' behaviors run (`0x08000DF4` on 19 to 1).
    fn open_step(&mut self, entities: &mut Entities, frame: u16) {
        if frame < GRID_FRAMES {
            if let Some(grid) = entities.get_mut(GRID) {
                grid.set(VISIBLE, true);
                grid.affine = Some(grid_transform(frame + 1));
            }
        } else {
            let step = frame - GRID_FRAMES;
            if step == 0
                && let Some(grid) = entities.get_mut(GRID)
            {
                grid.affine = None;
            }
            for cell in 0..SLOTS {
                if let Some(icon) = entities.get_mut(FIRST_ICON + cell) {
                    icon.affine = Some(icon_transform(step));
                    if step == 0 {
                        icon.set(VISIBLE, self.icons[cell]);
                    }
                }
            }
        }
        self.behave(entities, OPENING_ENTITIES);
    }

    /// A frame of the grid's closing, `wait` frames a step from the icons'
    /// full size down; `true` once it is over and they and the grid hide.
    fn shut_step(&mut self, entities: &mut Entities, frame: u16, wait: u16) -> bool {
        let wait = wait.max(1);
        if frame >= OPENING_STEPS * wait {
            for index in (FIRST_ICON..FIRST_ICON + SLOTS).chain([GRID]) {
                if let Some(entity) = entities.get_mut(index) {
                    entity.set(VISIBLE, false);
                }
            }
            return true;
        }
        let step = OPENING_STEPS - 1 - frame / wait;
        for cell in 0..SLOTS {
            if let Some(icon) = entities.get_mut(FIRST_ICON + cell) {
                icon.affine = Some(icon_transform(step));
                if frame == 0 {
                    icon.set(VISIBLE, self.icons[cell]);
                }
            }
        }
        false
    }

    /// The Zoid fading for the weapon behind it (`0x080481D4(0, …, 0)`):
    /// twelve frames, from the whole Zoid to a quarter of it.
    fn fade(&mut self, entities: &mut Entities) {
        let step = self.pulse;
        self.blend = Some(Blend {
            zoid: true,
            alpha: (15 - step, step),
        });
        for index in [super::effects::FIRST_MOUNT, super::effects::FIRST_MOUNT + 1] {
            if let Some(mount) = entities.get_mut(index) {
                mount.semi_transparent = true;
            }
        }
        self.pulse += 1;
        if self.pulse > 11 {
            self.pulse = 0;
        }
    }

    /// The Zoid coming back (`0x080481D4(0, …, 1)`): twelve frames of the
    /// blend, then the registers' reset a frame later.
    fn fade_back(&mut self, entities: &mut Entities) {
        if self.pulse == 100 {
            self.blend = None;
            self.pulse = 0;
            return;
        }
        let step = self.pulse;
        self.blend = Some(Blend {
            zoid: self.blend.is_some_and(|blend| blend.zoid),
            alpha: (step + 4, 11u8.saturating_sub(step)),
        });
        self.pulse += 1;
        if self.pulse > 11 {
            for index in [super::effects::FIRST_MOUNT, super::effects::FIRST_MOUNT + 1] {
                if let Some(mount) = entities.get_mut(index) {
                    mount.semi_transparent = false;
                }
            }
            self.pulse = 100;
        }
    }

    /// The cursors' behaviors, from entity `last` down to the first.
    fn behave(&mut self, entities: &mut Entities, last: usize) {
        for index in (CURSOR..=last.min(FRAME_ENTITIES)).rev() {
            if index == CURSOR {
                self.fly_cursor(entities);
            } else {
                self.fly_lock(entities, index);
            }
        }
    }

    /// The weapon's cursor (`0x080492DC`): to the weapon, three pixels a
    /// frame; it tells the task once it is there.
    fn fly_cursor(&mut self, entities: &mut Entities) {
        let target = self.places[self.slot];
        let flight = &mut self.flights[0];
        let Some(cursor) = entities.get_mut(CURSOR) else {
            return;
        };
        match flight.phase {
            0 => {
                flight.busy = true;
                flight.aim((cursor.x, cursor.y), (target.0 << 16, target.1 << 16));
                flight.phase = 1;
            }
            1 => {
                let mut at = (cursor.x, cursor.y);
                if flight.advance(&mut at, CURSOR_STEPS) {
                    flight.phase = 2;
                }
                (cursor.x, cursor.y) = at;
            }
            2 => {
                self.arrived = true;
                flight.busy = false;
                flight.phase = 3;
            }
            _ => {}
        }
    }

    /// A locking cursor (`0x08049560`): behind the one before it, hidden
    /// on the weapon, or off to its cell's icon, where its figures count
    /// up.
    fn fly_lock(&mut self, entities: &mut Entities, index: usize) {
        let k = index - FIRST_LOCK;
        let (percent, digits) = (FIRST_PERCENT + k, FIRST_DIGITS + k);
        let mut flight = self.flights[index - CURSOR];
        match flight.phase {
            0 | 2 => {
                let trailing = flight.phase == 0;
                if flight.sub == 0 {
                    entities.animate(index, CURSOR_ANIMATION);
                    if let Some(lock) = entities.get_mut(index) {
                        lock.set(VISIBLE, trailing);
                    }
                    if trailing {
                        Self::restart_cursor(entities);
                    }
                    Self::hide_figures(entities, k);
                    if let Some(cell) = flight.cell {
                        self.set_digits(entities, k, cell, 0);
                    }
                    flight.sub = 1;
                }
                if flight.sub == 1 {
                    let leader = if trailing { index - 1 } else { CURSOR };
                    let at = entities.get(leader).map(|entity| (entity.x, entity.y));
                    if let (Some(at), Some(lock)) = (at, entities.get_mut(index)) {
                        (lock.x, lock.y) = at;
                    }
                    flight.busy = false;
                    flight.counted = None;
                }
            }
            1 => {
                let Some(cell) = flight.cell else {
                    return;
                };
                match flight.sub {
                    0 => {
                        flight.busy = true;
                        let icon = entities
                            .get(FIRST_ICON + cell)
                            .map_or((0, 0), |icon| (icon.x, icon.y - (LOCK_ABOVE << 16)));
                        if let Some(lock) = entities.get_mut(index) {
                            lock.set(VISIBLE, true);
                            flight.aim((lock.x, lock.y), icon);
                        }
                        flight.sub = 1;
                    }
                    1 => {
                        if let Some(lock) = entities.get_mut(index) {
                            let mut at = (lock.x, lock.y);
                            if flight.advance(&mut at, LOCK_STEPS) {
                                flight.sub = 2;
                            }
                            (lock.x, lock.y) = at;
                        }
                    }
                    2 => {
                        entities.animate(index, LOCK_ANIMATION);
                        let at = entities.get(index).map_or((0, 0), |lock| (lock.x, lock.y));
                        for (entity, (dx, dy)) in
                            [(percent, PERCENT_OFFSET), (digits, DIGITS_OFFSET)]
                        {
                            if let Some(figure) = entities.get_mut(entity) {
                                figure.set(VISIBLE, true);
                                (figure.x, figure.y) = (at.0 + (dx << 16), at.1 + (dy << 16));
                            }
                        }
                        self.set_digits(entities, k, cell, 0);
                        flight.busy = false;
                        if flight.counted == Some(cell) {
                            flight.sub = 4;
                        } else {
                            show_percent(entities, percent, 0);
                            flight.sub = 3;
                        }
                    }
                    3 => {
                        flight.counted = None;
                        let chance = usize::from(self.chances[cell]);
                        let shown = entities.get(percent).map_or(0, Entity::step) + PERCENT_COUNT;
                        let mut done = u8::from(chance <= shown);
                        show_percent(entities, percent, shown.min(chance));
                        let hp = self.hp[cell];
                        let count = (hp >> 4).max(1);
                        let counted = self.counted_hp[cell].wrapping_add(count);
                        self.set_digits(entities, k, cell, counted);
                        if hp <= self.counted_hp[cell] {
                            self.set_digits(entities, k, cell, hp);
                            done += 1;
                        }
                        if done == 2 {
                            flight.sub = 4;
                        }
                    }
                    4 => flight.counted = Some(cell),
                    _ => {}
                }
            }
            _ => {}
        }
        self.flights[index - CURSOR] = flight;
    }

    /// The weapon's cursor goes back to its animation's first step, which
    /// keeps the cursors in time.
    fn restart_cursor(entities: &mut Entities) {
        let sprite = entities.get(CURSOR).map(|cursor| cursor.sprite);
        if let Some(sprite) = sprite.and_then(|sprite| entities.sprites.get(sprite).cloned())
            && let Some(cursor) = entities.get_mut(CURSOR)
        {
            cursor.show_step(&sprite, 0);
        }
    }

    fn hide_figures(entities: &mut Entities, k: usize) {
        show_percent(entities, FIRST_PERCENT + k, 0);
        for index in [FIRST_PERCENT + k, FIRST_DIGITS + k] {
            if let Some(entity) = entities.get_mut(index) {
                entity.set(VISIBLE, false);
            }
        }
    }

    /// The hit points cursor `k` shows for cell `cell` (`0x08048F88`).
    fn set_digits(&mut self, entities: &mut Entities, k: usize, cell: usize, value: u16) {
        if let Some(counted) = self.counted_hp.get_mut(cell) {
            *counted = value;
        }
        // Each value is a frame of its own: the sprites the screen shows
        // were built the frame before, with the value they had then.
        if let Some(sprite) = entities.sprites.get_mut(self.digit_sprites[k]) {
            sprite.frames.push(digit_pieces(value));
            let frame = sprite.frames.len() - 1;
            sprite.animations = vec![vec![extraction::saga::AnimationStep { frame, duration: 1 }]];
        }
    }
}

/// Shows step `step` of the percent's animation: that many percent.
fn show_percent(entities: &mut Entities, index: usize, step: usize) {
    let sprite = entities
        .get(index)
        .and_then(|entity| entities.sprites.get(entity.sprite).cloned());
    if let Some(sprite) = sprite
        && let Some(entity) = entities.get_mut(index)
    {
        entity.show_step(&sprite, step);
    }
}

impl Flight {
    /// Sets its line from `from` to `target`, in 16.16 (`0x080492DC`): the
    /// longer axis leads, the error starting at the axes' difference.
    fn aim(&mut self, from: (i32, i32), target: (i32, i32)) {
        let axis = |from: i32, to: i32| {
            let unsigned = |value: i32| u32::from_ne_bytes(value.to_ne_bytes());
            let (from, to) = (unsigned(from), unsigned(to));
            if from < to {
                (to - from, 0x1_0000)
            } else {
                (from - to, -0x1_0000)
            }
        };
        let (dx, sx) = axis(from.0, target.0);
        let (dy, sy) = axis(from.1, target.1);
        self.target = target;
        self.step = (sx, sy);
        self.delta = (dx, dy);
        self.along_y = dy >= dx;
        self.error = if self.along_y { dy - dx } else { dx - dy };
    }

    /// Up to `steps` pixels along the line; `true` once it stands on its
    /// target.
    fn advance(&mut self, at: &mut (i32, i32), steps: usize) -> bool {
        /// The error has gone below 0, as the game's unsigned test reads it.
        const NEGATIVE: u32 = 0xFF_0000;
        let (dx, dy) = self.delta;
        for _ in 0..steps {
            if *at == self.target {
                return true;
            }
            if self.along_y {
                self.error = self.error.wrapping_sub(dx);
                if self.error > NEGATIVE {
                    at.0 = at.0.wrapping_add(self.step.0);
                    self.error = self.error.wrapping_add(dy);
                }
                at.1 = at.1.wrapping_add(self.step.1);
            } else {
                self.error = self.error.wrapping_sub(dy);
                if self.error > NEGATIVE {
                    at.1 = at.1.wrapping_add(self.step.1);
                    self.error = self.error.wrapping_add(dx);
                }
                at.0 = at.0.wrapping_add(self.step.0);
            }
        }
        false
    }
}

/// The grid's transform at step `step` of its opening (`0x080455FC`): its
/// ratio from 4.75 down to 1, a sixteenth of a turn more each step.
fn grid_transform(step: u16) -> (i16, i16, u16) {
    let ratio = GRID_RATIO_FROM - RATIO_STEP * i16::try_from(step).unwrap_or(0);
    (ratio, ratio, (step + 1) * ANGLE_STEP)
}

/// The icons' transform at step `step` (`0x080455B0`): mirrored, their
/// ratio from 5.375 down to 1.625.
fn icon_transform(step: u16) -> (i16, i16, u16) {
    let ratio = ICON_RATIO_FROM - RATIO_STEP * i16::try_from(step).unwrap_or(0);
    (-ratio, ratio, 0)
}

/// The four digits' pieces of the hit points `value` (`0x08048F88`):
/// blanks for the leading zeros, `????` from 10000.
fn digit_pieces(value: u16) -> Vec<EffectPiece> {
    let digits = if value >= UNKNOWN_HP {
        [UNKNOWN_DIGIT; 4]
    } else {
        let value = usize::from(value);
        let (thousands, hundreds, tens, ones) =
            (value / 1000, value / 100 % 10, value / 10 % 10, value % 10);
        let blank = |digit: usize, leading: bool| if leading { BLANK_DIGIT } else { digit };
        [
            blank(thousands, thousands == 0),
            blank(hundreds, thousands == 0 && hundreds == 0),
            blank(tens, thousands == 0 && hundreds == 0 && tens == 0),
            ones,
        ]
    };
    digits
        .iter()
        .zip(DIGIT_X)
        .map(|(&digit, x)| EffectPiece {
            tile: DIGIT_TILES[digit],
            attributes: 0,
            x,
            y: DIGIT_Y,
            width: 8,
            height: 8,
            scale_x: 0x100,
            scale_y: 0x100,
            affine: 0xFF,
        })
        .collect()
}

/// The keys held, as the GBA's bits.
fn held_keys(input: Input) -> u16 {
    [
        (Button::A, KEY_A),
        (Button::B, KEY_B),
        (Button::Right, KEY_RIGHT),
        (Button::Left, KEY_LEFT),
        (Button::Up, KEY_UP),
        (Button::Down, KEY_DOWN),
    ]
    .iter()
    .filter(|(button, _)| input.is_held(*button))
    .fold(0, |keys, (_, bit)| keys | bit)
}

/// The next slot from `from` with a weapon the scene knows (`0x08047DAC`),
/// round the six.
fn next_weapon(parts: &[u16; SLOTS], from: usize) -> usize {
    let mut slot = from % SLOTS;
    for _ in 0..=SLOTS * 2 {
        if parts[slot] != 0 {
            return slot;
        }
        slot = (slot + 1) % SLOTS;
    }
    slot
}

/// The previous one (`0x08047DD8`); `None` stands for before the first.
fn previous_weapon(parts: &[u16; SLOTS], from: Option<usize>) -> usize {
    let mut slot = from.unwrap_or(SLOTS - 1);
    for _ in 0..=SLOTS * 2 {
        if parts[slot] != 0 {
            return slot;
        }
        slot = slot.checked_sub(1).unwrap_or(SLOTS - 1);
    }
    slot
}

/// A weapon's shape on the grid from its reach's code and whether it is
/// one of the fixed weapons (`0x0804593C`): `0xFE` and `0xFF` for one
/// that reaches nothing from there.
#[must_use]
pub fn shape(code: u8, fixed: bool) -> u8 {
    let pick = |rack: u8, fixed_one: u8| if fixed { fixed_one } else { rack };
    match code {
        0 | 0xF | 0x10 => 0,
        2 | 0x11 => 2,
        5 => pick(5, 0xFF),
        6 => pick(0, 5),
        7 => pick(7, 5),
        8 => pick(7, 0),
        9 => pick(0xFE, 7),
        10 => pick(10, 0xFF),
        0xB => pick(2, 10),
        0xC => pick(0xC, 10),
        0xD => pick(0xC, 2),
        0xE => pick(0xFF, 0xC),
        other => other,
    }
}

/// The groups each shape first tries, in order (ROM `0x6D43BC`, six a
/// shape).
const ORDER: [[u8; 6]; 13] = [
    [2, 1, 0, 5, 4, 3],
    [8, 7, 6, 8, 7, 6],
    [9, 10, 9, 10, 9, 10],
    [12, 11, 12, 11, 12, 11],
    [13, 13, 13, 13, 13, 13],
    [2, 1, 0, 2, 1, 0],
    [2, 1, 0, 5, 4, 3],
    [5, 4, 3, 5, 4, 3],
    [2, 1, 0, 5, 4, 3],
    [5, 4, 3, 5, 4, 3],
    [9, 9, 9, 9, 9, 9],
    [9, 10, 9, 10, 9, 10],
    [10, 10, 10, 10, 10, 10],
];

/// The group a shape starts on (`0x080470D8`, `0x08047310`): the first of
/// its order with a unit; for the one-column shapes, `0xFF` or `0xFE` when
/// none has one.
fn first_group(shape: u8, icons: [bool; SLOTS]) -> u8 {
    let (tries, missing) = match shape {
        0 | 1 | 2 | 4 => (6, 6),
        3 => (3, 3),
        5 | 10 => (3, 0xFF),
        7 | 12 => (3, 0xFE),
        other => return other,
    };
    ORDER[usize::from(shape)][..tries]
        .iter()
        .copied()
        .find(|&group| valid(group, icons))
        .unwrap_or(missing)
}

/// The cells of group `group`, in the order its cursors take them: 0–5 one
/// cell, 6–8 a cell and the one behind it, 9 and 10 a column, 11 and 12 a
/// square, 13 all.
fn group_cells(group: u8) -> Vec<usize> {
    match group {
        0..=5 => vec![usize::from(group)],
        6..=8 => {
            let cell = usize::from(group - 6);
            vec![cell, cell + 3]
        }
        9 => vec![0, 1, 2],
        10 => vec![3, 4, 5],
        11 => vec![0, 1, 3, 4],
        12 => vec![1, 2, 4, 5],
        13 => (0..SLOTS).collect(),
        _ => Vec::new(),
    }
}

/// Whether group `group` has a unit (`0x08047184`).
fn valid(group: u8, icons: [bool; SLOTS]) -> bool {
    group_cells(group).iter().any(|&cell| icons[cell])
}

/// The group the keys move to from `group` (`0x08047754`): up and down
/// round a column, sideways to the other column's same row first.
fn navigate(shape: u8, group: u8, keys: u16, icons: [bool; SLOTS]) -> u8 {
    let first = |candidates: &[u8]| candidates.iter().copied().find(|&cell| valid(cell, icons));
    let round = |base: u8, at: u8| -> (Vec<u8>, Vec<u8>) {
        let row = at - base;
        let up = vec![base + (row + 1) % 3, base + (row + 2) % 3];
        let down = vec![base + (row + 2) % 3, base + (row + 1) % 3];
        (up, down)
    };
    let in_column = |base: u8, sideways: bool| -> Option<u8> {
        let (up, down) = round(base, group);
        if keys & KEY_UP != 0
            && let Some(found) = first(&up)
        {
            return Some(found);
        }
        if keys & KEY_DOWN != 0
            && let Some(found) = first(&down)
        {
            return Some(found);
        }
        if sideways && keys & KEYS_SIDEWAYS != 0 {
            let row = group % 3;
            let other = 3 - (group / 3) * 3;
            let mut rows = vec![row];
            rows.extend((0..3).rev().filter(|&r| r != row));
            let cells: Vec<u8> = rows.iter().map(|r| other + r).collect();
            if let Some(found) = first(&cells) {
                return Some(found);
            }
        }
        None
    };
    let moved = match (shape, group) {
        (0, 0..=5) => in_column(group / 3 * 3, true),
        (5, 0..=2) => in_column(0, false),
        (7, 3..=5) => in_column(3, false),
        (1, 6..=8) => in_column(6, false),
        (2, 9) if keys & KEYS_SIDEWAYS != 0 => Some(10),
        (2, 10) if keys & KEYS_SIDEWAYS != 0 => Some(9),
        (3, 11) if keys & KEYS_UPRIGHT != 0 => Some(12),
        (3, 12) if keys & KEYS_UPRIGHT != 0 => Some(11),
        (4, _) => Some(13),
        (10, _) => Some(9),
        (0xC, _) => Some(10),
        _ => None,
    };
    moved.unwrap_or(group)
}

/// The grid's animation for group `group` of shape `shape` (`0x080459F4`),
/// which lights its cells; 0 lights none.
fn grid_animation(shape: u8, group: u8) -> usize {
    let animation = match (shape, group) {
        (0, 0..=5) => group + 1,
        (1, 7) => 0xE,
        (1, 6) => 0xF,
        (1, 8) => 0xD,
        (2, 9) => 0x15,
        (2, 10) => 0x16,
        (3, 11) => 0x18,
        (3, 12) => 0x17,
        (4, _) => 0x14,
        (5, 0..=2) | (7, 3..=5) => group + 7,
        (10, _) => 0x10,
        (0xC, _) => 0x11,
        _ => 0,
    };
    usize::from(animation)
}

/// The keys the task reads, repeating when held (`0x0800175C`): pressed
/// this frame, or held past the delay, then every few frames.
#[derive(Debug, Clone, Copy, Default)]
struct Repeat {
    held: [u8; KEYS],
    counters: [u8; KEYS],
}

impl Repeat {
    /// The keys that count this frame, from the bits held.
    fn update(&mut self, held: u16) -> u16 {
        let mut out = 0;
        for key in 0..KEYS {
            let down = held & (1 << key) != 0;
            let history = self.held[key];
            self.held[key] = (history << 1) | u8::from(down);
            if !down {
                continue;
            }
            if history & 1 == 0 {
                self.counters[key] = REPEAT_DELAY;
                out |= 1 << key;
            } else {
                self.counters[key] = self.counters[key].wrapping_sub(1);
                if self.counters[key] == 0xFF {
                    self.counters[key] = REPEAT_EVERY;
                    out |= 1 << key;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_come_from_the_reach_and_the_rack() {
        assert_eq!(shape(0, false), 0);
        assert_eq!(shape(6, true), 5);
        assert_eq!(shape(9, false), 0xFE);
        assert_eq!(shape(0xB, false), 2);
        assert_eq!(shape(5, true), 0xFF);
    }

    #[test]
    fn a_shape_starts_on_its_first_group_with_a_unit() {
        let icons = [false, true, false, false, false, true];
        assert_eq!(first_group(0, icons), 1);
        assert_eq!(first_group(1, icons), 8);
        assert_eq!(
            first_group(7, [true, false, false, false, false, false]),
            0xFE
        );
        assert_eq!(first_group(2, icons), 9);
    }

    #[test]
    fn the_arrows_go_round_a_column_and_across() {
        let icons = [true, true, false, true, false, true];
        assert_eq!(navigate(0, 0, KEY_UP, icons), 1);
        assert_eq!(navigate(0, 1, KEY_UP, icons), 0);
        assert_eq!(navigate(0, 1, KEY_DOWN, icons), 0);
        assert_eq!(navigate(0, 1, KEY_LEFT, icons), 5);
        assert_eq!(navigate(0, 3, KEY_RIGHT, icons), 0);
        assert_eq!(navigate(2, 9, KEY_RIGHT, icons), 10);
        assert_eq!(group_cells(7), vec![1, 4]);
        assert_eq!(grid_animation(0, 1), 2);
        assert_eq!(grid_animation(1, 7), 0xE);
    }

    #[test]
    fn keys_repeat_after_a_delay() {
        let mut repeat = Repeat::default();
        assert_eq!(repeat.update(KEY_UP), KEY_UP);
        let fired: Vec<usize> = (1..40).filter(|_| repeat.update(KEY_UP) != 0).collect();
        assert_eq!(fired, vec![17, 24, 31, 38]);
    }

    #[test]
    fn a_cursor_flies_its_line_three_pixels_a_frame() {
        let mut flight = Flight::default();
        let mut at = (176 << 16, 46 << 16);
        flight.aim(at, (166 << 16, 43 << 16));
        assert!(!flight.advance(&mut at, CURSOR_STEPS));
        assert_eq!((at.0 >> 16, at.1 >> 16), (173, 45));
        for _ in 0..3 {
            flight.advance(&mut at, CURSOR_STEPS);
        }
        assert_eq!((at.0 >> 16, at.1 >> 16), (166, 43));
        assert!(flight.advance(&mut at, CURSOR_STEPS));
    }

    #[test]
    fn hit_points_show_without_leading_zeros() {
        let tiles = |value: u16| {
            digit_pieces(value)
                .iter()
                .map(|piece| piece.tile)
                .collect::<Vec<_>>()
        };
        assert_eq!(tiles(0), vec![4, 4, 4, 3]);
        assert_eq!(tiles(305), vec![4, 0, 3, 10]);
        assert_eq!(tiles(UNKNOWN_HP), vec![5; 4]);
    }
}
