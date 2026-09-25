//! Cutscenes and map events: the game's cooperative tasks, rewritten as
//! programs of [`Op`]s.
//!
//! The original runs its map events as tasks of a small cooperative kernel
//! (spawned with `0x08003D78`, ended with `0x08003DD8`, yielding for a
//! number of frames with `0x0805EF90`); task 3 is the map's event and
//! tasks 4–7 its helpers (fades, shakes). Each frame the actors update
//! first, then every task runs in slot order until it waits, a task
//! spawned into a later slot running in the same frame. A dialogue or a
//! blocking fade holds the whole game: nothing else moves until it ends,
//! and the task that started it goes on in the frame it ends. Handlers the
//! game calls directly (what an object runs when spoken to, what a map
//! runs when it loads) run at once, until they wait or end.
//!
//! Source of knowledge: own reading of the event code of Zoids Saga (Japan,
//! Rev 1) and per-frame traces of the entity table in a reference emulator
//! (see `docs/events.md`).

use crate::field::{Actor, Command, Direction, Field, Walk};
use crate::menu::Shop;

/// Task slots, as in the game's kernel.
pub const TASKS: usize = 8;
/// The slot of a map's own event.
pub const MAP_TASK: usize = 3;
const IMMEDIATE: usize = TASKS;
/// The darkest brightness level; levels above 16 all show black.
pub const BLACK: u8 = 31;
const FADE_IN_DELAY: u8 = 1;
/// Frames the scene loader holds the game besides one per object: the
/// decompressions it waits a frame after, and the frame it returns on.
const LOAD_FRAMES: u32 = 6;

