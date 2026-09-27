//! Chapter 4: Gale's raids on the Empire's border bases in the time of
//! Van and Fiene, up to the fight at the research base Deme (area 4).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the
//! map handlers of area 4, the tasks and field hooks they install and the
//! objects' code, named at each item. See `docs/events.md`.

use extraction::saga::Reward;

use super::{
    FIELD_HOOK, PLAYER, SCENE_BRIGHTEN, SCENE_DARKEN, ZI_DATA_REWARD, glide, learn, shop,
    story_battle, through,
};
use crate::event::{FIELD_WATCH, MAP_TASK, Op};
use crate::field::{Direction, PIXEL};
use crate::menu::Shop;

/// Set once the Emperor has sent Gale through time (`0x080172F4`).
const CHAPTER_OPENED: u16 = 0x16D;
/// Set once the party has found the Ark base destroyed and met Gale.
const ARK_BASE_SEEN: u16 = 0x16E;
/// Set once Fiene has joined.
const FIENE_JOINED: u16 = 0x16F;
/// Set once Colonel Schwarz has joined at the Bego base.
const SCHWARZ_JOINED: u16 = 0x170;
/// Set once the party has reached the Zeta base too late.
const ZETA_REACHED: u16 = 0x171;
/// Set once Van has joined at the Zeta base.
const VAN_JOINED: u16 = 0x172;
/// Set once Irvine and Moonbay have pointed the party to the Deme base.
const MOONBAY_MET: u16 = 0x173;
/// Set once Schwarz has argued with the Deme base's guard.
const GUARD_ARGUED: u16 = 0x174;
/// Set once Thomas is beaten (story battle 8).
const THOMAS_BEATEN: u16 = 0x175;
/// Set once Hiltz and Reese have fled the Deme base.
const HILTZ_MET: u16 = 0x176;
/// Set once Gale is beaten (story battle 9).
const GALE_BEATEN: u16 = 0x177;

/// The Emperor's throne room (map 120), where chapter 3's end leaves the
/// party.
const THRONE_ROOM: usize = 120;
/// The castle's portal room (map 123) and the room above the base's bar
/// in the kingdom's time (129).
const PORTAL_ROOM: usize = 123;
const BASE_ROOM: usize = 129;
/// The border's plains, `mq0400` (map 94), a Zoid map with the portal.
const PLAINS: usize = 94;
/// The border town (95) and the house where Irvine and Moonbay rest (97).
const TOWN: usize = 95;
const MOONBAY_HOUSE: usize = 97;
/// The Ark base, destroyed (101), and the room where Fiene is found (103).
const ARK_BASE: usize = 101;
const FIENE_ROOM: usize = 103;
/// The Bego base (104) and its hall (105).
const BEGO_BASE: usize = 104;
const BEGO_HALL: usize = 105;
/// The Zeta base (106).
const ZETA_BASE: usize = 106;
/// The Deme base's gate (107), its side road after Thomas's fight (131),
/// its yard (111) and the passage where Hiltz and Reese wait (132).
const DEME_GATE: usize = 107;
const DEME_ROAD: usize = 131;
const DEME_YARD: usize = 111;
const DEME_PASSAGE: usize = 132;
/// Chapter 5's first map, where the chapter's end leaves the party.
const CHAPTER_5_START: usize = 165;

/// The songs the chapter switches to.
const DANGER_MUSIC: u16 = 4;
const GALE_MUSIC: u16 = 9;
const FRIENDS_MUSIC: u16 = 8;
const DUEL_MUSIC: u16 = 1;

const PORTAL_SOUND: u16 = 0x6F;
const PORTAL_GONE_SOUND: u16 = 0x44;
const PORTAL_ARRIVAL_SOUND: u16 = 0x49;
const BLAST_SOUND: u16 = 0x5B;
/// The portal's runs and the steps on whose last frame the one taken is
/// gone, the second sound plays, and the one brought is out.
const PORTAL_IDLE: usize = 0;
const PORTAL_TAKES: usize = 1;
const PORTAL_OPENS: usize = 2;
const PORTAL_TAKES_STEP: usize = 8;
const PORTAL_GONE_STEP: usize = 41;
const PORTAL_BRINGS_STEP: usize = 32;
/// The plains' portal (object 1 of its lists) and its cell.
const PLAINS_PORTAL: usize = 1;
const PLAINS_PORTAL_CELL: (usize, usize) = (47, 3);

/// Frames a task waits after the fade in before its first line.
const SETTLE: u32 = 32;
const OFF_THE_MAP: (usize, usize) = (0xFF, 0xFF);
const HIDDEN: (usize, usize) = (0xFFFF, 0xFFFF);
/// The prince standing again (`0x080089A0`).
const PRINCE_STANDING: usize = 0x98;
/// Moonbay's sprite as she turns to go (`0x080184BC`).
const MOONBAY_LEAVING: usize = 0xA6;
/// The Zi data Irvine gives (`0x08037A24` with `0x48`).
const IRVINE_GIFT: u8 = 0x48;
/// The characters of group 3, met as the chapter opens (`0x080099D8`).
const CHAPTER_GROUP: u8 = 3;

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
/// and the field brightens at once.
const BEATEN: &[Op] = &[Op::WarpHome, Op::FadeInHolding];

/// Scrolls the view a pixel down, then a frame.
const PAN_DOWN: &[Op] = &[Op::Pan(0, PIXEL), Op::Wait(1)];
/// Scrolls the view a pixel right, then a frame.
const PAN_RIGHT: &[Op] = &[Op::Pan(PIXEL, 0), Op::Wait(1)];

/// The throne room for the opening (ROM `0x0866815C`): the prince off the
/// map, the Emperor and Gale.
const THRONE_OBJECTS: u32 = 0x0866_815C;
const THRONE_GALE: usize = 2;
/// The portal room (ROM `0x08668198`): two soldiers and the portal.
const PORTAL_ROOM_OBJECTS: u32 = 0x0866_8198;
const FIRST_SOLDIER: usize = 1;
const SECOND_SOLDIER: usize = 2;
/// The room above the base's bar (ROM `0x086681E8`): the prince seated,
/// Regina, Jack and Earth off the map.
const BASE_OBJECTS: u32 = 0x0866_81E8;
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

