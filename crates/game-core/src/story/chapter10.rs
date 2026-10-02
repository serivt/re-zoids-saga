//! Chapter 10: back home after the Emperor's fall, the prince, now king,
//! sets out again with Regina, Earth and Jack into a newly found
//! space-time, with companions called from the castle; there Vega, thrown
//! out of his own time with his Führer, challenges the party (story battle
//! 41) and leaves Zi data behind (area 10).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the map
//! handlers of area 10 (`0x08029E50`, `0x0802A848`), the tasks they spawn,
//! the objects' code and the routines named below. See `docs/events.md`.

use super::{
    CORE_REWARD, FADE_IN, HELPER_TASK, PLAYER, ZI_DATA_REWARD, learn, shop, story_battle, through,
    walk,
};
use crate::event::{HERE, MAP_TASK, Op};
use crate::field::{Direction, PIXEL};
use crate::menu::Shop;
use extraction::saga::Reward;

/// Set once the chapter's opening has played at the castle (`0x08029E50`).
pub(super) const CHAPTER_OPENED: u16 = 0x137;
/// Set when the prince tells Regina he is ready (`0x0802A698`); cleared
/// each time the castle's room loads.
const SET_OUT: u16 = 0x138;
/// Set while Regina, Earth and Jack wait in the castle's room for the
/// prince; cleared as the party sets out.
const GATHERED: u16 = 0x139;
/// Set once each researcher of the lab (map 322) has handed over his Zi
/// data (`0x08006D30`, `0x08006D94`).
pub(super) const FIRST_DATA_GIVEN: u16 = 0x13A;
pub(super) const SECOND_DATA_GIVEN: u16 = 0x13B;
/// Set once Vega is beaten (story battle 41), once he has challenged the
/// party (spoken to, `0x0802AA14`), and once his Zoid has been placed on
/// map 317 the first time.
pub(super) const VEGA_BEATEN: u16 = 0x13C;
const VEGA_CHALLENGED: u16 = 0x13D;
const VEGA_PLACED: u16 = 0x13E;

/// The castle's room where the chapter opens and the party gathers, and
/// the place where Vega's Führer roams.
const CASTLE_ROOM: usize = 340;
const VEGA_GROUND: usize = 317;

const REGINA: usize = 1;
const EARTH: usize = 2;
const JACK: usize = 3;
/// Vega's Führer on map 317.
const VEGA: usize = 1;

const OFF_THE_MAP: (usize, usize) = (0xFF, 0xFF);
/// A walk a pixel a frame through everything (entity command 11), the
/// scenes' pace.
const PACE: i32 = PIXEL;
/// A scene's pause after a line.
const LINE_PAUSE: u32 = 30;

/// The prince's own sprite, which the opening gives him back after the
/// king's (`0x080089A0`).
const PRINCE_STANDING: usize = 0x98;

const PORTAL_SOUND: u16 = 0x6F;
const JUMP_SOUND: u16 = 0x6D;

/// Walks actor `actor` to `cell` through everything at a pixel a frame
/// (entity command 11).
const fn steps_to(actor: usize, cell: (usize, usize)) -> Op {
    through(actor, cell, PACE, 1)
}

/// The same, waiting for it.
const fn steps(actor: usize, cell: (usize, usize)) -> [Op; 2] {
    [steps_to(actor, cell), Op::AwaitArrival(actor)]
}

/// The castle's room the first time (ROM `0x0832B3F8`): the prince in the
/// king's clothes, Regina, Earth and Jack around the table.
const OPENING_OBJECTS: u32 = 0x0832_B3F8;
/// The room once the party has set out (ROM `0x0832B448`): the three off
/// the map, in the party.
const AWAY_OBJECTS: u32 = 0x0832_B448;
/// The room while the three wait (ROM `0x0832B498`): Regina by the door,
/// Earth and Jack by the table.
const GATHERED_OBJECTS: u32 = 0x0832_B498;
/// Where the opening stands the prince.
const OPENING_CELL: (usize, usize) = (2, 5);
/// Where the prince stands when he comes back with the party, and the
/// room's way out.
const RETURN_CELL: (usize, usize) = (6, 2);
const ROOM_EXIT: (usize, usize) = (0xD, 2);