/// One step of an event program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Yields for this many frames (`0x0805EF90`).
    Wait(u32),
    /// Holds the whole game for this many frames, the field included, as
    /// code the field's own task calls does.
    Freeze(u32),
    /// Runs a string of the `dialogue` table, holding the game until it
    /// ends (`0x08008B58`).
    Dialogue(u16),
    /// Starts actor `actor` walking to `to` (entity command 10, or 11
    /// through everything); does not wait for it to arrive.
    Walk {
        /// Actor index (object index; the game's entity index minus one).
        actor: usize,
        /// Destination metatile.
        to: (usize, usize),
        /// Pixels a frame, 16.16 fixed point.
        speed: i32,
        /// Animation tick shift while walking.
        shift: i8,
        /// Whether it walks through walls and actors.
        through: bool,
    },
    /// Waits until actor `actor` has arrived (its command is idle again).
    AwaitArrival(usize),
    /// Turns actor `actor` (`0x08000BD8`).
    Face(usize, Direction),
    /// Gives the player the buttons back (command 0) or takes them (1).
    Control(bool),
    /// Hides actor `actor` (its flags cleared).
    Hide(usize),
    /// Moves actor `actor`'s sprite by `(dx, dy)` pixels.
    Nudge(usize, (i32, i32)),
    /// Stands actor `actor` on a metatile (`0x08008B70`).
    Place(usize, (usize, usize)),
    /// Sets or clears a game flag.
    Flag(u16, bool),
    /// Runs `then` when every flag of `all` is set and none of `none` is,
    /// `otherwise` else.
    IfFlags {
        /// Flags that must be set.
        all: &'static [u16],
        /// Flags that must be clear.
        none: &'static [u16],
        /// Program run when they are.
        then: &'static [Op],
        /// Program run when they are not.
        otherwise: &'static [Op],
    },
    /// Starts `program` in task slot `slot`, replacing what runs there.
    Spawn(usize, &'static [Op]),
    /// Ends the task running this program.
    End,
    /// Plays a song unless it is already playing (`0x080019B4`).
    Music(u16),
    /// Plays a sound effect (`0x080019EC`).
    Sound(u16),
    /// Sets the brightness level and restarts the fade counter
    /// (`0x08001624` sets 0, `0x0800159C` sets 31).
    Brightness(u8),
    /// Marks the characters of group `group` as met, for the character
    /// guide (`0x08037858`).
    Meet(u8),
    /// Steps the brightness toward black every `mask + 1` frames until it
    /// is black (the loop of the task at `0x0800C6A4`).
    FadeOut(u16),
    /// Steps the brightness back every `mask + 1` frames until it is
    /// normal (the task at `0x0800C680`).
    FadeIn(u16),
    /// Brightens the screen to normal one level a frame, holding the game
    /// (what follows loading a map).
    FadeInHolding,
    /// Runs `program` and comes back.
    Call(&'static [Op]),
    /// Runs `program` this many times.
    Repeat(u16, &'static [Op]),
    /// A battle the game stages for a cutscene (`0x08008E4C`: battle scene
    /// `n` of the table at ROM `0x66429C`, run by the battle module),
    /// holding the game until it and the map's reload after it end.
    Battle(u8),
    /// Opens a shop (`0x08008F58`), holding the game until it closes and
    /// the map is loaded again behind it.
    Shop(Shop),
    /// Fights the roaming enemy the player met (`0x0800B9CC`), holding the
    /// game until the battle hands back.
    Combat,
    /// What the battle's outcome does to the player and the enemy met,
    /// once the field is bright again (`0x0800B9CC`).
    AfterCombat,
    /// Scrolls the camera by `(dx, dy)` 16.16 fixed-point pixels
    /// (`0x08008324`, called once a frame for a pan).
    Pan(i32, i32),
    /// Runs string `index` of script table `table`, holding the game until
    /// it ends (`0x0803E51C` on another table than the dialogue one).
    Script(&'static str, u16),
    /// Marks deck command `n` as learned (`0x080370C0`).
    LearnCommand(u8),
    /// Runs `then` when deck command `command` has been learned
    /// (`0x080370DC`), `otherwise` else.
    IfCommand {
        /// The deck command.
        command: u8,
        /// Program run when it has.
        then: &'static [Op],
        /// Program run when it has not.
        otherwise: &'static [Op],
    },
    /// Runs `then` when the current map's area, the low byte of its record
    /// id (RAM `0x0200000C`), is `area`, `otherwise` else.
    IfArea {
        /// The area.
        area: u8,
        /// Program run in it.
        then: &'static [Op],
        /// Program run elsewhere.
        otherwise: &'static [Op],
    },
    /// Darkens the screen a level a frame to black, holding the game
    /// (`0x08001524`).
    FadeOutHolding,
    /// Darkens the screen as [`Op::FadeOutHolding`] does after `n` frames
    /// at the level it has: a handler the talk dispatch calls (a frame
    /// after the talk) whose `0x08001524` sets level 0 on its first frame.
    FadeOutHoldingAfter(u8),
    /// Shows actor `actor` again.
    Show(usize),
    /// Puts actor `actor`'s sprite at map pixel `(x, y)`.
    Position(usize, (i32, i32)),
    /// Plays animation `anim` of actor `actor` once.
    PlayOnce(usize, usize),
    /// Waits until actor `actor`'s animation played once has ended.
    AwaitAnimation(usize),
    /// Shows step `step` of actor `actor`'s animation without animating.
    Pose(usize, usize),
    /// Lets actor `actor`'s animation loop again from the start of `anim`.
    Animate(usize, usize),
    /// Sets actor `actor`'s animation tick shift.
    Shift(usize, i8),
    /// Waits until one of the flags is set.
    AwaitAnyFlag(&'static [u16]),
    /// Waits until the player's cell is on column `column` (when given)
    /// and between rows `rows.0` and `rows.1` inclusive (when given).
    AwaitPlayer {
        /// Column the player must reach.
        column: Option<usize>,
        /// First and last row that count.
        rows: Option<(usize, usize)>,
    },
    /// Marks Zoid picture `id` as seen, entering it in the Zoid guide
    /// (`0x08037098`).
    SeeZoid(u8),
    /// Opens the chest being searched: its opening animation and sound
    /// (`0x0800B938`).
    OpenChest,
    /// Marks the chest being searched as opened.
    MarkChest,
    /// Runs `then` when the chest holds money, `otherwise` else.
    IfChestMoney {
        /// Program run for money.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Adds the chest's money and prints the amount into window 1
    /// (`0x08037100`, `0x08001848`).
    TakeChestMoney,
    /// Runs `then` when the last menu answered its first line (the saved
    /// var1 is 0 and var0 is not), `otherwise` else.
    IfChoice {
        /// Program run on the first line.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Takes the player to metatile `cell` of map `map` with its own objects,
    /// facing `facing` when given (`0x08007188`), running the map's handler.
    Warp {
        /// Map record.
        map: usize,
        /// Arrival metatile.
        cell: (usize, usize),
        /// Facing on arrival.
        facing: Option<Direction>,
    },
    /// Forms the party around the Zoid picked in the hangar
    /// (`0x08037644`: 0 the Shield Liger, 1 the Saber Tiger, 2 the Raynos).
    FormParty(u8),
    /// Loads map `map` with `count` objects from the list at ROM address
    /// `objects` and the player on `player` (`0x080079E8`); the game holds
    /// while it decompresses and places one object a frame.
    LoadMap {
        /// Map record.
        map: usize,
        /// The player's metatile.
        player: (usize, usize),
        /// ROM address of the object list.
        objects: u32,
        /// Objects in the list.
        count: usize,
    },
}

/// What an event asks of the game it runs in.
pub trait EventHost {
    /// The field, when one is loaded.
    fn field(&mut self) -> Option<&mut Field>;
    /// Whether game flag `flag` is set.
    fn flag(&self, flag: u16) -> bool;
    /// Sets or clears game flag `flag`.
    fn set_flag(&mut self, flag: u16, set: bool);
    /// Starts dialogue string `index`; the game holds until it ends.
    fn start_dialogue(&mut self, index: u16);
    /// Starts battle scene `scene`; the game holds until it ends.
    fn start_battle(&mut self, scene: u8);
    /// Opens `shop`; the game holds until it closes.
    fn start_shop(&mut self, shop: Shop);
    /// Starts the battle against the enemy the player met; the game holds
    /// until it hands back.
    fn start_combat(&mut self);
    /// Gives the player and the enemy met the battle's outcome: the beaten
    /// one is wrecked, the enemy retreated from stands still.
    fn after_combat(&mut self);
    /// Plays song `song` unless it is playing.
    fn play_music(&mut self, song: u16);
    /// Plays sound effect `sound`.
    fn play_sound(&mut self, sound: u16);
    /// Loads a map for a cutscene.
    fn load_map(&mut self, map: usize, player: (usize, usize), objects: u32, count: usize);
    /// Marks the characters of group `group` as met.
    fn meet(&mut self, group: u8);
    /// Starts string `index` of script table `table`; the game holds until
    /// it ends.
    fn start_script(&mut self, table: &'static str, index: u16);
    /// Marks deck command `command` as learned.
    fn learn_command(&mut self, command: u8);
    /// Whether deck command `command` has been learned.
    fn command_learned(&self, command: u8) -> bool;
    /// The area of the map being walked.
    fn area(&self) -> u8;
    /// The variables the last script saved.
    fn saved_vars(&self) -> [u16; 8];
    /// Forms the party around the Zoid picked in the hangar.
    fn form_party(&mut self, choice: u8);
    /// Marks Zoid picture `id` as seen.
    fn see_zoid(&mut self, id: u8);
    /// Plays the searched chest's opening animation and sound.
    fn open_chest(&mut self);
    /// Marks the searched chest as opened.
    fn mark_chest(&mut self);
    /// The money in the searched chest.
    fn chest_money(&self) -> u32;
    /// Adds the searched chest's money and prints it into window 1.
    fn take_chest_money(&mut self);
    /// Takes the player to `cell` of `map` with its own objects and runs the
    /// map's handler; returns how many objects the map places.
    fn warp(&mut self, map: usize, cell: (usize, usize), facing: Option<Direction>) -> usize;
}

/// A program running in a task, where it is and how many more times it
/// runs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Frame {
    program: &'static [Op],
    pc: usize,
    repeat: u16,
}

impl Frame {
    const fn new(program: &'static [Op], repeat: u16) -> Self {
        Self {
            program,
            pc: 0,
            repeat,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Task {
    frames: Vec<Frame>,
    wait: u32,
    fade_counter: u16,
}

impl Task {
    fn new(program: &'static [Op]) -> Self {
        Self {
            frames: vec![Frame::new(program, 1)],
            wait: 0,
            fade_counter: 0,
        }
    }
}

/// What holds the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hold {
    /// A dialogue started by the task in this slot.
    Dialogue(usize),
    /// A battle scene started by the task in this slot.
    Battle(usize),
    /// A shop opened by the task in this slot.
    Shop(usize),
    /// A battle started by the task in this slot.
    Combat(usize),
    /// A fade to black holding the game after `delay` frames of waiting;
    /// once black, the task in `resume` goes on, or else the caller is told.
    FadeOut {
        /// Frames left before the first level.
        delay: u8,
        /// The task that started it.
        resume: Option<usize>,
    },
    /// Loading a map: the game holds this many more frames, then the task in
    /// this slot goes on.
    Loading {
        /// Frames left.
        frames: u32,
        /// The task that loads.
        slot: usize,
    },
    /// A fade in holding the game after `delay` frames of waiting, and
    /// `settle` more once normal.
    FadeIn {
        /// Frames left before the first level.
        delay: u8,
        /// Frames held once the brightness is normal.
        settle: u8,
    },
}

/// What a frame of a hold did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldStep {
    /// Nothing holds the game.
    Free,
    /// The game stays held for the rest of the frame.
    Held,
    /// A fade in has just ended: the rest of the frame runs.
    Released,
    /// A fade to black has just reached black: the caller loads what
    /// comes next and starts a fade in.
    Darkened,
}

/// What running an op leaves the task to do.
enum Flow {
    /// Go on with the next op.
    Next,
    /// Go on without moving past the op: it moved the program itself.
    Continue,
    /// Stop until a later frame; the op moved the program as it needed.
    Yield,
    /// End the task.
    Stop,
}

/// The running events and the screen brightness they control.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Events {
    tasks: [Option<Task>; TASKS + 1],
    hold: Option<Hold>,
    brightness: u8,
}

impl Events {
    /// No events, normal brightness.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The brightness level: 0 normal, 16 and above black.
    #[must_use]
    pub fn brightness(&self) -> u8 {
        self.brightness
    }

    /// Sets the brightness level.
    pub fn set_brightness(&mut self, level: u8) {
        self.brightness = level.min(BLACK);
    }

    /// Whether a dialogue or a fade holds the game.
    #[must_use]
    pub fn holding(&self) -> bool {
        self.hold.is_some()
    }

    /// Whether a dialogue an event started is running.
    #[must_use]
    pub fn in_dialogue(&self) -> bool {
        matches!(self.hold, Some(Hold::Dialogue(_)))
    }

    /// Whether the running dialogue was started by a task: the original
    /// runs it inside the task, after the actors' update, so they go on
    /// moving. A handler the game calls directly runs its dialogue inside
    /// that call, and nothing moves.
    #[must_use]
    pub fn dialogue_in_task(&self) -> bool {
        matches!(self.hold, Some(Hold::Dialogue(slot)) if slot < IMMEDIATE)
    }

    /// Whether a battle scene an event started is running.
    #[must_use]
    pub fn in_battle(&self) -> bool {
        matches!(self.hold, Some(Hold::Battle(_)))
    }

    /// Whether a shop an event opened is open.
    #[must_use]
    pub fn in_shop(&self) -> bool {
        matches!(self.hold, Some(Hold::Shop(_)))
    }

    /// Whether a battle an event started is running.
    #[must_use]
    pub fn in_combat(&self) -> bool {
        matches!(self.hold, Some(Hold::Combat(_)))
    }

    /// Whether any task runs.
    #[must_use]
    pub fn running(&self) -> bool {
        self.tasks.iter().any(Option::is_some) || self.hold.is_some()
    }

    /// Whether task `slot` runs.
    #[must_use]
    pub fn task_running(&self, slot: usize) -> bool {
        self.tasks.get(slot).is_some_and(Option::is_some)
    }

    /// Starts `program` in `slot`, replacing what runs there.
    pub fn spawn(&mut self, slot: usize, program: &'static [Op]) {
        if let Some(task) = self.tasks.get_mut(slot) {
            *task = Some(Task::new(program));
        }
    }

    /// Ends the task in `slot` (`0x08003DD8`), as the loader does with a
    /// map's event when the player walks into another map.
    pub fn end(&mut self, slot: usize) {
        if let Some(task) = self.tasks.get_mut(slot) {
            *task = None;
        }
    }

    /// Runs a handler the game calls directly, until it waits or ends.
    pub fn run_now(&mut self, program: &'static [Op], host: &mut impl EventHost) {
        self.tasks[IMMEDIATE] = Some(Task::new(program));
        self.run_task(IMMEDIATE, host);
    }

    /// Holds the game while the screen brightens one level a frame, after
    /// a frame at the level it has; the frame it reaches normal runs (what
    /// follows a map loaded from black).
    pub fn fade_in_holding(&mut self) {
        self.fade_in_after(FADE_IN_DELAY, 0);
    }

    /// Holds the game while the screen brightens one level a frame, after
    /// `delay` frames at the level it has and `settle` frames once normal.
    pub fn fade_in_after(&mut self, delay: u8, settle: u8) {
        self.hold = Some(Hold::FadeIn { delay, settle });
    }

    /// Holds the game while the screen darkens one level a frame to black,
    /// after `delay` frames.
    pub fn fade_out_holding(&mut self, delay: u8) {
        self.hold = Some(Hold::FadeOut {
            delay,
            resume: None,
        });
    }

    /// Advances a hold by one frame: a finished dialogue or battle scene
    /// (`dialogue_done`) lets the task that started it go on, then the
    /// tasks after it run; a fade moves a level.
    pub fn update_hold(&mut self, dialogue_done: bool, host: &mut impl EventHost) -> HoldStep {
        match self.hold {
            None => HoldStep::Free,
            Some(
                Hold::Dialogue(slot) | Hold::Battle(slot) | Hold::Shop(slot) | Hold::Combat(slot),
            ) => {
                if dialogue_done {
                    self.hold = None;
                    self.run_task(slot, host);
                    self.run_slots(slot + 1, host);
                }
                HoldStep::Held
            }
            Some(Hold::Loading { frames, slot }) => {
                if frames > 1 {
                    self.hold = Some(Hold::Loading {
                        frames: frames - 1,
                        slot,
                    });
                    return HoldStep::Held;
                }
                self.hold = None;
                self.run_task(slot, host);
                self.run_slots(slot + 1, host);
                HoldStep::Held
            }
            Some(Hold::FadeOut { delay, resume }) if delay > 0 => {
                self.hold = Some(Hold::FadeOut {
                    delay: delay - 1,
                    resume,
                });
                HoldStep::Held
            }
            Some(Hold::FadeOut { resume, .. }) => {
                self.brightness = (self.brightness + 1).min(BLACK);
                if self.brightness < BLACK {
                    return HoldStep::Held;
                }
                self.hold = None;
                let Some(slot) = resume else {
                    return HoldStep::Darkened;
                };
                self.run_task(slot, host);
                self.run_slots(slot + 1, host);
                HoldStep::Held
            }
            Some(Hold::FadeIn { delay, settle }) if delay > 0 => {
                self.hold = Some(Hold::FadeIn {
                    delay: delay - 1,
                    settle,
                });
                HoldStep::Held
            }
            Some(Hold::FadeIn { settle, .. }) if self.brightness == 0 => {
                if settle > 0 {
                    self.hold = Some(Hold::FadeIn {
                        delay: 0,
                        settle: settle - 1,
                    });
                    return HoldStep::Held;
                }
                self.hold = None;
                HoldStep::Released
            }
            Some(Hold::FadeIn { settle, .. }) => {
                self.brightness -= 1;
                if self.brightness > 0 {
                    return HoldStep::Held;
                }
                if settle > 0 {
                    self.hold = Some(Hold::FadeIn {
                        delay: 0,
                        settle: settle - 1,
                    });
                    return HoldStep::Held;
                }
                self.hold = None;
                HoldStep::Released
            }
        }
    }

    /// Runs every task for one frame, in slot order.
    pub fn update(&mut self, host: &mut impl EventHost) {
        self.run_slots(0, host);
    }

    fn run_slots(&mut self, from: usize, host: &mut impl EventHost) {
        for slot in from..=TASKS {
            if self.hold.is_some() {
                return;
            }
            if let Some(task) = self.tasks[slot].as_mut() {
                if task.wait > 1 {
                    task.wait -= 1;
                    continue;
                }
                task.wait = 0;
                self.run_task(slot, host);
            }
        }
    }

    fn run_task(&mut self, slot: usize, host: &mut impl EventHost) {
        loop {
            let Some(task) = self.tasks[slot].as_mut() else {
                return;
            };
            let Some(frame) = task.frames.last_mut() else {
                self.tasks[slot] = None;
                return;
            };
            let Some(&op) = frame.program.get(frame.pc) else {
                if frame.repeat > 1 {
                    frame.repeat -= 1;
                    frame.pc = 0;
                } else {
                    task.frames.pop();
                }
                continue;
            };
            match self.execute(slot, op, host) {
                Flow::Next => self.advance(slot),
                Flow::Continue => {}
                Flow::Yield => return,
                Flow::Stop => {
                    self.tasks[slot] = None;
                    return;
                }
            }
        }
    }

    fn advance(&mut self, slot: usize) {
        if let Some(task) = self.tasks[slot].as_mut()
            && let Some(frame) = task.frames.last_mut()
        {
            frame.pc += 1;
        }
    }

    fn push(&mut self, slot: usize, program: &'static [Op], repeat: u16) {
        self.advance(slot);
        if let Some(task) = self.tasks[slot].as_mut() {
            task.frames.push(Frame::new(program, repeat));
        }
    }

    fn execute(&mut self, slot: usize, op: Op, host: &mut impl EventHost) -> Flow {
        match op {
            Op::Wait(_)
            | Op::Dialogue(_)
            | Op::Battle(_)
            | Op::Shop(_)
            | Op::Combat
            | Op::Freeze(_)
            | Op::Script(..)
            | Op::AwaitArrival(_)
            | Op::AwaitAnimation(_)
            | Op::AwaitAnyFlag(_)
            | Op::AwaitPlayer { .. }
            | Op::FadeInHolding
            | Op::FadeOutHolding
            | Op::FadeOutHoldingAfter(_)
            | Op::LoadMap { .. }
            | Op::Warp { .. } => self.wait(slot, op, host),
            Op::IfFlags { .. }
            | Op::IfCommand { .. }
            | Op::IfArea { .. }
            | Op::IfChoice { .. }
            | Op::IfChestMoney { .. }
            | Op::Call(_)
            | Op::Repeat(..)
            | Op::Spawn(..)
            | Op::End => self.branch(slot, op, host),
            Op::Brightness(level) => {
                self.brightness = level.min(BLACK);
                if let Some(task) = self.tasks[slot].as_mut() {
                    task.fade_counter = 0;
                }
                Flow::Next
            }
            Op::FadeOut(mask) => self.fade_step(slot, mask, true),
            Op::FadeIn(mask) => self.fade_step(slot, mask, false),
            Op::Walk { .. }
            | Op::Face(..)
            | Op::Control(_)
            | Op::Hide(_)
            | Op::Show(_)
            | Op::Nudge(..)
            | Op::Place(..)
            | Op::Position(..)
            | Op::PlayOnce(..)
            | Op::Pose(..)
            | Op::Animate(..)
            | Op::Shift(..)
            | Op::Pan(..) => {
                if let Some(field) = host.field() {
                    command_actor(field, op);
                }
                Flow::Next
            }
            _ => {
                apply(op, host);
                Flow::Next
            }
        }
    }

    /// Ops that make the task wait: frames, a dialogue, an actor, a flag,
    /// the player's position, a fade or a load.
    fn wait(&mut self, slot: usize, op: Op, host: &mut impl EventHost) -> Flow {
        let ready = match op {
            Op::Wait(frames) => {
                self.advance(slot);
                if frames == 0 {
                    return Flow::Continue;
                }
                if let Some(task) = self.tasks[slot].as_mut() {
                    task.wait = frames;
                }
                return Flow::Yield;
            }
            Op::Dialogue(_) | Op::Script(..) | Op::Battle(_) | Op::Shop(_) | Op::Combat => {
                self.advance(slot);
                self.hold = Some(start_hold(slot, op, host));
                return Flow::Yield;
            }
            Op::Freeze(frames) => {
                self.advance(slot);
                if frames == 0 {
                    return Flow::Continue;
                }
                self.hold = Some(Hold::Loading { frames, slot });
                return Flow::Yield;
            }
            Op::FadeInHolding => {
                self.advance(slot);
                self.fade_in_holding();
                return Flow::Yield;
            }
            Op::FadeOutHolding => {
                self.advance(slot);
                self.hold = Some(Hold::FadeOut {
                    delay: 0,
                    resume: Some(slot),
                });
                return Flow::Yield;
            }
            Op::FadeOutHoldingAfter(delay) => {
                self.advance(slot);
                self.hold = Some(Hold::FadeOut {
                    delay,
                    resume: Some(slot),
                });
                return Flow::Yield;
            }
            Op::LoadMap {
                map,
                player,
                objects,
                count,
            } => {
                host.load_map(map, player, objects, count);
                return self.hold_loading(slot, count);
            }
            Op::Warp { map, cell, facing } => {
                let count = host.warp(map, cell, facing);
                return self.hold_loading(slot, count);
            }
            Op::AwaitArrival(actor) => host
                .field()
                .and_then(|field| field.actor(actor).map(Actor::idle))
                .unwrap_or(true),
            Op::AwaitAnimation(actor) => host
                .field()
                .and_then(|field| field.actor(actor).map(Actor::animation_done))
                .unwrap_or(true),
            Op::AwaitAnyFlag(flags) => flags.iter().any(|&flag| host.flag(flag)),
            Op::AwaitPlayer { column, rows } => host.field().is_some_and(|field| {
                let player = field.player();
                column.is_none_or(|column| player.column == column)
                    && rows.is_none_or(|(first, last)| (first..=last).contains(&player.row))
            }),
            _ => true,
        };
        if ready {
            return Flow::Next;
        }
        if let Some(task) = self.tasks[slot].as_mut() {
            task.wait = 1;
        }
        Flow::Yield
    }

    /// Holds the game while a map loads. A handler the game calls directly
    /// loads within the entry that called it: a map's handler run by a
    /// warp delays the warp's fade in by the load instead.
    fn hold_loading(&mut self, slot: usize, count: usize) -> Flow {
        let frames = LOAD_FRAMES + u32::try_from(count).unwrap_or(u32::MAX);
        if slot == IMMEDIATE {
            if let Some(Hold::FadeIn { delay, settle }) = self.hold {
                let delay = delay.saturating_add(u8::try_from(frames).unwrap_or(u8::MAX));
                self.hold = Some(Hold::FadeIn { delay, settle });
            }
            return Flow::Next;
        }
        self.advance(slot);
        self.hold = Some(Hold::Loading { frames, slot });
        Flow::Yield
    }

    /// Ops that decide where the task goes next.
    fn branch(&mut self, slot: usize, op: Op, host: &mut impl EventHost) -> Flow {
        let taken = |yes: bool, then, otherwise| if yes { then } else { otherwise };
        let program = match op {
            Op::IfFlags {
                all,
                none,
                then,
                otherwise,
            } => taken(
                all.iter().all(|&flag| host.flag(flag))
                    && !none.iter().any(|&flag| host.flag(flag)),
                then,
                otherwise,
            ),
            Op::IfCommand {
                command,
                then,
                otherwise,
            } => taken(host.command_learned(command), then, otherwise),
            Op::IfArea {
                area,
                then,
                otherwise,
            } => taken(host.area() == area, then, otherwise),
            Op::IfChoice { then, otherwise } => {
                let vars = host.saved_vars();
                taken(vars[1] == 0 && vars[0] != 0, then, otherwise)
            }
            Op::IfChestMoney { then, otherwise } => taken(host.chest_money() > 0, then, otherwise),
            Op::Call(program) => program,
            Op::Repeat(times, program) => {
                if times == 0 {
                    return Flow::Next;
                }
                self.push(slot, program, times);
                return Flow::Continue;
            }
            Op::Spawn(target, program) => {
                self.spawn(target, program);
                return Flow::Next;
            }
            _ => return Flow::Stop,
        };
        self.push(slot, program, 1);
        Flow::Continue
    }

    /// One call of the game's fade step (`0x08001650` toward black,
    /// `0x080015CC` back): the counter rises every frame and the level
    /// moves when `counter & mask` is 0; the task waits a frame between
    /// calls until the level is reached.
    fn fade_step(&mut self, slot: usize, mask: u16, darker: bool) -> Flow {
        let target = if darker { BLACK } else { 0 };
        let Some(task) = self.tasks[slot].as_mut() else {
            return Flow::Stop;
        };
        if self.brightness == target {
            task.fade_counter = 0;
            return Flow::Next;
        }
        task.fade_counter = task.fade_counter.wrapping_add(1);
        if task.fade_counter & mask == 0 {
            self.brightness = if darker {
                self.brightness + 1
            } else {
                self.brightness - 1
            };
        }
        task.wait = 1;
        Flow::Yield
    }
}

/// Applies an op that changes the game's state and returns at once.
fn apply(op: Op, host: &mut impl EventHost) {
    match op {
        Op::Flag(flag, set) => host.set_flag(flag, set),
        Op::Music(song) => host.play_music(song),
        Op::Sound(sound) => host.play_sound(sound),
        Op::Meet(group) => host.meet(group),
        Op::LearnCommand(command) => host.learn_command(command),
        Op::FormParty(choice) => host.form_party(choice),
        Op::SeeZoid(id) => host.see_zoid(id),
        Op::OpenChest => host.open_chest(),
        Op::MarkChest => host.mark_chest(),
        Op::TakeChestMoney => host.take_chest_money(),
        Op::AfterCombat => host.after_combat(),
        _ => {}
    }
}

/// Applies an op that commands an actor.
fn command_actor(field: &mut Field, op: Op) {
    match op {
        Op::Walk {
            actor,
            to,
            speed,
            shift,
            through,
        } => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.command = Command::WalkTo(Walk {
                    column: to.0,
                    row: to.1,
                    speed,
                    animation_shift: shift,
                    through,
                });
            }
        }
        Op::Face(actor, direction) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.face(direction);
            }
        }
        Op::Control(player) => {
            field.player_mut().command = if player {
                Command::Player
            } else {
                Command::Idle
            };
        }
        Op::Hide(actor) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.visible = false;
            }
        }
        Op::Nudge(actor, offset) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.nudge(offset);
            }
        }
        Op::Place(actor, cell) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.place(cell);
            }
        }
        Op::Show(actor) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.visible = true;
            }
        }
        Op::Position(actor, pixel) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.set_position(pixel);
            }
        }
        Op::PlayOnce(actor, animation) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.play_once(animation);
            }
        }
        Op::Pose(actor, step) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.pose = Some(step);
            }
        }
        Op::Animate(actor, animation) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.pose = None;
                actor.once = false;
                actor.play(animation);
            }
        }
        Op::Shift(actor, shift) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.animation_shift = shift;
            }
        }
        Op::Pan(dx, dy) => {
            field.pan.0 += dx;
            field.pan.1 += dy;
        }
        _ => {}
    }
}