/// The chapter's opening (task at `0x080173E0`): Gale answers the
/// Emperor's summons and counsels him, and the Emperor puts his loyalty to
/// the test (dialogues `0xFC`, `0xFD`); in the portal room the soldiers
/// sense the castle's device in use (`0xFE`); in the room above the bar
/// Jack brings the news and the party gets ready to follow (`0xFF`).
const OPENING: &[Op] = &[
    Op::Wait(SETTLE),
    glide16(THRONE_GALE, (0x80, 0x30), 1, true),
    Op::Dialogue(0xFC),
    glide16(THRONE_GALE, (0x80, 0xC0), 1, true),
    Op::Place(THRONE_GALE, OFF_THE_MAP),
    Op::Dialogue(0xFD),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PORTAL_ROOM,
        player: (4, 1),
        objects: PORTAL_ROOM_OBJECTS,
        count: 4,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    glide(FIRST_SOLDIER, (0x80, 0x60), 1, true),
    Op::Face(FIRST_SOLDIER, Direction::Up),
    Op::Wait(60),
    Op::Face(SECOND_SOLDIER, Direction::Left),
    Op::Repeat(64, PAN_DOWN),
    Op::Dialogue(0xFE),
    glide(FIRST_SOLDIER, (0x80, 0xE0), 2, true),
    Op::Place(FIRST_SOLDIER, OFF_THE_MAP),
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
    Op::Dialogue(0xFF),
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

/// The throne room (`0x080172F4`): once chapter 3 is over, the first time,
/// the characters of group 3 are met, the room loads with the Emperor and
/// Gale and the player off the map, and the opening runs.
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

/// The plains as the portal brings the party in (ROM `0x08667F54`): the
/// Gustav off the map and the portal; the same with Gale's Zoid in its
/// place for the Ark base and the plains after it (ROM `0x08668238`).
const PORTAL_ARRIVAL_OBJECTS: u32 = 0x0866_7F54;
const GALE_OBJECTS: u32 = 0x0866_8238;
const GALE_ZOID: usize = 1;
const PLAINS_ARRIVAL_CELL: (usize, usize) = (0x2E, 3);
/// Where the scene leaves the party on the plains.
const PLAINS_AFTER_GALE: (usize, usize) = (0x27, 3);

const GUSTAV_BACKS_AWAY: &[Op] = &[glide(PLAYER, (0xE0, 0x100), 2, false), Op::End];

/// Through the portal (task at `0x08017648`): the portal brings the Gustav
/// as in chapter 3; the party wonders where it is (`0x102`); three blasts
/// sound in the distance (`0x5B`) and the party heads for them (`0x103`);
/// the Ark base lies in ruins (`0x104`) and someone slips away (`0x105`); on
/// the plains Jack knows the stranger, Gale, his old instructor (`0x106`),
/// and blames himself for letting him go (`0x107`).
const PORTAL_ARRIVAL: &[Op] = &[
    Op::Wait(SETTLE),
    Op::PlayOnce(PLAINS_PORTAL, PORTAL_OPENS),
    Op::Sound(PORTAL_SOUND),
    Op::AwaitStepEnd(PLAINS_PORTAL, PORTAL_BRINGS_STEP),
    Op::Show(PLAYER),
    Op::Place(PLAYER, PLAINS_PORTAL_CELL),
    through(PLAYER, PLAINS_ARRIVAL_CELL, PIXEL, 1),
    Op::Sound(PORTAL_ARRIVAL_SOUND),
    Op::AwaitAnimation(PLAINS_PORTAL),
    Op::Animate(PLAINS_PORTAL, PORTAL_IDLE),
    Op::Wait(30),
    glide(PLAYER, (0x560, 0x40), 1, true),
    Op::Dialogue(0x102),
    Op::Sound(BLAST_SOUND),
    Op::AwaitSoundEnd(BLAST_SOUND),
    Op::Wait(30),
    Op::Sound(BLAST_SOUND),
    Op::AwaitSoundEnd(BLAST_SOUND),
    Op::Wait(15),
    Op::Sound(BLAST_SOUND),
    Op::AwaitSoundEnd(BLAST_SOUND),
    Op::Wait(6),
    Op::Dialogue(0x103),
    glide(PLAYER, (0x500, 0x40), 1, true),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: ARK_BASE,
        player: (7, 8),
        objects: GALE_OBJECTS,
        count: 2,
    },
    Op::Call(SCENE_BRIGHTEN),
    glide(PLAYER, (0xE0, 0xC0), 1, false),
    Op::Dialogue(0x104),
    Op::Place(GALE_ZOID, (0xB, 4)),
    glide(GALE_ZOID, (0x160, 0xA0), 2, true),
    glide(GALE_ZOID, (0xE0, 0x140), 2, true),
    Op::Place(GALE_ZOID, OFF_THE_MAP),
    Op::RestartMusic(DANGER_MUSIC),
    Op::Dialogue(0x105),
    Op::Spawn(MAP_TASK + 1, GUSTAV_BACKS_AWAY),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::LoadMap {
        map: PLAINS,
        player: PLAINS_AFTER_GALE,
        objects: GALE_OBJECTS,
        count: 2,
    },
    Op::RestartMusic(GALE_MUSIC),
    Op::Place(GALE_ZOID, (0x24, 3)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Wait(15),
    Op::Face(GALE_ZOID, Direction::Right),
    Op::Wait(15),
    Op::Dialogue(0x106),
    glide(GALE_ZOID, (0x440, 0x60), 2, true),
    Op::Place(GALE_ZOID, OFF_THE_MAP),
    Op::Dialogue(0x107),
    Op::Call(&back_to(PLAINS, PLAINS_AFTER_GALE)),
];

/// The plains (`0x08016D20`): the first time after the opening, they load
/// with the portal and the Gustav off the map, to the danger song, and the
/// portal brings the party.
const PLAINS_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CHAPTER_OPENED],
    none: &[ARK_BASE_SEEN],
    then: &[
        Op::LoadMap {
            map: PLAINS,
            player: (0x2F, 3),
            objects: PORTAL_ARRIVAL_OBJECTS,
            count: 2,
        },
        Op::Place(PLAYER, OFF_THE_MAP),
        Op::Control(false),
        Op::Spawn(MAP_TASK, PORTAL_ARRIVAL),
        Op::RestartMusic(DANGER_MUSIC),
        Op::Flag(ARK_BASE_SEEN, true),
    ],
    otherwise: &[],
}];

/// The room where Fiene is found (ROM `0x08668058`): the prince, Regina,
/// Jack and Earth off the map, and Fiene.
const FIENE_OBJECTS: u32 = 0x0866_8058;
const FIENE: usize = 4;
/// The list Fiene joins with.
const FIENE_LIST: u8 = 9;

const JACK_STEPS_UP: &[Op] = &[glide16(JACK, (0xA0, 0x40), 1, true), Op::End];
const EARTH_STEPS_UP: &[Op] = &[
    glide16(EARTH, (0xC0, 0x40), 1, true),
    Op::Face(EARTH, Direction::Right),
    Op::End,
];

