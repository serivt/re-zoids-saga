//! Chapter 9: the Emperor wakes a Zoid core and fires on Arcadia Castle;
//! the party gathers at the base, picks two companions for the assault,
//! fights through the occupied castle (story battles 36 to 38: Fran, Gale
//! and Opis) and faces the Emperor at its heart (39 and 40); the overloaded
//! space-time transfer device takes everyone home, and the staff roll
//! (area 9).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the map
//! handlers of area 9 (`0x08025468` to `0x080279C0`), the tasks they spawn,
//! the objects' code and the routines named below. See `docs/events.md`.

use super::{FADE_IN, FADE_OUT, HELPER_TASK, PLAYER, learn, shop, story_battle, through, walk};
use crate::event::{HERE, MAP_TASK, Op};
use crate::field::{Direction, PIXEL};
use crate::menu::Shop;

/// Set once the chapter's opening has played (`0x08025468`).
const CHAPTER_OPENED: u16 = 0x12F;
/// Set once the party is ready to set out from the briefing room (Regina's
/// question, `0x0802672C`).
const SET_OUT: u16 = 0x130;
/// Set once Jack's briefing has played.
const BASE_BRIEFED: u16 = 0x131;
/// Set once each of the castle's guardians is beaten: Fran, Gale and
/// Opis (story battles 36 to 38, maps 279, 283 and 285).
const FRAN_BEATEN: u16 = 0x132;
const GALE_BEATEN: u16 = 0x133;
const OPIS_BEATEN: u16 = 0x134;
/// Set once the Emperor's first Zoid is beaten (story battle 39), and once
/// his last one is (40, map 296).
const FIRST_ZOID_BEATEN: u16 = 0x135;
const EMPEROR_BEATEN: u16 = 0x136;

/// The maps of area 9 with a story: the Emperor's hall (281, loaded as
/// 282), the castle's rooms where Fran, Gale and Opis wait (279 loaded as
/// 280, 283, 285), the base's lookout and briefing room (289, 295), the
/// castle's heart (296), and the rooms the ending loads (276, 277).
const FRAN_ROOM: usize = 279;
const FRAN_ROOM_SCENE: usize = 280;
const HALL: usize = 281;
const HALL_SCENE: usize = 282;
const GALE_ROOM: usize = 283;
const GALE_ROOM_EXIT: usize = 284;
const OPIS_ROOM: usize = 285;
const LOOKOUT: usize = 289;
const BRIEFING_ROOM: usize = 295;
const HEART: usize = 296;
const GATEWAY: usize = 276;
const HOMECOMING: usize = 277;
/// Chapter 10's first map, where the chapter's end leaves the party.
const CHAPTER_10_START: usize = 340;

/// The songs the chapter switches to.
const ALERT_MUSIC: u16 = 4;
const ENEMY_MUSIC: u16 = 9;
const STANDOFF_MUSIC: u16 = 0x14;
const EMPEROR_MUSIC: u16 = 6;
const PORTAL_MUSIC: u16 = 5;
const FAREWELL_MUSIC: u16 = 0xF;
const HOME_MUSIC: u16 = 3;

const CORE_SOUND: u16 = 0x62;
const ALARM_SOUND: u16 = 0x6B;
const FLASH_SOUND: u16 = 0x81;
const PORTAL_SOUND: u16 = 0x6F;
const WARP_SOUND: u16 = 0x5A;
const DOOR_SOUND: u16 = 0x82;
const BLAST_SOUND: u16 = 0x5B;
const CANNON_SOUND: u16 = 0x85;
const JUMP_SOUND: u16 = 0x6D;
const CLOSING_SOUND: u16 = 0x4C;
const OPENING_SOUND: u16 = 0x7F;

/// The groups of characters met as the chapter opens (`0x08009A14`) and
/// as it ends (`0x08009A20`).
const CHAPTER_GROUP: u8 = 8;
const NEXT_GROUP: u8 = 9;

const OFF_THE_MAP: (usize, usize) = (0xFF, 0xFF);
/// A walk a pixel a frame, and two, the scenes' paces (entity command
/// 11, through everything).
const PACE: i32 = PIXEL;
const HURRY: i32 = 2 * PIXEL;

/// Walks actor `actor` to `cell` through everything at `speed` and waits
/// for it (entity command 11).
const fn goes(actor: usize, cell: (usize, usize), speed: i32, shift: i8) -> [Op; 2] {
    [through(actor, cell, speed, shift), Op::AwaitArrival(actor)]
}

/// The same at a pixel a frame.
const fn steps(actor: usize, cell: (usize, usize)) -> [Op; 2] {
    goes(actor, cell, PACE, 1)
}

/// The same at two pixels a frame.
const fn hurries(actor: usize, cell: (usize, usize)) -> [Op; 2] {
    goes(actor, cell, HURRY, 2)
}

/// Scrolls the view a pixel a frame (`0x08008324` in a loop).
const PAN_UP: &[Op] = &[Op::Pan(0, -PIXEL), Op::Wait(1)];
const PAN_DOWN: &[Op] = &[Op::Pan(0, PIXEL), Op::Wait(1)];
/// Two pixels a frame.
const PAN_DOWN_FAST: &[Op] = &[Op::Pan(0, 2 * PIXEL), Op::Wait(1)];

/// The darkening and brightening halves of a flash (`0x0800C6E8`,
/// `0x0800C6C8`), ten frames each.
const DARKEN: &[Op] = &[Op::FadeOut(1), Op::End];
const BRIGHTEN: &[Op] = &[Op::FadeIn(1), Op::End];
const FLASH: &[Op] = &[
    Op::Spawn(HELPER_TASK, DARKEN),
    Op::Wait(10),
    Op::Spawn(HELPER_TASK, BRIGHTEN),
    Op::Wait(10),
];

/// The slow whitening (`0x080016A8` with 7): `BLDCNT` brightens every layer
/// toward white, and the fade level goes up a step every eighth frame from
/// 0 to 31, the task holding; then `0x0800196C` turns it to black at once.
const WHITE_OUT: [Op; 68] = white_out();

const fn white_out() -> [Op; 68] {
    let mut ops = [Op::Wait(8); 68];
    ops[0] = Op::FadeToWhite(true);
    ops[1] = Op::Brightness(0);
    let mut level: u8 = 0;
    while level < 32 {
        ops[3 + 2 * level as usize] = Op::Brightness(level);
        level += 1;
    }
    ops[66] = Op::FadeToWhite(false);
    ops[67] = Op::Brightness(super::BLACK);
    ops
}

/// The Emperor's hall for the opening (ROM `0x0832B010`): the Emperor,
/// Fran, a guard left off the map, Opis at the Zoid core, and Gale.
const HALL_OBJECTS: u32 = 0x0832_B010;
const EMPEROR: usize = 0;
const FRAN: usize = 1;
const OPIS: usize = 3;
const GALE: usize = 4;

