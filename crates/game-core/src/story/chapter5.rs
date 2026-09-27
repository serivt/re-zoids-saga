//! Chapter 5: the Zoid battle colosseum of the time of Bit and Team Blitz,
//! its three domes' tournaments, and the final against Blood for the
//! Trinity Liger (area 5).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the map
//! handlers of area 5, the tasks and field hooks they install, the
//! objects' code and the match task at `0x0801A3DC`, named at each item.
//! See `docs/events.md`.

use extraction::saga::Reward;

use super::{
    CORE_REWARD, FIELD_HOOK, PLAYER, SCENE_BRIGHTEN, SCENE_DARKEN, glide, learn, shop,
    story_battle, through,
};
use crate::event::{FIELD_WATCH, HERE, MAP_TASK, Op};
use crate::field::{Direction, PIXEL};
use crate::menu::Shop;

/// Set once the Emperor has sent Blood to the colosseum (`0x08019B04`).
const CHAPTER_OPENED: u16 = 0x178;
/// Set once the portal has brought the party into the battle field.
const DISTRICT_REACHED: u16 = 0x179;
/// Set once Team Blitz has told the party how to reach the champion's
/// final.
const FINAL_EXPLAINED: u16 = 0x17A;
/// Set while the party is entered in the South dome's tournament.
const SOUTH_ENTERED: u16 = 0x17B;
/// The South dome's five matches won.
const SOUTH_WINS: [u16; 5] = [0x17D, 0x17E, 0x17F, 0x180, 0x181];
/// Set while the party is entered in the East dome's tournament.
const EAST_ENTERED: u16 = 0x183;
/// The East dome's five matches won.
const EAST_WINS: [u16; 5] = [0x184, 0x185, 0x186, 0x187, 0x188];
/// Set while the party is entered in the Main dome's tournament.
const MAIN_ENTERED: u16 = 0x189;
/// The Main dome's five matches won; the last is the final.
const MAIN_WINS: [u16; 5] = [0x18A, 0x18B, 0x18C, 0x18D, 0x18E];
/// Set when the South dome was won before the East one.
const SOUTH_FIRST: u16 = 0x182;
/// Set when the East dome was won before the South one.
const EAST_FIRST: u16 = 0x17C;
/// Set once Naomi's team has spoken after the South dome's last match.
const SOUTH_CELEBRATED: u16 = 0x191;
/// Set once Harry's team has spoken after the East dome's last match.
const EAST_CELEBRATED: u16 = 0x192;
/// Set once Bit and Ballad have met the party at the Main dome.
const MAIN_WELCOMED: u16 = 0x193;
/// Set once Ballad has joined.
const BALLAD_JOINED: u16 = 0x190;

/// The Emperor's throne room (map 165), where chapter 4's end leaves the
/// party; the castle's portal room (168) and the room above the base's bar
/// (174).
const THRONE_ROOM: usize = 165;
const PORTAL_ROOM: usize = 168;
const BASE_ROOM: usize = 174;
/// The colosseum's district (map 133), a Zoid map with the portal.
const DISTRICT: usize = 133;
/// Dr. Tros's rooms (134 to 136).
const TROS_ROOM: usize = 134;
const TROS_HALL: usize = 135;
const TROS_BACK_ROOM: usize = 136;
/// The domes' halls, with their way out on the left, and their desks.
const SOUTH_HALL: usize = 143;
const SOUTH_DESK: usize = 144;
const EAST_HALL: usize = 146;
const EAST_DESK: usize = 147;
const MAIN_HALL: usize = 149;
const MAIN_DESK: usize = 150;
/// The arena every match is fought in.
const ARENA: usize = 152;
/// Chapter 6's first map, where the chapter's end leaves the party.
const CHAPTER_6_START: usize = 205;

/// The songs the chapter switches to.
const DANGER_MUSIC: u16 = 4;
const BLOOD_MUSIC: u16 = 9;
const FRIENDS_MUSIC: u16 = 8;
const DUEL_MUSIC: u16 = 1;
const COLOSSEUM_MUSIC: u16 = 0x12;
const LIGER_MUSIC: u16 = 0x17;

const PORTAL_SOUND: u16 = 0x6F;
const PORTAL_ARRIVAL_SOUND: u16 = 0x49;
const BLAST_SOUND: u16 = 0x5B;
const PORTAL_IDLE: usize = 0;
const PORTAL_OPENS: usize = 2;
const PORTAL_BRINGS_STEP: usize = 32;

/// Frames a task waits after the fade in before its first line.
const SETTLE: u32 = 32;
const OFF_THE_MAP: (usize, usize) = (0xFF, 0xFF);
const HIDDEN: (usize, usize) = (0xFFFF, 0xFFFF);
/// The prince standing again (`0x080089A0`).
const PRINCE_STANDING: usize = 0x98;
/// The characters of group 4, met as the chapter opens (`0x080099E4`).
const CHAPTER_GROUP: u8 = 4;
/// The Zoid core Dr. Tros gives (`0x080379B4` with `0x14`).
const TROS_GIFT: u8 = 0x14;

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

/// The same, the player turned `facing` first (`0x08000BD8`), which the
/// warp keeps.
const fn back_facing(map: usize, cell: (usize, usize), facing: Direction) -> [Op; 5] {
    [
        Op::Call(SCENE_DARKEN),
        Op::Face(PLAYER, facing),
        Op::Warp {
            map,
            cell,
            facing: Some(facing),
        },
        Op::FadeInHoldingSlow,
        Op::End,
    ]
}

/// A blast over `cell` (`0x08016CA8`): explosion object `actor` plays once
/// there with its sound, then goes.
const fn blast(actor: usize, cell: (usize, usize)) -> [Op; 5] {
    [
        Op::Place(actor, cell),
        Op::PlayOnce(actor, 0),
        Op::Sound(BLAST_SOUND),
        Op::AwaitAnimation(actor),
        Op::Place(actor, OFF_THE_MAP),
    ]
}

/// A story battle's hook: beaten, the party is taken to its return point
/// and the field brightens at once.
const BEATEN: &[Op] = &[Op::WarpHome, Op::FadeInHolding];

/// The throne room for the opening (ROM `0x08668558`): the prince off the
/// map, the Emperor and Blood.
const THRONE_OBJECTS: u32 = 0x0866_8558;
const THRONE_BLOOD: usize = 2;
/// The portal room (ROM `0x08668594`): three soldiers and the portal.
const PORTAL_ROOM_OBJECTS: u32 = 0x0866_8594;
/// The room above the base's bar (ROM `0x086685F8`): the prince seated,
/// Regina, Jack and Earth off the map.
const BASE_OBJECTS: u32 = 0x0866_85F8;
const REGINA: usize = 1;
const JACK: usize = 2;
const EARTH: usize = 3;

const JACK_COMES_IN: &[Op] = &[
    Op::Place(JACK, (0xE, 2)),
    glide16(JACK, (0x30, 0x20), 1, true),
    glide16(JACK, (0x30, 0x30), 1, true),
    Op::Face(JACK, Direction::Down),
    Op::End,
];