/// The first time (in the castle's task): Regina scolds the king for
/// leaving on a journey again, the others side with him (`0x2CD`), and
/// they tell him how to set out (`0x2CE`); the three take their places
/// and the prince changes into his own clothes.
const OPENING: &[Op] = &[
    Op::Flag(CHAPTER_OPENED, true),
    Op::Wait(60),
    Op::Dialogue(0x2CD),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x2CE),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(REGINA, (3, 2))),
    steps_to(REGINA, (0xC, 2)),
    steps_to(EARTH, (7, 5)),
    steps_to(JACK, (7, 1)),
    Op::AwaitArrival(EARTH),
    Op::Face(EARTH, Direction::Up),
    Op::AwaitArrival(JACK),
    Op::Face(JACK, Direction::Down),
    Op::AwaitArrival(REGINA),
    Op::Face(REGINA, Direction::Left),
    Op::Sprite(PLAYER, PRINCE_STANDING),
    Op::Call(&steps(PLAYER, (2, 3))),
    Op::Call(&steps(PLAYER, (3, 3))),
];

/// Back in the room with the party (in the castle's task): the prince
/// walks in, the three step out of the party to their places, and they
/// tell him again how to set out (`0x2CE`).
const RETURN: &[Op] = &[
    Op::Call(&steps(PLAYER, RETURN_CELL)),
    Op::Face(PLAYER, Direction::Right),
    Op::Place(REGINA, RETURN_CELL),
    Op::Place(EARTH, RETURN_CELL),
    Op::Place(JACK, RETURN_CELL),
    steps_to(REGINA, (8, 2)),
    steps_to(EARTH, (7, 3)),
    steps_to(JACK, (7, 1)),
    Op::AwaitArrival(REGINA),
    Op::Face(REGINA, Direction::Left),
    Op::AwaitArrival(EARTH),
    Op::Face(EARTH, Direction::Up),
    Op::AwaitArrival(JACK),
    Op::Face(JACK, Direction::Down),
    Op::Wait(60),
    Op::Dialogue(0x2CE),
    Op::Wait(LINE_PAUSE),
    steps_to(REGINA, (0xC, 2)),
    steps_to(EARTH, (7, 5)),
    Op::AwaitArrival(EARTH),
    Op::Face(EARTH, Direction::Up),
    Op::AwaitArrival(REGINA),
    Op::Face(REGINA, Direction::Left),
    Op::Call(&steps(PLAYER, (6, 3))),
];

/// The castle's room (task at `0x08029F40`, which the end of chapter 9
/// calls too): the opening, or the party's return, unless the three wait
/// already; then the prince walks about freely until he tells Regina he
/// is ready, and the party leaves the room.
pub(super) const CASTLE_TASK: &[Op] = &[
    Op::IfFlags {
        all: &[],
        none: &[GATHERED],
        then: &[
            Op::IfFlags {
                all: &[],
                none: &[CHAPTER_OPENED],
                then: OPENING,
                otherwise: RETURN,
            },
            Op::Flag(GATHERED, true),
        ],
        otherwise: &[],
    },
    Op::Control(true),
    Op::AwaitAnyFlag(&[SET_OUT]),
    Op::Control(false),
    Op::Flag(GATHERED, false),
    steps_to(JACK, (7, 2)),
    steps_to(EARTH, (7, 2)),
    Op::AwaitArrival(JACK),
    steps_to(JACK, (0xB, 2)),
    Op::AwaitArrival(EARTH),
    steps_to(EARTH, (0xB, 2)),
    Op::AwaitArrival(JACK),
    Op::Hide(JACK),
    Op::AwaitArrival(EARTH),
    Op::Hide(EARTH),
    Op::Call(&steps(REGINA, (0xB, 2))),
    Op::Hide(REGINA),
    walk(PLAYER, ROOM_EXIT, PACE, 1),
    Op::End,
];

/// The castle's room (`0x08029E50`): the first time it is laid out for
/// the opening with the prince by the table; while the three wait, with
/// them in their places; once the party has set out, without them. Then
/// the castle's task runs.
const CASTLE_ROOM_ARRIVAL: &[Op] = &[
    Op::Flag(SET_OUT, false),
    Op::IfFlags {
        all: &[],
        none: &[CHAPTER_OPENED],
        then: &[Op::IfFlags {
            all: &[],
            none: &[GATHERED],
            then: &[
                Op::LoadScene {
                    map: CASTLE_ROOM,
                    player: HERE,
                    objects: OPENING_OBJECTS,
                    count: 4,
                },
                Op::Place(PLAYER, OPENING_CELL),
            ],
            otherwise: &[Op::LoadScene {
                map: CASTLE_ROOM,
                player: HERE,
                objects: GATHERED_OBJECTS,
                count: 4,
            }],
        }],
        otherwise: &[Op::IfFlags {
            all: &[GATHERED],
            none: &[],
            then: &[Op::LoadScene {
                map: CASTLE_ROOM,
                player: HERE,
                objects: GATHERED_OBJECTS,
                count: 4,
            }],
            otherwise: &[Op::LoadScene {
                map: CASTLE_ROOM,
                player: HERE,
                objects: AWAY_OBJECTS,
                count: 4,
            }],
        }],
    },
    Op::Spawn(MAP_TASK, CASTLE_TASK),
];

