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
//! screen (`0x0802BCA4`), its messages (`0x0802C084`, `0x0802CE98` after
//! a support part) and displays (`0x0802D36C`, `0x0802CA54`), the name
//! routine (`0x080339C4`), the number routine
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
use extraction::saga_encounter::StoryBattle;
use platform::Input;

/// The frames the panels slide down in once the party engages: two pixels
/// a frame from 16 (`0x0802E9CC`, state `0x2328`).
/// The player's character, the protagonist.
const PROTAGONIST: u8 = 0;
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
const RETURN_BUILD: u32 = 17;
const RETURN_PER_PANEL: u32 = 8;
/// The frames the rebuild after a given-up aim takes besides
/// [`RETURN_PER_PANEL`] a panel, and how many before its end the message
/// window opens.
const REBUILD: u32 = 18;
const REBUILD_MESSAGE_WINDOW: u32 = 2;
/// The frames a sacrifice's explosions take after their end
/// (`0x0802DCB4`).
const DISPLAY_TAIL: u32 = 7;
/// Frames before the display's task first runs a spark.
const SPARK_DELAY: u32 = 1;
/// Frames a hit's spark task takes to report once its animation is over:
/// it sees the end in one state (200) and hides and reports in the next
/// (900, `0x0802D9A4`), a frame after the sprite system flags it.
const SPARK_REPORT_LAG: u32 = 2;
/// The spark a hit unit shows (the battle screen's effect 6), placed 8
/// pixels right of and 16 above the unit (`0x0802D9A4`).
pub(super) const SPARK: usize = 6;
const SPARK_OFFSET: (i32, i32) = (8, -16);
/// A sacrificed unit's explosion (the display's effect 1, which the table
/// at ROM `0x66BA0C` makes record 27 of the table at `0x6F8174`, the
/// effects' 181st), where a hit's spark goes, 8 pixels right of and 16
/// above the unit's anchor; the unit vanishes on its 24th frame
/// (`0x0802DBE4`).
pub(super) const BLAST: usize = 154 + 27;
const BLAST_VANISH: u32 = 24;
/// The glow a unit a support part raised shows on its anchor (the battle
/// screen's effect record 2, `0x0802CBBC`), and a repaired one's (record 4,
/// `0x0802C588`).
pub(super) const GLOW: usize = 2;
pub(super) const MEND: usize = 4;
/// The glows of a lowered statistic (the display's effect 5, which the
/// table at ROM `0x66BA0C` maps to screen record 3, `0x0802CD38`) and of a
/// unit a command stops (effect 3, screen record 1, `0x0802DFDC`).
pub(super) const LOWER: usize = 3;
pub(super) const STOP: usize = 1;
/// A paralysed unit's display (effect 6, screen record 0, `0x0802DAF4`):
/// its colors darken by 2 a frame from 2 to 22 while its animation plays.
pub(super) const STUN: usize = 0;
const STUN_LAST: u32 = 22;
const STUN_FRAMES: u32 = 11;
/// A revived unit's display (effect 7, screen record 5, `0x0802E618`):
/// the unit shows again on its 29th frame, its colors raised by 32, then
/// by half a level less a frame down to its own; sound `0x56`.
pub(super) const REVIVE: usize = 5;
const REVIVE_SHOWN: u32 = 29;
const REVIVE_WHITE: u32 = 32;
const REVIVE_FALL_END: u32 = REVIVE_SHOWN + 2 * REVIVE_WHITE - 1;
pub(super) const REVIVE_SOUND: u16 = 0x56;
/// A beaten unit's explosion's sound, by the largest size of the beaten
/// ones (`0x0802D36C` state 4000).
const BEATEN_SOUNDS: [u16; 3] = [0x5B, 0x5C, 0x5D];
/// The glows' sounds (`0x0802CA54`, `0x0802C420`, `0x0802C8EC`,
/// `0x0802DE74`).
const GLOW_SOUND: u16 = 0x57;
const MEND_SOUND: u16 = 0x55;
const LOWER_SOUND: u16 = 0x58;
const STOP_SOUND: u16 = 0x4B;
/// A glow's frames: the unit's colors change from 8 by 2 a frame to 30,
/// then back to 0; the glow hides once both it and its animation are done,
/// and its task reports a frame later (`0x0802CBBC`). A repair's starts a
/// frame later (`0x0802C588` writes no colors in its first frame).
const GLOW_FIRST: u32 = 8;
const GLOW_STEP: u32 = 2;
const GLOW_RISE: u32 = 12;
const GLOW_FALL_END: u32 = 27;
/// A lowered statistic's glow starts from 0 (`0x0802CD38` leaves its level
/// as the entity starts), so it rises for 16 frames.
const LOWER_RISE: u32 = 16;
const LOWER_FALL_END: u32 = 31;
/// The glow display's frames once its glows are done (`0x0802CA54`,
/// states 100 to `0x38E`).
const GLOW_TAIL: u32 = 2;
/// How a hit unit shakes, a step a frame, three times (`0x0802D9A4`).
const SHAKE: [i32; 4] = [-2, 0, 2, 0];
const SHAKES: u32 = 3;
/// Frames the messages' task takes after its last wait before it reports
/// (`0x0802C084`, states 2000 to `0x2328`).
const MESSAGE_TAIL: u32 = 5;
/// The return's frames after it killed the display's task, before it
/// reports.
const RETURN_TAIL: u32 = 1;
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
/// The actor's menu without アイテム (`0x0802E814`: a story battle's
/// mode 1).
pub(super) const MENU_ACTION_NO_ITEMS: u16 = 0x1A;
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
/// はダメージを一切受けない！！ and の運動性能が上がった！！.
const TEXT_UNTOUCHABLE: u16 = 81;
const TEXT_RESTORED: u16 = 82;
/// は頭がクラクラした, after a pilot's name.
const TEXT_DAZED: u16 = 79;
/// はマヒしてしまった.
const TEXT_PARALYSED: u16 = 80;
const TEXT_ROUND: u16 = 0x3B;
/// The letters that tell same Zoids apart: `battle-text` 88 on.
const TEXT_LETTERS: u16 = 88;
/// The digits: `battle-text` 94 on.
const TEXT_DIGITS: u16 = 94;
/// A support part's message (`0x0802CE98`): the unit, の, what changed,
/// the amount, then 上がった or 下がった.
const TEXT_OF: u16 = 63;
/// の効果が無くなりました, after the part's name (`0x0802FD4C`).
const TEXT_EXPIRED: u16 = 87;
/// The parts the expiry names at most.
const EXPIRED_PARTS: usize = 36;
/// Frames between an expiry message's wait and the next message or the
/// report (states `0x514` and `0x44C` or `0x2328`).
const EXPIRY_GAP: u32 = 1;
const TEXT_ROSE: u16 = 72;
/// A repair's message (`0x0802C708`): ＨＰが, then 回復した.
const TEXT_HIT_POINTS: u16 = 69;
const TEXT_REPAIRED: u16 = 74;
const TEXT_FELL: u16 = 73;
/// What changed, by the first of the effect's bits the message finds:
/// 攻撃力, 命中率, 総合防御, 物理防御, レーザー防御, スピード, 回避率.
const CHANGE_TEXTS: [(u16, u16); 7] = [
    (0x10, 64),
    (0x20, 66),
    (0x100, 65),
    (0x200, 85),
    (0x400, 86),
    (0x800, 67),
    (0x8000, 68),
];
/// "敵ゾイド　", the enemy's actions' prefix.
const LABEL_ENEMY: u16 = 16;
/// The round menu's lines.
/// A weapon's 「格闘」 flag.
const MELEE_FLAG: u32 = 0x100;
const ROUND_FIGHT: u16 = 0;
const ROUND_COMMAND: u16 = 1;
const ROUND_RETREAT: u16 = 2;
/// The action menu's lines.
const ACTION_ATTACK: u16 = 0;
const ACTION_DEFEND: u16 = 1;
const ACTION_ITEM: u16 = 2;
/// The unit is defending: it takes half the damage until its next action.
const DEFENDING: u16 = 0x100;
/// The unit is paralysed: it can't act.
pub(super) const PARALYSED: u16 = 0x4000;
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
    /// 0x157C and 0x15E0: an item (the task `0x08038FC4`); once used the
    /// actor's turn ends, given up it acts again.
    Item,
    AwaitItem,
    /// The round's コマンド: the command task (states 8000, `0x1FA4`).
    Command,
    AwaitCommand,
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
            Stage::Item => 0x157C,
            Stage::AwaitItem => 0x15E0,
            Stage::Command => 8000,
            Stage::AwaitCommand => 0x1FA4,
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
    /// The row advance (`0x0802F09C`).
    Rows(super::rows::Rows),
    /// The effects' expiry (`0x0802FD4C`).
    Expiry(u32),
    /// Its message for the `n`th part whose effects ran out, then the
    /// frames after its wait.
    Expired(usize, Option<u32>),
    /// The actor's status check (`0x0802FAA8`).
    Status(u32),
    /// The action task (`0x0802E814`).
    Action(ActionStep),
    /// The return to the screen (`0x0802BCA4`).
    Return(u32),
    /// The item task (`0x08038FC4`).
    Item(super::items::ItemStep),
    /// The command task (`0x0803B7D0`).
    Command(super::commands::CommandStep),
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
    /// The message of a support part's target `n`.
    Support(usize),
    /// The message of target `n` of a part that sets a state or repairs in
    /// full.
    State(usize),
    /// The message of a repaired target `n`.
    Repair(usize),
    /// An item's message, waiting for its wait.
    Item,
    /// What follows target `n`'s message, a state a frame (`0x0802C084`
    /// states 2000 to `0x2328`).
    Check(usize, Check),
    /// A line about target `n` after its message, waiting for its wait
    /// (states `0x834`, `0xC1C` and `0x17D4`), and the check after it.
    Line(usize, Check),
    /// The frames after the last wait.
    Tail(u32),
    /// The messages task reported.
    Done,
    /// Its task killed, the return waits for the display's (state
    /// `0x1BBC`).
    Awaiting,
    /// The return's own frames once both tasks are killed (states 8000 to
    /// `0x2328`).
    Closing(u32),
}

