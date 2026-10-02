//! Zoids Saga's own events, transcribed from its code into [`Op`]
//! programs: what each map runs when it loads and what objects whose
//! script is code run when spoken to.
//!
//! Source of knowledge: own reading of the Thumb routines named on each
//! program in Zoids Saga (Japan, Rev 1), checked against per-frame traces
//! of the entity table and the brightness register in a reference emulator
//! (see `docs/events.md`). Dialogue indices are strings of the `dialogue`
//! table; actors are object indices (the game's entity index minus one).

/// A companion's caller (chapters 9 and 10): with slot `$slot` empty,
/// the pick of the other slot is taken off the list, the list is offered
/// (`$list`) and a pick joins the party (`$taken`), with a unit of its
/// Zoid when `$zoid`; with the slot full, he offers to send the pick away
/// (`$change`).
macro_rules! caller {
    ($slot:expr, $other:expr, $list:expr, $taken:expr, $change:expr, $zoid:expr) => {
        &[
            Op::OfferCompanions,
            Op::Wait(1),
            Op::IfCompanion {
                slot: $slot,
                then: &[
                    Op::Dialogue($change),
                    Op::IfChoice {
                        then: &[Op::DropCompanion($slot)],
                        otherwise: &[],
                    },
                ],
                otherwise: &[
                    Op::HideCompanion($other),
                    Op::Dialogue($list),
                    Op::TakeCompanion {
                        slot: $slot,
                        zoid: $zoid,
                        then: &[Op::Dialogue($taken)],
                    },
                ],
            },
        ]
    };
}
use caller;

mod chapter10;
mod chapter2;
mod chapter3;
mod chapter4;
mod chapter5;
mod chapter6;
mod chapter7;
mod chapter8;
mod chapter9;

use crate::event::{
    AT_THE_PORTAL, BELOW_THE_PORTAL, BLACK, ChestKind, EXIT_ARRIVAL, FIELD_HOOK, HERE, MAP_TASK,
    Op, THE_PORTAL,
};
use crate::field::{Direction, PIXEL};
use crate::menu::Shop;

/// The flags the openings of chapters 2 to 10 set as the party arrives in
/// each, so the chapter before is over: Sand Colony, the throne room of
/// chapter 3 and so on to the castle's room after the staff roll.
pub(crate) const CHAPTER_OPENINGS: [u16; 9] = [
    chapter2::SAND_COLONY_ARRIVED,
    chapter3::CHAPTER_OPENED,
    chapter4::CHAPTER_OPENED,
    chapter5::CHAPTER_OPENED,
    chapter6::CHAPTER_OPENED,
    chapter7::CHAPTER_OPENED,
    chapter8::CHAPTER_OPENED,
    chapter9::CHAPTER_OPENED,
    chapter10::CHAPTER_OPENED,
];
/// Set once Vega's Führer is beaten (chapter 10).
pub(crate) const VEGA_BEATEN: u16 = chapter10::VEGA_BEATEN;
/// Set once Rosso and Viola join in the ruins (chapter 6).
pub(crate) const ROSSO_JOINED: u16 = chapter6::ROSSO_JOINED;
/// Set once each of chapter 10's researchers has given his Zi data.
pub(crate) const RESEARCHERS_GAVE: [u16; 2] =
    [chapter10::FIRST_DATA_GIVEN, chapter10::SECOND_DATA_GIVEN];
/// The Zoid Federation's Ultrasaurus fight, which only a defiant answer
/// starts (chapter 5).
pub(crate) const ULTRASAURUS_BATTLE: u8 = chapter5::ULTRASAURUS_BATTLE;

/// The Zi data the story gives outside chests: Irvine's (chapter 4),
/// Rosso's (chapter 6), Blue Gem's (chapter 7), Vega's and the
/// researchers' (chapter 10).
pub(crate) fn zi_data_gifts() -> impl Iterator<Item = u8> {
    [
        chapter4::IRVINE_GIFT,
        chapter6::ROSSO_GIFT,
        chapter10::VEGA_ZI_DATA,
    ]
    .into_iter()
    .chain(chapter7::BLUE_GEM_GIFTS)
    .chain(chapter10::FIRST_DATA)
    .chain(chapter10::SECOND_DATA)
}

/// Set by the first room once the opening has played.
pub const OPENING_SEEN: u16 = 0x11F;
/// Set by speaking to the soldier by the stairs of the ground floor.
pub const MET_FIRST_WARRIOR: u16 = 0x120;
/// Set by speaking to the warrior on the second floor.
pub const MET_SECOND_WARRIOR: u16 = 0x121;
/// Set when the king starts the throne room's cutscene.
pub const KING_SPOKEN: u16 = 0x122;

/// Set by choosing the Shield Liger in the hangar (Regina's offer).
pub const CHOSE_SHIELD_LIGER: u16 = 0x123;
/// Set by choosing the Saber Tiger in the hangar (Ace's offer).
pub const CHOSE_SABER_TIGER: u16 = 0x124;
/// Set by choosing the Raynos in the hangar (Jack's offer).
pub const CHOSE_RAYNOS: u16 = 0x125;

const HALF: i32 = PIXEL / 2;
const DOUBLE: i32 = PIXEL * 2;
const QUARTER: i32 = PIXEL / 4;
const HELPER_TASK: usize = 4;
const REGINA: usize = 1;
const PLAYER: usize = 0;
const KING: usize = 2;
const KING_APPEARS_AT: (usize, usize) = (38, 1);
const FIRST_ROOM_MUSIC: u16 = 7;

const fn walk(actor: usize, to: (usize, usize), speed: i32, shift: i8) -> Op {
    Op::Walk {
        actor,
        to,
        speed,
        shift,
        through: false,
    }
}

/// A walk at a pixel a frame, the pace most cutscenes use.
const fn stride(actor: usize, to: (usize, usize)) -> Op {
    walk(actor, to, PIXEL, 1)
}

/// A walk at two pixels a frame.
const fn run(actor: usize, to: (usize, usize)) -> Op {
    walk(actor, to, DOUBLE, 2)
}

const fn through(actor: usize, to: (usize, usize), speed: i32, shift: i8) -> Op {
    Op::Walk {
        actor,
        to,
        speed,
        shift,
        through: true,
    }
}

/// Darkening a scene and waiting for black (`0x08011E08`): the level is
/// set, and from the next frame it steps a level every other frame.
const SCENE_DARKEN: &[Op] = &[Op::Brightness(0), Op::Wait(1), Op::FadeOut(1)];
/// Brightening a scene and waiting for full light (`0x08011E24`).
const SCENE_BRIGHTEN: &[Op] = &[Op::Brightness(BLACK), Op::Wait(1), Op::FadeIn(1)];

/// The fade-out task (`0x0800C6A4`): from normal to black, a level every
/// other frame.
pub const FADE_OUT: &[Op] = &[Op::Brightness(0), Op::FadeOut(1), Op::End];
/// The fade-in task (`0x0800C680`): from black to normal, a level every
/// other frame.
pub const FADE_IN: &[Op] = &[Op::Brightness(31), Op::FadeIn(1), Op::End];

/// The darkening half of a flicker (`0x0800C6E8`): toward black from the
/// current level, a level every other frame.
const DARKEN: &[Op] = &[Op::FadeOut(1), Op::End];
/// The brightening half (`0x0800C6C8`).
const BRIGHTEN: &[Op] = &[Op::FadeIn(1), Op::End];

/// The screen shaken by a blast (`0x08009938`): it darkens and brightens
/// for ten frames each, three times, with the blast's sound on the first
/// two, then a second passes.
const FLASH: &[Op] = &[
    Op::Spawn(HELPER_TASK, DARKEN),
    Op::Wait(10),
    Op::Sound(0x5B),
    Op::Spawn(HELPER_TASK, BRIGHTEN),
    Op::Wait(10),
    Op::Spawn(HELPER_TASK, DARKEN),
    Op::Wait(10),
    Op::Spawn(HELPER_TASK, BRIGHTEN),
    Op::Wait(10),
    Op::Sound(0x5B),
    Op::Spawn(HELPER_TASK, DARKEN),
    Op::Wait(10),
    Op::Spawn(HELPER_TASK, BRIGHTEN),
    Op::Wait(10),
    Op::Wait(60),
];

/// Where the item table's strings for the deck commands start.
const COMMAND_NAMES: u16 = 77;
/// 王子のはげまし, the prince's encouragement.
const PRINCES_ENCOURAGEMENT_COMMAND: u8 = 19;

/// Learning deck command `command` (`0x080378F0`): the message box, the
/// battle system's lines around the command's name, then the key.
const fn learn(command: u8) -> [Op; 9] {
    [
        Op::Dialogue(0x1F),
        Op::Script("battle-menu", 6),
        Op::Script("battle-menu", 7),
        Op::Script("battle-text", 0x39),
        Op::Script("item", COMMAND_NAMES + command as u16),
        Op::Script("battle-text", 0x3A),
        Op::Script("battle-menu", 5),
        Op::LearnCommand(command),
        Op::Dialogue(0x22),
    ]
}

/// The king teaches the prince's encouragement.
const PRINCES_ENCOURAGEMENT: &[Op] = &learn(PRINCES_ENCOURAGEMENT_COMMAND);

/// One blast at the gate (tasks at `0x0800DEA4`, `0x0800DF0C`, `0x0800DF74`,
/// `0x0800DFDC`): the explosion sprite plays once with its sound, then
/// leaves the screen.
const fn blast(actor: usize) -> [Op; 6] {
    [
        Op::PlayOnce(actor, 0),
        Op::Show(actor),
        Op::Sound(0x5A),
        Op::AwaitAnimation(actor),
        Op::Place(actor, (0xFF, 0xFF)),
        Op::End,
    ]
}

const BLAST_A: &[Op] = &blast(13);
const BLAST_B: &[Op] = &blast(14);
const BLAST_C: &[Op] = &blast(15);
const BLAST_D: &[Op] = &blast(16);

/// The first room's opening (task at `0x0800C854`): Regina paces, lectures,
/// paces in the dark, lectures again and leaves; the player stands up.
const OPENING: &[Op] = &[
    Op::Wait(120),
    Op::AwaitArrival(REGINA),
    Op::Face(REGINA, Direction::Right),
    Op::Wait(30),
    walk(REGINA, (8, 4), HALF, 0),
    Op::AwaitArrival(REGINA),
    Op::Face(REGINA, Direction::Left),
    Op::Wait(60),
    walk(REGINA, (6, 4), HALF, 0),
    Op::AwaitArrival(REGINA),
    Op::Wait(10),
    Op::Face(REGINA, Direction::Up),
    Op::Wait(10),
    Op::Dialogue(40),
    Op::Wait(10),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(4),
    walk(REGINA, (3, 4), HALF, 0),
    Op::AwaitArrival(REGINA),
    Op::Face(REGINA, Direction::Right),
    Op::Wait(60),
    walk(REGINA, (8, 4), HALF, 0),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::AwaitArrival(REGINA),
    Op::Face(REGINA, Direction::Left),
    Op::Wait(60),
    Op::Dialogue(41),
    Op::Wait(20),
    walk(REGINA, (2, 8), HALF, 0),
    Op::Wait(120),
    Op::Face(PLAYER, Direction::Down),
    Op::Nudge(PLAYER, (0, -2)),
    Op::AwaitArrival(REGINA),
    walk(REGINA, (2, 12), HALF, 0),
    Op::AwaitArrival(REGINA),
    Op::Hide(REGINA),
    Op::Nudge(PLAYER, (0, 2)),
    walk(PLAYER, (5, 2), PIXEL, 1),
    Op::AwaitArrival(PLAYER),
    Op::Control(true),
    Op::Music(FIRST_ROOM_MUSIC),
    Op::End,
];

