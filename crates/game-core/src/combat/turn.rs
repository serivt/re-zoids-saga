//! The fight on the battle screen: once the party engages, the round's
//! menu, the turn order, each unit's action, the attack scenes and the
//! messages that follow them.
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! battle controller's states (`0x0802AC0C`), the engage step of the
//! opening task (`0x0802E9CC`, state `0x2328`, and `0x0802F010`), the row
//! advance (`0x0802F09C`), the effects' expiry (`0x0802FD4C`), the turn
//! order (`0x08032410`), the rolls (`0x08033D94`), the actor's status check
//! (`0x0802FAA8`), the action task (`0x0802E814`), the return to the
//! screen (`0x0802BCA4`), its messages (`0x0802C084`) and hit display
//! (`0x0802D36C`), the name routine (`0x080339C4`), the number routine
//! (`0x08001848`), the round's end (`0x0802F5C4`) and the result routine
//! (`0x08032684`); checked frame by frame against a round of a battle on
//! the world map in a reference emulator. See `docs/combat.md`.
//!
//! As the rest of the battle, the controller takes one state a frame and
//! the tasks it starts run in the frame they start in.

use super::ai::{self, Choice, ENEMY, PARTY};
use super::aim::{AimMemory, AimSetup, AimUnit, AimWeapon, UNKNOWN_HP};
use super::attack::{self, Blow, DESTROYED, ROLLS};
use super::scene::{Attack, AttackScene, Hit, SceneEvent, SceneUnit};
use super::units::OUT;
use super::{Act, Call, Combat, Controller, Fade, Outcome};
use crate::data::GameData;
use crate::script::ScriptError;
use crate::windows::ScriptWindows;
use extraction::saga_battle::EffectSprite;
use extraction::saga_combat::SLOTS;
use platform::Input;

/// The frames the panels slide down in once the party engages: two pixels
/// a frame from 16 (`0x0802E9CC`, state `0x2328`).
const PANEL_SLIDE: i32 = 2;
/// Where the panels start: two rows up, their names hidden.
pub(super) const PANELS_HIDDEN: i32 = 16;
/// The grounds and their units come down 12 pixels, one a frame
/// (`0x0802F010`).
pub(super) const GROUNDS_DOWN: i32 = 12;
/// Frames the engage step waits once the panels are down.
const ENGAGE_WAIT: u32 = 15;
/// The song of the fight (`0x08033D6C`).
const FIGHT_SONG: u16 = 0x17;
/// The status check's frames for each of the twelve slots (`0x0802FAA8`).
const STATUS_FRAMES_PER_SLOT: u32 = 2;
/// Frames the return to the screen takes before its fade in, besides
/// [`RETURN_PER_PANEL`] a panel (`0x0802BCA4`, state 0).
const RETURN_BUILD: u32 = 20;
const RETURN_PER_PANEL: u32 = 8;
/// The frames the rebuild after a given-up aim takes besides
/// [`RETURN_PER_PANEL`] a panel, and how many before its end the message
/// window opens.
const REBUILD: u32 = 18;
const REBUILD_MESSAGE_WINDOW: u32 = 2;
/// The hit display's frames once its sparks have gone (`0x0802D36C`,
/// states 100 to `0x2328`, with the graphics its next states load).
const DISPLAY_TAIL: u32 = 7;
/// Frames before the display's task first runs a spark.
const SPARK_DELAY: u32 = 1;
/// The spark a hit unit shows (the battle screen's effect 6), placed 8
/// pixels right of and 16 above the unit (`0x0802D9A4`).
pub(super) const SPARK: usize = 6;
const SPARK_OFFSET: (i32, i32) = (8, -16);
/// How a hit unit shakes, a step a frame, three times (`0x0802D9A4`).
const SHAKE: [i32; 4] = [-2, 0, 2, 0];
const SHAKES: u32 = 3;
/// Frames the messages' task takes after its last wait before it reports
/// (`0x0802C084`, states 2000 to `0x2328`).
const MESSAGE_TAIL: u32 = 5;
/// The return's frames after its two tasks have ended.
const RETURN_TAIL: u32 = 2;
/// The hit display's sound.
const HIT_SOUND: u16 = 0x5A;
/// The rolls are `% 100` (`0x08033D94`).
const ROLL_RANGE: u16 = 100;

// Scripts of the battle-menu table.
const MENU_DRAW: u16 = 6;
const MENU_CLEAR: u16 = 7;
const MENU_PRESENT: u16 = 5;
const MENU_RESET: u16 = 1;
const MENU_MESSAGE_WINDOW: u16 = 2;
pub(super) const MENU_ROUND: u16 = 3;
pub(super) const MENU_ROUND_STORY: u16 = 0x19;
pub(super) const MENU_ACTION: u16 = 4;
// Scripts of the battle-text table.
const TEXT_IS: u16 = 2;
const TEXT_DAMAGE: u16 = 3;
const TEXT_DODGED: u16 = 4;
const TEXT_ACTION: u16 = 5;
const TEXT_DESTROYED: u16 = 6;
const TEXT_CRITICAL: u16 = 0x25;
const TEXT_CRITICAL_DAMAGE: u16 = 0x26;
const TEXT_DEFENDS: u16 = 0x1A;
const TEXT_CANNOT_ACT: u16 = 0x20;
const TEXT_ROUND: u16 = 0x3B;
/// The letters that tell same Zoids apart: `battle-text` 88 on.
const TEXT_LETTERS: u16 = 88;
/// The digits: `battle-text` 94 on.
const TEXT_DIGITS: u16 = 94;
/// "敵ゾイド　", the enemy's actions' prefix.
const LABEL_ENEMY: u16 = 16;
/// The round menu's lines.
const ROUND_FIGHT: u16 = 0;
const ROUND_RETREAT: u16 = 2;
/// The action menu's lines.
const ACTION_ATTACK: u16 = 0;
const ACTION_DEFEND: u16 = 1;
/// The unit is defending: it takes half the damage until its next action.
const DEFENDING: u16 = 0x100;
/// The unit is paralysed: it can't act.
const PARALYSED: u16 = 0x4000;
/// How much the units that don't act are darkened.
const DIM_OTHERS: u8 = 12;
const DIM_PARALYSED: u8 = 24;

