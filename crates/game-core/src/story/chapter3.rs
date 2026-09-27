//! Chapter 3: the rare-hertz desert, the Mount Ossa fortress and the
//! Kronos fort (area 3).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! map handlers of area 3, the tasks and field hooks they install and the
//! objects' code, named at each item. See `docs/events.md`.

use super::{
    FIELD_HOOK, PLAYER, SCENE_BRIGHTEN, SCENE_DARKEN, glide, learn, shop, story_battle, through,
};
use crate::event::{HERE, MAP_TASK, Op};
use crate::field::{Direction, PIXEL};
use crate::menu::Shop;

/// Set once the Emperor's scene that opens the chapter has run
/// (`0x08014C68`).
pub const CHAPTER_OPENED: u16 = 0x158;
/// Set once the party has come through the portal into the desert.
const DESERT_REACHED: u16 = 0x159;
/// Tested with [`DESERT_REACHED`] before the rare-hertz zone is watched;
/// the game's code never sets it.
const ZONE_UNWATCHED: u16 = 0x15A;
/// Set the first time the party is turned back at the rare-hertz zone.
const TURNED_BACK: u16 = 0x15B;
/// Set once Opis's runaway Zoids are beaten (story battle 6).
const AMBUSH_BEATEN: u16 = 0x15C;
/// Set on entering the town; the next time out, the runaway Zoids strike.
const TOWN_VISITED: u16 = 0x15D;
/// Set once Opis has shown himself after the ambush.
const OPIS_MET: u16 = 0x15E;
/// Set with the amplifier's core destroyed: the fortress's rear entrance
/// lets the party through.
const REAR_ENTRANCE_OPEN: u16 = 0x15F;
/// Set once the rear entrance's guard has sent the party away once.
const REAR_GUARD_MET: u16 = 0x160;
/// Set once Dr. D has taken the party into the fortress.
const FORTRESS_ENTERED: u16 = 0x161;
/// Set once the command room has sent the party to measure the zone.
const MISSION_GIVEN: u16 = 0x162;
/// Set once the party has been told it stands at the zone's edge.
const ZONE_REACHED: u16 = 0x163;
/// Set once the measurements are in and the amplifiers can be destroyed.
const AMPLIFIERS_FOUND: u16 = 0x164;
/// Set as each of the four amplifiers of the desert is destroyed.
const AMPLIFIERS_DESTROYED: [u16; 4] = [0x165, 0x166, 0x167, 0x168];
/// Set once the party has reached the Kronos fort.
const KRONOS_REACHED: u16 = 0x169;
/// Set once the amplifier's core in the fort is destroyed.
const CORE_DESTROYED: u16 = 0x16A;
/// Set once the rock-boring laser has been found.
const LASER_FOUND: u16 = 0x16B;
/// Set once Opis is beaten at the crater (story battle 7).
const OPIS_BEATEN: u16 = 0x16C;
/// Set once Van and Irvine have left for the crater; kept when the fight
/// that follows is lost, so they leave once.
const SENT_TO_THE_CRATER: u16 = 0x198;

/// The desert around Mount Ossa, `mq0300` (map 50), a Zoid map.
const DESERT: usize = 50;
/// The Emperor's throne room (map 82), where the chapter opens and where
/// chapter 2's end leaves the party.
const THRONE_ROOM: usize = 82;
/// The castle's hall (map 81) and the room above the base's bar (map 92).
const CASTLE_HALL: usize = 81;
const BASE_ROOM: usize = 92;
/// The base's portal room (map 86).
const PORTAL_ROOM: usize = 86;
/// The fortress's gate (map 61), a wing (64) and the command room (66).
const FORTRESS_GATE: usize = 61;
const FORTRESS_WING: usize = 64;
const FORTRESS_HALL: usize = 65;
const COMMAND_ROOM: usize = 66;
/// The rear entrance (map 68), and the tunnels where the laser lies (71).
const REAR_ENTRANCE: usize = 68;
const TUNNELS: usize = 71;
/// The Kronos fort: its outskirts (54), their other view (55), the room
/// where Krueger waits (57) and its heart (60).
const KRONOS_OUTSKIRTS: usize = 54;
const KRONOS_VIEW: usize = 55;
const KRONOS_ROOM: usize = 57;
const KRONOS_HEART: usize = 60;
/// The crater's path (map 77).
const CRATER: usize = 77;
/// Chapter 4's first map, where the chapter's end leaves the party.
const CHAPTER_4_START: usize = 120;

/// The songs the chapter switches to.
const ALARM_MUSIC: u16 = 0x1E;
const DANGER_MUSIC: u16 = 4;
const OPIS_MUSIC: u16 = 9;
const DR_D_MUSIC: u16 = 7;
const RESCUE_MUSIC: u16 = 0xE;
const CHASE_MUSIC: u16 = 5;
const VICTORY_MUSIC: u16 = 1;
const LASER_MUSIC: u16 = 0x16;

const PORTAL_SOUND: u16 = 0x6F;
const PORTAL_GONE_SOUND: u16 = 0x44;
const PORTAL_ARRIVAL_SOUND: u16 = 0x49;
const BLAST_SOUND: u16 = 0x5B;
const CONTAINER_SOUND: u16 = 0x46;
const ERUPTION_SOUND: u16 = 0x62;
/// The portal's runs: standing, taking whoever stands on it, and opening
/// to bring someone; the steps on whose last frame the one taken is gone,
/// the second sound plays, and the one brought is out.
const PORTAL_IDLE: usize = 0;
const PORTAL_TAKES: usize = 1;
const PORTAL_OPENS: usize = 2;
const PORTAL_TAKES_STEP: usize = 8;
const PORTAL_GONE_STEP: usize = 41;
const PORTAL_BRINGS_STEP: usize = 32;

/// Frames a task waits after the fade in before its first line.
const SETTLE: u32 = 32;
const OFF_THE_MAP: (usize, usize) = (0xFF, 0xFF);
/// The rare-hertz zone: the attribute bit of its cells in the desert.
const RARE_HERTZ: u16 = 0x1000;
/// Roaming battles the party must win in a row inside the zone before the
/// measurements are complete: more than this.
const ZONE_BATTLES: u16 = 2;

/// A glide with cells of 16 (`0x08011F18`): the routine moves its target 8
/// pixels left, where the sprite's box starts on such a map.
const fn glide16(actor: usize, (x, y): (i32, i32), speed: i32, camera: bool) -> Op {
    Op::Glide {
        actor,
        to: (x - 8, y),
        speed,
        frames: 16 / speed.unsigned_abs(),
        camera,
    }
}

/// A scene's last beat (`0x08011E08`, the field's hooks cleared, the task
/// ended, `0x08007188` and `0x080014A8` with 1): the screen darkens, and
/// the party is taken to `cell` of `map` with its own objects, which
/// brightens slowly.
const fn back_to(map: usize, cell: (usize, usize)) -> [Op; 4] {
    [
        Op::Call(SCENE_DARKEN),
        Op::Warp {
            map,
            cell,
            facing: None,
        },
        Op::FadeInHoldingSlow,
        Op::End,
    ]
}

/// A story battle's hook: beaten, the party is taken to its return point
/// and the field brightens at once (the defeat's branch of `0x08015338`
/// and `0x080168B0`, as chapter 2's); otherwise `won` runs.
const BEATEN: &[Op] = &[Op::WarpHome, Op::FadeInHolding];

/// The throne room's objects for the opening (ROM `0x0866770C`): the
/// prince off the map, the Emperor, Fran, Blood, Gale and Opis.
const OPENING_OBJECTS: u32 = 0x0866_770C;
const FRAN: usize = 2;
const BLOOD: usize = 3;
const GALE: usize = 4;
const OPIS: usize = 5;
/// The castle's hall with Blood and Gale (ROM `0x08667784`).
const HALL_OBJECTS: u32 = 0x0866_7784;
const HALL_BLOOD: usize = 1;
const HALL_GALE: usize = 2;
/// The room above the base's bar (ROM `0x086677C0`): the prince seated,
/// Regina, Jack, Earth and a soldier off the map.
const BASE_OBJECTS: u32 = 0x0866_77C0;
const REGINA: usize = 1;
const JACK: usize = 2;
const EARTH: usize = 3;
const BASE_SOLDIER: usize = 4;
/// The prince standing again (`0x080089A0`).
const PRINCE_STANDING: usize = 0x98;

/// Gale leaves the throne room (task at `0x08014FD8`).
const GALE_LEAVES: &[Op] = &[
    glide16(GALE, (0x90, 0xC0), 1, true),
    Op::Place(GALE, OFF_THE_MAP),
    Op::End,
];