/// The king's summons (task at `0x0800CC94`), on the ground floor: Regina
/// runs up to the king, he calls everyone to the throne room.
const THRONE_SUMMONS: &[Op] = &[
    Op::Wait(30),
    Op::Dialogue(0x2F),
    Op::Place(3, (34, 16)),
    run(3, (34, 1)),
    Op::AwaitArrival(3),
    Op::Face(3, Direction::Right),
    Op::Wait(30),
    Op::Face(PLAYER, Direction::Left),
    Op::Wait(30),
    Op::Face(KING, Direction::Left),
    Op::Wait(30),
    Op::Dialogue(0x30),
    Op::Wait(120),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Wait(1),
    Op::LoadMap {
        map: 5,
        player: (8, 3),
        objects: 0x0832_A894,
        count: 7,
    },
    Op::Music(3),
    Op::Wait(1),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Wait(60),
    Op::Call(THRONE_ROOM),
];

/// In the throne room (objects: 1 the king, 2 Regina, 3 Jack, 4 Ace, 5 and 6
/// soldiers): the warriors step up, the castle comes under attack, a
/// soldier reports the Empire's Zoids, the king teaches the prince's
/// encouragement and sends everyone away.
const THRONE_ROOM: &[Op] = &[
    walk(3, (6, 4), HALF, 0),
    Op::Wait(7),
    walk(4, (10, 4), HALF, 0),
    Op::AwaitArrival(3),
    Op::Face(3, Direction::Up),
    Op::AwaitArrival(4),
    Op::Face(4, Direction::Up),
    Op::Wait(30),
    Op::Dialogue(0x31),
    Op::Wait(60),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Music(4),
    Op::Wait(30),
    Op::Battle(0),
    Op::Wait(10),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Call(FLASH),
    Op::Face(2, Direction::Left),
    Op::Wait(16),
    Op::Face(2, Direction::Right),
    Op::Wait(16),
    Op::Face(2, Direction::Left),
    Op::Wait(16),
    Op::Face(2, Direction::Right),
    Op::Wait(16),
    Op::Face(PLAYER, Direction::Left),
    Op::Wait(16),
    Op::Face(PLAYER, Direction::Right),
    Op::Wait(16),
    Op::Face(PLAYER, Direction::Left),
    Op::Wait(16),
    Op::Face(PLAYER, Direction::Right),
    Op::Wait(16),
    Op::Face(PLAYER, Direction::Left),
    Op::Wait(16),
    Op::Face(2, Direction::Up),
    Op::Wait(4),
    Op::Face(PLAYER, Direction::Up),
    Op::Wait(4),
    Op::Dialogue(0x32),
    Op::Wait(1),
    run(5, (8, 4)),
    Op::Wait(10),
    Op::Face(3, Direction::Right),
    Op::Wait(4),
    Op::Face(4, Direction::Left),
    Op::Wait(4),
    stride(2, (6, 3)),
    Op::Wait(4),
    stride(PLAYER, (9, 3)),
    Op::AwaitArrival(2),
    Op::Face(2, Direction::Right),
    Op::AwaitArrival(PLAYER),
    Op::Face(PLAYER, Direction::Left),
    Op::AwaitArrival(5),
    Op::Dialogue(0x33),
    Op::Wait(10),
    Op::FadeOutHolding,
    Op::Battle(1),
    Op::FadeInHolding,
    Op::Wait(10),
    Op::Call(FLASH),
    run(6, (8, 4)),
    Op::Wait(10),
    stride(5, (7, 5)),
    Op::AwaitArrival(5),
    Op::Face(5, Direction::Up),
    Op::AwaitArrival(6),
    Op::Dialogue(0x34),
    stride(6, (9, 5)),
    Op::AwaitArrival(6),
    Op::Face(6, Direction::Up),
    Op::Wait(6),
    Op::Face(3, Direction::Up),
    Op::Wait(3),
    Op::Face(4, Direction::Up),
    Op::Wait(3),
    Op::Face(2, Direction::Up),
    Op::Wait(6),
    Op::Face(PLAYER, Direction::Up),
    Op::Wait(6),
    Op::Dialogue(0x35),
    Op::Call(PRINCES_ENCOURAGEMENT),
    stride(5, (7, 10)),
    stride(6, (9, 10)),
    Op::Wait(120),
    stride(3, (6, 10)),
    stride(4, (10, 10)),
    Op::Wait(60),
    stride(2, (6, 10)),
    stride(PLAYER, (9, 10)),
    Op::Wait(10),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(120),
    Op::Wait(1),
    Op::LoadMap {
        map: 2,
        player: (24, 16),
        objects: 0x0832_A920,
        count: 4,
    },
    Op::Music(4),
    Op::Wait(1),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(30),
    Op::Call(TO_THE_GATE),
];

/// Back on the ground floor (1 Regina, 2 Ace, 3 Jack): the castle shakes,
/// and they head for the main gate.
const TO_THE_GATE: &[Op] = &[
    stride(PLAYER, (23, 16)),
    Op::Wait(8),
    stride(1, (20, 14)),
    stride(2, (22, 15)),
    stride(3, (23, 14)),
    Op::AwaitArrival(PLAYER),
    Op::Face(PLAYER, Direction::Down),
    Op::AwaitArrival(2),
    Op::Face(2, Direction::Down),
    Op::AwaitArrival(3),
    Op::Face(3, Direction::Down),
    Op::AwaitArrival(1),
    Op::Wait(30),
    Op::Face(1, Direction::Down),
    Op::Wait(30),
    Op::Dialogue(0x36),
    Op::Wait(60),
    Op::Call(FLASH),
    Op::Dialogue(0x37),
    Op::Wait(60),
    through(PLAYER, (23, 18), PIXEL, 1),
    Op::AwaitArrival(PLAYER),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(120),
    Op::Wait(1),
    Op::LoadMap {
        map: 8,
        player: (24, 0),
        objects: 0x0832_A970,
        count: 17,
    },
    Op::Music(5),
    Op::Wait(1),
    Op::Place(PLAYER, (24, 0xFFFE)),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(120),
    Op::Call(AT_THE_GATE),
];

const ALARM: &[Op] = &[Op::Sound(0x72), Op::Wait(10)];
const PAN_DOWN: &[Op] = &[Op::Pan(0, DOUBLE), Op::Wait(1)];
const PAN_UP: &[Op] = &[Op::Pan(0, -DOUBLE), Op::Wait(1)];
const PAN_RIGHT: &[Op] = &[Op::Pan(DOUBLE, 0), Op::Wait(1)];
const PAN_LEFT: &[Op] = &[Op::Pan(-DOUBLE, 0), Op::Wait(1)];

/// Outside the main gate (1 the emperor, 2–5 his four kings, 6–12 soldiers,
/// 13–16 blasts): the army shells the gate, the camera goes down to the
/// emperor and back.
const AT_THE_GATE: &[Op] = &[
    Op::Sound(0x7D),
    Op::Wait(40),
    Op::Sound(0x7D),
    Op::Wait(60),
    Op::Wait(60),
    Op::Repeat(4, ALARM),
    Op::Wait(10),
    Op::Repeat(4, ALARM),
    Op::Wait(10),
    Op::Repeat(4, ALARM),
    Op::Wait(30),
    Op::Shift(13, 1),
    Op::Shift(14, 1),
    Op::Shift(15, 1),
    Op::Shift(16, 1),
    Op::Position(13, (320, 80)),
    Op::Spawn(4, BLAST_A),
    Op::Wait(10),
    Op::Position(14, (336, 72)),
    Op::Spawn(5, BLAST_B),
    Op::Wait(10),
    Op::Position(15, (352, 64)),
    Op::Spawn(6, BLAST_C),
    Op::Wait(10),
    Op::Position(16, (368, 56)),
    Op::Spawn(7, BLAST_D),
    Op::Wait(10),
    Op::Wait(10),
    Op::Position(13, (352, 72)),
    Op::Spawn(4, BLAST_A),
    Op::Wait(10),
    Op::Position(14, (368, 64)),
    Op::Spawn(5, BLAST_B),
    Op::Wait(10),
    Op::Position(15, (384, 56)),
    Op::Spawn(6, BLAST_C),
    Op::Wait(10),
    Op::Position(16, (400, 48)),
    Op::Spawn(7, BLAST_D),
    Op::Wait(10),
    Op::Wait(10),
    Op::Position(13, (384, 80)),
    Op::Spawn(4, BLAST_A),
    Op::Wait(10),
    Op::Position(14, (404, 72)),
    Op::Spawn(5, BLAST_B),
    Op::Wait(10),
    Op::Position(15, (424, 64)),
    Op::Spawn(6, BLAST_C),
    Op::Wait(10),
    Op::Position(16, (444, 56)),
    Op::Spawn(7, BLAST_D),
    Op::Wait(10),
    Op::Wait(180),
    Op::Repeat(100, PAN_DOWN),
    Op::Wait(120),
    Op::Dialogue(0x38),
    Op::Wait(60),
    Op::Repeat(100, PAN_UP),
    Op::Wait(30),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Wait(1),
    Op::LoadMap {
        map: 2,
        player: (23, 18),
        objects: 0x0832_AAC4,
        count: 4,
    },
    Op::Music(5),
    Op::Wait(1),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Call(TO_THE_HANGAR),
];

/// Back inside (1 Regina, 2 Ace, 3 Jack): the way out is blocked, so down to
/// the hangar.
const TO_THE_HANGAR: &[Op] = &[
    stride(1, (22, 15)),
    stride(2, (23, 14)),
    stride(3, (24, 15)),
    stride(PLAYER, (23, 16)),
    Op::AwaitArrival(PLAYER),
    Op::AwaitArrival(1),
    Op::Face(1, Direction::Down),
    Op::AwaitArrival(2),
    Op::Face(2, Direction::Down),
    Op::AwaitArrival(3),
    Op::Face(3, Direction::Down),
    Op::Wait(60),
    Op::Dialogue(0x39),
    Op::Wait(60),
    stride(PLAYER, (10, 16)),
    stride(1, (10, 15)),
    stride(2, (10, 14)),
    stride(3, (10, 15)),
    Op::Wait(60),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Wait(1),
    Op::LoadMap {
        map: 6,
        player: (7, 10),
        objects: 0x0832_AB14,
        count: 10,
    },
    Op::Wait(1),
    Op::PlayOnce(4, 1),
    Op::Pose(5, 4),
    Op::Pose(6, 4),
    Op::Pose(7, 4),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Call(IN_THE_HANGAR),
];

/// Regina walks to her Zoid and waits by it (`0x0800E550`).
const REGINA_TO_HER_ZOID: &[Op] = &[
    stride(1, (17, 9)),
    Op::AwaitArrival(1),
    Op::Place(1, (24, 9)),
    Op::Face(1, Direction::Left),
    Op::End,
];
/// Ace walks to his (`0x0800E5C8`).
const ACE_TO_HIS_ZOID: &[Op] = &[
    stride(2, (18, 9)),
    Op::AwaitArrival(2),
    Op::Place(2, (32, 9)),
    Op::Face(2, Direction::Left),
    Op::End,
];
/// Jack walks to his (`0x0800E640`).
const JACK_TO_HIS_ZOID: &[Op] = &[
    stride(3, (19, 9)),
    Op::AwaitArrival(3),
    Op::Place(3, (40, 9)),
    Op::Face(3, Direction::Left),
    Op::End,
];