/// The chapter's opening (task at `0x08019BEC`): Blood answers the Emperor,
/// who asks about Zoid battles (dialogue `0x123`); in the portal room the
/// soldiers sense the device in use again, far from before (`0x124`); in
/// the room above the bar Jack brings the news (`0x125`) and the party
/// leaves.
const OPENING: &[Op] = &[
    Op::Wait(SETTLE),
    glide16(THRONE_BLOOD, (0x80, 0x30), 1, true),
    Op::Dialogue(0x123),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PORTAL_ROOM,
        player: (4, 4),
        objects: PORTAL_ROOM_OBJECTS,
        count: 5,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x124),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: BASE_ROOM,
        player: (2, 5),
        objects: BASE_OBJECTS,
        count: 4,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, JACK_COMES_IN),
    Op::Wait(60),
    Op::Place(EARTH, (0xE, 2)),
    glide16(EARTH, (0x50, 0x20), 1, true),
    glide16(EARTH, (0x50, 0x30), 1, true),
    Op::Face(EARTH, Direction::Down),
    Op::Face(PLAYER, Direction::Up),
    Op::Dialogue(0x125),
    Op::Sprite(PLAYER, PRINCE_STANDING),
    Op::Face(PLAYER, Direction::Right),
    glide16(PLAYER, (0x20, 0x40), 1, false),
    glide16(REGINA, (0x10, 0x40), 1, true),
    glide16(REGINA, (0x20, 0x40), 1, true),
    Op::Place(REGINA, OFF_THE_MAP),
    glide16(JACK, (0x20, 0x30), 1, true),
    glide16(JACK, (0x20, 0x40), 1, true),
    Op::Place(JACK, OFF_THE_MAP),
    glide16(EARTH, (0x20, 0x30), 1, true),
    glide16(EARTH, (0x20, 0x40), 1, true),
    Op::Place(EARTH, OFF_THE_MAP),
    Op::Call(&back_to(BASE_ROOM, (2, 4))),
];

/// The throne room (`0x08019B04`): once chapter 4 is over, the first time,
/// the characters of group 4 are met, the room loads with the Emperor and
/// Blood and the player off the map, and the opening runs.
const THRONE_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[CHAPTER_OPENED],
    then: &[
        Op::Meet(CHAPTER_GROUP),
        Op::LoadMap {
            map: THRONE_ROOM,
            player: (8, 2),
            objects: THRONE_OBJECTS,
            count: 3,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, OPENING),
        Op::Place(PLAYER, HIDDEN),
        Op::Flag(CHAPTER_OPENED, true),
    ],
    otherwise: &[],
}];

/// The district as the portal brings the party in (ROM `0x0866847C`): the
/// Gustav, Team Blitz's and the ambushers' Zoids off the map, three
/// explosions and the portal; the same list, without the portal's run,
/// for Team Blitz's second talk.
const DISTRICT_OBJECTS: u32 = 0x0866_847C;
const DISTRICT_PORTAL: usize = 10;
const DISTRICT_PORTAL_CELL: (usize, usize) = (41, 7);
const DISTRICT_ARRIVAL_CELL: (usize, usize) = (0x29, 8);
const RAIDER: usize = 1;
const RAIDER_2: usize = 2;
const RAIDER_3: usize = 3;
const LEON: usize = 4;
const JAMIE: usize = 5;
const BIT_ZOID: usize = 6;
const EXPLOSION_1: usize = 7;
const EXPLOSION_2: usize = 8;
const EXPLOSION_3: usize = 9;
/// Where the Gustav stops before the shelling: cell (`0x29`, `0xC`).
const SHELLED_1: [Op; 5] = blast(EXPLOSION_1, (0x2A, 0xB));
const SHELLED_2: [Op; 5] = blast(EXPLOSION_1, (0x28, 0xD));
const SHELLED_3: [Op; 5] = blast(EXPLOSION_1, (0x29, 0xD));
const SHELLING_A: [Op; 5] = blast(EXPLOSION_1, (0x28, 0xD));
const SHELLING_B: [Op; 5] = blast(EXPLOSION_2, (0x29, 0xD));
const SHELLING_C: [Op; 5] = blast(EXPLOSION_3, (0x2A, 0xD));
const SHELL_A: &[Op] = &[Op::Call(&SHELLING_A), Op::End];
const SHELL_B: &[Op] = &[Op::Wait(2), Op::Call(&SHELLING_B), Op::End];
const SHELL_C: &[Op] = &[Op::Wait(4), Op::Call(&SHELLING_C), Op::End];
const RAIDER_2_COMES: &[Op] = &[
    Op::Place(RAIDER_2, (0x2E, 0xD)),
    glide(RAIDER_2, (0x560, 0x1A0), 1, true),
    Op::End,
];
const RAIDER_3_COMES: &[Op] = &[
    Op::Place(RAIDER_3, (0x2E, 0xF)),
    glide(RAIDER_3, (0x560, 0x1E0), 1, true),
    Op::End,
];
const RAIDER_2_GOES: &[Op] = &[
    glide(RAIDER_2, (0x5A0, 0x1A0), 2, true),
    Op::Place(RAIDER_2, OFF_THE_MAP),
    Op::End,
];
const RAIDER_3_GOES: &[Op] = &[
    glide(RAIDER_3, (0x5A0, 0x1E0), 2, true),
    Op::Place(RAIDER_3, OFF_THE_MAP),
    Op::End,
];
const JAMIE_COMES: &[Op] = &[
    Op::Place(JAMIE, (0x24, 0xE)),
    glide(JAMIE, (0x500, 0x1C0), 1, true),
    Op::End,
];
const BIT_ZOID_COMES: &[Op] = &[
    Op::Place(BIT_ZOID, (0x23, 0xE)),
    glide(BIT_ZOID, (0x4E0, 0x1C0), 1, true),
    Op::End,
];

/// Dr. Tros's rooms for the arrival's end (ROM `0x08668648`, `0x086686E8`).
const TROS_HALL_OBJECTS: u32 = 0x0866_8648;
const TROS_BACK_ROOM_OBJECTS: u32 = 0x0866_86E8;
/// The lists Team Blitz joins with.
const BLITZ_LISTS: [u8; 3] = [0xD, 0xE, 0xF];