/// The chapter's opening (task at `0x08014D5C`; turns name the entity,
/// one past the object): Fran reports her failure and is sent away
/// (dialogues `0xA8`, `0xA9`); Opis brings the rare-hertz amplifier and
/// takes the command, Blood and Gale are sent to guard the castle (`0xAA`,
/// `0xAB`); in the hall Blood hints that Gale let the prince go (`0xAC`,
/// `0xAD`); at the base a soldier reports the space-time device in use
/// (`0xAE`), and the party gets ready to follow.
const OPENING: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Dialogue(0xA8),
    glide16(FRAN, (0x80, 0xC0), 1, true),
    Op::Place(FRAN, OFF_THE_MAP),
    Op::Dialogue(0xA9),
    Op::Place(OPIS, (8, 0xC)),
    glide16(OPIS, (0x80, 0x30), 1, true),
    Op::Dialogue(0xAA),
    Op::Spawn(MAP_TASK + 1, GALE_LEAVES),
    glide16(BLOOD, (0x70, 0xC0), 1, true),
    Op::Place(BLOOD, OFF_THE_MAP),
    Op::Dialogue(0xAB),
    glide16(OPIS, (0x80, 0xC0), 1, true),
    Op::Place(OPIS, OFF_THE_MAP),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: CASTLE_HALL,
        player: (0x17, 0xF),
        objects: HALL_OBJECTS,
        count: 3,
    },
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    glide16(HALL_GALE, (0x180, 0xE0), 1, true),
    Op::Face(HALL_GALE, Direction::Up),
    Op::Dialogue(0xAC),
    glide16(HALL_BLOOD, (0x170, 0xE0), 1, true),
    Op::Face(HALL_BLOOD, Direction::Right),
    Op::Face(HALL_GALE, Direction::Left),
    Op::Dialogue(0xAD),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: BASE_ROOM,
        player: (2, 5),
        objects: BASE_OBJECTS,
        count: 5,
    },
    Op::RestartMusic(ALARM_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Place(BASE_SOLDIER, (0xE, 2)),
    glide16(BASE_SOLDIER, (0x60, 0x20), 1, true),
    glide16(BASE_SOLDIER, (0x60, 0x50), 1, true),
    Op::Face(BASE_SOLDIER, Direction::Left),
    Op::Face(JACK, Direction::Right),
    Op::Face(EARTH, Direction::Right),
    Op::Dialogue(0xAE),
    Op::Sprite(PLAYER, PRINCE_STANDING),
    Op::Face(PLAYER, Direction::Right),
    glide16(PLAYER, (0x20, 0x40), 1, false),
    glide16(REGINA, (0x10, 0x40), 1, true),
    glide16(REGINA, (0x20, 0x40), 1, true),
    Op::Place(REGINA, OFF_THE_MAP),
    glide16(JACK, (0x10, 0x70), 1, true),
    glide16(JACK, (0x10, 0x40), 1, true),
    glide16(JACK, (0x20, 0x40), 1, true),
    Op::Place(JACK, OFF_THE_MAP),
    glide16(EARTH, (0x10, 0x70), 1, true),
    glide16(EARTH, (0x10, 0x40), 1, true),
    glide16(EARTH, (0x20, 0x40), 1, true),
    Op::Place(EARTH, OFF_THE_MAP),
    Op::Call(SCENE_DARKEN),
    Op::Warp {
        map: BASE_ROOM,
        cell: (2, 4),
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// The throne room (`0x08014C68`): once chapter 2 is over, the first time,
/// the characters of group 2 are met, the room loads with the court and
/// the player off the map, and the opening runs.
pub(super) const THRONE_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[CHAPTER_OPENED],
    then: &[
        Op::Meet(2),
        Op::LoadMap {
            map: THRONE_ROOM,
            player: (8, 2),
            objects: OPENING_OBJECTS,
            count: 6,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, OPENING),
        Op::Place(PLAYER, (0xFFFF, 0xFFFF)),
        Op::Flag(CHAPTER_OPENED, true),
    ],
    otherwise: &[],
}];

/// The desert as the portal brings the party in (ROM `0x08667554`): the
/// Gustav off the map and the portal.
const PORTAL_ARRIVAL_OBJECTS: u32 = 0x0866_7554;
const DESERT_PORTAL: usize = 1;
const DESERT_PORTAL_CELL: (usize, usize) = (14, 2);
const DESERT_ARRIVAL_CELL: (usize, usize) = (0xD, 2);

/// Through the portal (task at `0x08015000`): it opens with its sound, on
/// the last frame of its 32nd step the Gustav is out and drives off a cell
/// with sound `0x49`; the portal stands again, the party wonders where it
/// is (dialogue `0xB1`), and the desert loads with the player in control.
const PORTAL_ARRIVAL: &[Op] = &[
    Op::Wait(SETTLE),
    Op::PlayOnce(DESERT_PORTAL, PORTAL_OPENS),
    Op::Sound(PORTAL_SOUND),
    Op::AwaitStepEnd(DESERT_PORTAL, PORTAL_BRINGS_STEP),
    Op::Show(PLAYER),
    Op::Place(PLAYER, DESERT_PORTAL_CELL),
    through(PLAYER, DESERT_ARRIVAL_CELL, PIXEL, 1),
    Op::Sound(PORTAL_ARRIVAL_SOUND),
    Op::AwaitAnimation(DESERT_PORTAL),
    Op::Animate(DESERT_PORTAL, PORTAL_IDLE),
    Op::Dialogue(0xB1),
    Op::Call(&back_to(DESERT, DESERT_ARRIVAL_CELL)),
];

/// The ambush (ROM `0x086674DC`): the Gustav and three runaway Zoids; and
/// Opis's Zoid alone after it (ROM `0x0866752C`).
const AMBUSH_OBJECTS: u32 = 0x0866_74DC;
const OPIS_OBJECTS: u32 = 0x0866_752C;
const AMBUSH_LOAD_CELL: (usize, usize) = (0xA, 5);
const AMBUSH_CELL: (usize, usize) = (0xA, 4);
const RUNAWAY_1: usize = 1;
const RUNAWAY_2: usize = 2;
const RUNAWAY_3: usize = 3;
const OPIS_ZOID: usize = 1;
/// The runaway Zoids' story battle.
const AMBUSH_BATTLE: u8 = 6;

const RUNAWAY_1_CLOSES_IN: &[Op] = &[glide(RUNAWAY_1, (0x140, 0xA0), 2, true), Op::End];
const RUNAWAY_2_CLOSES_IN: &[Op] = &[
    glide(RUNAWAY_2, (0x140, 0xC0), 2, true),
    glide(RUNAWAY_2, (0x120, 0xC0), 2, true),
    glide(RUNAWAY_2, (0x120, 0x80), 2, true),
    Op::Face(RUNAWAY_2, Direction::Right),
    Op::End,
];

/// The runaway Zoids (task at `0x080152A4`, helpers `0x08015390` and
/// `0x080153B0`): the party senses something (dialogue `0xBE`), three
/// Zoids surround the Gustav (`0xBF`), and the fight follows from the
/// field's hook.
const AMBUSH: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Dialogue(0xBE),
    Op::Spawn(MAP_TASK + 1, RUNAWAY_1_CLOSES_IN),
    Op::Spawn(MAP_TASK + 2, RUNAWAY_2_CLOSES_IN),
    glide(RUNAWAY_3, (0x140, 0xE0), 2, true),
    glide(RUNAWAY_3, (0x160, 0xE0), 2, true),
    glide(RUNAWAY_3, (0x160, 0x80), 2, true),
    Op::Face(RUNAWAY_3, Direction::Left),
    Op::Dialogue(0xBF),
    Op::Spawn(FIELD_HOOK, AMBUSH_FIGHT),
    Op::End,
];

const AMBUSH_BATTLE_OPS: [Op; 4] = story_battle(AMBUSH_BATTLE);

/// The ambush's battle (the hook at `0x08015338`): won, the desert loads
/// again where it was fought.
const AMBUSH_FIGHT: &[Op] = &[
    Op::Call(&AMBUSH_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::Flag(AMBUSH_BEATEN, true),
            Op::Warp {
                map: DESERT,
                cell: AMBUSH_CELL,
                facing: None,
            },
            Op::FadeInHoldingSlow,
        ],
    },
    Op::End,
];

/// Opis shows himself (task at `0x08015400`): the party wonders whether
/// that was all (dialogue `0xC0`); to his song Opis drives up and boasts
/// of his runaway Zoids (`0xC1`), then drives off toward the Mount Ossa
/// fortress (`0xC2`).
const OPIS_TAUNT: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Dialogue(0xC0),
    Op::RestartMusic(OPIS_MUSIC),
    glide(OPIS_ZOID, (0x140, 0xA0), 2, true),
    Op::Dialogue(0xC1),
    glide(OPIS_ZOID, (0x140, 0x100), 2, true),
    Op::Dialogue(0xC2),
    Op::Call(&back_to(DESERT, AMBUSH_CELL)),
];

/// Amplifier `n` stays off the map unless the amplifiers are located and
/// it has not been destroyed.
macro_rules! amplifier_stands {
    ($n:expr) => {
        Op::IfFlags {
            all: &[AMPLIFIERS_FOUND],
            none: &[AMPLIFIERS_DESTROYED[$n]],
            then: &[],
            otherwise: &[Op::Place($n + 1, OFF_THE_MAP)],
        }
    };
}