/// Opis at work (task at `0x08025E6C`): after three seconds he walks the
/// hall's aisles, then paces the top row, pausing half a second at each
/// cell, until the opening stops him.
const OPIS_AT_WORK: &[Op] = &[
    Op::Wait(180),
    Op::Call(&hurries(OPIS, (4, 1))),
    Op::Call(&hurries(OPIS, (4, 7))),
    Op::Call(&hurries(OPIS, (0x10, 7))),
    Op::Call(&hurries(OPIS, (0x10, 1))),
    Op::Call(&hurries(OPIS, (0x12, 1))),
    Op::Call(&hurries(OPIS, (0x10, 1))),
    Op::Call(&hurries(OPIS, (0x10, 7))),
    Op::Call(&hurries(OPIS, (4, 7))),
    Op::Call(&hurries(OPIS, (4, 1))),
    Op::Call(&hurries(OPIS, (2, 1))),
    Op::Loop(&[
        Op::Call(&hurries(OPIS, (5, 1))),
        Op::Face(OPIS, Direction::Up),
        Op::Wait(30),
        Op::Call(&hurries(OPIS, (4, 1))),
        Op::Face(OPIS, Direction::Up),
        Op::Wait(30),
        Op::Call(&hurries(OPIS, (3, 1))),
        Op::Face(OPIS, Direction::Up),
        Op::Wait(30),
        Op::Call(&hurries(OPIS, (4, 1))),
        Op::Face(OPIS, Direction::Up),
        Op::Wait(30),
    ]),
];

/// The lookout (ROM `0x0832B074`): the party, two soldiers below the map
/// and the portal.
const LOOKOUT_OBJECTS: u32 = 0x0832_B074;
const FIRST_SOLDIER: usize = 4;
const SECOND_SOLDIER: usize = 5;
/// The briefing room for the opening (ROM `0x0832B100`): Regina, Earth
/// and Jack off the map, and the Emperor's image.
const BRIEFING_OBJECTS: u32 = 0x0832_B100;
const REGINA: usize = 1;
const EARTH: usize = 2;
const JACK: usize = 3;
const EMPEROR_IMAGE: usize = 4;
const EMPEROR_IMAGE_CELL: (usize, usize) = (4, 5);

/// The Emperor's image flickering (task at `0x080262E0`), on and off
/// every other frame until it is stopped.
const EMPEROR_IMAGE_FLICKERS: &[Op] = &[Op::Loop(&[
    Op::Place(EMPEROR_IMAGE, EMPEROR_IMAGE_CELL),
    Op::Wait(2),
    Op::Place(EMPEROR_IMAGE, OFF_THE_MAP),
    Op::Wait(2),
])];

/// The chapter's opening (task at `0x080254D0`): in his hall the Emperor
/// presses Opis (`0x271`, `0x272`), who works at the Zoid core; Gale brings
/// Fran (`0x273`); the core is waking (`0x274`), it wakes (`0x275`) to Fran's
/// and Gale's horror (`0x276`) and the Emperor's delight (`0x277`), and the
/// hall whitens. At the lookout two soldiers report a huge energy at
/// Arcadia Castle (`0x278`, `0x279`) and the prince runs off (`0x27A`); in
/// the briefing room the charged particle cannon flashes (`0x27B`), the
/// Emperor's image speaks to the world (`0x27C` to `0x27E`), and Jack's
/// briefing follows.
const OPENING: &[Op] = &[
    Op::Wait(120),
    Op::Spawn(HELPER_TASK, OPIS_AT_WORK),
    Op::Place(EMPEROR, (10, 0x10)),
    Op::Call(&steps(EMPEROR, (10, 9))),
    Op::Repeat(80, PAN_UP),
    Op::Dialogue(0x271),
    Op::Wait(30),
    Op::Wait(180),
    Op::Wait(120),
    Op::Dialogue(0x272),
    Op::Wait(30),
    Op::Call(&steps(EMPEROR, (10, 7))),
    Op::Wait(120),
    Op::Face(EMPEROR, Direction::Down),
    Op::Repeat(112, PAN_DOWN),
    Op::Place(GALE, (10, 0x10)),
    Op::Call(&steps(GALE, (10, 0xF))),
    Op::Place(FRAN, (10, 0x10)),
    through(FRAN, (10, 9), PACE, 1),
    Op::Call(&steps(GALE, (10, 9))),
    Op::Call(&steps(GALE, (9, 8))),
    Op::Face(GALE, Direction::Up),
    Op::Dialogue(0x273),
    Op::Wait(30),
    Op::Repeat(96, PAN_UP),
    Op::Wait(120),
    Op::Stop(HELPER_TASK),
    Op::Call(&steps(OPIS, (4, 3))),
    Op::Face(OPIS, Direction::Down),
    Op::Wait(30),
    Op::Face(EMPEROR, Direction::Left),
    Op::Dialogue(0x274),
    Op::Wait(30),
    Op::Sound(CORE_SOUND),
    Op::Call(&steps(OPIS, (4, 4))),
    Op::Face(OPIS, Direction::Right),
    Op::Face(EMPEROR, Direction::Up),
    Op::Wait(60),
    Op::Dialogue(0x275),
    Op::Wait(30),
    Op::Sound(ALARM_SOUND),
    Op::Wait(60),
    Op::Dialogue(0x276),
    Op::Wait(30),
    Op::Dialogue(0x277),
    Op::Wait(30),
    Op::Call(&WHITE_OUT),
    Op::LoadScene {
        map: LOOKOUT,
        player: (8, 5),
        objects: LOOKOUT_OBJECTS,
        count: 7,
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(30),
    Op::Wait(60),
    Op::Place(FIRST_SOLDIER, (8, 10)),
    Op::Call(&hurries(FIRST_SOLDIER, (8, 7))),
    Op::Music(ALERT_MUSIC),
    Op::Wait(60),
    Op::Dialogue(0x278),
    Op::Wait(30),
    Op::Place(SECOND_SOLDIER, (8, 10)),
    through(SECOND_SOLDIER, (8, 7), HURRY, 2),
    Op::Call(&hurries(FIRST_SOLDIER, (7, 7))),
    Op::Face(FIRST_SOLDIER, Direction::Right),
    Op::AwaitArrival(SECOND_SOLDIER),
    Op::Wait(60),
    Op::Dialogue(0x279),
    Op::Wait(30),
    through(PLAYER, (8, 10), HURRY, 2),
    Op::Call(&hurries(SECOND_SOLDIER, (9, 7))),
    Op::Face(SECOND_SOLDIER, Direction::Down),
    Op::Wait(8),
    Op::Face(FIRST_SOLDIER, Direction::Down),
    Op::AwaitArrival(PLAYER),
    Op::Hide(PLAYER),
    Op::Wait(30),
    Op::Dialogue(0x27A),
    Op::Wait(30),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::LoadScene {
        map: BRIEFING_ROOM,
        player: (0x11, 2),
        objects: BRIEFING_OBJECTS,
        count: 5,
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Place(PLAYER, (0xE, 2)),
    Op::Call(&hurries(PLAYER, (6, 2))),
    through(PLAYER, (6, 1), HURRY, 2),
    Op::Place(JACK, (0xE, 2)),
    through(JACK, (5, 2), HURRY, 2),
    Op::Wait(30),
    Op::Place(REGINA, (0xE, 2)),
    through(REGINA, (6, 2), HURRY, 2),
    Op::Wait(30),
    Op::Place(EARTH, (0xE, 2)),
    through(EARTH, (7, 2), HURRY, 2),
    Op::AwaitArrival(JACK),
    Op::Face(JACK, Direction::Up),
    Op::AwaitArrival(REGINA),
    Op::Face(REGINA, Direction::Up),
    Op::AwaitArrival(EARTH),
    Op::Face(EARTH, Direction::Up),
    Op::Sound(FLASH_SOUND),
    Op::FadeToWhite(true),
    Op::Repeat(3, FLASH),
    Op::FadeToWhite(false),
    Op::Wait(180),
    Op::Dialogue(0x27B),
    Op::Wait(30),
    Op::Spawn(HELPER_TASK, EMPEROR_IMAGE_FLICKERS),
    Op::Wait(60),
    Op::Dialogue(0x27C),
    Op::Wait(30),
    Op::Face(REGINA, Direction::Down),
    Op::Wait(3),
    Op::Face(EARTH, Direction::Down),
    Op::Wait(3),
    Op::Face(JACK, Direction::Down),
    Op::Wait(3),
    Op::Face(PLAYER, Direction::Down),
    Op::Wait(30),
    Op::Dialogue(0x27D),
    Op::Wait(60),
    Op::Stop(HELPER_TASK),
    Op::Place(EMPEROR_IMAGE, OFF_THE_MAP),
    Op::Wait(60),
    Op::Dialogue(0x27E),
    Op::Wait(30),
    Op::Face(REGINA, Direction::Up),
    Op::Face(EARTH, Direction::Up),
    Op::Face(JACK, Direction::Up),
    Op::Wait(30),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Warp {
        map: BRIEFING_ROOM,
        cell: (6, 1),
        facing: Some(Direction::Down),
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Call(BRIEFING),
];

/// The Emperor's hall (`0x08025468`): the first time, the characters of
/// group 8 are met and the opening plays.
const HALL_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[CHAPTER_OPENED],
    then: &[
        Op::Meet(CHAPTER_GROUP),
        Op::Flag(CHAPTER_OPENED, true),
        Op::LoadScene {
            map: HALL_SCENE,
            player: (10, 0x12),
            objects: HALL_OBJECTS,
            count: 5,
        },
        Op::Spawn(MAP_TASK, OPENING),
    ],
    otherwise: &[],
}];

/// The briefing room while the party gets ready (ROM `0x0832B164`):
/// Regina, who asks whether the prince is ready; Earth and Jack, who
/// call in companions.
const READY_OBJECTS: u32 = 0x0832_B164;
/// Where Jack's briefing leaves the prince, and the room's exit.
const BRIEFED_CELL: (usize, usize) = (6, 2);
const BRIEFING_EXIT: (usize, usize) = (0xD, 2);

/// Jack's briefing and the setting out (task at `0x0802636C`): the first
/// time, Jack explains what is known of the occupied castle (`0x280`) and
/// the companions' slots are emptied; the prince then walks about freely
/// until he tells Regina he is ready, and the party leaves the room.
const BRIEFING: &[Op] = &[
    Op::IfFlags {
        all: &[],
        none: &[BASE_BRIEFED],
        then: &[
            Op::Control(false),
            Op::Dialogue(0x280),
            Op::Wait(30),
            Op::Flag(BASE_BRIEFED, true),
            Op::Call(&steps(PLAYER, BRIEFED_CELL)),
            Op::ForgetCompanions,
        ],
        otherwise: &[],
    },
    Op::Control(true),
    Op::AwaitAnyFlag(&[SET_OUT]),
    Op::Control(false),
    through(JACK, (1, 2), PACE, 1),
    through(EARTH, (7, 2), PACE, 1),
    Op::AwaitArrival(JACK),
    through(JACK, (0xB, 2), PACE, 1),
    through(EARTH, (0xB, 2), PACE, 1),
    Op::AwaitArrival(EARTH),
    Op::Hide(EARTH),
    Op::AwaitArrival(JACK),
    Op::Hide(JACK),
    Op::Call(&steps(REGINA, (0xB, 2))),
    Op::Hide(REGINA),
    walk(PLAYER, BRIEFING_EXIT, PACE, 1),
    Op::End,
];

/// The briefing room (`0x08026304`): until the party sets out, the room
/// is laid out for the getting ready and Jack's briefing runs.
const BRIEFING_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[SET_OUT],
    then: &[
        Op::LoadScene {
            map: BRIEFING_ROOM,
            player: HERE,
            objects: READY_OBJECTS,
            count: 4,
        },
        Op::Spawn(MAP_TASK, BRIEFING),
    ],
    otherwise: &[],
}];