/// Into the battle field (task at `0x08019DE4`): the portal brings the
/// Gustav; the party wonders where it is (`0x126`); shells fall around it
/// (`0x127`) and it drives on under fire; the judge calls the battle off
/// (`0x128`); to Blood's song Blood's team drives up and goes (`0x129`,
/// `0x12A`); to the song of friends Team Blitz drives up, and Leena scolds
/// the party (`0x12B`); at Dr. Tros's the party hears of the colosseum
/// (`0x12C`) and of the top rankers gone missing, and Team Blitz joins
/// (`0x12D`, lists 13 to 15).
const ARRIVAL: &[Op] = &[
    Op::Wait(SETTLE),
    Op::PlayOnce(DISTRICT_PORTAL, PORTAL_OPENS),
    Op::Sound(PORTAL_SOUND),
    Op::AwaitStepEnd(DISTRICT_PORTAL, PORTAL_BRINGS_STEP),
    Op::Show(PLAYER),
    Op::Place(PLAYER, DISTRICT_PORTAL_CELL),
    through(PLAYER, DISTRICT_ARRIVAL_CELL, PIXEL, 1),
    Op::Sound(PORTAL_ARRIVAL_SOUND),
    Op::AwaitAnimation(DISTRICT_PORTAL),
    Op::Animate(DISTRICT_PORTAL, PORTAL_IDLE),
    Op::Dialogue(0x126),
    glide(PLAYER, (0x520, 0x180), 1, true),
    Op::Call(&SHELLED_1),
    Op::Call(&SHELLED_2),
    Op::Call(&SHELLED_3),
    Op::Dialogue(0x127),
    Op::Spawn(MAP_TASK + 1, SHELL_A),
    Op::Spawn(MAP_TASK + 2, SHELL_B),
    Op::Spawn(MAP_TASK + 3, SHELL_C),
    glide(PLAYER, (0x520, 0x1C0), 1, true),
    Op::Stop(MAP_TASK + 1),
    Op::Stop(MAP_TASK + 2),
    Op::Stop(MAP_TASK + 3),
    Op::Place(EXPLOSION_1, OFF_THE_MAP),
    Op::Place(EXPLOSION_2, OFF_THE_MAP),
    Op::Place(EXPLOSION_3, OFF_THE_MAP),
    Op::Dialogue(0x128),
    Op::RestartMusic(BLOOD_MUSIC),
    Op::Spawn(MAP_TASK + 1, RAIDER_2_COMES),
    Op::Spawn(MAP_TASK + 2, RAIDER_3_COMES),
    Op::Place(RAIDER, (0x2D, 0xE)),
    glide(RAIDER, (0x540, 0x1C0), 1, true),
    Op::RestartMusic(BLOOD_MUSIC),
    Op::Dialogue(0x129),
    Op::Spawn(MAP_TASK + 1, RAIDER_2_GOES),
    Op::Spawn(MAP_TASK + 2, RAIDER_3_GOES),
    glide(RAIDER, (0x5A0, 0x1C0), 2, true),
    Op::Place(RAIDER, OFF_THE_MAP),
    Op::Wait(1),
    Op::Dialogue(0x12A),
    Op::RestartMusic(FRIENDS_MUSIC),
    Op::Spawn(MAP_TASK + 1, JAMIE_COMES),
    Op::Spawn(MAP_TASK + 2, BIT_ZOID_COMES),
    Op::Place(LEON, (0x24, 0xD)),
    glide(LEON, (0x500, 0x1A0), 1, true),
    Op::RestartMusic(FRIENDS_MUSIC),
    Op::Dialogue(0x12B),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: TROS_HALL,
        player: (9, 3),
        objects: TROS_HALL_OBJECTS,
        count: 8,
    },
    Op::Place(PLAYER, (8, 3)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x12C),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: TROS_BACK_ROOM,
        player: (9, 8),
        objects: TROS_BACK_ROOM_OBJECTS,
        count: 8,
    },
    Op::Place(PLAYER, (7, 7)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x12D),
    Op::Join(BLITZ_LISTS[0]),
    Op::Join(BLITZ_LISTS[1]),
    Op::Join(BLITZ_LISTS[2]),
    Op::Call(&back_to(TROS_HALL, (0xA, 1))),
];

/// Back in the district (task at `0x0801A2DC`): Team Blitz tells how to
/// reach the district's champion's final (`0x12F`).
const FINAL_EXPLANATION: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Dialogue(0x12F),
    Op::Call(&back_to(DISTRICT, (0x25, 0x13))),
];

/// The district (`0x08018EEC`): once chapter 4 is over, the first time, it
/// loads with the portal and the Gustav hidden and the portal brings the
/// party; the next time it loads with Team Blitz's talk, and a beaten party
/// is taken to Dr. Tros's from then on.
const DISTRICT_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CHAPTER_OPENED],
    none: &[DISTRICT_REACHED],
    then: &[
        Op::LoadMap {
            map: DISTRICT,
            player: (0x29, 7),
            objects: DISTRICT_OBJECTS,
            count: 11,
        },
        Op::Place(PLAYER, HIDDEN),
        Op::Control(false),
        Op::Spawn(MAP_TASK, ARRIVAL),
        Op::Flag(DISTRICT_REACHED, true),
    ],
    otherwise: &[Op::IfFlags {
        all: &[DISTRICT_REACHED],
        none: &[FINAL_EXPLAINED],
        then: &[
            Op::LoadMap {
                map: DISTRICT,
                player: (0x25, 0x13),
                objects: DISTRICT_OBJECTS,
                count: 11,
            },
            Op::Control(false),
            Op::Spawn(MAP_TASK, FINAL_EXPLANATION),
            Op::ReturnPoint(DISTRICT_RETURN_POINT),
            Op::Flag(FINAL_EXPLAINED, true),
        ],
        otherwise: &[],
    }],
}];
/// The district's return point: Dr. Tros's.
const DISTRICT_RETURN_POINT: u8 = 0xC;

/// The Zoid Federation's Ultrasaurus fight (story battle 25).
const ULTRASAURUS_BATTLE: u8 = 25;
const ULTRASAURUS_BATTLE_OPS: [Op; 4] = story_battle(ULTRASAURUS_BATTLE);
/// The Ultrasaurus's battle (the hook at `0x08018FFC`): won, the field
/// brightens at once and nothing else changes.
const ULTRASAURUS_FIGHT: &[Op] = &[
    Op::Call(&ULTRASAURUS_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[Op::FadeInHolding],
    },
    Op::End,
];
/// Speaking to the Zoid Federation's Ultrasaurus in the district
/// (`0x08018FCC`): it orders the party to leave (`0x16C`); answering
/// anything but its first choice starts the fight from the field's hook.
const ULTRASAURUS_TALK: &[Op] = &[
    Op::Dialogue(0x16C),
    Op::IfChoice {
        then: &[],
        otherwise: &[Op::Spawn(FIELD_HOOK, ULTRASAURUS_FIGHT)],
    },
];

/// Where a dome's hall lets the party out (the hooks at `0x080190B0`,
/// `0x080193E8` and `0x080197B0`): the player's sprite in its doorway on
/// row 27.
const EXIT_Y: (i32, i32) = (0x1B0, 0x1B0);
const SOUTH_EXIT_X: (i32, i32) = (0x158, 0x178);
const MAIN_EXIT_X: (i32, i32) = (0x148, 0x188);
/// Where leaving a tournament takes the party, and where staying does.
const SOUTH_OUTSIDE: (usize, usize) = (0xC, 0xD);
const EAST_OUTSIDE: (usize, usize) = (0x1A, 6);
const MAIN_OUTSIDE: (usize, usize) = (7, 3);
const HALL_INSIDE: (usize, usize) = (0x17, 0x1A);

/// The South dome's hall: leaving during the tournament (task at
/// `0x08019104`): the desk's warning (`0x132`); leaving loses every win,
/// staying brings the party back in.
const SOUTH_LEAVING: &[Op] = &[
    Op::Dialogue(0x132),
    Op::IfChoice {
        then: &[
            Op::Flag(SOUTH_ENTERED, false),
            Op::Flag(SOUTH_WINS[0], false),
            Op::Flag(SOUTH_WINS[1], false),
            Op::Flag(SOUTH_WINS[2], false),
            Op::Flag(SOUTH_WINS[3], false),
            Op::Flag(SOUTH_WINS[4], false),
            Op::Call(&back_to(DISTRICT, SOUTH_OUTSIDE)),
        ],
        otherwise: &[Op::Call(&back_to(SOUTH_HALL, HALL_INSIDE))],
    },
];
const EAST_LEAVING: &[Op] = &[
    Op::Dialogue(0x14F),
    Op::IfChoice {
        then: &[
            Op::Flag(EAST_ENTERED, false),
            Op::Flag(EAST_WINS[0], false),
            Op::Flag(EAST_WINS[1], false),
            Op::Flag(EAST_WINS[2], false),
            Op::Flag(EAST_WINS[3], false),
            Op::Flag(EAST_WINS[4], false),
            Op::Call(&back_to(DISTRICT, EAST_OUTSIDE)),
        ],
        otherwise: &[Op::Call(&back_to(EAST_HALL, HALL_INSIDE))],
    },
];
const MAIN_LEAVING: &[Op] = &[
    Op::Dialogue(0x170),
    Op::IfChoice {
        then: &[
            Op::Flag(MAIN_ENTERED, false),
            Op::Flag(MAIN_WINS[0], false),
            Op::Flag(MAIN_WINS[1], false),
            Op::Flag(MAIN_WINS[2], false),
            Op::Flag(MAIN_WINS[3], false),
            Op::Flag(MAIN_WINS[4], false),
            Op::Call(&back_to(DISTRICT, MAIN_OUTSIDE)),
        ],
        otherwise: &[Op::Call(&back_to(MAIN_HALL, HALL_INSIDE))],
    },
];