/// The desert's field (`0x0801426C`): through the portal the first time;
/// the runaway Zoids the first time out of the town; Opis once they are
/// beaten. Otherwise, until the amplifiers are located, the rare-hertz
/// zone is watched, and the amplifiers stand once they are located until
/// each is destroyed.
pub(super) const DESERT_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CHAPTER_OPENED],
    none: &[DESERT_REACHED],
    then: &[
        Op::LoadMap {
            map: DESERT,
            player: DESERT_ARRIVAL_CELL,
            objects: PORTAL_ARRIVAL_OBJECTS,
            count: 2,
        },
        Op::Place(PLAYER, OFF_THE_MAP),
        Op::Control(false),
        Op::Spawn(MAP_TASK, PORTAL_ARRIVAL),
        Op::Flag(DESERT_REACHED, true),
    ],
    otherwise: &[Op::IfFlags {
        all: &[TOWN_VISITED],
        none: &[AMBUSH_BEATEN],
        then: &[
            Op::Flag(TOWN_VISITED, false),
            Op::LoadMap {
                map: DESERT,
                player: AMBUSH_LOAD_CELL,
                objects: AMBUSH_OBJECTS,
                count: 4,
            },
            Op::RestartMusic(DANGER_MUSIC),
            Op::Place(PLAYER, AMBUSH_CELL),
            Op::Control(false),
            Op::Spawn(MAP_TASK, AMBUSH),
        ],
        otherwise: &[Op::IfFlags {
            all: &[AMBUSH_BEATEN],
            none: &[OPIS_MET],
            then: &[
                Op::LoadMap {
                    map: DESERT,
                    player: AMBUSH_LOAD_CELL,
                    objects: OPIS_OBJECTS,
                    count: 2,
                },
                Op::Place(PLAYER, AMBUSH_CELL),
                Op::Control(false),
                Op::Spawn(MAP_TASK, OPIS_TAUNT),
                Op::Flag(OPIS_MET, true),
            ],
            otherwise: &[
                Op::IfFlags {
                    all: &[DESERT_REACHED],
                    none: &[ZONE_UNWATCHED, AMPLIFIERS_FOUND],
                    then: &[Op::Spawn(FIELD_HOOK, ZONE_WATCH)],
                    otherwise: &[],
                },
                amplifier_stands!(0),
                amplifier_stands!(1),
                amplifier_stands!(2),
                amplifier_stands!(3),
            ],
        }],
    }],
}];

/// The desert's four amplifiers (objects 1 to 4, sprite `0xF9`), where
/// they stand, and the explosion that takes each (object 5).
const AMPLIFIER_CELLS: [(usize, usize); 4] = [(7, 10), (18, 10), (5, 17), (2, 12)];
const EXPLOSION: usize = 5;

/// The rare-hertz zone's watch (the desert's hook, `0x080144DC`, run every
/// frame; the tasks it starts clear the hook while they run and set it
/// again as they end). On a cell of the zone (`0x08014480`): before the
/// command room has sent the party, it is turned back; once sent, the
/// first time it is told it has reached the zone; while measuring, the
/// danger song plays and once more than two roaming battles have been won
/// in a row there, the measurements are in. Off the zone the count starts
/// again and the map's song comes back.
const ZONE_WATCH: &[Op] = &[Op::Loop(&[
    Op::IfTask {
        slot: MAP_TASK,
        then: &[],
        otherwise: ZONE_CHECK,
    },
    Op::Wait(1),
])];

const ZONE_CHECK: &[Op] = &[Op::IfPlayerOn {
    bits: RARE_HERTZ,
    then: &[Op::IfFlags {
        all: &[MISSION_GIVEN],
        none: &[ZONE_REACHED],
        then: &[
            Op::Control(false),
            Op::Spawn(MAP_TASK, ZONE_EDGE),
            Op::ForgetBattlesWon,
            Op::Flag(ZONE_REACHED, true),
        ],
        otherwise: &[Op::IfFlags {
            all: &[ZONE_REACHED],
            none: &[AMPLIFIERS_FOUND],
            then: &[
                Op::IfMusic {
                    song: DANGER_MUSIC,
                    then: &[],
                    otherwise: &[Op::RestartMusic(DANGER_MUSIC)],
                },
                Op::IfBattlesWon {
                    more_than: ZONE_BATTLES,
                    then: &[
                        Op::Control(false),
                        Op::Spawn(MAP_TASK, MEASURED),
                        Op::Flag(AMPLIFIERS_FOUND, true),
                        Op::End,
                    ],
                    otherwise: &[],
                },
            ],
            otherwise: &[Op::Control(false), Op::Spawn(MAP_TASK, TURNED_BACK_TASK)],
        }],
    }],
    otherwise: &[
        Op::ForgetBattlesWon,
        Op::IfMusic {
            song: DANGER_MUSIC,
            then: &[Op::RestartMapMusic],
            otherwise: &[],
        },
    ],
}];

/// Turned back at the zone (task at `0x08015118`): once the step is over,
/// the first time to the danger song Regina warns of the rare hertz
/// (dialogue `0xB2`), later a shorter warning (`0xB3`); the Gustav backs
/// off a cell at two pixels a frame, and the player is in control again
/// (`0x080144BC`).
const TURNED_BACK_TASK: &[Op] = &[
    Op::AwaitArrival(PLAYER),
    Op::IfFlags {
        all: &[],
        none: &[TURNED_BACK],
        then: &[
            Op::Flag(TURNED_BACK, true),
            Op::RestartMusic(DANGER_MUSIC),
            Op::Dialogue(0xB2),
            Op::RestartMapMusic,
        ],
        otherwise: &[Op::Dialogue(0xB3)],
    },
    Op::StepBack {
        speed: 2 * PIXEL,
        shift: 2,
    },
    Op::AwaitArrival(PLAYER),
    Op::Control(true),
    Op::End,
];

/// At the zone's edge (task at `0x08015968`): once the step is over, Jack
/// says the zone starts here (dialogue `0xD4`), and the player is in
/// control again.
const ZONE_EDGE: &[Op] = &[
    Op::AwaitArrival(PLAYER),
    Op::Dialogue(0xD4),
    Op::Control(true),
    Op::End,
];

/// The command room once the measurements are in (ROM `0x08667824`), and
/// the fortress's wing where Irvine, Moonbay and Zeke turn up (ROM
/// `0x086678C4`).
const MEASURED_ROOM_OBJECTS: u32 = 0x0866_7824;
const WING_OBJECTS: u32 = 0x0866_78C4;
const WING_DR_D: usize = 4;
const WING_IRVINE: usize = 5;
const WING_MOONBAY: usize = 6;
const WING_ZEKE: usize = 7;
const WING_FORD: usize = 8;
const WING_HERMAN: usize = 9;
const WING_OCONNELL: usize = 10;
/// The lists of characters who join once the measurements are in (Irvine,
/// Moonbay and Zeke).
const IRVINE_AND_MOONBAY: [u8; 3] = [4, 3, 5];
/// Where the scenes back in the desert leave the Gustav.
const FORTRESS_FOOT: (usize, usize) = (0x11, 0x15);

const WING_FORD_COMES: &[Op] = &[glide16(WING_FORD, (0xB0, 0), 1, true), Op::End];
const WING_HERMAN_COMES: &[Op] = &[glide16(WING_HERMAN, (0xA0, 0), 1, true), Op::End];
const WING_OCONNELL_COMES: &[Op] = &[glide16(WING_OCONNELL, (0x90, 0), 1, true), Op::End];

/// The measurements are in (task at `0x080159B4`; helpers `0x08015B98`,
/// `0x08015BB4`, `0x08015BD0`): Jack calls the party back (dialogue
/// `0xD5`); in the command room Dr. D finds five amplifiers (`0xD6`); in
/// the wing Irvine and Moonbay burst in after Van (`0xD7`, to the rescue's
/// song), the officers want no more civilians (`0xD8`), they file out, and
/// Irvine, Moonbay and Zeke join the party.
const MEASURED: &[Op] = &[
    Op::Wait(SETTLE),
    Op::AwaitArrival(PLAYER),
    Op::Dialogue(0xD5),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: COMMAND_ROOM,
        player: (8, 5),
        objects: MEASURED_ROOM_OBJECTS,
        count: 8,
    },
    Op::Place(PLAYER, (6, 5)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xD6),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: FORTRESS_WING,
        player: (0xD, 1),
        objects: WING_OBJECTS,
        count: 11,
    },
    Op::Place(PLAYER, (5, 1)),
    Op::Call(SCENE_BRIGHTEN),
    glide16(PLAYER, (0xC0, 0x10), 1, false),
    Op::Spawn(MAP_TASK + 1, WING_FORD_COMES),
    Op::Spawn(MAP_TASK + 2, WING_HERMAN_COMES),
    Op::Spawn(MAP_TASK + 3, WING_OCONNELL_COMES),
    glide16(WING_DR_D, (0xC0, 0), 1, true),
    Op::RestartMusic(RESCUE_MUSIC),
    Op::Dialogue(0xD7),
    Op::RestartMapMusic,
    Op::Face(WING_IRVINE, Direction::Left),
    Op::Face(WING_MOONBAY, Direction::Left),
    Op::Face(WING_ZEKE, Direction::Left),
    Op::Face(WING_FORD, Direction::Down),
    Op::Face(WING_HERMAN, Direction::Down),
    Op::Face(WING_OCONNELL, Direction::Down),
    Op::Dialogue(0xD8),
    glide16(WING_FORD, (0xB0, 0x10), 1, true),
    glide16(WING_FORD, (0xC0, 0x10), 1, true),
    Op::Place(WING_FORD, OFF_THE_MAP),
    glide16(WING_HERMAN, (0xC0, 0x10), 1, true),
    Op::Place(WING_HERMAN, OFF_THE_MAP),
    glide16(WING_OCONNELL, (0xC0, 0x10), 1, true),
    Op::Place(WING_OCONNELL, OFF_THE_MAP),
    Op::Join(IRVINE_AND_MOONBAY[0]),
    Op::Join(IRVINE_AND_MOONBAY[1]),
    Op::Join(IRVINE_AND_MOONBAY[2]),
    Op::Call(&back_to(DESERT, FORTRESS_FOOT)),
];