/// In the hangar (1 Regina, 2 Ace, 3 Jack, 4 the carrier, 5–7 the three
/// Zoids): the camera looks along the Zoids, the warriors take their
/// places, and the player picks one by speaking to its warrior.
const IN_THE_HANGAR: &[Op] = &[
    Op::Dialogue(0x3A),
    Op::Wait(60),
    Op::Face(2, Direction::Right),
    Op::Face(3, Direction::Right),
    Op::Repeat(320, PAN_RIGHT),
    Op::Wait(60),
    Op::Repeat(320, PAN_LEFT),
    Op::Wait(30),
    Op::Dialogue(0x3B),
    Op::Wait(60),
    stride(2, (19, 9)),
    stride(3, (23, 9)),
    Op::Wait(60),
    Op::Dialogue(0x3C),
    Op::Wait(60),
    Op::AwaitArrival(3),
    stride(2, (8, 9)),
    stride(3, (10, 9)),
    Op::AwaitArrival(3),
    Op::Wait(30),
    Op::Animate(5, 1),
    Op::Animate(6, 1),
    Op::Animate(7, 1),
    Op::Dialogue(0x3D),
    Op::Wait(60),
    Op::Spawn(HELPER_TASK, REGINA_TO_HER_ZOID),
    Op::Spawn(5, ACE_TO_HIS_ZOID),
    Op::Spawn(6, JACK_TO_HIS_ZOID),
    Op::Wait(1),
    Op::AwaitArrival(1),
    Op::AwaitArrival(2),
    Op::AwaitArrival(3),
    stride(PLAYER, (8, 10)),
    Op::AwaitArrival(PLAYER),
    Op::Control(true),
    Op::AwaitAnyFlag(&[CHOSE_SHIELD_LIGER, CHOSE_SABER_TIGER, CHOSE_RAYNOS]),
    Op::IfFlags {
        all: &[CHOSE_SHIELD_LIGER],
        none: &[],
        then: &[Op::FormParty(0)],
        otherwise: &[Op::IfFlags {
            all: &[CHOSE_SABER_TIGER],
            none: &[],
            then: &[Op::FormParty(1)],
            otherwise: &[Op::FormParty(2)],
        }],
    },
    Op::Call(DEPARTURE),
];

/// The party leaves in the carrier (the end of `0x0800E07C`; objects: 0 the
/// carrier, now the player, 1 the prince, 2 Regina, 3 Ace, 4 Jack, 5 the
/// hangar door): everyone boards, the castle shakes, the carrier rolls out
/// through the door, and the eastern tunnels begin.
const DEPARTURE: &[Op] = &[
    Op::Control(false),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Wait(1),
    Op::LoadMap {
        map: 6,
        player: (8, 6),
        objects: 0x0832_ABDC,
        count: 6,
    },
    Op::PlayOnce(PLAYER, 1),
    Op::Wait(1),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Dialogue(0x41),
    Op::Wait(60),
    through(1, (8, 6), PIXEL, 1),
    Op::AwaitArrival(1),
    Op::Hide(1),
    stride(2, (8, 9)),
    Op::AwaitArrival(2),
    through(2, (8, 6), PIXEL, 1),
    Op::AwaitArrival(2),
    Op::Hide(2),
    stride(3, (8, 9)),
    Op::AwaitArrival(3),
    through(3, (8, 6), PIXEL, 1),
    Op::AwaitArrival(3),
    Op::Hide(3),
    stride(4, (8, 9)),
    Op::AwaitArrival(4),
    through(4, (8, 6), PIXEL, 1),
    Op::AwaitArrival(4),
    Op::Hide(4),
    Op::Wait(60),
    Op::Call(FLASH),
    Op::Dialogue(0x42),
    Op::Wait(60),
    Op::Animate(PLAYER, 1),
    Op::Wait(60),
    Op::Sound(0x82),
    through(PLAYER, (8, 7), QUARTER, -1),
    Op::AwaitArrival(PLAYER),
    through(PLAYER, (8, 8), HALF, 0),
    Op::AwaitArrival(PLAYER),
    through(PLAYER, (8, 11), PIXEL, 1),
    Op::AwaitArrival(PLAYER),
    Op::Shift(5, 0),
    Op::PlayOnce(5, 2),
    Op::Sound(0x6D),
    Op::Wait(60),
    through(PLAYER, (8, 12), HALF, 0),
    Op::AwaitArrival(PLAYER),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Warp {
        map: 9,
        cell: (4, 1),
        facing: Some(Direction::Down),
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::End,
];

/// The first room's handler (`0x0800C794`): unless the opening was seen,
/// it marks it seen, reloads the room with the opening's cast (the player
/// at the desk, Regina, the chair, the queen and a maid) and starts it,
/// Regina already heading left.
const FIRST_ROOM: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[OPENING_SEEN],
    then: &[
        Op::Meet(0),
        Op::Flag(OPENING_SEEN, true),
        Op::LoadMap {
            map: 4,
            player: (6, 2),
            objects: 0x0832_A830,
            count: 5,
        },
        Op::Spawn(MAP_TASK, OPENING),
        walk(REGINA, (3, 4), HALF, 0),
    ],
    otherwise: &[],
}];

/// The ground floor's handler (`0x0800C70C`): once both warriors have been
/// met, the king waits in the north-east hall.
const GROUND_FLOOR: &[Op] = &[Op::IfFlags {
    all: &[MET_FIRST_WARRIOR, MET_SECOND_WARRIOR],
    none: &[],
    then: &[Op::Place(KING, KING_APPEARS_AT)],
    otherwise: &[],
}];

/// The warrior of the ground floor (`0x0800C73C`): says his line, marks him
/// met, and lets the king appear once the other one was met too.
const FIRST_WARRIOR: &[Op] = &[
    Op::Dialogue(43),
    Op::Flag(MET_FIRST_WARRIOR, true),
    Op::IfFlags {
        all: &[MET_FIRST_WARRIOR, MET_SECOND_WARRIOR],
        none: &[],
        then: &[Op::Place(KING, KING_APPEARS_AT)],
        otherwise: &[],
    },
];

/// The warrior of the second floor (`0x0800C77C`).
const SECOND_WARRIOR: &[Op] = &[Op::Dialogue(42), Op::Flag(MET_SECOND_WARRIOR, true)];

/// A castle soldier (`0x08006628`, `0x08006658`, `0x0800668C`): his own line
/// until both warriors were met, then the one urging the player to see the
/// king.
const fn soldier(own: &'static [Op]) -> Op {
    Op::IfFlags {
        all: &[MET_FIRST_WARRIOR, MET_SECOND_WARRIOR],
        none: &[],
        then: &[Op::Dialogue(0x2D)],
        otherwise: own,
    }
}

const SOLDIER_BY_THE_STAIRS: &[Op] = &[soldier(&[Op::Dialogue(0x2DC)])];
const SOLDIER_AT_THE_DOOR: &[Op] = &[soldier(&[Op::Dialogue(0x2DD)])];
const SOLDIER_IN_THE_HALL: &[Op] = &[soldier(&[Op::Dialogue(0x2DE)])];

/// The king (`0x0800CB84`): his line, and once both warriors were met the
/// throne room's cutscene, which takes the buttons away.
const KING_TALK: &[Op] = &[
    Op::Dialogue(0x2E),
    Op::IfFlags {
        all: &[MET_FIRST_WARRIOR, MET_SECOND_WARRIOR],
        none: &[KING_SPOKEN],
        then: &[
            Op::Flag(KING_SPOKEN, true),
            Op::Control(false),
            Op::Spawn(MAP_TASK, THRONE_SUMMONS),
        ],
        otherwise: &[],
    },
];

/// Regina offers the Shield Liger (`0x0800CBEC`): her pitch and a yes/no;
/// yes picks it.
const REGINA_OFFER: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[CHOSE_SHIELD_LIGER],
    then: &[
        Op::Dialogue(0x3E),
        Op::IfChoice {
            then: &[Op::Flag(CHOSE_SHIELD_LIGER, true)],
            otherwise: &[],
        },
    ],
    otherwise: &[],
}];

/// Ace offers the Saber Tiger (`0x0800CC24`).
const ACE_OFFER: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[CHOSE_SABER_TIGER],
    then: &[
        Op::Dialogue(0x3F),
        Op::IfChoice {
            then: &[Op::Flag(CHOSE_SABER_TIGER, true)],
            otherwise: &[],
        },
    ],
    otherwise: &[],
}];

/// Jack offers the Raynos (`0x0800CC5C`).
const JACK_OFFER: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[CHOSE_RAYNOS],
    then: &[
        Op::Dialogue(0x40),
        Op::IfChoice {
            then: &[Op::Flag(CHOSE_RAYNOS, true)],
            otherwise: &[],
        },
    ],
    otherwise: &[],
}];

/// Set once the party has noticed the tunnel's end in `mq0158`.
pub const TUNNEL_NOTICED: u16 = 0x126;
/// Set once Regina has found the Trinity Liger in `mq0159`.
pub const TRINITY_FOUND: u16 = 0x127;
const TRINITY_LIGER: u8 = 0x8F;

/// The long tunnel's event (task at `0x0800E6F8`): at column 23 the party
/// stops and talks.
const TUNNEL_TALK: &[Op] = &[
    Op::AwaitPlayer {
        columns: Some((23, 23)),
        rows: None,
    },
    Op::Flag(TUNNEL_NOTICED, true),
    Op::Control(false),
    Op::Wait(30),
    Op::Dialogue(0x43),
    Op::Wait(30),
    Op::Control(true),
    Op::End,
];

/// The long tunnel's handler (`0x0800E6B8`).
const LONG_TUNNEL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[TUNNEL_NOTICED],
    then: &[Op::Spawn(MAP_TASK, TUNNEL_TALK)],
    otherwise: &[],
}];

/// The Trinity Liger (task at `0x0800E78C`): on column 2, rows 2 or 3,
/// the carrier stops by the wreck, Regina recognizes it and keeps its data.
const TRINITY: &[Op] = &[
    Op::AwaitPlayer {
        columns: Some((2, 2)),
        rows: Some((2, 3)),
    },
    Op::Flag(TRINITY_FOUND, true),
    stride(PLAYER, (1, 3)),
    Op::AwaitArrival(PLAYER),
    Op::Face(PLAYER, Direction::Down),
    Op::Wait(30),
    Op::Dialogue(0x2C0),
    Op::Wait(30),
    Op::SeeZoid(TRINITY_LIGER),
    Op::Control(true),
    Op::End,
];

/// The tunnels' exit's handler (`0x0800E758`).
const TUNNEL_EXIT: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[TRINITY_FOUND],
    then: &[Op::Spawn(MAP_TASK, TRINITY)],
    otherwise: &[],
}];

/// Set the first time the Gustav comes out onto the world map.
pub const WORLD_REACHED: u16 = 0x11E;
/// The door of アーカナの町 on the world map.
const ARCANA_GATE: (usize, usize) = (14, 7);

/// Out of the labyrinth (task at `0x080103B4`): Regina proposes the
/// nearby town of Arcana and the Gustav drives off toward it. The last walk
/// ends against the town's door, which the Gustav takes; the task ends a
/// second after starting it, while the Gustav still drives. The task's
/// first second counts from the frame the fade in ends.
const TO_ARCANA: &[Op] = &[
    Op::Wait(60),
    Op::Dialogue(0x44),
    stride(PLAYER, (11, 6)),
    Op::AwaitArrival(PLAYER),
    stride(PLAYER, (11, 7)),
    Op::AwaitArrival(PLAYER),
    stride(PLAYER, ARCANA_GATE),
    Op::Wait(60),
    Op::End,
];

/// The world map's handler (`0x08010358`): the first time, the Gustav
/// faces right, stands still and the drive to Arcana starts.
const WORLD_MAP: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[WORLD_REACHED],
    then: &[
        Op::Flag(WORLD_REACHED, true),
        Op::Face(PLAYER, Direction::Right),
        Op::Spawn(MAP_TASK, TO_ARCANA),
        Op::Control(false),
    ],
    otherwise: &[],
}];

/// Set when the party first walks into Arcana.
pub const ARCANA_ARRIVED: u16 = 0x128;
/// Set by Dr. T's first talk with Regina, in his lab.
pub const DR_T_MET: u16 = 0x13F;
const ARCANA: usize = 24;
const ARCANA_MUSIC: u16 = 6;
const BAR: usize = 28;
const BAR_UPSTAIRS: usize = 29;
/// Characters of the arrival: 1 Regina, 2 Ace, 3 Jack, 4 Roman, 5–7 the
/// soldiers (the list at ROM `0x0832AC54`); upstairs the same three and
/// Roman (`0x0832ACF4`).
const ACE: usize = 2;
const JACK: usize = 3;
const ROMAN: usize = 4;
const CAPTAIN: usize = 5;
const FIRST_SOLDIER: usize = 6;
const SECOND_SOLDIER: usize = 7;
/// The bar's door, where the party goes in.
const BAR_FRONT: (usize, usize) = (23, 10);
const BAR_DOOR: (usize, usize) = (23, 9);
/// The top of the stairs, where the party leaves the room above the bar.
const STAIRS_TOP: (usize, usize) = (14, 2);

