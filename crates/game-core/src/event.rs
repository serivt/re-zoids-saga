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

use crate::field::{Actor, Command, Direction, Field, PIXEL, Walk};
use crate::menu::Shop;
use extraction::saga::Reward;

/// Task slots, as in the game's kernel.
pub const TASKS: usize = 16;
/// The slot of a map's own event.
pub const MAP_TASK: usize = 3;
/// The companions' slots in the game-state block (`+0xCD0`, `+0xCD1`):
/// the record each holds, [`NO_COMPANION`] when empty.
const COMPANION_SLOTS: usize = 0xCD0;
const NO_COMPANION: u8 = 0xFF;
/// Records of the companions' table, and the flag set once they are
/// offered.
const COMPANION_RECORDS: usize = 0x1D;
const COMPANIONS_OFFERED: u16 = 0x1D;
/// The records that are two forms of one character: one taken hides the
/// other.
const COMPANION_FORMS: [[u8; 2]; 2] = [[0x19, 0x1B], [0x1A, 0x1C]];
/// The slot that stands for the field's per-frame hook (RAM
/// `0x02000000`), which runs outside the tasks: a slot before the map's
/// task, so a load the hook makes lets the map's handler start its task.
/// Like the map's task, it ends when the player leaves the map.
pub const FIELD_HOOK: usize = 2;
/// The slot for a per-frame hook that watches where the player stands
/// (`0x08017208`): the original runs its field hook before the entities
/// move, so the game runs this slot on its own before the field's update
/// ([`Events::update_watch`]), never with the other tasks. It ends when the
/// player leaves the map.
pub const FIELD_WATCH: usize = 1;
const IMMEDIATE: usize = TASKS;
/// The darkest brightness level; levels above 16 all show black.
pub const BLACK: u8 = 31;
const FADE_IN_DELAY: u8 = 1;
/// Frames a slow fade in holds black before its first brighter level,
/// the frame of the op included: two, the frame it stores black, and one.
const SLOW_FADE_IN_DELAY: u8 = 2;
/// Frames the scene loader holds the game besides one per object: the
/// decompressions it waits a frame after, and the frame it returns on.
const LOAD_FRAMES: u32 = 6;
/// The same for the loader `0x080079E8`, which waits a frame less.
const SCENE_LOAD_FRAMES: u32 = 5;
/// A walking animation's number past its standing one.
const WALKING: usize = 4;