/// The flicker of a destroyed amplifier (`0x08016BC8`): off and back on
/// for eight frames each, four times.
const fn flicker(actor: usize, cell: (usize, usize)) -> [Op; 4] {
    [
        Op::Place(actor, OFF_THE_MAP),
        Op::Wait(8),
        Op::Place(actor, cell),
        Op::Wait(8),
    ]
}

/// An object blown up (`0x08016CA8` and `0x08016BC8`): the explosion over
/// it plays once with its sound, then the object flickers and is gone.
const fn blown_up(actor: usize, cell: (usize, usize), flicker: &'static [Op]) -> [Op; 7] {
    [
        Op::Place(EXPLOSION, cell),
        Op::PlayOnce(EXPLOSION, 0),
        Op::Sound(BLAST_SOUND),
        Op::AwaitAnimation(EXPLOSION),
        Op::Place(EXPLOSION, OFF_THE_MAP),
        Op::Repeat(4, flicker),
        Op::Place(actor, OFF_THE_MAP),
    ]
}

const FLICKERS: [[Op; 4]; 4] = [
    flicker(1, AMPLIFIER_CELLS[0]),
    flicker(2, AMPLIFIER_CELLS[1]),
    flicker(3, AMPLIFIER_CELLS[2]),
    flicker(4, AMPLIFIER_CELLS[3]),
];

const BLASTS: [[Op; 7]; 4] = [
    blown_up(1, AMPLIFIER_CELLS[0], &FLICKERS[0]),
    blown_up(2, AMPLIFIER_CELLS[1], &FLICKERS[1]),
    blown_up(3, AMPLIFIER_CELLS[2], &FLICKERS[2]),
    blown_up(4, AMPLIFIER_CELLS[3], &FLICKERS[3]),
];

/// An amplifier destroyed (tasks at `0x08016B88` on, `0x08016C38`): the
/// officer's order (dialogue `0xD9`), the blast, and the player in control
/// again.
const fn destroyed(n: usize) -> [Op; 4] {
    [
        Op::Dialogue(0xD9),
        Op::Call(&BLASTS[n]),
        Op::Control(true),
        Op::End,
    ]
}

const DESTROYED: [[Op; 4]; 4] = [destroyed(0), destroyed(1), destroyed(2), destroyed(3)];

/// Speaking to amplifier `n` (`0x08016A58` on): once they are located, the
/// player stops and it is destroyed.
macro_rules! amplifier_talk {
    ($n:expr) => {
        &[Op::IfFlags {
            all: &[AMPLIFIERS_FOUND],
            none: &[],
            then: &[
                Op::Control(false),
                Op::Spawn(MAP_TASK, &DESTROYED[$n]),
                Op::Flag(AMPLIFIERS_DESTROYED[$n], true),
            ],
            otherwise: &[],
        }]
    };
}

const AMPLIFIER_TALKS: [&[Op]; 4] = [
    amplifier_talk!(0),
    amplifier_talk!(1),
    amplifier_talk!(2),
    amplifier_talk!(3),
];

/// The town (`0x08014628`): entering it sets the flag the runaway Zoids
/// wait for.
pub(super) const TOWN_ARRIVAL: &[Op] = &[Op::Flag(TOWN_VISITED, true)];

/// The fortress's gate with Dr. D, the party off the map and a guard (ROM
/// `0x0866757C`).
const GATE_OBJECTS: u32 = 0x0866_757C;
const GATE_CELL: (usize, usize) = (0xE, 0x1F);
const GATE_DR_D: usize = 4;
const GATE_GUARD: usize = 5;

/// Turned away at the fortress (task at `0x0801574C`): the Gustav drives
/// up, a guard steps out and stops it (dialogue `0xC3`).
const GATE_REFUSED: &[Op] = &[
    Op::Wait(SETTLE),
    glide16(PLAYER, (0xE0, 0x1C0), 1, false),
    glide16(GATE_GUARD, (0xE0, 0x1A0), 1, true),
    Op::Dialogue(0xC3),
    Op::Call(&back_to(DESERT, FORTRESS_FOOT)),
];

/// Dr. D at the gate (task at `0x080154FC`): the guard stops the party
/// again (dialogue `0xC6`); the others step out and wonder what to do
/// (`0xC7`); Dr. D comes up to his song and pinches Regina (`0xC8`), the
/// party knows him (`0xC9`), he has the guard let them in (`0xCA`) and asks
/// to be taken to the commander (`0xCB`); they go in.
const DR_D_AT_THE_GATE: &[Op] = &[
    Op::Wait(SETTLE),
    glide16(PLAYER, (0xE0, 0x1C0), 1, false),
    glide16(GATE_GUARD, (0xE0, 0x1A0), 1, true),
    Op::Dialogue(0xC6),
    Op::Place(REGINA, (0xE, 0x1C)),
    glide16(REGINA, (0xD0, 0x1C0), 1, true),
    Op::Face(REGINA, Direction::Right),
    Op::Place(JACK, (0xE, 0x1C)),
    glide16(JACK, (0xF0, 0x1D0), 1, true),
    Op::Face(JACK, Direction::Up),
    Op::Place(EARTH, (0xE, 0x1C)),
    glide16(EARTH, (0xF0, 0x1C0), 1, true),
    Op::Face(EARTH, Direction::Left),
    Op::Dialogue(0xC7),
    glide16(GATE_DR_D, (0xC0, 0x1C0), 1, true),
    Op::RestartMusic(DR_D_MUSIC),
    Op::Dialogue(0xC8),
    Op::Face(PLAYER, Direction::Left),
    Op::Face(REGINA, Direction::Left),
    Op::Face(JACK, Direction::Left),
    Op::Face(EARTH, Direction::Left),
    Op::Wait(30),
    glide16(GATE_DR_D, (0xB0, 0x1C0), 1, true),
    Op::Face(GATE_DR_D, Direction::Right),
    Op::Dialogue(0xC9),
    glide16(GATE_DR_D, (0xB0, 0x1A0), 1, true),
    glide16(GATE_DR_D, (0xD0, 0x1A0), 1, true),
    Op::Face(GATE_GUARD, Direction::Left),
    Op::Face(PLAYER, Direction::Up),
    Op::Face(REGINA, Direction::Up),
    Op::Face(JACK, Direction::Up),
    Op::Face(EARTH, Direction::Up),
    Op::Dialogue(0xCA),
    glide16(GATE_GUARD, (0xE0, 0x150), 1, true),
    Op::Place(GATE_GUARD, OFF_THE_MAP),
    glide16(GATE_DR_D, (0xE0, 0x1A0), 1, true),
    Op::Face(GATE_DR_D, Direction::Down),
    Op::Dialogue(0xCB),
    glide16(REGINA, (0xE0, 0x1C0), 1, true),
    Op::Place(REGINA, OFF_THE_MAP),
    glide16(JACK, (0xE0, 0x1C0), 1, true),
    Op::Place(JACK, OFF_THE_MAP),
    glide16(EARTH, (0xE0, 0x1C0), 1, true),
    Op::Place(EARTH, OFF_THE_MAP),
    glide16(GATE_DR_D, (0xE0, 0x1C0), 1, true),
    Op::Place(GATE_DR_D, OFF_THE_MAP),
    Op::Call(&back_to(FORTRESS_GATE, (0xE, 0x1C))),
];

/// The fortress's gate (`0x080147F8`): until Dr. D has taken the party in,
/// it loads with him and a guard; before Opis has been met the guard sends
/// the party away, after it Dr. D's scene runs and a beaten party is
/// taken to the fortress's lab (return point 10).
pub(super) const GATE_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CHAPTER_OPENED],
    none: &[FORTRESS_ENTERED],
    then: &[
        Op::LoadMap {
            map: FORTRESS_GATE,
            player: GATE_CELL,
            objects: GATE_OBJECTS,
            count: 6,
        },
        Op::Control(false),
        Op::IfFlags {
            all: &[OPIS_MET],
            none: &[],
            then: &[
                Op::Spawn(MAP_TASK, DR_D_AT_THE_GATE),
                Op::Flag(FORTRESS_ENTERED, true),
                Op::ReturnPoint(FORTRESS_LAB_RETURN),
            ],
            otherwise: &[Op::Spawn(MAP_TASK, GATE_REFUSED)],
        },
    ],
    otherwise: &[],
}];

/// The return point in the fortress's lab (map 62).
const FORTRESS_LAB_RETURN: u8 = 10;

/// The command room the first time (ROM `0x086675F4`): the prince, the
/// party off the map, Dr. D, Colonel Ford, and Captain Herman and
/// Lieutenant O'Connell off the map.
const COMMAND_ROOM_OBJECTS: u32 = 0x0866_75F4;
const ROOM_DR_D: usize = 4;
const ROOM_HERMAN: usize = 6;
const ROOM_OCONNELL: usize = 7;

const PRINCE_FOLLOWS: &[Op] = &[glide16(PLAYER, (0x80, 0x70), 1, true), Op::End];
const HERMAN_COMES_IN: &[Op] = &[
    Op::Place(ROOM_HERMAN, (4, 8)),
    glide16(ROOM_HERMAN, (0x40, 0x10), 1, true),
    glide16(ROOM_HERMAN, (0x50, 0x10), 1, true),
    Op::Face(ROOM_HERMAN, Direction::Down),
    Op::End,
];

