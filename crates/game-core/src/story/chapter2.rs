//! Chapter 2: Sand Colony and the desert around it (area 2).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! map handlers of area 2 and the tasks they spawn, named at each item;
//! checked against a reference emulator. See `docs/events.md`.

use super::{
    BESIDE_THE_MAP, FIELD_HOOK, PLAYER, SAND_COLONY_FIELD, SCENE_BRIGHTEN, SCENE_DARKEN,
    THRONE_ROOM_SEEN, glide, learn, shop, story_battle, stride,
};
use crate::event::{HERE, MAP_TASK, Op};
use crate::field::{Direction, PIXEL};
use crate::menu::Shop;

/// Set once the party has landed in Sand Colony and met Van (`0x08012350`).
pub const SAND_COLONY_ARRIVED: u16 = 0x144;
/// Set once Irvine and Moonbay have joined the party in the bar
/// (`0x08012444`); the party's talk follows on the next entry to the town.
const MERCENARIES_JOINED: u16 = 0x145;
/// Set once that scene has run.
const MERCENARIES_TALKED: u16 = 0x146;
/// The arrival's objects (ROM `0x08666F50`): the prince, Regina, Jack,
/// Earth, Van and Zeke, the five off the map.
const ARRIVAL_OBJECTS: u32 = 0x0866_6F50;
const ARRIVAL_OBJECT_COUNT: usize = 6;
/// Where the arrival stands the player, and where its end warps back to.
const ARRIVAL_CELL: (usize, usize) = (0x16, 0x1D);
const ARRIVAL_END_CELL: (usize, usize) = (0x16, 0x1A);
/// Where the party walks in from and Van and Zeke run in from.
const TOWN_GATE: (usize, usize) = (0x16, 0x1A);
const VAN_ENTRANCE: (usize, usize) = (0x16, 0x14);
const REGINA: usize = 1;
const JACK: usize = 2;
const EARTH: usize = 3;
const VAN: usize = 4;
const ZEKE: usize = 5;
/// Van's chase music and the crash's sound.
const CHASE_MUSIC: u16 = 8;
const CRASH_SOUND: u16 = 0x48;
/// The town's own song (its record's `+0x1A`), which comes back once Van
/// has gone.
const SAND_COLONY_MUSIC: u16 = 7;
/// Map pixels a room's cell spans.
const ROOM_CELL: u32 = 16;
/// Frames the task waits between beats.
const BEAT: u32 = 60;

/// A glide in a room (`0x08011F18` with cells of 16): the routine moves
/// its target 8 pixels left, where a room's sprite box starts.
const fn walk_to(actor: usize, (x, y): (i32, i32), speed: i32) -> Op {
    Op::Glide {
        actor,
        to: (x - 8, y),
        speed,
        frames: ROOM_CELL / speed.unsigned_abs(),
        camera: true,
    }
}

/// Van runs off (a task of its own, `0x08013184`) while Zeke does.
const VAN_RUNS_OFF: &[Op] = &[
    walk_to(VAN, (0x160, 0x1F0), 2),
    Op::Place(VAN, BESIDE_THE_MAP),
    Op::End,
];