macro_rules! exit_watch {
    ($x:expr, $leaving:expr) => {
        &[
            Op::AwaitPlayerSprite { x: $x, y: EXIT_Y },
            Op::Control(false),
            Op::Spawn(MAP_TASK, $leaving),
            Op::End,
        ]
    };
}
const SOUTH_WATCH: &[Op] = exit_watch!(SOUTH_EXIT_X, SOUTH_LEAVING);
const EAST_WATCH: &[Op] = exit_watch!(SOUTH_EXIT_X, EAST_LEAVING);
const MAIN_WATCH: &[Op] = exit_watch!(MAIN_EXIT_X, MAIN_LEAVING);

/// The domes' halls (`0x0801907C`, `0x080193B4`, `0x080196E8`): while the
/// party is entered and the dome is not won, the way out asks first. The
/// Main dome's hall shows Ballad once the party is entered, until he joins.
const SOUTH_HALL_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[SOUTH_ENTERED],
    none: &[SOUTH_WINS[4]],
    then: &[Op::Spawn(FIELD_WATCH, SOUTH_WATCH)],
    otherwise: &[],
}];
const EAST_HALL_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[EAST_ENTERED],
    none: &[EAST_WINS[4]],
    then: &[Op::Spawn(FIELD_WATCH, EAST_WATCH)],
    otherwise: &[],
}];
const BALLAD: usize = 1;
const MAIN_HALL_ARRIVAL: &[Op] = &[
    Op::IfFlags {
        all: &[MAIN_ENTERED],
        none: &[MAIN_WINS[4]],
        then: &[Op::Spawn(FIELD_WATCH, MAIN_WATCH)],
        otherwise: &[],
    },
    Op::IfFlags {
        all: &[MAIN_ENTERED],
        none: &[BALLAD_JOINED],
        then: &[],
        otherwise: &[Op::Place(BALLAD, OFF_THE_MAP)],
    },
];

/// Ballad joins and the hall loads again without him (the hook at
/// `0x0801977C`), the player where it stands. The original's field loop
/// waits inside the hook while the screen darkens, so the player cannot
/// act: the port takes the control away for it.
const BALLAD_LEAVES_THE_HALL: &[Op] = &[
    Op::Control(false),
    Op::Call(SCENE_DARKEN),
    Op::Warp {
        map: MAIN_HALL,
        cell: HERE,
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];
/// The list Ballad joins with.
const BALLAD_LIST: u8 = 0x15;
/// The list Bit joins with.
const BIT_LIST: u8 = 0x14;
/// Speaking to Ballad in the Main dome's hall (`0x0801973C`): he offers his
/// services (`0x171`); taken, he joins.
const BALLAD_TALK: &[Op] = &[
    Op::Dialogue(0x171),
    Op::IfChoice {
        then: &[
            Op::Join(BALLAD_LIST),
            Op::Flag(BALLAD_JOINED, true),
            Op::Spawn(FIELD_HOOK, BALLAD_LEAVES_THE_HALL),
        ],
        otherwise: &[],
    },
];

/// Where a lost match, or a won one once its scene is over, leaves the
/// party: in front of its dome's desk.
const SOUTH_LOST: [Op; 5] = back_facing(SOUTH_DESK, (0xB, 8), Direction::Down);
const EAST_LOST: [Op; 5] = back_facing(EAST_DESK, (0xB, 8), Direction::Down);
const MAIN_LOST: [Op; 5] = back_facing(MAIN_DESK, (8, 4), Direction::Down);
const SOUTH_HOME: [Op; 5] = back_facing(SOUTH_DESK, (0xB, 8), Direction::Up);
const EAST_HOME: [Op; 5] = back_facing(EAST_DESK, (0xB, 8), Direction::Up);
const MAIN_HOME: [Op; 5] = back_facing(MAIN_DESK, (8, 4), Direction::Right);

/// A match (task at `0x0801A3DC` with `n`): the desk asks whether to fight
/// (`question`); yes, `before` runs, the arena loads with the match's
/// enemies and the formation's Zoids, `opening` runs, and the fight follows
/// from the field's hook.
macro_rules! colosseum_match {
    ($n:expr, $question:expr, $before:expr, $opening:expr, $fight:expr) => {
        &[
            Op::Dialogue($question),
            Op::IfChoice {
                then: &[
                    Op::Control(false),
                    Op::Call($before),
                    Op::Call(SCENE_DARKEN),
                    Op::LoadArena($n),
                    Op::Place(PLAYER, OFF_THE_MAP),
                    Op::Call($opening),
                    Op::Spawn(FIELD_HOOK, $fight),
                    Op::End,
                ],
                otherwise: &[Op::Control(true), Op::End],
            },
        ]
    };
}

/// The judge's opening in the arena, once it brightens.
macro_rules! opening {
    ($dialogue:expr) => {
        &[Op::Call(SCENE_BRIGHTEN), Op::Dialogue($dialogue)]
    };
}

/// A match's battle (the hook at `0x0801AE30`): lost, the party is taken
/// back to the desk; won, the match's flag is set and `won` runs.
macro_rules! fight {
    ($battle:expr, $lost:expr, $flag:expr, $won:expr) => {
        &[
            Op::Call(&story_battle($battle)),
            Op::IfLost {
                then: $lost,
                otherwise: &[
                    Op::Flag($flag, true),
                    Op::Control(false),
                    Op::Spawn(MAP_TASK, $won),
                ],
            },
            Op::End,
        ]
    };
}

/// A won match (task at `0x0801A904`): the arena brightens, the judge's
/// closing, `after`, and back to the desk.
macro_rules! won {
    ($closing:expr, $after:expr, $home:expr) => {
        &[
            Op::Call(SCENE_BRIGHTEN),
            Op::Dialogue($closing),
            Op::Call($after),
            Op::Call($home),
        ]
    };
}

/// Naomi before the South dome's last match (`0x0801A4D8`): the desk's room
/// loads with Team Blitz and Naomi comes in (`0x144` to `0x146`).
const NAOMI_OBJECTS: u32 = 0x0866_8F30;
const NAOMI: usize = 3;
const BEFORE_NAOMI: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: SOUTH_DESK,
        player: (0xB, 9),
        objects: NAOMI_OBJECTS,
        count: 4,
    },
    Op::RestartMusic(COLOSSEUM_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x144),
    Op::Place(NAOMI, (9, 0xD)),
    glide16(NAOMI, (0x70, 0x90), 1, true),
    Op::Face(NAOMI, Direction::Right),
    Op::Wait(15),
    Op::Face(PLAYER, Direction::Left),
    Op::Face(1, Direction::Left),
    Op::Face(2, Direction::Left),
    Op::Wait(15),
    Op::Dialogue(0x145),
    glide16(NAOMI, (0x70, 0x30), 1, true),
    Op::Place(NAOMI, OFF_THE_MAP),
    Op::Dialogue(0x146),
];