/// The command room (task at `0x080157C0`; helpers `0x0801590C`,
/// `0x08015928`): Dr. D walks up to Colonel Ford, who explains the Kronos
/// fort's plight (dialogues `0xCC`, `0xCD`); Herman and O'Connell come in
/// and want the new Gojulas (`0xCE`); after a black pause everyone stands
/// in place and Dr. D sends the party to measure the zone (`0xCF`).
const COMMAND_ROOM_TASK: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Spawn(MAP_TASK + 1, PRINCE_FOLLOWS),
    glide16(ROOM_DR_D, (0x80, 0x50), 1, true),
    Op::Dialogue(0xCC),
    glide16(PLAYER, (0x60, 0x70), 1, false),
    glide16(PLAYER, (0x60, 0x50), 1, false),
    Op::Dialogue(0xCD),
    Op::Spawn(MAP_TASK + 1, HERMAN_COMES_IN),
    Op::Wait(0x10),
    Op::Place(ROOM_OCONNELL, (4, 8)),
    glide16(ROOM_OCONNELL, (0x40, 0x10), 1, true),
    Op::Face(ROOM_OCONNELL, Direction::Down),
    Op::Dialogue(0xCE),
    Op::Call(SCENE_DARKEN),
    Op::Wait(0x78),
    Op::Place(PLAYER, (6, 5)),
    Op::Face(PLAYER, Direction::Up),
    Op::Place(REGINA, (5, 5)),
    Op::Face(REGINA, Direction::Up),
    Op::Place(JACK, (7, 6)),
    Op::Face(JACK, Direction::Up),
    Op::Place(EARTH, (6, 6)),
    Op::Face(EARTH, Direction::Up),
    Op::Place(ROOM_HERMAN, (5, 1)),
    Op::Face(ROOM_HERMAN, Direction::Down),
    Op::Place(ROOM_OCONNELL, (4, 1)),
    Op::Face(ROOM_OCONNELL, Direction::Down),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xCF),
    Op::Call(&back_to(COMMAND_ROOM, (8, 7))),
];

/// The command room (`0x08014918`): the first time it loads with the
/// officers and the briefing runs. Its officers leave once the amplifiers
/// are located; once the core is destroyed, Colonel Ford and Colonel
/// Krueger stand at their places.
pub(super) const COMMAND_ROOM_ARRIVAL: &[Op] = &[
    Op::IfFlags {
        all: &[],
        none: &[MISSION_GIVEN],
        then: &[
            Op::LoadMap {
                map: COMMAND_ROOM,
                player: (8, 0x12),
                objects: COMMAND_ROOM_OBJECTS,
                count: 8,
            },
            Op::Control(false),
            Op::Spawn(MAP_TASK, COMMAND_ROOM_TASK),
            Op::Flag(MISSION_GIVEN, true),
        ],
        otherwise: &[],
    },
    Op::IfFlags {
        all: &[AMPLIFIERS_FOUND],
        none: &[],
        then: &[
            Op::Place(2, OFF_THE_MAP),
            Op::Place(3, OFF_THE_MAP),
            Op::Place(4, OFF_THE_MAP),
        ],
        otherwise: &[],
    },
    Op::IfFlags {
        all: &[CORE_DESTROYED],
        none: &[],
        then: &[Op::Place(1, (7, 1)), Op::Place(5, (9, 5))],
        otherwise: &[],
    },
];

/// Dr. D in the command room (`0x080149C0`): a line for each stage of the
/// chapter.
pub(super) const DR_D_TALK: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[AMPLIFIERS_FOUND],
    then: &[Op::Dialogue(0x2D7)],
    otherwise: &[Op::IfFlags {
        all: &[],
        none: &[KRONOS_REACHED],
        then: &[Op::Dialogue(0x2D8)],
        otherwise: &[Op::IfFlags {
            all: &[],
            none: &[CORE_DESTROYED],
            then: &[Op::Dialogue(0x2D9)],
            otherwise: &[Op::IfFlags {
                all: &[],
                none: &[LASER_FOUND],
                then: &[Op::Dialogue(0x2DA)],
                otherwise: &[Op::Dialogue(0x2DB)],
            }],
        }],
    }],
}];

/// The fortress's wing (`0x080148CC`): Irvine, Moonbay and Zeke wait there
/// from the measurements until the core is destroyed.
pub(super) const WING_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[AMPLIFIERS_FOUND],
    none: &[CORE_DESTROYED],
    then: &[],
    otherwise: &[
        Op::Place(1, OFF_THE_MAP),
        Op::Place(2, OFF_THE_MAP),
        Op::Place(3, OFF_THE_MAP),
    ],
}];

/// The rear entrance (ROM `0x08667694`): the Gustav and a guard.
const REAR_OBJECTS: u32 = 0x0866_7694;

/// The rear entrance's guard (task at `0x08015498`): the first time he
/// sends the party round to the front (dialogue `0xC4`), later he keeps it
/// out (`0xC5`).
const REAR_GUARD: &[Op] = &[
    Op::Wait(SETTLE),
    Op::IfFlags {
        all: &[],
        none: &[REAR_GUARD_MET],
        then: &[Op::Dialogue(0xC4), Op::Flag(REAR_GUARD_MET, true)],
        otherwise: &[Op::Dialogue(0xC5)],
    },
    Op::Call(&back_to(DESERT, (0x13, 0x13))),
];

/// The rear entrance (`0x08014A44`): until the core is destroyed, a guard
/// stands at it.
pub(super) const REAR_ENTRANCE_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[REAR_ENTRANCE_OPEN],
    then: &[
        Op::LoadMap {
            map: REAR_ENTRANCE,
            player: (0x15, 0xD),
            objects: REAR_OBJECTS,
            count: 2,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, REAR_GUARD),
    ],
    otherwise: &[],
}];

/// The fort's other view (ROM `0x086679A0`): the party, Irvine, Moonbay
/// and Zeke off the map, Ford, Herman and O'Connell, and Van, Fiene and
/// Krueger off the map.
const KRONOS_OBJECTS: u32 = 0x0866_79A0;
const KRONOS_VAN: usize = 10;
const KRONOS_FIENE: usize = 11;
const KRONOS_KRUEGER: usize = 12;
const KRONOS_DOOR: (usize, usize) = (0x1E, 0x1B);
/// Krueger's room (ROM `0x08667AA4`).
const KRONOS_ROOM_OBJECTS: u32 = 0x0866_7AA4;
const ROOM_VAN: usize = 7;
const ROOM_FIENE: usize = 8;
const ROOM_KRUEGER: usize = 9;
/// Van and Fiene join; Irvine, Moonbay and Zeke leave.
const VAN_AND_FIENE: [u8; 2] = [6, 7];

const PRINCE_DRIVES_UP: &[Op] = &[glide16(PLAYER, (0x1E0, 0x1D0), 1, false), Op::End];
const REGINA_GOES_IN: &[Op] = &[
    glide16(REGINA, (0x20, 0x20), 1, true),
    Op::Place(REGINA, OFF_THE_MAP),
    Op::End,
];
const EARTH_GOES_IN: &[Op] = &[
    glide16(EARTH, (0x20, 0x20), 1, true),
    Op::Place(EARTH, OFF_THE_MAP),
    Op::End,
];
const VAN_GOES_IN: &[Op] = &[
    glide16(ROOM_VAN, (0x20, 0x10), 1, true),
    glide16(ROOM_VAN, (0x20, 0x20), 1, true),
    Op::Place(ROOM_VAN, OFF_THE_MAP),
    Op::End,
];
const FIENE_GOES_IN: &[Op] = &[
    glide16(ROOM_FIENE, (0x20, 0x10), 1, true),
    glide16(ROOM_FIENE, (0x20, 0x20), 1, true),
    Op::Place(ROOM_FIENE, OFF_THE_MAP),
    Op::End,
];