/// The party walks into Sand Colony (task at `0x08012EF8`; its turns,
/// `0x08000BD8`, name the entity, one past the object): the prince walks
/// in, the others step in one after another and Regina and Jack wonder
/// about Earth and Fran (dialogue `0x66`); Van runs into the prince to
/// song 8 (`0x67`), and he and Zeke run off after her; the town's song
/// comes back, the party talks it over (`0x68`), the field darkens and the
/// town loads again with the party gone and the player in control.
const ARRIVAL_TASK: &[Op] = &[
    Op::Wait(32),
    stride(PLAYER, ARRIVAL_END_CELL),
    Op::AwaitArrival(PLAYER),
    Op::Place(REGINA, TOWN_GATE),
    walk_to(REGINA, (0x150, 0x170), 1),
    Op::Wait(BEAT),
    Op::Place(JACK, TOWN_GATE),
    walk_to(JACK, (0x150, 0x1A0), 1),
    Op::Face(JACK, Direction::Right),
    Op::Place(EARTH, TOWN_GATE),
    walk_to(EARTH, (0x160, 0x1B0), 1),
    Op::Face(EARTH, Direction::Up),
    Op::Wait(BEAT),
    Op::Face(PLAYER, Direction::Down),
    Op::Wait(BEAT),
    Op::Dialogue(0x66),
    Op::Wait(BEAT),
    Op::Face(PLAYER, Direction::Up),
    Op::Wait(BEAT),
    Op::Place(VAN, VAN_ENTRANCE),
    walk_to(VAN, (0x160, 0x190), 2),
    Op::RestartMusic(CHASE_MUSIC),
    Op::Sound(CRASH_SOUND),
    Op::AwaitSoundEnd(CRASH_SOUND),
    Op::Face(REGINA, Direction::Down),
    Op::Wait(30),
    Op::Place(ZEKE, VAN_ENTRANCE),
    walk_to(ZEKE, (0x160, 0x180), 1),
    Op::Dialogue(0x67),
    walk_to(PLAYER, (0x170, 0x1A0), 1),
    Op::Face(PLAYER, Direction::Left),
    walk_to(EARTH, (0x170, 0x1B0), 1),
    Op::Face(EARTH, Direction::Left),
    Op::Wait(BEAT),
    Op::Spawn(MAP_TASK + 1, VAN_RUNS_OFF),
    walk_to(ZEKE, (0x160, 0x1F0), 2),
    Op::Place(ZEKE, BESIDE_THE_MAP),
    Op::Face(PLAYER, Direction::Down),
    Op::Face(JACK, Direction::Down),
    Op::Face(EARTH, Direction::Down),
    Op::Wait(BEAT),
    walk_to(REGINA, (0x150, 0x180), 1),
    Op::RestartMusic(SAND_COLONY_MUSIC),
    Op::Face(PLAYER, Direction::Up),
    Op::Face(JACK, Direction::Up),
    Op::Face(EARTH, Direction::Up),
    Op::Wait(BEAT),
    Op::Dialogue(0x68),
    Op::Call(SCENE_DARKEN),
    Op::Warp {
        map: SAND_COLONY_FIELD,
        cell: ARRIVAL_END_CELL,
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// The scene once the mercenaries have joined (task at `0x080131B0`):
/// half a second in, the party plans its search (dialogue `0x77`) and the
/// player has the controls back.
const MERCENARIES_TASK: &[Op] = &[Op::Wait(32), Op::Dialogue(0x77), Op::Control(true), Op::End];

/// Sand Colony's field (`0x08012350`): after the flashback, the first
/// time, the town loads with the arrival's objects and the player stands
/// at its gate without the controls while the arrival runs; later, the
/// first time the mercenaries have joined, their scene runs.
pub(super) const SAND_COLONY: &[Op] = &[Op::IfFlags {
    all: &[THRONE_ROOM_SEEN],
    none: &[SAND_COLONY_ARRIVED],
    then: &[
        Op::LoadMap {
            map: SAND_COLONY_FIELD,
            player: ARRIVAL_CELL,
            objects: ARRIVAL_OBJECTS,
            count: ARRIVAL_OBJECT_COUNT,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, ARRIVAL_TASK),
        Op::Flag(SAND_COLONY_ARRIVED, true),
    ],
    otherwise: &[Op::IfFlags {
        all: &[MERCENARIES_JOINED],
        none: &[MERCENARIES_TALKED],
        then: &[
            Op::Control(false),
            Op::Spawn(MAP_TASK, MERCENARIES_TASK),
            Op::Flag(MERCENARIES_TALKED, true),
        ],
        otherwise: &[],
    }],
}];

/// The bar, `md0252` (map 32).
const BAR: usize = 32;
/// The bar until the mercenaries have joined (ROM `0x08666FC8`): its three
/// regulars, Moonbay at the back (whose talk starts the scene), Irvine and
/// a second stranger (dialogues `0x73`, `0x74`), and Regina, Jack and
/// Earth off the map.
const BAR_OBJECTS: u32 = 0x0866_6FC8;
const BAR_OBJECT_COUNT: usize = 10;
const MOONBAY: usize = 4;
const IRVINE: usize = 5;
const STRANGER: usize = 6;
const BAR_REGINA: usize = 7;
const BAR_JACK: usize = 8;
const BAR_EARTH: usize = 9;
/// Moonbay's sprite once she turns to the party (`0x080089A0`).
const MOONBAY_FACING_SPRITE: usize = 0x9E;
/// The bar's tune and the chase's, which the scene switches between.
const BAR_MUSIC: u16 = 5;
/// The list of characters who join in the bar (`0x080374B8`, ROM
/// `0x0867E380`).
const MERCENARIES: u8 = 1;
/// The bar's door, where the scene's party leaves.
const BAR_DOOR: (i32, i32) = (0x180, 0x70);
/// The cell the scene loads the bar again with.
const BAR_END_CELL: (usize, usize) = (0x18, 7);

/// The mercenaries join (task at `0x080133DC`; turns and sprites name the
/// entity, one past the object): the field darkens, the party stands
/// before Moonbay, who has turned; to the bar's tune she makes her offer
/// (dialogue `0x75`), then to song 8 Regina steps up and Irvine comes
/// over (`0x76`); everyone files out of the door, Irvine and Moonbay join
/// the party, and the bar loads again without them.
const MERCENARIES_JOIN_TASK: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Place(PLAYER, BAR_END_CELL),
    Op::Face(PLAYER, Direction::Up),
    Op::Place(BAR_REGINA, (0x19, 7)),
    Op::Face(BAR_REGINA, Direction::Up),
    Op::Place(BAR_JACK, (0x17, 7)),
    Op::Face(BAR_JACK, Direction::Up),
    Op::Place(BAR_EARTH, (0x16, 7)),
    Op::Face(BAR_EARTH, Direction::Up),
    Op::Sprite(MOONBAY, MOONBAY_FACING_SPRITE),
    Op::Place(MOONBAY, (0x18, 4)),
    Op::Face(MOONBAY, Direction::Down),
    Op::Face(IRVINE, Direction::Down),
    Op::Face(STRANGER, Direction::Down),
    Op::Call(SCENE_BRIGHTEN),
    Op::RestartMusic(BAR_MUSIC),
    Op::Wait(30),
    Op::Dialogue(0x75),
    Op::RestartMusic(CHASE_MUSIC),
    walk_to(BAR_REGINA, (0x190, 0x40), 1),
    Op::Face(BAR_REGINA, Direction::Left),
    Op::Face(MOONBAY, Direction::Right),
    Op::Wait(15),
    walk_to(IRVINE, (0x180, 0x30), 1),
    Op::Face(IRVINE, Direction::Down),
    Op::Wait(30),
    Op::Dialogue(0x76),
    walk_to(BAR_REGINA, (0x190, 0x70), 1),
    walk_to(BAR_REGINA, BAR_DOOR, 1),
    Op::Place(BAR_REGINA, OFF_THE_MAP),
    walk_to(BAR_JACK, BAR_DOOR, 1),
    Op::Place(BAR_JACK, OFF_THE_MAP),
    walk_to(BAR_EARTH, BAR_DOOR, 1),
    Op::Place(BAR_EARTH, OFF_THE_MAP),
    walk_to(MOONBAY, (0x190, 0x40), 1),
    walk_to(MOONBAY, (0x190, 0x70), 1),
    walk_to(MOONBAY, BAR_DOOR, 1),
    Op::Place(MOONBAY, OFF_THE_MAP),
    walk_to(IRVINE, (0x190, 0x30), 1),
    walk_to(IRVINE, (0x190, 0x70), 1),
    walk_to(IRVINE, BAR_DOOR, 1),
    Op::Place(IRVINE, OFF_THE_MAP),
    walk_to(STRANGER, (0x140, 0x70), 1),
    walk_to(STRANGER, BAR_DOOR, 1),
    Op::Place(STRANGER, OFF_THE_MAP),
    Op::Join(MERCENARIES),
    Op::Call(SCENE_DARKEN),
    Op::Warp {
        map: BAR,
        cell: BAR_END_CELL,
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// A cell off the map on both axes, where the bar's scene sends those who
/// leave.
const OFF_THE_MAP: (usize, usize) = (0xFF, 0xFF);

/// The bar (`0x08012408`): until the mercenaries have joined, it loads with
/// them and the party's other three, the player where it stands.
pub(super) const BAR_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[MERCENARIES_JOINED],
    then: &[Op::LoadMap {
        map: BAR,
        player: HERE,
        objects: BAR_OBJECTS,
        count: BAR_OBJECT_COUNT,
    }],
    otherwise: &[],
}];

/// Speaking to Moonbay (`0x08012444`): the player loses the controls and
/// her scene starts.
pub(super) const MOONBAY_TALK: &[Op] = &[
    Op::Control(false),
    Op::Spawn(MAP_TASK, MERCENARIES_JOIN_TASK),
    Op::Flag(MERCENARIES_JOINED, true),
];

/// Set once the party has talked over the bar's scene in the town; the
/// desert's lookout waits for the Gustav from then on.
const MERCENARIES_TALKED_ABOUT: u16 = MERCENARIES_TALKED;
/// Set once the thieves who ambushed Van are beaten (story battle 2).
const AMBUSH_BEATEN: u16 = 0x147;
/// Set once the party has decided to chase Van.
const CHASE_DECIDED: u16 = TOLD_TO_ASK_AROUND;
/// Set once the six people of map 34 have been heard and the desert's
/// scene has sent the party on.
const HEARD_EVERYONE: u16 = 0x150;

/// The desert past Sand Colony, `mq0200` (map 30), and its song.
const DESERT: usize = 30;
const DESERT_MUSIC: u16 = 17;
/// The ambush's objects (ROM `0x086672C0`): Van's Zoid and two of the
/// thieves'.
const AMBUSH_OBJECTS: u32 = 0x0866_72C0;
const VAN_ZOID: usize = 1;
/// Where the Gustav stands for the ambush and the scenes after it.
const AMBUSH_CELL: (usize, usize) = (0x1D, 4);
const AMBUSH_LOAD_CELL: (usize, usize) = (0x1A, 4);
/// The objects the desert's later scenes load (ROM `0x08666F28`,
/// `0x08666F3C`): the Gustav alone.
const GUSTAV_ALONE_FACING_LEFT: u32 = 0x0866_6F28;
const GUSTAV_ALONE_FACING_DOWN: u32 = 0x0866_6F3C;
/// The thieves' ambush (story battle 2).
const AMBUSH_BATTLE: u8 = 2;

/// The desert's lookout (the handler's hook, `0x080122F0`, run every
/// frame): once the Gustav stands at x `0x340` (column 26) between y
/// `0x60` and `0xA0` (rows 3 to 5), it stops.
const DESERT_LOOKOUT: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x340, 0x340),
        y: (0x60, 0xA0),
    },
    Op::Control(false),
    Op::Call(DESERT_AMBUSH),
    Op::End,
];