/// The controller's states in the fight, by the original's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    /// 0x3F2: the engage step has ended.
    Engaged,
    /// 0x76C.
    Prepared,
    /// 0x1B58: the round's menu.
    RoundMenu,
    AwaitRoundMenu,
    /// 2000: the round starts; 0x7DA waits for the row advance.
    RoundStart,
    AwaitRows,
    /// 0x8FC and 0x906: the effects' expiry.
    Expiry,
    AwaitExpiry,
    /// 0x9C4: the turn's order, which costs a frame more.
    Order,
    OrderLoad,
    /// 0xBB8: the next actor; 0xBD6 waits for its status check.
    Actor,
    AwaitStatus,
    /// 0xBEA: the actor acts; 0xC1C waits for its choice.
    Act,
    AwaitAction,
    /// 0xED8 and 0xEE2: it can't act.
    CannotAct,
    AwaitCannotAct,
    /// 0x1194 and 0x119E: it defends.
    Defend,
    AwaitDefend,
    /// 4000 to 0x1004: the screen fades out; 0x1068 starts the scene.
    Attack,
    AwaitAttackFade,
    StartScene,
    /// 0x10CC: the scene plays.
    Scene,
    /// 0x1130 and 0x113A: the aim was given up; the screen comes back and
    /// the actor chooses again.
    Rebuild,
    AwaitRebuild,
    /// 0x1388 and 0x13EC: back to the screen.
    Return,
    AwaitReturn,
    /// 0x1770: the actor's turn ends.
    Next,
    /// 0x2328: the battle is over.
    Over,
}

impl Stage {
    /// The original's state number.
    pub(super) fn number(self) -> u16 {
        match self {
            Stage::Engaged => 0x3F2,
            Stage::Prepared => 0x76C,
            Stage::RoundMenu | Stage::AwaitRoundMenu => 0x1B58,
            Stage::RoundStart => 2000,
            Stage::AwaitRows => 0x7DA,
            Stage::Expiry => 0x8FC,
            Stage::AwaitExpiry => 0x906,
            Stage::Order | Stage::OrderLoad => 0x9C4,
            Stage::Actor => 0xBB8,
            Stage::AwaitStatus => 0xBD6,
            Stage::Act => 0xBEA,
            Stage::AwaitAction => 0xC1C,
            Stage::CannotAct => 0xED8,
            Stage::AwaitCannotAct => 0xEE2,
            Stage::Defend => 0x1194,
            Stage::AwaitDefend => 0x119E,
            Stage::Attack => 4000,
            Stage::AwaitAttackFade => 0x1004,
            Stage::StartScene => 0x1068,
            Stage::Scene => 0x10CC,
            Stage::Rebuild => 0x1130,
            Stage::AwaitRebuild => 0x113A,
            Stage::Return => 0x1388,
            Stage::AwaitReturn => 0x13EC,
            Stage::Next => 0x1770,
            Stage::Over => 0x2328,
        }
    }
}

/// The fight's slot-5 task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Task {
    Idle,
    /// The row advance (`0x0802F09C`): the frames it has run.
    Rows(u32),
    /// The effects' expiry (`0x0802FD4C`).
    Expiry(u32),
    /// The actor's status check (`0x0802FAA8`).
    Status(u32),
    /// The action task (`0x0802E814`).
    Action(ActionStep),
    /// The return to the screen (`0x0802BCA4`).
    Return(u32),
    /// A task's result, until the controller takes it.
    Reported,
}

/// The action task's states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActionStep {
    /// State 0: the actor's message.
    Message,
    /// 1000: the player's menu.
    Menu,
    AwaitMenu,
    /// 2000: the enemy's choice.
    Choose,
}

/// What the return's messages task does (`0x0802C084`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Report {
    /// The message of target `n`, waiting for its wait.
    Target(usize),
    Destroyed(usize),
    /// The frames after the last wait.
    Tail(u32),
    Done,
    /// The return's own frames once the messages and the hit display have
    /// ended (states `0x1BBC` to `0x2328`).
    Closing(u32),
}

/// The hit display (`0x0802D36C`): each hit unit's spark and shake, then
/// its own frames.
pub(super) struct Display {
    pub(super) sparks: Vec<Spark>,
    frames: u32,
    tail: Option<u32>,
}

/// A hit unit's spark (`0x0802D9A4`).
#[derive(Clone)]
pub(super) struct Spark {
    pub(super) side: usize,
    pub(super) slot: usize,
    /// Frames until the display's task first runs it.
    delay: u32,
    frame: u32,
    step: usize,
    ticks: u32,
    ended: bool,
    shown: bool,
}

impl Spark {
    /// How far the unit is shaken this frame.
    pub(super) fn shake(&self) -> i32 {
        if self.shown && self.frame < SHAKES * 4 {
            SHAKE[usize::try_from(self.frame % 4).unwrap_or(0)]
        } else {
            0
        }
    }