/// The Kronos fort (task at `0x08015BEC`; helpers `0x08015E34` on): the
/// officers reach it (dialogue `0xDF`); at its door Van turns up (`0xE0`)
/// to Dr. D's song, Fiene with him (`0xE1`), and Colonel Krueger comes out
/// (`0xE2`); in his room he hears who the party is (`0xE3`); everyone goes
/// in, Van and Fiene join, and Irvine, Moonbay and Zeke leave.
const KRONOS: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Dialogue(0xDF),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: KRONOS_VIEW,
        player: (0x1E, 0x1C),
        objects: KRONOS_OBJECTS,
        count: 13,
    },
    Op::Place(PLAYER, (0x1E, 0x1E)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, PRINCE_DRIVES_UP),
    glide16(JACK, (0x1F0, 0x1D0), 1, true),
    glide16(JACK, (0x200, 0x1D0), 1, true),
    Op::Face(JACK, Direction::Up),
    Op::Dialogue(0xE0),
    Op::RestartMusic(DR_D_MUSIC),
    Op::Place(KRONOS_VAN, KRONOS_DOOR),
    glide16(KRONOS_VAN, (0x1E0, 0x1C0), 1, true),
    glide16(KRONOS_VAN, (0x1F0, 0x1C0), 1, true),
    Op::Face(KRONOS_VAN, Direction::Down),
    Op::Place(KRONOS_FIENE, KRONOS_DOOR),
    glide16(KRONOS_FIENE, (0x1E0, 0x1C0), 1, true),
    Op::Dialogue(0xE1),
    Op::Place(KRONOS_KRUEGER, KRONOS_DOOR),
    glide16(KRONOS_FIENE, (0x1D0, 0x1C0), 1, true),
    Op::Face(KRONOS_FIENE, Direction::Down),
    glide16(KRONOS_KRUEGER, (0x1E0, 0x1C0), 1, true),
    Op::Dialogue(0xE2),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: KRONOS_ROOM,
        player: (0xF, 0xE),
        objects: KRONOS_ROOM_OBJECTS,
        count: 10,
    },
    Op::Place(PLAYER, (2, 2)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xE3),
    Op::Spawn(MAP_TASK + 1, REGINA_GOES_IN),
    glide16(EARTH, (0x20, 0x20), 1, true),
    Op::Place(EARTH, OFF_THE_MAP),
    glide16(JACK, (0x20, 0x20), 1, true),
    Op::Place(JACK, OFF_THE_MAP),
    Op::Spawn(MAP_TASK + 2, EARTH_GOES_IN),
    Op::Spawn(MAP_TASK + 1, VAN_GOES_IN),
    Op::Spawn(MAP_TASK + 2, FIENE_GOES_IN),
    glide16(ROOM_KRUEGER, (0x20, 0x10), 1, true),
    glide16(ROOM_KRUEGER, (0x20, 0x20), 1, true),
    Op::Place(ROOM_KRUEGER, OFF_THE_MAP),
    Op::Join(VAN_AND_FIENE[0]),
    Op::Join(VAN_AND_FIENE[1]),
    Op::Leave(IRVINE_AND_MOONBAY[0]),
    Op::Leave(IRVINE_AND_MOONBAY[1]),
    Op::Leave(IRVINE_AND_MOONBAY[2]),
    Op::Call(&back_to(KRONOS_OUTSKIRTS, (0xF, 0xE))),
];

/// Too close to the fort (task at `0x08015F18`): before the amplifiers
/// are located Jack says the rare hertz is too strong (dialogue `0xDD`),
/// after it the officers say the outer amplifiers must go first (`0xDE`).
const FORT_TOO_STRONG: &[Op] = &[
    Op::Wait(SETTLE),
    Op::IfFlags {
        all: &[AMPLIFIERS_FOUND],
        none: &[],
        then: &[Op::Dialogue(0xDE)],
        otherwise: &[Op::Dialogue(0xDD)],
    },
    Op::Call(&back_to(DESERT, (0xA, 0xF))),
];

/// The fort's outskirts (`0x08014654`): with the four amplifiers destroyed
/// the party reaches the fort; before that it is turned back.
pub(super) const KRONOS_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[
        AMPLIFIERS_DESTROYED[0],
        AMPLIFIERS_DESTROYED[1],
        AMPLIFIERS_DESTROYED[2],
        AMPLIFIERS_DESTROYED[3],
    ],
    none: &[KRONOS_REACHED],
    then: &[
        Op::Control(false),
        Op::Spawn(MAP_TASK, KRONOS),
        Op::Flag(KRONOS_REACHED, true),
    ],
    otherwise: &[Op::IfFlags {
        all: &[CHAPTER_OPENED],
        none: &[KRONOS_REACHED],
        then: &[Op::Control(false), Op::Spawn(MAP_TASK, FORT_TOO_STRONG)],
        otherwise: &[],
    }],
}];

/// Krueger's room (`0x0801472C`): the officers stand in it once the core
/// is destroyed.
pub(super) const KRONOS_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CORE_DESTROYED],
    none: &[],
    then: &[],
    otherwise: &[
        Op::Place(1, (0xFFFF, 0xFFFF)),
        Op::Place(2, (0xFFFF, 0xFFFF)),
        Op::Place(3, (0xFFFF, 0xFFFF)),
    ],
}];

/// The fort's heart (`0x08014780`): the amplifier's core stands in it from
/// the fort's reaching until it is destroyed.
pub(super) const KRONOS_HEART_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[KRONOS_REACHED],
    none: &[CORE_DESTROYED],
    then: &[],
    otherwise: &[Op::Place(1, OFF_THE_MAP)],
}];

/// The core's scene (ROM `0x08667B6C`): the Gustav, the core, three
/// runaway Zoids and the explosion.
const CORE_OBJECTS: u32 = 0x0866_7B6C;
const CORE: usize = 1;
const CORE_CELL: (usize, usize) = (21, 7);
const CORE_RUNAWAY_1: usize = 2;
const CORE_RUNAWAY_2: usize = 3;
const CORE_RUNAWAY_3: usize = 4;
/// Krueger's room after it (ROM `0x08667BE4`), and the command room (ROM
/// `0x08667CAC`).
const AFTER_CORE_ROOM_OBJECTS: u32 = 0x0866_7BE4;
const AFTER_CORE_COMMAND_OBJECTS: u32 = 0x0866_7CAC;
const COMMAND_IRVINE: usize = 5;
const COMMAND_VAN: usize = 7;
/// The list of the characters who join once the core is destroyed, and of
/// those who leave.
const CORE_JOINS: u8 = 8;
const CORE_LEAVES: u8 = 7;

const CORE_FLICKER: [Op; 4] = flicker(CORE, CORE_CELL);
const CORE_BLAST: [Op; 7] = blown_up(CORE, CORE_CELL, &CORE_FLICKER);
const RUNAWAY_RUNS_ON: &[Op] = &[glide(CORE_RUNAWAY_3, (0x320, 0xE0), 2, true), Op::End];
const REGINA_LEAVES_THE_ROOM: &[Op] = &[
    glide16(REGINA, (0x50, 0xC0), 2, true),
    Op::Place(REGINA, OFF_THE_MAP),
    Op::End,
];
const JACK_LEAVES_THE_ROOM: &[Op] = &[
    glide16(JACK, (0x70, 0xC0), 2, true),
    Op::Place(JACK, OFF_THE_MAP),
    Op::End,
];
const EARTH_LEAVES_THE_ROOM: &[Op] = &[
    glide16(EARTH, (0x60, 0xC0), 2, true),
    Op::Place(EARTH, OFF_THE_MAP),
    Op::End,
];

/// The party leaves the command room (helpers `0x08016128`, `0x08016150`,
/// `0x08016178`), the prince last.
const LEAVE_THE_COMMAND_ROOM: &[Op] = &[
    Op::Spawn(MAP_TASK + 1, REGINA_LEAVES_THE_ROOM),
    Op::Spawn(MAP_TASK + 2, JACK_LEAVES_THE_ROOM),
    Op::Spawn(MAP_TASK + 3, EARTH_LEAVES_THE_ROOM),
    glide16(PLAYER, (0x60, 0xC0), 2, false),
    Op::Place(PLAYER, OFF_THE_MAP),
];

/// The amplifier's core (task at `0x08015F74`; helper `0x080161A0`): to the
/// danger song the party finds it (dialogue `0xE7`) and blows it up, but
/// the runaway Zoids run on (`0xE8`); in Krueger's room the officers
/// despair (`0xE9`); in the command room Dr. D sees the enemy's real aim,
/// the volcano (`0xEA`), and the party sets off, the lists changing.
const CORE_TASK: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: KRONOS_HEART,
        player: (0x16, 7),
        objects: CORE_OBJECTS,
        count: 6,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xE7),
    Op::Call(&CORE_BLAST),
    glide(CORE_RUNAWAY_2, (0x300, 0xE0), 1, true),
    Op::Spawn(MAP_TASK + 1, RUNAWAY_RUNS_ON),
    glide(CORE_RUNAWAY_1, (0x300, 0xC0), 1, true),
    Op::Dialogue(0xE8),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: KRONOS_ROOM,
        player: (0xF, 0xE),
        objects: AFTER_CORE_ROOM_OBJECTS,
        count: 10,
    },
    Op::Place(PLAYER, (2, 2)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xE9),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: COMMAND_ROOM,
        player: (8, 5),
        objects: AFTER_CORE_COMMAND_OBJECTS,
        count: 8,
    },
    Op::Place(PLAYER, (6, 5)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xEA),
    glide16(COMMAND_IRVINE, (0x80, 0xC0), 2, true),
    Op::Place(COMMAND_IRVINE, OFF_THE_MAP),
    glide16(COMMAND_VAN, (0xA0, 0xC0), 2, true),
    Op::Place(COMMAND_VAN, OFF_THE_MAP),
    Op::Call(LEAVE_THE_COMMAND_ROOM),
    Op::Join(CORE_JOINS),
    Op::Leave(CORE_LEAVES),
    Op::Call(&back_to(FORTRESS_HALL, (0x11, 1))),
];

/// Speaking to the core (`0x080147B0`): the player stops, the core's scene
/// runs, and the rear entrance opens.
pub(super) const CORE_TALK: &[Op] = &[
    Op::Control(false),
    Op::Spawn(MAP_TASK, CORE_TASK),
    Op::Flag(REAR_ENTRANCE_OPEN, true),
    Op::Flag(CORE_DESTROYED, true),
];

/// The tunnels as the laser is found (ROM `0x08667D4C`), and the command
/// room after (ROM `0x08667D74`).
const LASER_OBJECTS: u32 = 0x0866_7D4C;
const LASER_COMMAND_OBJECTS: u32 = 0x0866_7D74;
/// The laser's container (object 1 of map 71) and its opened look.
const CONTAINER: usize = 1;
const CONTAINER_OPENED: usize = 1;