/// Fiene (task at `0x08017938`): she looks through the ruins, the party
/// comes in to the song of friends and knows her (`0x108`); she tells of
/// the raids on the border's bases and the Guardian Force (`0x109` to
/// `0x10C`) and joins to reach the Bego base (`0x10D`, list 9).
const FIENE_SCENE: &[Op] = &[
    glide16(FIENE, (0x70, 0x50), 1, true),
    glide16(FIENE, (0x90, 0x30), 1, true),
    glide16(FIENE, (0x80, 0x30), 1, true),
    glide16(FIENE, (0x70, 0x40), 1, true),
    glide16(FIENE, (0x90, 0x40), 1, true),
    Op::Wait(SETTLE),
    Op::Place(PLAYER, (0xE, 2)),
    glide16(PLAYER, (0xE0, 0x40), 1, false),
    Op::Face(PLAYER, Direction::Left),
    Op::Place(EARTH, (0xE, 4)),
    glide16(EARTH, (0xD0, 0x50), 1, true),
    Op::Face(EARTH, Direction::Left),
    Op::Place(JACK, (0xE, 4)),
    glide16(JACK, (0xD0, 0x40), 1, true),
    Op::Face(JACK, Direction::Left),
    Op::Place(REGINA, (0xE, 4)),
    glide16(REGINA, (0xE0, 0x50), 1, true),
    Op::Face(REGINA, Direction::Left),
    Op::RestartMusic(FRIENDS_MUSIC),
    Op::Dialogue(0x108),
    Op::Spawn(MAP_TASK + 1, JACK_STEPS_UP),
    glide16(EARTH, (0x90, 0x50), 1, true),
    Op::Face(EARTH, Direction::Up),
    Op::Face(FIENE, Direction::Right),
    Op::Dialogue(0x109),
    glide16(JACK, (0xA0, 0x30), 1, true),
    Op::Face(JACK, Direction::Down),
    glide16(FIENE, (0xD0, 0x40), 1, true),
    Op::Face(FIENE, Direction::Right),
    glide16(EARTH, (0xA0, 0x40), 1, true),
    Op::Face(EARTH, Direction::Right),
    Op::Face(JACK, Direction::Right),
    Op::Face(FIENE, Direction::Left),
    Op::Dialogue(0x10A),
    Op::Spawn(MAP_TASK + 1, EARTH_STEPS_UP),
    glide16(JACK, (0xC0, 0x30), 1, true),
    Op::Face(EARTH, Direction::Right),
    Op::Face(FIENE, Direction::Right),
    Op::Dialogue(0x10B),
    glide16(REGINA, (0xC0, 0x50), 1, true),
    Op::Face(EARTH, Direction::Up),
    Op::Face(EARTH, Direction::Down),
    Op::Face(REGINA, Direction::Up),
    Op::Dialogue(0x10C),
    Op::Dialogue(0x10D),
    glide16(FIENE, (0xE0, 0x40), 1, true),
    Op::Place(FIENE, OFF_THE_MAP),
    glide16(REGINA, (0xE0, 0x40), 1, true),
    Op::Place(REGINA, OFF_THE_MAP),
    glide16(EARTH, (0xE0, 0x40), 1, true),
    Op::Place(EARTH, OFF_THE_MAP),
    glide16(JACK, (0xC0, 0x40), 1, true),
    glide16(JACK, (0xE0, 0x40), 1, true),
    Op::Place(JACK, OFF_THE_MAP),
    Op::Join(FIENE_LIST),
    Op::Call(&back_to(ARK_BASE, (0xB, 5))),
];

/// The room in the Ark base (`0x08016E78`): once the party has met Gale,
/// the first time, it loads with Fiene and the party off the map.
const FIENE_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[ARK_BASE_SEEN],
    none: &[FIENE_JOINED],
    then: &[
        Op::LoadMap {
            map: FIENE_ROOM,
            player: (8, 2),
            objects: FIENE_OBJECTS,
            count: 5,
        },
        Op::Place(PLAYER, HIDDEN),
        Op::Control(false),
        Op::Spawn(MAP_TASK, FIENE_SCENE),
        Op::Flag(FIENE_JOINED, true),
    ],
    otherwise: &[],
}];

/// The Bego base as the First Armoured Division closes in (ROM
/// `0x086680BC`): the Gustav, three of the division's Zoids off the map and
/// Schwarz's; its hall with the party, Fiene and Schwarz (ROM
/// `0x08668260`).
const BEGO_OBJECTS: u32 = 0x0866_80BC;
const DIVISION_1: usize = 1;
const DIVISION_2: usize = 2;
const DIVISION_3: usize = 3;
const SCHWARZ_ZOID: usize = 4;
const BEGO_HALL_OBJECTS: u32 = 0x0866_8260;
/// The list Schwarz joins with.
const SCHWARZ_LIST: u8 = 10;
/// The Bego base's return point: its hall.
const BEGO_RETURN_POINT: u8 = 0xB;

const DIVISION_2_CLOSES_IN: &[Op] = &[
    glide(DIVISION_2, (0x180, 0x180), 2, true),
    Op::Face(DIVISION_2, Direction::Left),
    Op::End,
];
const DIVISION_3_CLOSES_IN: &[Op] = &[
    glide(DIVISION_3, (0x160, 0x160), 2, true),
    Op::Face(DIVISION_3, Direction::Down),
    Op::End,
];

/// A division Zoid circles the Gustav for good, a cell at a time at four
/// pixels a frame, turning to it at each corner (`0x08017E90`, `0x08017F6C`,
/// `0x08018048`); each starts at another corner.
macro_rules! circling {
    ($actor:expr, $corners:expr, $facings:expr) => {
        &[Op::Loop(&[
            glide($actor, $corners[0], 4, true),
            glide($actor, $corners[1], 4, true),
            Op::Face($actor, $facings[0]),
            Op::Wait(10),
            glide($actor, $corners[2], 4, true),
            glide($actor, $corners[3], 4, true),
            Op::Face($actor, $facings[1]),
            Op::Wait(10),
            glide($actor, $corners[4], 4, true),
            glide($actor, $corners[5], 4, true),
            Op::Face($actor, $facings[2]),
            Op::Wait(10),
            glide($actor, $corners[6], 4, true),
            glide($actor, $corners[7], 4, true),
            Op::Face($actor, $facings[3]),
            Op::Wait(10),
        ])]
    };
}

const DIVISION_1_CIRCLES: &[Op] = circling!(
    DIVISION_1,
    [
        (0x140, 0x1A0),
        (0x160, 0x1A0),
        (0x180, 0x1A0),
        (0x180, 0x180),
        (0x180, 0x160),
        (0x160, 0x160),
        (0x140, 0x160),
        (0x140, 0x180),
    ],
    [
        Direction::Up,
        Direction::Left,
        Direction::Down,
        Direction::Right
    ]
);
const DIVISION_2_CIRCLES: &[Op] = circling!(
    DIVISION_2,
    [
        (0x180, 0x160),
        (0x160, 0x160),
        (0x140, 0x160),
        (0x140, 0x180),
        (0x140, 0x1A0),
        (0x160, 0x1A0),
        (0x180, 0x1A0),
        (0x180, 0x180),
    ],
    [
        Direction::Down,
        Direction::Right,
        Direction::Up,
        Direction::Left
    ]
);
const DIVISION_3_CIRCLES: &[Op] = circling!(
    DIVISION_3,
    [
        (0x140, 0x160),
        (0x140, 0x180),
        (0x140, 0x1A0),
        (0x160, 0x1A0),
        (0x180, 0x1A0),
        (0x180, 0x180),
        (0x180, 0x160),
        (0x160, 0x160),
    ],
    [
        Direction::Right,
        Direction::Up,
        Direction::Left,
        Direction::Down
    ]
);