/// Van fights the thieves (task at `0x080131EC`): the field darkens and
/// loads again with Van's Zoid and the thieves ahead of the Gustav, to the
/// chase's song; Irvine and Moonbay catch up with Van (dialogue `0x7C`),
/// who drives off, and they turn on the thieves (`0x7D`); the battle
/// follows from the field's hook.
const DESERT_AMBUSH: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: DESERT,
        player: AMBUSH_LOAD_CELL,
        objects: AMBUSH_OBJECTS,
        count: 4,
    },
    Op::RestartMusic(BAR_MUSIC),
    Op::Place(PLAYER, AMBUSH_CELL),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x7C),
    glide(VAN_ZOID, (0x2A0, 0xA0), 2, true),
    Op::Place(VAN_ZOID, (0xFFFF, 0xFFFF)),
    Op::Dialogue(0x7D),
    Op::Spawn(FIELD_HOOK, AMBUSH_FIGHT),
];

/// The ambush's battle (the hook at `0x0801328C`): beaten, the party goes
/// back to its return point; winning, the desert loads again where the
/// ambush was and its song comes back.
const AMBUSH_FIGHT: &[Op] = &[
    Op::Call(&story_battle(AMBUSH_BATTLE)),
    Op::IfLost {
        then: DESERT_BATTLE_LOST,
        otherwise: &[
            Op::Flag(AMBUSH_BEATEN, true),
            Op::Warp {
                map: DESERT,
                cell: AMBUSH_LOAD_CELL,
                facing: None,
            },
            Op::FadeInHoldingSlow,
            Op::RestartMusic(DESERT_MUSIC),
        ],
    },
    Op::End,
];

/// Beaten in a story battle of the desert (`0x08006E08` with 1): the party
/// wakes at its return point facing up, the field brightening at once.
const DESERT_BATTLE_LOST: &[Op] = &[Op::WarpHome, Op::FadeInHolding];

