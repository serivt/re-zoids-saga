//! The attack scenes (`0x08042348`, task slot 5, with the scenery's task
//! `0x08043FF8` in slot 6): the attacker's Zoid rolls in over the battle
//! scenery, its pilot speaks and it fires; then each target's view shows
//! the shots landing and its pilot's reaction.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! scene task's states (`0x08042348`), the scene records it builds from
//! the units (`0x08041AA8`, `0x080418B0`, `0x080419CC`, `0x08041A54`),
//! a view's setup (`0x08043EAC`, `0x08043D6C`, `0x08043DE8`, `0x08044E98`,
//! `0x08044EE4`, `0x0804588C`), the fades (`0x08042100`, `0x08042128`,
//! `0x08042078`, `0x080420A0`), the slide (`0x08044FA0`), the quotes
//! (`0x080421FC`, `0x08042218`), the scenery's task and its per-scenery
//! line routines (`0x08043FF8`, `0x0804418C`, `0x08044A8C` and the shakes
//! after it); checked frame by frame against a battle on the world map in
//! a reference emulator (the task states, the scroll shadows, the
//! per-scanline table, the brightness and blend registers and the entity
//! table). See `docs/combat.md`.
//!
//! The original loads a view's graphics in one go, and the frames that
//! takes pass without the main loop: the port counts them as the trace
//! showed them for the views it measured.

use std::collections::VecDeque;

use extraction::saga_battle::{self, BattleImage, FirePart};
use formats::tile::TILE_PIXELS;
use gba_runtime::ppu::{FullPalette, Palette, darken};
use platform::{Button, Frame, Input, Rgb};

use super::aim::{Aim, AimMemory, AimOutcome, AimSetup};
use super::effects::{self, Entities, Request, ShotPlace, Shown, View};
use super::units::{BattleUnit, NO_PART};
use crate::battle::{piece_matrix, piece_pixel};
use crate::data::GameData;
use crate::script::{ScriptError, ScriptRunner};
use crate::translation::BATTLE_TABLE;
use crate::windows::ScriptWindows;
use crate::{ScriptHost, TextPainter, WindowPainter};

const WIDTH: usize = 240;
/// The part id in a weapon's part, without the back rack's bit.
const PART_ID: u16 = 0x0FFF;
/// Weapon flags for the attacker's own side (`2`) or itself (`4`).
const OWN_SIDE_WEAPON: u32 = 6;
const HEIGHT: usize = 160;
const TILE: usize = 8;
const IMAGE_TILES_SIDE: usize = 16;
const ZOID_MAP_WIDTH: usize = 512;
const ZOID_MAP_HEIGHT: usize = 256;
const SCENERY_MAP_WIDTH: usize = 256;
const SCENERY_MAP_HEIGHT: usize = 256;
const ZOID_COLORS: usize = 0;
const SCENERY_COLORS: usize = 64;
/// Lines of the scroll table the scenery's routines move; the rest follow
/// the layer's own scroll (`0x08044A8C`).
const MOVING_LINES: usize = 0x90;
/// The screen line the scroll table's rewrite reaches the drawing at.
const TABLE_WRITE_LINE: usize = 16;
/// Where the Zoid starts its slide, off the screen's edge (`0x08044FA0`).
const SLIDE_FROM: i32 = -0xB0_0000;
const SLIDE_FIRST_STEP: i32 = 0x2000;
const SLIDE_ACCELERATION: i32 = 0x4000;
/// How far left the party's Zoid sits (`0x08044A8C`).
const PARTY_ZOID_OFFSET: i32 = 0x70_0000;
const BLACK: u8 = 16;
const FADE_STEP: u8 = 2;
const EFFECT_ALPHA: (u8, u8) = (15, 8);
/// Frames the attacker waits before speaking: 90 in the story's battles
/// (`0x08042810`).
const PAUSE_FRAMES: u32 = 30;
const STORY_PAUSE_FRAMES: u32 = 90;
/// Frames a view holds once its shots are over (`[sp]`), and the frames A
/// must be held to skip ahead (`[sp + 4]`).
const HOLD_FRAMES: u16 = 30;
const SKIP_HOLD: u16 = 60;
const HITS_SKIP_HOLD: u16 = 35;
const REACTION_HOLD: u16 = 50;
const STORY_REACTION_HOLD: u16 = 180;
const SKIPPED_REACTION_HOLD: u16 = 10;
// Scripts of the `system` table (`0x0803E4E8`).
const VIEW_WINDOWS: usize = 0xB;
const CLEAR_MESSAGE: usize = 0x12;
const MESSAGE_WINDOW: u8 = 1;
/// Where a pilot's reactions start in the `battle` table (ROM
/// `0x755E88`): its attack lines are `battle` string `pilot`.
const REACTIONS: usize = 86;
const QUOTE_DESTROYED: u16 = 3;
const QUOTE_MISSED: u16 = 4;
const QUOTE_VARIANTS: u16 = 3;
const QUOTE_VARIABLE: usize = 3;
const PILOT_VARIABLE: usize = 6;
/// The scenery kind of the sky, whose Zoids bob and whose shots have
/// their own animations.
const SKY: u8 = 7;
/// The frames after a view's setup until it fades in, as the trace
/// showed them: `true` for a frame the scenery's task runs, `false` for
/// one the loads and the text system's reset keep the main loop from.
const ATTACKER_LOAD: [bool; 14] = [
    false, false, true, false, false, false, false, false, false, true, true, true, true, true,
];
const TARGET_LOAD: [bool; 13] = [
    false, true, false, false, false, false, false, false, true, true, true, true, true,
];
/// The tiles' bytes a view's shots may load within their frame.
const SPAWN_LAG_BYTES: usize = 0x1000;
/// The frame the aim's sprites' load costs after the one it starts in.
const AIM_LOAD_LAG: u32 = 1;
/// Frames the windows' script may take at most.
const WINDOWS_SCRIPT_LIMIT: u32 = 64;

/// A unit as the scenes know it (the 28-byte records at EWRAM
/// `0x0200D920`, `0x080418B0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneUnit {
    /// Its Zoid.
    pub zoid: u16,
    /// Its size class, which picks the scenery's zoom (`+2`).
    pub size: u8,
    /// The scenery kind its ground calls for (`+3`).
    pub kind: u8,
    /// The parts in its slots, 0 for none or one without a place on the
    /// Zoid's picture (`+4`).
    pub parts: [u16; 6],
    /// Its pilot (`+0x15`): the portrait and the lines.
    pub pilot: u8,
    /// On the enemy's side.
    pub enemy: bool,
    /// Its slot.
    pub slot: usize,
}