/// Jack looks around town (task at `0x0800FBA0`).
const JACK_LOOKS_AROUND: &[Op] = &[
    stride(JACK, (17, 25)),
    Op::AwaitArrival(JACK),
    stride(JACK, (17, 26)),
    Op::AwaitArrival(JACK),
    stride(JACK, (2, 26)),
    Op::AwaitArrival(JACK),
    stride(JACK, (2, 18)),
    Op::AwaitArrival(JACK),
    stride(JACK, (17, 18)),
    Op::AwaitArrival(JACK),
    stride(JACK, (17, 13)),
    Op::AwaitArrival(JACK),
    stride(JACK, (20, 11)),
    Op::AwaitArrival(JACK),
    stride(JACK, (22, 11)),
    Op::AwaitArrival(JACK),
    Op::Face(JACK, Direction::Right),
    Op::End,
];

/// Ace looks around (`0x0800FE14`).
const ACE_LOOKS_AROUND: &[Op] = &[
    stride(ACE, (17, 24)),
    Op::AwaitArrival(ACE),
    stride(ACE, (17, 18)),
    Op::AwaitArrival(ACE),
    stride(ACE, (8, 18)),
    Op::AwaitArrival(ACE),
    stride(ACE, (8, 7)),
    Op::AwaitArrival(ACE),
    through(ACE, (17, 7), PIXEL, 1),
    Op::AwaitArrival(ACE),
    stride(ACE, (17, 12)),
    Op::AwaitArrival(ACE),
    stride(ACE, (17, 12)),
    Op::AwaitArrival(ACE),
    stride(ACE, (23, 12)),
    Op::AwaitArrival(ACE),
    Op::Face(ACE, Direction::Up),
    Op::End,
];

/// Regina looks around (`0x08010088`).
const REGINA_LOOKS_AROUND: &[Op] = &[
    stride(REGINA, (27, 24)),
    Op::AwaitArrival(REGINA),
    stride(REGINA, (27, 10)),
    Op::AwaitArrival(REGINA),
    stride(REGINA, (28, 10)),
    Op::AwaitArrival(REGINA),
    stride(REGINA, (28, 9)),
    Op::AwaitArrival(REGINA),
    stride(REGINA, (38, 8)),
    Op::AwaitArrival(REGINA),
    stride(REGINA, (45, 8)),
    Op::AwaitArrival(REGINA),
    stride(REGINA, (30, 8)),
    Op::AwaitArrival(REGINA),
    stride(REGINA, (28, 10)),
    Op::AwaitArrival(REGINA),
    stride(REGINA, (24, 11)),
    Op::AwaitArrival(REGINA),
    Op::Face(REGINA, Direction::Left),
    Op::End,
];

/// One of the party goes into the bar and out of sight.
const fn into_the_bar(actor: usize) -> [Op; 5] {
    [
        stride(actor, BAR_FRONT),
        Op::AwaitArrival(actor),
        through(actor, BAR_DOOR, PIXEL, 1),
        Op::AwaitArrival(actor),
        Op::Hide(actor),
    ]
}

const PRINCE_INTO_THE_BAR: &[Op] = &into_the_bar(PLAYER);
const REGINA_INTO_THE_BAR: &[Op] = &into_the_bar(REGINA);
const ACE_INTO_THE_BAR: &[Op] = &into_the_bar(ACE);
const JACK_INTO_THE_BAR: &[Op] = &into_the_bar(JACK);

/// Arriving in Arcana (task at `0x0800E8B8`): the party splits up to look
/// around, the prince finds the bar, soldiers enforcing a curfew stop the
/// party, the old Roman passes them off as his relatives and bribes the
/// captain away, then takes everyone into the bar.
const ARCANA_ARRIVAL: &[Op] = &[
    Op::Wait(60),
    stride(PLAYER, (23, 26)),
    Op::AwaitArrival(PLAYER),
    Op::Place(REGINA, (23, 26)),
    Op::Place(ACE, (23, 26)),
    Op::Place(JACK, (23, 26)),
    through(JACK, (22, 25), PIXEL, 1),
    through(ACE, (23, 24), PIXEL, 1),
    through(REGINA, (24, 25), PIXEL, 1),
    Op::AwaitArrival(REGINA),
    Op::Face(JACK, Direction::Right),
    Op::Face(ACE, Direction::Down),
    Op::Face(REGINA, Direction::Left),
    Op::Wait(60),
    Op::Dialogue(0x49),
    Op::Wait(60),
    Op::Spawn(HELPER_TASK, JACK_LOOKS_AROUND),
    Op::Spawn(HELPER_TASK + 1, ACE_LOOKS_AROUND),
    Op::Spawn(HELPER_TASK + 2, REGINA_LOOKS_AROUND),
    Op::Wait(60),
    Op::Wait(60),
    stride(PLAYER, (23, 23)),
    Op::AwaitArrival(PLAYER),
    stride(PLAYER, (27, 23)),
    Op::AwaitArrival(PLAYER),
    stride(PLAYER, (27, 10)),
    Op::AwaitArrival(PLAYER),
    Op::Face(PLAYER, Direction::Right),
    Op::Wait(30),
    Op::Face(PLAYER, Direction::Left),
    Op::Wait(30),
    stride(PLAYER, (21, 10)),
    Op::AwaitArrival(PLAYER),
    Op::Face(PLAYER, Direction::Up),
    Op::Wait(60),
    stride(PLAYER, BAR_FRONT),
    Op::AwaitArrival(PLAYER),
    Op::Face(PLAYER, Direction::Up),
    Op::Wait(30),
    Op::Face(PLAYER, Direction::Right),
    Op::Wait(30),
    Op::Face(PLAYER, Direction::Left),
    Op::Wait(30),
    Op::Wait(60),
    Op::Wait(60),
    Op::Face(PLAYER, Direction::Right),
    Op::Wait(30),
    Op::AwaitArrival(REGINA),
    Op::Face(PLAYER, Direction::Down),
    Op::Wait(30),
    Op::Wait(60),
    Op::Dialogue(0x4A),
    Op::Wait(60),
    Op::Call(SOLDIERS_ARRIVE),
];

/// The soldiers march up and surround the party.
const SOLDIERS_ARRIVE: &[Op] = &[
    stride(CAPTAIN, (17, 9)),
    stride(FIRST_SOLDIER, (17, 8)),
    stride(SECOND_SOLDIER, (17, 7)),
    Op::AwaitArrival(CAPTAIN),
    stride(CAPTAIN, (17, 11)),
    Op::AwaitArrival(FIRST_SOLDIER),
    stride(FIRST_SOLDIER, (17, 12)),
    Op::AwaitArrival(SECOND_SOLDIER),
    stride(SECOND_SOLDIER, (17, 10)),
    Op::AwaitArrival(CAPTAIN),
    stride(CAPTAIN, (20, 11)),
    Op::AwaitArrival(FIRST_SOLDIER),
    stride(FIRST_SOLDIER, (19, 12)),
    Op::AwaitArrival(SECOND_SOLDIER),
    stride(SECOND_SOLDIER, (19, 10)),
    Op::AwaitArrival(SECOND_SOLDIER),
    Op::Wait(10),
    Op::Face(JACK, Direction::Left),
    Op::Wait(10),
    Op::Face(ACE, Direction::Left),
    Op::Wait(10),
    Op::Face(PLAYER, Direction::Left),
    Op::Wait(10),
    Op::Face(REGINA, Direction::Left),
    Op::Wait(60),
    Op::Dialogue(0x4B),
    Op::Wait(60),
    Op::Call(ROMAN_STEPS_IN),
];

/// Roman comes out, talks the captain round and the soldiers leave.
const ROMAN_STEPS_IN: &[Op] = &[
    through(ROMAN, (26, 11), PIXEL, 1),
    Op::AwaitArrival(ROMAN),
    stride(ROMAN, (26, 13)),
    Op::AwaitArrival(ROMAN),
    stride(ROMAN, (22, 13)),
    Op::AwaitArrival(ROMAN),
    stride(ROMAN, (22, 12)),
    Op::AwaitArrival(ROMAN),
    Op::Face(ROMAN, Direction::Up),
    Op::Wait(10),
    Op::Face(JACK, Direction::Down),
    Op::Wait(60),
    Op::Dialogue(0x4C),
    Op::Wait(60),
    stride(ROMAN, (20, 12)),
    Op::Wait(10),
    Op::Face(JACK, Direction::Left),
    Op::AwaitArrival(ROMAN),
    Op::Face(ROMAN, Direction::Up),
    Op::Wait(10),
    Op::Face(CAPTAIN, Direction::Down),
    Op::Wait(60),
    Op::Dialogue(0x4D),
    Op::Wait(60),
    stride(CAPTAIN, (17, 11)),
    Op::AwaitArrival(CAPTAIN),
    stride(CAPTAIN, (17, 9)),
    stride(SECOND_SOLDIER, (17, 10)),
    stride(FIRST_SOLDIER, (17, 12)),
    Op::AwaitArrival(CAPTAIN),
    stride(CAPTAIN, (4, 9)),
    Op::AwaitArrival(SECOND_SOLDIER),
    stride(SECOND_SOLDIER, (17, 9)),
    stride(FIRST_SOLDIER, (17, 9)),
    Op::AwaitArrival(SECOND_SOLDIER),
    stride(SECOND_SOLDIER, (5, 9)),
    Op::AwaitArrival(FIRST_SOLDIER),
    stride(FIRST_SOLDIER, (5, 9)),
    stride(ROMAN, (20, 10)),
    Op::AwaitArrival(ROMAN),
    stride(ROMAN, (22, 10)),
    Op::AwaitArrival(ROMAN),
    Op::Face(JACK, Direction::Up),
    Op::Wait(10),
    Op::Face(ACE, Direction::Up),
    Op::Wait(60),
    Op::Dialogue(0x4E),
    Op::Wait(60),
    stride(PLAYER, (24, 10)),
    Op::Wait(10),
    stride(ROMAN, BAR_FRONT),
    Op::AwaitArrival(ROMAN),
    Op::Face(PLAYER, Direction::Left),
    through(ROMAN, BAR_DOOR, PIXEL, 1),
    Op::AwaitArrival(ROMAN),
    Op::Hide(ROMAN),
    Op::Call(PRINCE_INTO_THE_BAR),
    Op::Call(REGINA_INTO_THE_BAR),
    Op::Call(ACE_INTO_THE_BAR),
    Op::Call(JACK_INTO_THE_BAR),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Wait(1),
    Op::LoadMap {
        map: BAR_UPSTAIRS,
        player: (4, 3),
        objects: 0x0832_ACF4,
        count: 5,
    },
    Op::Wait(1),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Wait(60),
    Op::Call(ABOVE_THE_BAR),
];