/// The laser found (task at `0x080161C0`): to its song the party looks at
/// the rock-boring laser (dialogue `0xED`) and brings it to Dr. D (`0xEE`).
const LASER_TASK: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: TUNNELS,
        player: (0x26, 0xC),
        objects: LASER_OBJECTS,
        count: 2,
    },
    Op::RestartMusic(LASER_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xED),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: COMMAND_ROOM,
        player: (8, 5),
        objects: LASER_COMMAND_OBJECTS,
        count: 5,
    },
    Op::Place(PLAYER, (6, 5)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xEE),
    Op::Call(&back_to(DESERT, FORTRESS_FOOT)),
];

/// Speaking to the laser's container (`0x08014AD4`): the first time the
/// player stops, it opens with the container's sound, and the laser's
/// scene runs.
pub(super) const CONTAINER_TALK: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[LASER_FOUND],
    then: &[
        Op::Control(false),
        Op::Animate(CONTAINER, CONTAINER_OPENED),
        Op::Sound(CONTAINER_SOUND),
        Op::Wait(1),
        Op::Control(false),
        Op::Spawn(MAP_TASK, LASER_TASK),
        Op::Flag(LASER_FOUND, true),
    ],
    otherwise: &[],
}];

/// The tunnels (`0x08014AB4`): once the laser is found its container
/// stands open.
pub(super) const TUNNELS_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[LASER_FOUND],
    none: &[],
    then: &[Op::Animate(CONTAINER, CONTAINER_OPENED)],
    otherwise: &[],
}];

/// The crater's path while Opis waits (ROM `0x086676BC`): the Gustav where
/// it stands, Van's and Irvine's Zoids off the map, and Opis's Zoid.
const CRATER_OBJECTS: u32 = 0x0866_76BC;
const VAN_ZOID: usize = 1;
const IRVINE_ZOID: usize = 2;
const CRATER_OPIS: usize = 3;
/// The rock that closes the path until the laser is found.
const ROCK: usize = 1;
/// The lists of the characters who go to the crater (Van and Irvine's).
const TO_THE_CRATER: [u8; 2] = [8, 6];
/// The fight with Opis.
const OPIS_BATTLE: u8 = 7;

const IRVINE_DRIVES_OFF: &[Op] = &[
    Op::Place(IRVINE_ZOID, (0x20, 0x14)),
    glide(IRVINE_ZOID, (0x400, 0x2A0), 2, true),
    glide(IRVINE_ZOID, (0x3C0, 0x2A0), 2, true),
    glide(IRVINE_ZOID, (0x3C0, 0x2C0), 2, true),
    glide(IRVINE_ZOID, (0x380, 0x2C0), 2, true),
    Op::Place(IRVINE_ZOID, OFF_THE_MAP),
    Op::End,
];

/// Opis at the crater (the hook `0x08014BE0` waits for the Gustav at
/// pixel (`0x440`, `0x280`); task at `0x08016264`, helper `0x08016928`): to
/// the rescue's song the Gustav drives up to Opis (dialogue `0xEF`); the
/// first time Jack sends Van and Irvine on to the crater (`0xF0`), who
/// drive off to the chase's song, and apologizes (`0xF1`); the fight
/// follows from the field's hook.
const CRATER_LOOKOUT: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x440, 0x440),
        y: (0x280, 0x280),
    },
    Op::Control(false),
    Op::RestartMusic(RESCUE_MUSIC),
    glide(PLAYER, (0x400, 0x280), 1, true),
    Op::Dialogue(0xEF),
    Op::IfFlags {
        all: &[],
        none: &[SENT_TO_THE_CRATER],
        then: &[
            Op::Dialogue(0xF0),
            Op::RestartMusic(CHASE_MUSIC),
            Op::Place(VAN_ZOID, (0x20, 0x14)),
            glide(VAN_ZOID, (0x400, 0x2A0), 2, true),
            glide(VAN_ZOID, (0x3C0, 0x2A0), 2, true),
            Op::Spawn(MAP_TASK + 1, IRVINE_DRIVES_OFF),
            glide(VAN_ZOID, (0x3C0, 0x2C0), 2, true),
            glide(VAN_ZOID, (0x380, 0x2C0), 2, true),
            Op::Place(VAN_ZOID, OFF_THE_MAP),
            Op::Wait(0x5A),
            Op::Dialogue(0xF1),
            Op::Leave(TO_THE_CRATER[0]),
            Op::Leave(TO_THE_CRATER[1]),
            Op::Flag(SENT_TO_THE_CRATER, true),
        ],
        otherwise: &[],
    },
    Op::Spawn(FIELD_HOOK, OPIS_FIGHT),
    Op::End,
];

const OPIS_BATTLE_OPS: [Op; 4] = story_battle(OPIS_BATTLE);

/// The fight with Opis (the hook at `0x080168B0`): won, to the victory
/// song the chapter's end runs.
const OPIS_FIGHT: &[Op] = &[
    Op::Call(&OPIS_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::Flag(OPIS_BEATEN, true),
            Op::RestartMusic(VICTORY_MUSIC),
            Op::Control(false),
            Op::Spawn(MAP_TASK, FINALE),
        ],
    },
    Op::End,
];

/// The crater's path (`0x08014B74`): with the laser found and Opis not yet
/// beaten, it loads with him and his watch starts; the rock is gone once
/// the laser is found.
pub(super) const CRATER_ARRIVAL: &[Op] = &[
    Op::IfFlags {
        all: &[LASER_FOUND],
        none: &[OPIS_BEATEN],
        then: &[
            Op::LoadMap {
                map: CRATER,
                player: HERE,
                objects: CRATER_OBJECTS,
                count: 4,
            },
            Op::Spawn(MAP_TASK, CRATER_LOOKOUT),
        ],
        otherwise: &[],
    },
    Op::IfFlags {
        all: &[LASER_FOUND],
        none: &[],
        then: &[Op::Place(ROCK, OFF_THE_MAP)],
        otherwise: &[],
    },
];

/// The crater once Opis is beaten (ROM `0x08667DD8`): the Gustav off the
/// map, Van's and Irvine's Zoids, the laser's beam and the crater.
const ERUPTION_OBJECTS: u32 = 0x0866_7DD8;
const ERUPTION_VAN: usize = 1;
const ERUPTION_IRVINE: usize = 2;
const BEAM: usize = 3;
const BEAM_CELL: (usize, usize) = (0x10, 0x15);
const ERUPTION_VIEW: (usize, usize) = (0x10, 0x15);
/// The crater (object 4) and its runs: bursting, then erupting.
const CRATER_MOUTH: usize = 4;
const CRATER_BURSTS: usize = 1;
const CRATER_ERUPTS: usize = 2;
/// The command room at the chapter's end (ROM `0x08667E3C`).
const END_COMMAND_OBJECTS: u32 = 0x0866_7E3C;
const END_KRUEGER: usize = 5;
/// The desert (ROM `0x08667EB4`), the throne room (ROM `0x08667EDC`) and
/// the base's portal room (ROM `0x08667F18`) as the party goes home.
const HOMEWARD_OBJECTS: u32 = 0x0866_7EB4;
const THRONE_OBJECTS: u32 = 0x0866_7EDC;
const THRONE_BLOOD: usize = 2;
const PORTAL_ROOM_OBJECTS: u32 = 0x0866_7F18;
const BASE_PORTAL_SOLDIER: usize = 1;
const BASE_PORTAL: usize = 2;
const BASE_PORTAL_CELL: (usize, usize) = (4, 1);

const VAN_DRIVES_UP: &[Op] = &[
    glide(ERUPTION_VAN, (0x220, 0x2E0), 2, true),
    glide(ERUPTION_VAN, (0x220, 0x2A0), 2, true),
    Op::Face(ERUPTION_VAN, Direction::Left),
    Op::End,
];
const IRVINE_ESCAPES: &[Op] = &[
    glide(ERUPTION_IRVINE, (0x200, 0x2E0), 2, true),
    glide(ERUPTION_IRVINE, (0x2A0, 0x2E0), 2, true),
    Op::End,
];
const VAN_ESCAPES: &[Op] = &[
    glide(ERUPTION_VAN, (0x220, 0x2E0), 2, true),
    glide(ERUPTION_VAN, (0x2A0, 0x2E0), 2, true),
    Op::End,
];
const PAN_DOWN: &[Op] = &[Op::Pan(0, PIXEL), Op::Wait(1)];