/// Starts what `op` holds the game for, for the task in `slot`: a
/// dialogue or another table's script, a battle scene, a shop or a battle.
fn start_hold(slot: usize, op: Op, host: &mut impl EventHost) -> Hold {
    match op {
        Op::Script(table, index) => {
            host.start_script(table, index);
            Hold::Dialogue(slot)
        }
        Op::Battle(scene) => {
            host.start_battle(scene);
            Hold::Battle(slot)
        }
        Op::Shop(shop) => {
            host.start_shop(shop);
            Hold::Shop(slot)
        }
        Op::Combat => {
            host.start_combat();
            Hold::Combat(slot)
        }
        Op::Dialogue(index) => {
            host.start_dialogue(index);
            Hold::Dialogue(slot)
        }
        _ => Hold::Dialogue(slot),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Host {
        flags: Vec<u16>,
        log: Vec<String>,
    }

    impl EventHost for Host {
        fn field(&mut self) -> Option<&mut Field> {
            None
        }

        fn flag(&self, flag: u16) -> bool {
            self.flags.contains(&flag)
        }

        fn set_flag(&mut self, flag: u16, set: bool) {
            self.flags.retain(|&other| other != flag);
            if set {
                self.flags.push(flag);
            }
        }

        fn start_dialogue(&mut self, index: u16) {
            self.log.push(format!("dialogue {index}"));
        }

        fn start_battle(&mut self, scene: u8) {
            self.log.push(format!("battle {scene}"));
        }

        fn start_shop(&mut self, shop: Shop) {
            self.log.push(format!("shop {shop:?}"));
        }

        fn start_combat(&mut self) {
            self.log.push("combat".to_owned());
        }

        fn after_combat(&mut self) {
            self.log.push("after combat".to_owned());
        }

        fn play_music(&mut self, song: u16) {
            self.log.push(format!("music {song}"));
        }

        fn play_sound(&mut self, sound: u16) {
            self.log.push(format!("sound {sound}"));
        }

        fn load_map(&mut self, map: usize, _: (usize, usize), _: u32, _: usize) {
            self.log.push(format!("map {map}"));
        }

        fn meet(&mut self, group: u8) {
            self.log.push(format!("meet {group}"));
        }

        fn start_script(&mut self, table: &'static str, index: u16) {
            self.log.push(format!("script {table} {index}"));
        }

        fn learn_command(&mut self, command: u8) {
            self.log.push(format!("command {command}"));
        }

        fn command_learned(&self, command: u8) -> bool {
            self.log.contains(&format!("command {command}"))
        }

        fn area(&self) -> u8 {
            1
        }

        fn saved_vars(&self) -> [u16; 8] {
            [0; 8]
        }

        fn form_party(&mut self, choice: u8) {
            self.log.push(format!("party {choice}"));
        }

        fn see_zoid(&mut self, id: u8) {
            self.log.push(format!("zoid {id}"));
        }

        fn open_chest(&mut self) {
            self.log.push("open".to_owned());
        }

        fn mark_chest(&mut self) {
            self.log.push("mark".to_owned());
        }

        fn chest_money(&self) -> u32 {
            0
        }

        fn take_chest_money(&mut self) {
            self.log.push("money".to_owned());
        }

        fn warp(&mut self, map: usize, _: (usize, usize), _: Option<Direction>) -> usize {
            self.log.push(format!("warp {map}"));
            1
        }
    }

    const WAITING: &[Op] = &[Op::Music(1), Op::Wait(3), Op::Music(2), Op::End];

    #[test]
    fn waits_resume_that_many_frames_later() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.spawn(MAP_TASK, WAITING);
        events.update(&mut host);
        assert_eq!(host.log, ["music 1"]);
        events.update(&mut host);
        events.update(&mut host);
        assert_eq!(host.log, ["music 1"]);
        events.update(&mut host);
        assert_eq!(host.log, ["music 1", "music 2"]);
        assert!(!events.running());
    }

    const HELPER: &[Op] = &[Op::Sound(9), Op::End];
    const SPAWNING: &[Op] = &[Op::Spawn(4, HELPER), Op::Wait(1), Op::End];

    #[test]
    fn a_task_spawned_into_a_later_slot_runs_the_same_frame() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.spawn(MAP_TASK, SPAWNING);
        events.update(&mut host);
        assert_eq!(host.log, ["sound 9"]);
    }

    const TALKING: &[Op] = &[Op::Dialogue(40), Op::Music(3), Op::Wait(2), Op::End];

    #[test]
    fn a_dialogue_holds_until_it_ends_then_the_task_goes_on_that_frame() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.spawn(MAP_TASK, TALKING);
        events.update(&mut host);
        assert!(events.in_dialogue());
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
        assert_eq!(host.log, ["dialogue 40"]);
        assert_eq!(events.update_hold(true, &mut host), HoldStep::Held);
        assert_eq!(host.log, ["dialogue 40", "music 3"]);
        assert!(!events.holding());
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Free);
    }

    const SHOPPING: &[Op] = &[
        Op::FadeOutHoldingAfter(2),
        Op::Shop(Shop::Items(1)),
        Op::Brightness(0),
    ];

    #[test]
    fn a_shop_opens_once_black_and_the_field_shows_at_once_after_it() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.run_now(SHOPPING, &mut host);
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
        assert_eq!(events.brightness(), 0);
        while !events.in_shop() {
            events.update_hold(false, &mut host);
        }
        assert_eq!(events.brightness(), BLACK);
        assert_eq!(host.log, ["shop Items(1)"]);
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
        assert_eq!(events.update_hold(true, &mut host), HoldStep::Held);
        assert!(!events.holding());
        assert_eq!(events.brightness(), 0);
    }

    const MEETING: &[Op] = &[
        Op::Freeze(1),
        Op::Sound(0x52),
        Op::FadeOutHoldingAfter(1),
        Op::Combat,
        Op::Freeze(2),
        Op::Sound(1),
    ];

    #[test]
    fn a_battle_holds_the_game_and_the_field_stays_frozen_after_it() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.run_now(MEETING, &mut host);
        assert!(host.log.is_empty());
        events.update_hold(false, &mut host);
        assert_eq!(host.log, ["sound 82"]);
        while !events.in_combat() {
            assert!(events.holding());
            events.update_hold(false, &mut host);
        }
        assert_eq!(events.brightness(), BLACK);
        assert_eq!(host.log, ["sound 82", "combat"]);
        events.update_hold(true, &mut host);
        assert!(events.holding());
        events.update_hold(false, &mut host);
        assert!(events.holding());
        events.update_hold(false, &mut host);
        assert_eq!(host.log.last().map(String::as_str), Some("sound 1"));
    }

    const CHOOSING: &[Op] = &[
        Op::IfFlags {
            all: &[0x120, 0x121],
            none: &[0x122],
            then: &[Op::Flag(0x122, true), Op::Music(5)],
            otherwise: &[Op::Music(6)],
        },
        Op::Sound(1),
    ];

    #[test]
    fn branches_on_flags_and_returns_to_the_program() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.run_now(CHOOSING, &mut host);
        assert_eq!(host.log, ["music 6", "sound 1"]);
        host.flags = vec![0x120, 0x121];
        host.log.clear();
        events.run_now(CHOOSING, &mut host);
        assert_eq!(host.log, ["music 5", "sound 1"]);
        assert!(host.flag(0x122));
        host.log.clear();
        events.run_now(CHOOSING, &mut host);
        assert_eq!(host.log, ["music 6", "sound 1"]);
    }

    const TEACHING: &[Op] = &[Op::IfCommand {
        command: 26,
        then: &[Op::Sound(2)],
        otherwise: &[Op::Sound(1), Op::LearnCommand(26)],
    }];

    #[test]
    fn a_lesson_runs_until_the_command_is_learned() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.run_now(TEACHING, &mut host);
        events.run_now(TEACHING, &mut host);
        assert_eq!(host.log, ["sound 1", "command 26", "sound 2"]);
    }

    const BY_AREA: &[Op] = &[Op::IfArea {
        area: 9,
        then: &[Op::Sound(9)],
        otherwise: &[Op::Sound(1)],
    }];

    #[test]
    fn branches_on_the_map_area() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.run_now(BY_AREA, &mut host);
        assert_eq!(host.log, ["sound 1"]);
    }

    #[test]
    fn only_a_task_dialogue_lets_the_actors_move() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.spawn(MAP_TASK, TALKING);
        events.update(&mut host);
        assert!(events.dialogue_in_task());
        let mut direct = Events::new();
        direct.run_now(TALKING, &mut host);
        assert!(direct.in_dialogue() && !direct.dialogue_in_task());
    }

    const RELOADING: &[Op] = &[Op::LoadMap {
        map: 24,
        player: (23, 29),
        objects: 0,
        count: 8,
    }];

    #[test]
    fn a_load_in_a_warps_handler_delays_its_fade_in() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.set_brightness(BLACK);
        events.fade_in_after(10, 0);
        events.run_now(RELOADING, &mut host);
        let held = (0..24)
            .take_while(|_| events.update_hold(false, &mut host) == HoldStep::Held)
            .count();
        assert_eq!(events.brightness(), BLACK);
        assert_eq!(held, 24);
        assert_eq!(host.log, ["map 24"]);
    }

    const TWICE: &[Op] = &[
        Op::Repeat(2, &[Op::Sound(1), Op::Call(&[Op::Sound(2)])]),
        Op::Sound(3),
    ];

    #[test]
    fn calls_and_repeats_come_back_to_the_program() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.run_now(TWICE, &mut host);
        assert_eq!(
            host.log,
            ["sound 1", "sound 2", "sound 1", "sound 2", "sound 3"]
        );
    }

    const FADING: &[Op] = &[Op::Brightness(0), Op::FadeOut(1), Op::End];

    #[test]
    fn a_fade_task_steps_every_other_frame_to_black() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.spawn(4, FADING);
        events.update(&mut host);
        assert_eq!(events.brightness(), 0);
        events.update(&mut host);
        assert_eq!(events.brightness(), 1);
        for _ in 0..60 {
            events.update(&mut host);
        }
        assert_eq!(events.brightness(), BLACK);
        assert!(events.task_running(4));
        events.update(&mut host);
        assert!(!events.task_running(4));
    }

    #[test]
    fn a_holding_fade_in_brightens_a_level_a_frame() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.set_brightness(BLACK);
        events.fade_in_holding();
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
        assert_eq!(events.brightness(), BLACK);
        for _ in 0..30 {
            assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
        }
        assert_eq!(events.brightness(), 1);
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Released);
        assert!(!events.holding());
    }

    #[test]
    fn an_exit_darkens_then_brightens_after_waiting() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.fade_out_holding(1);
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
        assert_eq!(events.brightness(), 0);
        for level in 1..BLACK {
            assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
            assert_eq!(events.brightness(), level);
        }
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Darkened);
        events.fade_in_after(10, 1);
        for _ in 0..10 {
            assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
        }
        for _ in 0..BLACK {
            assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
        }
        assert_eq!(events.brightness(), 0);
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Released);
    }
}