/// The Bego base (task at `0x08017C3C`): the base is quiet and Fiene has
/// gone in alone (`0x10E`); the First Armoured Division surrounds the
/// party, Colonel Schwarz calls on it to surrender, and Fiene comes to the
/// rescue (`0x10F`); in the hall Schwarz hears of the raids and joins
/// (`0x110`, list 10).
const BEGO_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    glide(PLAYER, (0x160, 0x180), 1, false),
    Op::Dialogue(0x10E),
    Op::Face(PLAYER, Direction::Down),
    Op::Place(DIVISION_1, (0xB, 8)),
    Op::Place(DIVISION_2, (0xB, 8)),
    Op::Place(DIVISION_3, (0xB, 8)),
    Op::Spawn(MAP_TASK + 1, DIVISION_2_CLOSES_IN),
    Op::Spawn(MAP_TASK + 2, DIVISION_3_CLOSES_IN),
    glide(DIVISION_1, (0x140, 0x180), 2, true),
    Op::Face(DIVISION_1, Direction::Right),
    Op::Place(SCHWARZ_ZOID, (0xB, 8)),
    glide(SCHWARZ_ZOID, (0x160, 0x140), 1, true),
    Op::Face(SCHWARZ_ZOID, Direction::Down),
    Op::Spawn(MAP_TASK + 1, DIVISION_1_CIRCLES),
    Op::Spawn(MAP_TASK + 2, DIVISION_2_CIRCLES),
    Op::Spawn(MAP_TASK + 3, DIVISION_3_CIRCLES),
    Op::Wait(60),
    Op::RestartMusic(DANGER_MUSIC),
    Op::Dialogue(0x10F),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::Stop(MAP_TASK + 2),
    Op::Stop(MAP_TASK + 3),
    Op::LoadMap {
        map: BEGO_HALL,
        player: (0x19, 0x10),
        objects: BEGO_HALL_OBJECTS,
        count: 6,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x110),
    glide16(1, (0x190, 0x100), 1, true),
    Op::Place(1, OFF_THE_MAP),
    glide16(2, (0x190, 0x100), 1, true),
    Op::Place(2, OFF_THE_MAP),
    glide16(3, (0x190, 0x100), 1, true),
    Op::Place(3, OFF_THE_MAP),
    glide16(4, (0x190, 0x100), 1, true),
    Op::Place(4, OFF_THE_MAP),
    glide16(5, (0x190, 0x100), 1, true),
    Op::Place(5, OFF_THE_MAP),
    Op::Join(SCHWARZ_LIST),
    Op::Call(&back_to(BEGO_HALL, (0x19, 0x10))),
];

/// The Bego base (`0x08016EF4`): once Fiene has joined, the first time, it
/// loads with the division's Zoids off the map, and a beaten party is taken
/// to the base from then on.
const BEGO_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[FIENE_JOINED],
    none: &[SCHWARZ_JOINED],
    then: &[
        Op::LoadMap {
            map: BEGO_BASE,
            player: (0xB, 0xD),
            objects: BEGO_OBJECTS,
            count: 5,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, BEGO_SCENE),
        Op::ReturnPoint(BEGO_RETURN_POINT),
        Op::Flag(SCHWARZ_JOINED, true),
    ],
    otherwise: &[],
}];

/// The Zeta base as the party arrives (ROM `0x08668120`): the Gustav alone;
/// and with Van's Zoid once the party has looked around (ROM
/// `0x086682D8`).
const ZETA_OBJECTS: u32 = 0x0866_8120;
const VAN_OBJECTS: u32 = 0x0866_82D8;
const VAN_ZOID: usize = 1;
const VAN_ZOID_CELL: (usize, usize) = (0x14, 2);
/// The list Van joins with.
const VAN_LIST: u8 = 11;

/// The Zeta base (task at `0x08018124`): the view pans across the ruined
/// base and Schwarz sees they are too late (`0x111`).
const ZETA_SCENE: &[Op] = &[
    Op::Repeat(256, PAN_RIGHT),
    Op::Wait(SETTLE),
    Op::Dialogue(0x111),
    Op::Call(&back_to(ZETA_BASE, (0x10, 0xD))),
];

/// The Zeta base (`0x08016F74`): once Schwarz has joined, the first time,
/// it loads with the Gustav hidden and the view pans across it; later, until
/// Van has joined, his Zoid stands by the base.
const ZETA_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[SCHWARZ_JOINED],
    none: &[ZETA_REACHED],
    then: &[
        Op::LoadMap {
            map: ZETA_BASE,
            player: (0x10, 4),
            objects: ZETA_OBJECTS,
            count: 1,
        },
        Op::Place(PLAYER, HIDDEN),
        Op::Control(false),
        Op::Spawn(MAP_TASK, ZETA_SCENE),
        Op::Flag(ZETA_REACHED, true),
    ],
    otherwise: &[Op::IfFlags {
        all: &[ZETA_REACHED],
        none: &[VAN_JOINED],
        then: &[Op::Place(VAN_ZOID, VAN_ZOID_CELL)],
        otherwise: &[],
    }],
}];

/// Van (task at `0x08018188`): the base loads again with Van's Zoid, to the
/// song of friends; the party meets Van again and he joins (`0x112`, list
/// 11).
const VAN_SCENE: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: ZETA_BASE,
        player: (0x15, 2),
        objects: VAN_OBJECTS,
        count: 2,
    },
    Op::RestartMusic(FRIENDS_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x112),
    glide(VAN_ZOID, (0x2A0, 0x40), 1, true),
    Op::Place(VAN_ZOID, OFF_THE_MAP),
    Op::Join(VAN_LIST),
    Op::Call(&back_to(ZETA_BASE, (0x15, 2))),
];

/// Speaking to Van's Zoid (`0x08017014`).
const VAN_TALK: &[Op] = &[
    Op::Control(false),
    Op::Spawn(MAP_TASK, VAN_SCENE),
    Op::Flag(VAN_JOINED, true),
];

/// The house in the town (ROM `0x08667F7C`): the prince, Regina, Jack and
/// Earth off the map, Moonbay, Irvine, Van and Fiene off the map, and three
/// townsfolk.
const HOUSE_OBJECTS: u32 = 0x0866_7F7C;
const MOONBAY: usize = 4;
const IRVINE: usize = 5;
const VAN: usize = 6;
const HOUSE_FIENE: usize = 7;