/// In the room above the bar: Roman tells what he knows of the castle, the
/// prince storms off with Regina behind him, the camera follows them to
/// the stairs, and Jack and Ace go after them; the prince comes down into
/// the bar.
const ABOVE_THE_BAR: &[Op] = &[
    Op::Dialogue(0x4F),
    Op::Wait(60),
    stride(PLAYER, (8, 3)),
    Op::AwaitArrival(PLAYER),
    stride(REGINA, (6, 3)),
    stride(PLAYER, (8, 2)),
    Op::AwaitArrival(PLAYER),
    through(PLAYER, STAIRS_TOP, PIXEL, 1),
    stride(REGINA, (8, 3)),
    Op::AwaitArrival(REGINA),
    stride(REGINA, (8, 2)),
    Op::AwaitArrival(REGINA),
    through(REGINA, STAIRS_TOP, PIXEL, 1),
    Op::AwaitArrival(PLAYER),
    Op::Hide(PLAYER),
    Op::AwaitArrival(REGINA),
    Op::Hide(REGINA),
    Op::Repeat(32, PAN_LEFT),
    Op::Wait(60),
    Op::Dialogue(0x50),
    Op::Wait(60),
    through(ACE, STAIRS_TOP, PIXEL, 1),
    through(JACK, STAIRS_TOP, PIXEL, 1),
    Op::AwaitArrival(ACE),
    Op::Hide(ACE),
    Op::AwaitArrival(JACK),
    Op::Hide(JACK),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Warp {
        map: BAR,
        cell: (13, 12),
        facing: Some(Direction::Left),
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::End,
];

/// Arcana's handler (`0x0800E850`): the first time, the town reloads with
/// the arrival's cast and its song, and the arrival starts.
const ARCANA_STREETS: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[ARCANA_ARRIVED],
    then: &[
        Op::Flag(ARCANA_ARRIVED, true),
        Op::LoadMap {
            map: ARCANA,
            player: (23, 29),
            objects: 0x0832_AC54,
            count: 8,
        },
        Op::Music(ARCANA_MUSIC),
        Op::Spawn(MAP_TASK, ARCANA_ARRIVAL),
    ],
    otherwise: &[],
}];

/// Set the first time the party comes back to the castle's grounds.
pub const CASTLE_GROUNDS_REACHED: u16 = 0x129;
const CASTLE_GROUNDS: usize = 7;
const CASTLE_GROUNDS_MUSIC: u16 = 4;

/// Back at the castle (task at `0x08010514`): the camera goes up the
/// grounds for 100 frames and comes back down, and a second later Jack
/// finds the guard thin (dialogue `0x51`).
const CASTLE_GROUNDS_VIEW: &[Op] = &[
    Op::Repeat(100, PAN_UP),
    Op::Repeat(100, PAN_DOWN),
    Op::Wait(60),
    Op::Dialogue(0x51),
    Op::Wait(60),
    Op::Control(true),
    Op::End,
];

/// The castle grounds' handler (`0x080104A0`): the first time after
/// Arcana, the castle's song, the Gustav facing up and the view.
const CASTLE_GROUNDS_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[ARCANA_ARRIVED],
    none: &[CASTLE_GROUNDS_REACHED],
    then: &[
        Op::Flag(CASTLE_GROUNDS_REACHED, true),
        Op::Music(CASTLE_GROUNDS_MUSIC),
        Op::Face(PLAYER, Direction::Up),
        Op::Spawn(MAP_TASK, CASTLE_GROUNDS_VIEW),
        Op::Control(false),
    ],
    otherwise: &[],
}];

/// Set once Regina has opened the factory's door with the pass code.
pub const FACTORY_DOOR_OPENED: u16 = 0x12A;
const FACTORY_DOOR_ROOM: usize = 16;
/// The door (object 1), which stands off the map once opened.
const FACTORY_DOOR: usize = 1;
const FACTORY_DOOR_CELL: (usize, usize) = (4, 0);
const DOOR_OPENING: usize = 2;
const DOOR_SOUND: u16 = 0x6D;
const DRIVE_OFF_SOUND: u16 = 0x82;

/// Before the factory's door (task at `0x080105F0`): on the cell in front
/// of it the Gustav stops, Regina gives the pass code (dialogue `0x52`),
/// the door opens with its sound and a second later the Gustav drives
/// through it at half a pixel a frame, its animation at whole ticks; the
/// player has the controls back from the next frame, the step going on.
const FACTORY_DOOR_TASK: &[Op] = &[
    Op::AwaitPlayer {
        columns: Some((4, 4)),
        rows: Some((1, 1)),
    },
    Op::Flag(FACTORY_DOOR_OPENED, true),
    Op::Control(false),
    Op::Wait(60),
    Op::Dialogue(0x52),
    Op::Wait(60),
    Op::Shift(FACTORY_DOOR, 0),
    Op::PlayOnce(FACTORY_DOOR, DOOR_OPENING),
    Op::Sound(DOOR_SOUND),
    Op::Wait(60),
    Op::Sound(DRIVE_OFF_SOUND),
    through(PLAYER, FACTORY_DOOR_CELL, HALF, 0),
    Op::Wait(1),
    Op::Control(true),
    Op::End,
];

/// The factory door's room (`0x08010588`): until it is opened the door
/// stands in its frame, and once Arcana is done the task waits for the
/// party.
const FACTORY_DOOR_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[FACTORY_DOOR_OPENED],
    then: &[
        Op::Place(FACTORY_DOOR, FACTORY_DOOR_CELL),
        Op::IfFlags {
            all: &[ARCANA_ARRIVED],
            none: &[],
            then: &[Op::Spawn(MAP_TASK, FACTORY_DOOR_TASK)],
            otherwise: &[],
        },
    ],
    otherwise: &[],
}];

/// Set once the party has beaten Blood's squad in the factory's hall.
pub const HALL_BATTLE_WON: u16 = 0x12B;
/// Set by the party's first crossing of the factory's corridor.
pub const CORRIDOR_CROSSED: u16 = 0x12C;
/// Set at the corridor's far end.
pub const CORRIDOR_END_REACHED: u16 = 0x12D;
const FACTORY_HALL: usize = 12;
const FACTORY_HALL_AFTER: usize = 13;
const FACTORY_CORRIDOR: usize = 17;
/// The hall with Blood and three soldiers below it (ROM `0x0832AD80`), and
/// after the battle the Gustav, Blood, two soldiers wrecked and a third
/// (`0x0832ADD0`).
const HALL_OBJECTS: u32 = 0x0832_AD80;
const HALL_AFTER_OBJECTS: u32 = 0x0832_ADD0;
const BLOOD: usize = 1;
const LEFT_SOLDIER: usize = 2;
const RIGHT_SOLDIER: usize = 3;
const LAST_SOLDIER: usize = 4;
const HALL_MUSIC: u16 = 9;
const AFTER_BATTLE_MUSIC: u16 = 4;
const BLAST_SOUND: u16 = 0x5A;
const HALL_BATTLE: u8 = 0;

/// A walk through everything at two pixels a frame, a quarter-tick
/// animation.
const fn dash(actor: usize, to: (usize, usize)) -> Op {
    through(actor, to, DOUBLE, 2)
}

/// A story battle (`0x08008D28`): sound `0x52`, the field darkens to
/// black, the battle runs and the field is set up again in the dark.
const fn story_battle(battle: u8) -> [Op; 4] {
    [
        Op::Sound(ENCOUNTER_SOUND),
        Op::FadeOutHoldingAfter(ENCOUNTER_FADE_DELAY),
        Op::StoryBattle(battle),
        Op::Freeze(ENCOUNTER_RELOAD_FRAMES),
    ]
}

/// Beaten, the party is taken to its return point (the defeat's branch of
/// `0x08010754` and `0x08011408`).
const STORY_BATTLE_LOST: &[Op] = &[
    Op::WarpHome,
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::End,
];

/// Blood's squad beaten (the victory's branch of `0x08010754`): the hall
/// is set up again, the two wrecked soldiers blow up, the camera goes up,
/// the Gustav drives out the top (dialogues `0x55`, `0x56`), Blood and the
/// last soldier flee the same way, and the party goes on into the
/// corridor, whose task follows. The code walks Blood again where it meant
/// the last soldier; Blood is hidden by then.
const HALL_WON: &[Op] = &[
    Op::Flag(HALL_BATTLE_WON, true),
    Op::LoadMap {
        map: FACTORY_HALL_AFTER,
        player: (4, 3),
        objects: HALL_AFTER_OBJECTS,
        count: 5,
    },
    Op::Wait(1),
    Op::Place(PLAYER, (4, 2)),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Wait(60),
    Op::Shift(LEFT_SOLDIER, 1),
    Op::Shift(RIGHT_SOLDIER, 1),
    Op::PlayOnce(LEFT_SOLDIER, 3),
    Op::PlayOnce(RIGHT_SOLDIER, 2),
    Op::AwaitAnimation(LEFT_SOLDIER),
    Op::PlayOnce(LEFT_SOLDIER, 0),
    Op::Sprite(LEFT_SOLDIER, crate::field::EXPLOSION_SPRITE),
    Op::PlayOnce(RIGHT_SOLDIER, 0),
    Op::Sprite(RIGHT_SOLDIER, crate::field::EXPLOSION_SPRITE),
    Op::Sound(BLAST_SOUND),
    Op::AwaitAnimation(LEFT_SOLDIER),
    Op::Hide(LEFT_SOLDIER),
    Op::Hide(RIGHT_SOLDIER),
    Op::Repeat(16, PAN_UP),
    Op::Wait(60),
    Op::Dialogue(0x55),
    Op::Wait(60),
    dash(PLAYER, (4, 1)),
    Op::AwaitArrival(PLAYER),
    dash(PLAYER, (5, 1)),
    Op::AwaitArrival(PLAYER),
    dash(PLAYER, (5, 0)),
    Op::AwaitArrival(PLAYER),
    Op::Hide(PLAYER),
    Op::Wait(60),
    Op::Dialogue(0x56),
    Op::Wait(60),
    dash(BLOOD, (4, 5)),
    Op::AwaitArrival(BLOOD),
    dash(BLOOD, (2, 5)),
    Op::AwaitArrival(BLOOD),
    dash(BLOOD, (2, 1)),
    Op::AwaitArrival(BLOOD),
    dash(BLOOD, (5, 1)),
    Op::Place(LAST_SOLDIER, (2, 5)),
    dash(LAST_SOLDIER, (2, 1)),
    Op::AwaitArrival(BLOOD),
    dash(BLOOD, (5, 0)),
    Op::AwaitArrival(BLOOD),
    Op::Hide(BLOOD),
    Op::AwaitArrival(LAST_SOLDIER),
    dash(BLOOD, (5, 1)),
    Op::AwaitArrival(LAST_SOLDIER),
    dash(BLOOD, (5, 0)),
    Op::AwaitArrival(LAST_SOLDIER),
    Op::Hide(LAST_SOLDIER),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Warp {
        map: FACTORY_CORRIDOR,
        cell: (4, 6),
        facing: Some(Direction::Up),
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    // The task ends setting the field's hook `0x0801112C`, which the next
    // frame spawns the corridor's task (the one the corridor's handler
    // spawned during the warp was lost).
    Op::Wait(1),
    Op::Call(CORRIDOR_TASK),
];

/// Blood's ambush in the factory's hall (task at `0x08010754`): on the
/// cell (4, 1) the Gustav comes down a cell and stops (dialogue `0x53`),
/// the camera goes down, the hall's song starts and three soldiers and
/// Blood come in and take their places, the camera comes back up, Blood
/// speaks (dialogue `0x54`) and story battle 0 follows.
const HALL_AMBUSH: &[Op] = &[
    Op::AwaitPlayer {
        columns: Some((4, 4)),
        rows: Some((1, 1)),
    },
    stride(PLAYER, (4, 2)),
    Op::AwaitArrival(PLAYER),
    Op::Wait(60),
    Op::Dialogue(0x53),
    Op::Wait(60),
    Op::Repeat(64, PAN_DOWN),
    Op::Music(HALL_MUSIC),
    Op::Place(LEFT_SOLDIER, (4, 6)),
    stride(LEFT_SOLDIER, (4, 5)),
    Op::AwaitArrival(LEFT_SOLDIER),
    stride(LEFT_SOLDIER, (2, 5)),
    Op::Place(RIGHT_SOLDIER, (4, 6)),
    stride(RIGHT_SOLDIER, (4, 5)),
    Op::AwaitArrival(RIGHT_SOLDIER),
    stride(RIGHT_SOLDIER, (6, 5)),
    Op::AwaitArrival(LEFT_SOLDIER),
    stride(LEFT_SOLDIER, (2, 3)),
    Op::AwaitArrival(RIGHT_SOLDIER),
    stride(RIGHT_SOLDIER, (6, 3)),
    Op::AwaitArrival(LEFT_SOLDIER),
    stride(LEFT_SOLDIER, (3, 3)),
    Op::AwaitArrival(RIGHT_SOLDIER),
    stride(RIGHT_SOLDIER, (5, 3)),
    Op::Place(BLOOD, (4, 6)),
    stride(BLOOD, (4, 4)),
    Op::AwaitArrival(BLOOD),
    Op::Repeat(16, PAN_UP),
    Op::Wait(60),
    Op::Dialogue(0x54),
    Op::Wait(60),
    Op::Call(&story_battle(HALL_BATTLE)),
    Op::Music(AFTER_BATTLE_MUSIC),
    Op::Wait(1),
    Op::IfLost {
        then: STORY_BATTLE_LOST,
        otherwise: HALL_WON,
    },
];

/// The factory's hall (`0x080106C4`, maps 12 and 13): until Blood's squad
/// is beaten the hall reloads with them waiting below and the ambush
/// starts.
const FACTORY_HALL_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[HALL_BATTLE_WON],
    then: &[
        Op::LoadMap {
            map: FACTORY_HALL,
            player: HERE,
            objects: HALL_OBJECTS,
            count: 4,
        },
        Op::Spawn(MAP_TASK, HALL_AMBUSH),
        Op::Control(true),
    ],
    otherwise: &[],
}];