/// Harry's team before the East dome's last match (`0x0801A584`, `0x161`
/// to `0x164`).
const HARRY_OBJECTS: u32 = 0x0866_8F80;
const BEFORE_HARRY: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: EAST_DESK,
        player: (0xB, 9),
        objects: HARRY_OBJECTS,
        count: 6,
    },
    Op::RestartMusic(COLOSSEUM_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x161),
    Op::Place(3, (9, 0xD)),
    glide16(3, (0x90, 0xA0), 1, true),
    Op::Place(4, (8, 0xD)),
    glide16(4, (0x80, 0xA0), 1, true),
    Op::Place(5, (8, 0xD)),
    glide16(5, (0x80, 0xC0), 1, true),
    glide16(5, (0x70, 0xC0), 1, true),
    glide16(5, (0x70, 0xA0), 1, true),
    Op::Wait(15),
    Op::Face(PLAYER, Direction::Down),
    Op::Face(1, Direction::Down),
    Op::Face(2, Direction::Down),
    Op::Wait(15),
    Op::Dialogue(0x162),
    glide16(3, (0x90, 0xD0), 1, true),
    Op::Place(3, OFF_THE_MAP),
    Op::Dialogue(0x163),
    glide16(4, (0x80, 0xD0), 1, true),
    Op::Place(4, OFF_THE_MAP),
    glide16(5, (0x80, 0xA0), 1, true),
    glide16(5, (0x80, 0xD0), 1, true),
    Op::Place(5, OFF_THE_MAP),
    Op::Dialogue(0x164),
];

/// Before the final (`0x0801A6CC`): the Main dome's desk with Team Blitz,
/// Bit and Ballad (ROM `0x08668FF8`), and Leena's thanks (`0x184`). The
/// original sets flag `0x190` where it means to test it (`0x08000F88`
/// returns the flag's number), so Ballad always stays and has joined from
/// then on.
const FINAL_EVE_OBJECTS: u32 = 0x0866_8FF8;
const BEFORE_FINAL: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: MAIN_DESK,
        player: (8, 8),
        objects: FINAL_EVE_OBJECTS,
        count: 9,
    },
    Op::Place(PLAYER, (8, 6)),
    Op::Flag(BALLAD_JOINED, true),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x184),
];

/// The final's opening (`0x0801A838`): the Trinity Liger is the prize
/// (`0x185`), and to Blood's song Blood's team drives in (`0x186`).
const FINAL_OPENING: &[Op] = &[
    Op::Place(2, OFF_THE_MAP),
    Op::Place(3, OFF_THE_MAP),
    Op::Place(4, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x185),
    Op::RestartMusic(BLOOD_MUSIC),
    Op::Place(2, (1, 7)),
    glide16(2, (0x60, 0x70), 1, true),
    Op::Place(3, (1, 6)),
    glide16(3, (0x50, 0x60), 1, true),
    Op::Place(4, (1, 8)),
    glide16(4, (0x50, 0x80), 1, true),
    Op::Dialogue(0x186),
];

/// The list Naomi joins with, and Harry's team's.
const NAOMI_LIST: u8 = 0x10;
const HARRY_LISTS: [u8; 3] = [0x11, 0x12, 0x13];

/// After the South dome's last match (`0x0801A944`), the first time: the
/// desk's room with Naomi (ROM `0x086690E8`, `0x149`); if the East dome
/// was won first, Harry shows up (`0x14A`, `0x14B`), else Naomi hears of
/// the missing rankers and joins (`0x14C`, list 16).
const SOUTH_WON_OBJECTS: u32 = 0x0866_90E8;
const SOUTH_WON: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[SOUTH_CELEBRATED],
    then: &[
        Op::Flag(SOUTH_CELEBRATED, true),
        Op::Call(SCENE_DARKEN),
        Op::LoadMap {
            map: SOUTH_DESK,
            player: (0xB, 9),
            objects: SOUTH_WON_OBJECTS,
            count: 6,
        },
        Op::RestartMusic(COLOSSEUM_MUSIC),
        Op::Call(SCENE_BRIGHTEN),
        Op::Dialogue(0x149),
        Op::IfFlags {
            all: &[EAST_FIRST],
            none: &[],
            then: &[
                Op::Place(5, (9, 0xD)),
                glide16(5, (0x80, 0xA0), 1, true),
                Op::Dialogue(0x14A),
                glide16(4, (0x70, 0x30), 1, true),
                Op::Place(4, OFF_THE_MAP),
                Op::Dialogue(0x14B),
            ],
            otherwise: &[
                Op::Dialogue(0x14C),
                Op::Flag(SOUTH_FIRST, true),
                Op::Join(NAOMI_LIST),
            ],
        },
    ],
    otherwise: &[],
}];

/// Harry's friends take him away (`0x0801ABCC`, `0x169`, `0x16A`).
const HARRY_TAKEN_AWAY: &[Op] = &[
    Op::Dialogue(0x169),
    glide16(1, (0x90, 0xC0), 1, true),
    glide16(1, (0xA0, 0xC0), 1, true),
    Op::Face(1, Direction::Left),
    glide16(2, (0x80, 0xC0), 1, true),
    glide16(2, (0x70, 0xC0), 1, true),
    Op::Face(2, Direction::Right),
    glide16(4, (0x90, 0xB0), 1, true),
    glide16(4, (0xA0, 0xB0), 1, true),
    Op::Face(4, Direction::Left),
    glide16(5, (0x70, 0xA0), 1, true),
    Op::Face(5, Direction::Right),
    Op::Face(PLAYER, Direction::Left),
    Op::Face(3, Direction::Right),
    glide16(7, (0x80, 0xD0), 1, true),
    Op::Place(7, OFF_THE_MAP),
    glide16(6, (0x90, 0xD0), 1, true),
    Op::Place(6, OFF_THE_MAP),
    glide16(8, (0x80, 0x90), 1, true),
    glide16(8, (0x80, 0xD0), 1, true),
    Op::Place(8, OFF_THE_MAP),
    Op::Dialogue(0x16A),
];

/// After the East dome's last match (`0x0801AA16`), the first time: the
/// desk's room fills with the party (ROM `0x08669160`); Harry comes to
/// (`0x167`, `0x168`); if the South dome was won first his friends take
/// him away, else he tells of the man with odd glasses and his team joins
/// (`0x16B`, lists 17 to 19).
const EAST_WON_OBJECTS: u32 = 0x0866_9160;
const EAST_WON: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[EAST_CELEBRATED],
    then: &[
        Op::Flag(EAST_CELEBRATED, true),
        Op::Call(SCENE_DARKEN),
        Op::LoadMap {
            map: EAST_DESK,
            player: (9, 9),
            objects: EAST_WON_OBJECTS,
            count: 9,
        },
        Op::RestartMusic(COLOSSEUM_MUSIC),
        Op::Place(PLAYER, OFF_THE_MAP),
        Op::Call(SCENE_BRIGHTEN),
        Op::Dialogue(0x167),
        Op::Place(PLAYER, (9, 0xD)),
        glide16(PLAYER, (0x90, 0xC0), 1, false),
        glide16(PLAYER, (0xA0, 0xA0), 1, false),
        Op::Face(PLAYER, Direction::Up),
        Op::Place(4, (9, 0xD)),
        glide16(4, (0x90, 0xC0), 1, true),
        glide16(4, (0x90, 0xA0), 1, true),
        Op::Face(4, Direction::Up),
        Op::Place(5, (9, 0xD)),
        glide16(5, (0x90, 0xC0), 1, true),
        glide16(5, (0x80, 0xA0), 1, true),
        Op::Face(5, Direction::Up),
        Op::Place(1, (9, 0xD)),
        glide16(1, (0x90, 0xC0), 1, true),
        glide16(1, (0x90, 0xB0), 1, true),
        Op::Face(1, Direction::Up),
        Op::Place(2, (9, 0xD)),
        glide16(2, (0x90, 0xC0), 1, true),
        glide16(2, (0x80, 0xB0), 1, true),
        Op::Face(2, Direction::Up),
        Op::Place(3, (9, 0xD)),
        glide16(3, (0x90, 0xC0), 1, true),
        glide16(3, (0x70, 0xC0), 1, true),
        glide16(3, (0x70, 0xB0), 1, true),
        Op::Face(3, Direction::Up),
        Op::Wait(15),
        Op::Face(6, Direction::Down),
        Op::Face(7, Direction::Down),
        Op::Face(8, Direction::Down),
        Op::Wait(15),
        Op::Dialogue(0x168),
        Op::IfFlags {
            all: &[SOUTH_FIRST],
            none: &[],
            then: HARRY_TAKEN_AWAY,
            otherwise: &[
                Op::Dialogue(0x16B),
                Op::Join(HARRY_LISTS[0]),
                Op::Join(HARRY_LISTS[1]),
                Op::Join(HARRY_LISTS[2]),
                Op::Flag(EAST_FIRST, true),
            ],
        },
    ],
    otherwise: &[],
}];