impl SceneUnit {
    /// The scene record of `unit`, standing in `slot` of its side on
    /// terrain `terrain` (`0x08041AA8`).
    #[must_use]
    pub fn new(rom: &[u8], unit: &BattleUnit, enemy: bool, slot: usize, terrain: u8) -> Self {
        let parts = std::array::from_fn(|rack| {
            let part = unit.parts[rack];
            let placed =
                saga_battle::weapon_mount_raw(rom, unit.zoid, rack).is_some_and(|y| y != 0);
            if part == NO_PART || !placed { 0 } else { part }
        });
        let size = if unit.size > 2 { 0 } else { unit.size };
        Self {
            zoid: unit.zoid,
            size,
            kind: scenery_kind(terrain, unit.traits & FLYING != 0),
            parts,
            pilot: unit.face,
            enemy,
            slot,
        }
    }

    fn scenery(&self) -> u8 {
        self.kind * 3 + self.size
    }
}

const FLYING: u16 = 2;

/// The scenery kind terrain `terrain` calls for (`0x080419CC`); a flying
/// unit takes the sky's but over the terrains it keeps (`0x08041A54`).
#[must_use]
pub fn scenery_kind(terrain: u8, flying: bool) -> u8 {
    let kind = match terrain {
        0 => 1,
        1 => 5,
        2 => 8,
        3 => 4,
        4 => 2,
        5 => 12,
        6 => 3,
        7 => 10,
        8 => 6,
        9 => 13,
        10 => 16,
        11 => 11,
        12 => 14,
        13 => 15,
        other => other,
    };
    if flying && !matches!(kind, 8 | 10 | 11 | 15 | 16) {
        SKY
    } else {
        kind
    }
}

/// What an attack did to one target (`0x0200EB84 + 0x220C`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hit {
    /// It landed.
    pub landed: bool,
    /// A critical hit.
    pub critical: bool,
    /// The target was destroyed.
    pub destroyed: bool,
}

/// An attack to stage.
#[derive(Debug, Clone)]
pub struct Attack {
    /// The attacker.
    pub attacker: SceneUnit,
    /// The weapon's slot.
    pub weapon: usize,
    /// The targets, in the order their views show.
    pub targets: Vec<SceneUnit>,
    /// A story battle's: longer pauses, no skipping.
    pub story: bool,
    /// The turn's first roll (`0x020143BC`), which picks the lines.
    pub roll: u16,
    /// For the party's attack outside the story's battles, what the
    /// player's aim chooses from; the weapon and the targets come from it.
    pub aim: Option<AimSetup>,
}

/// What a scene asks of the battle as it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneEvent {
    /// The player chose the weapon in slot `weapon` and the targets'
    /// slots on side `side`.
    Aimed {
        /// The weapon's slot.
        weapon: usize,
        /// The side aimed at.
        side: usize,
        /// The targets, in order.
        targets: Vec<usize>,
    },
    /// The weapon the unit starts on the next time it aims.
    Remember(AimMemory),
    /// The player went back to the battle's menu; the scene has ended
    /// (state `0x2010`).
    Cancelled,
    /// The attacker has spoken: the attack applies now (`0x08046918`), and
    /// the battle answers with [`AttackScene::set_hits`].
    Apply,
    /// The scene has ended and handed the screen back.
    Done,
}

/// The task's state (`0x08042348`), by the original's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// State 0: the scene's records and screen.
    Init(u32),
    /// 0x1050 and 0x1060: a view's setup.
    Setup(View),
    /// The view's load, then its windows' script.
    Loading(View, usize),
    /// 0x1000 and 0x1002: the fade in, then `next`.
    FadeIn(Next),
    FadingIn(Next),
    /// 0x1051 and 0x1061: the Zoid slides in.
    Slide(View, u32),
    /// 0x1052: the player's aim starts; 0x1054 waits for it.
    AimStart,
    Aiming,
    /// 0x1010: the pause before the quote.
    Pause(u32),
    ClearMessage,
    Quote,
    /// 0x1070 and 0x1090: the shots play.
    Shots,
    /// 0x10A0: the target's pilot reacts.
    Reaction,
    Reacting,
    /// 0x1005: the view holds.
    Hold,
    /// 0x1001 and 0x1003: the fade out, then the next target.
    FadeOut,
    FadingOut,
    /// 0x10B0: the next target's view, or the end.
    NextTarget,
    /// 0x2000.
    End,
    Done,
}

/// Where a fade in goes on to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    Slide(View),
}

/// The graphics of the view shown.
struct Stage {
    unit: SceneUnit,
    scenery: Option<BattleImage>,
    zoid: Option<BattleImage>,
    palette: FullPalette,
    fire: Option<FirePart>,
}

/// An attack scene being shown.
pub struct AttackScene {
    attack: Attack,
    targets: VecDeque<SceneUnit>,
    hits: Vec<Hit>,
    stage: Option<Stage>,
    step: Step,
    lag: u32,
    entities: Entities,
    view: View,
    oam: Vec<Shown>,
    shown: Vec<Shown>,
    // The scenery's task and the registers it keeps.
    scenery_task: bool,
    enemy_view: bool,
    kind: u8,
    scenery: u8,
    counter: u8,
    phase: u8,
    bg2_x: i32,
    lines: [i32; HEIGHT],
    table: [i32; HEIGHT],
    shown_table: [i32; HEIGHT],
    previous_table: [i32; HEIGHT],
    zoid_x: i32,
    shake_offset: i32,
    shake: Option<(u8, usize)>,
    bg1: (i32, i32),
    shown_bg1: (i32, i32),
    brightness: u8,
    shown_brightness: u8,
    fade: u8,
    alpha: (u8, u8),
    // The task's locals.
    hold: u16,
    skip: u16,
    // Scripts.
    system: ScriptRunner,
    battle: ScriptRunner,
    sounds: Vec<u16>,
    events: Vec<SceneEvent>,
    input: Input,
    /// The player's aim (task slot 8), while it runs.
    aim: Option<Aim>,
    /// The aim was given up: the fade out ends the scene.
    cancelled: bool,
    /// Frames the scene's task and the scenery's lose while the aim runs
    /// past the frame.
    stall: u8,
}