/// Regina (`0x0802672C`): the text system is reset (`0x0803E0B8`, a
/// frame), and she asks whether the prince is ready (`0x28D`); yes sets
/// the party out.
const REGINA_ASKS: &[Op] = &[
    Op::Wait(1),
    Op::Dialogue(0x28D),
    Op::IfChoice {
        then: &[Op::Flag(SET_OUT, true)],
        otherwise: &[],
    },
];

/// A companion's caller (see [`super::caller`]): Earth (`0x08026764`)
/// fills slot 0, Jack (`0x0802681C`) slot 1; the list (`0x288`, `0x282`),
/// the pick joining with a unit of its Zoid (`0x28C`, `0x286`) and the
/// offer to send it away (`0x28A`, `0x284`).
const EARTH_CALLS: &[Op] = super::caller!(0, 1, 0x288, 0x28C, 0x28A, true);
const JACK_CALLS: &[Op] = super::caller!(1, 0, 0x282, 0x286, 0x284, true);

/// A beaten party's way home: the return point, facing up, and the field
/// brightening while the game holds; the scene's task ends.
const BEATEN: &[Op] = &[Op::WarpHome, Op::FadeInHolding, Op::End];

/// The frames a story battle's black screen waits for its song after a
/// win, and a scene's pause after a line.
const LINE_PAUSE: u32 = 30;

/// Where the warp sprite a Zoid leaves with plays (`0x080089A0` with
/// `0xFD`): it plays once to the warp's sound, and the Zoid is gone.
const WARP_SPRITE: usize = 0xFD;
const fn warps_out(actor: usize) -> [Op; 5] {
    [
        Op::Sprite(actor, WARP_SPRITE),
        Op::PlayOnce(actor, 0),
        Op::Sound(WARP_SOUND),
        Op::AwaitAnimation(actor),
        Op::Hide(actor),
    ]
}

/// Fran's room as the party arrives (ROM `0x0832B1B4`): the party's Zoid
/// and Fran's, off the map.
const FRAN_ROOM_OBJECTS: u32 = 0x0832_B1B4;
const FRAN_ZOID: usize = 1;
/// The Emperor in his hall (ROM `0x0832B1DC`).
const EMPEROR_ALONE: u32 = 0x0832_B1DC;
/// Fran's room after the battle (ROM `0x0832B1F0`): Fran's Zoid where the
/// party stood, and Blood's off the map.
const FRAN_FALLEN_OBJECTS: u32 = 0x0832_B1F0;
const BLOOD_ZOID: usize = 1;
/// The room once they have gone (ROM `0x0832B218`): the party's Zoid.
const ROOM_LEFT_OBJECTS: u32 = 0x0832_B218;