const NOTHING: &[Op] = &[];

const SOUTH_OPENINGS: [&[Op]; 5] = [
    opening!(0x135),
    opening!(0x138),
    opening!(0x13C),
    opening!(0x140),
    opening!(0x147),
];
const EAST_OPENINGS: [&[Op]; 5] = [
    opening!(0x152),
    opening!(0x155),
    opening!(0x159),
    opening!(0x15D),
    opening!(0x165),
];
const MAIN_OPENINGS: [&[Op]; 4] = [
    opening!(0x174),
    opening!(0x178),
    opening!(0x17C),
    opening!(0x180),
];

const SOUTH_WON_0: &[Op] = won!(0x136, NOTHING, &SOUTH_HOME);
const SOUTH_WON_1: &[Op] = won!(0x139, NOTHING, &SOUTH_HOME);
const SOUTH_WON_2: &[Op] = won!(0x13D, NOTHING, &SOUTH_HOME);
const SOUTH_WON_3: &[Op] = won!(0x141, NOTHING, &SOUTH_HOME);
const SOUTH_WON_4: &[Op] = won!(0x148, SOUTH_WON, &SOUTH_HOME);
const EAST_WON_0: &[Op] = won!(0x153, NOTHING, &EAST_HOME);
const EAST_WON_1: &[Op] = won!(0x156, NOTHING, &EAST_HOME);
const EAST_WON_2: &[Op] = won!(0x15A, NOTHING, &EAST_HOME);
const EAST_WON_3: &[Op] = won!(0x15E, NOTHING, &EAST_HOME);
const EAST_WON_4: &[Op] = won!(0x166, EAST_WON, &EAST_HOME);
const MAIN_WON_0: &[Op] = won!(0x175, NOTHING, &MAIN_HOME);
const MAIN_WON_1: &[Op] = won!(0x179, NOTHING, &MAIN_HOME);
const MAIN_WON_2: &[Op] = won!(0x17D, NOTHING, &MAIN_HOME);
const MAIN_WON_3: &[Op] = won!(0x181, NOTHING, &MAIN_HOME);

/// The final won (`0x0801AD18`): to the duel's song Blood gives up the
/// fight but not the Trinity Liger (`0x187` to `0x189`); the judge goes,
/// and the field's hook stages the Liger's destruction.
const FINAL_WON: &[Op] = &[
    Op::Call(SCENE_BRIGHTEN),
    Op::RestartMusic(DUEL_MUSIC),
    Op::Dialogue(0x187),
    Op::Dialogue(0x188),
    Op::Dialogue(0x189),
    glide16(1, (0x80, 0xC0), 2, true),
    Op::Place(1, OFF_THE_MAP),
    Op::Spawn(FIELD_HOOK, LIGER_STRUCK),
    Op::End,
];

/// Blood strikes the Trinity Liger (the hook at `0x0801B030`): battle scene
/// 2 to the Liger's song (`0x08012040`); then the arena brightens with the
/// Liger broken and Blood gets away (task at `0x0801B068`, `0x18A`).
const LIGER_STRUCK: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Music(LIGER_MUSIC),
    Op::Battle(2),
    Op::Music(DUEL_MUSIC),
    Op::Control(false),
    Op::Spawn(MAP_TASK, BLOOD_GETS_AWAY),
    Op::End,
];
const TRINITY_LIGER: usize = 5;
const BROKEN_LIGER: usize = 6;
const BLOOD_GETS_AWAY: &[Op] = &[
    Op::Place(TRINITY_LIGER, OFF_THE_MAP),
    Op::Place(BROKEN_LIGER, (8, 2)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x18A),
    Op::Spawn(FIELD_HOOK, LIGER_WAKES),
    Op::End,
];

/// The Liger stirs (the hook at `0x0801B0B0`): battle scene 3 to its song,
/// and Blood's team is gone; then the farewells (task at `0x0801B108`).
const LIGER_WAKES: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Music(LIGER_MUSIC),
    Op::Battle(3),
    Op::Music(DUEL_MUSIC),
    Op::Place(2, OFF_THE_MAP),
    Op::Place(3, OFF_THE_MAP),
    Op::Place(4, OFF_THE_MAP),
    Op::Control(false),
    Op::Spawn(MAP_TASK, FAREWELL),
    Op::End,
];

/// The arena with the party around the broken Liger (ROM `0x08669340`),
/// Dr. Tros's room with everyone (ROM `0x086693B8`), and the district with
/// the portal (ROM `0x0866946C`).
const LIGER_OBJECTS: u32 = 0x0866_9340;
const FAREWELL_OBJECTS: u32 = 0x0866_93B8;
const DEPARTURE_OBJECTS: u32 = 0x0866_946C;
/// The lists that leave as the party goes home.
const COLOSSEUM_LISTS: [u8; 9] = [0xD, 0xE, 0xF, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15];

const TROS_GIFT_OPS: &[Op] = &[
    Op::Gift(Some(Reward::Core(TROS_GIFT))),
    Op::Dialogue(0x1F),
    Op::Call(&CORE_REWARD),
    Op::Dialogue(0x22),
    Op::Gift(None),
];
const DEPARTURE_BRIGHTEN: &[Op] = &[Op::Call(SCENE_BRIGHTEN), Op::End];

/// The farewells (task at `0x0801B108`): the Liger is still alive (`0x18B`,
/// to the arena's song again); at Dr. Tros's everyone says goodbye
/// (`0x18C`) and Dr. Tros gives a Zoid core; the party drives off and
/// Team Blitz, Naomi, Harry's team, Bit and Ballad leave it (lists 13 to
/// 21); the hook at `0x0801B224` warps to map 205, chapter 6's first.
const FAREWELL: &[Op] = &[
    Op::LoadMap {
        map: ARENA,
        player: (8, 6),
        objects: LIGER_OBJECTS,
        count: 6,
    },
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::RestartMusic(DUEL_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x18B),
    Op::RestartMapMusic,
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: TROS_ROOM,
        player: (0xF, 0x14),
        objects: FAREWELL_OBJECTS,
        count: 9,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x18C),
    Op::Call(TROS_GIFT_OPS),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: DISTRICT,
        player: (0x21, 7),
        objects: DEPARTURE_OBJECTS,
        count: 2,
    },
    Op::Spawn(MAP_TASK + 1, DEPARTURE_BRIGHTEN),
    glide(PLAYER, (0x520, 0xE0), 1, true),
    Op::Leave(COLOSSEUM_LISTS[0]),
    Op::Leave(COLOSSEUM_LISTS[1]),
    Op::Leave(COLOSSEUM_LISTS[2]),
    Op::Leave(COLOSSEUM_LISTS[3]),
    Op::Leave(COLOSSEUM_LISTS[4]),
    Op::Leave(COLOSSEUM_LISTS[5]),
    Op::Leave(COLOSSEUM_LISTS[6]),
    Op::Leave(COLOSSEUM_LISTS[7]),
    Op::Leave(COLOSSEUM_LISTS[8]),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, TO_CHAPTER_6),
    Op::End,
];