/// After the ambush (task at `0x08013304`): Irvine grumbles about Van and
/// the party sets off after him (dialogue `0x7E`); the desert loads again
/// with the player in control.
const CHASE_TALK: &[Op] = &[
    Op::Wait(32),
    Op::Dialogue(0x7E),
    Op::Call(SCENE_DARKEN),
    Op::Warp {
        map: DESERT,
        cell: AMBUSH_CELL,
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// Once everyone in map 34 has been heard (task at `0x08013620`): the
/// party puts it together (dialogue `0x86`) and the desert loads again.
const EVERYONE_HEARD_TALK: &[Op] = &[
    Op::Wait(32),
    Op::Dialogue(0x86),
    Op::Call(SCENE_DARKEN),
    Op::Warp {
        map: DESERT,
        cell: (0xE, 9),
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// The desert (`0x080120C0`): the lookout while the party is on Van's
/// trail, the talk after the ambush, and the talk once map 34's people
/// have all been heard; the later branches are still to be ported.
pub(super) const DESERT_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[MERCENARIES_TALKED_ABOUT],
    none: &[AMBUSH_BEATEN],
    then: &[Op::Spawn(MAP_TASK, DESERT_LOOKOUT)],
    otherwise: &[Op::IfFlags {
        all: &[AMBUSH_BEATEN],
        none: &[CHASE_DECIDED],
        then: &[
            Op::LoadMap {
                map: DESERT,
                player: AMBUSH_CELL,
                objects: GUSTAV_ALONE_FACING_LEFT,
                count: 1,
            },
            Op::Control(false),
            Op::Spawn(MAP_TASK, CHASE_TALK),
            Op::Flag(CHASE_DECIDED, true),
        ],
        otherwise: &[Op::IfFlags {
            all: &ASKED_AROUND,
            none: &[HEARD_EVERYONE],
            then: &[
                Op::LoadMap {
                    map: DESERT,
                    player: (0xE, 9),
                    objects: GUSTAV_ALONE_FACING_DOWN,
                    count: 1,
                },
                Op::Control(false),
                Op::Spawn(MAP_TASK, EVERYONE_HEARD_TALK),
                Op::Flag(HEARD_EVERYONE, true),
            ],
            otherwise: &[Op::IfFlags {
                all: &[PARTY_SPLIT, RAVEN_AHEAD],
                none: &[RAVEN_BEATEN],
                then: &[Op::Call(RAVEN_STANDOFF_LOAD), Op::Flag(RAVEN_AHEAD, false)],
                otherwise: &[Op::IfFlags {
                    all: &[RAVEN_BEATEN],
                    none: &[CHAPTER_ENDED],
                    then: &[
                        Op::LoadMap {
                            map: DESERT,
                            player: RAVEN_CELL,
                            objects: RAVEN_OBJECTS,
                            count: 2,
                        },
                        Op::Place(RAVEN_ZOID, (5, 2)),
                        Op::RestartMusic(HOUSE_MUSIC),
                        Op::Control(false),
                        Op::Spawn(MAP_TASK, FINALE),
                        Op::Flag(CHAPTER_ENDED, true),
                    ],
                    otherwise: &[],
                }],
            }],
        }],
    }],
}];

/// The house of map 34, `md0256`, and its song.
const HOUSE: usize = 34;
const HOUSE_MUSIC: u16 = 6;
/// Set once the party has come into the house after the chase's talk.
const HOUSE_ENTERED: u16 = 0x149;

/// Into the house (task at `0x0801334C`): the prince walks in and looks
/// around, and the party decides to ask the people (dialogue `0x7F`); the
/// house loads again with the player in control.
const HOUSE_TALK: &[Op] = &[
    Op::Wait(32),
    walk_to(PLAYER, (0xC0, 0x130), 1),
    Op::Wait(30),
    Op::Face(PLAYER, Direction::Right),
    Op::Wait(BEAT),
    Op::Face(PLAYER, Direction::Left),
    Op::Wait(BEAT),
    Op::Face(PLAYER, Direction::Up),
    Op::Wait(BEAT),
    Op::Dialogue(0x7F),
    Op::Call(SCENE_DARKEN),
    Op::Warp {
        map: HOUSE,
        cell: (0xC, 0x13),
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// The house (`0x08012498`): the first time after the chase's talk, it
/// loads with the player at the door, to its song, and the talk runs.
pub(super) const HOUSE_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CHASE_DECIDED],
    none: &[HOUSE_ENTERED],
    then: &[
        Op::LoadMap {
            map: HOUSE,
            player: (0xC, 0x14),
            objects: 0x0866_7090,
            count: 1,
        },
        Op::RestartMusic(HOUSE_MUSIC),
        Op::Control(false),
        Op::Spawn(MAP_TASK, HOUSE_TALK),
        Op::Flag(HOUSE_ENTERED, true),
    ],
    otherwise: &[],
}];

/// Set once the desert thieves' hideout has been dealt with, after which
/// the townsfolk speak of it.
const HIDEOUT_CLEARED: u16 = 0x151;
/// Set by the desert scene that sends the party to map 34, whose six
/// people each tell a piece of the story (flags `0x14A`–`0x14F`).
const TOLD_TO_ASK_AROUND: u16 = 0x148;

/// A line chosen by the story's progress: the line of the first flag not
/// set yet, else the last line.
macro_rules! by_progress {
    ($last:expr) => {
        &[Op::Dialogue($last)]
    };
    ($flag:expr => $line:expr, $($rest:tt)+) => {
        &[Op::IfFlags {
            all: &[],
            none: &[$flag],
            then: &[Op::Dialogue($line)],
            otherwise: by_progress!($($rest)+),
        }]
    };
}

/// Sand Colony's townsfolk (`0x080066C0` on; `0x0801247C` tests flag
/// `0x145`, `0x080126D0` flag `0x151`).
pub(super) const TOWNSFOLK: [(u32, &[Op]); 10] = [
    (
        0x0800_66C0,
        by_progress!(MERCENARIES_JOINED => 0x69, HIDEOUT_CLEARED => 0x78, 0x91),
    ),
    (0x0800_66EC, by_progress!(HIDEOUT_CLEARED => 0x6A, 0x92)),
    (
        0x0800_6708,
        by_progress!(MERCENARIES_JOINED => 0x6B, HIDEOUT_CLEARED => 0x79, 0x93),
    ),
    (
        0x0800_6734,
        by_progress!(MERCENARIES_JOINED => 0x6C, HIDEOUT_CLEARED => 0x7A, 0x94),
    ),
    (
        0x0800_6760,
        by_progress!(MERCENARIES_JOINED => 0x70, HIDEOUT_CLEARED => 0x6D, 0x96),
    ),
    (0x0800_67CC, by_progress!(HIDEOUT_CLEARED => 0x70, 0x96)),
    (0x0800_6808, by_progress!(HIDEOUT_CLEARED => 0x72, 0x97)),
    (0x0800_678C, by_progress!(HIDEOUT_CLEARED => 0x6E, 0x2F9)),
    (0x0800_67AC, by_progress!(HIDEOUT_CLEARED => 0x6F, 0x2FA)),
    (0x0800_67E8, by_progress!(HIDEOUT_CLEARED => 0x71, 0x2FB)),
];

/// The people of map 34 (`0x0800683C` on; `0x0801251C` tests flag
/// `0x148`): before the party is sent there, a line each; after, each
/// tells a piece of the story and sets its flag (`0x0801252C` on).
const ASKED_AROUND: [u16; 6] = [0x14A, 0x14B, 0x14C, 0x14D, 0x14E, 0x14F];

macro_rules! witness {
    ($before:expr, $told:expr, $flag:expr) => {
        &[Op::IfFlags {
            all: &[TOLD_TO_ASK_AROUND],
            none: &[],
            then: &[Op::Dialogue($told), Op::Flag($flag, true)],
            otherwise: &[Op::Dialogue($before)],
        }]
    };
}

/// The six people of map 34 with their lines.
pub(super) const WITNESSES: [(u32, &[Op]); 6] = [
    (0x0800_683C, witness!(0x87, 0x80, 0x14A)),
    (0x0800_685C, witness!(0x88, 0x81, 0x14B)),
    (0x0800_687C, witness!(0x89, 0x82, 0x14C)),
    (0x0800_689C, witness!(0x8A, 0x83, 0x14D)),
    (0x0800_68BC, witness!(0x8B, 0x84, 0x14E)),
    (0x0800_68DC, witness!(0x8C, 0x85, 0x14F)),
];

/// The teachers of area 2 (`0x08012090`: the pitch and the lesson, later a
/// reminder): deck command 2 in Sand Colony, 0x18 in map 34.
const TEACHES_COMMAND_2: &[Op] = &[Op::IfCommand {
    command: 2,
    then: &[Op::Dialogue(0x3E6)],
    otherwise: &[Op::Dialogue(0x3D1), Op::Call(&learn(2))],
}];
const TEACHES_COMMAND_24: &[Op] = &[Op::IfCommand {
    command: 0x18,
    then: &[Op::Dialogue(0x3E7)],
    otherwise: &[Op::Dialogue(0x3D2), Op::Call(&learn(0x18))],
}];

/// Sand Colony's item shop (`0x08009114`, item shop 2), armaments shop
/// (`0x08009120`, 2) and Zoid lab (`0x0800912C`, 2).
const ITEM_SHOP: &[Op] = &shop(Shop::Items(2));
const ARMS_SHOP: &[Op] = &shop(Shop::Arms(2));
const LAB: &[Op] = &shop(Shop::Lab(2));

/// What speaking to an object of area 2 whose script is code runs.
pub(super) fn talk_handler(address: u32) -> Option<&'static [Op]> {
    let listed = TOWNSFOLK
        .iter()
        .chain(WITNESSES.iter())
        .find(|(at, _)| *at == address)
        .map(|(_, program)| *program);
    listed.or(match address {
        0x0801_2444 => Some(MOONBAY_TALK),
        0x0800_6824 => Some(TEACHES_COMMAND_2),
        0x0800_68FC => Some(TEACHES_COMMAND_24),
        0x0800_9114 => Some(ITEM_SHOP),
        0x0800_9120 => Some(ARMS_SHOP),
        0x0800_912C => Some(LAB),
        _ => None,
    })
}