    /// The spark's frame, when it shows.
    pub(super) fn sprite_frame(&self, sprite: &EffectSprite) -> Option<usize> {
        if !self.shown {
            return None;
        }
        sprite.animation.get(self.step).map(|step| step.frame)
    }

    /// Where the spark's anchor is, from the unit's.
    pub(super) fn anchor((x, y): (i32, i32)) -> (i32, i32) {
        (x + SPARK_OFFSET.0, y + SPARK_OFFSET.1)
    }

    fn advance(&mut self, sprite: &EffectSprite) {
        if self.delay > 0 {
            self.delay -= 1;
            if self.delay > 0 {
                return;
            }
            // It spawns, and the sprite system's pass at the end of the
            // frame counts its first tick.
            self.shown = true;
        } else {
            self.frame += 1;
        }
        if self.ended {
            return;
        }
        self.ticks = self.ticks.saturating_sub(1);
        if self.ticks > 0 {
            return;
        }
        if self.step + 1 >= sprite.animation.len() {
            self.ended = true;
        } else {
            self.step += 1;
            self.ticks = sprite.animation[self.step].duration.max(1);
        }
    }
}

/// The fight's state beside the controller's.
#[derive(Default)]
pub(super) struct Fight {
    pub(super) order: Vec<(usize, usize)>,
    pub(super) index: usize,
    pub(super) rolls: [u16; ROLLS],
    pub(super) choice: Option<Choice>,
    pub(super) action: Option<u16>,
    pub(super) blows: Vec<Blow>,
    pub(super) scene: Option<AttackScene>,
    pub(super) report: Option<Report>,
    pub(super) display: Option<Display>,
    pub(super) experience: u32,
    pub(super) money: u32,
    /// The weapon each party slot last aimed.
    pub(super) memory: [Option<AimMemory>; SLOTS],
    /// The player gave the aim up: the screen comes back for the menu.
    pub(super) cancelled: bool,
    /// Frames the screen's rebuild has taken.
    pub(super) rebuild: u32,
}

impl Combat {
    /// The actor of the turn, as (side, slot).
    pub(super) fn actor(&self) -> Option<(usize, usize)> {
        self.fight.order.get(self.fight.index).copied()
    }

    /// Whether the actor is the party's.
    fn actor_is_party(&self) -> bool {
        self.actor().is_some_and(|(side, _)| side == PARTY)
    }