/// The hook the chapter's end leaves (`0x0801B224`): chapter 6's first map
/// loads and brightens.
const TO_CHAPTER_6: &[Op] = &[
    Op::Warp {
        map: CHAPTER_6_START,
        cell: (8, 2),
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

const SOUTH_FIGHTS: [&[Op]; 5] = [
    fight!(10, &SOUTH_LOST, SOUTH_WINS[0], SOUTH_WON_0),
    fight!(11, &SOUTH_LOST, SOUTH_WINS[1], SOUTH_WON_1),
    fight!(12, &SOUTH_LOST, SOUTH_WINS[2], SOUTH_WON_2),
    fight!(13, &SOUTH_LOST, SOUTH_WINS[3], SOUTH_WON_3),
    fight!(14, &SOUTH_LOST, SOUTH_WINS[4], SOUTH_WON_4),
];
const EAST_FIGHTS: [&[Op]; 5] = [
    fight!(15, &EAST_LOST, EAST_WINS[0], EAST_WON_0),
    fight!(16, &EAST_LOST, EAST_WINS[1], EAST_WON_1),
    fight!(17, &EAST_LOST, EAST_WINS[2], EAST_WON_2),
    fight!(18, &EAST_LOST, EAST_WINS[3], EAST_WON_3),
    fight!(19, &EAST_LOST, EAST_WINS[4], EAST_WON_4),
];
const MAIN_FIGHTS: [&[Op]; 5] = [
    fight!(20, &MAIN_LOST, MAIN_WINS[0], MAIN_WON_0),
    fight!(21, &MAIN_LOST, MAIN_WINS[1], MAIN_WON_1),
    fight!(22, &MAIN_LOST, MAIN_WINS[2], MAIN_WON_2),
    fight!(23, &MAIN_LOST, MAIN_WINS[3], MAIN_WON_3),
    fight!(24, &MAIN_LOST, MAIN_WINS[4], FINAL_WON),
];

const SOUTH_MATCHES: [&[Op]; 5] = [
    colosseum_match!(0, 0x134, NOTHING, SOUTH_OPENINGS[0], SOUTH_FIGHTS[0]),
    colosseum_match!(1, 0x137, NOTHING, SOUTH_OPENINGS[1], SOUTH_FIGHTS[1]),
    colosseum_match!(2, 0x13B, NOTHING, SOUTH_OPENINGS[2], SOUTH_FIGHTS[2]),
    colosseum_match!(3, 0x13F, NOTHING, SOUTH_OPENINGS[3], SOUTH_FIGHTS[3]),
    colosseum_match!(4, 0x143, BEFORE_NAOMI, SOUTH_OPENINGS[4], SOUTH_FIGHTS[4]),
];
const EAST_MATCHES: [&[Op]; 5] = [
    colosseum_match!(5, 0x151, NOTHING, EAST_OPENINGS[0], EAST_FIGHTS[0]),
    colosseum_match!(6, 0x154, NOTHING, EAST_OPENINGS[1], EAST_FIGHTS[1]),
    colosseum_match!(7, 0x158, NOTHING, EAST_OPENINGS[2], EAST_FIGHTS[2]),
    colosseum_match!(8, 0x15C, NOTHING, EAST_OPENINGS[3], EAST_FIGHTS[3]),
    colosseum_match!(9, 0x160, BEFORE_HARRY, EAST_OPENINGS[4], EAST_FIGHTS[4]),
];
const MAIN_MATCHES: [&[Op]; 5] = [
    colosseum_match!(10, 0x173, NOTHING, MAIN_OPENINGS[0], MAIN_FIGHTS[0]),
    colosseum_match!(11, 0x177, NOTHING, MAIN_OPENINGS[1], MAIN_FIGHTS[1]),
    colosseum_match!(12, 0x17B, NOTHING, MAIN_OPENINGS[2], MAIN_FIGHTS[2]),
    colosseum_match!(13, 0x17F, NOTHING, MAIN_OPENINGS[3], MAIN_FIGHTS[3]),
    colosseum_match!(14, 0x183, BEFORE_FINAL, FINAL_OPENING, MAIN_FIGHTS[4]),
];

/// A desk's next match: while `flag` is not set, the match `n` starts when
/// the formation meets its regulation (the field's watch stopped first),
/// else `refused` explains the regulation.
macro_rules! next_match {
    ($flag:expr, $n:expr, $task:expr, $refused:expr, $later:expr) => {
        &[Op::IfFlags {
            all: &[],
            none: &[$flag],
            then: &[Op::IfRegulation {
                game: $n,
                then: &[Op::Stop(FIELD_WATCH), Op::Spawn(MAP_TASK, $task)],
                otherwise: $refused,
            }],
            otherwise: $later,
        }]
    };
}

const REFUSED: &[Op] = &[Op::Control(true)];
macro_rules! refused {
    ($dialogue:expr) => {
        &[Op::Dialogue($dialogue), Op::Control(true)]
    };
}

const SOUTH_DONE: &[Op] = &[Op::Dialogue(0x131), Op::Control(true)];
const SOUTH_4: &[Op] = next_match!(
    SOUTH_WINS[4],
    4,
    SOUTH_MATCHES[4],
    refused!(0x142),
    SOUTH_DONE
);
const SOUTH_3: &[Op] = next_match!(SOUTH_WINS[3], 3, SOUTH_MATCHES[3], refused!(0x13E), SOUTH_4);
const SOUTH_2: &[Op] = next_match!(SOUTH_WINS[2], 2, SOUTH_MATCHES[2], refused!(0x13A), SOUTH_3);
const SOUTH_1: &[Op] = next_match!(SOUTH_WINS[1], 1, SOUTH_MATCHES[1], REFUSED, SOUTH_2);
const SOUTH_0: &[Op] = next_match!(SOUTH_WINS[0], 0, SOUTH_MATCHES[0], REFUSED, SOUTH_1);

const EAST_DONE: &[Op] = &[Op::Dialogue(0x14E), Op::Control(true)];
const EAST_4: &[Op] = next_match!(EAST_WINS[4], 9, EAST_MATCHES[4], refused!(0x15F), EAST_DONE);
const EAST_3: &[Op] = next_match!(EAST_WINS[3], 8, EAST_MATCHES[3], refused!(0x15B), EAST_4);
const EAST_2: &[Op] = next_match!(EAST_WINS[2], 7, EAST_MATCHES[2], refused!(0x157), EAST_3);
const EAST_1: &[Op] = next_match!(EAST_WINS[1], 6, EAST_MATCHES[1], REFUSED, EAST_2);
const EAST_0: &[Op] = next_match!(EAST_WINS[0], 5, EAST_MATCHES[0], REFUSED, EAST_1);

const MAIN_DONE: &[Op] = &[Op::Control(true)];
const MAIN_4: &[Op] = next_match!(
    MAIN_WINS[4],
    14,
    MAIN_MATCHES[4],
    refused!(0x182),
    MAIN_DONE
);
const MAIN_3: &[Op] = next_match!(MAIN_WINS[3], 13, MAIN_MATCHES[3], refused!(0x17E), MAIN_4);
const MAIN_2: &[Op] = next_match!(MAIN_WINS[2], 12, MAIN_MATCHES[2], refused!(0x17A), MAIN_3);
const MAIN_1: &[Op] = next_match!(MAIN_WINS[1], 11, MAIN_MATCHES[1], refused!(0x176), MAIN_2);
const MAIN_0: &[Op] = next_match!(MAIN_WINS[0], 10, MAIN_MATCHES[0], REFUSED, MAIN_1);

/// The South dome's desk (`0x080191E4`): before entering, it offers the
/// tournament (`0x130`), which, taken, keeps the party in the dome and
/// makes it the return point; entered, it starts the next match; once the
/// dome is won it sends the party elsewhere (`0x131`).
const SOUTH_DESK_TALK: &[Op] = &[
    Op::Control(false),
    Op::IfFlags {
        all: &[],
        none: &[SOUTH_ENTERED],
        then: &[
            Op::Dialogue(0x130),
            Op::IfChoice {
                then: &[
                    Op::Flag(SOUTH_ENTERED, true),
                    Op::Spawn(FIELD_WATCH, SOUTH_WATCH),
                    Op::ReturnPoint(0xD),
                    Op::Control(true),
                ],
                otherwise: &[Op::Control(true)],
            },
        ],
        otherwise: SOUTH_0,
    },
];
/// The East dome's desk (`0x08019518`, `0x14D`, `0x14E`).
const EAST_DESK_TALK: &[Op] = &[
    Op::Control(false),
    Op::IfFlags {
        all: &[],
        none: &[EAST_ENTERED],
        then: &[
            Op::Dialogue(0x14D),
            Op::IfChoice {
                then: &[
                    Op::Flag(EAST_ENTERED, true),
                    Op::Spawn(FIELD_WATCH, EAST_WATCH),
                    Op::ReturnPoint(0xE),
                    Op::Control(true),
                ],
                otherwise: &[Op::Control(true)],
            },
        ],
        otherwise: EAST_0,
    },
];

/// Bit and Ballad at the Main dome (task at `0x0801AEDC`): the desk's room
/// loads with Team Blitz (ROM `0x0866928C`), Bit and Ballad come in and
/// Bit joins, Ballad too if taken (`0x16F`).
const MAIN_WELCOME_OBJECTS: u32 = 0x0866_928C;
const BIT: usize = 8;
const WELCOME_BALLAD: usize = 7;
const BALLAD_COMES_IN: &[Op] = &[
    Op::Place(WELCOME_BALLAD, (8, 0xF)),
    glide16(WELCOME_BALLAD, (0x80, 0x80), 1, true),
    Op::End,
];
const MAIN_WELCOME: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: MAIN_DESK,
        player: (8, 8),
        objects: MAIN_WELCOME_OBJECTS,
        count: 9,
    },
    Op::RestartMusic(COLOSSEUM_MUSIC),
    Op::Place(PLAYER, (8, 6)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, BALLAD_COMES_IN),
    Op::Wait(8),
    Op::Place(BIT, (8, 0xF)),
    glide16(BIT, (0x70, 0x80), 1, true),
    Op::Wait(15),
    Op::Face(PLAYER, Direction::Down),
    Op::Face(1, Direction::Down),
    Op::Face(2, Direction::Down),
    Op::Face(3, Direction::Down),
    Op::Face(4, Direction::Down),
    Op::Face(5, Direction::Down),
    Op::Face(6, Direction::Down),
    Op::Wait(15),
    Op::Dialogue(0x16F),
    Op::IfChoice {
        then: &[
            Op::Join(BIT_LIST),
            Op::Join(BALLAD_LIST),
            Op::Flag(BALLAD_JOINED, true),
        ],
        otherwise: &[Op::Join(BIT_LIST)],
    },
    Op::Call(&back_facing(MAIN_DESK, (8, 4), Direction::Right)),
];