/// The factory's corridor (task at `0x0801114C`): the first time the
/// Gustav drives in to (14, 2) and Regina speaks (dialogue `0x57`); at the
/// corridor's far end, cell (37, 1), it stops for dialogue `0x59`.
const CORRIDOR_TASK: &[Op] = &[
    Op::IfFlags {
        all: &[],
        none: &[CORRIDOR_CROSSED],
        then: &[
            Op::Flag(CORRIDOR_CROSSED, true),
            stride(PLAYER, (4, 2)),
            Op::AwaitArrival(PLAYER),
            stride(PLAYER, (14, 2)),
            Op::AwaitArrival(PLAYER),
            Op::Wait(60),
            Op::Dialogue(0x57),
            Op::Wait(60),
        ],
        otherwise: &[],
    },
    Op::Control(true),
    Op::IfFlags {
        all: &[],
        none: &[CORRIDOR_END_REACHED],
        then: &[
            Op::AwaitPlayer {
                columns: Some((37, 37)),
                rows: Some((1, 1)),
            },
            Op::Control(false),
            Op::AwaitArrival(PLAYER),
            Op::Flag(CORRIDOR_END_REACHED, true),
            Op::Dialogue(0x59),
            Op::Wait(60),
            Op::Control(true),
        ],
        otherwise: &[],
    },
    Op::End,
];

/// The factory's corridor (`0x080110C8`): until its two events are done,
/// the Gustav stands still and the corridor's task starts.
const FACTORY_CORRIDOR_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CORRIDOR_CROSSED, CORRIDOR_END_REACHED],
    none: &[],
    then: &[],
    otherwise: &[Op::Spawn(MAP_TASK, CORRIDOR_TASK), Op::Control(false)],
}];

/// Set once the party has beaten Blood at the space-time transfer device.
pub const DEVICE_BATTLE_WON: u16 = 0x12E;
const DEVICE_ROOM: usize = 23;
/// The device room with the party's four Zoids, Blood, his officer and the
/// device (ROM `0x0832AE34`).
const DEVICE_ROOM_OBJECTS: u32 = 0x0832_AE34;
const DEVICE_ROOM_MUSIC: u16 = 9;
const AFTER_DEVICE_BATTLE_MUSIC: u16 = 0x1E;
const DEVICE_BATTLE: u8 = 1;
const FIRST_ZOID: usize = 1;
const SECOND_ZOID: usize = 2;
const THIRD_ZOID: usize = 3;
const FOURTH_ZOID: usize = 4;
const DEVICE_BLOOD: usize = 5;
const OFFICER: usize = 6;
const DEVICE: usize = 7;
const DEVICE_CELL: (usize, usize) = (4, 1);
const OFF_THE_MAP: (usize, usize) = (0xFF, 0xFF);
const DEVICE_RUNNING: usize = 1;
const DEVICE_IDLE: usize = 0;
const DEVICE_SOUND: u16 = 0x6F;
const DEVICE_GONE_SOUND: u16 = 0x44;
/// The step of the device's run on whose last frame the one on it is gone,
/// and the one of its second sound.
const DEVICE_TAKES_STEP: usize = 8;
const DEVICE_SOUND_STEP: usize = 41;
const PAN_RIGHT_SLOW: &[Op] = &[Op::Pan(PIXEL, 0), Op::Wait(1)];
const PAN_UP_SLOW: &[Op] = &[Op::Pan(0, -PIXEL), Op::Wait(1)];

/// The device sends away whoever stands on it (the loops of `0x08011408`):
/// it runs once with sound `0x6F`, the one on it goes on the last frame of
/// its eighth step, sound `0x44` plays on its 41st, and it stands again.
const fn device_takes(actor: usize) -> [Op; 8] {
    [
        Op::PlayOnce(DEVICE, DEVICE_RUNNING),
        Op::Sound(DEVICE_SOUND),
        Op::AwaitStepEnd(DEVICE, DEVICE_TAKES_STEP),
        Op::Place(actor, OFF_THE_MAP),
        Op::AwaitStepEnd(DEVICE, DEVICE_SOUND_STEP),
        Op::Sound(DEVICE_GONE_SOUND),
        Op::AwaitAnimation(DEVICE),
        Op::Animate(DEVICE, DEVICE_IDLE),
    ]
}

/// A slow walk through everything onto the device, which sends the walker
/// away.
const fn onto_device(actor: usize) -> [Op; 9] {
    let takes = device_takes(actor);
    [
        through(actor, DEVICE_CELL, PIXEL, 1),
        takes[0],
        takes[1],
        takes[2],
        takes[3],
        takes[4],
        takes[5],
        takes[6],
        takes[7],
    ]
}

const OFFICER_LEAVES: [Op; 9] = onto_device(OFFICER);
const FIRST_LEAVES: [Op; 9] = onto_device(FIRST_ZOID);
const SECOND_LEAVES: [Op; 9] = onto_device(SECOND_ZOID);
const PLAYER_LEAVES: [Op; 9] = onto_device(PLAYER);

/// Blood beaten at the device (the victory's branch of `0x08011408`): he
/// backs away and is gone (dialogues `0x5C`, `0x5D`), the first two Zoids
/// take the device, then (dialogue `0x5E`) the other two drive off the
/// left edge and the Gustav takes the device last; the party lands in
/// map 48, whose scene the task runs itself (the one the map's handler
/// spawns during the warp is lost).
const DEVICE_BATTLE_WON_SCENE: &[Op] = &[
    Op::Flag(DEVICE_BATTLE_WON, true),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Wait(60),
    Op::Dialogue(0x5C),
    Op::Wait(60),
    through(DEVICE_BLOOD, (4, 5), PIXEL, 1),
    Op::AwaitArrival(DEVICE_BLOOD),
    Op::Hide(DEVICE_BLOOD),
    Op::Wait(60),
    Op::Dialogue(0x5D),
    Op::Wait(60),
    stride(FIRST_ZOID, (4, 3)),
    Op::AwaitArrival(FIRST_ZOID),
    through(FIRST_ZOID, (4, 2), PIXEL, 1),
    Op::AwaitArrival(FIRST_ZOID),
    Op::Call(&FIRST_LEAVES),
    stride(SECOND_ZOID, (4, 4)),
    Op::AwaitArrival(SECOND_ZOID),
    through(SECOND_ZOID, (4, 2), PIXEL, 1),
    Op::AwaitArrival(SECOND_ZOID),
    Op::Call(&SECOND_LEAVES),
    Op::Wait(60),
    Op::Dialogue(0x5E),
    Op::Wait(60),
    Op::Place(PLAYER, (0, 4)),
    stride(PLAYER, (2, 4)),
    Op::AwaitArrival(PLAYER),
    through(THIRD_ZOID, (2, 4), PIXEL, 1),
    Op::AwaitArrival(THIRD_ZOID),
    Op::Hide(THIRD_ZOID),
    through(FOURTH_ZOID, (2, 4), PIXEL, 1),
    Op::AwaitArrival(FOURTH_ZOID),
    Op::Hide(FOURTH_ZOID),
    stride(PLAYER, (4, 4)),
    Op::AwaitArrival(PLAYER),
    through(PLAYER, (4, 2), PIXEL, 1),
    Op::AwaitArrival(PLAYER),
    Op::Call(&PLAYER_LEAVES),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Warp {
        map: ARCADIA_THRONE_ROOM,
        cell: (8, 1),
        facing: Some(Direction::Up),
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Call(THRONE_ROOM_SCENE),
];

/// Set once the throne room's flashback has been shown.
pub const THRONE_ROOM_SEEN: u16 = 0x143;
/// Arcadia castle's throne room, hours before (map 48): the Emperor on the
/// throne and Fran at the door (ROM `0x086671F8`).
const ARCADIA_THRONE_ROOM: usize = 48;
const THRONE_ROOM_OBJECTS: u32 = 0x0866_71F8;
const FRAN_IN_THE_THRONE_ROOM: usize = 2;
/// The land past the Red River (map 30), with the party's four Zoids, Fran
/// and the portal the device opens (ROM `0x08667234`).
const RED_RIVER: usize = 30;
const RED_RIVER_OBJECTS: u32 = 0x0866_7234;
const RED_RIVER_MUSIC: u16 = 0x1E;
/// The map's own song (its record's `+0x1A`), which comes back once the
/// party has spoken.
const RED_RIVER_MAP_MUSIC: u16 = 0x11;
const FRAN_MUSIC: u16 = 9;
const FRAN: usize = 5;
const FRAN_SOUND: u16 = 0x68;
const PORTAL: usize = 6;
const PORTAL_CELL: (usize, usize) = (19, 21);
const PORTAL_OPENING: usize = 2;
const PORTAL_IDLE: usize = 0;
const PORTAL_SOUND: u16 = 0x6F;
const PORTAL_ARRIVAL_SOUND: u16 = 0x49;
/// The step of the portal's opening on whose last frame the one it brings
/// is there.
const PORTAL_BRINGS_STEP: usize = 32;
/// Sand Colony's field, where the party goes on (map 31).
const SAND_COLONY_FIELD: usize = 31;
/// A cell off the map, left of its top row.
const BESIDE_THE_MAP: (usize, usize) = (0xFF, 0);

/// Map pixels a Zoid map's cell spans.
const ZOID_CELL: u32 = 32;

/// A glide a cell at a time at `speed` pixels a frame (`0x08011F18`).
const fn glide(actor: usize, to: (i32, i32), speed: i32, camera: bool) -> Op {
    Op::Glide {
        actor,
        to,
        speed,
        frames: ZOID_CELL / speed.unsigned_abs(),
        camera,
    }
}

/// The portal brings actor `actor` (a loop of `0x080129CC`): it opens with
/// its sound, on the last frame of its 32nd step the one it brings is
/// shown on it and sent there through everything with sound `0x49`, and
/// once it has played it stands again.
const fn portal_brings(actor: usize) -> [Op; 9] {
    [
        Op::PlayOnce(PORTAL, PORTAL_OPENING),
        Op::Sound(PORTAL_SOUND),
        Op::AwaitStepEnd(PORTAL, PORTAL_BRINGS_STEP),
        Op::Show(actor),
        Op::Place(actor, PORTAL_CELL),
        through(actor, PORTAL_CELL, PIXEL, 1),
        Op::Sound(PORTAL_ARRIVAL_SOUND),
        Op::AwaitAnimation(PORTAL),
        Op::Animate(PORTAL, PORTAL_IDLE),
    ]
}

const PORTAL_BRINGS_FIRST: [Op; 9] = portal_brings(FIRST_ZOID);
const PORTAL_BRINGS_SECOND: [Op; 9] = portal_brings(SECOND_ZOID);
const PORTAL_BRINGS_PLAYER: [Op; 9] = portal_brings(PLAYER);

/// The throne room's flashback and the party's landing (`0x080129CC`,
/// which the device room's task calls): Fran reports to the Emperor
/// (dialogue `0x5F`) and leaves, the Emperor speaks (`0x60`); past the Red
/// River the portal brings two Zoids and the Gustav (`0x61`), the Zoids
/// drive off into the Gustav, the map's song comes back and the party
/// talks (`0x63`) until Fran drives in (`0x64`) and away (`0x65`); the
/// Gustav drives on and the field darkens, and from the next frame a hook
/// (`0x08012ED4`) warps to Sand Colony's field and brightens it holding the
/// game (see [`TO_SAND_COLONY`]).
const THRONE_ROOM_SCENE: &[Op] = &[
    Op::Wait(32),
    stride(FRAN_IN_THE_THRONE_ROOM, (8, 4)),
    Op::AwaitArrival(FRAN_IN_THE_THRONE_ROOM),
    Op::Dialogue(0x5F),
    stride(FRAN_IN_THE_THRONE_ROOM, (8, 12)),
    Op::AwaitArrival(FRAN_IN_THE_THRONE_ROOM),
    Op::Dialogue(0x60),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: RED_RIVER,
        player: (19, 19),
        objects: RED_RIVER_OBJECTS,
        count: 7,
    },
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::RestartMusic(RED_RIVER_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Call(&PORTAL_BRINGS_FIRST),
    glide(FIRST_ZOID, (0x240, 0x260), 1, true),
    Op::Call(&PORTAL_BRINGS_SECOND),
    glide(SECOND_ZOID, (0x280, 0x260), 1, true),
    Op::Call(&PORTAL_BRINGS_PLAYER),
    glide(PLAYER, (0x260, 0x260), 1, false),
    Op::AwaitArrival(PLAYER),
    Op::Dialogue(0x61),
    glide(FIRST_ZOID, (0x260, 0x260), 1, true),
    Op::Place(FIRST_ZOID, BESIDE_THE_MAP),
    glide(SECOND_ZOID, (0x260, 0x260), 1, true),
    Op::Place(SECOND_ZOID, BESIDE_THE_MAP),
    Op::Wait(30),
    Op::RestartMusic(RED_RIVER_MAP_MUSIC),
    glide(PLAYER, (0x260, 0x200), 1, true),
    Op::Dialogue(0x63),
    Op::RestartMusic(FRAN_MUSIC),
    Op::Place(FRAN, (0x12, 0xC)),
    Op::Sound(FRAN_SOUND),
    glide(FRAN, (0x240, 0x200), 4, true),
    Op::Face(FRAN, Direction::Right),
    Op::Wait(60),
    Op::AwaitSoundEnd(FRAN_SOUND),
    Op::Dialogue(0x64),
    Op::Face(FRAN, Direction::Up),
    Op::Wait(60),
    Op::Sound(FRAN_SOUND),
    glide(FRAN, (0x240, 0x180), 4, true),
    Op::Place(FRAN, BESIDE_THE_MAP),
    Op::AwaitSoundEnd(FRAN_SOUND),
    Op::Dialogue(0x65),
    glide(PLAYER, (0x220, 0x1C0), 1, true),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, TO_SAND_COLONY),
    Op::End,
];