/// A step of a pan: a pixel a frame.
const PAN_UP: &[Op] = &[Op::Pan(0, -PIXEL), Op::Wait(1)];
const PAN_DOWN: &[Op] = &[Op::Pan(0, PIXEL), Op::Wait(1)];
const PAN_RIGHT: &[Op] = &[Op::Pan(PIXEL, 0), Op::Wait(1)];

/// Beaten in one of the chapter's story battles, the hook's defeat
/// branch (see [`DESERT_BATTLE_LOST`]).
const fn story_fight(battle: u8, won: &'static [Op]) -> [Op; 3] {
    [
        Op::Call(&STORY_BATTLES[battle as usize - AMBUSH_BATTLE as usize]),
        Op::IfLost {
            then: DESERT_BATTLE_LOST,
            otherwise: won,
        },
        Op::End,
    ]
}

/// The chapter's story battles 2 to 5 (`0x08008D28`).
const STORY_BATTLES: [[Op; 4]; 4] = [
    story_battle(2),
    story_battle(3),
    story_battle(4),
    story_battle(5),
];

/// The thieves' hideout, `mq0260` (map 37).
const HIDEOUT: usize = 37;
/// Its objects (ROM `0x086670A4`): the party's four Zoids, Irvine's and
/// Moonbay's, Van's Blade Liger and the thieves' two.
const HIDEOUT_OBJECTS: u32 = 0x0866_70A4;
const HIDEOUT_OBJECT_COUNT: usize = 9;
const HIDEOUT_MUSIC: u16 = 9;
/// Set once Van and Zeke have joined the party in the hideout; kept when
/// the battle there is lost, so they do not join twice.
const VAN_JOINED: u16 = 0x197;
/// The hideout's fight (story battle 3).
const HIDEOUT_BATTLE: u8 = 3;
/// Set once the party has left the hideout after the fight.
const HIDEOUT_LEFT: u16 = 0x152;
/// The list of characters who join in the hideout (Van and Zeke).
const VAN_AND_ZEKE: u8 = 2;
const HIDEOUT_CELL: (usize, usize) = (4, 5);

/// Into the hideout (task at `0x08013668`; turns name the entity, one past
/// the object): the Gustav drives in, Van's Blade Liger (unless Van has
/// joined already) and the party's Zoids gather, the camera goes up two
/// cells, and the first time Van joins (dialogue `0x8D`); the fight
/// follows from the field's hook.
const HIDEOUT_ENTRY: &[Op] = &[
    Op::Wait(32),
    glide(PLAYER, (0x80, 0xA0), 1, true),
    Op::IfFlags {
        all: &[VAN_JOINED],
        none: &[],
        then: &[Op::Place(6, HIDEOUT_CELL), glide(6, (0x60, 0x60), 2, true)],
        otherwise: &[],
    },
    Op::Place(5, HIDEOUT_CELL),
    glide(5, (0x80, 0x60), 2, true),
    Op::Place(1, HIDEOUT_CELL),
    glide(1, (0xA0, 0x60), 2, true),
    Op::Place(2, HIDEOUT_CELL),
    Op::Place(3, HIDEOUT_CELL),
    Op::Place(4, HIDEOUT_CELL),
    Op::Spawn(
        MAP_TASK + 1,
        &[
            glide(3, (0x80, 0x80), 1, true),
            Op::Face(3, Direction::Up),
            Op::End,
        ],
    ),
    Op::Spawn(
        MAP_TASK + 2,
        &[
            glide(4, (0xA0, 0x80), 1, true),
            Op::Face(4, Direction::Up),
            Op::End,
        ],
    ),
    glide(2, (0x60, 0x80), 1, true),
    Op::Repeat(64, PAN_UP),
    Op::IfFlags {
        all: &[],
        none: &[VAN_JOINED],
        then: &[
            Op::Dialogue(0x8D),
            Op::Join(VAN_AND_ZEKE),
            Op::Flag(VAN_JOINED, true),
        ],
        otherwise: &[],
    },
    Op::Spawn(FIELD_HOOK, &HIDEOUT_FIGHT),
    Op::End,
];

/// The hideout's fight (the hook at `0x080137D0`): won, the hideout loads
/// again to its song.
const HIDEOUT_FIGHT: [Op; 3] = story_fight(
    HIDEOUT_BATTLE,
    &[
        Op::Flag(HIDEOUT_CLEARED, true),
        Op::Warp {
            map: HIDEOUT,
            cell: HIDEOUT_CELL,
            facing: None,
        },
        Op::FadeInHoldingSlow,
        Op::RestartMusic(HIDEOUT_MUSIC),
    ],
);