/// The hit display's task (`0x0802D36C`): the hit units' sparks, then the
/// dazed pilots' units' glows, the paralysed units' darkening and the
/// beaten units' explosions, a stage each. A stage ends as soon as its
/// first unit's task reports; the next starts the frame after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HitStep {
    /// A stage's units' tasks run (states 100, `0x44C`, `0x834`,
    /// `0x1004`).
    Running(HitStage),
    /// A stage starts: the last one's tasks killed, its graphics loaded
    /// (`0x08032134`) and its units' tasks spawned (states 1000, 2000,
    /// 4000); with none, the next starts the frame after.
    Start(HitStage),
    /// The task reports (state `0x2328`).
    Reporting,
}

/// The hit display's stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HitStage {
    Sparks,
    /// Pilots a weapon dazed (the blow's flag 8): a lowered statistic's
    /// glow (`0x0802CD38`).
    Dazed,
    /// Units a weapon paralysed (flag 4, `0x0802DAF4`).
    Stunned,
    /// Beaten units (flag `0x8000`): their explosions (`0x0802DBE4`).
    Beaten,
}

impl HitStage {
    fn next(self) -> Self {
        match self {
            HitStage::Sparks => HitStage::Dazed,
            HitStage::Dazed => HitStage::Stunned,
            HitStage::Stunned | HitStage::Beaten => HitStage::Beaten,
        }
    }
}

/// The messages task's steps after a target's message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Check {
    /// State 2000: its pilot's daze.
    Pilot,
    /// State `0xBB8`: its paralysis.
    Paralysis,
    /// State `0x1770`: its destruction.
    Beaten,
    /// State `0x1B58`: the next target, or the end.
    Count,
    /// State 1000: the next target's message.
    Print,
    /// State `0x2328`: the task reports.
    Last,
}

/// The hit display (`0x0802D36C`): each hit unit's spark and shake, then
/// its own frames.
pub(super) struct Display {
    pub(super) sparks: Vec<Spark>,
    frames: u32,
    tail: Option<u32>,
}