/// The hook the flashback's task leaves (`0x08012ED4`): the frame after,
/// it warps to Sand Colony's field, facing kept, and brightens it holding
/// the game, so the town's handler starts its own arrival.
const TO_SAND_COLONY: &[Op] = &[
    Op::Warp {
        map: SAND_COLONY_FIELD,
        cell: (22, 29),
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// The throne room (`0x08012948`): the first time, the party meets its
/// guide, the room loads with the Gustav off the map and the flashback's
/// two, and the player loses the controls. (The handler's own spawn of
/// the scene is lost in the warp's load; the device room's task runs it.)
const THRONE_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[THRONE_ROOM_SEEN],
    then: &[
        Op::Meet(1),
        Op::LoadMap {
            map: ARCADIA_THRONE_ROOM,
            player: (8, 2),
            objects: THRONE_ROOM_OBJECTS,
            count: 3,
        },
        Op::Control(false),
        Op::Place(PLAYER, (0xFFFF, 0xFFFF)),
        Op::Flag(THRONE_ROOM_SEEN, true),
    ],
    otherwise: &[],
}];

/// The space-time transfer device (task at `0x08011408`): the party's four
/// Zoids drive in, the camera moves over to the device, Blood's officer
/// takes it away (dialogue `0x5A`), Blood comes out to face the party
/// (`0x5B`) and story battle 1 follows.
const DEVICE_ROOM_TASK: &[Op] = &[
    Op::Place(FIRST_ZOID, (0, 3)),
    stride(FIRST_ZOID, (2, 3)),
    Op::AwaitArrival(FIRST_ZOID),
    Op::Place(SECOND_ZOID, (0, 3)),
    stride(SECOND_ZOID, (1, 3)),
    Op::AwaitArrival(SECOND_ZOID),
    stride(SECOND_ZOID, (1, 4)),
    Op::Place(FOURTH_ZOID, (0, 3)),
    stride(FOURTH_ZOID, (1, 3)),
    Op::AwaitArrival(FOURTH_ZOID),
    stride(FOURTH_ZOID, (1, 2)),
    Op::Place(THIRD_ZOID, (0, 3)),
    stride(THIRD_ZOID, (1, 3)),
    Op::AwaitArrival(THIRD_ZOID),
    Op::Face(SECOND_ZOID, Direction::Right),
    Op::Face(FOURTH_ZOID, Direction::Right),
    Op::Repeat(24, PAN_RIGHT_SLOW),
    Op::Repeat(32, PAN_UP_SLOW),
    Op::Call(&OFFICER_LEAVES),
    Op::Wait(60),
    Op::Dialogue(0x5A),
    Op::Wait(60),
    Op::Place(DEVICE_BLOOD, (4, 5)),
    stride(DEVICE_BLOOD, (4, 3)),
    Op::AwaitArrival(DEVICE_BLOOD),
    Op::Face(DEVICE_BLOOD, Direction::Left),
    Op::Wait(60),
    Op::Dialogue(0x5B),
    Op::Wait(60),
    Op::Call(&story_battle(DEVICE_BATTLE)),
    Op::Music(AFTER_DEVICE_BATTLE_MUSIC),
    Op::Wait(1),
    Op::IfLost {
        then: STORY_BATTLE_LOST,
        otherwise: DEVICE_BATTLE_WON_SCENE,
    },
];

/// The device room (`0x080113A8`): until Blood is beaten there, the room
/// reloads with the party's Zoids waiting outside and the officer at the
/// device, the hall's song plays and the scene starts.
const DEVICE_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[DEVICE_BATTLE_WON],
    then: &[
        Op::LoadMap {
            map: DEVICE_ROOM,
            player: (0, 3),
            objects: DEVICE_ROOM_OBJECTS,
            count: 8,
        },
        Op::Music(DEVICE_ROOM_MUSIC),
        Op::Spawn(MAP_TASK, DEVICE_ROOM_TASK),
    ],
    otherwise: &[],
}];

/// Dr. T in his lab (`0x0802AB08`). In area 1 he first talks with Regina
/// about the Trinity Liger, then repeats his advice; areas 9 and 10 have a
/// line each. Elsewhere his line follows the party's progress: once the
/// Trinity Liger is rebuilt (flag `0x140`) `0x2C8`; with a unit of Zoid
/// `0x90` `0x2C7`, which sets that flag; with one of `0x8F` `0x2C6`; with
/// the game state's byte `+0x3320` set `0x2C5`; the first time `0x2C3`
/// (flag `0x141`), later `0x2C4`.
const DR_T: &[Op] = &[Op::IfArea {
    area: 1,
    then: &[Op::IfFlags {
        all: &[],
        none: &[DR_T_MET],
        then: &[Op::Dialogue(0x2C1), Op::Flag(DR_T_MET, true)],
        otherwise: &[Op::Dialogue(0x2C2)],
    }],
    otherwise: &[Op::IfArea {
        area: 9,
        then: &[Op::Dialogue(0x2C9)],
        otherwise: &[Op::IfArea {
            area: 10,
            then: &[Op::Dialogue(0x2CA)],
            otherwise: DR_T_ELSEWHERE,
        }],
    }],
}];

/// Set once the party has a unit of Zoid `0x90` and Dr. T has seen it.
const TRINITY_REBUILT: u16 = 0x140;
/// Set once Dr. T has greeted the party outside area 1.
const DR_T_GREETED: u16 = 0x141;
/// The game-state byte Dr. T asks outside area 1 (what sets it is not
/// traced).
const DR_T_STATE: usize = 0x3320;

const DR_T_ELSEWHERE: &[Op] = &[Op::IfFlags {
    all: &[TRINITY_REBUILT],
    none: &[],
    then: &[Op::Dialogue(0x2C8)],
    otherwise: &[Op::IfZoidOwned {
        zoid: 0x90,
        then: &[Op::Dialogue(0x2C7), Op::Flag(TRINITY_REBUILT, true)],
        otherwise: &[Op::IfZoidOwned {
            zoid: 0x8F,
            then: &[Op::Dialogue(0x2C6)],
            otherwise: &[Op::IfStateSet {
                at: DR_T_STATE,
                then: &[Op::Dialogue(0x2C5)],
                otherwise: &[Op::IfFlags {
                    all: &[DR_T_GREETED],
                    none: &[],
                    then: &[Op::Dialogue(0x2C4)],
                    otherwise: &[Op::Dialogue(0x2C3), Op::Flag(DR_T_GREETED, true)],
                }],
            }],
        }],
    }],
}];

/// データ収集, Data Compare.
const DATA_COMPARE: u8 = 0;
/// 節電, Brown-Out.
const BROWN_OUT: u8 = 22;
/// 包囲攻撃, Enveloping Attack.
const ENVELOPING_ATTACK: u8 = 26;

const LEARN_DATA_COMPARE: &[Op] = &learn(DATA_COMPARE);
const LEARN_BROWN_OUT: &[Op] = &learn(BROWN_OUT);
const LEARN_ENVELOPING_ATTACK: &[Op] = &learn(ENVELOPING_ATTACK);

/// The teachers of deck commands (`0x08009430`, one entry `0x08009480 +
/// 12 × n` a command): the first time the teacher's pitch and the lesson,
/// later a reminder. The two lines of each come from the table at ROM
/// `0x08328EC4`.
const LAB_ASSISTANT: &[Op] = &[Op::IfCommand {
    command: DATA_COMPARE,
    then: &[Op::Dialogue(0x2CC)],
    otherwise: &[Op::Dialogue(0x2CB), Op::Call(LEARN_DATA_COMPARE)],
}];
const OLD_MAN_IN_THE_SHOP: &[Op] = &[Op::IfCommand {
    command: BROWN_OUT,
    then: &[Op::Dialogue(0x3E5)],
    otherwise: &[Op::Dialogue(0x3D0), Op::Call(LEARN_BROWN_OUT)],
}];
const ROMAN_TEACHES: &[Op] = &[Op::IfCommand {
    command: ENVELOPING_ATTACK,
    then: &[Op::Dialogue(0x2F2)],
    otherwise: &[Op::Dialogue(0x2F3), Op::Call(LEARN_ENVELOPING_ATTACK)],
}];

/// The lines around a reward's announcement: the message box's two
/// helpers before it (`battle-menu` 6 and 7) and the one after (5).
const BEFORE_REWARD: [Op; 2] = [Op::Script("battle-menu", 6), Op::Script("battle-menu", 7)];
const AFTER_REWARD: Op = Op::Script("battle-menu", 5);

/// A reward announced as its label, its name and 」を手に入れた
/// (`battle-text` 9), once given.
const fn named_reward(label: u16) -> [Op; 7] {
    [
        Op::TakeChest,
        BEFORE_REWARD[0],
        BEFORE_REWARD[1],
        Op::Script("battle-text", label),
        Op::ChestName,
        Op::Script("battle-text", 9),
        AFTER_REWARD,
    ]
}