const VAN_COMES_IN: &[Op] = &[
    Op::Place(VAN, (2, 8)),
    glide16(VAN, (0x20, 0x70), 1, true),
    glide16(VAN, (0x50, 0x50), 1, true),
    Op::Face(VAN, Direction::Up),
    Op::End,
];
const FIENE_COMES_IN: &[Op] = &[
    Op::Place(HOUSE_FIENE, (2, 8)),
    glide16(HOUSE_FIENE, (0x20, 0x70), 1, true),
    glide16(HOUSE_FIENE, (0x40, 0x50), 1, true),
    Op::Face(HOUSE_FIENE, Direction::Up),
    Op::End,
];
const PRINCE_COMES_IN: &[Op] = &[
    Op::Place(PLAYER, (2, 8)),
    glide16(PLAYER, (0x20, 0x70), 1, false),
    glide16(PLAYER, (0x50, 0x60), 1, false),
    Op::Face(PLAYER, Direction::Up),
    Op::End,
];
const REGINA_COMES_IN: &[Op] = &[
    Op::Place(REGINA, (2, 8)),
    glide16(REGINA, (0x20, 0x70), 1, true),
    glide16(REGINA, (0x40, 0x60), 1, true),
    Op::Face(REGINA, Direction::Up),
    Op::End,
];
const JACK_COMES_TO_THE_TABLE: &[Op] = &[
    Op::Place(JACK, (2, 8)),
    glide16(JACK, (0x20, 0x70), 1, true),
    glide16(JACK, (0x30, 0x60), 1, true),
    Op::Face(JACK, Direction::Up),
    Op::End,
];
const MOONBAY_LEAVES: &[Op] = &[
    Op::Sprite(MOONBAY, MOONBAY_LEAVING),
    glide16(MOONBAY, (0x70, 0x30), 1, true),
    glide16(MOONBAY, (0x70, 0x70), 1, true),
    glide16(MOONBAY, (0x20, 0x70), 1, true),
    glide16(MOONBAY, (0x20, 0x80), 1, true),
    Op::Place(MOONBAY, OFF_THE_MAP),
    Op::End,
];

/// Irvine's gift, announced as a chest's (`0x08037A24`): the message box
/// opens (dialogue `0x1F`), Ｚｉデータ「…」を手に入れた, and it closes (`0x22`).
const IRVINE_GIFT_OPS: &[Op] = &[
    Op::Gift(Some(Reward::ZiData(IRVINE_GIFT))),
    Op::Dialogue(0x1F),
    Op::Call(ZI_DATA_REWARD),
    Op::Dialogue(0x22),
    Op::Gift(None),
];

/// Irvine and Moonbay (task at `0x08018224`): the party comes into the
/// house one by one, Fiene knows the pair, and Moonbay tells of the three
/// raiders heading for the Deme base (`0x113`); Irvine gives the party a
/// Zoid's Zi data, the pair leaves, and the party sets off for Deme
/// (`0x114`).
const MOONBAY_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Spawn(MAP_TASK + 1, VAN_COMES_IN),
    Op::Wait(30),
    Op::Spawn(MAP_TASK + 2, FIENE_COMES_IN),
    Op::Wait(30),
    Op::Spawn(MAP_TASK + 3, PRINCE_COMES_IN),
    Op::Wait(30),
    Op::Spawn(MAP_TASK + 4, REGINA_COMES_IN),
    Op::Wait(30),
    Op::Spawn(MAP_TASK + 5, JACK_COMES_TO_THE_TABLE),
    Op::Wait(30),
    Op::Place(EARTH, (2, 8)),
    glide16(EARTH, (0x20, 0x70), 1, true),
    glide16(EARTH, (0x20, 0x60), 1, true),
    Op::Face(EARTH, Direction::Up),
    Op::Dialogue(0x113),
    Op::Call(IRVINE_GIFT_OPS),
    Op::Spawn(MAP_TASK + 1, MOONBAY_LEAVES),
    Op::Wait(60),
    glide16(IRVINE, (0x70, 0x70), 1, true),
    glide16(IRVINE, (0x20, 0x70), 1, true),
    glide16(IRVINE, (0x20, 0x80), 1, true),
    Op::Place(IRVINE, OFF_THE_MAP),
    Op::Wait(15),
    Op::Face(VAN, Direction::Down),
    Op::Face(HOUSE_FIENE, Direction::Down),
    Op::Wait(15),
    Op::Dialogue(0x114),
    Op::Call(&back_to(TOWN, (0xC, 5))),
];

/// The town's house (`0x08016DC4`): once Van has joined, the first time,
/// it loads with Irvine and Moonbay and the party off the map.
const HOUSE_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[VAN_JOINED],
    none: &[MOONBAY_MET],
    then: &[
        Op::LoadMap {
            map: MOONBAY_HOUSE,
            player: (2, 8),
            objects: HOUSE_OBJECTS,
            count: 11,
        },
        Op::Place(PLAYER, OFF_THE_MAP),
        Op::Control(false),
        Op::Spawn(MAP_TASK, MOONBAY_SCENE),
        Op::Flag(MOONBAY_MET, true),
    ],
    otherwise: &[],
}];

/// The Deme base's gate with Thomas's Dibison (ROM `0x08668134`).
const THOMAS_OBJECTS: u32 = 0x0866_8134;
/// The gate's guard (object 1 of the map), gone once Thomas is beaten.
const GATE_GUARD: usize = 1;
/// Thomas's story battle.
const THOMAS_BATTLE: u8 = 8;

const THOMAS_BATTLE_OPS: [Op; 4] = story_battle(THOMAS_BATTLE);

/// Thomas's battle (the hook at `0x08018550`): won, the party is taken to
/// the side road where Thomas comes to.
const THOMAS_FIGHT: &[Op] = &[
    Op::Call(&THOMAS_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::Flag(THOMAS_BEATEN, true),
            Op::Warp {
                map: DEME_ROAD,
                cell: (0x16, 0x18),
                facing: None,
            },
            Op::FadeInHoldingSlow,
        ],
    },
    Op::End,
];

/// Thomas (task at `0x0801851C`): Schwarz's brother, under Reese's spell,
/// attacks Van (`0x117`), and the fight follows from the field's hook.
const THOMAS_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Dialogue(0x117),
    Op::Spawn(FIELD_HOOK, THOMAS_FIGHT),
    Op::End,
];