/// Fran's room (task at `0x08026934`): the party comes in (`0x28F`) and
/// Fran comes down to meet it (`0x290`); story battle 36. Won, Fran falls
/// (`0x291`) and the party moves on; the Emperor scorns her from his hall
/// (`0x292`); Fran, alone (`0x293`), is found by Blood (`0x294`), who
/// takes her away; the party, back in the room, wonders (`0x295`) and
/// walks on.
const FRAN_SCENE: &[Op] = &[
    Op::Wait(60),
    Op::Call(&steps(PLAYER, (0xC, 6))),
    Op::Dialogue(0x28F),
    Op::Wait(LINE_PAUSE),
    Op::Repeat(64, PAN_UP),
    Op::Wait(60),
    Op::Place(FRAN_ZOID, (0xC, 3)),
    through(FRAN_ZOID, (0xC, 5), PACE, 1),
    Op::Repeat(64, PAN_DOWN),
    Op::AwaitArrival(FRAN_ZOID),
    Op::Music(ENEMY_MUSIC),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x290),
    Op::Wait(LINE_PAUSE),
    Op::Call(&story_battle(36)),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[],
    },
    Op::Flag(FRAN_BEATEN, true),
    Op::Music(STANDOFF_MUSIC),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Once(FRAN_ZOID),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x291),
    Op::Wait(LINE_PAUSE),
    through(PLAYER, (2, 6), PACE, 1),
    Op::Wait(60),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::LoadScene {
        map: HALL_SCENE,
        player: (10, 7),
        objects: EMPEROR_ALONE,
        count: 1,
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x292),
    Op::Wait(60),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::LoadScene {
        map: FRAN_ROOM_SCENE,
        player: (0xC, 5),
        objects: FRAN_FALLEN_OBJECTS,
        count: 2,
    },
    Op::Once(PLAYER),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x293),
    Op::Wait(60),
    Op::Repeat(32, PAN_UP),
    Op::Wait(LINE_PAUSE),
    Op::Wait(LINE_PAUSE),
    Op::Place(BLOOD_ZOID, (0xC, 3)),
    through(BLOOD_ZOID, (0xC, 4), PACE, 1),
    Op::Repeat(32, PAN_DOWN),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x294),
    Op::Wait(60),
    Op::Call(&steps(BLOOD_ZOID, (0xD, 4))),
    Op::Call(&steps(BLOOD_ZOID, (0xD, 5))),
    Op::Wait(60),
    Op::Call(&steps(BLOOD_ZOID, (0xD, 4))),
    Op::Call(&steps(BLOOD_ZOID, (0xC, 4))),
    Op::Call(&steps(BLOOD_ZOID, (0xC, 3))),
    Op::Hide(BLOOD_ZOID),
    Op::Wait(LINE_PAUSE),
    Op::Sound(PORTAL_SOUND),
    Op::Wait(60),
    Op::Call(&warps_out(PLAYER)),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::LoadScene {
        map: FRAN_ROOM_SCENE,
        player: (3, 6),
        objects: ROOM_LEFT_OBJECTS,
        count: 1,
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x295),
    Op::Wait(60),
    Op::Call(&steps(PLAYER, (2, 6))),
    walk(PLAYER, (2, 2), PACE, 1),
    Op::End,
];

/// Fran's room (`0x080268D8`): until Fran is beaten, the scene.
const FRAN_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[FRAN_BEATEN],
    then: &[
        Op::LoadScene {
            map: FRAN_ROOM_SCENE,
            player: (0xC, 9),
            objects: FRAN_ROOM_OBJECTS,
            count: 2,
        },
        Op::Spawn(MAP_TASK, FRAN_SCENE),
    ],
    otherwise: &[],
}];

/// Gale's room (map 283's objects): Gale's Zoid and Blood's, off the map.
const GALE_ZOID: usize = 1;
const BLOOD_IN_GALE_ROOM: usize = 2;
/// The cell that sets the scene off.
const GALE_ROOM_TRIGGER: (usize, usize) = (9, 5);

/// Gale's room (task at `0x08026F7C`): reaching the cell before him, the
/// party walks up to Gale (`0x296`); story battle 37. Won, Gale falls
/// before Jack (`0x297`) and the party goes on through the door; Gale,
/// alone (`0x298`), is found by Blood (`0x299`, `0x29A`), whom Fran's
/// warp takes away with him; the party is in the next room.
const GALE_SCENE: &[Op] = &[
    Op::AwaitPlayer {
        columns: Some((GALE_ROOM_TRIGGER.0, GALE_ROOM_TRIGGER.0)),
        rows: Some((GALE_ROOM_TRIGGER.1, GALE_ROOM_TRIGGER.1)),
    },
    Op::Control(false),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(PLAYER, (7, 5))),
    Op::Call(&steps(PLAYER, (7, 2))),
    Op::Call(&steps(PLAYER, (5, 2))),
    Op::Music(ENEMY_MUSIC),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x296),
    Op::Wait(LINE_PAUSE),
    Op::Call(&story_battle(37)),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[],
    },
    Op::Flag(GALE_BEATEN, true),
    Op::Place(GALE_ZOID, (4, 1)),
    Op::Face(GALE_ZOID, Direction::Down),
    Op::Once(GALE_ZOID),
    Op::Music(STANDOFF_MUSIC),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x297),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(PLAYER, (2, 2))),
    Op::Call(&steps(PLAYER, (2, 5))),
    Op::Hide(PLAYER),
    Op::Sound(DOOR_SOUND),
    Op::Wait(60),
    Op::Repeat(64, PAN_UP),
    Op::Wait(60),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x298),
    Op::Wait(LINE_PAUSE),
    Op::Place(BLOOD_IN_GALE_ROOM, (8, 2)),
    Op::Call(&steps(BLOOD_IN_GALE_ROOM, (4, 2))),
    Op::Face(BLOOD_IN_GALE_ROOM, Direction::Up),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x299),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(BLOOD_IN_GALE_ROOM, (3, 2))),
    Op::Call(&steps(BLOOD_IN_GALE_ROOM, (3, 1))),
    Op::Wait(60),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x29A),
    Op::Wait(LINE_PAUSE),
    through(BLOOD_IN_GALE_ROOM, (2, 1), PACE, 1),
    Op::Sound(PORTAL_SOUND),
    Op::Wait(60),
    Op::Call(&warps_out(GALE_ZOID)),
    Op::AwaitArrival(BLOOD_IN_GALE_ROOM),
    Op::Call(&steps(BLOOD_IN_GALE_ROOM, (2, 5))),
    Op::Hide(BLOOD_IN_GALE_ROOM),
    Op::Warp {
        map: GALE_ROOM_EXIT,
        cell: (2, 8),
        facing: Some(Direction::Left),
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::End,
];

/// Gale's room (`0x08026F2C`): until Gale is beaten, he stands at the top
/// and the scene waits for the party.
const GALE_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[GALE_BEATEN],
    then: &[
        Op::Place(GALE_ZOID, (4, 2)),
        Op::Spawn(MAP_TASK, GALE_SCENE),
    ],
    otherwise: &[],
}];

/// Opis's room (map 285's objects): Opis's Zoid and Blood's, off the map.
const OPIS_ZOID: usize = 1;
const BLOOD_IN_OPIS_ROOM: usize = 2;