/// Zoid core: Ｚｉデータ用アイテム「…」を手に入れた.
pub(super) const CORE_REWARD: [Op; 7] = named_reward(0x28);
/// Part: 武装「…」を手に入れた.
const PART_REWARD: [Op; 7] = named_reward(0x29);
/// Consumable: アイテム「…」を手に入れた.
const CONSUMABLE_REWARD: [Op; 7] = named_reward(0x27);
/// Zi data held already: だが、そのＺｉデータは既に持っていた・・・ after
/// the announcement.
const ZI_DATA_HELD: &[Op] = &[
    Op::TakeChest,
    Op::Script("battle-menu", 0x14),
    BEFORE_REWARD[0],
    BEFORE_REWARD[1],
    Op::Script("battle-text", 0x2C),
    AFTER_REWARD,
];
/// Zi data: Ｚｉデータ「…」を手に入れた, then whether the party had it.
pub(super) const ZI_DATA_REWARD: &[Op] = &[
    BEFORE_REWARD[0],
    BEFORE_REWARD[1],
    Op::Script("battle-text", 0x2A),
    Op::ChestName,
    Op::Script("battle-text", 0x2B),
    AFTER_REWARD,
    Op::IfZiDataHeld {
        then: ZI_DATA_HELD,
        otherwise: &[Op::TakeChest],
    },
];
/// Money: the amount and Ｇ手に入れた.
const MONEY_REWARD: &[Op] = &[
    BEFORE_REWARD[0],
    BEFORE_REWARD[1],
    Op::TakeChest,
    Op::Script("battle-text", 10),
    AFTER_REWARD,
];

/// Searching a chest (entity command 21 at `0x0800B938` and the reward
/// routine at `0x080376A8`): it opens, half a second later it counts as
/// opened, and dialogue `0x1F` opens the message box, which announces the
/// one reward the chest gives (a Zoid core, a Zoid's Zi data, a part, a
/// consumable or money) before dialogue `0x22` closes it.
pub const CHEST: &[Op] = &[
    Op::OpenChest,
    Op::Wait(30),
    Op::MarkChest,
    Op::Dialogue(0x1F),
    Op::IfChest {
        kind: ChestKind::Core,
        then: &CORE_REWARD,
        otherwise: &[],
    },
    Op::IfChest {
        kind: ChestKind::ZiData,
        then: ZI_DATA_REWARD,
        otherwise: &[],
    },
    Op::IfChest {
        kind: ChestKind::Part,
        then: &PART_REWARD,
        otherwise: &[],
    },
    Op::IfChest {
        kind: ChestKind::Consumable,
        then: &CONSUMABLE_REWARD,
        otherwise: &[],
    },
    Op::IfChest {
        kind: ChestKind::Money,
        then: MONEY_REWARD,
        otherwise: &[],
    },
    Op::Dialogue(0x22),
];

/// The slot the trip through a portal runs in (`0x0800990C` spawns it
/// into slot 10).
pub const PORTAL_TASK: usize = 10;
/// The portal's runs: standing, taking whoever stands on it, and opening
/// to bring someone; the steps on whose last frame the one taken is gone
/// and the second sound plays, and on whose last the one brought is out.
const PORTAL_STANDS: usize = 0;
const PORTAL_TAKES_ONE: usize = 1;
const PORTAL_BRINGS_ONE: usize = 2;
const PORTAL_TAKEN_STEP: usize = 8;
const PORTAL_TAKEN_SOUND_STEP: usize = 41;
const PORTAL_BROUGHT_STEP: usize = 32;
const PORTAL_TAKEN_SOUND: u16 = 0x44;
/// A second before the next map loads, and one after it has brightened.
const PORTAL_PAUSE: u32 = 60;

/// A trip through a space-time portal (`0x0800960C`, which pushing toward
/// a portal's exit starts, the player's controls taken): the Gustav drives
/// onto the portal, which takes it with its sounds; a second later the
/// exit's sound plays, the field darkens and the exit's map loads, the
/// Gustav keeping its facing; if that map has a portal, a second after
/// it brightens the portal opens and on the last frame of its 32nd step
/// the Gustav comes out and drives a cell down; otherwise it stands at the
/// exit's arrival. The player then has the controls back.
pub const PORTAL_TRIP: &[Op] = &[
    Op::IfPortal {
        then: &[
            through(PLAYER, AT_THE_PORTAL, PIXEL, 1),
            Op::PlayOnce(THE_PORTAL, PORTAL_TAKES_ONE),
            Op::Sound(PORTAL_SOUND),
            Op::AwaitStepEnd(THE_PORTAL, PORTAL_TAKEN_STEP),
            Op::Place(PLAYER, OFF_THE_MAP),
            Op::AwaitStepEnd(THE_PORTAL, PORTAL_TAKEN_SOUND_STEP),
            Op::Sound(PORTAL_TAKEN_SOUND),
            Op::AwaitAnimation(THE_PORTAL),
            Op::Animate(THE_PORTAL, PORTAL_STANDS),
        ],
        otherwise: &[],
    },
    Op::Wait(PORTAL_PAUSE),
    Op::ExitSound,
    Op::FadeOutHolding,
    Op::TakeExit,
    Op::Control(false),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::IfPortal {
        then: &[],
        otherwise: &[Op::Place(PLAYER, EXIT_ARRIVAL)],
    },
    Op::FadeInHolding,
    Op::Wait(PORTAL_PAUSE),
    Op::IfPortal {
        then: &[
            Op::PlayOnce(THE_PORTAL, PORTAL_BRINGS_ONE),
            Op::Sound(PORTAL_SOUND),
            Op::AwaitStepEnd(THE_PORTAL, PORTAL_BROUGHT_STEP),
            Op::Show(PLAYER),
            Op::Place(PLAYER, AT_THE_PORTAL),
            stride(PLAYER, BELOW_THE_PORTAL),
            Op::Sound(PORTAL_ARRIVAL_SOUND),
            Op::AwaitAnimation(THE_PORTAL),
            Op::Animate(THE_PORTAL, PORTAL_STANDS),
        ],
        otherwise: &[],
    },
    Op::Control(true),
    Op::End,
];

/// What map `map` runs when it loads, when it runs anything.
#[must_use]
pub fn map_handler(map: usize) -> Option<&'static [Op]> {
    match map {
        1 => Some(WORLD_MAP),
        2 => Some(GROUND_FLOOR),
        4 => Some(FIRST_ROOM),
        10 => Some(LONG_TUNNEL),
        11 => Some(TUNNEL_EXIT),
        CASTLE_GROUNDS => Some(CASTLE_GROUNDS_ARRIVAL),
        FACTORY_DOOR_ROOM => Some(FACTORY_DOOR_ARRIVAL),
        FACTORY_HALL | FACTORY_HALL_AFTER => Some(FACTORY_HALL_ARRIVAL),
        FACTORY_CORRIDOR => Some(FACTORY_CORRIDOR_ARRIVAL),
        DEVICE_ROOM => Some(DEVICE_ROOM_ARRIVAL),
        ARCADIA_THRONE_ROOM => Some(THRONE_ROOM_ARRIVAL),
        ARCANA => Some(ARCANA_STREETS),
        SAND_COLONY_FIELD => Some(chapter2::SAND_COLONY),
        30 => Some(chapter2::DESERT_ARRIVAL),
        32 => Some(chapter2::BAR_ARRIVAL),
        34 => Some(chapter2::HOUSE_ARRIVAL),
        37 => Some(chapter2::HIDEOUT_ARRIVAL),
        38 => Some(chapter2::CANYON_ARRIVAL),
        _ => chapter3::map_handler(map)
            .or_else(|| chapter4::map_handler(map))
            .or_else(|| chapter5::map_handler(map))
            .or_else(|| chapter6::map_handler(map))
            .or_else(|| chapter7::map_handler(map))
            .or_else(|| chapter8::map_handler(map))
            .or_else(|| chapter9::map_handler(map))
            .or_else(|| chapter10::map_handler(map)),
    }
}

/// A keeper's shop (`0x08008F58`), called a frame after the talk: the
/// field darkens to black from the next frame (`0x08001524`), the shop
/// opens, and once it closes the map is loaded again behind it with the
/// objects where they stood, and shown at once.
const fn shop(shop: Shop) -> [Op; 3] {
    [
        Op::FadeOutHoldingAfter(SHOP_FADE_DELAY),
        Op::Shop(shop),
        Op::Brightness(0),
    ]
}

/// Frames from a keeper's talk until the field starts darkening.
const SHOP_FADE_DELAY: u8 = 2;

/// Meeting a roaming enemy (`0x0800B9CC`, entity state 4, a frame after
/// the step that met it): sound `0x52`, the field darkens to black from
/// the next frame (`0x08001524`), the battle runs, the field returns from
/// black a level a frame (`0x080014A8`), and then the outcome takes hold.
pub const ENCOUNTER: &[Op] = &[
    Op::Freeze(1),
    Op::Sound(ENCOUNTER_SOUND),
    Op::FadeOutHoldingAfter(ENCOUNTER_FADE_DELAY),
    Op::Combat,
    Op::Freeze(ENCOUNTER_RELOAD_FRAMES),
    Op::FadeInHolding,
    Op::AfterCombat,
];

const ENCOUNTER_SOUND: u16 = 0x52;
/// Frames from the sound until the field starts darkening: the first
/// frame of `0x08001524` sets level 0.
const ENCOUNTER_FADE_DELAY: u8 = 1;
/// Black frames between the battle's end and the field's first brighter
/// level: the map's reload.
const ENCOUNTER_RELOAD_FRAMES: u32 = 14;

/// Arcana's item shop (`0x080090F0`).
const ARCANA_ITEM_SHOP: &[Op] = &shop(Shop::Items(1));

/// Arcana's armaments shop (`0x080090FC`).
const ARCANA_ARMS_SHOP: &[Op] = &shop(Shop::Arms(1));

/// Dr. T's Zoid lab in Arcana (`0x08009108`): the area's objects are
/// rebuilt before it opens (`0x08006E4C`) and every unit is repaired once
/// it closes (`0x08037148`).
const ARCANA_LAB: &[Op] = &shop(Shop::Lab(1));

/// What an object whose script is the code at `address` runs when spoken
/// to, for the code this port has transcribed.
#[must_use]
pub fn talk_handler(address: u32) -> Option<&'static [Op]> {
    match address {
        0x0800_C73C => Some(FIRST_WARRIOR),
        0x0800_C77C => Some(SECOND_WARRIOR),
        0x0800_6628 => Some(SOLDIER_BY_THE_STAIRS),
        0x0800_6658 => Some(SOLDIER_AT_THE_DOOR),
        0x0800_668C => Some(SOLDIER_IN_THE_HALL),
        0x0800_CB84 => Some(KING_TALK),
        0x0800_CBEC => Some(REGINA_OFFER),
        0x0800_CC24 => Some(ACE_OFFER),
        0x0800_CC5C => Some(JACK_OFFER),
        0x0802_AB08 => Some(DR_T),
        0x0800_9480 => Some(LAB_ASSISTANT),
        0x0800_9588 => Some(OLD_MAN_IN_THE_SHOP),
        0x0800_95B8 => Some(ROMAN_TEACHES),
        0x0800_90F0 => Some(ARCANA_ITEM_SHOP),
        0x0800_90FC => Some(ARCANA_ARMS_SHOP),
        0x0800_9108 => Some(ARCANA_LAB),
        _ => chapter2::talk_handler(address)
            .or_else(|| chapter3::talk_handler(address))
            .or_else(|| chapter4::talk_handler(address))
            .or_else(|| chapter5::talk_handler(address))
            .or_else(|| chapter6::talk_handler(address))
            .or_else(|| chapter7::talk_handler(address))
            .or_else(|| chapter8::talk_handler(address))
            .or_else(|| chapter9::talk_handler(address))
            .or_else(|| chapter10::talk_handler(address)),
    }
}