impl AttackScene {
    /// A scene for `attack`, started in the frame the battle screen has
    /// gone black.
    #[must_use]
    pub fn new(data: &GameData<'_>, attack: Attack) -> Self {
        let offsets = |table: &str| {
            data.script_offsets(table)
                .ok()
                .flatten()
                .unwrap_or_default()
        };
        let system = saga_battle_system_offsets(data.bytes());
        let targets = attack.targets.iter().copied().collect();
        Self {
            attack,
            targets,
            hits: Vec::new(),
            stage: None,
            step: Step::Init(0),
            lag: 0,
            entities: Entities::new(),
            view: View::Attacker,
            oam: Vec::new(),
            shown: Vec::new(),
            scenery_task: false,
            enemy_view: false,
            kind: 0,
            scenery: 0,
            counter: 0,
            phase: 0,
            bg2_x: 0,
            lines: [0; HEIGHT],
            table: [0; HEIGHT],
            shown_table: [0; HEIGHT],
            previous_table: [0; HEIGHT],
            zoid_x: 0,
            shake_offset: 0,
            shake: None,
            bg1: (0, 0),
            shown_bg1: (0, 0),
            brightness: BLACK,
            shown_brightness: BLACK,
            fade: 0,
            alpha: EFFECT_ALPHA,
            hold: 0,
            skip: 0,
            system: ScriptRunner::named("system", system),
            battle: ScriptRunner::named(BATTLE_TABLE, offsets(BATTLE_TABLE)),
            sounds: Vec::new(),
            events: Vec::new(),
            input: Input::default(),
            aim: None,
            cancelled: false,
            stall: 0,
        }
    }

    /// What each target took, once the attack has applied.
    pub fn set_hits(&mut self, hits: Vec<Hit>) {
        self.hits = hits;
    }

    /// The events since the last call.
    pub fn take_events(&mut self) -> Vec<SceneEvent> {
        std::mem::take(&mut self.events)
    }