/// Opis's room (task at `0x080274B4`): the party comes up to Opis
/// (`0x29B`); story battle 38. Won, Opis sinks, crawls away and seems to
/// fall (`0x29C`, `0x29D`); the party fears a blast (`0x29E`) and runs
/// through the door; Opis rises (`0x29F`) and meets Blood (`0x2A0`,
/// `0x2A1`), who goes on; the party is in the hall.
const OPIS_SCENE: &[Op] = &[
    through(OPIS_ZOID, (4, 4), PACE, 1),
    Op::Call(&steps(PLAYER, (4, 8))),
    Op::Call(&steps(PLAYER, (4, 6))),
    Op::Music(ENEMY_MUSIC),
    Op::Wait(60),
    Op::Dialogue(0x29B),
    Op::Wait(LINE_PAUSE),
    Op::Call(&story_battle(38)),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[],
    },
    Op::Flag(OPIS_BEATEN, true),
    Op::Music(STANDOFF_MUSIC),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x29C),
    Op::Wait(LINE_PAUSE),
    Op::Call(&goes(OPIS_ZOID, (4, 5), PIXEL / 4, -1)),
    Op::Once(OPIS_ZOID),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x29D),
    Op::Wait(LINE_PAUSE),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x29E),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(PLAYER, (3, 6))),
    Op::Call(&steps(PLAYER, (3, 4))),
    Op::Call(&steps(PLAYER, (4, 4))),
    Op::Call(&steps(PLAYER, (4, 0))),
    Op::Hide(PLAYER),
    Op::Sound(DOOR_SOUND),
    Op::Wait(LINE_PAUSE),
    Op::Repeat(128, PAN_DOWN),
    Op::Wait(60),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x29F),
    Op::Wait(LINE_PAUSE),
    Op::Place(BLOOD_IN_OPIS_ROOM, (6, 8)),
    Op::Call(&steps(BLOOD_IN_OPIS_ROOM, (4, 8))),
    Op::Call(&steps(BLOOD_IN_OPIS_ROOM, (4, 6))),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x2A0),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(BLOOD_IN_OPIS_ROOM, (4, 5))),
    Op::Hide(OPIS_ZOID),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x2A1),
    Op::Wait(LINE_PAUSE),
    through(BLOOD_IN_OPIS_ROOM, (4, 0), PACE, 1),
    Op::Repeat(128, PAN_UP),
    Op::Wait(LINE_PAUSE),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Warp {
        map: HALL,
        cell: (5, 8),
        facing: Some(Direction::Left),
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::End,
];

/// Opis's room (`0x0802745C`): until Opis is beaten, he waits at the top
/// and the party is walked in.
const OPIS_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[OPIS_BEATEN],
    then: &[
        Op::Place(OPIS_ZOID, (4, 0)),
        Op::Control(false),
        Op::Spawn(MAP_TASK, OPIS_SCENE),
    ],
    otherwise: &[],
}];

/// The castle's heart (ROM `0x0832B22C`): the party's leading Zoid, the
/// Emperor's, the party's six units, Blood, Fran, Gale and Opis off the
/// map, the space-time transfer device and an explosion.
const HEART_OBJECTS: u32 = 0x0832_B22C;
const EMPEROR_ZOID: usize = 1;
const PRINCE_UNIT: usize = 2;
const REGINA_UNIT: usize = 3;
const EARTH_UNIT: usize = 4;
const JACK_UNIT: usize = 5;
const BLOOD: usize = 8;
const FRAN_UNIT: usize = 9;
const GALE_UNIT: usize = 10;
const OPIS_UNIT: usize = 11;
const DEVICE: usize = 12;
const EXPLOSION: usize = 13;
/// Where the party's units come out of the leading Zoid's cell.
const HEART_GATHERING: (usize, usize) = (8, 7);

/// The device's animation: the step its loop goes back to, the step
/// whose end sends it back, the one whose end sounds as it closes, the
/// last, and the step it opens from once the Emperor is through.
const DEVICE_LOOP: usize = 16;
const DEVICE_LOOP_END: usize = 41;
const DEVICE_CLOSING: usize = 42;
const DEVICE_CLOSED: usize = 48;
const DEVICE_REOPENS: usize = 47;
const DEVICE_SPRITE: usize = 0xF7;
const DEVICE_OVERLOAD_SPRITE: usize = 0x102;

/// The Emperor's first Zoid beaten (tasks at `0x08027AAC`): the party
/// walks in and looks up at the Emperor (`0x2A2`), gathers
/// (`0x2A3`), and faces him while he turns about (`0x2A4` to `0x2A6`);
/// story battle 39, then the rest of the finale.
const HEART_SCENE: &[Op] = &[
    Op::Call(&steps(PLAYER, (8, 0xA))),
    Op::Face(PLAYER, Direction::Up),
    Op::Wait(60),
    Op::Repeat(192, PAN_UP),
    Op::Wait(60),
    Op::Wait(60),
    Op::Dialogue(0x2A2),
    Op::Wait(LINE_PAUSE),
    Op::Repeat(96, PAN_DOWN_FAST),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(PLAYER, HEART_GATHERING)),
    Op::Wait(LINE_PAUSE),
    Op::Repeat(64, PAN_UP),
    Op::Wait(LINE_PAUSE),
    Op::Place(PRINCE_UNIT, HEART_GATHERING),
    Op::Call(&steps(PRINCE_UNIT, (8, 5))),
    Op::Place(EARTH_UNIT, HEART_GATHERING),
    Op::Call(&steps(EARTH_UNIT, (9, 5))),
    Op::Place(JACK_UNIT, HEART_GATHERING),
    Op::Call(&steps(JACK_UNIT, (7, 5))),
    Op::Place(REGINA_UNIT, HEART_GATHERING),
    Op::Call(&steps(REGINA_UNIT, (8, 6))),
    Op::Wait(60),
    Op::Dialogue(0x2A3),
    Op::Wait(LINE_PAUSE),
    Op::Face(EMPEROR_ZOID, Direction::Left),
    Op::Sound(ALARM_SOUND),
    Op::Wait(60),
    Op::Face(EMPEROR_ZOID, Direction::Right),
    Op::Sound(ALARM_SOUND),
    Op::Wait(60),
    Op::Face(EMPEROR_ZOID, Direction::Down),
    Op::Sound(ALARM_SOUND),
    Op::Wait(60),
    Op::Music(STANDOFF_MUSIC),
    Op::Wait(60),
    Op::Dialogue(0x2A4),
    Op::Wait(LINE_PAUSE),
    Op::Wait(LINE_PAUSE),
    Op::Wait(60),
    Op::Dialogue(0x2A5),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(PRINCE_UNIT, (8, 4))),
    Op::Wait(60),
    Op::Dialogue(0x2A6),
    Op::Wait(60),
    Op::Sound(ALARM_SOUND),
    Op::Wait(60),
    Op::Call(&story_battle(39)),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[],
    },
    Op::Flag(FIRST_ZOID_BEATEN, true),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Call(FINALE),
    Op::End,
];

/// An explosion on the Emperor's Zoid, a little apart from the next.
const BLAST_ON_THE_EMPEROR: &[Op] = &[
    Op::PlayOnce(EXPLOSION, 0),
    Op::PlaceOnActor(EXPLOSION, EMPEROR_ZOID),
    Op::Sound(BLAST_SOUND),
    Op::AwaitAnimation(EXPLOSION),
    Op::Place(EXPLOSION, OFF_THE_MAP),
    Op::Wait(40),
];