/// A hit unit's spark (`0x0802D9A4`), or a raised or repaired one's glow
/// (`0x0802CBBC`, `0x0802C588`).
#[derive(Clone)]
pub(super) struct Spark {
    pub(super) side: usize,
    pub(super) slot: usize,
    /// For a glow, its kind and the frames since its display started.
    pub(super) glow: Option<(Glow, u32)>,
    /// The frame a glow hid in, once both its colors and its animation
    /// were done; its task reports the frame after.
    hidden: Option<u32>,
    /// Frames until the display's task first runs it.
    delay: u32,
    frame: u32,
    step: usize,
    ticks: u32,
    ended: bool,
    shown: bool,
    /// A unit a command sacrifices: its explosion (`0x0802DBE4`).
    pub(super) blast: bool,
}

/// The screen effects the displays show: a hit's spark, the glows, and a
/// sacrifice's explosion.
pub(super) struct EffectSprites {
    pub(super) spark: Option<EffectSprite>,
    pub(super) glow: Option<EffectSprite>,
    pub(super) mend: Option<EffectSprite>,
    pub(super) lower: Option<EffectSprite>,
    pub(super) stop: Option<EffectSprite>,
    pub(super) stun: Option<EffectSprite>,
    pub(super) revive: Option<EffectSprite>,
    pub(super) blast: Option<EffectSprite>,
}

impl EffectSprites {
    pub(super) fn load(rom: &[u8]) -> Self {
        use extraction::saga_battle::{effect_sprite, screen_effect};
        Self {
            spark: screen_effect(rom, SPARK),
            glow: screen_effect(rom, GLOW),
            mend: screen_effect(rom, MEND),
            lower: screen_effect(rom, LOWER),
            stop: screen_effect(rom, STOP),
            stun: screen_effect(rom, STUN),
            revive: screen_effect(rom, REVIVE),
            blast: effect_sprite(rom, BLAST),
        }
    }
}

/// What a glow shows (`0x08031E90`'s flags).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Glow {
    /// A raised statistic: red raised then lowered (6, then 5).
    Raise,
    /// A repair: blue raised, red and green lowered (`0x12`, then `0xD`).
    Mend,
    /// A lowered statistic: red and blue raised, green lowered (`0x16`,
    /// then 9).
    Lower,
    /// A unit a command stops: every channel lowered (`0x1D`).
    Stop,
    /// A unit a hit paralyses: every channel lowered, and kept so.
    Stun,
    /// A revived unit: hidden, then every channel raised, falling back.
    Revive,
}

impl Glow {
    /// Frames its task takes before its first colors: a repair's and a
    /// stop's write none in their first frame (`0x0802C588`, `0x0802DFDC`).
    fn lead(self) -> u32 {
        match self {
            Glow::Mend | Glow::Stop => 1,
            Glow::Raise | Glow::Lower | Glow::Stun | Glow::Revive => 0,
        }
    }

    /// Frames its task takes to report once it hid: a lowered statistic's
    /// reports at once (`0x0802CD38`), the others a frame later.
    fn report_lag(self) -> u32 {
        match self {
            Glow::Lower | Glow::Stun => 0,
            Glow::Raise | Glow::Mend | Glow::Stop | Glow::Revive => 1,
        }
    }

    /// Its first level, the frames it rises for, and the frame its colors
    /// are back.
    fn levels(self) -> (u32, u32, u32) {
        match self {
            Glow::Lower => (0, LOWER_RISE, LOWER_FALL_END),
            Glow::Stun => (GLOW_STEP, STUN_FRAMES, STUN_FRAMES - 1),
            Glow::Revive => (REVIVE_WHITE, REVIVE_SHOWN, REVIVE_FALL_END),
            Glow::Raise | Glow::Mend | Glow::Stop => (GLOW_FIRST, GLOW_RISE, GLOW_FALL_END),
        }
    }

    /// The flags of `0x08031E90` it writes the colors with, in turn.
    pub(super) fn flags(self) -> &'static [u8] {
        match self {
            Glow::Raise => &[6, 5],
            Glow::Mend => &[0x12, 0xD],
            Glow::Lower => &[0x16, 9],
            Glow::Stop | Glow::Stun => &[0x1D],
            Glow::Revive => &[0x1E],
        }
    }
}