/// After the fight (task at `0x08013840`): the thieves are gone
/// (dialogue `0x8E`), everyone drives out, the town's song plays and the
/// party plans (`0x90`); the desert loads by the hideout.
const HIDEOUT_CLEARED_TALK: &[Op] = &[
    Op::Wait(32),
    Op::Dialogue(0x8E),
    glide(3, (0x80, 0xA0), 2, true),
    Op::Place(3, OFF_THE_MAP),
    glide(4, (0x80, 0xA0), 2, true),
    Op::Place(4, OFF_THE_MAP),
    glide(2, (0x80, 0xA0), 2, true),
    Op::Place(2, OFF_THE_MAP),
    glide(5, (0x80, 0xA0), 2, true),
    Op::Place(5, OFF_THE_MAP),
    glide(1, (0x80, 0xA0), 2, true),
    Op::Place(1, OFF_THE_MAP),
    glide(6, (0x80, 0xA0), 2, true),
    Op::Place(6, OFF_THE_MAP),
    glide(PLAYER, (0x80, 0xC0), 2, true),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::RestartMusic(SAND_COLONY_MUSIC),
    Op::Dialogue(0x90),
    Op::Call(SCENE_DARKEN),
    Op::Warp {
        map: DESERT,
        cell: (0x11, 1),
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// The hideout (`0x080125A4`): after the party has put the story
/// together, it loads with the Zoids off the map (Van's Blade Liger hidden
/// once Van has joined) and the entry runs; after the fight, it loads with
/// everyone standing and the talk runs.
pub(super) const HIDEOUT_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[HEARD_EVERYONE],
    none: &[HIDEOUT_CLEARED],
    then: &[
        Op::LoadMap {
            map: HIDEOUT,
            player: (4, 9),
            objects: HIDEOUT_OBJECTS,
            count: HIDEOUT_OBJECT_COUNT,
        },
        Op::RestartMusic(BAR_MUSIC),
        Op::IfFlags {
            all: &[VAN_JOINED],
            none: &[],
            then: &[Op::Place(6, OFF_THE_MAP)],
            otherwise: &[],
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, HIDEOUT_ENTRY),
    ],
    otherwise: &[Op::IfFlags {
        all: &[HIDEOUT_CLEARED],
        none: &[HIDEOUT_LEFT],
        then: &[
            Op::LoadMap {
                map: HIDEOUT,
                player: (4, 3),
                objects: HIDEOUT_OBJECTS,
                count: HIDEOUT_OBJECT_COUNT,
            },
            Op::Place(PLAYER, (4, 5)),
            Op::Place(1, (5, 3)),
            Op::Place(2, (3, 4)),
            Op::Place(3, (4, 4)),
            Op::Place(4, (5, 4)),
            Op::Place(5, (4, 3)),
            Op::Control(false),
            Op::Spawn(MAP_TASK, HIDEOUT_CLEARED_TALK),
            Op::Flag(HIDEOUT_LEFT, true),
        ],
        otherwise: &[],
    }],
}];

/// The canyon, `mq0261` (map 38).
const CANYON: usize = 38;
/// Its objects (ROM `0x08667158`): the party's four Zoids, Irvine's,
/// Moonbay's and Van's, and Raven's Zoid below.
const CANYON_OBJECTS: u32 = 0x0866_7158;
const CANYON_OBJECT_COUNT: usize = 8;
/// Set once Raven's first fight is over (story battle 4).
const RAVEN_FOUGHT: u16 = 0x153;
/// Set once the party has split up after it.
const PARTY_SPLIT: u16 = 0x154;
/// Set while Raven waits in the desert for the prince alone; the desert's
/// handler clears it as it starts the standoff.
const RAVEN_AHEAD: u16 = 0x157;
const CANYON_BATTLE: u8 = 4;
const CANYON_CELL: (usize, usize) = (5, 8);
const RAVEN: usize = 7;

/// The canyon's lookout (the handler's hook, `0x08012888`): once the Gustav
/// stands at y `0xA0` (row 5) with x `0x80` or less (the first five
/// columns), it stops and the approach runs.
const CANYON_LOOKOUT: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (i32::MIN, 0x80),
        y: (0xA0, 0xA0),
    },
    Op::Control(false),
    Op::Call(CANYON_APPROACH),
    Op::End,
];

/// Raven's ambush in the canyon (task at `0x08013964`; turns name the
/// entity): to the chase's song the Gustav drives down, the camera pans two
/// cells right, the party's Zoids gather beside it and Raven speaks
/// (dialogue `0x98`); the fight follows from the field's hook.
const CANYON_APPROACH: &[Op] = &[
    Op::RestartMusic(BAR_MUSIC),
    glide(PLAYER, (0x40, 0x100), 1, true),
    Op::Face(PLAYER, Direction::Right),
    Op::Repeat(64, PAN_RIGHT),
    Op::Place(6, (2, 8)),
    glide(6, (0x80, 0x100), 2, true),
    Op::Place(1, (2, 8)),
    glide(1, (0x80, 0xE0), 2, true),
    Op::Place(5, (2, 8)),
    glide(5, (0x80, 0x120), 2, true),
    Op::Place(2, (2, 8)),
    Op::Place(3, (2, 8)),
    Op::Place(4, (2, 8)),
    Op::Spawn(
        MAP_TASK + 1,
        &[
            glide(3, (0x60, 0x100), 1, true),
            Op::Face(3, Direction::Right),
            Op::End,
        ],
    ),
    Op::Spawn(
        MAP_TASK + 2,
        &[
            glide(4, (0x60, 0x120), 1, true),
            Op::Face(4, Direction::Right),
            Op::End,
        ],
    ),
    glide(2, (0x60, 0xE0), 1, true),
    Op::Face(2, Direction::Right),
    Op::Dialogue(0x98),
    Op::Spawn(FIELD_HOOK, &CANYON_FIGHT),
    Op::End,
];

/// The canyon's fight (the hook at `0x08013AD0`).
const CANYON_FIGHT: [Op; 3] = story_fight(
    CANYON_BATTLE,
    &[
        Op::Flag(RAVEN_FOUGHT, true),
        Op::Warp {
            map: CANYON,
            cell: CANYON_CELL,
            facing: None,
        },
        Op::FadeInHoldingSlow,
    ],
);

/// The canyon's second objects (ROM `0x08667310`), for the talk above.
const CANYON_TALK_OBJECTS: u32 = 0x0866_7310;