/// Two units drawing a pixel a frame toward each other, then apart.
const fn face_off(near: usize, far: usize) -> [Op; 3] {
    [
        Op::Nudge(near, (-1, 0)),
        Op::Nudge(far, (1, 0)),
        Op::Wait(1),
    ]
}

/// Every pair stepping back into its place.
const STEP_BACK: &[Op] = &[
    Op::Nudge(PRINCE_UNIT, (1, 0)),
    Op::Nudge(BLOOD, (-1, 0)),
    Op::Nudge(EARTH_UNIT, (1, 0)),
    Op::Nudge(FRAN_UNIT, (-1, 0)),
    Op::Nudge(JACK_UNIT, (1, 0)),
    Op::Nudge(GALE_UNIT, (-1, 0)),
    Op::Nudge(REGINA_UNIT, (1, 0)),
    Op::Nudge(OPIS_UNIT, (-1, 0)),
    Op::Wait(1),
];

/// A unit jumping into the device (entity command 10, four pixels a
/// frame), gone once through.
const fn into_the_device(unit: usize) -> [Op; 4] {
    [
        Op::Sound(JUMP_SOUND),
        walk(unit, (8, 1), 4 * PIXEL, 3),
        Op::AwaitArrival(unit),
        Op::Place(unit, OFF_THE_MAP),
    ]
}

/// The castle's heart after the first battle (task at `0x08027E8C`): the
/// Emperor's Zoid burns (`0x2A7`) and he calls on his son (`0x2A8`); Opis,
/// Blood, Fran and Gale come in, and each faces his rival
/// (`0x2A9` to `0x2AF`); story battle 40. Won, the space-time transfer
/// device opens, the Emperor overloads it (`0x2B0`, `0x2B1`) and flees
/// through it (`0x2B2`); Blood blocks it (`0x2B3`, `0x2B4`), Fran follows
/// him (`0x2B5`), then Gale and Opis (`0x2B6`, `0x2B7`), and it closes
/// (`0x2B8`). At home the Queen welcomes the prince (`0x2B9`, `0x2BA`),
/// Regina talks with him (`0x2BB` to `0x2BD`), Earth teases them (`0x2BE`),
/// and at the ceremony the prince kneels; the staff roll, and the party
/// is taken to chapter 10's first map, whose handler runs within this
/// task (what it spawns is lost), so the task calls the castle's own after
/// the fade in (see [`super::chapter10::CASTLE_TASK`]).
const FINALE: &[Op] = &[
    Op::Sound(ALARM_SOUND),
    Op::Wait(60),
    Op::Repeat(4, BLAST_ON_THE_EMPEROR),
    Op::Wait(60),
    Op::Dialogue(0x2A7),
    Op::Wait(LINE_PAUSE),
    Op::Repeat(4, BLAST_ON_THE_EMPEROR),
    Op::Repeat(32, PAN_DOWN),
    Op::Wait(60),
    Op::Music(EMPEROR_MUSIC),
    Op::Wait(60),
    Op::Dialogue(0x2A8),
    Op::Wait(LINE_PAUSE),
    Op::Place(OPIS_UNIT, (5, 9)),
    Op::Call(&steps(OPIS_UNIT, (5, 5))),
    Op::Face(OPIS_UNIT, Direction::Right),
    Op::Face(PRINCE_UNIT, Direction::Left),
    Op::Face(REGINA_UNIT, Direction::Left),
    Op::Face(EARTH_UNIT, Direction::Left),
    Op::Face(JACK_UNIT, Direction::Left),
    Op::Music(STANDOFF_MUSIC),
    Op::Wait(60),
    Op::Dialogue(0x2A9),
    Op::Wait(LINE_PAUSE),
    Op::Place(BLOOD, (7, 9)),
    Op::Place(FRAN_UNIT, (6, 9)),
    Op::Place(GALE_UNIT, (7, 0xA)),
    through(BLOOD, (7, 4), PACE, 1),
    through(FRAN_UNIT, (6, 4), PACE, 1),
    through(GALE_UNIT, (7, 5), PACE, 1),
    Op::Wait(60),
    through(REGINA_UNIT, (0xA, 6), PACE, 1),
    through(EARTH_UNIT, (0xA, 5), PACE, 1),
    through(JACK_UNIT, (9, 5), PACE, 1),
    through(PRINCE_UNIT, (9, 4), PACE, 1),
    Op::AwaitArrival(EARTH_UNIT),
    through(EARTH_UNIT, (0xA, 4), PACE, 1),
    Op::AwaitArrival(REGINA_UNIT),
    Op::Call(&steps(REGINA_UNIT, (0xA, 5))),
    Op::Face(PRINCE_UNIT, Direction::Left),
    Op::Face(REGINA_UNIT, Direction::Left),
    Op::Face(EARTH_UNIT, Direction::Left),
    Op::Face(JACK_UNIT, Direction::Left),
    Op::AwaitArrival(GALE_UNIT),
    Op::Call(&steps(OPIS_UNIT, (6, 5))),
    Op::Face(BLOOD, Direction::Right),
    Op::Face(FRAN_UNIT, Direction::Right),
    Op::Face(GALE_UNIT, Direction::Right),
    Op::Face(OPIS_UNIT, Direction::Right),
    Op::Wait(60),
    Op::Dialogue(0x2AA),
    Op::Wait(LINE_PAUSE),
    Op::Repeat(8, &face_off(PRINCE_UNIT, BLOOD)),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x2AB),
    Op::Wait(LINE_PAUSE),
    Op::Repeat(8, &face_off(EARTH_UNIT, FRAN_UNIT)),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x2AC),
    Op::Wait(LINE_PAUSE),
    Op::Repeat(8, &face_off(JACK_UNIT, GALE_UNIT)),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x2AD),
    Op::Wait(LINE_PAUSE),
    Op::Repeat(8, &face_off(REGINA_UNIT, OPIS_UNIT)),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x2AE),
    Op::Wait(LINE_PAUSE),
    Op::Repeat(8, STEP_BACK),
    Op::Wait(LINE_PAUSE),
    Op::Dialogue(0x2AF),
    Op::Wait(LINE_PAUSE),
    Op::Call(&story_battle(40)),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[],
    },
    Op::Flag(EMPEROR_BEATEN, true),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Repeat(128, PAN_UP),
    Op::Wait(60),
    Op::Music(PORTAL_MUSIC),
    Op::Face(PRINCE_UNIT, Direction::Up),
    Op::Face(REGINA_UNIT, Direction::Up),
    Op::Face(EARTH_UNIT, Direction::Up),
    Op::Face(JACK_UNIT, Direction::Up),
    Op::Face(BLOOD, Direction::Up),
    Op::Face(FRAN_UNIT, Direction::Up),
    Op::Face(GALE_UNIT, Direction::Up),
    Op::Face(OPIS_UNIT, Direction::Up),
    Op::Animate(DEVICE, 1),
    Op::Sound(PORTAL_SOUND),
    Op::RestartStep(DEVICE, DEVICE_LOOP),
    Op::Repeat(
        4,
        &[
            Op::AwaitStepEnd(DEVICE, DEVICE_LOOP_END),
            Op::Sound(PORTAL_SOUND),
            Op::RestartStep(DEVICE, DEVICE_LOOP),
            Op::Wait(1),
        ],
    ),
    Op::AwaitStepEnd(DEVICE, DEVICE_CLOSING),
    Op::Sound(CLOSING_SOUND),
    Op::AwaitStepEnd(DEVICE, DEVICE_CLOSED),
    Op::Place(DEVICE, OFF_THE_MAP),
    Op::Wait(60),
    Op::Dialogue(0x2B0),
    Op::Wait(LINE_PAUSE),
    Op::Sound(CANNON_SOUND),
    Op::Repeat(5, FLASH),
    Op::Wait(60),
    Op::Sound(PORTAL_SOUND),
    Op::Sprite(DEVICE, DEVICE_OVERLOAD_SPRITE),
    Op::PlayOnce(DEVICE, 0),
    Op::Place(DEVICE, (8, 1)),
    Op::AwaitAnimation(DEVICE),
    Op::Animate(DEVICE, 1),
    Op::Call(&steps(EMPEROR_ZOID, (8, 2))),
    Op::Wait(60),
    Op::Dialogue(0x2B1),
    Op::Wait(LINE_PAUSE),
    Op::Wait(60),
    Op::Dialogue(0x2B2),
    Op::Wait(LINE_PAUSE),
    Op::Call(&into_the_device(EMPEROR_ZOID)),
    Op::Sound(ALARM_SOUND),
    Op::Wait(60),
    Op::Wait(60),
    Op::Dialogue(0x2B3),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(PRINCE_UNIT, (9, 3))),
    through(PRINCE_UNIT, (8, 3), PIXEL / 2, 0),
    Op::Call(&hurries(BLOOD, (7, 3))),
    Op::Call(&hurries(BLOOD, (8, 3))),
    Op::Sound(WARP_SOUND),
    Op::Animate(PRINCE_UNIT, 8),
    Op::Repeat(32, &[Op::Nudge(PRINCE_UNIT, (2, 0)), Op::Wait(1)]),
    Op::Settle(PRINCE_UNIT),
    through(PRINCE_UNIT, (9, 3), PIXEL / 2, 0),
    Op::Call(&steps(BLOOD, (8, 2))),
    Op::Face(BLOOD, Direction::Down),
    Op::Music(FAREWELL_MUSIC),
    Op::Wait(60),
    Op::Dialogue(0x2B4),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(FRAN_UNIT, (8, 3))),
    Op::Face(BLOOD, Direction::Up),
    Op::Wait(60),
    Op::Dialogue(0x2B5),
    Op::Wait(LINE_PAUSE),
    Op::Call(&into_the_device(BLOOD)),
    walk(FRAN_UNIT, (8, 2), PACE, 1),
    Op::AwaitArrival(FRAN_UNIT),
    Op::Call(&into_the_device(FRAN_UNIT)),
    Op::Repeat(32, PAN_DOWN),
    Op::Wait(60),
    Op::Dialogue(0x2B6),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(GALE_UNIT, (8, 3))),
    through(GALE_UNIT, (8, 2), PACE, 1),
    Op::Repeat(32, PAN_UP),
    Op::AwaitArrival(GALE_UNIT),
    Op::Call(&into_the_device(GALE_UNIT)),
    Op::Repeat(32, PAN_DOWN),
    Op::Wait(60),
    Op::Dialogue(0x2B7),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(OPIS_UNIT, (8, 3))),
    through(OPIS_UNIT, (8, 2), PACE, 1),
    Op::Repeat(32, PAN_UP),
    Op::AwaitArrival(OPIS_UNIT),
    Op::Call(&into_the_device(OPIS_UNIT)),
    Op::Wait(60),
    Op::Sound(PORTAL_SOUND),
    Op::PlayOnce(DEVICE, 2),
    Op::AwaitAnimation(DEVICE),
    Op::Wait(60),
    Op::Sprite(DEVICE, DEVICE_SPRITE),
    Op::Animate(DEVICE, 1),
    Op::RestartStep(DEVICE, DEVICE_REOPENS),
    Op::Sound(OPENING_SOUND),
    Op::Once(DEVICE),
    Op::AwaitAnimation(DEVICE),
    Op::Animate(DEVICE, 0),
    Op::Wait(60),
    Op::Dialogue(0x2B8),
    Op::Wait(LINE_PAUSE),
    Op::Wait(60),
    through(PRINCE_UNIT, (9, 0x10), PIXEL / 2, 0),
    through(REGINA_UNIT, (0xA, 0x10), PIXEL / 2, 0),
    through(EARTH_UNIT, (0xA, 0x10), PIXEL / 2, 0),
    through(JACK_UNIT, (9, 0x10), PIXEL / 2, 0),
    Op::Repeat(96, PAN_DOWN),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Call(HOMECOMING_SCENE),
];