impl Spark {
    /// How far the unit is shaken this frame.
    pub(super) fn shake(&self) -> i32 {
        if self.glow.is_none() && !self.blast && self.shown && self.frame < SHAKES * 4 {
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
    pub(super) fn anchor(&self, (x, y): (i32, i32)) -> (i32, i32) {
        if self.glow.is_some() {
            (x, y)
        } else {
            (x + SPARK_OFFSET.0, y + SPARK_OFFSET.1)
        }
    }

    /// A glow's change of the unit's colors this frame (`0x08031E90`).
    pub(super) fn tint(&self) -> Option<(Glow, u8)> {
        let (glow, frame) = self.glow?;
        let frame = frame.checked_sub(glow.lead())?;
        let (first, rise, fall_end) = glow.levels();
        let amount = if glow == Glow::Stun {
            (first + GLOW_STEP * frame).min(STUN_LAST)
        } else if glow == Glow::Revive {
            // Raised by 32 as it shows, then by the whole part of 32 less
            // half a level each frame after; the task writes the colors
            // straight into the palette, so the frame shows them at once.
            let after = (frame + 1).checked_sub(rise)?;
            first.saturating_sub(after.div_ceil(2))
        } else if frame < rise {
            first + GLOW_STEP * frame
        } else if frame <= fall_end {
            GLOW_STEP * (fall_end - frame)
        } else {
            0
        };
        Some((glow, u8::try_from(amount).unwrap_or(0)))
    }

    /// Whether a sacrificed unit has vanished behind its explosion.
    pub(super) fn vanished(&self) -> bool {
        match self.glow {
            Some((Glow::Revive, frame)) => frame < REVIVE_SHOWN,
            _ => self.blast && self.frame >= BLAST_VANISH,
        }
    }

    /// Whether its task has reported.
    fn done(&self) -> bool {
        match (self.glow, self.hidden) {
            (Some((glow, frame)), Some(hidden)) => frame >= hidden + glow.report_lag(),
            (Some(_), None) => false,
            (None, Some(hidden)) => self.frame >= hidden + SPARK_REPORT_LAG,
            (None, None) => !self.shown && self.delay == 0,
        }
    }

    fn advance(&mut self, sprite: &EffectSprite) {
        if let Some((_, frame)) = self.glow.as_mut() {
            *frame += 1;
        }
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
    /// The port's debugging aid: the protagonist's attacks always land
    /// and beat what they hurt.
    pub(super) overpowered: bool,
    pub(super) order: Vec<(usize, usize)>,
    pub(super) index: usize,
    pub(super) rolls: [u16; ROLLS],
    pub(super) choice: Option<Choice>,
    pub(super) action: Option<u16>,
    pub(super) blows: Vec<Blow>,
    pub(super) scene: Option<AttackScene>,
    pub(super) report: Option<Report>,
    pub(super) display: Option<Display>,
    /// The hit display's task, while it runs.
    pub(super) hits: Option<HitStep>,
    /// The units' sprites moving up to the front row, and the sides whose
    /// back row moved up (`0x0200EB84` flags `0x80` and `0x100`).
    pub(super) row_moves: Vec<super::rows::RowMove>,
    pub(super) advanced: [bool; 2],
    /// The accuracy past 100 of the turn's last chance to hit that went
    /// past it (`0x0200EB84 + 0x229C`), which the critical hits read; the
    /// actor's turn clears it.
    pub(super) accuracy_excess: u16,
    /// The units an explosion took off the screen (`0x0802DBE4` clears
    /// their sprites' shown flag).
    pub(super) gone: [[bool; SLOTS]; 2],
    pub(super) experience: u32,
    pub(super) money: u32,
    /// The weapon each party slot last aimed.
    pub(super) memory: [Option<AimMemory>; SLOTS],
    /// The player gave the aim up: the screen comes back for the menu.
    pub(super) cancelled: bool,
    /// Frames the screen's rebuild has taken.
    pub(super) rebuild: u32,
    /// The parts whose effects ran out this round, for their messages.
    pub(super) expired: Vec<u16>,
}

impl Combat {
    /// The actor of the turn, as (side, slot).
    pub(super) fn actor(&self) -> Option<(usize, usize)> {
        self.fight.order.get(self.fight.index).copied()
    }

    /// The fight's song (`0x08033D6C`): a story battle's own.
    fn fight_song(&self) -> u16 {
        self.lineup
            .story()
            .map_or(FIGHT_SONG, |battle| u16::from(battle.song()))
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
                self.acts.push_back(Act::Music(self.fight_song()));
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
                self.apply_round_rules();
                self.task = Task::Rows(super::rows::Rows::Check(PARTY));
                Some(Stage::AwaitRows)
            }
            Stage::AwaitRows => self.take_task().then_some(Stage::Expiry),
            Stage::Expiry => {
                self.task = Task::Expiry(0);
                Some(Stage::AwaitExpiry)
            }
            Stage::AwaitExpiry => self.take_task().then_some(Stage::Order),
            Stage::Order => {
                // The round's commands may turn the order (`0x080324C4`).
                let order = if self.command_state.flags & super::commands::SLOWEST_FIRST != 0 {
                    ai::Order::Slowest
                } else if self.command_state.flags & super::commands::RANDOM_ORDER != 0 {
                    ai::Order::Shuffled
                } else {
                    ai::Order::Fastest
                };
                self.fight.order = ai::turn_order(&self.sides, order, &mut self.rng, self.vblank);
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
                    Some(ACTION_ITEM) => Stage::Item,
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
            Stage::Item | Stage::AwaitItem => self.step_item_stage(stage),
            Stage::Command | Stage::AwaitCommand => self.step_command_stage(stage),
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
        self.fight.accuracy_excess = 0;
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
        // Paralysed, or stopped by a command for the round (`0xBEA`).
        if unit.traits & PARALYSED != 0 || unit.status & super::commands::STOPPED != 0 {
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

    /// The parts whose effects have run out, each once, in the units'
    /// order: those effects forget their part (`0x0802FD4C`, state 0; at
    /// most 36).
    fn expire_effects(&mut self) -> Vec<u16> {
        let mut parts = Vec::new();
        for unit in self.sides.iter_mut().flatten().flatten() {
            if !unit.fighting() {
                continue;
            }
            for effect in &mut unit.effects {
                if effect.turns == 0 && effect.part != 0 {
                    if !parts.contains(&effect.part) && parts.len() < EXPIRED_PARTS {
                        parts.push(effect.part);
                    }
                    effect.part = 0;
                }
            }
        }
        parts
    }

    /// The message that part `index`'s effects have gone (state `0x44C`):
    /// its name and の効果が無くなりました, then the wait.
    fn expiry_message(&mut self, index: usize) -> Task {
        let part = self.fight.expired.get(index).copied().unwrap_or(0);
        self.message(&[Call::Part(part), Call::Text(TEXT_EXPIRED)]);
        Task::Expired(index, None)
    }

    /// Once the wait is over (state `0x4B0`), the next part's message two
    /// frames on, or the report (states `0x514`, `0x2328`).
    fn step_expired(&mut self, index: usize, after: Option<u32>) -> Task {
        let Some(frames) = after else {
            let busy = self.running.is_some() || !self.acts.is_empty();
            return if !busy && self.wait_done() {
                Task::Expired(index, Some(0))
            } else {
                Task::Expired(index, None)
            };
        };
        if frames < EXPIRY_GAP {
            return Task::Expired(index, Some(frames + 1));
        }
        if index + 1 < self.fight.expired.len() {
            self.expiry_message(index + 1)
        } else {
            self.fight.expired.clear();
            Task::Reported
        }
    }

    /// The round's end (`0x0802F5C4`): the effects count a turn down and
    /// the units' round flags clear.
    pub(super) fn end_round(&mut self) {
        for unit in self.sides.iter_mut().flatten().flatten() {
            for effect in &mut unit.effects {
                effect.turns = effect.turns.saturating_sub(1);
            }
            unit.status = 0;
        }
        self.command_state.flags = 0;
        self.restore_round_weapons();
    }

    /// What the round's commands leave for its fight (`0x0802F5D8`): no
    /// fighting weapons, or only them, and twice the energy for the units
    /// that spend it; the round's end gives the weapons back
    /// (`0x0802F81C`).
    pub(super) fn apply_round_rules(&mut self) {
        use super::commands::{DOUBLE_ENERGY, MELEE_ONLY, NO_MELEE};
        let flags = self.command_state.flags;
        let doubled = self
            .sides
            .iter()
            .flatten()
            .flatten()
            .any(|unit| unit.status & DOUBLE_ENERGY != 0);
        if flags & (NO_MELEE | MELEE_ONLY) == 0 && !doubled {
            return;
        }
        self.command_state.round_weapons = Some(std::array::from_fn(|side| {
            std::array::from_fn(|slot| {
                self.sides[side][slot]
                    .as_ref()
                    .map(|unit| unit.weapons)
                    .unwrap_or_default()
            })
        }));
        for unit in self.sides.iter_mut().flatten().flatten() {
            for weapon in &mut unit.weapons {
                let Some(held) = weapon else {
                    continue;
                };
                let melee = held.flags & MELEE_FLAG != 0;
                if (flags & NO_MELEE != 0 && melee)
                    || (flags & MELEE_ONLY != 0 && !melee && held.offensive())
                {
                    *weapon = None;
                }
            }
            if unit.status & DOUBLE_ENERGY != 0 {
                for weapon in unit.weapons.iter_mut().flatten() {
                    weapon.cost = weapon.cost.wrapping_shl(1);
                }
            }
        }
    }

    fn restore_round_weapons(&mut self) {
        let Some(saved) = self.command_state.round_weapons.take() else {
            return;
        };
        for (side, units) in saved.into_iter().enumerate() {
            for (slot, weapons) in units.into_iter().enumerate() {
                if let Some(unit) = self.sides[side][slot].as_mut() {
                    unit.weapons = weapons;
                }
            }
        }
    }

    /// Stages the attack the actor chose (`0x0802BBC8`, `0x08042348`): the
    /// party's with the player's aim.
    fn start_scene(&mut self, rom: &[u8]) {
        let Some((side, slot)) = self.actor() else {
            return;
        };
        let Some(attacker) = self.scene_unit(rom, side, slot) else {
            return;
        };
        let aim = (side == PARTY).then(|| self.aim_setup(rom, slot, &attacker));
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
                roll: self.fight.rolls[0],
                aim,
                staged: None,
                staged_reaction: None,
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
                code: weapon.kind(),
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

    /// Whether the actor is the protagonist while the port's debugging aid
    /// makes it overwhelming (see [`Combat::set_overpowered`]).
    fn overwhelms(&self, (side, slot): (usize, usize)) -> bool {
        self.fight.overpowered
            && side == PARTY
            && self.sides[side][slot]
                .as_ref()
                .is_some_and(|unit| unit.character == PROTAGONIST)
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
        let overwhelming = self.overwhelms(actor);
        let outcome = attack::attack(
            &mut self.sides,
            actor,
            weapon,
            &targets,
            &self.fight.rolls,
            (
                &mut self.fight.accuracy_excess,
                terrain,
                self.chapter,
                overwhelming,
            ),
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
            Task::Rows(rows) => self.step_rows(rows),
            // 0x0802FD4C: the effects that ran out; without any it reports
            // on its second frame.
            Task::Expiry(0) => {
                self.fight.expired = self.expire_effects();
                Task::Expiry(1)
            }
            Task::Expiry(_) => {
                if self.fight.expired.is_empty() {
                    Task::Reported
                } else {
                    self.expiry_message(0)
                }
            }
            Task::Expired(index, after) => self.step_expired(index, after),
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
            Task::Item(step) => self.step_item(step),
            Task::Command(step) => self.step_command(step),
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
                let menu = if self.lineup.story().is_some_and(StoryBattle::without_items) {
                    MENU_ACTION_NO_ITEMS
                } else {
                    MENU_ACTION
                };
                self.acts.push_back(Act::Call(Call::Menu(menu)));
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
        if frame <= faded + 2 {
            return Task::Return(frame + 1);
        }
        match self.fight.report {
            Some(Report::Done) => self.fight.report = Some(Report::Awaiting),
            Some(Report::Awaiting) if self.fight.display.is_none() && self.fight.hits.is_none() => {
                self.fight.report = Some(Report::Closing(0));
                return Task::Return(frame + 1);
            }
            Some(Report::Closing(tail)) => {
                if tail >= RETURN_TAIL {
                    self.fight.report = None;
                    return Task::Reported;
                }
                self.fight.report = Some(Report::Closing(tail + 1));
                return Task::Return(frame + 1);
            }
            _ => {}
        }
        self.step_report();
        Task::Return(frame + 1)
    }

    /// The display and the first message the first blow's kind picks
    /// (`0x0802BCA4`, the table at `0x0802BEB0`): after damage the hit
    /// display (`0x0802D36C`, `0x0802C084`); after a raised or lowered
    /// statistic the glows and their messages (`0x0802CA54` or
    /// `0x0802C8EC`, `0x0802CE98`); after a repair its glows and messages
    /// (`0x0802C420`, `0x0802C708`).
    fn start_report(&mut self) {
        match self.fight.blows.first().map(|blow| blow.kind) {
            Some(attack::RAISED) => {
                self.start_glows(Glow::Raise);
                self.report_support(0);
            }
            Some(attack::LOWERED) => {
                self.start_glows(Glow::Lower);
                self.report_support(0);
            }
            Some(attack::REPAIRED) => {
                self.start_glows(Glow::Mend);
                self.report_repair(0);
            }
            Some(attack::AFFLICTED | attack::RESTORED) => {
                self.start_glows(Glow::Raise);
                self.report_state(0);
            }
            Some(attack::DAMAGE) | None => self.start_hit_report(),
            Some(_) => self.fight.report = Some(Report::Tail(0)),
        }
    }

    /// The sprite a spark or a glow shows.
    pub(super) fn spark_sprite(&self, spark: &Spark) -> Option<&EffectSprite> {
        match spark.glow {
            Some((glow, _)) => self.glow_sprite(glow),
            None if spark.blast => self.effect_sprites.blast.as_ref(),
            None => self.effect_sprites.spark.as_ref(),
        }
    }

    /// Each sacrificed unit's explosion (`0x0802DCB4`): the unit's hit
    /// points go to 0 and it is destroyed at once, its panel shown again.
    pub(super) fn start_blasts(&mut self) {
        let ticks = self
            .effect_sprites
            .blast
            .as_ref()
            .and_then(|sprite| sprite.animation.first())
            .map_or(1, |step| step.duration.max(1));
        let mut sparks = Vec::new();
        for blow in self.fight.blows.clone().into_iter().filter(Blow::landed) {
            if let Some(unit) = self.sides[blow.side][blow.slot].as_mut() {
                unit.hp = 0;
                unit.traits |= DESTROYED;
            }
            if blow.side == PARTY {
                self.refresh_panel(blow.slot);
                self.light_panel(blow.slot);
            }
            sparks.push(Spark {
                side: blow.side,
                slot: blow.slot,
                glow: None,
                hidden: None,
                delay: SPARK_DELAY,
                frame: 0,
                step: 0,
                ticks,
                ended: false,
                shown: false,
                blast: true,
            });
        }
        if !sparks.is_empty() {
            self.fight.display = Some(Display {
                sparks,
                frames: 0,
                tail: None,
            });
        }
    }

    fn glow_sprite(&self, glow: Glow) -> Option<&extraction::saga_battle::EffectSprite> {
        let sprites = &self.effect_sprites;
        match glow {
            Glow::Raise => sprites.glow.as_ref(),
            Glow::Mend => sprites.mend.as_ref(),
            Glow::Lower => sprites.lower.as_ref(),
            Glow::Stop => sprites.stop.as_ref(),
            Glow::Stun => sprites.stun.as_ref(),
            Glow::Revive => sprites.revive.as_ref(),
        }
    }

    /// Each unit the blows landed on glows, with the display's sound
    /// (`0x0802CA54`, `0x0802C420`).
    pub(super) fn start_glows(&mut self, glow: Glow) {
        let sprite = self.glow_sprite(glow);
        let ticks = sprite
            .and_then(|sprite| sprite.animation.first())
            .map_or(1, |step| step.duration.max(1));
        let sparks: Vec<Spark> = self
            .fight
            .blows
            .iter()
            .filter(|blow| blow.landed())
            .map(|blow| Spark {
                side: blow.side,
                slot: blow.slot,
                glow: Some((glow, 0)),
                hidden: None,
                delay: SPARK_DELAY,
                frame: 0,
                step: 0,
                ticks,
                ended: false,
                shown: false,
                blast: false,
            })
            .collect();
        if !sparks.is_empty() {
            self.sounds.push(match glow {
                Glow::Raise => GLOW_SOUND,
                Glow::Mend => MEND_SOUND,
                Glow::Lower => LOWER_SOUND,
                Glow::Stop => STOP_SOUND,
                Glow::Revive => REVIVE_SOUND,
                Glow::Stun => return,
            });
            self.fight.display = Some(Display {
                sparks,
                frames: 0,
                tail: None,
            });
        }
    }

    /// The message of blow `index` of a repair, or of the next that landed
    /// (`0x0802C708`): the unit, ＨＰが, the points given back, 回復した; a
    /// party unit's panel follows.
    pub(super) fn report_repair(&mut self, index: usize) {
        let Some(offset) = self.fight.blows[index.min(self.fight.blows.len())..]
            .iter()
            .position(Blow::landed)
        else {
            self.fight.report = Some(Report::Tail(0));
            return;
        };
        let index = index + offset;
        let blow = self.fight.blows[index];
        let mut calls = self.unit_name(Some((blow.side, blow.slot)));
        calls.extend([Call::Text(TEXT_OF), Call::Text(TEXT_HIT_POINTS)]);
        calls.extend(number(blow.damage));
        calls.push(Call::Text(TEXT_REPAIRED));
        self.message_then(
            &calls,
            (blow.side == PARTY).then_some(Act::Panel(blow.slot)),
        );
        self.fight.report = Some(Report::Repair(index));
    }

    /// The message of blow `index` of a support part, or of the next that
    /// landed (`0x0802CE98`): the unit, what changed, by how much, and
    /// whether it rose or fell.
    fn report_support(&mut self, index: usize) {
        let Some(offset) = self.fight.blows[index.min(self.fight.blows.len())..]
            .iter()
            .position(Blow::landed)
        else {
            self.fight.report = Some(Report::Tail(0));
            return;
        };
        let index = index + offset;
        let blow = self.fight.blows[index];
        let mut calls = self.unit_name(Some((blow.side, blow.slot)));
        calls.push(Call::Text(TEXT_OF));
        if let Some(&(_, text)) = CHANGE_TEXTS.iter().find(|(bit, _)| blow.code & bit != 0) {
            calls.push(Call::Text(text));
        }
        calls.extend(number(blow.damage));
        if blow.code & 1 != 0 {
            calls.push(Call::Text(TEXT_ROSE));
        } else if blow.code & 2 != 0 {
            calls.push(Call::Text(TEXT_FELL));
        }
        self.message(&calls);
        self.fight.report = Some(Report::Support(index));
    }

    /// The message of blow `index` of a part that sets a state
    /// (`0x0802D224`: the unit's name and `battle-text` 81,
    /// はダメージを一切受けない！！) or repairs in full (`0x0802D0C0`: 82,
    /// の運動性能が上がった！！, and a party unit's panel), for a blow that
    /// landed; each blow has its wait.
    fn report_state(&mut self, index: usize) {
        let Some(blow) = self.fight.blows.get(index).copied() else {
            self.fight.report = Some(Report::Tail(0));
            return;
        };
        let restored = blow.kind == attack::RESTORED;
        if blow.landed() {
            let mut calls = self.unit_name(Some((blow.side, blow.slot)));
            calls.push(Call::Text(if restored {
                TEXT_RESTORED
            } else {
                TEXT_UNTOUCHABLE
            }));
            self.message_then(
                &calls,
                (restored && blow.side == PARTY).then_some(Act::Panel(blow.slot)),
            );
        } else {
            self.acts.push_back(Act::Wait);
        }
        self.fight.report = Some(Report::State(index));
    }

    /// The hit display and the first message (`0x0802D36C`, `0x0802C084`):
    /// the display loads the sparks' graphics (`0x08032134`), which holds
    /// up the main loop for the next frame, and sounds once a blow landed.
    fn start_hit_report(&mut self) {
        let landed: Vec<Blow> = self
            .fight
            .blows
            .iter()
            .copied()
            .filter(Blow::landed)
            .collect();
        self.fight.hits = Some(if landed.is_empty() {
            HitStep::Start(HitStage::Dazed)
        } else {
            self.sounds.push(HIT_SOUND);
            self.lag = 1;
            self.fight.display = Some(self.hit_sparks(&landed, None, false));
            HitStep::Running(HitStage::Sparks)
        });
        self.fight.report = Some(Report::Target(0));
        self.report_target(0);
    }

    /// A display of the units `blows` name: sparks, glows of `glow`, or
    /// explosions.
    fn hit_sparks(&self, blows: &[Blow], glow: Option<Glow>, blast: bool) -> Display {
        let sprite = match glow {
            Some(glow) => self.glow_sprite(glow),
            None if blast => self.effect_sprites.blast.as_ref(),
            None => self.effect_sprites.spark.as_ref(),
        };
        let ticks = sprite
            .and_then(|sprite| sprite.animation.first())
            .map_or(1, |step| step.duration.max(1));
        Display {
            sparks: blows
                .iter()
                .map(|blow| Spark {
                    side: blow.side,
                    slot: blow.slot,
                    glow: glow.map(|glow| (glow, 0)),
                    hidden: None,
                    delay: SPARK_DELAY,
                    frame: 0,
                    step: 0,
                    ticks,
                    ended: false,
                    shown: false,
                    blast,
                })
                .collect(),
            frames: 0,
            tail: None,
        }
    }

    /// A frame of the hit display's task.
    fn step_hits(&mut self) {
        let Some(step) = self.fight.hits else {
            return;
        };
        self.fight.hits = match step {
            HitStep::Running(stage) => {
                self.advance_sparks();
                let reported = self
                    .fight
                    .display
                    .as_ref()
                    .and_then(|display| display.sparks.first())
                    .is_none_or(Spark::done);
                if !reported {
                    Some(step)
                } else if stage == HitStage::Beaten {
                    Some(HitStep::Reporting)
                } else {
                    Some(HitStep::Start(stage.next()))
                }
            }
            HitStep::Start(stage) => Some(self.start_hit_stage(stage)),
            HitStep::Reporting => {
                self.fight.display = None;
                None
            }
        };
    }

    /// The start of a stage of the hit display: its units, by their blows'
    /// flags; the explosions' graphics hold up the main loop for the next
    /// frame.
    fn start_hit_stage(&mut self, stage: HitStage) -> HitStep {
        self.fight.display = None;
        let units: Vec<Blow> = self
            .fight
            .blows
            .iter()
            .copied()
            .filter(|blow| match stage {
                HitStage::Sparks => blow.landed(),
                HitStage::Dazed => blow.flags & attack::PILOT_HURT != 0,
                HitStage::Stunned => blow.flags & attack::STUNNED != 0,
                HitStage::Beaten => blow.destroyed(),
            })
            .collect();
        if stage == HitStage::Beaten {
            self.lag = 1;
        }
        if units.is_empty() {
            return if stage == HitStage::Beaten {
                HitStep::Reporting
            } else {
                HitStep::Start(stage.next())
            };
        }
        let display = match stage {
            HitStage::Sparks => self.hit_sparks(&units, None, false),
            HitStage::Dazed => self.hit_sparks(&units, Some(Glow::Lower), false),
            HitStage::Stunned => self.hit_sparks(&units, Some(Glow::Stun), false),
            HitStage::Beaten => {
                let largest = units
                    .iter()
                    .filter_map(|blow| self.sides[blow.side][blow.slot].as_ref())
                    .map(|unit| usize::from(unit.size))
                    .max()
                    .unwrap_or(0);
                if let Some(&sound) = BEATEN_SOUNDS.get(largest) {
                    self.sounds.push(sound);
                }
                self.hit_sparks(&units, None, true)
            }
        };
        self.fight.display = Some(display);
        HitStep::Running(stage)
    }

    /// Advances the display's sparks a frame: a spark hides once its
    /// animation and its own frames are over; a paralysed unit stays dark
    /// and paralysed (`0x0802DAF4` sets its trait), an exploded one gone.
    fn advance_sparks(&mut self) {
        let Some(mut display) = self.fight.display.take() else {
            return;
        };
        display.frames += 1;
        for spark in &mut display.sparks {
            if let Some(sprite) = self.spark_sprite(spark) {
                spark.advance(sprite);
            }
            if spark.vanished() && spark.blast {
                self.fight.gone[spark.side][spark.slot] = true;
            }
            let over = match spark.glow {
                Some((glow, frame)) => frame > glow.levels().2 + glow.lead(),
                None if spark.blast => true,
                None => spark.frame >= SHAKES * 4,
            };
            if !(spark.ended && over && spark.shown) {
                continue;
            }
            spark.shown = false;
            spark.hidden = match spark.glow {
                Some((_, frame)) => Some(frame),
                None if spark.blast => None,
                None => Some(spark.frame),
            };
            match spark.glow {
                Some((Glow::Stun, _)) => {
                    self.dims[spark.side][spark.slot] = u8::try_from(STUN_LAST).unwrap_or(u8::MAX);
                    if let Some(unit) = self.sides[spark.side][spark.slot].as_mut() {
                        unit.traits |= PARALYSED;
                    }
                }
                // A glow writes the Zoid's own colors, so the unit stays
                // undarkened once it is over (`0x0802CBBC`, `0x0802C588`).
                Some(_) => self.dims[spark.side][spark.slot] = 0,
                None => {}
            }
        }
        self.fight.display = Some(display);
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

    /// One of the messages task's steps after target `index`'s message
    /// (`0x0802C084`): a dazed pilot's line (the pilot's name and
    /// `battle-text` 79), a paralysed unit's (80), a destroyed one's (6),
    /// then the next target's message a frame later, or the report.
    fn check_target(&mut self, index: usize, check: Check) {
        let Some(blow) = self.fight.blows.get(index).copied() else {
            self.fight.report = Some(Report::Done);
            return;
        };
        let target = Some((blow.side, blow.slot));
        let line = |calls: Vec<Call>, text: u16| {
            let mut calls = calls;
            calls.push(Call::Text(text));
            calls
        };
        self.fight.report = Some(match check {
            Check::Pilot if blow.landed() && blow.flags & attack::PILOT_HURT != 0 => {
                let character = self.sides[blow.side][blow.slot]
                    .as_ref()
                    .map_or(0, |unit| unit.character);
                self.message(&line(vec![Call::Character(character)], TEXT_DAZED));
                Report::Line(index, Check::Paralysis)
            }
            Check::Pilot => Report::Check(index, Check::Paralysis),
            Check::Paralysis if blow.flags & attack::STUNNED != 0 => {
                self.message(&line(self.unit_name(target), TEXT_PARALYSED));
                Report::Line(index, Check::Beaten)
            }
            Check::Paralysis => Report::Check(index, Check::Beaten),
            Check::Beaten if blow.destroyed() => {
                self.message(&line(self.unit_name(target), TEXT_DESTROYED));
                Report::Line(index, Check::Count)
            }
            Check::Beaten => Report::Check(index, Check::Count),
            Check::Count if index + 1 < self.fight.blows.len() => {
                Report::Check(index + 1, Check::Print)
            }
            Check::Count => Report::Check(index, Check::Last),
            Check::Print => {
                self.report_target(index);
                return;
            }
            Check::Last => Report::Done,
        });
    }

    pub(super) fn step_report(&mut self) {
        self.step_display();
        let busy = self.running.is_some() || !self.acts.is_empty();
        let report = self.fight.report;
        let waited = !busy
            && matches!(
                report,
                Some(
                    Report::Target(_)
                        | Report::Line(..)
                        | Report::Support(_)
                        | Report::State(_)
                        | Report::Repair(_)
                        | Report::Item
                )
            )
            && self.wait_done();
        match report {
            Some(Report::Target(index)) if waited => {
                self.fight.report = Some(Report::Check(index, Check::Pilot));
            }
            Some(Report::Line(index, next)) if waited => {
                self.fight.report = Some(Report::Check(index, next));
            }
            Some(Report::Check(index, check)) => self.check_target(index, check),
            Some(Report::Support(index)) if waited => self.report_support(index + 1),
            Some(Report::State(index)) if waited => self.report_state(index + 1),
            Some(Report::Repair(index)) if waited => self.report_repair(index + 1),
            Some(Report::Item) if waited => self.fight.report = Some(Report::Tail(0)),
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

    /// The round's コマンド (states 8000, `0x1FA4`): the command task; once
    /// issued the round starts, given up its menu comes back.
    fn step_command_stage(&mut self, stage: Stage) -> Option<Stage> {
        if stage == Stage::Command {
            self.task = Task::Command(super::commands::CommandStep::Start);
            return Some(Stage::AwaitCommand);
        }
        self.take_task().then(|| {
            if self.command_state.issued.take() == Some(true) {
                Stage::RoundStart
            } else {
                Stage::RoundMenu
            }
        })
    }

    /// The item's stages (`0x157C`, `0x15E0`): its task, then the next
    /// actor once an item was used, or the actions again.
    fn step_item_stage(&mut self, stage: Stage) -> Option<Stage> {
        if stage == Stage::Item {
            self.task = Task::Item(super::items::ItemStep::Open);
            return Some(Stage::AwaitItem);
        }
        self.take_task().then(|| {
            if self.item_used.take() == Some(true) {
                self.restore_units();
                Stage::Next
            } else {
                Stage::Act
            }
        })
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

    /// Dims every panel but party slot `slot`'s, found by its pilot
    /// (`0x08031598`).
    pub(super) fn light_panel(&mut self, slot: usize) {
        let lit = match self.sides[PARTY][slot].as_ref() {
            Some(unit) => self
                .panels
                .iter()
                .position(|panel| panel.character == unit.character),
            None => self.panels.iter().position(|panel| panel.slot == slot),
        };
        self.dim_panels = super::PanelLight::Dimmed { lit };
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
        let character = unit.character;
        for panel in self
            .panels
            .iter_mut()
            .filter(|panel| panel.character == character)
        {
            if (panel.hp, panel.ep) != (hp, ep) {
                self.palette_writes += 1;
            }
            panel.hp = hp;
            panel.ep = ep;
        }
    }

    /// A frame of the display: the hit display's task, or the glows or
    /// explosions, which end a few frames after the last has gone.
    fn step_display(&mut self) {
        if self.fight.hits.is_some() {
            self.step_hits();
            return;
        }
        self.advance_sparks();
        let Some(mut display) = self.fight.display.take() else {
            return;
        };
        if display.tail.is_none() && display.sparks.iter().all(Spark::done) {
            display.tail = Some(0);
        }
        let glows = || display.sparks.iter().filter_map(|spark| spark.glow);
        let last = if glows().any(|(glow, _)| glow == Glow::Revive) {
            // The revival's task reports the frame after its units'
            // (`0x0802E40C`).
            0
        } else if glows().next().is_some() {
            GLOW_TAIL
        } else {
            DISPLAY_TAIL
        };
        if let Some(tail) = display.tail.as_mut() {
            *tail += 1;
            if *tail > last {
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
                ROUND_COMMAND => Stage::Command,
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