/// Regina (`0x0802A698`): the text system is reset (`0x0803E0B8`, a
/// frame), and she asks whether the prince is ready (`0x2CF`); yes sets
/// the party out.
const REGINA_ASKS: &[Op] = &[
    Op::Wait(1),
    Op::Dialogue(0x2CF),
    Op::IfChoice {
        then: &[Op::Flag(SET_OUT, true)],
        otherwise: &[],
    },
];

/// Earth (`0x0802A6D0`) fills companion slot 0 and Jack (`0x0802A78C`)
/// slot 1, as in chapter 9 but with their own lines (`0x287`, `0x28B`,
/// `0x289`; `0x281`, `0x285`, `0x283`); a pick joins without a Zoid of
/// its own (`0x0802A67C`: `0x080372D0` with the character, 6 and 0).
const EARTH_CALLS: &[Op] = super::caller!(0, 1, 0x287, 0x28B, 0x289, false);
const JACK_CALLS: &[Op] = super::caller!(1, 0, 0x281, 0x285, 0x283, false);

/// A beaten party's way home: the return point, facing up, and the field
/// brightening while the game holds; the scene's task ends.
const BEATEN: &[Op] = &[Op::WarpHome, Op::FadeInHolding, Op::End];

/// A Zi data given (`0x08037A24`), announced as a chest's: the message box
/// opens (dialogue `0x1F`), Ｚｉデータ「…」を手に入れた, and it closes (`0x22`).
const fn zi_gift(id: u8) -> [Op; 5] {
    [
        Op::Gift(Some(Reward::ZiData(id))),
        Op::Dialogue(0x1F),
        Op::Call(ZI_DATA_REWARD),
        Op::Dialogue(0x22),
        Op::Gift(None),
    ]
}

/// A Zoid core given (`0x080379B4`), announced the same way:
/// Ｚｉデータ用アイテム「…」を手に入れた.
const fn core_gift(id: u8) -> [Op; 5] {
    [
        Op::Gift(Some(Reward::Core(id))),
        Op::Dialogue(0x1F),
        Op::Call(&CORE_REWARD),
        Op::Dialogue(0x22),
        Op::Gift(None),
    ]
}

/// Vega's Führer roaming map 317 (task at `0x0802A8C8`): once he has
/// challenged the party, story battle 41. Won, the Führer stops (`0x2D1`),
/// a space-time rift takes it away, and the party finds what he left
/// (`0x2D2`): the Zi data of `0x78` and Zoid core 8.
const VEGA_TASK: &[Op] = &[
    Op::Control(true),
    Op::AwaitAnyFlag(&[VEGA_CHALLENGED]),
    Op::Control(false),
    Op::Call(&story_battle(41)),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[],
    },
    Op::Flag(VEGA_BEATEN, true),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Once(VEGA),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x2D1),
    Op::Wait(LINE_PAUSE),
    Op::Sound(PORTAL_SOUND),
    Op::Wait(60),
    Op::Sound(JUMP_SOUND),
    Op::Place(VEGA, OFF_THE_MAP),
    Op::KeepCell(VEGA),
    Op::Roam(VEGA, false),
    Op::Wait(60),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x2D2),
    Op::Wait(LINE_PAUSE),
    Op::Call(&zi_gift(VEGA_ZI_DATA)),
    Op::Call(&core_gift(VEGA_CORE)),
    Op::Control(true),
    Op::End,
];

/// What Vega leaves behind.
pub(super) const VEGA_ZI_DATA: u8 = 0x78;
const VEGA_CORE: u8 = 8;
/// Where his Führer appears the first time.
const VEGA_CELL: (usize, usize) = (0xD, 4);

/// Map 317 (`0x0802A848`): until Vega is beaten his Führer roams it, placed
/// on its cell the first time; his challenge waits.
const VEGA_GROUND_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[VEGA_BEATEN],
    then: &[
        Op::IfFlags {
            all: &[],
            none: &[VEGA_PLACED],
            then: &[
                Op::Place(VEGA, VEGA_CELL),
                Op::Roam(VEGA, true),
                Op::Flag(VEGA_PLACED, true),
            ],
            otherwise: &[Op::Roam(VEGA, true)],
        },
        Op::Flag(VEGA_CHALLENGED, false),
        Op::Spawn(MAP_TASK, VEGA_TASK),
    ],
    otherwise: &[],
}];