/// After Raven's first fight (task at `0x08013B2C`): Raven taunts and
/// drives off (dialogues `0x99`, `0x9A`); the field darkens and loads the
/// canyon's top, where the party gathers to the town's song (`0x9B`), the
/// camera goes down two cells and they part (`0x9C`): Irvine, Moonbay,
/// Van and Zeke leave the party, and Raven waits in the desert.
const RAVEN_TAUNT: &[Op] = &[
    Op::Wait(32),
    Op::Dialogue(0x99),
    glide(RAVEN, (0xA0, 0x100), 4, true),
    glide(RAVEN, (0xA0, 0x140), 4, true),
    glide(RAVEN, (0, 0x140), 4, true),
    Op::Place(RAVEN, OFF_THE_MAP),
    Op::Dialogue(0x9A),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: CANYON,
        player: (3, 1),
        objects: CANYON_TALK_OBJECTS,
        count: 9,
    },
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::RestartMusic(SAND_COLONY_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Place(8, (3, 1)),
    Op::Wait(BEAT),
    glide(5, (0x60, 0x40), 2, true),
    Op::Wait(15),
    Op::Spawn(MAP_TASK + 1, &[glide(7, (0x40, 0x40), 2, true), Op::End]),
    glide(6, (0x80, 0x40), 2, true),
    Op::Wait(15),
    Op::Spawn(MAP_TASK + 1, &[glide(2, (0x40, 0x80), 1, true), Op::End]),
    Op::Spawn(MAP_TASK + 2, &[glide(3, (0x60, 0x80), 1, true), Op::End]),
    Op::Spawn(MAP_TASK + 3, &[glide(4, (0x80, 0x80), 1, true), Op::End]),
    glide(1, (0x80, 0x60), 2, true),
    Op::Wait(BEAT),
    Op::Dialogue(0x9B),
    Op::Wait(15),
    Op::Face(5, Direction::Down),
    Op::Face(6, Direction::Down),
    Op::Face(7, Direction::Down),
    Op::Wait(15),
    Op::Repeat(64, PAN_DOWN),
    Op::Dialogue(0x9C),
    Op::Leave(MERCENARIES),
    Op::Leave(VAN_AND_ZEKE),
    Op::Call(SCENE_DARKEN),
    Op::Flag(RAVEN_AHEAD, true),
    Op::Spawn(FIELD_HOOK, TO_RAVEN),
    Op::End,
];

/// The hook the canyon's talk leaves (`0x08013CCC`): the desert loads
/// where Raven waits.
const TO_RAVEN: &[Op] = &[
    Op::Warp {
        map: DESERT,
        cell: RAVEN_CELL,
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// The canyon (`0x080126E0`): with the hideout left behind and the Gustav
/// at x `0x200` or less, it loads with the party's Zoids and Raven and the
/// lookout waits; after Raven's first fight it loads with everyone
/// standing to the hideout's song and the taunt runs; with the party split
/// but Raven not yet met in the desert, it takes the prince to the desert's
/// standoff.
pub(super) const CANYON_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[HIDEOUT_LEFT],
    none: &[RAVEN_FOUGHT],
    then: &[Op::IfPlayerSprite {
        x: (i32::MIN, 0x200),
        y: (i32::MIN, i32::MAX),
        then: &[
            Op::LoadMap {
                map: CANYON,
                player: HERE,
                objects: CANYON_OBJECTS,
                count: CANYON_OBJECT_COUNT,
            },
            Op::Spawn(MAP_TASK, CANYON_LOOKOUT),
        ],
        otherwise: &[],
    }],
    otherwise: &[Op::IfFlags {
        all: &[RAVEN_FOUGHT],
        none: &[PARTY_SPLIT],
        then: &[
            Op::LoadMap {
                map: CANYON,
                player: CANYON_CELL,
                objects: CANYON_OBJECTS,
                count: CANYON_OBJECT_COUNT,
            },
            Op::RestartMusic(HIDEOUT_MUSIC),
            Op::Place(PLAYER, (2, 8)),
            Op::Face(PLAYER, Direction::Right),
            Op::Control(false),
            Op::Place(1, (4, 7)),
            Op::Place(2, (3, 7)),
            Op::Place(3, (3, 8)),
            Op::Place(4, (3, 9)),
            Op::Place(5, (4, 9)),
            Op::Place(6, (4, 8)),
            Op::Flag(PARTY_SPLIT, true),
            Op::Control(false),
            Op::Spawn(MAP_TASK, RAVEN_TAUNT),
        ],
        otherwise: &[Op::IfFlags {
            all: &[PARTY_SPLIT],
            none: &[RAVEN_BEATEN, RAVEN_AHEAD],
            then: RAVEN_STANDOFF_LOAD,
            otherwise: &[],
        }],
    }],
}];

/// Where the prince meets Raven alone in the desert, and its objects (ROM
/// `0x08666F00`): the Gustav and Raven's Zoid.
const RAVEN_CELL: (usize, usize) = (4, 2);
const RAVEN_OBJECTS: u32 = 0x0866_6F00;
const RAVEN_ZOID: usize = 1;
/// Set once Raven is beaten in the desert (story battle 5).
const RAVEN_BEATEN: u16 = 0x155;
/// Set once the chapter's last scene has started.
const CHAPTER_ENDED: u16 = 0x156;
const RAVEN_BATTLE: u8 = 5;
/// The Zoids' drive, which the standoff waits for.
const DRIVE_SOUND: u16 = 0x68;

/// The desert loaded for the standoff with Raven, to the chase's song.
const RAVEN_STANDOFF_LOAD: &[Op] = &[
    Op::LoadMap {
        map: DESERT,
        player: RAVEN_CELL,
        objects: RAVEN_OBJECTS,
        count: 2,
    },
    Op::RestartMusic(BAR_MUSIC),
    Op::Control(false),
    Op::Spawn(MAP_TASK, RAVEN_STANDOFF),
];

/// The prince alone against Raven (task at `0x08013D70`): Raven's Zoid
/// drives in to its sound (dialogue `0x9D`); the fight follows from the
/// field's hook.
const RAVEN_STANDOFF: &[Op] = &[
    Op::Wait(32),
    Op::Sound(DRIVE_SOUND),
    Op::Wait(BEAT),
    Op::Wait(BEAT),
    glide(RAVEN_ZOID, (0xA0, 0x40), 4, true),
    Op::AwaitSoundEnd(DRIVE_SOUND),
    Op::Dialogue(0x9D),
    Op::Spawn(FIELD_HOOK, &RAVEN_FIGHT),
    Op::End,
];

/// The standoff's fight (the hook at `0x08013DD8`).
const RAVEN_FIGHT: [Op; 3] = story_fight(
    RAVEN_BATTLE,
    &[
        Op::Flag(RAVEN_BEATEN, true),
        Op::Warp {
            map: DESERT,
            cell: RAVEN_CELL,
            facing: None,
        },
        Op::FadeInHoldingSlow,
    ],
);