    /// One state of the fight's controller.
    pub(super) fn step_fight(&mut self, rom: &[u8]) {
        let Controller::Fight(stage) = self.controller else {
            return;
        };
        let next = match stage {
            Stage::Engaged => {
                self.acts.push_back(Act::Music(FIGHT_SONG));
                Some(Stage::Prepared)
            }
            Stage::Prepared => Some(if self.battle_over() {
                Stage::Over
            } else {
                Stage::RoundMenu
            }),
            Stage::RoundMenu => {
                self.restore_units();
                self.acts.push_back(Act::Call(Call::Menu(MENU_DRAW)));
                self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR)));
                self.acts.push_back(Act::Call(Call::Text(TEXT_ROUND)));
                self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
                let menu = if self.story {
                    MENU_ROUND_STORY
                } else {
                    MENU_ROUND
                };
                self.acts.push_back(Act::Call(Call::Menu(menu)));
                Some(Stage::AwaitRoundMenu)
            }
            // The menu's return moves the controller on in its own frame
            // (`choose_in_fight`), as the original's call returns within
            // its state.
            Stage::AwaitRoundMenu => None,
            Stage::RoundStart => {
                self.task = Task::Rows(0);
                Some(Stage::AwaitRows)
            }
            Stage::AwaitRows => self.take_task().then_some(Stage::Expiry),
            Stage::Expiry => {
                self.task = Task::Expiry(0);
                Some(Stage::AwaitExpiry)
            }
            Stage::AwaitExpiry => self.take_task().then_some(Stage::Order),
            Stage::Order => {
                self.fight.order =
                    ai::turn_order(&self.sides, ai::Order::Fastest, &mut self.rng, self.vblank);
                self.fight.index = 0;
                Some(Stage::OrderLoad)
            }
            Stage::OrderLoad | Stage::Actor => {
                self.start_actor();
                Some(Stage::AwaitStatus)
            }
            Stage::AwaitStatus => self.take_task().then_some(Stage::Act),
            Stage::Act => Some(self.act()),
            Stage::AwaitAction => {
                let next = match self.fight.action {
                    Some(ACTION_DEFEND) => Stage::Defend,
                    Some(ACTION_ATTACK) => Stage::Attack,
                    _ => Stage::Next,
                };
                self.take_task().then_some(next)
            }
            Stage::CannotAct => {
                self.actor_message(TEXT_CANNOT_ACT);
                Some(Stage::AwaitCannotAct)
            }
            Stage::Defend => {
                if let Some(unit) = self.actor_unit_mut() {
                    unit.traits |= DEFENDING;
                }
                self.actor_message(TEXT_DEFENDS);
                Some(Stage::AwaitDefend)
            }
            Stage::AwaitCannotAct | Stage::AwaitDefend => self.wait_done().then_some(Stage::Next),
            Stage::Attack
            | Stage::AwaitAttackFade
            | Stage::StartScene
            | Stage::Scene
            | Stage::Rebuild
            | Stage::AwaitRebuild
            | Stage::Return
            | Stage::AwaitReturn => self.step_attack_stage(rom, stage),
            Stage::Next => Some(self.end_actor()),
            Stage::Over => {
                self.finish();
                None
            }
        };
        if let Some(next) = next
            && self.controller == Controller::Fight(stage)
        {
            self.controller = Controller::Fight(next);
        }
    }

    /// The battle is over (`0x2328`): won unless the party is beaten, when
    /// nothing else ended it.
    fn finish(&mut self) {
        if self.outcome.is_none() {
            let beaten = |side: usize| {
                self.sides[side]
                    .iter()
                    .flatten()
                    .all(|unit| !unit.fighting())
            };
            self.outcome = Some(if beaten(PARTY) {
                Outcome::Lost
            } else {
                Outcome::Won
            });
        }
        self.controller = Controller::StartResults;
    }

    /// The attack's stages of the controller: the fade out, the scene and
    /// the return to the screen.
    fn step_attack_stage(&mut self, rom: &[u8], stage: Stage) -> Option<Stage> {
        match stage {
            Stage::Attack => {
                self.fade = Some(Fade { out: true, age: 0 });
                Some(Stage::AwaitAttackFade)
            }
            Stage::AwaitAttackFade => self
                .fade
                .is_some_and(|fade| fade.age >= super::FADE_SEEN_AGE)
                .then(|| {
                    self.fade = None;
                    Stage::StartScene
                }),
            Stage::StartScene => {
                self.start_scene(rom);
                Some(Stage::Scene)
            }
            Stage::Scene => self
                .fight
                .scene
                .as_ref()
                .is_some_and(AttackScene::is_done)
                .then(|| {
                    self.fight.scene = None;
                    if std::mem::take(&mut self.fight.cancelled) {
                        Stage::Rebuild
                    } else {
                        Stage::Return
                    }
                }),
            Stage::Rebuild => Some(self.rebuild()),
            Stage::AwaitRebuild => self
                .fade
                .is_some_and(|fade| fade.age >= super::FADE_SEEN_AGE)
                .then(|| {
                    self.fade = None;
                    Stage::Act
                }),
            Stage::Return => {
                self.task = Task::Return(0);
                Some(Stage::AwaitReturn)
            }
            Stage::AwaitReturn => self.take_task().then_some(Stage::Next),
            _ => None,
        }
    }

    /// A frame of the screen's rebuild once the aim was given up (state
    /// `0x1130`, `0x0802B0B4`): built again while black, a panel every 8
    /// frames, the message window, then the fade in.
    fn rebuild(&mut self) -> Stage {
        let panels = u32::try_from(self.panels.len()).unwrap_or(0);
        let build = REBUILD + RETURN_PER_PANEL * panels;
        let frame = self.fight.rebuild;
        self.fight.rebuild += 1;
        if frame == 0 {
            self.level = super::BLACK;
            self.restore_units();
            self.dim_panels = super::PanelLight::Own;
        }
        if frame + REBUILD_MESSAGE_WINDOW == build {
            self.acts
                .push_back(Act::Call(Call::Menu(MENU_MESSAGE_WINDOW)));
        }
        if frame + 1 < build {
            return Stage::Rebuild;
        }
        self.fight.rebuild = 0;
        self.fade = Some(Fade { out: false, age: 0 });
        Stage::AwaitRebuild
    }

    /// Takes a finished task's result.
    fn take_task(&mut self) -> bool {
        if self.task == Task::Reported {
            self.task = Task::Idle;
            return true;
        }
        false
    }

    /// Whether a side has no unit left (`0x08032684`).
    pub(super) fn battle_over(&self) -> bool {
        let beaten = |side: usize| {
            self.sides[side]
                .iter()
                .flatten()
                .all(|unit| !unit.fighting())
        };
        self.outcome.is_some() || beaten(PARTY) || beaten(ENEMY)
    }

    fn actor_unit_mut(&mut self) -> Option<&mut super::units::BattleUnit> {
        let (side, slot) = self.actor()?;
        self.sides[side][slot].as_mut()
    }

    /// The next actor (`0x0802AF70`): the turn's rolls, its statistics
    /// again, its defense dropped, and its status check.
    fn start_actor(&mut self) {
        for roll in &mut self.fight.rolls {
            *roll = self.rng.next(self.vblank) % ROLL_RANGE;
        }
        if let Some(unit) = self.actor_unit_mut() {
            unit.traits &= !DEFENDING;
        }
        self.dim_panels = super::PanelLight::Dimmed { lit: None };
        self.task = Task::Status(0);
    }

    /// The actor acts (state `0xBEA`): unless it is out or can't act, the
    /// action task starts.
    fn act(&mut self) -> Stage {
        let Some((side, slot)) = self.actor() else {
            return Stage::Next;
        };
        let Some(unit) = self.sides[side][slot].as_ref() else {
            return Stage::Next;
        };
        if !unit.fighting() {
            return Stage::Next;
        }
        if unit.traits & PARALYSED != 0 {
            return Stage::CannotAct;
        }
        self.fight.action = None;
        self.fight.choice = None;
        if side == PARTY {
            self.light_panel(slot);
        }
        self.task = Task::Action(ActionStep::Message);
        Stage::AwaitAction
    }

    /// A message about the actor, then the wait: its name and `text`.
    fn actor_message(&mut self, text: u16) {
        let mut calls = self.unit_name(self.actor());
        calls.push(Call::Text(text));
        self.message(&calls);
    }

    /// The calls that print a unit's name (`0x080339C4`): its Zoid, and a
    /// letter when its side has more than one of it.
    pub(super) fn unit_name(&self, unit: Option<(usize, usize)>) -> Vec<Call> {
        let Some((side, slot)) = unit else {
            return Vec::new();
        };
        let Some(zoid) = self.sides[side][slot].as_ref().map(|unit| unit.zoid) else {
            return Vec::new();
        };
        let mut calls = vec![Call::Name(zoid)];
        let same: Vec<usize> = (0..SLOTS)
            .filter(|other| {
                self.sides[side][*other]
                    .as_ref()
                    .is_some_and(|unit| unit.zoid == zoid)
            })
            .collect();
        if same.len() > 1
            && let Some(position) = same.iter().position(|other| *other == slot)
        {
            calls.push(Call::Text(
                TEXT_LETTERS + u16::try_from(position).unwrap_or(0),
            ));
        }
        calls
    }

    /// Ends the actor's turn (`0x1770`): the beaten units leave the
    /// battle, then the next actor, or the next round once all have
    /// acted, or the end.
    fn end_actor(&mut self) -> Stage {
        if let Some((PARTY, slot)) = self.actor() {
            self.refresh_panel(slot);
            self.light_panel(slot);
        }
        for unit in self.sides.iter_mut().flatten().flatten() {
            if unit.traits & DESTROYED != 0 {
                unit.traits |= OUT;
            }
        }
        if self.battle_over() {
            return Stage::Over;
        }
        self.fight.index += 1;
        if self.fight.index < self.fight.order.len() {
            return Stage::Actor;
        }
        self.fight.index = 0;
        self.end_round();
        Stage::RoundMenu
    }

    /// The round's end (`0x0802F5C4`): the effects count a turn down and
    /// the units' round flags clear.
    fn end_round(&mut self) {
        for unit in self.sides.iter_mut().flatten().flatten() {
            for effect in &mut unit.effects {
                effect.turns = effect.turns.saturating_sub(1);
            }
            unit.status = 0;
        }
    }

    /// Stages the attack the actor chose (`0x0802BBC8`, `0x08042348`): the
    /// party's, outside the story's battles, with the player's aim.
    fn start_scene(&mut self, rom: &[u8]) {
        let Some((side, slot)) = self.actor() else {
            return;
        };
        let Some(attacker) = self.scene_unit(rom, side, slot) else {
            return;
        };
        let aim = (side == PARTY && !self.story).then(|| self.aim_setup(rom, slot, &attacker));
        let (weapon, targets) = match (&aim, self.fight.choice.clone()) {
            (
                None,
                Some(Choice::Weapon {
                    weapon,
                    side: aimed,
                    targets,
                }),
            ) => (
                weapon,
                targets
                    .iter()
                    .filter_map(|target| self.scene_unit(rom, aimed, *target))
                    .collect(),
            ),
            (None, _) => return,
            (Some(_), _) => (0, Vec::new()),
        };
        let data = GameData::new(rom);
        self.fight.scene = Some(AttackScene::new(
            &data,
            Attack {
                attacker,
                weapon,
                targets,
                story: self.story,
                roll: self.fight.rolls[0],
                aim,
            },
        ));
    }

    /// The scene record of `side`'s unit in `slot` (`0x08041AA8`).
    fn scene_unit(&self, rom: &[u8], side: usize, slot: usize) -> Option<SceneUnit> {
        let terrain = if side == PARTY {
            self.terrains.0
        } else {
            self.terrains.1
        };
        self.sides[side][slot]
            .as_ref()
            .map(|unit| SceneUnit::new(rom, unit, side == ENEMY, slot, terrain))
    }

    /// What the player's aim chooses from: the attacker's weapons with
    /// their figures and chances, and the units still fighting.
    fn aim_setup(&self, rom: &[u8], slot: usize, attacker: &SceneUnit) -> AimSetup {
        /// Targets hidden behind `????` from chapter 6 on.
        const HIDDEN: u16 = 0x200;
        let unit = self.sides[PARTY][slot].as_ref();
        let stats = unit.map(super::units::BattleUnit::derived);
        let units: [[Option<AimUnit>; SLOTS]; 2] = std::array::from_fn(|side| {
            std::array::from_fn(|target| {
                let other = self.sides[side][target]
                    .as_ref()
                    .filter(|other| other.fighting())?;
                let hp = if side == ENEMY && other.traits & HIDDEN != 0 {
                    UNKNOWN_HP
                } else {
                    u16::try_from(other.hp.max(0)).unwrap_or(u16::MAX)
                };
                Some(AimUnit {
                    zoid: other.zoid,
                    hp,
                    view: self.scene_unit(rom, side, target)?,
                })
            })
        });
        let weapons = std::array::from_fn(|index| {
            let (unit, stats) = (unit?, stats?);
            let weapon = unit.weapons[index]?;
            let chances = std::array::from_fn(|side| {
                std::array::from_fn(|target| {
                    self.sides[side][target].as_ref().map_or(0, |other| {
                        super::units::hit_chance(
                            (unit, &stats),
                            (other, &other.derived()),
                            index,
                            self.terrains.0,
                        )
                    })
                })
            });
            let power = if weapon.power > 0xFFFF {
                let power = stats.power[index];
                u16::try_from((power >> 16) + ((power >> 15) & 1)).unwrap_or(u16::MAX)
            } else {
                0
            };
            Some(AimWeapon {
                part: weapon.part,
                power,
                accuracy: u16::try_from(weapon.accuracy.max(0)).unwrap_or(0),
                code: weapon.spread,
                cost: u16::try_from(weapon.cost.max(0)).unwrap_or(0),
                flags: weapon.flags,
                chances,
            })
        });
        AimSetup {
            zoid: unit.map_or(0, |unit| unit.zoid),
            slot,
            ep: unit.map_or(0, |unit| unit.ep),
            parts: attacker.parts,
            weapons,
            units,
            memory: self.fight.memory[slot],
        }
    }

    /// The scene's frame, with the attack applied when it asks.
    pub(super) fn update_scene(
        &mut self,
        rom: &[u8],
        input: Input,
        windows: &mut ScriptWindows<'_>,
    ) -> Result<(), ScriptError> {
        let Some(mut scene) = self.fight.scene.take() else {
            return Ok(());
        };
        let data = GameData::new(rom);
        scene.update(&data, input, windows)?;
        for event in scene.take_events() {
            match event {
                SceneEvent::Apply => {
                    let hits = self.apply_attack();
                    scene.set_hits(hits);
                }
                SceneEvent::Aimed {
                    weapon,
                    side,
                    targets,
                } => {
                    self.fight.choice = Some(Choice::Weapon {
                        weapon,
                        side,
                        targets,
                    });
                }
                SceneEvent::Remember(memory) => {
                    if let Some((PARTY, slot)) = self.actor() {
                        self.fight.memory[slot] = Some(memory);
                    }
                }
                SceneEvent::Cancelled => self.fight.cancelled = true,
                SceneEvent::Done => {}
            }
        }
        self.sounds.extend(scene.take_sounds());
        self.fight.scene = Some(scene);
        Ok(())
    }

    /// Applies the chosen attack (`0x08046918`, `0x08033E40`).
    fn apply_attack(&mut self) -> Vec<Hit> {
        let (
            Some(actor),
            Some(Choice::Weapon {
                weapon, targets, ..
            }),
        ) = (self.actor(), self.fight.choice.clone())
        else {
            return Vec::new();
        };
        let terrain = if actor.0 == PARTY {
            self.terrains.0
        } else {
            self.terrains.1
        };
        let outcome = attack::attack(
            &mut self.sides,
            actor,
            weapon,
            &targets,
            &self.fight.rolls,
            (0, terrain, self.chapter),
        );
        self.fight.experience = self.fight.experience.wrapping_add(outcome.experience);
        self.fight.money = self.fight.money.wrapping_add(outcome.money);
        self.fight.blows = outcome.blows;
        self.fight
            .blows
            .iter()
            .map(|blow| Hit {
                landed: blow.landed(),
                critical: blow.critical(),
                destroyed: blow.destroyed(),
            })
            .collect()
    }

    /// One step of the fight's slot-5 task, when no script is running.
    pub(super) fn step_fight_task(&mut self) {
        self.task = match self.task {
            // 0x0802F09C: each side's front row is checked, two frames a
            // side; a side whose front row is empty would move its back
            // row up, which the port does not model yet.
            Task::Rows(frame) => {
                if frame >= 4 {
                    Task::Reported
                } else {
                    Task::Rows(frame + 1)
                }
            }
            // 0x0802FD4C: the effects that ran out; without any it reports
            // on its second frame.
            Task::Expiry(frame) => {
                if frame >= 1 {
                    Task::Reported
                } else {
                    Task::Expiry(frame + 1)
                }
            }
            Task::Status(frame) => {
                let slots = u32::try_from(2 * SLOTS).unwrap_or(0);
                if frame >= slots * STATUS_FRAMES_PER_SLOT {
                    Task::Reported
                } else {
                    Task::Status(frame + 1)
                }
            }
            Task::Action(step) => self.step_action(step),
            Task::Return(frame) => self.step_return(frame),
            other => other,
        };
    }

    /// The action task (`0x0802E814`).
    fn step_action(&mut self, step: ActionStep) -> Task {
        match step {
            ActionStep::Message => {
                self.dim_all_but_actor();
                let mut calls = Vec::new();
                if !self.actor_is_party() {
                    calls.push(Call::Label(LABEL_ENEMY));
                }
                calls.extend(self.unit_name(self.actor()));
                calls.push(Call::Text(TEXT_ACTION));
                self.acts.push_back(Act::Call(Call::Menu(MENU_DRAW)));
                self.acts.push_back(Act::Call(Call::Menu(MENU_CLEAR)));
                for call in calls {
                    self.acts.push_back(Act::Call(call));
                }
                self.acts.push_back(Act::Call(Call::Menu(MENU_PRESENT)));
                Task::Action(if self.actor_is_party() {
                    ActionStep::Menu
                } else {
                    ActionStep::Choose
                })
            }
            ActionStep::Menu => {
                self.acts.push_back(Act::Call(Call::Menu(MENU_ACTION)));
                Task::Action(ActionStep::AwaitMenu)
            }
            ActionStep::AwaitMenu => Task::Action(ActionStep::AwaitMenu),
            ActionStep::Choose => {
                let Some((side, slot)) = self.actor() else {
                    return Task::Reported;
                };
                let terrain = if side == PARTY {
                    self.terrains.0
                } else {
                    self.terrains.1
                };
                let choice =
                    ai::choose(&self.sides, side, slot, terrain, &mut self.rng, self.vblank);
                self.fight.action = Some(match choice {
                    Choice::Defend => ACTION_DEFEND,
                    Choice::Weapon { .. } => ACTION_ATTACK,
                });
                self.fight.choice = Some(choice);
                Task::Reported
            }
        }
    }

    /// The return to the screen (`0x0802BCA4`): rebuilt while black, faded
    /// in, then the hit display and the messages.
    fn step_return(&mut self, frame: u32) -> Task {
        let panels = u32::try_from(self.panels.len()).unwrap_or(0);
        let build = RETURN_BUILD + RETURN_PER_PANEL * panels;
        if frame == 0 {
            self.level = super::BLACK;
            self.restore_units();
            self.dim_panels = super::PanelLight::Own;
            self.acts.push_back(Act::Call(Call::Menu(MENU_RESET)));
        }
        if frame + 3 == build {
            self.acts
                .push_back(Act::Call(Call::Menu(MENU_MESSAGE_WINDOW)));
        }
        if frame == build {
            self.fade = Some(Fade { out: false, age: 0 });
        }
        let faded = build + super::FADE_SEEN_AGE;
        if frame == faded {
            self.fade = None;
        }
        if frame == faded + 2 {
            self.start_report();
        }
        if frame > faded + 2
            && self.fight.report == Some(Report::Done)
            && self.fight.display.is_none()
        {
            self.fight.report = Some(Report::Closing(0));
        }
        if let Some(Report::Closing(tail)) = self.fight.report {
            if tail >= RETURN_TAIL {
                self.fight.report = None;
                return Task::Reported;
            }
            self.fight.report = Some(Report::Closing(tail + 1));
            return Task::Return(frame + 1);
        }
        if frame > faded + 2 {
            self.step_report();
        }
        Task::Return(frame + 1)
    }

    /// The hit display and the first message (`0x0802D36C`, `0x0802C084`).
    fn start_report(&mut self) {
        let sparks: Vec<Spark> = self
            .fight
            .blows
            .iter()
            .filter(|blow| blow.landed())
            .map(|blow| Spark {
                side: blow.side,
                slot: blow.slot,
                delay: SPARK_DELAY,
                frame: 0,
                step: 0,
                ticks: self
                    .spark
                    .as_ref()
                    .and_then(|sprite| sprite.animation.first())
                    .map_or(1, |step| step.duration.max(1)),
                ended: false,
                shown: false,
            })
            .collect();
        if !sparks.is_empty() {
            self.sounds.push(HIT_SOUND);
            // Loading the spark's graphics (`0x08032134`) holds up the
            // main loop for the next frame.
            self.lag = 1;
            self.fight.display = Some(Display {
                sparks,
                frames: 0,
                tail: None,
            });
        }
        self.fight.report = Some(Report::Target(0));
        self.report_target(0);
    }

    fn report_target(&mut self, index: usize) {
        let Some(blow) = self.fight.blows.get(index).copied() else {
            self.fight.report = Some(Report::Tail(0));
            return;
        };

        let mut calls = self.unit_name(Some((blow.side, blow.slot)));
        if blow.landed() {
            let (before, after) = if blow.critical() {
                (TEXT_CRITICAL, TEXT_CRITICAL_DAMAGE)
            } else {
                (TEXT_IS, TEXT_DAMAGE)
            };
            calls.push(Call::Text(before));
            calls.extend(number(blow.damage >> 16));
            calls.push(Call::Text(after));
        } else {
            calls.push(Call::Text(TEXT_DODGED));
        }
        self.message_then(
            &calls,
            (blow.side == PARTY).then_some(Act::Panel(blow.slot)),
        );
        self.fight.report = Some(Report::Target(index));
    }

    fn step_report(&mut self) {
        self.step_display();
        let busy = self.running.is_some() || !self.acts.is_empty();
        let report = self.fight.report;
        let waited = !busy
            && matches!(report, Some(Report::Target(_) | Report::Destroyed(_)))
            && self.wait_done();
        match report {
            Some(Report::Target(index)) if waited => {
                let destroyed = self.fight.blows.get(index).is_some_and(Blow::destroyed);
                if destroyed {
                    let blow = self.fight.blows[index];
                    let mut calls = self.unit_name(Some((blow.side, blow.slot)));
                    calls.push(Call::Text(TEXT_DESTROYED));
                    self.message(&calls);
                    self.fight.report = Some(Report::Destroyed(index));
                } else if index + 1 < self.fight.blows.len() {
                    self.report_target(index + 1);
                } else {
                    self.fight.report = Some(Report::Tail(0));
                }
            }
            Some(Report::Destroyed(index)) if waited => {
                if index + 1 < self.fight.blows.len() {
                    self.report_target(index + 1);
                } else {
                    self.fight.report = Some(Report::Tail(0));
                }
            }
            Some(Report::Tail(frame)) => {
                self.fight.report = Some(if frame + 1 >= MESSAGE_TAIL {
                    Report::Done
                } else {
                    Report::Tail(frame + 1)
                });
            }
            _ => {}
        }
    }

    /// Darkens every unit but the actor (`0x08031D50`): by 12 in each
    /// channel, 24 for a paralysed one.
    fn dim_all_but_actor(&mut self) {
        let actor = self.actor();
        for side in [PARTY, ENEMY] {
            for slot in 0..SLOTS {
                let paralysed = self.sides[side][slot]
                    .as_ref()
                    .is_some_and(|unit| unit.traits & PARALYSED != 0);
                self.dims[side][slot] = if paralysed {
                    DIM_PARALYSED
                } else if actor == Some((side, slot)) {
                    0
                } else {
                    DIM_OTHERS
                };
            }
        }
    }

    /// The units' own colors again, the paralysed ones darkened
    /// (`0x08031C64`).
    fn restore_units(&mut self) {
        for side in [PARTY, ENEMY] {
            for slot in 0..SLOTS {
                let paralysed = self.sides[side][slot]
                    .as_ref()
                    .is_some_and(|unit| unit.traits & PARALYSED != 0);
                self.dims[side][slot] = if paralysed { DIM_PARALYSED } else { 0 };
            }
        }
    }

    /// Dims every panel but party slot `slot`'s (`0x08031598`).
    pub(super) fn light_panel(&mut self, slot: usize) {
        self.dim_panels = super::PanelLight::Dimmed {
            lit: self.panels.iter().position(|panel| panel.slot == slot),
        };
    }

    /// Shows a party unit's hit points and energy on its panel
    /// (`0x08031618`, `0x08031598`).
    pub(super) fn refresh_panel(&mut self, slot: usize) {
        let Some(unit) = self.sides[PARTY][slot].as_ref() else {
            return;
        };
        let value = |current: i32, full: i32| {
            (
                u32::try_from(current.max(0)).unwrap_or(0),
                u32::try_from(full.max(1)).unwrap_or(1),
            )
        };
        let (hp, ep) = (value(unit.hp, unit.max_hp), value(unit.ep, unit.max_ep));
        for panel in self.panels.iter_mut().filter(|panel| panel.slot == slot) {
            panel.hp = hp;
            panel.ep = ep;
        }
    }

    /// A frame of the hit display: the sparks play, and the display ends a
    /// few frames after the last has gone.
    fn step_display(&mut self) {
        let Some(mut display) = self.fight.display.take() else {
            return;
        };
        display.frames += 1;
        if let Some(sprite) = self.spark.as_ref() {
            for spark in &mut display.sparks {
                spark.advance(sprite);
                if spark.ended && spark.frame >= SHAKES * 4 {
                    spark.shown = false;
                }
            }
        }
        if display.tail.is_none()
            && display
                .sparks
                .iter()
                .all(|spark| !spark.shown && spark.delay == 0)
        {
            display.tail = Some(0);
        }
        if let Some(tail) = display.tail.as_mut() {
            *tail += 1;
            if *tail > DISPLAY_TAIL {
                return;
            }
        }
        self.fight.display = Some(display);
    }

    /// Where a menu that the fight shows returns to, in the frame it
    /// returns: the round's menu moves the controller on, the action menu
    /// ends the action task.
    pub(super) fn choose_in_fight(&mut self, line: u16) {
        if self.controller == Controller::Fight(Stage::AwaitRoundMenu) {
            let next = match line {
                ROUND_FIGHT => Stage::RoundStart,
                ROUND_RETREAT => {
                    self.outcome = Some(Outcome::Retreated);
                    Stage::Over
                }
                _ => Stage::RoundMenu,
            };
            self.controller = Controller::Fight(next);
            return;
        }
        if self.task == Task::Action(ActionStep::AwaitMenu) {
            // The weapon and the targets are chosen in the scene.
            self.fight.action = Some(line);
            self.fight.choice = None;
            self.task = Task::Reported;
        }
    }

    /// Whether the fight's slot-5 task wants its step this frame.
    pub(super) fn fight_task_active(&self) -> bool {
        !matches!(self.task, Task::Idle | Task::Reported)
    }
}