/// The Deme base's gate (`0x08017048`): once Irvine and Moonbay have been
/// met and until Thomas is beaten, it loads with his Dibison to Gale's song
/// and he strikes; once he is beaten the guard is gone.
const GATE_ARRIVAL: &[Op] = &[
    Op::IfFlags {
        all: &[MOONBAY_MET],
        none: &[THOMAS_BEATEN],
        then: &[
            Op::LoadMap {
                map: DEME_GATE,
                player: (0xB, 0xD),
                objects: THOMAS_OBJECTS,
                count: 2,
            },
            Op::RestartMusic(GALE_MUSIC),
            Op::Place(PLAYER, (0xB, 0xC)),
            Op::Control(false),
            Op::Spawn(MAP_TASK, THOMAS_SCENE),
        ],
        otherwise: &[],
    },
    Op::IfFlags {
        all: &[THOMAS_BEATEN],
        none: &[],
        then: &[Op::Place(GATE_GUARD, HIDDEN)],
        otherwise: &[],
    },
];

/// The guard at the Deme base's gate (`0x080170E8`): no one passes
/// (`0x115`); once Schwarz is with the party he argues with the guard in
/// vain (`0x116`).
const GATE_GUARD_TALK: &[Op] = &[Op::IfFlags {
    all: &[SCHWARZ_JOINED],
    none: &[],
    then: &[
        Op::Control(false),
        Op::Dialogue(0x116),
        Op::Flag(GUARD_ARGUED, true),
        Op::Control(true),
    ],
    otherwise: &[Op::Control(false), Op::Dialogue(0x115), Op::Control(true)],
}];

/// Fiene and Thomas on the side road (objects 5 and 7 of the map's own
/// list).
const ROAD_FIENE: usize = 5;
const ROAD_THOMAS: usize = 7;

const THOMAS_LEADS_OFF: &[Op] = &[
    glide16(ROAD_THOMAS, (0x160, 0xF0), 1, true),
    Op::Place(ROAD_THOMAS, OFF_THE_MAP),
    Op::End,
];

/// Thomas comes to (task at `0x080185AC`): he denies being under Reese's
/// spell but tells where Hiltz and Reese went, and leads the way (`0x118`).
const ROAD_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Dialogue(0x118),
    Op::Spawn(MAP_TASK + 1, THOMAS_LEADS_OFF),
    glide16(ROAD_FIENE, (0x160, 0xF0), 1, true),
    Op::Place(ROAD_FIENE, OFF_THE_MAP),
    Op::Wait(1),
    Op::Call(&back_to(DEME_GATE, (0xB, 0xD))),
];

/// The side road (`0x08017138`), reached only from Thomas's fight: to the
/// song of friends, with the player placed, Thomas's scene runs.
const ROAD_ARRIVAL: &[Op] = &[
    Op::RestartMusic(FRIENDS_MUSIC),
    Op::Control(false),
    Op::Spawn(MAP_TASK, ROAD_SCENE),
    Op::Place(PLAYER, (0x16, 0x18)),
];

/// Where on the Deme base's yard the player's sprite has to stand for its
/// scenes (the hooks at `0x08017208` and `0x08017264`).
const YARD_SPOT_X: (i32, i32) = (0x540, 0x560);
const YARD_SPOT_Y: (i32, i32) = (0x200, 0x200);

/// The passage with Hiltz and Reese (ROM `0x08668300`): the prince,
/// Regina, Jack, Earth, Van, Fiene, Schwarz and Thomas, and the raiders,
/// Hiltz, Reese and their Organoids (sprites `0xBB`, `0xAD`, `0xCE` and
/// `0xCD`).
const PASSAGE_OBJECTS: u32 = 0x0866_8300;
const PASSAGE_REGINA: usize = 1;
const PASSAGE_JACK: usize = 2;
const PASSAGE_EARTH: usize = 3;
const PASSAGE_VAN: usize = 4;
const PASSAGE_FIENE: usize = 5;
const PASSAGE_SCHWARZ: usize = 6;
const PASSAGE_THOMAS: usize = 7;
const RAIDER_1: usize = 8;
const RAIDER_2: usize = 9;
const RAIDER_3: usize = 10;
const RAIDER_4: usize = 11;
/// The lists that leave after the raiders' escape: Fiene, Schwarz, Van
/// and Thomas.
const DEME_LEAVERS: [u8; 4] = [9, 0xA, 0xB, 0xC];
/// Where the raiders and those who chase them leave the passage.
const PASSAGE_EXIT: (i32, i32) = (0x5C0, 0x2D0);

const PRINCE_STEPS_IN: &[Op] = &[glide16(PLAYER, (0x550, 0x280), 1, false), Op::End];
const PASSAGE_JACK_IN: &[Op] = &[glide16(PASSAGE_JACK, (0x540, 0x270), 1, true), Op::End];
const PASSAGE_EARTH_IN: &[Op] = &[glide16(PASSAGE_EARTH, (0x530, 0x270), 1, true), Op::End];
const PASSAGE_VAN_IN: &[Op] = &[glide16(PASSAGE_VAN, (0x560, 0x280), 1, true), Op::End];
const PASSAGE_FIENE_IN: &[Op] = &[glide16(PASSAGE_FIENE, (0x570, 0x280), 1, true), Op::End];
const PASSAGE_SCHWARZ_IN: &[Op] = &[glide16(PASSAGE_SCHWARZ, (0x560, 0x270), 1, true), Op::End];
const RAIDER_3_STEPS_UP: &[Op] = &[
    glide16(RAIDER_3, (0x540, 0x2A0), 2, true),
    Op::Face(RAIDER_3, Direction::Up),
    Op::End,
];
const RAIDER_1_GOES: &[Op] = &[
    glide16(RAIDER_1, (0x550, 0x2C0), 2, true),
    glide16(RAIDER_1, PASSAGE_EXIT, 2, true),
    Op::Place(RAIDER_1, OFF_THE_MAP),
    Op::End,
];
const RAIDER_4_GOES: &[Op] = &[
    glide16(RAIDER_4, PASSAGE_EXIT, 2, true),
    Op::Place(RAIDER_4, OFF_THE_MAP),
    Op::End,
];
const VAN_RUNS_AFTER: &[Op] = &[
    glide16(PASSAGE_VAN, (0x560, 0x2C0), 2, true),
    glide16(PASSAGE_VAN, PASSAGE_EXIT, 2, true),
    Op::Place(PASSAGE_VAN, OFF_THE_MAP),
    Op::End,
];
const FIENE_RUNS_AFTER: &[Op] = &[
    glide16(PASSAGE_FIENE, (0x570, 0x2C0), 2, true),
    glide16(PASSAGE_FIENE, PASSAGE_EXIT, 2, true),
    Op::Place(PASSAGE_FIENE, OFF_THE_MAP),
    Op::End,
];