/// Where the portal past the Red River stands (map 30) and in the ruins
/// beyond it (map 49), and the ruins (`mq0292`).
const DESERT_PORTAL: usize = 1;
const DESERT_PORTAL_CELL: (usize, usize) = (19, 21);
const RUINS: usize = 49;
const RUINS_PORTAL: usize = 6;
const RUINS_PORTAL_CELL: (usize, usize) = (4, 1);
/// The ruins' objects (ROM `0x08667428`): Raven's Zoid, the three
/// guardians, the Gustav off the map and the portal.
const RUINS_OBJECTS: u32 = 0x0866_7428;
const RUINS_GUSTAV: usize = 5;
/// The throne room hours before, with the Emperor, Fran and the guards
/// (ROM `0x086673C4`).
const THRONE_ROOM_OBJECTS: u32 = 0x0866_73C4;
const THRONE_ROOM: usize = 48;
const FRAN: usize = 3;
const GUARD: usize = 4;
/// The portal's run that takes whoever stands on it, and its opening.
const PORTAL_TAKES: usize = 1;
const PORTAL_OPENS: usize = 2;
const PORTAL_IDLE: usize = 0;
const PORTAL_SOUND: u16 = 0x6F;
const PORTAL_GONE_SOUND: u16 = 0x44;
const PORTAL_ARRIVAL_SOUND: u16 = 0x49;
/// The steps of the portal's runs on whose last frame the one it takes is
/// gone, its second sound plays, and the one it brings is out.
const PORTAL_TAKES_STEP: usize = 8;
const PORTAL_GONE_STEP: usize = 41;
const PORTAL_BRINGS_STEP: usize = 32;
/// The song of the throne room's flashback, of the portal's crossing, and
/// of the arrival beyond.
const CROSSING_MUSIC: u16 = 4;
const BEYOND_MUSIC: u16 = 0xA;
/// Chapter 3's first map, where the chapter's end leaves the party.
const CHAPTER_3_START: usize = 82;

/// The chapter's end (task at `0x08013E34`; turns name the entity, one past
/// the object): Raven drives off (dialogues `0x9E`, `0x9F`); in the throne
/// room Fran steps aside and a guard comes up to report (`0xA0` to `0xA2`); in the ruins the guardians wait
/// (`0xA3`); past the Red River the Gustav drives into the portal
/// (`0xA4`), which takes it; in the ruins it comes out of the other portal
/// to a new song after Raven leaves (`0xA5` to `0xA7`), and the party goes
/// on into chapter 3's first map.
const FINALE: &[Op] = &[
    Op::Wait(32),
    Op::Dialogue(0x9E),
    Op::Sound(DRIVE_SOUND),
    glide(RAVEN_ZOID, (0x120, 0x40), 4, true),
    Op::AwaitSoundEnd(DRIVE_SOUND),
    Op::Dialogue(0x9F),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: THRONE_ROOM,
        player: (8, 2),
        objects: THRONE_ROOM_OBJECTS,
        count: 5,
    },
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xA0),
    Op::Face(FRAN, Direction::Down),
    Op::Wait(BEAT),
    walk_to(FRAN, (0xA0, 0x40), 1),
    Op::Face(FRAN, Direction::Left),
    Op::Wait(BEAT),
    walk_to(GUARD, (0x80, 0x30), 1),
    Op::Dialogue(0xA1),
    walk_to(GUARD, (0x80, 0xA0), 1),
    Op::Dialogue(0xA2),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: RUINS,
        player: (4, 2),
        objects: RUINS_OBJECTS,
        count: 7,
    },
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xA3),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: DESERT,
        player: (0x13, 0x16),
        objects: 0x0866_74B4,
        count: 2,
    },
    Op::RestartMusic(CROSSING_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xA4),
    glide(PLAYER, (0x260, 0x2C0), 1, true),
    super::through(PLAYER, DESERT_PORTAL_CELL, PIXEL, 1),
    Op::PlayOnce(DESERT_PORTAL, PORTAL_TAKES),
    Op::Sound(PORTAL_SOUND),
    Op::AwaitStepEnd(DESERT_PORTAL, PORTAL_TAKES_STEP),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::AwaitStepEnd(DESERT_PORTAL, PORTAL_GONE_STEP),
    Op::Sound(PORTAL_GONE_SOUND),
    Op::AwaitAnimation(DESERT_PORTAL),
    Op::Animate(DESERT_PORTAL, PORTAL_IDLE),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: RUINS,
        player: (4, 2),
        objects: RUINS_OBJECTS,
        count: 7,
    },
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xA5),
    Op::Spawn(MAP_TASK + 1, &[glide(2, (0x80, 0x60), 1, true), Op::End]),
    Op::Spawn(MAP_TASK + 2, &[glide(3, (0x60, 0x60), 1, true), Op::End]),
    glide(4, (0xA0, 0x60), 1, true),
    Op::Face(RAVEN_ZOID, Direction::Down),
    Op::Dialogue(0xA6),
    Op::Sound(DRIVE_SOUND),
    glide(RAVEN_ZOID, (0, 0x40), 2, true),
    Op::Place(RAVEN_ZOID, OFF_THE_MAP),
    Op::AwaitSoundEnd(DRIVE_SOUND),
    Op::RestartMusic(BEYOND_MUSIC),
    Op::PlayOnce(RUINS_PORTAL, PORTAL_OPENS),
    Op::Sound(PORTAL_SOUND),
    Op::AwaitStepEnd(RUINS_PORTAL, PORTAL_BRINGS_STEP),
    Op::Show(RUINS_GUSTAV),
    Op::Place(RUINS_GUSTAV, RUINS_PORTAL_CELL),
    super::through(RUINS_GUSTAV, (4, 2), PIXEL, 1),
    Op::Sound(PORTAL_ARRIVAL_SOUND),
    Op::AwaitAnimation(RUINS_PORTAL),
    Op::Animate(RUINS_PORTAL, PORTAL_IDLE),
    Op::Repeat(32, PAN_DOWN),
    Op::Dialogue(0xA7),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, TO_CHAPTER_3),
    Op::End,
];

/// The hook the chapter's end leaves (`0x0801423C`): chapter 3's first map
/// loads and brightens.
const TO_CHAPTER_3: &[Op] = &[
    Op::Warp {
        map: CHAPTER_3_START,
        cell: (8, 2),
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// Where the port's story stops: chapter 3's first map once chapter 2 is
/// over.
pub(super) const STORY_END: (usize, u16) = (CHAPTER_3_START, CHAPTER_ENDED);