/// The calls that print `value` (`0x08001848` with 7 digits and no
/// padding): its digits from the first that is not 0.
pub(super) fn number(value: i32) -> Vec<Call> {
    number_in(value, 7, 1)
}

/// The calls that print `value` in its last `digits` of seven places
/// (`0x08001848`): mode bit 0 leaves out the leading zeros, bit 1 prints
/// them, and without either they are spaces; bit 2 puts ＋ before, and a
/// value below 0 gets －.
pub(super) fn number_in(value: i32, digits: u8, mode: u8) -> Vec<Call> {
    const PLACES: usize = 7;
    const PLUS: u16 = 10;
    const MINUS: u16 = 11;
    const SPACE: u16 = 12;
    let digits = usize::from(digits.min(7));
    let magnitude = value.unsigned_abs();
    let mut places = [0u16; PLACES];
    let mut rest = magnitude;
    for (at, place) in places.iter_mut().enumerate() {
        let unit = 10u32.pow(u32::try_from(PLACES - 1 - at).unwrap_or(0));
        *place = u16::try_from(rest / unit).unwrap_or(u16::MAX);
        rest %= unit;
    }
    if places[0] > 9 {
        places = [9; PLACES];
    }
    let mut calls = Vec::new();
    if value < 0 {
        calls.push(Call::Text(TEXT_DIGITS + MINUS));
    } else if mode & 4 != 0 {
        calls.push(Call::Text(TEXT_DIGITS + PLUS));
    }
    let mut started = false;
    for (at, &digit) in places.iter().enumerate().skip(PLACES - digits) {
        if digit != 0 || started || at == PLACES - 1 {
            calls.push(Call::Text(TEXT_DIGITS + digit));
            started = true;
        } else if mode & 1 == 0 {
            calls.push(Call::Text(
                TEXT_DIGITS + if mode & 2 == 0 { SPACE } else { 0 },
            ));
        }
    }
    calls
}

/// The fight's panels and grounds, moving once the party engages.
pub(super) fn engage_scrolls(frame: u32) -> (i32, i32) {
    let panels =
        (PANELS_HIDDEN - PANEL_SLIDE * i32::try_from(frame + 1).unwrap_or(i32::MAX)).max(0);
    let grounds = -(i32::try_from(frame + 1)
        .unwrap_or(i32::MAX)
        .min(GROUNDS_DOWN));
    (panels, grounds)
}

/// Frames of the engage step: the slide, then its wait.
pub(super) fn engage_frames() -> u32 {
    u32::try_from(PANELS_HIDDEN / PANEL_SLIDE).unwrap_or(0) + ENGAGE_WAIT
}