/// Hiltz and Reese (task at `0x08018650`): the passage loads with everyone,
/// to Gale's song; Hiltz and Reese mock the party and slip away (`0x119`);
/// Thomas gives chase (`0x11A`), Van, Fiene and Schwarz follow (`0x11B`) and
/// leave the party with Thomas (lists 9 to 12); the others set off after
/// them (`0x11C`).
const HILTZ_SCENE: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: DEME_PASSAGE,
        player: (0x55, 0x2A),
        objects: PASSAGE_OBJECTS,
        count: 12,
    },
    Op::RestartMusic(GALE_MUSIC),
    Op::Place(PLAYER, (0x55, 0x23)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, PRINCE_STEPS_IN),
    Op::Spawn(MAP_TASK + 2, PASSAGE_JACK_IN),
    Op::Spawn(MAP_TASK + 3, PASSAGE_EARTH_IN),
    glide16(PASSAGE_REGINA, (0x540, 0x280), 1, true),
    Op::Wait(8),
    Op::Spawn(MAP_TASK + 1, PASSAGE_VAN_IN),
    Op::Spawn(MAP_TASK + 2, PASSAGE_FIENE_IN),
    Op::Spawn(MAP_TASK + 3, PASSAGE_SCHWARZ_IN),
    glide16(PASSAGE_THOMAS, (0x570, 0x270), 1, true),
    Op::Wait(8),
    Op::Dialogue(0x119),
    Op::Spawn(MAP_TASK + 1, RAIDER_3_STEPS_UP),
    glide16(RAIDER_4, (0x570, 0x2A0), 1, true),
    Op::Face(RAIDER_4, Direction::Up),
    Op::Spawn(MAP_TASK + 1, RAIDER_1_GOES),
    Op::Wait(30),
    glide16(RAIDER_2, (0x560, 0x2C0), 2, true),
    glide16(RAIDER_2, PASSAGE_EXIT, 2, true),
    Op::Place(RAIDER_2, OFF_THE_MAP),
    Op::Wait(15),
    Op::Spawn(MAP_TASK + 1, RAIDER_4_GOES),
    glide16(RAIDER_3, PASSAGE_EXIT, 2, true),
    Op::Place(RAIDER_3, OFF_THE_MAP),
    Op::Dialogue(0x11A),
    glide16(PASSAGE_THOMAS, (0x580, 0x270), 1, true),
    glide16(PASSAGE_THOMAS, PASSAGE_EXIT, 2, true),
    Op::Place(PASSAGE_THOMAS, OFF_THE_MAP),
    Op::Dialogue(0x11B),
    Op::Spawn(MAP_TASK + 1, VAN_RUNS_AFTER),
    Op::Spawn(MAP_TASK + 2, FIENE_RUNS_AFTER),
    glide16(PASSAGE_SCHWARZ, (0x560, 0x2C0), 2, true),
    glide16(PASSAGE_SCHWARZ, PASSAGE_EXIT, 2, true),
    Op::Place(PASSAGE_SCHWARZ, OFF_THE_MAP),
    Op::Leave(DEME_LEAVERS[0]),
    Op::Leave(DEME_LEAVERS[1]),
    Op::Leave(DEME_LEAVERS[2]),
    Op::Leave(DEME_LEAVERS[3]),
    Op::Dialogue(0x11C),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, TO_THE_YARD),
    Op::End,
];

/// The hook Hiltz's scene leaves (`0x0801889C`): the yard loads and
/// brightens.
const TO_THE_YARD: &[Op] = &[
    Op::Warp {
        map: DEME_YARD,
        cell: (0x2A, 0x10),
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// The plains with the portal alone, for the way home (ROM `0x08668454`).
const FAREWELL_OBJECTS: u32 = 0x0866_8454;

/// The yard before Gale (ROM `0x086683F0`): the Gustav, Gale's Zoid, and
/// Van's, Thomas's and Schwarz's off the map.
const GALE_YARD_OBJECTS: u32 = 0x0866_83F0;
const YARD_GALE: usize = 1;
const YARD_VAN: usize = 2;
const YARD_THOMAS: usize = 3;
const YARD_SCHWARZ: usize = 4;
const YARD_ENTRANCE: (usize, usize) = (0x2E, 0x17);
/// Gale's story battle.
const GALE_BATTLE: u8 = 9;

/// Gale (task at `0x08018AA8`): the yard loads to the danger song; Gale
/// waits for Jack (`0x11D`) and, to the duel's song, tells of the Emperor's
/// aim and challenges him (`0x11E`); the fight follows from the field's
/// hook.
const GALE_SCENE: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: DEME_YARD,
        player: (0x2A, 0x10),
        objects: GALE_YARD_OBJECTS,
        count: 5,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    glide(PLAYER, (0x560, 0x2A0), 1, true),
    Op::Place(YARD_GALE, YARD_ENTRANCE),
    Op::Wait(30),
    glide(YARD_GALE, (0x580, 0x2A0), 2, true),
    Op::Face(YARD_GALE, Direction::Left),
    Op::Dialogue(0x11D),
    Op::RestartMusic(DUEL_MUSIC),
    Op::Dialogue(0x11E),
    Op::Spawn(FIELD_HOOK, GALE_FIGHT),
    Op::End,
];

const GALE_BATTLE_OPS: [Op; 4] = story_battle(GALE_BATTLE);

/// Gale's battle (the hook at `0x08018DB0`): won, the field brightens at
/// once and the chapter's end runs.
const GALE_FIGHT: &[Op] = &[
    Op::Call(&GALE_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::FadeInHolding,
            Op::Flag(GALE_BEATEN, true),
            Op::Control(false),
            Op::Spawn(MAP_TASK, FAREWELL),
        ],
    },
    Op::End,
];

const VAN_RETURNS: &[Op] = &[
    Op::Place(YARD_VAN, YARD_ENTRANCE),
    glide(YARD_VAN, (0x580, 0x280), 1, true),
    Op::Face(YARD_VAN, Direction::Left),
    Op::End,
];
const THOMAS_RETURNS: &[Op] = &[
    Op::Place(YARD_THOMAS, YARD_ENTRANCE),
    glide(YARD_THOMAS, (0x580, 0x2A0), 1, true),
    Op::Face(YARD_THOMAS, Direction::Left),
    Op::End,
];
const VAN_GOES: &[Op] = &[
    glide(YARD_VAN, (0x580, 0x240), 2, true),
    Op::Place(YARD_VAN, OFF_THE_MAP),
    Op::End,
];
const THOMAS_GOES: &[Op] = &[
    glide(YARD_THOMAS, (0x580, 0x240), 2, true),
    Op::Place(YARD_THOMAS, OFF_THE_MAP),
    Op::End,
];
const FAREWELL_BRIGHTEN: &[Op] = &[Op::Call(SCENE_BRIGHTEN), Op::End];