    /// The sound effects requested since the last call.
    pub fn take_sounds(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.sounds)
    }

    /// The task's state by the original's numbers (`0x08042348`), for
    /// comparing runs.
    #[must_use]
    pub fn state(&self) -> u16 {
        match self.step {
            Step::Init(_) => 0,
            Step::Setup(View::Attacker) | Step::Loading(View::Attacker, _) => 0x1050,
            Step::Setup(View::Target) | Step::Loading(View::Target, _) => 0x1060,
            Step::FadeIn(_) => 0x1000,
            Step::FadingIn(_) => 0x1002,
            Step::Slide(View::Attacker, _) => 0x1051,
            Step::Slide(View::Target, _) => 0x1061,
            Step::AimStart => 0x1052,
            Step::Aiming => 0x1054,
            Step::Pause(_) | Step::ClearMessage | Step::Quote => 0x1010,
            Step::Shots => match self.view {
                View::Attacker => 0x1070,
                View::Target => 0x1090,
            },
            Step::Reaction | Step::Reacting => 0x10A0,
            Step::Hold => 0x1005,
            Step::FadeOut => 0x1001,
            Step::FadingOut => 0x1003,
            Step::NextTarget => 0x10B0,
            Step::End if self.cancelled => 0x2010,
            Step::End => 0x2000,
            Step::Done => 0x2001,
        }
    }

    /// The aim's state, while it runs.
    #[must_use]
    pub fn aim_state(&self) -> Option<u16> {
        self.aim.as_ref().map(Aim::state)
    }

    /// Whether the scene has ended.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.step == Step::Done
    }

    /// Keeps what the vertical blank copies: the layers' scroll, the
    /// brightness and the sprites built the frame before.
    pub fn latch(&mut self) {
        self.shown_bg1 = self.bg1;
        self.shown_brightness = self.brightness;
        self.shown.clone_from(&self.oam);
    }

    /// Advances one frame: the scene's task, the scenery's, then the sprite
    /// system's pass; a frame the view's load takes runs none of them.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] when a script cannot run.
    pub fn update(
        &mut self,
        data: &GameData<'_>,
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        self.input = input;
        self.previous_table = self.table;
        if self.lag > 0 {
            self.lag -= 1;
            self.shown_table = self.table;
            return Ok(());
        }
        let stalled = self.stall > 0;
        self.stall = self.stall.saturating_sub(1);
        let scenery_runs = !stalled && self.step_task(data, windows)?;
        for request in self.entities.take_requests() {
            match request {
                Request::Sound(sound) => self.sounds.push(u16::from(sound)),
                Request::Shake(kind) => self.shake = Some((kind, 0)),
                Request::Blend(eva, evb) => self.alpha = (eva, evb),
            }
        }
        if self.scenery_task && scenery_runs {
            self.update_scenery(data.bytes());
        }
        if let Some(aim) = self.aim.as_mut() {
            aim.update(
                data.bytes(),
                self.input,
                &mut self.system,
                windows,
                &mut self.entities,
            )?;
            self.sounds.extend(aim.take_sounds());
            self.stall = self.stall.max(aim.take_stall());
            if let Some(memory) = aim.take_memory() {
                self.events.push(SceneEvent::Remember(memory));
            }
        }
        if !stalled {
            self.entities.step_animations();
        }
        self.oam = self.entities.shown((self.bg1.0 >> 16, self.bg1.1 >> 16));
        self.shown_table = self.table;
        Ok(())
    }

    fn held_a(&self) -> bool {
        self.input.is_held(Button::A)
    }

    /// One step of the scene's task; `false` when the frame belongs to a
    /// view's load and the scenery's task does not run either.
    fn step_task(
        &mut self,
        data: &GameData<'_>,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<bool, ScriptError> {
        let rom = data.bytes();
        match self.step {
            Step::Init(frame) => {
                // 0x08041DB4: the text system's reset waits a frame, then
                // the scenery's task starts and runs from the next.
                if frame == 1 {
                    windows.close_window(None);
                    self.scenery_task = true;
                    self.step = Step::Init(frame + 1);
                    return Ok(false);
                }
                self.step = if frame >= 3 {
                    Step::Setup(View::Attacker)
                } else {
                    Step::Init(frame + 1)
                };
            }
            Step::Setup(view) => {
                self.set_up(data, view);
                self.open_windows(rom, windows)?;
                self.step = Step::Loading(view, 0);
                return Ok(false);
            }
            Step::Loading(view, frame) => {
                let load: &[bool] = match view {
                    View::Attacker => &ATTACKER_LOAD,
                    View::Target => &TARGET_LOAD,
                };
                let runs = load.get(frame).copied().unwrap_or(true);
                self.step = if frame + 1 >= load.len() {
                    Step::FadeIn(Next::Slide(view))
                } else {
                    Step::Loading(view, frame + 1)
                };
                return Ok(runs);
            }
            Step::FadeIn(next) => {
                // 0x08042100.
                self.brightness = BLACK;
                self.fade = 0;
                self.update_shots();
                self.step = Step::FadingIn(next);
            }
            Step::FadingIn(next) => {
                // 0x08042128.
                self.brightness = BLACK - self.fade;
                self.fade += FADE_STEP;
                self.update_shots();
                if self.fade > BLACK {
                    self.step = match next {
                        Next::Slide(view) => Step::Slide(view, 0),
                    };
                }
            }
            Step::Slide(view, t) => self.slide(rom, view, t),
            Step::AimStart => self.start_aim(data),
            Step::Aiming => self.await_aim(rom),
            Step::Pause(left) => {
                if left > 0 {
                    self.step = Step::Pause(left - 1);
                } else {
                    self.hold = HOLD_FRAMES;
                    self.skip = SKIP_HOLD;
                    self.system.start(CLEAR_MESSAGE)?;
                    self.system.update(rom, self.input, windows)?;
                    self.step = Step::ClearMessage;
                }
            }
            Step::ClearMessage => {
                if self.system.update(rom, self.input, windows)? {
                    self.start_quote(rom, windows)?;
                }
            }
            Step::Quote => {
                if self.battle.update(rom, self.input, windows)? {
                    self.after_quote(rom);
                }
            }
            Step::Shots
            | Step::Reaction
            | Step::Reacting
            | Step::Hold
            | Step::FadeOut
            | Step::FadingOut => self.step_view(rom, windows)?,
            Step::NextTarget => {
                self.step = if self.targets.is_empty() || self.own_side(rom) {
                    Step::End
                } else {
                    Step::Setup(View::Target)
                };
            }
            Step::End => self.end(windows),
            Step::Done => {}
        }
        Ok(true)
    }

    /// State `0x2000`, or `0x2010` for an aim given up (`0x08041EB0`): the
    /// scene hands the screen back.
    fn end(&mut self, windows: &mut ScriptWindows<'_>) {
        windows.close_window(None);
        self.scenery_task = false;
        self.brightness = BLACK;
        self.events.push(if self.cancelled {
            SceneEvent::Cancelled
        } else {
            SceneEvent::Done
        });
        self.step = Step::Done;
    }

    /// The steps of a view once its Zoid has arrived: the shots, the
    /// target's reaction, the hold and the fade out.
    fn step_view(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        match self.step {
            Step::Shots => {
                self.update_shots();
                let mut alive = self.entities.shots_alive();
                if self.held_a() {
                    if self.skip > 0 {
                        self.skip -= 1;
                    } else if !self.attack.story {
                        alive = false;
                    }
                }
                if !alive {
                    self.step = match self.view {
                        View::Attacker => Step::Hold,
                        View::Target => Step::Reaction,
                    };
                }
            }
            Step::Reaction => {
                // 0x08042C40.
                self.update_shots();
                self.entities.pause_shots(true);
                self.start_reaction()?;
                self.step = Step::Reacting;
                if self.battle.update(rom, self.input, windows)? {
                    self.after_reaction();
                }
            }
            Step::Reacting => {
                if self.battle.update(rom, self.input, windows)? {
                    self.after_reaction();
                }
            }
            Step::Hold => {
                // 0x08042F88.
                if self.hold == 0 {
                    self.step = Step::FadeOut;
                } else {
                    self.hold -= 1;
                    if self.held_a() {
                        if self.skip > 0 {
                            self.skip -= 1;
                        } else {
                            self.hold = 0;
                        }
                    }
                }
                self.update_shots();
            }
            Step::FadeOut => {
                // 0x08042078.
                self.brightness = 0;
                self.fade = 0;
                self.update_shots();
                self.step = Step::FadingOut;
            }
            Step::FadingOut => {
                // 0x080420A0.
                self.alpha = if self.fade == 0 {
                    EFFECT_ALPHA
                } else {
                    (
                        15 - ((self.fade * 15) >> 4),
                        8u8.saturating_sub(self.fade >> 1),
                    )
                };
                self.brightness = self.fade;
                self.fade += FADE_STEP;
                self.update_shots();
                if self.fade > BLACK {
                    self.step = if self.cancelled {
                        Step::End
                    } else {
                        Step::NextTarget
                    };
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Runs the shot entities' behaviors, as every state that waits on
    /// them does.
    fn update_shots(&mut self) {
        self.entities.update_shots(self.enemy_view);
    }

    /// Sets up a view (`0x08042570`, `0x08042B4C`): the scenery its unit
    /// stands on, its Zoid off the screen's edge, its mounted weapons.
    fn set_up(&mut self, data: &GameData<'_>, view: View) {
        let rom = data.bytes();
        self.view = view;
        let unit = match view {
            View::Attacker => self.attack.attacker,
            View::Target => self.targets.pop_front().unwrap_or(self.attack.attacker),
        };
        self.enemy_view = unit.enemy;
        self.kind = unit.kind;
        self.scenery = unit.scenery();
        self.lines = [0; HEIGHT];
        self.table = [0; HEIGHT];
        self.brightness = BLACK;
        self.shake_offset = 0;
        self.shake = None;
        self.zoid_x = SLIDE_FROM;
        self.bg1.1 = 0;
        self.entities.clear();
        let scenery = data.scenery_image(self.scenery);
        let zoid = u8::try_from(unit.zoid)
            .ok()
            .and_then(|zoid| data.zoid_image(zoid));
        let mut palette = FullPalette::from_bgr555(&[0]);
        if let Some(zoid) = &zoid {
            palette.write(ZOID_COLORS, &zoid.palette);
        }
        if let Some(scenery) = &scenery {
            palette.write(SCENERY_COLORS, &scenery.palette);
        }
        for rack in 0..effects::MOUNTED_RACKS {
            let part = unit.parts[rack];
            if part == 0 {
                continue;
            }
            let (Some(sprite), Some(place)) = (
                saga_battle::weapon_sprite(rom, part, rack),
                saga_battle::weapon_mount(rom, unit.zoid, rack),
            ) else {
                continue;
            };
            self.entities.mount(rack, sprite, place, unit.enemy);
        }
        let part = self.weapon_part(&self.attack.attacker);
        self.stage = Some(Stage {
            unit,
            scenery,
            zoid,
            palette,
            fire: saga_battle::fire_part(rom, part & 0x0FFF),
        });
        self.battle_vars(unit.pilot);
    }

    /// The part of the weapon the attacker fires, as the scene lists it
    /// (`0x0200E23C`): 0 for one without a place on the picture, with
    /// `0x1000` for the back rack's.
    /// Whether the weapon is for the attacker's own side (bits 1 and 2): the
    /// scene then shows no target's view (`0x0200E24C`).
    fn own_side(&self, rom: &[u8]) -> bool {
        let part = self.weapon_part(&self.attack.attacker) & PART_ID;
        extraction::saga_party::part_record(rom, part)
            .is_some_and(|record| record.flags & OWN_SIDE_WEAPON != 0)
    }

    fn weapon_part(&self, attacker: &SceneUnit) -> u16 {
        let part = attacker.parts.get(self.attack.weapon).copied().unwrap_or(0);
        if self.attack.weapon == effects::BACK_RACK && part != 0 {
            part | 0x1000
        } else {
            part
        }
    }

    fn battle_vars(&mut self, pilot: u8) {
        let mut vars = [0; 8];
        vars[PILOT_VARIABLE] = u16::from(pilot);
        self.system.set_vars(vars);
    }

    /// The view's windows: the pilot's portrait and the message window
    /// (`system` script 0xB). They open while the screen is black, within
    /// the view's load.
    fn open_windows(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let vars = *self.system.vars();
        self.system.start(VIEW_WINDOWS)?;
        self.system.set_vars(vars);
        for _ in 0..WINDOWS_SCRIPT_LIMIT {
            if self.system.update(rom, Input::default(), windows)? {
                break;
            }
        }
        Ok(())
    }

    /// The attacker's line (`0x08042884`): one of its pilot's three,
    /// picked by the turn's first roll.
    fn start_quote(
        &mut self,
        rom: &[u8],
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let pilot = self.attack.attacker.pilot;
        let variant = (self.attack.roll >> 1) % QUOTE_VARIANTS;
        self.battle.select_window(MESSAGE_WINDOW);
        self.battle.start(usize::from(pilot))?;
        let mut vars = [0; 8];
        vars[QUOTE_VARIABLE] = variant;
        vars[PILOT_VARIABLE] = u16::from(pilot);
        self.battle.set_vars(vars);
        self.step = Step::Quote;
        if self.battle.update(rom, self.input, windows)? {
            self.after_quote(rom);
        }
        Ok(())
    }

    /// Once the attacker has spoken (`0x0804292A`): the attack applies and
    /// the shots spawn.
    fn after_quote(&mut self, rom: &[u8]) {
        self.events.push(SceneEvent::Apply);
        self.fire_view(rom, View::Attacker);
        self.step = Step::Shots;
    }

    /// Once the target's pilot has spoken: the shots go on and the view
    /// holds (`0x08042D12`).
    fn after_reaction(&mut self) {
        self.entities.pause_shots(false);
        self.update_shots();
        self.hold = if self.attack.story {
            STORY_REACTION_HOLD
        } else if self.skip == 0 {
            SKIPPED_REACTION_HOLD
        } else {
            REACTION_HOLD
        };
        self.step = Step::Hold;
    }

    /// The target's reaction (`0x08042C8C`): its line for a miss, for its
    /// destruction, or one of three.
    fn start_reaction(&mut self) -> Result<(), ScriptError> {
        let Some(stage) = &self.stage else {
            return Ok(());
        };
        let pilot = stage.unit.pilot;
        let hit = self.hit_of(&stage.unit);
        let variant = if !hit.landed {
            QUOTE_MISSED
        } else if hit.destroyed {
            QUOTE_DESTROYED
        } else {
            (self.attack.roll >> 1) % QUOTE_VARIANTS
        };
        self.battle.select_window(MESSAGE_WINDOW);
        self.battle.start(REACTIONS + usize::from(pilot))?;
        let mut vars = [0; 8];
        vars[QUOTE_VARIABLE] = variant;
        vars[PILOT_VARIABLE] = u16::from(pilot);
        self.battle.set_vars(vars);
        Ok(())
    }

    fn hit_of(&self, unit: &SceneUnit) -> Hit {
        self.attack
            .targets
            .iter()
            .position(|target| target == unit)
            .and_then(|index| self.hits.get(index).copied())
            .unwrap_or_default()
    }

    /// State `0x1052` (`0x080426D4`): the aim's sprites and its task.
    fn start_aim(&mut self, data: &GameData<'_>) {
        if let Some(setup) = self.attack.aim.clone() {
            self.aim = Some(Aim::new(data, setup, &mut self.entities));
        }
        self.step = Step::Aiming;
        self.lag = AIM_LOAD_LAG;
    }

    /// State `0x1054` (`0x08042750`): once the aim has ended, the attacker
    /// speaks and fires, or the scene fades out for the battle's menu.
    fn await_aim(&mut self, rom: &[u8]) {
        let Some(outcome) = self.aim.as_ref().and_then(Aim::outcome).cloned() else {
            return;
        };
        self.aim = None;
        self.alpha = EFFECT_ALPHA;
        match outcome {
            AimOutcome::Chosen {
                weapon,
                side,
                targets,
            } => {
                let views: Vec<SceneUnit> =
                    self.attack.aim.as_ref().map_or_else(Vec::new, |setup| {
                        targets
                            .iter()
                            .filter_map(|&slot| setup.units[side][slot].map(|unit| unit.view))
                            .collect()
                    });
                self.attack.weapon = weapon;
                self.attack.targets.clone_from(&views);
                let part = self.weapon_part(&self.attack.attacker);
                if let Some(stage) = self.stage.as_mut() {
                    stage.fire = saga_battle::fire_part(rom, part & 0x0FFF);
                }
                self.targets = views.into_iter().collect();
                self.events.push(SceneEvent::Aimed {
                    weapon,
                    side,
                    targets,
                });
                self.step = Step::Pause(PAUSE_FRAMES);
            }
            AimOutcome::Cancelled => {
                self.cancelled = true;
                self.step = Step::FadeOut;
            }
        }
    }

    /// One frame of the slide (`0x08044FA0`): the Zoid comes in from the
    /// edge, faster each frame, then the view goes on in the frame it
    /// arrives.
    fn slide(&mut self, rom: &[u8], view: View, t: u32) {
        if t == 0 {
            self.alpha = EFFECT_ALPHA;
            self.zoid_x = SLIDE_FROM;
            self.step = Step::Slide(view, 1);
            return;
        }
        let k = i32::try_from(t - 1).unwrap_or(i32::MAX);
        let position = SLIDE_FROM + SLIDE_FIRST_STEP * k + (SLIDE_ACCELERATION / 2) * k * (k - 1);
        self.zoid_x = position;
        if position < 0 {
            self.step = Step::Slide(view, t + 1);
            return;
        }
        self.zoid_x = 0;
        match view {
            View::Attacker if self.attack.aim.is_some() && !self.attack.story => {
                self.step = Step::AimStart;
            }
            View::Attacker => {
                self.step = Step::Pause(if self.attack.story {
                    STORY_PAUSE_FRAMES
                } else {
                    PAUSE_FRAMES
                });
            }
            View::Target => {
                self.skip = HITS_SKIP_HOLD;
                self.fire_view(rom, View::Target);
                self.step = Step::Shots;
            }
        }
    }

    /// Spawns the shots of the view's animation (`0x080485C4`). Loading a
    /// heavy set of sprites costs the next frame: the trace showed it for
    /// 4128 bytes of tiles and not for 1920, and the port draws the line at
    /// [`SPAWN_LAG_BYTES`].
    fn fire_view(&mut self, rom: &[u8], view: View) {
        self.lag = 0;
        let Some(stage) = &self.stage else {
            return;
        };
        let Some(fire) = stage.fire else {
            return;
        };
        let part = self.weapon_part(&self.attack.attacker);
        let back = part & 0x1000 != 0;
        let unit = stage.unit;
        let missed = view == View::Target && !self.hit_of(&unit).landed;
        let animation = match view {
            View::Attacker => {
                if back && fire.back != 0 {
                    fire.back
                } else if unit.kind == SKY && fire.sky_attacker != 0 {
                    fire.sky_attacker
                } else {
                    fire.attacker
                }
            }
            View::Target => {
                if unit.kind == SKY && fire.sky_target != 0 {
                    fire.sky_target
                } else {
                    fire.target
                }
            }
        };
        if animation == 0 {
            return;
        }
        let Some(spawns) = saga_battle::shot_animation(rom, animation, missed) else {
            return;
        };
        let mount = saga_battle::weapon_mount(rom, unit.zoid, self.attack.weapon)
            .map_or((0, 0), |(x, y)| (unsigned(x), unsigned(y)));
        let place = ShotPlace {
            view,
            enemy: unit.enemy,
            mount,
            fire,
            back,
            spread: saga_battle::shot_spread(rom, unit.zoid).unwrap_or((0, 0)),
            chosen_slot: 0,
            own_side: self.own_side(rom),
        };
        let loaded = self
            .entities
            .spawn(&spawns, &place, |id| saga_battle::shot_sprite(rom, id));
        self.lag = u32::from(loaded > SPAWN_LAG_BYTES);
    }

    /// The scenery's task, once a frame (`0x08043FF8`): the layer's own
    /// scroll by the kind, the lines by the scenery's routine, the shake,
    /// then the table the `HBlank` handler reads and the Zoid's layer.
    fn update_scenery(&mut self, rom: &[u8]) {
        let rate = match (self.kind, self.enemy_view) {
            (3, false) => -0x4000,
            (3, true) => 0x4000,
            (SKY, false) => -0x3_0000,
            (SKY, true) => 0x3_0000,
            (_, false) => -0x1000,
            (_, true) => 0x1000,
        };
        self.bg2_x = self.bg2_x.wrapping_add(rate);
        self.bg1.1 = if self.kind == SKY {
            let wave = saga_battle::wave(rom, self.counter).unwrap_or(0);
            i32::from(unsigned(wave)) << 10
        } else {
            0
        };
        self.run_lines(rom);
        if let Some((kind, step)) = self.shake {
            let steps = saga_battle::shake_steps(rom, kind).unwrap_or_default();
            let value = steps.get(step).map_or(0, |value| i32::from(*value) << 16);
            let cumulative = matches!(kind, 2 | 3 | 5);
            if cumulative {
                self.shake_offset = self.shake_offset.wrapping_add(value);
            } else {
                self.shake_offset = value;
            }
            if step + 1 >= steps.len() {
                self.shake = None;
                if !cumulative {
                    self.shake_offset = 0;
                }
            } else {
                self.shake = Some((kind, step + 1));
            }
        }
        for line in 0..MOVING_LINES {
            self.table[line] = if self.enemy_view {
                self.bg2_x.wrapping_add(self.lines[line])
            } else {
                self.bg2_x.wrapping_sub(self.lines[line])
            };
        }
        for entry in &mut self.table[MOVING_LINES..] {
            *entry = self.bg2_x;
        }
        self.bg1.0 = if self.enemy_view {
            self.zoid_x.wrapping_neg().wrapping_sub(self.shake_offset)
        } else {
            self.zoid_x
                .wrapping_sub(PARTY_ZOID_OFFSET)
                .wrapping_add(self.shake_offset)
        };
        self.counter = self.counter.wrapping_add(1);
    }

    /// The scenery's line routine (the jump table at `0x080441B8`).
    fn run_lines(&mut self, rom: &[u8]) {
        for op in line_program(self.scenery) {
            match *op {
                LineOp::Add(first, last, amount) => {
                    for line in first..=last {
                        self.lines[line] = self.lines[line].wrapping_add(amount);
                    }
                }
                LineOp::Ramp(first, last, at_first, step) => {
                    for line in first..=last {
                        let index = i32::try_from(line - first).unwrap_or(0);
                        self.lines[line] = self.lines[line].wrapping_add(at_first + step * index);
                    }
                }
                LineOp::Wave {
                    first,
                    last,
                    origin,
                    spacing,
                    speed,
                    shift,
                } => {
                    self.phase = self.phase.wrapping_add(speed);
                    for line in first..=last {
                        let offset = (line.wrapping_sub(origin)).wrapping_mul(spacing);
                        let index = u8::try_from(offset & 0xFF)
                            .unwrap_or(0)
                            .wrapping_add(self.phase);
                        let wave = unsigned(saga_battle::wave(rom, index).unwrap_or(0));
                        self.lines[line] = match shift {
                            WaveShift::Pixels16 => i32::from(wave >> 4) << 16,
                            WaveShift::Fraction(bits) => {
                                i32::from_ne_bytes((u32::from(wave) << bits).to_ne_bytes())
                            }
                        };
                    }
                }
            }
        }
    }

    /// The horizontal scroll of the scenery on screen line `line`.
    fn scenery_scroll(&self, line: usize) -> i32 {
        let entry = if line == 0 { 1 } else { line - 1 };
        let table = if line < TABLE_WRITE_LINE {
            &self.previous_table
        } else {
            &self.shown_table
        };
        table.get(entry).copied().unwrap_or(0) >> 16
    }

    /// Draws the scene: the scenery, the sprites behind the Zoid, the Zoid,
    /// the sprites in front, the windows, darkened by the brightness.
    pub fn draw(
        &self,
        frame: &mut Frame,
        windows: &ScriptWindows<'_>,
        skin: &WindowPainter,
        painter: &TextPainter,
    ) {
        frame.fill(Rgb::default());
        let Some(stage) = &self.stage else {
            return;
        };
        if self.shown_brightness >= BLACK {
            return;
        }
        let mirrored = stage.unit.enemy;
        let objects = self.object_layer();
        let (zoid_x, zoid_y) = (self.shown_bg1.0 >> 16, self.shown_bg1.1 >> 16);
        for y in 0..HEIGHT.min(frame.height()) {
            let scroll = self.scenery_scroll(y);
            for x in 0..WIDTH.min(frame.width()) {
                let scenery = stage.scenery.as_ref().map_or(0, |image| {
                    let sx = wrap(x, scroll, SCENERY_MAP_WIDTH);
                    layer_pixel(&image.tiles, sx, y % SCENERY_MAP_HEIGHT, mirrored, true)
                });
                let zoid = stage.zoid.as_ref().map_or(0, |image| {
                    let zx = wrap(x, zoid_x, ZOID_MAP_WIDTH);
                    let zy = wrap(y, zoid_y, ZOID_MAP_HEIGHT);
                    layer_pixel(&image.tiles, zx, zy, mirrored, false)
                });
                let backdrop = stage.palette.color(0);
                let scenery_color = (scenery != 0).then(|| stage.palette.color(scenery));
                let zoid_color = (zoid != 0).then(|| stage.palette.color(zoid));
                let object = objects[y * WIDTH + x];
                let mut layers: Vec<(u8, Rgb, bool)> = Vec::with_capacity(3);
                // Priority first; a sprite goes over a layer of its own
                // priority.
                if let Some((color, priority, semi)) = object
                    && priority <= 2
                {
                    layers.push((priority, color, semi));
                }
                if let Some(color) = zoid_color {
                    layers.push((2, color, false));
                }
                if let Some((color, priority, semi)) = object
                    && priority == 3
                {
                    layers.push((priority, color, semi));
                }
                if let Some(color) = scenery_color {
                    layers.push((3, color, false));
                }
                let color = match layers.as_slice() {
                    [] => backdrop,
                    [(_, top, true), (_, below, _), ..] => blend(*top, *below, self.alpha),
                    [(_, top, _), ..] => *top,
                };
                frame.set_pixel(x, y, color);
            }
        }
        windows.draw_shown(frame, skin, painter);
        darken(frame, self.shown_brightness);
    }

    /// The sprites' pixels: for each screen pixel, the first sprite's in
    /// OAM order among the highest priority, with its priority and whether
    /// it blends.
    fn object_layer(&self) -> Vec<Option<(Rgb, u8, bool)>> {
        let mut layer: Vec<Option<(Rgb, u8, bool)>> = vec![None; WIDTH * HEIGHT];
        for shown in &self.shown {
            let Some(sprite) = self.entities.sprites.get(shown.sprite) else {
                continue;
            };
            let Some(pieces) = sprite.frames.get(shown.frame) else {
                continue;
            };
            let palette = Palette::new(sprite.palette.map(Palette::from_bgr555));
            for piece in pieces {
                draw_piece(&mut layer, sprite, &palette, piece, shown);
            }
        }
        layer
    }
}

/// A scenery line routine's step.
#[derive(Debug, Clone, Copy)]
enum LineOp {
    /// Lines `first..=last` move by the amount (16.16) a frame.
    Add(usize, usize, i32),
    /// Lines `first..=last` move by the first amount on the first and a
    /// step more on each.
    Ramp(usize, usize, i32, i32),
    /// Lines `first..=last` sway: set each frame from the wave table.
    Wave {
        first: usize,
        last: usize,
        origin: usize,
        spacing: usize,
        speed: u8,
        shift: WaveShift,
    },
}

#[derive(Debug, Clone, Copy)]
enum WaveShift {
    /// The wave in sixteenths of a pixel.
    Pixels16,
    /// The wave shifted up by the bits into 16.16.
    Fraction(u32),
}

const FOREST_WAVE: LineOp = LineOp::Wave {
    first: 0,
    last: 127,
    origin: 0,
    spacing: 1,
    speed: 2,
    shift: WaveShift::Pixels16,
};
const HEAT_WAVE: LineOp = LineOp::Wave {
    first: 32,
    last: 79,
    origin: 32,
    spacing: 32,
    speed: 2,
    shift: WaveShift::Fraction(10),
};
const CITY_WAVE: LineOp = LineOp::Wave {
    first: 24,
    last: 74,
    origin: 32,
    spacing: 32,
    speed: 4,
    shift: WaveShift::Fraction(9),
};

/// Scenery `scenery`'s line routine (the jump table at `0x080441B8`, by
/// scenery − 3), read from the routines' code.
fn line_program(scenery: u8) -> &'static [LineOp] {
    use LineOp::{Add, Ramp};
    match scenery {
        3 => &[Add(56, 63, 0x2000), Ramp(64, 127, 0x8300, 0x300)],
        4 => &[Add(72, 79, 0x3000), Ramp(80, 127, 0x5400, 0x400)],
        5 => &[Add(85, 95, 0x1000), Ramp(96, 127, 0x3300, 0x300)],
        6 => &[Add(54, 104, 0x6000), Ramp(105, 127, 0x8800, 0x800)],
        7 => &[Add(72, 112, 0x3000), Ramp(113, 127, 0x5800, 0x800)],
        8 => &[
            Add(85, 91, 0x1000),
            Add(92, 113, 0x2000),
            Ramp(114, 127, 0x5400, 0x400),
        ],
        9..=11 => &[FOREST_WAVE],
        12 => &[Add(46, 95, 0x6000), Ramp(96, 127, 0x8800, 0x800)],
        13 => &[Add(71, 103, 0x3000), Ramp(104, 127, 0x5800, 0x800)],
        14 => &[
            Add(85, 95, 0x1000),
            Add(96, 114, 0x3000),
            Ramp(115, 127, 0x5800, 0x800),
        ],
        15 => &[HEAT_WAVE, Add(64, 79, 0x3000), Ramp(80, 127, 0xA800, 0x800)],
        16 | 17 => &[
            HEAT_WAVE,
            Add(71, 111, 0x3000),
            Ramp(112, 127, 0x6800, 0x800),
        ],
        18 => &[Add(79, 95, 0x6000), Ramp(96, 127, 0x8800, 0x800)],
        19 => &[Add(96, 103, 0x3000), Ramp(104, 127, 0x5800, 0x800)],
        20 => &[Add(103, 110, 0x1000), Ramp(111, 127, 0x7800, 0x800)],
        30..=32 => &[
            Add(0, 23, 0x3000),
            Ramp(59, 74, 0x200, 0x200),
            Add(75, 91, 0x2000),
            Ramp(92, 127, 0x2400, 0x400),
        ],
        33..=35 => &[
            CITY_WAVE,
            Add(0, 23, 0x3000),
            Ramp(59, 74, 0x200, 0x200),
            Add(75, 91, 0x2000),
            Ramp(92, 127, 0x2400, 0x400),
        ],
        36..=38 => &[Add(72, 81, 0x1000), Ramp(82, 127, 0x1300, 0x300)],
        39..=41 => &[Ramp(86, 127, 0x1300, 0x300)],
        42..=44 => &[Add(52, 89, 0x2000), Ramp(90, 127, 0x3400, 0x400)],
        45..=47 => &[Add(0, 127, 0x2000)],
        48..=50 => &[
            Add(0, 8, 0x4000),
            Add(9, 22, 0x3000),
            Add(23, 32, 0x2000),
            Add(33, 39, 0x1000),
            Add(61, 84, 0x2000),
            Ramp(85, 127, 0x2400, 0x400),
        ],
        51..=53 => &[Add(110, 120, 0x1000), Add(121, 143, 0x1_0000)],
        _ => &[],
    }
}

/// The `system` table's strings (ROM `0x6D0880`).
fn saga_battle_system_offsets(rom: &[u8]) -> Vec<usize> {
    extraction::saga_guide::SYSTEM_SCRIPTS
        .offsets(rom)
        .map(|offsets| offsets.into_iter().collect())
        .unwrap_or_default()
}

/// A sprite piece's pixels into the object layer, as the hardware wraps
/// them: 9-bit x and 8-bit y.
fn draw_piece(
    layer: &mut [Option<(Rgb, u8, bool)>],
    sprite: &extraction::saga_battle::EffectSprite,
    palette: &Palette,
    piece: &extraction::saga_battle::EffectPiece,
    shown: &Shown,
) {
    const PLAIN: u16 = 0xFF;
    const DOUBLE_SIZE: u16 = 0x200;
    const FLIP_X: u16 = 1;
    const FLIP_Y: u16 = 2;
    let (width, height) = (i32::from(piece.width), i32::from(piece.height));
    let affine = piece.affine != PLAIN || shown.affine.is_some();
    let double = shown.affine.is_none() && affine && piece.affine & DOUBLE_SIZE != 0;
    let (box_width, box_height) = if double {
        (width * 2, height * 2)
    } else {
        (width, height)
    };
    let (anchor_x, anchor_y) = shown.anchor;
    let left = if shown.mirrored {
        anchor_x - i32::from(piece.x) - width - if double { width } else { 0 }
    } else {
        anchor_x + i32::from(piece.x)
    };
    let top = anchor_y + i32::from(piece.y);
    let (left, top) = (left & 0x1FF, top & 0xFF);
    let matrix = match shown.affine {
        Some(transform) => Some(entity_matrix(transform, shown.mirrored)),
        None => affine.then(|| piece_matrix(piece, shown.mirrored)),
    };
    for row in 0..box_height {
        let y = usize::try_from((top + row) & 0xFF).unwrap_or(usize::MAX);
        if y >= HEIGHT {
            continue;
        }
        for column in 0..box_width {
            let x = usize::try_from((left + column) & 0x1FF).unwrap_or(usize::MAX);
            if x >= WIDTH {
                continue;
            }
            let slot = &mut layer[y * WIDTH + x];
            if slot.is_some_and(|(_, priority, _)| priority <= shown.priority) {
                continue;
            }
            let source = if let Some((pa, pb, pc, pd)) = matrix {
                let (dx, dy) = (column - box_width / 2, row - box_height / 2);
                (
                    ((pa * dx + pb * dy) >> 8) + width / 2,
                    ((pc * dx + pd * dy) >> 8) + height / 2,
                )
            } else {
                let flipped = (piece.attributes & FLIP_X != 0) != shown.mirrored;
                let tx = if flipped { width - 1 - column } else { column };
                let ty = if piece.attributes & FLIP_Y != 0 {
                    height - 1 - row
                } else {
                    row
                };
                (tx, ty)
            };
            if let Some(index) = piece_pixel(sprite, piece, source)
                && index != 0
            {
                *slot = Some((palette.color(index), shown.priority, shown.semi_transparent));
            }
        }
    }
}

/// The matrix an entity's own transform sets for all its pieces
/// (`0x08000560` with `ObjAffineSet`, whose ratios are taken as they are):
/// the BIOS's sine table in 1.14 by the angle's high byte, the first entry
/// negated for a mirrored entity.
#[allow(clippy::cast_possible_truncation)]
fn entity_matrix((sx, sy, angle): (i16, i16, u16), mirrored: bool) -> (i32, i32, i32, i32) {
    let turn = f64::from(angle >> 8) * std::f64::consts::TAU / 256.0;
    let sin = (turn.sin() * 16384.0).round() as i32;
    let cos = (turn.cos() * 16384.0).round() as i32;
    let (sx, sy) = (i32::from(sx), i32::from(sy));
    let mut pa = (sx * cos) >> 14;
    let pb = (-sx * sin) >> 14;
    let pc = (sy * sin) >> 14;
    let pd = (sy * cos) >> 14;
    if mirrored {
        pa = -pa;
    }
    (pa, pb, pc, pd)
}

/// A semi-transparent sprite's pixel over the layer below it, `eva`/16 of
/// the sprite and `evb`/16 of the layer, per 5-bit channel.
fn blend(sprite: Rgb, below: Rgb, (eva, evb): (u8, u8)) -> Rgb {
    let channel = |top: u8, bottom: u8| {
        let mixed =
            ((u16::from(top >> 3) * u16::from(eva) + u16::from(bottom >> 3) * u16::from(evb)) >> 4)
                .min(31);
        let five = u8::try_from(mixed).unwrap_or(31);
        five << 3 | five >> 2
    };
    Rgb::new(
        channel(sprite.r, below.r),
        channel(sprite.g, below.g),
        channel(sprite.b, below.b),
    )
}

/// A half-word read as unsigned.
fn unsigned(value: i16) -> u16 {
    u16::from_ne_bytes(value.to_ne_bytes())
}

fn wrap(x: usize, scroll: i32, width: usize) -> usize {
    let width = i64::try_from(width).unwrap_or(i64::MAX);
    let position = i64::try_from(x).unwrap_or(0) + i64::from(scroll);
    usize::try_from(position.rem_euclid(width)).unwrap_or(0)
}

/// The palette index at map pixel `(x, y)` of a battle image's layer: its
/// 16×16 tiles in the map's top-left corner, again beside it when the map
/// `repeats` (the scenery's), mirrored on the enemy's side
/// (`0x08043DE8`, `0x08044EE4`); the rest of the map is tile 0.
fn layer_pixel(
    tiles: &[[u8; TILE_PIXELS]],
    x: usize,
    y: usize,
    mirrored: bool,
    repeats: bool,
) -> u8 {
    let (column, row) = (x / TILE, y / TILE);
    let inside = row < IMAGE_TILES_SIDE
        && (column < IMAGE_TILES_SIDE || repeats && column < 2 * IMAGE_TILES_SIDE);
    let (tile, pixel_x) = if !inside {
        (0, x % TILE)
    } else if mirrored {
        (
            row * IMAGE_TILES_SIDE + IMAGE_TILES_SIDE - 1 - column % IMAGE_TILES_SIDE,
            TILE - 1 - x % TILE,
        )
    } else {
        (row * IMAGE_TILES_SIDE + column % IMAGE_TILES_SIDE, x % TILE)
    };
    tiles
        .get(tile)
        .map_or(0, |pixels| pixels[(y % TILE) * TILE + pixel_x])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrains_map_to_scenery_kinds_and_fliers_take_the_sky() {
        assert_eq!(scenery_kind(4, false), 2);
        assert_eq!(scenery_kind(4, true), SKY);
        assert_eq!(scenery_kind(2, true), 8);
        assert_eq!(scenery_kind(20, false), 20);
    }

    #[test]
    fn the_forest_moves_its_bands_each_frame() {
        let ops = line_program(7);
        assert_eq!(ops.len(), 2);
        assert!(line_program(0).is_empty());
    }
}