/// A [`Op::LoadMap`] or [`Op::Warp`] player cell that keeps the player
/// where it stands (the handlers that pass the player entity's own cell),
/// a warp keeping its facing too.
pub const HERE: (usize, usize) = (usize::MAX, usize::MAX);
/// The repeat count of a program that runs until its task ends.
const FOREVER: u16 = 0;
/// An actor index that stands for the space-time portal of the map
/// walked (see [`crate::field::Field::portal`]).
pub const THE_PORTAL: usize = usize::MAX - 1;
/// An actor index that stands for the guard that last saw the player
/// (see [`Op::IfSeen`]).
pub const SEEN_GUARD: usize = usize::MAX - 3;
/// A cell that stands for the portal's own.
pub const AT_THE_PORTAL: (usize, usize) = (usize::MAX - 1, usize::MAX - 1);
/// A cell that stands for the one below the portal.
pub const BELOW_THE_PORTAL: (usize, usize) = (usize::MAX - 2, usize::MAX - 2);
/// A cell that stands for the arrival of the exit the player pushed into.
pub const EXIT_ARRIVAL: (usize, usize) = (usize::MAX - 3, usize::MAX - 3);

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
    /// Runs dialogue string `index` as [`Op::Dialogue`] does, with script
    /// variable 0 (`0x02007574`) set to how many of `flags` are clear, as
    /// the count of what is left to find (`0x08021D1C`).
    DialogueCounting {
        /// The dialogue string.
        index: u16,
        /// The flags of what has been found.
        flags: &'static [u16],
    },
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
    /// Moves actor `actor`'s sprite to map pixel `to` a cell at a time
    /// (`0x08011F18`): a cell along x if it is not there yet, then one
    /// along y, and so on, `speed` pixels a frame for `frames` frames a
    /// cell with the walking animation of the way it goes, its cell left as
    /// it was; the camera scrolls along with the player when `camera` and
    /// otherwise stays, as it never follows a gliding sprite. It then stands
    /// facing its last way.
    Glide {
        /// Actor index.
        actor: usize,
        /// Target, in map pixels of the sprite's position.
        to: (i32, i32),
        /// Pixels a frame.
        speed: i32,
        /// Frames a cell takes.
        frames: u32,
        /// Whether the camera scrolls along with the player.
        camera: bool,
    },
    /// Waits until sound or song `n` has ended (`0x08001A28`).
    AwaitSoundEnd(u16),
    /// Waits for the last frame of step `step` of actor `actor`'s
    /// animation (its entity's `+0x34` word at `0x10000 | 2 × step`).
    AwaitStepEnd(usize, usize),
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
    /// Ends the task in slot `n` (`0x08003DD8`), a helper still under way.
    Stop(usize),
    /// Plays a song unless it is already playing (`0x080019B4`).
    Music(u16),
    /// Stops song `n` and plays it from its start (`0x08001A08`, then
    /// `0x080019B4` with the last song forgotten).
    RestartMusic(u16),
    /// Plays a sound effect (`0x080019EC`).
    Sound(u16),
    /// Sets the brightness level and restarts the fade counter
    /// (`0x08001624` sets 0, `0x0800159C` sets 31).
    Brightness(u8),
    /// Marks the characters of group `group` as met, for the character
    /// guide (`0x08037858`).
    Meet(u8),
    /// Adds the characters of list `list` to the party, each with a unit of
    /// its Zoid (`0x080374B8`).
    Join(u8),
    /// Takes the characters of list `list` out of the party (`0x080374E4`).
    Leave(u8),
    /// Works out which companions chapter 9's base offers (`0x08026694`):
    /// game flag `n`, for each record `n` of the companions' table (0 to
    /// `0x1C`), set when the record is offered; flag `0x1D` set.
    OfferCompanions,
    /// Takes the companion in slot `slot` off the lists: its record's flag
    /// cleared, with the other form of the same character (records `0x19`
    /// and `0x1B`, `0x1A` and `0x1C`).
    HideCompanion(usize),
    /// Runs `then` when companion slot `slot` (game state `+0xCD0` on) holds
    /// a record, `otherwise` when it is empty (`0xFF`).
    IfCompanion {
        /// The slot: 0 Jack's pick, 1 Earth's.
        slot: usize,
        /// Program run when the slot holds a record.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// When the menu just shown picked a record (script variable 1 below
    /// `0x1D`), puts it in companion slot `slot`, adds its character to the
    /// party and runs `then`.
    TakeCompanion {
        /// The slot.
        slot: usize,
        /// Whether the character joins with a unit of its Zoid
        /// (`0x08026614`, chapter 9) or alone (`0x0802A67C`, chapter 10).
        zoid: bool,
        /// Program run once a record is taken.
        then: &'static [Op],
    },
    /// Sends the companion in slot `slot` away (`0x08026634`) and empties
    /// the slot.
    DropCompanion(usize),
    /// Empties both companion slots, sending no one away.
    ForgetCompanions,
    /// Steps the brightness toward black every `mask + 1` frames until it
    /// is black (the loop of the task at `0x0800C6A4`).
    FadeOut(u16),
    /// Steps the brightness back every `mask + 1` frames until it is
    /// normal (the task at `0x0800C680`).
    FadeIn(u16),
    /// Brightens the screen to normal one level a frame, holding the game
    /// (what follows loading a map).
    FadeInHolding,
    /// Brightens the screen from black to normal a level every other
    /// frame, holding the game, after two frames at black (`0x080014A8`
    /// with 1).
    FadeInHoldingSlow,
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
    /// Takes actor `actor`'s animation to the last frame of step `step` (a
    /// task writing the entity's `+0x34` with its countdown `+0x36` at 1).
    RestartStep(usize, usize),
    /// Stands actor `actor` on the cell its sprite's position falls in.
    Settle(usize),
    /// Stops the music and every sound (`0x08001988`).
    Silence,
    /// Makes the fades brighten toward white (`BLDCNT` `0x3FBF`, every
    /// layer) or darken toward black (`0x3FFF`), as the fade level rises.
    FadeToWhite(bool),
    /// Sets actor `actor`'s animation tick shift.
    Shift(usize, i8),
    /// Waits until one of the flags is set.
    AwaitAnyFlag(&'static [u16]),
    /// Waits until the player's cell is between columns `columns.0` and
    /// `columns.1` inclusive (when given) and between rows `rows.0` and
    /// `rows.1` (when given).
    AwaitPlayer {
        /// First and last column that count.
        columns: Option<(usize, usize)>,
        /// First and last row that count.
        rows: Option<(usize, usize)>,
    },
    /// Runs `then` while the player is taking a step (entity `+0x4A` at 2,
    /// its cell already the one it steps onto), `otherwise` else.
    IfPlayerWalking {
        /// Program run when it is.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Runs `then` when the player's cell is within `columns` and `rows`
    /// (as [`Op::AwaitPlayer`] counts them), `otherwise` else.
    IfPlayer {
        /// First and last column that count.
        columns: Option<(usize, usize)>,
        /// First and last row that count.
        rows: Option<(usize, usize)>,
        /// Program run when it is.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Waits until the player's sprite is within `x` and `y`, inclusive
    /// ranges of map pixels of its box's top-left (the hooks that test the
    /// entity's position, `+0x08` and `+0x0C`): a step only gets there at
    /// its end.
    AwaitPlayerSprite {
        /// First and last x that count.
        x: (i32, i32),
        /// First and last y that count.
        y: (i32, i32),
    },
    /// Runs `then` when the player's sprite is within `x` and `y` (as
    /// [`Op::AwaitPlayerSprite`] counts them), `otherwise` else.
    IfPlayerSprite {
        /// First and last x that count.
        x: (i32, i32),
        /// First and last y that count.
        y: (i32, i32),
        /// Program run when it is.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Marks Zoid picture `id` as seen, entering it in the Zoid guide
    /// (`0x08037098`).
    SeeZoid(u8),
    /// Opens the chest being searched: its opening animation and sound
    /// (`0x0800B938`).
    OpenChest,
    /// Marks the chest being searched as opened.
    MarkChest,
    /// Runs `then` when the chest gives a reward of `kind`, `otherwise`
    /// else.
    IfChest {
        /// The kind of reward.
        kind: ChestKind,
        /// Program run for it.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Runs `then` when the party already holds the Zi data the chest
    /// gives (`0x08037098`), `otherwise` else.
    IfZiDataHeld {
        /// Program run when it does.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Makes `reward` the one the chest's ops give and announce, in place of
    /// the searched chest's, or with `None` gives them back to the chest:
    /// an event's gift runs the chest's announcement (`0x08037A24` gives a
    /// Zoid's Zi data that way).
    Gift(Option<Reward>),
    /// Runs `then` when one of the unit slots 0–`0xAC` holds Zoid `zoid`
    /// (`0x0802AACC`), `otherwise` else.
    IfZoidOwned {
        /// The Zoid.
        zoid: u16,
        /// Program run when a unit is of it.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Runs `then` when the formation meets the regulation of the
    /// colosseum's match `game` (`0x08038654`), `otherwise` else.
    IfRegulation {
        /// The match, 0 to 14.
        game: u8,
        /// Program run when it does.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Runs `then` when byte `at` of the game-state block is not 0,
    /// `otherwise` else.
    IfStateSet {
        /// Offset in the game-state block.
        at: usize,
        /// Program run when it is set.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Gives the chest's reward (`0x08037014`, `0x08037040`, `0x0803706C`,
    /// `0x08037098`, `0x08037100`); money is also printed into window 1
    /// (`0x08001848`).
    TakeChest,
    /// Prints the name of the chest's reward, holding the game while its
    /// script runs: `name` 241 + n for a consumable (`0x08032840`), `item`
    /// n for a core (`0x080328E4`), `part` n (`0x086664F0`) and `name`
    /// 1 + n for a Zoid (`0x08032800`).
    ChestName,
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
    /// Fights story battle `n` (`0x08008D28`, the battle module in story
    /// mode), holding the game until it hands back.
    StoryBattle(u8),
    /// Rolls the staff credits (`0x0800C430`), holding the game until they
    /// are over.
    Credits,
    /// Runs `then` when the last battle was lost (`0x08008D28` returned 1),
    /// `otherwise` else.
    IfLost {
        /// Program run after a defeat.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Takes the beaten party to its area's return point facing up
    /// (`0x08006E08`, then `0x08007188`), running the map's handler.
    WarpHome,
    /// Gives actor `actor` sprite `sprite` and starts its animation of the
    /// same facing (`0x080089A0`).
    Sprite(usize, usize),
    /// Forms the party around the Zoid picked in the hangar
    /// (`0x08037644`: 0 the Shield Liger, 1 the Saber Tiger, 2 the Raynos).
    FormParty(u8),
    /// Runs `then` when the map walked has a space-time portal, `otherwise`
    /// else.
    IfPortal {
        /// Program run when it has.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Plays the sound of the exit the player pushed into (`0x080083B8`).
    ExitSound,
    /// Takes the player through the exit it pushed into, keeping its facing
    /// (`0x08007188`), and runs the map's handler.
    TakeExit,
    /// Runs `program` over and over, as the field's per-frame hook runs its
    /// routine: the program yields every frame, and [`Op::End`] ends it.
    Loop(&'static [Op]),
    /// Runs `then` when the attribute of the player's cell has any of
    /// `bits` (`0x08008434` with the player's entity), `otherwise` else.
    IfPlayerOn {
        /// Attribute bits tested.
        bits: u16,
        /// Program run when one is set.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Runs `then` when more than `more_than` roaming battles have been won
    /// since the count was last cleared (the game state's half-word `+0x0A`,
    /// which `0x0800B9CC` counts), `otherwise` else.
    IfBattlesWon {
        /// The count to exceed.
        more_than: u16,
        /// Program run when it is exceeded.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Clears the count of roaming battles won.
    ForgetBattlesWon,
    /// Runs `then` when a task runs in slot `slot`, `otherwise` else.
    IfTask {
        /// The slot.
        slot: usize,
        /// Program run when a task runs there.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Runs `then` when song `song` is the one playing (RAM `0x02000B54`),
    /// `otherwise` else.
    IfMusic {
        /// The song.
        song: u16,
        /// Program run when it plays.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Plays the map's own song from its start (`0x08001A08` and
    /// `0x080019B4` with the song of the map's record).
    RestartMapMusic,
    /// Makes return point `index` the one a beaten party is taken to
    /// (`0x08006DFC`: the game state's byte 3), until the next map entered
    /// sets its area's.
    ReturnPoint(u8),
    /// Walks the player a cell back, away from the way it faces, through
    /// everything (entity command 11 toward the cell behind it).
    StepBack {
        /// Pixels a frame, 16.16 fixed point.
        speed: i32,
        /// Animation tick shift while walking.
        shift: i8,
    },
    /// Walks the player from its cell to the one `by` cells away, not
    /// through walls and actors (entity command 10 toward that cell).
    StepPlayer {
        /// Columns and rows to go.
        by: (isize, isize),
        /// Pixels a frame, 16.16 fixed point.
        speed: i32,
        /// Animation tick shift while walking.
        shift: i8,
    },
    /// Loads map `map` with `count` objects from the list at ROM address
    /// `objects` and the player on `player` (`0x080079E8`); the game holds
    /// while it decompresses and places one object a frame.
    LoadMap {
        /// Map record.
        map: usize,
        /// The player's metatile.
        player: (usize, usize),
        /// ROM address of the object list, where Rev 1 keeps it.
        objects: u32,
        /// Objects in the list.
        count: usize,
    },
    /// Loads a map for a cutscene as `0x080079E8` does: as
    /// [`Op::LoadMap`], but the map's song is left as it is.
    LoadScene {
        /// Map record.
        map: usize,
        /// The player's metatile.
        player: (usize, usize),
        /// ROM address of the object list, where Rev 1 keeps it.
        objects: u32,
        /// Objects in the list.
        count: usize,
    },
    /// Keeps the enemies from meeting the player (`0x02000008` bit 1 set),
    /// or lets them again.
    Calm(bool),
    /// Lets the party cross the sea, or keeps it off the water (the game
    /// state's half-word 0, bit 0).
    SeaCrossing(bool),
    /// Remembers the song the field or the story last asked for
    /// (`0x02000B54`, which a battle's own song leaves as it is), as the
    /// staged scenes' routine does before it plays theirs (`0x08012040`).
    SaveMusic,
    /// Plays the song [`Op::SaveMusic`] remembered again.
    RestoreMusic,
    /// Places actor `actor` on one of `cells`, drawn with the game's random
    /// numbers (`0x08001080`, modulo their count).
    PlaceRandom(usize, &'static [(usize, usize)]),
    /// Makes the field's cells `tiles` tiles on a side (`0x0200000C +
    /// 0x18`), as a cutscene does to walk people on a Zoid map.
    CellTiles(usize),
    /// Builds the area's roaming enemies and objects again (`0x08006E4C`).
    RebuildObjects,
    /// Sends the warps to another map until the next map's handler runs
    /// (the field's second hook, `0x02000004`, which the warp routine
    /// `0x08007188` calls with the destination).
    RedirectWarps(WarpRedirect),
    /// Brightens the field's layers and sprites toward white, the windows
    /// left out (`BLDCNT` `0xBE` with `BLDY` at `level` of 16), or stops
    /// (level 0).
    Whiten(u8),
    /// Runs `then` when one of `guards` sees the player (`0x0801BC2C`, see
    /// [`crate::field::Field::seen_by`]): the guard stops, and
    /// [`SEEN_GUARD`] stands for it from then on; `otherwise` else.
    IfSeen {
        /// The guards, by actor.
        guards: &'static [usize],
        /// Program run when one sees the player.
        then: &'static [Op],
        /// Program run otherwise.
        otherwise: &'static [Op],
    },
    /// Turns the player toward the guard that saw it (`0x0801BD9C`).
    FaceSeenGuard,
    /// Places actor `actor` on the player's cell moved by `(dx, dy)`
    /// cells, as the handlers that read the player's entity do.
    PlaceNearPlayer(usize, (i32, i32)),
    /// Places actor `actor` on the cell of actor `other`, as the tasks that
    /// read another entity's cell do.
    PlaceOnActor(usize, usize),
    /// Glides actor `actor` to the player's cell moved by `by` cells.
    GlideNearPlayer {
        /// Actor index.
        actor: usize,
        /// Cells from the player's.
        by: (i32, i32),
        /// Pixels a frame.
        speed: i32,
        /// Whether the camera scrolls along with the player.
        camera: bool,
    },
    /// Makes actor `actor`'s animation stop at its end (entity flag
    /// `0x400`) without changing it.
    Once(usize),
    /// Opens the armaments shop of area 10's map 337 (`0x080093FC`), the
    /// one the roaming battles won as the game was started or continued
    /// pick (see [`extraction::saga_shop::rotating_arms_shop`]), holding
    /// the game as [`Op::Shop`] does.
    RotatingArmsShop,
    /// Sets actor `actor` walking around at random (entity command 2), or
    /// standing still (1).
    Roam(usize, bool),
    /// Writes actor `actor`'s cell into its object state, as a step does
    /// (the game state's `+0x52 + 0x10 × n`).
    KeepCell(usize),
    /// Loads the colosseum's arena for match `n` (`0x0801A70C`): its list
    /// with the formation's members after it (see
    /// [`extraction::saga_arena::arena_objects`]) and the player on
    /// [`extraction::saga_arena::ARENA_CELL`], holding the game as
    /// [`Op::LoadMap`] does.
    LoadArena(u8),
}

/// What an event asks of the game it runs in.
pub trait EventHost {
    /// The field, when one is loaded.
    fn field(&mut self) -> Option<&mut Field>;
    /// Whether game flag `flag` is set.
    fn flag(&self, flag: u16) -> bool;
    /// Sets or clears game flag `flag`.
    fn set_flag(&mut self, flag: u16, set: bool);
    /// Starts dialogue string `index`, as a task's call when `called`; the
    /// game holds until it ends.
    fn start_dialogue(&mut self, index: u16, called: bool);
    /// Sets script variable `slot` of the dialogue just started.
    fn set_dialogue_variable(&mut self, slot: u8, value: u16);
    /// Starts battle scene `scene`; the game holds until it ends.
    fn start_battle(&mut self, scene: u8);
    /// Starts the staff credits; the game holds until they are over.
    fn start_credits(&mut self);
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
    /// Plays song `song` from its start.
    fn restart_music(&mut self, song: u16);
    /// Plays sound effect `sound`.
    fn play_sound(&mut self, sound: u16);
    /// Loads a map for a cutscene (`0x080076C0`).
    fn load_map(&mut self, map: usize, player: (usize, usize), objects: u32, count: usize);
    /// Loads the arena for match `game` and returns how many objects it
    /// has.
    fn load_arena(&mut self, game: u8) -> usize;
    /// Keeps the enemies from meeting the player, or lets them.
    fn set_calm(&mut self, calm: bool);
    /// Lets the party cross the sea, or keeps it off the water.
    fn set_sea_crossing(&mut self, crossing: bool);
    /// Sends the warps elsewhere until the next map's handler runs.
    fn redirect_warps(&mut self, redirect: WarpRedirect);
    /// Builds the area's roaming enemies and objects again.
    fn rebuild_objects(&mut self);
    /// Remembers the song the field or the story last asked for.
    fn save_music(&mut self);
    /// Plays the song remembered again.
    fn restore_music(&mut self);
    /// Brightens the field toward white by `level` of 16.
    fn set_whiten(&mut self, level: u8);
    /// Makes the fade level brighten toward white instead of darkening.
    fn set_fade_to_white(&mut self, white: bool);
    /// Stops the music and every sound.
    fn silence(&mut self);
    /// The guard that last saw the player.
    fn seen_guard(&self) -> Option<usize>;
    /// Remembers `guard` as the one that saw the player.
    fn set_seen_guard(&mut self, guard: usize);
    /// Whether the formation meets match `game`'s regulation.
    fn meets_regulation(&self, game: u8) -> bool;
    /// Starts map `map`'s song unless it plays already, as the cutscene
    /// loader does last.
    fn start_map_music(&mut self, map: usize);
    /// Marks the characters of group `group` as met.
    fn meet(&mut self, group: u8);
    /// Adds the characters of list `list` to the party.
    fn join(&mut self, list: u8);
    /// Takes the characters of list `list` out of the party.
    fn leave(&mut self, list: u8);
    /// Whether record `index` of the companions' table is offered.
    fn companion_offered(&self, index: usize) -> bool;
    /// Adds the character of the companions' record `index` to the party.
    fn join_companion(&mut self, index: u8, zoid: bool);
    /// Takes the character of the companions' record `index` out of the
    /// party.
    fn leave_companion(&mut self, index: u8);
    /// Sets byte `at` of the game-state block.
    fn set_state_byte(&mut self, at: usize, value: u8);
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
    /// The kind of reward the searched chest gives, if any.
    fn chest_kind(&self) -> Option<ChestKind>;
    /// Whether the party already holds the Zi data the searched chest
    /// gives.
    fn zi_data_held(&self) -> bool;
    /// Gives the searched chest's reward; money is printed into window 1.
    fn take_chest(&mut self);
    /// Makes `reward` the chest's ops' reward (see [`Op::Gift`]).
    fn set_gift(&mut self, reward: Option<Reward>);
    /// Whether a unit slot holds Zoid `zoid`.
    fn zoid_owned(&self, zoid: u16) -> bool;
    /// Byte `at` of the game-state block, 0 outside it.
    fn state_byte(&self, at: usize) -> u8;
    /// Starts the script that prints the name of the searched chest's
    /// reward.
    fn start_chest_name(&mut self);
    /// Takes the player to `cell` of `map` with its own objects and runs the
    /// map's handler; returns how many objects the map places.
    fn warp(&mut self, map: usize, cell: (usize, usize), facing: Option<Direction>) -> usize;
    /// Starts story battle `battle`; the game holds until it hands back.
    fn start_story_battle(&mut self, battle: u8);
    /// Whether the last battle was lost.
    fn battle_lost(&self) -> bool;
    /// The area's return point: its map and cell (`0x08006E08`).
    fn return_point(&self) -> Option<(usize, (usize, usize))>;
    /// Gives actor `actor` sprite `sprite` (`0x080089A0`).
    fn set_sprite(&mut self, actor: usize, sprite: usize);
    /// Whether sound or song `n` has ended.
    fn sound_ended(&self, n: u16) -> bool;
    /// The song playing, if any.
    fn music_playing(&self) -> Option<u16>;
    /// Plays the current map's own song from its start.
    fn restart_map_music(&mut self);
    /// The roaming battles won since the count was last cleared.
    fn battles_won(&self) -> u16;
    /// Clears the count of roaming battles won.
    fn forget_battles_won(&mut self);
    /// The armaments shop area 10's rotating keeper opens.
    fn rotating_arms_shop(&self) -> u8;
    /// Writes `cell` into object state `slot`.
    fn keep_object_cell(&mut self, slot: usize, cell: (usize, usize));
    /// Makes return point `index` the one a beaten party is taken to.
    fn set_return_point(&mut self, index: u8);
    /// Plays the sound of the exit the player pushed into.
    fn exit_sound(&mut self);
    /// Takes the player through the exit it pushed into; returns how many
    /// objects the map places.
    fn take_exit(&mut self) -> usize;
    /// Where the exit the player pushed into leads.
    fn exit_arrival(&self) -> Option<(usize, usize)>;
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
    glide: Option<Glide>,
}

impl Task {
    fn new(program: &'static [Op]) -> Self {
        Self {
            frames: vec![Frame::new(program, 1)],
            wait: 0,
            fade_counter: 0,
            glide: None,
        }
    }
}

/// What kind of reward a chest gives, as [`Op::IfChest`] tests it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChestKind {
    /// A Zoid core.
    Core,
    /// A Zoid's Zi data.
    ZiData,
    /// A part.
    Part,
    /// A consumable.
    Consumable,
    /// Money.
    Money,
}

/// Where an [`Op::Glide`] stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Glide {
    /// Whether the next check is along y.
    along_y: bool,
    /// Frames left of the cell under way, and its step a frame.
    left: u32,
    step: (i32, i32),
    /// The standing animation it plays once there: its own at the start,
    /// then the one of the way it last went.
    standing: usize,
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
    /// Holding the game this many more frames, every task then going on
    /// as it was: a map loaded within a warp's call.
    Frozen(u32),
    /// A fade in holding the game after `delay` frames of waiting, and
    /// `settle` more once normal.
    FadeIn {
        /// Frames left before the first level.
        delay: u8,
        /// Frames held once the brightness is normal.
        settle: u8,
        /// Whether the tasks already run in the frame the brightness is
        /// normal, while the world is still held: after a warp, but not
        /// after a map its handler loaded again.
        tasks_first: bool,
        /// Whether each level lasts two frames (`0x080014A8` with 1).
        slow: bool,
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
    /// The map a task's cutscene load brings, whose song starts once the
    /// load is over.
    loading_map: Option<usize>,
    /// The song of a map a handler loaded while a fade in holds the game,
    /// and the frames its load takes before the song starts.
    handler_music: Option<(u32, usize)>,
    /// Whether a task's dialogue started this frame: the other tasks have
    /// had their turn in it already.
    dialogue_started: bool,
    /// The slot of the task whose load last ended, and the slot whose
    /// spawns are lost while the handler of a map that task warped to runs.
    loaded_by: Option<usize>,
    within: Option<usize>,
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

    /// Whether `program`, run as a handler the game called directly (see
    /// [`Events::run_now`]), is still running.
    #[must_use]
    pub fn runs_handler(&self, program: &'static [Op]) -> bool {
        self.tasks[IMMEDIATE]
            .as_ref()
            .and_then(|task| task.frames.first())
            .is_some_and(|frame| std::ptr::eq(frame.program, program))
    }

    /// Starts `program` in `slot`, replacing what runs there.
    pub fn spawn(&mut self, slot: usize, program: &'static [Op]) {
        // The original's load runs within the task's own call: a map's
        // handler spawning into that task's slot is overwritten when the
        // task, going on, next yields.
        if matches!(self.hold, Some(Hold::Loading { slot: loading, .. }) if loading == slot)
            || self.within == Some(slot)
        {
            return;
        }
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

    /// Runs the handler of the map a task's warp just loaded: the original
    /// runs it within the task's own call, so what it spawns into that
    /// task's slot is overwritten when the task, going on, next yields, and
    /// a map it loads holds the game before the task goes on.
    pub fn run_after_load(&mut self, program: &'static [Op], host: &mut impl EventHost) {
        self.within = self.loaded_by.take();
        self.run_now(program, host);
        self.within = None;
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
        self.hold = Some(Hold::FadeIn {
            delay,
            settle,
            tasks_first: settle > 0,
            slow: false,
        });
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
        if let Some((frames, map)) = self.handler_music {
            if frames > 1 {
                self.handler_music = Some((frames - 1, map));
            } else {
                self.handler_music = None;
                host.start_map_music(map);
            }
        }
        match self.hold {
            None => HoldStep::Free,
            Some(
                Hold::Dialogue(slot) | Hold::Battle(slot) | Hold::Shop(slot) | Hold::Combat(slot),
            ) => {
                if dialogue_done {
                    self.hold = None;
                    self.dialogue_started = false;
                    self.run_task(slot, host);
                    self.run_slots(slot + 1, host);
                } else if self.task_dialogue().is_some()
                    && !std::mem::take(&mut self.dialogue_started)
                {
                    self.run_slots(0, host);
                }
                HoldStep::Held
            }
            Some(Hold::Frozen(frames)) => {
                self.hold = (frames > 1).then_some(Hold::Frozen(frames - 1));
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
                self.end_loading(slot, host);
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
            Some(Hold::FadeIn {
                delay,
                settle,
                tasks_first,
                slow,
            }) if delay > 0 => {
                self.hold = Some(Hold::FadeIn {
                    delay: delay - 1,
                    settle,
                    tasks_first,
                    slow,
                });
                HoldStep::Held
            }
            Some(Hold::FadeIn { settle, .. }) if self.brightness == 0 => {
                if settle > 0 {
                    self.hold = Some(Hold::FadeIn {
                        delay: 0,
                        settle: settle - 1,
                        tasks_first: false,
                        slow: false,
                    });
                    return HoldStep::Held;
                }
                self.hold = None;
                HoldStep::Released
            }
            Some(Hold::FadeIn {
                settle,
                tasks_first,
                slow,
                ..
            }) => self.brighten(settle, tasks_first, slow, host),
        }
    }

    /// Whether a map's load holds the game.
    #[must_use]
    pub fn loading(&self) -> bool {
        matches!(self.hold, Some(Hold::Loading { .. }))
    }

    /// Starts map `map`'s song once the load holding the game ends, as the
    /// warp routine (`0x08007188`) does after placing the objects; `false`
    /// when no load holds the game.
    pub fn start_music_after_loading(&mut self, map: usize) -> bool {
        if self.loading() {
            self.loading_map = Some(map);
            true
        } else {
            false
        }
    }

    /// A cutscene's map load ([`Op::LoadMap`] or [`Op::LoadScene`]): the
    /// game holds while it lasts, and the first starts the map's song once
    /// it is over, or, run by a handler while a fade in holds the game,
    /// once its load would be.
    fn load(&mut self, slot: usize, op: Op, host: &mut impl EventHost) -> Flow {
        let (Op::LoadMap {
            map,
            player,
            objects,
            count,
        }
        | Op::LoadScene {
            map,
            player,
            objects,
            count,
        }) = op
        else {
            return Flow::Next;
        };
        host.load_map(map, player, objects, count);
        let music = matches!(op, Op::LoadMap { .. });
        if music && slot == IMMEDIATE {
            if matches!(self.hold, Some(Hold::FadeIn { .. })) {
                let frames = LOAD_FRAMES + u32::try_from(count).unwrap_or(u32::MAX);
                self.handler_music = Some((frames, map));
            } else {
                host.start_map_music(map);
            }
        } else if music {
            self.loading_map = Some(map);
        }
        let base = if music {
            LOAD_FRAMES
        } else {
            SCENE_LOAD_FRAMES
        };
        self.hold_frames(slot, base + u32::try_from(count).unwrap_or(u32::MAX))
    }

    /// The end of a load: the task that made it goes on, then the tasks
    /// after it run.
    fn end_loading(&mut self, slot: usize, host: &mut impl EventHost) {
        self.hold = None;
        self.loaded_by = Some(slot);
        if let Some(map) = self.loading_map.take() {
            host.start_map_music(map);
        }
        self.run_task(slot, host);
        self.run_slots(slot + 1, host);
    }

    /// A level of a fade in; on reaching normal brightness, the frames
    /// held after it begin.
    fn brighten(
        &mut self,
        settle: u8,
        tasks_first: bool,
        slow: bool,
        host: &mut impl EventHost,
    ) -> HoldStep {
        self.brightness -= 1;
        if self.brightness > 0 {
            if slow {
                self.hold = Some(Hold::FadeIn {
                    delay: 1,
                    settle,
                    tasks_first,
                    slow,
                });
            }
            return HoldStep::Held;
        }
        if settle == 0 {
            self.hold = None;
            return HoldStep::Released;
        }
        // The tasks and the actors' animations may run in the frame the
        // screen is bright again; the rest of the world only from the next.
        self.hold = None;
        if tasks_first {
            if let Some(field) = host.field() {
                field.tick_animations();
            }
            self.run_slots(0, host);
        }
        if self.hold.is_none() {
            self.hold = Some(Hold::FadeIn {
                delay: 0,
                settle: settle - 1,
                tasks_first: false,
                slow: false,
            });
        }
        HoldStep::Held
    }

    /// Runs every task for one frame, in slot order.
    pub fn update(&mut self, host: &mut impl EventHost) {
        self.run_slots(0, host);
    }

    /// Runs the field's watch ([`FIELD_WATCH`]) for one frame, unless
    /// something holds the game.
    pub fn update_watch(&mut self, host: &mut impl EventHost) {
        if self.hold.is_none() {
            self.step_watch(host);
        }
    }

    /// Whether a story warp's fade in holds the game (`0x080014A8` with 1):
    /// its callback (`0x0800BEA4`) runs the field's hook and the objects
    /// every frame of it.
    #[must_use]
    pub fn fading_in_slowly(&self) -> bool {
        matches!(self.hold, Some(Hold::FadeIn { slow: true, .. }))
    }

    /// Runs the field's watch for one frame of a story warp's fade in.
    pub fn update_watch_fading(&mut self, host: &mut impl EventHost) {
        if self.fading_in_slowly() {
            self.step_watch(host);
        }
    }

    fn step_watch(&mut self, host: &mut impl EventHost) {
        if let Some(task) = self.tasks[FIELD_WATCH].as_mut() {
            if task.wait > 1 {
                task.wait -= 1;
                return;
            }
            task.wait = 0;
            self.run_task(FIELD_WATCH, host);
        }
    }

    /// The task whose dialogue or script holds only itself: the kernel
    /// switches away from a task waiting on its script runner every frame
    /// (`0x0803E51C` calls `0x0805EF90`), so the other tasks go on.
    fn task_dialogue(&self) -> Option<usize> {
        match self.hold {
            Some(Hold::Dialogue(slot)) if slot < IMMEDIATE => Some(slot),
            _ => None,
        }
    }

    fn run_slots(&mut self, from: usize, host: &mut impl EventHost) {
        for slot in from..=TASKS {
            if slot == FIELD_WATCH {
                continue;
            }
            if self.hold.is_some() {
                match self.task_dialogue() {
                    Some(talking) if talking == slot => continue,
                    Some(_) => {}
                    None => return,
                }
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
                if frame.repeat == FOREVER {
                    frame.pc = 0;
                } else if frame.repeat > 1 {
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
        let op = resolve(op, host);
        match op {
            op if starts_hold(&op) => self.wait(slot, op, host),
            Op::Wait(_)
            | Op::WarpHome
            | Op::Freeze(_)
            | Op::AwaitArrival(_)
            | Op::AwaitAnimation(_)
            | Op::AwaitStepEnd(..)
            | Op::AwaitSoundEnd(_)
            | Op::AwaitAnyFlag(_)
            | Op::AwaitPlayer { .. }
            | Op::AwaitPlayerSprite { .. }
            | Op::FadeInHolding
            | Op::FadeInHoldingSlow
            | Op::FadeOutHolding
            | Op::FadeOutHoldingAfter(_)
            | Op::LoadMap { .. }
            | Op::LoadScene { .. }
            | Op::LoadArena(_)
            | Op::TakeExit
            | Op::Warp { .. } => self.wait(slot, op, host),
            op if branches(&op) => self.branch(slot, op, host),
            Op::Sprite(actor, sprite) => {
                host.set_sprite(actor, sprite);
                Flow::Next
            }
            Op::Stop(other) => {
                self.end(other);
                Flow::Next
            }
            Op::Glide {
                actor,
                to,
                speed,
                frames,
                camera,
            } => self.glide(slot, actor, (to, speed, frames, camera), host),
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
            | Op::RestartStep(..)
            | Op::Settle(_)
            | Op::StepBack { .. }
            | Op::StepPlayer { .. }
            | Op::PlaceRandom(..)
            | Op::CellTiles(_)
            | Op::Once(_)
            | Op::Roam(..)
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
            op if starts_hold(&op) => {
                self.advance(slot);
                self.hold = Some(start_hold(slot, op, host));
                self.dialogue_started = self.task_dialogue().is_some();
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
            Op::FadeInHolding
            | Op::FadeInHoldingSlow
            | Op::FadeOutHolding
            | Op::FadeOutHoldingAfter(_) => {
                self.advance(slot);
                self.hold_fade(slot, op);
                return Flow::Yield;
            }
            Op::LoadMap { .. } | Op::LoadScene { .. } => return self.load(slot, op, host),
            Op::LoadArena(game) => {
                let count = host.load_arena(game);
                self.loading_map = Some(extraction::saga_arena::ARENA_MAP);
                return self.hold_loading(slot, count);
            }
            Op::Warp { map, cell, facing } => {
                let count = host.warp(map, cell, facing);
                return self.hold_loading(slot, count);
            }
            Op::TakeExit => {
                let count = host.take_exit();
                return self.hold_loading(slot, count);
            }
            Op::WarpHome => {
                let Some((map, cell)) = host.return_point() else {
                    return Flow::Next;
                };
                let count = host.warp(map, cell, Some(Direction::Up));
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
            Op::AwaitSoundEnd(sound) => host.sound_ended(sound),
            Op::AwaitStepEnd(actor, step) => host
                .field()
                .and_then(|field| field.actor(actor).map(|actor| actor.ends_step(step)))
                .unwrap_or(true),
            Op::AwaitAnyFlag(flags) => flags.iter().any(|&flag| host.flag(flag)),
            Op::AwaitPlayer { columns, rows } => player_within(host, columns, rows),
            Op::AwaitPlayerSprite { x, y } => sprite_within(host, x, y),
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

    /// Holds the game for a fade op.
    fn hold_fade(&mut self, slot: usize, op: Op) {
        match op {
            Op::FadeInHolding => self.fade_in_holding(),
            Op::FadeInHoldingSlow => {
                self.hold = Some(Hold::FadeIn {
                    delay: SLOW_FADE_IN_DELAY,
                    settle: 0,
                    tasks_first: false,
                    slow: true,
                });
            }
            Op::FadeOutHoldingAfter(delay) => {
                self.hold = Some(Hold::FadeOut {
                    delay,
                    resume: Some(slot),
                });
            }
            _ => {
                self.hold = Some(Hold::FadeOut {
                    delay: 0,
                    resume: Some(slot),
                });
            }
        }
    }

    /// A frame of an [`Op::Glide`]: a pixel of the cell under way, or the
    /// checks that start the next cell or end it.
    fn glide(
        &mut self,
        slot: usize,
        actor: usize,
        (to, speed, frames, camera): ((i32, i32), i32, u32, bool),
        host: &mut impl EventHost,
    ) -> Flow {
        let Some(field) = host.field() else {
            return Flow::Next;
        };
        let Some(task) = self.tasks[slot].as_mut() else {
            return Flow::Stop;
        };
        let mut glide = task.glide.take().unwrap_or(Glide {
            along_y: false,
            left: 0,
            step: (0, 0),
            standing: field.actor(actor).map_or(0, |actor| actor.animation_id),
        });
        let target = (to.0 * PIXEL, to.1 * PIXEL);
        let mut checked = 0;
        while glide.left == 0 {
            if checked == 2 {
                if let Some(actor) = field.actor_mut(actor) {
                    if let Some(facing) = Direction::from_index(glide.standing) {
                        actor.facing = facing;
                    }
                    actor.play(glide.standing);
                }
                return Flow::Next;
            }
            checked += 1;
            let Some(position) = field.actor(actor).map(Actor::fixed_position) else {
                return Flow::Next;
            };
            let (at, goal) = if glide.along_y {
                (position.1, target.1)
            } else {
                (position.0, target.0)
            };
            let along_y = glide.along_y;
            glide.along_y = !along_y;
            if at == goal {
                continue;
            }
            checked = 0;
            let forward = at < goal;
            let step = if forward { speed } else { -speed } * PIXEL;
            let facing = match (along_y, forward) {
                (false, false) => Direction::Left,
                (false, true) => Direction::Right,
                (true, false) => Direction::Up,
                (true, true) => Direction::Down,
            };
            if let Some(actor) = field.actor_mut(actor) {
                actor.play(facing.index() + WALKING);
            }
            glide.standing = facing.index();
            glide.left = frames;
            glide.step = if along_y { (0, step) } else { (step, 0) };
        }
        if actor == 0 {
            field.glide_player(glide.step.0, glide.step.1, camera);
        } else if let Some(moving) = field.actor_mut(actor) {
            moving.move_by(glide.step.0, glide.step.1);
        }
        glide.left -= 1;
        if let Some(task) = self.tasks[slot].as_mut() {
            task.glide = Some(glide);
            task.wait = 1;
        }
        Flow::Yield
    }

    /// Holds the game while a map of `count` objects loads through the
    /// scene loader (see [`Self::hold_frames`]).
    fn hold_loading(&mut self, slot: usize, count: usize) -> Flow {
        self.hold_frames(slot, LOAD_FRAMES + u32::try_from(count).unwrap_or(u32::MAX))
    }

    /// Holds the game `frames` frames while a map loads. A handler the game
    /// calls directly loads within the entry that called it: a map's
    /// handler run by a warp delays the warp's fade in by the load instead,
    /// and one run within a task's warp holds the game as it loads.
    fn hold_frames(&mut self, slot: usize, frames: u32) -> Flow {
        if slot == IMMEDIATE {
            if let Some(Hold::FadeIn {
                delay,
                settle,
                slow,
                ..
            }) = self.hold
            {
                let delay = delay.saturating_add(u8::try_from(frames).unwrap_or(u8::MAX));
                self.hold = Some(Hold::FadeIn {
                    delay,
                    settle,
                    tasks_first: false,
                    slow,
                });
            } else if self.within.is_some() && self.hold.is_none() {
                self.hold = Some(Hold::Frozen(frames));
            }
            return Flow::Next;
        }
        self.advance(slot);
        self.hold = Some(Hold::Loading { frames, slot });
        Flow::Yield
    }

    /// Ops that decide where the task goes next.
    fn branch(&mut self, slot: usize, op: Op, host: &mut impl EventHost) -> Flow {
        if let Some((yes, then, otherwise)) = field_condition(op, host) {
            self.push(slot, if yes { then } else { otherwise }, 1);
            return Flow::Continue;
        }
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
            Op::IfCompanion {
                slot,
                then,
                otherwise,
            } => taken(companion(host, slot).is_some(), then, otherwise),
            Op::TakeCompanion { slot, zoid, then } => {
                let picked = u8::try_from(host.saved_vars()[1])
                    .ok()
                    .filter(|&index| usize::from(index) < COMPANION_RECORDS);
                if let Some(index) = picked {
                    host.set_state_byte(COMPANION_SLOTS + slot, index);
                    host.join_companion(index, zoid);
                }
                taken(picked.is_some(), then, &[])
            }
            Op::IfChest {
                kind,
                then,
                otherwise,
            } => taken(host.chest_kind() == Some(kind), then, otherwise),
            Op::IfZiDataHeld { then, otherwise } => taken(host.zi_data_held(), then, otherwise),
            Op::IfZoidOwned {
                zoid,
                then,
                otherwise,
            } => taken(host.zoid_owned(zoid), then, otherwise),
            Op::IfRegulation {
                game,
                then,
                otherwise,
            } => taken(host.meets_regulation(game), then, otherwise),
            Op::IfStateSet {
                at,
                then,
                otherwise,
            } => taken(host.state_byte(at) != 0, then, otherwise),
            Op::IfLost { then, otherwise } => taken(host.battle_lost(), then, otherwise),
            Op::IfBattlesWon {
                more_than,
                then,
                otherwise,
            } => taken(host.battles_won() > more_than, then, otherwise),
            Op::IfMusic {
                song,
                then,
                otherwise,
            } => taken(host.music_playing() == Some(song), then, otherwise),
            Op::IfTask {
                slot: other,
                then,
                otherwise,
            } => taken(self.task_running(other), then, otherwise),
            Op::Call(program) => program,
            Op::Loop(_) | Op::Repeat(..) | Op::Spawn(..) => return self.start(slot, op),
            _ => return Flow::Stop,
        };
        self.push(slot, program, 1);
        Flow::Continue
    }

    /// Starts the program of a loop, a repeat or a spawn.
    fn start(&mut self, slot: usize, op: Op) -> Flow {
        match op {
            Op::Loop(program) => {
                self.push(slot, program, FOREVER);
                Flow::Continue
            }
            Op::Repeat(0, _) => Flow::Next,
            Op::Repeat(times, program) => {
                self.push(slot, program, times);
                Flow::Continue
            }
            Op::Spawn(target, program) => {
                self.spawn(target, program);
                Flow::Next
            }
            _ => Flow::Stop,
        }
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

/// Whether the player's cell is within `columns` and `rows`, each an
/// inclusive range when given.
/// For a condition on the field (the player's place and step, a guard's
/// sight, the portal), whether it holds and the programs it picks between.
fn field_condition(
    op: Op,
    host: &mut impl EventHost,
) -> Option<(bool, &'static [Op], &'static [Op])> {
    match op {
        Op::IfPlayerSprite {
            x,
            y,
            then,
            otherwise,
        } => Some((sprite_within(host, x, y), then, otherwise)),
        Op::IfPlayer {
            columns,
            rows,
            then,
            otherwise,
        } => Some((player_within(host, columns, rows), then, otherwise)),
        Op::IfPlayerWalking { then, otherwise } => {
            let walking = host.field().is_some_and(|field| field.player().walking());
            Some((walking, then, otherwise))
        }
        Op::IfPlayerOn {
            bits,
            then,
            otherwise,
        } => Some((player_on(host, bits), then, otherwise)),
        Op::IfSeen {
            guards,
            then,
            otherwise,
        } => Some((guard_sees(host, guards), then, otherwise)),
        Op::IfPortal { then, otherwise } => Some((
            host.field().and_then(|field| field.portal()).is_some(),
            then,
            otherwise,
        )),
        _ => None,
    }
}

fn player_within(
    host: &mut impl EventHost,
    columns: Option<(usize, usize)>,
    rows: Option<(usize, usize)>,
) -> bool {
    host.field().is_some_and(|field| {
        let player = field.player();
        columns.is_none_or(|(first, last)| (first..=last).contains(&player.column))
            && rows.is_none_or(|(first, last)| (first..=last).contains(&player.row))
    })
}

/// `op` with the stand-ins for the portal, its cells and the exit's
/// arrival replaced by what they stand for now.
fn resolve(op: Op, host: &mut impl EventHost) -> Op {
    let op = resolve_near(op, host);
    let stand_in =
        |cell: (usize, usize)| [AT_THE_PORTAL, BELOW_THE_PORTAL, EXIT_ARRIVAL].contains(&cell);
    let needed = match op {
        Op::Walk { actor, to, .. } => actor == THE_PORTAL || stand_in(to),
        Op::Place(actor, cell) => actor == THE_PORTAL || stand_in(cell),
        Op::PlayOnce(actor, _)
        | Op::AwaitStepEnd(actor, _)
        | Op::AwaitAnimation(actor)
        | Op::Animate(actor, _) => actor == THE_PORTAL,
        _ => false,
    };
    if !needed {
        return op;
    }
    let arrival = host.exit_arrival();
    let portal = host.field().and_then(|field| field.portal());
    let actor = |actor: usize| match (actor, portal) {
        (THE_PORTAL, Some((index, _))) => index,
        _ => actor,
    };
    let cell = |cell: (usize, usize)| match (cell, portal, arrival) {
        (AT_THE_PORTAL, Some((_, at)), _) | (EXIT_ARRIVAL, _, Some(at)) => at,
        (BELOW_THE_PORTAL, Some((_, (column, row))), _) => (column, row + 1),
        _ => cell,
    };
    match op {
        Op::Walk {
            actor: walker,
            to,
            speed,
            shift,
            through,
        } => Op::Walk {
            actor: actor(walker),
            to: cell(to),
            speed,
            shift,
            through,
        },
        Op::Place(placed, at) => Op::Place(actor(placed), cell(at)),
        Op::PlayOnce(played, animation) => Op::PlayOnce(actor(played), animation),
        Op::AwaitStepEnd(played, step) => Op::AwaitStepEnd(actor(played), step),
        Op::AwaitAnimation(played) => Op::AwaitAnimation(actor(played)),
        Op::Animate(played, animation) => Op::Animate(actor(played), animation),
        other => other,
    }
}

/// `op` with the guard that saw the player and the cells near the player
/// replaced by what they stand for now.
fn resolve_near(op: Op, host: &mut impl EventHost) -> Op {
    let guard = host.seen_guard();
    let seen = |actor: usize| match (actor, guard) {
        (SEEN_GUARD, Some(guard)) => guard,
        _ => actor,
    };
    let near = |field: &Field, (dx, dy): (i32, i32)| {
        let player = field.player();
        let size = if field.zoid_map() {
            ZOID_CELL
        } else {
            ROOM_CELL
        };
        let cell = |at: usize, by: i32| i32::try_from(at).unwrap_or(0) + by;
        (cell(player.column, dx), cell(player.row, dy), size)
    };
    match op {
        Op::Face(actor, facing) => Op::Face(seen(actor), facing),
        Op::AwaitArrival(actor) => Op::AwaitArrival(seen(actor)),
        Op::FaceSeenGuard => {
            let facing = guard.and_then(|guard| {
                host.field()
                    .and_then(|field| field.actor(guard))
                    .map(|actor| actor.facing.opposite())
            });
            facing.map_or(Op::Wait(0), |facing| Op::Face(0, facing))
        }
        Op::PlaceOnActor(actor, other) => host
            .field()
            .and_then(|field| field.actor(other).map(|placed| (placed.column, placed.row)))
            .map_or(Op::Wait(0), |cell| Op::Place(actor, cell)),
        Op::PlaceNearPlayer(actor, by) => host.field().map_or(op, |field| {
            let (column, row, _) = near(field, by);
            let cell = |value: i32| usize::try_from(value).unwrap_or(usize::from(u8::MAX));
            Op::Place(actor, (cell(column), cell(row)))
        }),
        Op::GlideNearPlayer {
            actor,
            by,
            speed,
            camera,
        } => host.field().map_or(op, |field| {
            let (column, row, size) = near(field, by);
            Op::Glide {
                actor,
                to: (column * size, row * size),
                speed,
                frames: size.unsigned_abs() / speed.unsigned_abs().max(1),
                camera,
            }
        }),
        other => other,
    }
}

/// The cell sizes of rooms and of Zoid maps, in pixels.
const ROOM_CELL: i32 = 16;
const ZOID_CELL: i32 = 32;

/// Whether one of `guards` sees the player, that guard stopped and kept
/// as the one that saw it.
fn guard_sees(host: &mut impl EventHost, guards: &[usize]) -> bool {
    let seen = host.field().and_then(|field| {
        let guard = field.seen_by(guards)?;
        if let Some(actor) = field.actor_mut(guard) {
            actor.command = crate::field::Command::Idle;
        }
        Some(guard)
    });
    if let Some(guard) = seen {
        host.set_seen_guard(guard);
    }
    seen.is_some()
}

/// Whether the attribute of the player's cell has any of `bits`.
fn player_on(host: &mut impl EventHost, bits: u16) -> bool {
    host.field().is_some_and(|field| {
        let (column, row) = field.player().footing();
        field.scene().attribute(column, row).unwrap_or(0) & bits != 0
    })
}

/// Whether the player's sprite is within `x` and `y`, inclusive ranges of
/// map pixels of its box's top-left.
fn sprite_within(host: &mut impl EventHost, x: (i32, i32), y: (i32, i32)) -> bool {
    host.field().is_some_and(|field| {
        let (at_x, at_y) = field.player().fixed_position();
        (x.0..=x.1).contains(&at_x.div_euclid(PIXEL))
            && (y.0..=y.1).contains(&at_y.div_euclid(PIXEL))
    })
}

/// Applies an op that changes the game's state and returns at once.
fn apply(op: Op, host: &mut impl EventHost) {
    match op {
        Op::Flag(flag, set) => host.set_flag(flag, set),
        Op::Music(song) => host.play_music(song),
        Op::RestartMusic(song) => host.restart_music(song),
        Op::Sound(sound) => host.play_sound(sound),
        Op::Meet(group) => host.meet(group),
        Op::Join(list) => host.join(list),
        Op::Leave(list) => host.leave(list),
        Op::OfferCompanions => {
            for index in 0..COMPANION_RECORDS {
                let flag = u16::try_from(index).unwrap_or(u16::MAX);
                host.set_flag(flag, false);
                if host.companion_offered(index) {
                    host.set_flag(flag, true);
                }
            }
            host.set_flag(COMPANIONS_OFFERED, true);
        }
        Op::HideCompanion(slot) => {
            if let Some(index) = companion(host, slot) {
                host.set_flag(u16::from(index), false);
                for pair in COMPANION_FORMS {
                    if pair.contains(&index) {
                        for form in pair {
                            host.set_flag(u16::from(form), false);
                        }
                    }
                }
            }
        }
        Op::DropCompanion(slot) => {
            if let Some(index) = companion(host, slot) {
                host.leave_companion(index);
            }
            host.set_state_byte(COMPANION_SLOTS + slot, NO_COMPANION);
        }
        Op::ForgetCompanions => {
            for slot in 0..2 {
                host.set_state_byte(COMPANION_SLOTS + slot, NO_COMPANION);
            }
        }
        Op::LearnCommand(command) => host.learn_command(command),
        Op::FormParty(choice) => host.form_party(choice),
        Op::SeeZoid(id) => host.see_zoid(id),
        Op::OpenChest => host.open_chest(),
        Op::MarkChest => host.mark_chest(),
        Op::TakeChest => host.take_chest(),
        Op::Gift(reward) => host.set_gift(reward),
        Op::Calm(calm) => host.set_calm(calm),
        Op::SeaCrossing(crossing) => host.set_sea_crossing(crossing),
        Op::RedirectWarps(redirect) => host.redirect_warps(redirect),
        Op::RebuildObjects => host.rebuild_objects(),
        Op::SaveMusic => host.save_music(),
        Op::RestoreMusic => host.restore_music(),
        Op::Whiten(level) => host.set_whiten(level),
        Op::FadeToWhite(white) => host.set_fade_to_white(white),
        Op::Silence => host.silence(),
        Op::AfterCombat => host.after_combat(),
        Op::RestartMapMusic => host.restart_map_music(),
        Op::ForgetBattlesWon => host.forget_battles_won(),
        Op::KeepCell(actor) => {
            let kept = host
                .field()
                .and_then(|field| field.actor(actor))
                .and_then(|actor| actor.slot.map(|slot| (slot, (actor.column, actor.row))));
            if let Some((slot, cell)) = kept {
                host.keep_object_cell(slot, cell);
            }
        }
        Op::ReturnPoint(index) => host.set_return_point(index),
        Op::ExitSound => host.exit_sound(),
        _ => {}
    }
}

/// Applies an op that commands an actor.
fn command_actor(field: &mut Field, op: Op) {
    match op {
        Op::Once(_) | Op::Roam(..) => mark_actor(field, op),
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
        Op::Place(actor, cell) => field.place_actor(actor, cell),
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
        Op::RestartStep(..) | Op::Settle(_) => set_actor_in_place(field, op),
        Op::Pan(dx, dy) => field.pan_by(dx, dy),
        Op::StepBack { speed, shift } => {
            let by = field.player().facing.opposite().delta();
            step_player(field, by, speed, shift, true);
        }
        Op::StepPlayer { by, speed, shift } => step_player(field, by, speed, shift, false),
        Op::PlaceRandom(actor, cells) => {
            let draw = usize::from(field.random());
            if let Some(&cell) = cells.get(draw % cells.len().max(1)) {
                field.place_actor(actor, cell);
            }
        }
        Op::CellTiles(tiles) => field.set_cell_tiles(tiles),
        _ => {}
    }
}

/// Makes an actor's animation stop at its end, or sets it roaming or
/// standing.
fn mark_actor(field: &mut Field, op: Op) {
    match op {
        Op::Once(actor) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.once = true;
            }
        }
        Op::Roam(actor, roams) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.command = if roams {
                    Command::Wander
                } else {
                    Command::Idle
                };
            }
        }
        _ => {}
    }
}

/// Walks the player to the cell `by` cells from its own, `through` walls
/// and actors or not.
fn step_player(field: &mut Field, by: (isize, isize), speed: i32, shift: i8, through: bool) {
    let player = field.player_mut();
    player.command = Command::WalkTo(Walk {
        column: player.column.saturating_add_signed(by.0),
        row: player.row.saturating_add_signed(by.1),
        speed,
        animation_shift: shift,
        through,
    });
}

/// Where warps go instead while a map's second hook is set: those to a map
/// within `from` go to `to`, on `cell` when given, else on the warp's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarpRedirect {
    /// First and last map sent elsewhere.
    pub from: (usize, usize),
    /// The map they go to.
    pub to: usize,
    /// The cell they arrive on, when not the warp's.
    pub cell: Option<(usize, usize)>,
}

impl WarpRedirect {
    /// Where a warp to `cell` of `map` goes.
    #[must_use]
    pub fn apply(&self, map: usize, cell: (usize, usize)) -> (usize, (usize, usize)) {
        if (self.from.0..=self.from.1).contains(&map) {
            (self.to, self.cell.unwrap_or(cell))
        } else {
            (map, cell)
        }
    }
}

/// Whether `op` starts something the game holds for: a dialogue or
/// another table's string, a battle scene, a shop or a battle.
const fn starts_hold(op: &Op) -> bool {
    matches!(
        op,
        Op::Dialogue(_)
            | Op::DialogueCounting { .. }
            | Op::Script(..)
            | Op::ChestName
            | Op::Battle(_)
            | Op::Shop(_)
            | Op::RotatingArmsShop
            | Op::Combat
            | Op::StoryBattle(_)
            | Op::Credits
    )
}

/// Starts what `op` holds the game for, for the task in `slot`: a
/// dialogue or another table's script, a battle scene, a shop or a battle.
fn start_hold(slot: usize, op: Op, host: &mut impl EventHost) -> Hold {
    match op {
        Op::Script(table, index) => {
            host.start_script(table, index);
            Hold::Dialogue(slot)
        }
        Op::ChestName => {
            host.start_chest_name();
            Hold::Dialogue(slot)
        }
        Op::Battle(scene) => {
            host.start_battle(scene);
            Hold::Battle(slot)
        }
        Op::Credits => {
            host.start_credits();
            Hold::Battle(slot)
        }
        Op::Shop(shop) => {
            host.start_shop(shop);
            Hold::Shop(slot)
        }
        Op::RotatingArmsShop => {
            let shop = host.rotating_arms_shop();
            host.start_shop(Shop::Arms(shop));
            Hold::Shop(slot)
        }
        Op::Combat => {
            host.start_combat();
            Hold::Combat(slot)
        }
        Op::StoryBattle(battle) => {
            host.start_story_battle(battle);
            Hold::Combat(slot)
        }
        Op::Dialogue(index) => {
            host.start_dialogue(index, slot < IMMEDIATE);
            Hold::Dialogue(slot)
        }
        Op::DialogueCounting { index, flags } => {
            let left = flags.iter().filter(|&&flag| !host.flag(flag)).count();
            host.start_dialogue(index, slot < IMMEDIATE);
            host.set_dialogue_variable(0, u16::try_from(left).unwrap_or(u16::MAX));
            Hold::Dialogue(slot)
        }
        _ => Hold::Dialogue(slot),
    }
}

/// The ops that set an actor where it stands: its animation's step, or its
/// cell under its sprite.
fn set_actor_in_place(field: &mut Field, op: Op) {
    match op {
        Op::RestartStep(actor, step) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.restart_at_step(step);
            }
        }
        Op::Settle(actor) => {
            if let Some(actor) = field.actor_mut(actor) {
                actor.settle();
            }
        }
        _ => {}
    }
}

/// Whether `op` picks the program that runs next.
fn branches(op: &Op) -> bool {
    matches!(
        op,
        Op::IfFlags { .. }
            | Op::IfCommand { .. }
            | Op::IfArea { .. }
            | Op::IfChoice { .. }
            | Op::IfLost { .. }
            | Op::IfChest { .. }
            | Op::IfZiDataHeld { .. }
            | Op::IfZoidOwned { .. }
            | Op::IfRegulation { .. }
            | Op::IfSeen { .. }
            | Op::IfStateSet { .. }
            | Op::IfPlayer { .. }
            | Op::IfPlayerWalking { .. }
            | Op::IfPlayerSprite { .. }
            | Op::IfPlayerOn { .. }
            | Op::IfBattlesWon { .. }
            | Op::IfMusic { .. }
            | Op::IfTask { .. }
            | Op::IfPortal { .. }
            | Op::IfCompanion { .. }
            | Op::TakeCompanion { .. }
            | Op::Call(_)
            | Op::Loop(_)
            | Op::Repeat(..)
            | Op::Spawn(..)
            | Op::End
    )
}

/// The record companion slot `slot` holds, `None` when it is empty.
fn companion(host: &impl EventHost, slot: usize) -> Option<u8> {
    Some(host.state_byte(COMPANION_SLOTS + slot)).filter(|&index| index != NO_COMPANION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Host {
        flags: Vec<u16>,
        log: Vec<String>,
        music: Option<u16>,
        battles_won: u16,
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

        fn start_dialogue(&mut self, index: u16, _called: bool) {
            self.log.push(format!("dialogue {index}"));
        }

        fn set_dialogue_variable(&mut self, slot: u8, value: u16) {
            self.log.push(format!("variable {slot} = {value}"));
        }

        fn start_battle(&mut self, scene: u8) {
            self.log.push(format!("battle {scene}"));
        }

        fn start_credits(&mut self) {
            self.log.push("credits".to_owned());
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

        fn restart_music(&mut self, song: u16) {
            self.log.push(format!("restart music {song}"));
        }

        fn play_sound(&mut self, sound: u16) {
            self.log.push(format!("sound {sound}"));
        }

        fn load_map(&mut self, map: usize, _: (usize, usize), _: u32, _: usize) {
            self.log.push(format!("map {map}"));
        }

        fn load_arena(&mut self, game: u8) -> usize {
            self.log.push(format!("arena {game}"));
            1
        }

        fn meets_regulation(&self, game: u8) -> bool {
            game < 2
        }

        fn set_calm(&mut self, calm: bool) {
            self.log.push(format!("calm {calm}"));
        }

        fn set_sea_crossing(&mut self, crossing: bool) {
            self.log.push(format!("sea crossing {crossing}"));
        }

        fn redirect_warps(&mut self, redirect: WarpRedirect) {
            self.log.push(format!("redirect {redirect:?}"));
        }

        fn rebuild_objects(&mut self) {
            self.log.push("rebuild".to_owned());
        }

        fn save_music(&mut self) {
            self.log.push("save music".to_owned());
        }

        fn restore_music(&mut self) {
            self.log.push("restore music".to_owned());
        }

        fn set_whiten(&mut self, level: u8) {
            self.log.push(format!("whiten {level}"));
        }

        fn set_fade_to_white(&mut self, white: bool) {
            self.log.push(format!("fade to white {white}"));
        }

        fn silence(&mut self) {
            self.log.push("silence".to_owned());
        }

        fn seen_guard(&self) -> Option<usize> {
            None
        }

        fn set_seen_guard(&mut self, _guard: usize) {}

        fn meet(&mut self, group: u8) {
            self.log.push(format!("meet {group}"));
        }

        fn start_map_music(&mut self, map: usize) {
            self.log.push(format!("map music {map}"));
        }

        fn join(&mut self, list: u8) {
            self.log.push(format!("join {list}"));
        }

        fn leave(&mut self, list: u8) {
            self.log.push(format!("leave {list}"));
        }

        fn companion_offered(&self, index: usize) -> bool {
            index % 2 == 0
        }

        fn join_companion(&mut self, index: u8, zoid: bool) {
            self.log.push(format!("join companion {index} {zoid}"));
        }

        fn leave_companion(&mut self, index: u8) {
            self.log.push(format!("leave companion {index}"));
        }

        fn set_state_byte(&mut self, at: usize, value: u8) {
            self.log.push(format!("state {at:#x} = {value:#x}"));
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

        fn chest_kind(&self) -> Option<ChestKind> {
            None
        }

        fn zi_data_held(&self) -> bool {
            false
        }

        fn take_chest(&mut self) {
            self.log.push("chest".to_owned());
        }

        fn set_gift(&mut self, reward: Option<Reward>) {
            self.log.push(format!("gift {reward:?}"));
        }

        fn zoid_owned(&self, zoid: u16) -> bool {
            zoid == 0x90
        }

        fn state_byte(&self, _at: usize) -> u8 {
            0
        }

        fn start_chest_name(&mut self) {
            self.log.push("chest name".to_owned());
        }

        fn warp(&mut self, map: usize, _: (usize, usize), _: Option<Direction>) -> usize {
            self.log.push(format!("warp {map}"));
            1
        }

        fn start_story_battle(&mut self, battle: u8) {
            self.log.push(format!("story battle {battle}"));
        }

        fn battle_lost(&self) -> bool {
            false
        }

        fn return_point(&self) -> Option<(usize, (usize, usize))> {
            Some((20, (4, 4)))
        }

        fn set_sprite(&mut self, actor: usize, sprite: usize) {
            self.log.push(format!("sprite {actor} {sprite}"));
        }

        fn sound_ended(&self, _: u16) -> bool {
            true
        }

        fn music_playing(&self) -> Option<u16> {
            self.music
        }

        fn restart_map_music(&mut self) {
            self.log.push("restart map music".to_owned());
        }

        fn battles_won(&self) -> u16 {
            self.battles_won
        }

        fn forget_battles_won(&mut self) {
            self.battles_won = 0;
        }

        fn rotating_arms_shop(&self) -> u8 {
            1
        }

        fn keep_object_cell(&mut self, slot: usize, cell: (usize, usize)) {
            self.log.push(format!("keep {slot} {cell:?}"));
        }

        fn set_return_point(&mut self, index: u8) {
            self.log.push(format!("return point {index}"));
        }

        fn exit_sound(&mut self) {
            self.log.push("exit sound".to_owned());
        }

        fn take_exit(&mut self) -> usize {
            self.log.push("exit".to_owned());
            0
        }

        fn exit_arrival(&self) -> Option<(usize, usize)> {
            None
        }
    }

    const WAITING: &[Op] = &[Op::Music(1), Op::Wait(3), Op::Music(2), Op::End];

    const REPLACED: &[Op] = &[Op::Music(9), Op::End];
    const WARPING: &[Op] = &[
        Op::Warp {
            map: 17,
            cell: (4, 6),
            facing: None,
        },
        Op::Music(3),
        Op::Wait(60),
        Op::Music(4),
        Op::End,
    ];

    const WATCHED: &[Op] = &[Op::Wait(2), Op::Music(9), Op::End];
    const WATCH: &[Op] = &[Op::Loop(&[
        Op::IfTask {
            slot: MAP_TASK,
            then: &[],
            otherwise: &[Op::IfBattlesWon {
                more_than: 1,
                then: &[Op::ForgetBattlesWon, Op::Spawn(MAP_TASK, WATCHED)],
                otherwise: &[Op::IfMusic {
                    song: 5,
                    then: &[Op::RestartMapMusic, Op::End],
                    otherwise: &[],
                }],
            }],
        },
        Op::Wait(1),
    ])];

    const TALKER: &[Op] = &[Op::Dialogue(7), Op::Music(1), Op::End];
    const COUNTER: &[Op] = &[Op::Loop(&[Op::Sound(2), Op::Wait(1)])];

    #[test]
    fn the_field_watch_runs_only_on_its_own() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.spawn(FIELD_WATCH, TALKER);
        events.update(&mut host);
        assert!(host.log.is_empty());
        events.update_watch(&mut host);
        assert_eq!(host.log, ["dialogue 7"]);
    }

    #[test]
    fn a_tasks_dialogue_holds_only_that_task() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.spawn(MAP_TASK, TALKER);
        events.spawn(MAP_TASK + 1, COUNTER);
        events.update(&mut host);
        assert_eq!(host.log, ["dialogue 7", "sound 2"]);
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
        assert_eq!(host.log, ["dialogue 7", "sound 2"]);
        assert_eq!(events.update_hold(false, &mut host), HoldStep::Held);
        assert_eq!(host.log, ["dialogue 7", "sound 2", "sound 2"]);
        events.update_hold(true, &mut host);
        assert_eq!(
            host.log,
            ["dialogue 7", "sound 2", "sound 2", "music 1", "sound 2"]
        );
    }

    #[test]
    fn a_loop_runs_every_frame_and_skips_while_its_helper_runs() {
        let mut events = Events::new();
        let mut host = Host {
            battles_won: 2,
            ..Host::default()
        };
        events.spawn(FIELD_HOOK, WATCH);
        events.update(&mut host);
        assert_eq!(host.battles_won, 0);
        assert!(events.task_running(MAP_TASK));
        for _ in 0..3 {
            events.update(&mut host);
        }
        assert_eq!(host.log, ["music 9"]);
        assert!(events.task_running(FIELD_HOOK));
        host.music = Some(5);
        events.update(&mut host);
        assert_eq!(host.log, ["music 9", "restart map music"]);
        assert!(!events.task_running(FIELD_HOOK));
    }

    #[test]
    fn a_task_spawned_into_a_loading_tasks_slot_is_lost() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.spawn(MAP_TASK, WARPING);
        events.update(&mut host);
        events.spawn(MAP_TASK, REPLACED);
        while events.update_hold(false, &mut host) == HoldStep::Held {}
        events.update(&mut host);
        for _ in 0..60 {
            events.update(&mut host);
        }
        assert_eq!(host.log, ["warp 17", "music 3", "music 4"]);
    }

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

    const CALMING: &[Op] = &[
        Op::Calm(true),
        Op::Whiten(3),
        Op::Repeat(0, &[Op::Sound(1)]),
        Op::Calm(false),
        Op::End,
    ];

    #[test]
    fn calm_and_whitening_reach_the_host_and_an_empty_repeat_runs_nothing() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.spawn(MAP_TASK, CALMING);
        events.update(&mut host);
        assert_eq!(host.log, ["calm true", "whiten 3", "calm false"]);
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

    const SEARCHING: &[Op] = &[Op::Sound(0x48), Op::Wait(2), Op::Sound(1)];

    #[test]
    fn a_handler_runs_until_its_last_wait_ends() {
        let mut events = Events::new();
        let mut host = Host::default();
        events.run_now(SEARCHING, &mut host);
        assert!(events.runs_handler(SEARCHING));
        assert!(!events.runs_handler(SHOPPING));
        events.update(&mut host);
        assert!(events.runs_handler(SEARCHING));
        events.update(&mut host);
        assert!(!events.runs_handler(SEARCHING));
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
        assert_eq!(host.log, ["map 24", "map music 24"]);
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