/// Vega's Führer spoken to (`0x0802AA14`): the text system is reset
/// (`0x0803E0B8`, a frame), Vega wants to play (`0x2D0`), and the Führer
/// stops.
const VEGA_CHALLENGES: &[Op] = &[
    Op::Wait(1),
    Op::Dialogue(0x2D0),
    Op::Flag(VEGA_CHALLENGED, true),
    Op::Roam(VEGA, false),
];

/// The Zi data the two researchers of map 322 hand over: the Imperial
/// army's own Zoids (`0x08006D30`) and those it had gathered
/// (`0x08006D94`).
pub(super) const FIRST_DATA: [u8; 8] = [0x95, 0x7C, 0x80, 0x7E, 0x7F, 0x7D, 0x81, 0x92];
pub(super) const SECOND_DATA: [u8; 8] = [0x3D, 0x3F, 0x17, 0x12, 6, 0x11, 0x20, 0x8C];

/// A researcher: the first time his line (`0x2D3` or `0x2D4`) and his eight
/// Zi data, later `0x2D5`.
macro_rules! researcher {
    ($flag:expr, $line:expr, $data:expr) => {
        &[Op::IfFlags {
            all: &[],
            none: &[$flag],
            then: &[
                Op::Flag($flag, true),
                Op::Dialogue($line),
                Op::Call(&zi_gift($data[0])),
                Op::Call(&zi_gift($data[1])),
                Op::Call(&zi_gift($data[2])),
                Op::Call(&zi_gift($data[3])),
                Op::Call(&zi_gift($data[4])),
                Op::Call(&zi_gift($data[5])),
                Op::Call(&zi_gift($data[6])),
                Op::Call(&zi_gift($data[7])),
            ],
            otherwise: &[Op::Dialogue(0x2D5)],
        }]
    };
}

const FIRST_RESEARCHER: &[Op] = researcher!(FIRST_DATA_GIVEN, 0x2D3, FIRST_DATA);
const SECOND_RESEARCHER: &[Op] = researcher!(SECOND_DATA_GIVEN, 0x2D4, SECOND_DATA);

/// The teacher of area 10: deck command `0x10` (`0x08009540` through
/// `0x08009430`).
const TEACHERS: [(u32, &[Op]); 1] = [(
    0x0800_9540,
    &[Op::IfCommand {
        command: 0x10,
        then: &[Op::Dialogue(0x3CE)],
        otherwise: &[Op::Dialogue(0x3CF), Op::Call(&learn(0x10))],
    }],
)];

/// The keeper of map 337 (`0x080093FC`), whose shop the roaming battles
/// won pick, called as [`shop`] calls the others.
const ROTATING_ARMS_SHOP: &[Op] = &{
    let [fade, _, brightness] = shop(Shop::Arms(0));
    [fade, Op::RotatingArmsShop, brightness]
};

/// The keepers of area 10: item shop 19 (`0x080093F0`), the armaments
/// shop the roaming battles won pick (`0x080093FC`) and lab 18
/// (`0x08009424`).
const SHOPS: [(u32, &[Op]); 3] = [
    (0x0800_93F0, &shop(Shop::Items(0x13))),
    (0x0800_93FC, ROTATING_ARMS_SHOP),
    (0x0800_9424, &shop(Shop::Lab(0x12))),
];

/// The objects of area 10 whose script is code: the castle room's
/// Regina, Earth and Jack, Vega's Führer and the lab's researchers.
const OBJECTS: [(u32, &[Op]); 6] = [
    (0x0802_A698, REGINA_ASKS),
    (0x0802_A6D0, EARTH_CALLS),
    (0x0802_A78C, JACK_CALLS),
    (0x0802_AA14, VEGA_CHALLENGES),
    (0x0800_6D30, FIRST_RESEARCHER),
    (0x0800_6D94, SECOND_RESEARCHER),
];

/// What speaking to an object of area 10 whose script is code runs.
pub(super) fn talk_handler(address: u32) -> Option<&'static [Op]> {
    TEACHERS
        .iter()
        .chain(SHOPS.iter())
        .chain(OBJECTS.iter())
        .find(|(at, _)| *at == address)
        .map(|(_, program)| *program)
}

/// What map `map` of area 10 runs when it loads, when it runs anything.
pub(super) fn map_handler(map: usize) -> Option<&'static [Op]> {
    match map {
        CASTLE_ROOM => Some(CASTLE_ROOM_ARRIVAL),
        VEGA_GROUND => Some(VEGA_GROUND_ARRIVAL),
        _ => None,
    }
}