/// The chapter's end (task at `0x08016378`; helpers `0x080169A8`,
/// `0x080169E8`, `0x08016A20`): Opis flees (dialogues `0xF2`, `0xF3`); at
/// the crater Van fires the laser (`0xF4`), the beam and the eruption's
/// sounds follow and the two drive out (`0xF5`); in the command room
/// Krueger and Dr. D thank the party (`0xF6` to `0xF8`); the Gustav goes
/// through the desert's portal; in the throne room the Emperor sends for
/// Gale (`0xF9`, `0xFA`); the party comes out of the base's portal and
/// is welcomed home (`0xFB`); chapter 4 follows.
const FINALE: &[Op] = &[
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xF2),
    glide(CRATER_OPIS, (0x3E0, 0x2A0), 2, true),
    glide(CRATER_OPIS, (0x440, 0x2A0), 2, true),
    glide(CRATER_OPIS, (0x440, 0x200), 2, true),
    Op::Place(CRATER_OPIS, OFF_THE_MAP),
    Op::Dialogue(0xF3),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: CRATER,
        player: ERUPTION_VIEW,
        objects: ERUPTION_OBJECTS,
        count: 5,
    },
    Op::RestartMusic(VICTORY_MUSIC),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, VAN_DRIVES_UP),
    glide(ERUPTION_IRVINE, (0x220, 0x2E0), 2, true),
    glide(ERUPTION_IRVINE, (0x200, 0x2A0), 2, true),
    Op::Dialogue(0xF4),
    Op::Sound(PORTAL_SOUND),
    Op::Place(BEAM, BEAM_CELL),
    Op::PlayOnce(BEAM, 0),
    Op::AwaitAnimation(BEAM),
    Op::Place(BEAM, OFF_THE_MAP),
    Op::AwaitSoundEnd(PORTAL_SOUND),
    Op::Sound(ERUPTION_SOUND),
    Op::PlayOnce(CRATER_MOUTH, CRATER_BURSTS),
    Op::AwaitAnimation(CRATER_MOUTH),
    Op::PlayOnce(CRATER_MOUTH, CRATER_ERUPTS),
    Op::AwaitSoundEnd(ERUPTION_SOUND),
    Op::Dialogue(0xF5),
    Op::Spawn(MAP_TASK + 1, IRVINE_ESCAPES),
    Op::Spawn(MAP_TASK + 2, VAN_ESCAPES),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::Stop(MAP_TASK + 2),
    Op::LoadMap {
        map: COMMAND_ROOM,
        player: (8, 5),
        objects: END_COMMAND_OBJECTS,
        count: 6,
    },
    Op::Place(PLAYER, (6, 5)),
    Op::RestartMusic(DR_D_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xF6),
    Op::Face(END_KRUEGER, Direction::Left),
    Op::Dialogue(0xF7),
    Op::Call(LEAVE_THE_COMMAND_ROOM),
    Op::Face(END_KRUEGER, Direction::Up),
    Op::Dialogue(0xF8),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: DESERT,
        player: (0xD, 3),
        objects: HOMEWARD_OBJECTS,
        count: 2,
    },
    Op::Call(SCENE_BRIGHTEN),
    glide(PLAYER, (0x1A0, 0x40), 1, false),
    through(PLAYER, DESERT_PORTAL_CELL, PIXEL, 1),
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
        map: THRONE_ROOM,
        player: (8, 2),
        objects: THRONE_OBJECTS,
        count: 3,
    },
    Op::Place(PLAYER, (0xFFFF, 0xFFFF)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0xF9),
    glide16(THRONE_BLOOD, (0x80, 0xC0), 1, true),
    Op::Place(THRONE_BLOOD, OFF_THE_MAP),
    Op::Dialogue(0xFA),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PORTAL_ROOM,
        player: BASE_PORTAL_CELL,
        objects: PORTAL_ROOM_OBJECTS,
        count: 3,
    },
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::PlayOnce(BASE_PORTAL, PORTAL_OPENS),
    Op::Sound(PORTAL_SOUND),
    Op::AwaitStepEnd(BASE_PORTAL, PORTAL_BRINGS_STEP),
    Op::Show(PLAYER),
    Op::Place(PLAYER, BASE_PORTAL_CELL),
    through(PLAYER, (4, 2), PIXEL, 1),
    Op::Sound(PORTAL_ARRIVAL_SOUND),
    Op::AwaitAnimation(BASE_PORTAL),
    Op::Animate(BASE_PORTAL, PORTAL_IDLE),
    glide(BASE_PORTAL_SOLDIER, (0x80, 0x60), 1, true),
    Op::Repeat(32, PAN_DOWN),
    Op::Dialogue(0xFB),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, TO_CHAPTER_4),
    Op::End,
];

/// The hook the chapter's end leaves (`0x08016880`): chapter 4's first map
/// loads and brightens.
const TO_CHAPTER_4: &[Op] = &[
    Op::Warp {
        map: CHAPTER_4_START,
        cell: (8, 2),
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// The townsfolk of area 3 (`0x08006914` on; `0x08014618` tests flag
/// `0x16A`): a line before the core is destroyed and one after.
macro_rules! before_and_after_the_core {
    ($before:expr, $after:expr) => {
        &[Op::IfFlags {
            all: &[],
            none: &[CORE_DESTROYED],
            then: &[Op::Dialogue($before)],
            otherwise: &[Op::Dialogue($after)],
        }]
    };
}

const TOWNSFOLK: [(u32, &[Op]); 10] = [
    (0x0800_6914, before_and_after_the_core!(0xB4, 0x2FD)),
    (0x0800_6934, before_and_after_the_core!(0xB5, 0x2FE)),
    (0x0800_6954, before_and_after_the_core!(0xB6, 0x2FF)),
    (0x0800_6974, before_and_after_the_core!(0xB7, 0x300)),
    (0x0800_6994, before_and_after_the_core!(0xB8, 0x301)),
    (0x0800_69B4, before_and_after_the_core!(0xB9, 0x302)),
    (0x0800_69D4, before_and_after_the_core!(0xBA, 0x303)),
    (0x0800_69F4, before_and_after_the_core!(0xBB, 0x304)),
    (0x0800_6A14, before_and_after_the_core!(0xBC, 0x305)),
    (0x0800_6A34, before_and_after_the_core!(0xBD, 0x306)),
];

/// A teacher of deck command `command` (`0x08012090` with the pitch and
/// the reminder, or `0x08009430`'s pair of lines).
macro_rules! teacher {
    ($command:expr, $pitch:expr, $reminder:expr) => {
        &[Op::IfCommand {
            command: $command,
            then: &[Op::Dialogue($reminder)],
            otherwise: &[Op::Dialogue($pitch), Op::Call(&learn($command))],
        }]
    };
}

/// The teachers of area 3: in the town (command 1), the fortress's gate
/// (9), wing (13), hall (10) and command room (11, whose reminder is
/// dialogue `0xEC` in the original), and the base's bar (21, `0x0800957C`).
const TEACHERS: [(u32, &[Op]); 6] = [
    (0x0800_6A54, teacher!(1, 0x3D3, 0x3E8)),
    (0x0800_6A6C, teacher!(9, 0x3D4, 0x3E9)),
    (0x0800_6A84, teacher!(0xD, 0x3D5, 0x3EA)),
    (0x0800_6A9C, teacher!(0xA, 0x3D6, 0x3EB)),
    (0x0800_6AB4, teacher!(0xB, 0x3D7, 0xEC)),
    (0x0800_957C, teacher!(0x15, 0x2F4, 0x316)),
];

/// The keepers of area 3 (`0x08009138` on): item shops 3 (the town) and 4
/// (the base), armaments shops 3 (the town), 4 (the fortress), 5 (the
/// base) and 26 (the Kronos fort), and the labs 3 (the fortress) and 4
/// (the base).
const SHOPS: [(u32, &[Op]); 8] = [
    (0x0800_9138, &shop(Shop::Items(3))),
    (0x0800_9144, &shop(Shop::Items(4))),
    (0x0800_9150, &shop(Shop::Arms(3))),
    (0x0800_915C, &shop(Shop::Arms(4))),
    (0x0800_9168, &shop(Shop::Arms(5))),
    (0x0800_9174, &shop(Shop::Arms(0x1A))),
    (0x0800_9180, &shop(Shop::Lab(3))),
    (0x0800_918C, &shop(Shop::Lab(4))),
];

/// What speaking to an object of area 3 whose script is code runs.
pub(super) fn talk_handler(address: u32) -> Option<&'static [Op]> {
    let listed = TOWNSFOLK
        .iter()
        .chain(TEACHERS.iter())
        .chain(SHOPS.iter())
        .find(|(at, _)| *at == address)
        .map(|(_, program)| *program);
    listed.or(match address {
        0x0801_6A58 => Some(AMPLIFIER_TALKS[0]),
        0x0801_6AA4 => Some(AMPLIFIER_TALKS[1]),
        0x0801_6AF0 => Some(AMPLIFIER_TALKS[2]),
        0x0801_6B3C => Some(AMPLIFIER_TALKS[3]),
        0x0801_47B0 => Some(CORE_TALK),
        0x0801_4AD4 => Some(CONTAINER_TALK),
        0x0801_49C0 => Some(DR_D_TALK),
        _ => None,
    })
}

/// What map `map` of area 3 runs when it loads, when it runs anything.
pub(super) fn map_handler(map: usize) -> Option<&'static [Op]> {
    match map {
        DESERT => Some(DESERT_ARRIVAL),
        51 => Some(TOWN_ARRIVAL),
        KRONOS_OUTSKIRTS => Some(KRONOS_ARRIVAL),
        KRONOS_ROOM => Some(KRONOS_ROOM_ARRIVAL),
        KRONOS_HEART => Some(KRONOS_HEART_ARRIVAL),
        FORTRESS_GATE => Some(GATE_ARRIVAL),
        FORTRESS_WING => Some(WING_ARRIVAL),
        COMMAND_ROOM => Some(COMMAND_ROOM_ARRIVAL),
        REAR_ENTRANCE => Some(REAR_ENTRANCE_ARRIVAL),
        TUNNELS => Some(TUNNELS_ARRIVAL),
        CRATER => Some(CRATER_ARRIVAL),
        THRONE_ROOM => Some(THRONE_ROOM_ARRIVAL),
        _ => None,
    }
}