/// The Main dome's desk (`0x080198E0`): once both other domes are won it
/// offers the final tournament (`0x16D`; the first time Bit and Ballad come
/// in), else it turns the party away (`0x16E`); entered, it starts the
/// next match.
const MAIN_DESK_TALK: &[Op] = &[
    Op::Control(false),
    Op::IfFlags {
        all: &[],
        none: &[MAIN_ENTERED],
        then: &[Op::IfFlags {
            all: &[SOUTH_WINS[4], EAST_WINS[4]],
            none: &[],
            then: &[
                Op::Dialogue(0x16D),
                Op::IfChoice {
                    then: &[
                        Op::IfFlags {
                            all: &[],
                            none: &[MAIN_WELCOMED],
                            then: &[
                                Op::Flag(MAIN_WELCOMED, true),
                                Op::Control(false),
                                Op::Spawn(MAP_TASK, MAIN_WELCOME),
                            ],
                            otherwise: &[Op::Control(true)],
                        },
                        Op::Flag(MAIN_ENTERED, true),
                        Op::ReturnPoint(0xF),
                    ],
                    otherwise: &[Op::Control(true)],
                },
            ],
            otherwise: &[Op::Dialogue(0x16E), Op::Control(true)],
        }],
        otherwise: MAIN_0,
    },
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

/// The teachers of area 5: deck commands 18, 5 and 30 (`0x08006B50` on),
/// and 20 (`0x08009570` through `0x08009430`).
const TEACHERS: [(u32, &[Op]); 4] = [
    (0x0800_6B50, teacher!(0x12, 0x3DD, 0x3F1)),
    (0x0800_6B68, teacher!(5, 0x3DE, 0x3F2)),
    (0x0800_6B80, teacher!(0x1E, 0x3DF, 0x3F3)),
    (0x0800_9570, teacher!(0x14, 0x2F6, 0x358)),
];

/// The keepers of area 5 (`0x080091F8` on): item shops 8 and 20,
/// armaments shops 9 to 16, and the labs 6 to 10.
const SHOPS: [(u32, &[Op]); 15] = [
    (0x0800_91F8, &shop(Shop::Items(8))),
    (0x0800_9204, &shop(Shop::Items(0x14))),
    (0x0800_9210, &shop(Shop::Arms(9))),
    (0x0800_921C, &shop(Shop::Arms(0xA))),
    (0x0800_9228, &shop(Shop::Arms(0xB))),
    (0x0800_9234, &shop(Shop::Arms(0xC))),
    (0x0800_9240, &shop(Shop::Arms(0xD))),
    (0x0800_924C, &shop(Shop::Arms(0xE))),
    (0x0800_9258, &shop(Shop::Arms(0xF))),
    (0x0800_9264, &shop(Shop::Arms(0x10))),
    (0x0800_9270, &shop(Shop::Lab(6))),
    (0x0800_927C, &shop(Shop::Lab(7))),
    (0x0800_9288, &shop(Shop::Lab(8))),
    (0x0800_9294, &shop(Shop::Lab(9))),
    (0x0800_92A0, &shop(Shop::Lab(0xA))),
];

/// What speaking to an object of area 5 whose script is code runs.
pub(super) fn talk_handler(address: u32) -> Option<&'static [Op]> {
    let listed = TEACHERS
        .iter()
        .chain(SHOPS.iter())
        .find(|(at, _)| *at == address)
        .map(|(_, program)| *program);
    listed.or(match address {
        0x0801_8FCC => Some(ULTRASAURUS_TALK),
        0x0801_91E4 => Some(SOUTH_DESK_TALK),
        0x0801_9518 => Some(EAST_DESK_TALK),
        0x0801_973C => Some(BALLAD_TALK),
        0x0801_98E0 => Some(MAIN_DESK_TALK),
        _ => None,
    })
}

/// What map `map` of area 5 runs when it loads, when it runs anything.
pub(super) fn map_handler(map: usize) -> Option<&'static [Op]> {
    match map {
        THRONE_ROOM => Some(THRONE_ROOM_ARRIVAL),
        DISTRICT => Some(DISTRICT_ARRIVAL),
        SOUTH_HALL => Some(SOUTH_HALL_ARRIVAL),
        EAST_HALL => Some(EAST_HALL_ARRIVAL),
        MAIN_HALL => Some(MAIN_HALL_ARRIVAL),
        _ => None,
    }
}