/// The chapter's end (task at `0x08018B74`): Gale gives way (`0x11F`) and
/// drives off, Jack calling after him (`0x120`); Van, Thomas and Schwarz
/// come back empty-handed and go their way (`0x121`); the party goes home
/// (`0x122`) through the plains' portal, and the hook at `0x08018D80` warps
/// to map 165, chapter 5's first.
const FAREWELL: &[Op] = &[
    Op::Dialogue(0x11F),
    glide(YARD_GALE, (0x580, 0x240), 1, true),
    glide(YARD_GALE, (0x560, 0x1E0), 1, true),
    Op::Place(YARD_GALE, OFF_THE_MAP),
    Op::Dialogue(0x120),
    Op::Spawn(MAP_TASK + 1, VAN_RETURNS),
    Op::Wait(SETTLE),
    Op::Spawn(MAP_TASK + 2, THOMAS_RETURNS),
    Op::Wait(SETTLE),
    Op::Place(YARD_SCHWARZ, YARD_ENTRANCE),
    glide(YARD_SCHWARZ, (0x5A0, 0x2A0), 1, true),
    Op::Face(YARD_SCHWARZ, Direction::Left),
    Op::Dialogue(0x121),
    Op::Spawn(MAP_TASK + 1, VAN_GOES),
    Op::Spawn(MAP_TASK + 2, THOMAS_GOES),
    glide(YARD_SCHWARZ, (0x5A0, 0x240), 2, true),
    Op::Place(YARD_SCHWARZ, OFF_THE_MAP),
    Op::Dialogue(0x122),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PLAINS,
        player: (0x28, 3),
        objects: FAREWELL_OBJECTS,
        count: 2,
    },
    Op::Spawn(MAP_TASK + 1, FAREWELL_BRIGHTEN),
    glide(PLAYER, (0x5C0, 0x60), 1, true),
    through(PLAYER, PLAINS_PORTAL_CELL, PIXEL, 1),
    Op::PlayOnce(PLAINS_PORTAL, PORTAL_TAKES),
    Op::Sound(PORTAL_SOUND),
    Op::AwaitStepEnd(PLAINS_PORTAL, PORTAL_TAKES_STEP),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::AwaitStepEnd(PLAINS_PORTAL, PORTAL_GONE_STEP),
    Op::Sound(PORTAL_GONE_SOUND),
    Op::AwaitAnimation(PLAINS_PORTAL),
    Op::Animate(PLAINS_PORTAL, PORTAL_IDLE),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, TO_CHAPTER_5),
    Op::End,
];

/// The hook the chapter's end leaves (`0x08018D80`): chapter 5's first map
/// loads and brightens.
const TO_CHAPTER_5: &[Op] = &[
    Op::Warp {
        map: CHAPTER_5_START,
        cell: (8, 2),
        facing: None,
    },
    Op::FadeInHoldingSlow,
    Op::End,
];

/// The Deme base's yard watches (the hooks `0x08017208` and `0x08017264`):
/// once the player's sprite stands on the spot, the first time Hiltz's
/// scene starts; after it, until Gale is beaten, Gale's.
const HILTZ_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: YARD_SPOT_X,
        y: YARD_SPOT_Y,
    },
    Op::Control(false),
    Op::Spawn(MAP_TASK, HILTZ_SCENE),
    Op::Flag(HILTZ_MET, true),
    Op::End,
];
const GALE_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: YARD_SPOT_X,
        y: YARD_SPOT_Y,
    },
    Op::Control(false),
    Op::Spawn(MAP_TASK, GALE_SCENE),
    Op::End,
];

/// The Deme base's yard (`0x080171AC`): once Thomas is beaten it watches for
/// Hiltz's scene, then for Gale's.
const YARD_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[THOMAS_BEATEN],
    none: &[HILTZ_MET],
    then: &[Op::Spawn(FIELD_WATCH, HILTZ_WATCH)],
    otherwise: &[Op::IfFlags {
        all: &[HILTZ_MET],
        none: &[GALE_BEATEN],
        then: &[Op::Spawn(FIELD_WATCH, GALE_WATCH)],
        otherwise: &[],
    }],
}];

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

/// The teachers of area 4: deck commands 25, 6, 27, 31, 28 and 14
/// (`0x08006AC8` on), and 32 (`0x08009600` through `0x08009430`).
const TEACHERS: [(u32, &[Op]); 7] = [
    (0x0800_6AC8, teacher!(0x19, 0x329, 0x32A)),
    (0x0800_6AE0, teacher!(6, 0x3D8, 0x3EC)),
    (0x0800_6AF4, teacher!(0x1B, 0x3D9, 0x3ED)),
    (0x0800_6B0C, teacher!(0x1F, 0x3DA, 0x3EE)),
    (0x0800_6B24, teacher!(0x1C, 0x3DB, 0x3EF)),
    (0x0800_6B3C, teacher!(0xE, 0x3DC, 0x3F0)),
    (0x0800_9600, teacher!(0x20, 0x2F5, 0x317)),
];

/// The keepers of area 4 (`0x08009198` on): item shops 5, 6 and 7,
/// armaments shops 6, 7 and 8, and the labs 5 and 19.
const SHOPS: [(u32, &[Op]); 8] = [
    (0x0800_9198, &shop(Shop::Items(5))),
    (0x0800_91A4, &shop(Shop::Items(6))),
    (0x0800_91B0, &shop(Shop::Items(7))),
    (0x0800_91BC, &shop(Shop::Arms(6))),
    (0x0800_91C8, &shop(Shop::Arms(7))),
    (0x0800_91D4, &shop(Shop::Arms(8))),
    (0x0800_91E0, &shop(Shop::Lab(5))),
    (0x0800_91EC, &shop(Shop::Lab(0x13))),
];

/// What speaking to an object of area 4 whose script is code runs.
pub(super) fn talk_handler(address: u32) -> Option<&'static [Op]> {
    let listed = TEACHERS
        .iter()
        .chain(SHOPS.iter())
        .find(|(at, _)| *at == address)
        .map(|(_, program)| *program);
    listed.or(match address {
        0x0801_7014 => Some(VAN_TALK),
        0x0801_70E8 => Some(GATE_GUARD_TALK),
        _ => None,
    })
}

/// What map `map` of area 4 runs when it loads, when it runs anything.
pub(super) fn map_handler(map: usize) -> Option<&'static [Op]> {
    match map {
        THRONE_ROOM => Some(THRONE_ROOM_ARRIVAL),
        PLAINS => Some(PLAINS_ARRIVAL),
        FIENE_ROOM => Some(FIENE_ROOM_ARRIVAL),
        BEGO_BASE => Some(BEGO_ARRIVAL),
        ZETA_BASE => Some(ZETA_ARRIVAL),
        MOONBAY_HOUSE => Some(HOUSE_ARRIVAL),
        DEME_GATE => Some(GATE_ARRIVAL),
        DEME_ROAD => Some(ROAD_ARRIVAL),
        DEME_YARD => Some(YARD_ARRIVAL),
        _ => None,
    }
}