/// Home (ROM `0x0832B344`): the prince, Regina, Earth and Jack off the
/// map, and the Queen.
const HOME_OBJECTS: u32 = 0x0832_B344;
const QUEEN: usize = 4;
/// The ceremony (ROM `0x0832B3A8`): the prince, Regina, Earth and Jack.
const CEREMONY_OBJECTS: u32 = 0x0832_B3A8;
/// The prince kneeling (`0x080089A0` with `0x104`).
const PRINCE_KNEELING: usize = 0x104;

/// The walk home, the ceremony and the staff credits (the end of the task
/// at `0x08027E8C`). Once the credits are over, their caller clears the
/// screen's memory and resets the text system (`0x0800C430`,
/// `0x0803E0B8`): two frames, measured.
const HOMECOMING_SCENE: &[Op] = &[
    Op::LoadMap {
        map: GATEWAY,
        player: (6, 2),
        objects: HOME_OBJECTS,
        count: 5,
    },
    Op::Music(HOME_MUSIC),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Call(&steps(QUEEN, (7, 2))),
    Op::Face(QUEEN, Direction::Left),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(QUEEN, (7, 3))),
    Op::Call(&steps(QUEEN, (6, 3))),
    Op::Face(QUEEN, Direction::Up),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(QUEEN, (5, 3))),
    Op::Call(&steps(QUEEN, (5, 2))),
    Op::Face(QUEEN, Direction::Right),
    Op::Wait(60),
    Op::Dialogue(0x2B9),
    Op::Wait(LINE_PAUSE),
    Op::Place(REGINA, (2, 0xC)),
    Op::Call(&steps(REGINA, (2, 7))),
    Op::Call(&steps(REGINA, (4, 4))),
    Op::Face(REGINA, Direction::Up),
    Op::Wait(60),
    Op::Dialogue(0x2BA),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(QUEEN, (5, 5))),
    Op::Call(&steps(QUEEN, (2, 5))),
    Op::Call(&steps(QUEEN, (2, 0xC))),
    Op::Place(QUEEN, OFF_THE_MAP),
    through(PLAYER, (8, 2), PACE, 1),
    Op::Call(&steps(REGINA, (4, 2))),
    Op::Face(REGINA, Direction::Right),
    Op::Wait(60),
    Op::Dialogue(0x2BB),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(REGINA, (5, 2))),
    Op::Wait(60),
    Op::Dialogue(0x2BC),
    Op::Wait(LINE_PAUSE),
    Op::Call(&steps(REGINA, (6, 2))),
    Op::Face(PLAYER, Direction::Left),
    Op::Wait(60),
    Op::Dialogue(0x2BD),
    Op::Wait(LINE_PAUSE),
    Op::Place(EARTH, (6, 0xB)),
    Op::Place(JACK, (7, 0xC)),
    Op::Face(EARTH, Direction::Left),
    Op::Face(JACK, Direction::Left),
    Op::Call(&steps(PLAYER, (8, 3))),
    Op::Call(&steps(PLAYER, (5, 3))),
    through(PLAYER, (2, 3), PACE, 1),
    through(REGINA, (2, 2), PACE, 1),
    Op::AwaitArrival(PLAYER),
    through(PLAYER, (2, 0xC), PACE, 1),
    through(REGINA, (2, 0xB), PACE, 1),
    Op::AwaitArrival(PLAYER),
    through(PLAYER, (5, 0xC), PACE, 1),
    through(REGINA, (4, 0xB), PACE, 1),
    Op::AwaitArrival(PLAYER),
    Op::Wait(60),
    Op::Dialogue(0x2BE),
    Op::Wait(LINE_PAUSE),
    walk(PLAYER, (6, 0xC), PACE, 1),
    walk(JACK, (7, 0xB), PACE, 1),
    walk(REGINA, (0x13, 0xB), PACE, 1),
    Op::AwaitArrival(PLAYER),
    walk(PLAYER, (0x13, 0xC), PACE, 1),
    walk(EARTH, (6, 0xC), PACE, 1),
    Op::AwaitArrival(EARTH),
    walk(JACK, (0x13, 0xB), PACE, 1),
    walk(EARTH, (0x13, 0xC), PACE, 1),
    Op::Wait(150),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::LoadMap {
        map: HOMECOMING,
        player: (8, 9),
        objects: CEREMONY_OBJECTS,
        count: 4,
    },
    walk(PLAYER, (8, 4), PACE, 1),
    walk(REGINA, (7, 4), PACE, 1),
    walk(EARTH, (8, 4), PACE, 1),
    walk(JACK, (9, 4), PACE, 1),
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::AwaitArrival(PLAYER),
    through(PLAYER, (8, 1), PIXEL / 2, 0),
    Op::AwaitArrival(REGINA),
    walk(REGINA, (5, 4), PACE, 1),
    Op::AwaitArrival(JACK),
    walk(JACK, (0xC, 4), PACE, 1),
    Op::AwaitArrival(EARTH),
    walk(EARTH, (0xB, 4), PACE, 1),
    Op::AwaitArrival(REGINA),
    Op::Face(REGINA, Direction::Down),
    Op::AwaitArrival(JACK),
    Op::Face(JACK, Direction::Down),
    Op::AwaitArrival(EARTH),
    Op::Face(EARTH, Direction::Down),
    Op::AwaitArrival(PLAYER),
    Op::Face(PLAYER, Direction::Down),
    Op::Wait(120),
    Op::Sprite(PLAYER, PRINCE_KNEELING),
    Op::Animate(PLAYER, 0),
    Op::Wait(240),
    Op::Spawn(HELPER_TASK, FADE_OUT),
    Op::Wait(60),
    Op::Silence,
    Op::Brightness(super::BLACK),
    Op::Wait(1),
    Op::Wait(1),
    Op::Wait(1),
    Op::Wait(60),
    Op::Credits,
    Op::Wait(2),
    Op::Wait(60),
    Op::DropCompanion(0),
    Op::DropCompanion(1),
    Op::Meet(NEXT_GROUP),
    Op::Warp {
        map: CHAPTER_10_START,
        cell: (6, 1),
        facing: Some(Direction::Up),
    },
    Op::Spawn(HELPER_TASK, FADE_IN),
    Op::Wait(60),
    Op::Call(super::chapter10::CASTLE_TASK),
];

