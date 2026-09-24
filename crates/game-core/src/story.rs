//! Zoids Saga's own events, transcribed from its code into [`Op`]
//! programs: what each map runs when it loads and what objects whose
//! script is code run when spoken to.
//!
//! Source of knowledge: own reading of the Thumb routines named on each
//! program in Zoids Saga (Japan, Rev 1), checked against per-frame traces
//! of the entity table and the brightness register in a reference emulator
//! (see `docs/events.md`). Dialogue indices are strings of the `dialogue`
//! table; actors are object indices (the game's entity index minus one).

use crate::event::{MAP_TASK, Op};
use crate::field::{Direction, PIXEL};
use crate::menu::Shop;

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
        column: Some(23),
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
        column: Some(2),
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
/// Frames between the end of a map's fade in and the port's first run of
/// the tasks its handler spawned.
const ARRIVAL_TASK_LAG: u32 = 2;
/// The door of アーカナの町 on the world map.
const ARCANA_GATE: (usize, usize) = (14, 7);

/// Out of the labyrinth (task at `0x080103B4`): Regina proposes the
/// nearby town of Arcana and the Gustav drives off toward it. The last walk
/// ends against the town's door, which the Gustav takes; the task ends a
/// second after starting it, while the Gustav still drives. The original
/// counts the task's first second from the frame the fade in ends, two
/// frames before the port lets the tasks run.
const TO_ARCANA: &[Op] = &[
    Op::Wait(60 - ARRIVAL_TASK_LAG),
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

/// Dr. T in his lab (`0x0802AB08`). In area 1 he first talks with Regina
/// about the Trinity Liger, then repeats his advice; areas 9 and 10 have a
/// line each.
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
            otherwise: &[],
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

/// Searching a chest (entity command 21 at `0x0800B938` and the reward
/// routine at `0x080376A8`): it opens, half a second later it counts as
/// opened, and money is announced in the message box.
pub const CHEST: &[Op] = &[
    Op::OpenChest,
    Op::Wait(30),
    Op::MarkChest,
    Op::IfChestMoney {
        then: &[
            Op::Dialogue(0x1F),
            Op::Script("battle-menu", 6),
            Op::Script("battle-menu", 7),
            Op::TakeChestMoney,
            Op::Script("battle-text", 10),
            Op::Script("battle-menu", 5),
            Op::Dialogue(0x22),
        ],
        otherwise: &[],
    },
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
        ARCANA => Some(ARCANA_STREETS),
        _ => None,
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
/// the next frame (`0x08001524`), the battle runs, and the field returns
/// from black a level a frame (`0x080014A8`).
pub const ENCOUNTER: &[Op] = &[
    Op::Freeze(1),
    Op::Sound(ENCOUNTER_SOUND),
    Op::FadeOutHoldingAfter(ENCOUNTER_FADE_DELAY),
    Op::Combat,
    Op::Freeze(ENCOUNTER_RELOAD_FRAMES),
    Op::FadeInHolding,
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
        _ => None,
    }
}