/// The castle's heart (`0x080279C0`): the first time, the party walks in
/// to the Emperor; once his first Zoid is beaten, the finale's layout and
/// the finale.
const HEART_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[FIRST_ZOID_BEATEN],
    then: &[
        Op::LoadScene {
            map: HEART,
            player: HERE,
            objects: HEART_OBJECTS,
            count: 14,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, HEART_SCENE),
    ],
    otherwise: &[Op::IfFlags {
        all: &[],
        none: &[EMPEROR_BEATEN],
        then: &[
            Op::LoadScene {
                map: HEART,
                player: (8, 5),
                objects: HEART_OBJECTS,
                count: 14,
            },
            Op::Place(PLAYER, HEART_GATHERING),
            Op::Place(PRINCE_UNIT, (8, 4)),
            Op::Place(REGINA_UNIT, (8, 6)),
            Op::Place(EARTH_UNIT, (9, 5)),
            Op::Place(JACK_UNIT, (7, 5)),
            Op::Face(PLAYER, Direction::Up),
            Op::Control(false),
            Op::Spawn(MAP_TASK, &[Op::Call(FINALE), Op::End]),
        ],
        otherwise: &[],
    }],
}];

/// The teacher of area 9: deck command `0x1D` (`0x080095DC` through
/// `0x08009430`).
const TEACHERS: [(u32, &[Op]); 1] = [(
    0x0800_95DC,
    &[Op::IfCommand {
        command: 0x1D,
        then: &[Op::Dialogue(0x3B5)],
        otherwise: &[Op::Dialogue(0x3B4), Op::Call(&learn(0x1D))],
    }],
)];

/// The keepers of area 9: item shop 18 (`0x080093CC`), armaments shop 24
/// (`0x080093D8`) and lab 17 (`0x080093E4`).
const SHOPS: [(u32, &[Op]); 3] = [
    (0x0800_93CC, &shop(Shop::Items(0x12))),
    (0x0800_93D8, &shop(Shop::Arms(0x18))),
    (0x0800_93E4, &shop(Shop::Lab(0x11))),
];

/// The objects of area 9 whose script is code: the briefing room's
/// Regina, Earth and Jack.
const OBJECTS: [(u32, &[Op]); 3] = [
    (0x0802_672C, REGINA_ASKS),
    (0x0802_6764, EARTH_CALLS),
    (0x0802_681C, JACK_CALLS),
];

/// What speaking to an object of area 9 whose script is code runs.
pub(super) fn talk_handler(address: u32) -> Option<&'static [Op]> {
    TEACHERS
        .iter()
        .chain(SHOPS.iter())
        .chain(OBJECTS.iter())
        .find(|(at, _)| *at == address)
        .map(|(_, program)| *program)
}

/// What map `map` of area 9 runs when it loads, when it runs anything.
pub(super) fn map_handler(map: usize) -> Option<&'static [Op]> {
    match map {
        HALL | HALL_SCENE => Some(HALL_ARRIVAL),
        FRAN_ROOM | FRAN_ROOM_SCENE => Some(FRAN_ROOM_ARRIVAL),
        GALE_ROOM => Some(GALE_ROOM_ARRIVAL),
        OPIS_ROOM => Some(OPIS_ROOM_ARRIVAL),
        BRIEFING_ROOM => Some(BRIEFING_ROOM_ARRIVAL),
        HEART => Some(HEART_ARRIVAL),
        _ => None,
    }
}
