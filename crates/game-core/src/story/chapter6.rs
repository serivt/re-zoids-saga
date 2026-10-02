//! Chapter 6: the Empire in the time of Prozen's regency, from Miletos
//! castle and Hagen City to the hidden lab and the Death Saurer's
//! prototype, with Stinger, Rosso's band, Raven and Gale (area 6).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the map
//! handlers of area 6 (`0x0801B284` to `0x0801BF90`), the tasks and field
//! hooks they install and the objects' code, named at each item. See
//! `docs/events.md`.

use extraction::saga::Reward;

use super::{
    FIELD_HOOK, PLAYER, SCENE_BRIGHTEN, SCENE_DARKEN, ZI_DATA_REWARD, glide, learn, shop,
    story_battle, through,
};
use crate::event::{FIELD_WATCH, HERE, MAP_TASK, Op, SEEN_GUARD};
use crate::field::{Direction, PIXEL};
use crate::menu::Shop;

/// Set once the Emperor has sent Gale for the Death Saurer's data
/// (`0x0801BF40`).
pub(super) const CHAPTER_OPENED: u16 = 0x194;
/// Set once the portal has brought the party to the Empire.
const ARRIVED: u16 = 0x195;
/// Set once the party has first seen the town below Miletos castle.
const TOWN_SEEN: u16 = 0x196;
/// Set once Van and Irvine have come to fight Gale.
const VAN_CAME: u16 = 0x19A;
/// Set once Gale is beaten (story battle 28): the chapter's end.
const GALE_BEATEN: u16 = 0x19B;
/// Set once the party has seen Gale in the town and followed him.
const GALE_SIGHTED: u16 = 0x19C;
/// Set once Rosso's band has carried Rudolph off from the castle.
const CASTLE_SEEN: u16 = 0x19D;
/// Set once Stinger has spoken to the party in Hagen City's bar.
const STINGER_MET: u16 = 0x19E;
/// Set once Stinger is beaten (story battle 26).
const STINGER_BEATEN: u16 = 0x19F;
/// Set once the party has caught Stinger in the bar again.
const STINGER_CAUGHT: u16 = 0x1A0;
/// Set once the party has found the wrecked Iron Kongs.
const WRECKS_SEEN: u16 = 0x1A1;
/// Set once the party has found Rosso's band hurt in the ruins.
const ROSSO_MET: u16 = 0x1A2;
/// Set once Raven has struck.
const RAVEN_MET: u16 = 0x1A3;
/// Set once Rosso and Viola have joined.
pub(super) const ROSSO_JOINED: u16 = 0x1A4;
/// Set once Gray Colony has pointed the party to the mountains' lab.
const LAB_POINTED: u16 = 0x1A5;
/// Set once the party has found the hidden lab.
const LAB_FOUND: u16 = 0x1A6;
/// Set once the party has reached the lab's wrecked room.
const LAB_ENTERED: u16 = 0x1A7;
/// Set once Raven has chased Gale off the plains.
const GENOSAURER_SEEN: u16 = 0x1A8;
/// Set once the party has come into Hagen City's institute.
const INSTITUTE_ENTERED: u16 = 0x1A9;
/// Set once the Death Saurer's prototype is beaten (story battle 27).
const PROTOTYPE_BEATEN: u16 = 0x1AA;

/// The area's Zoid map (176), with the portal.
const PLAINS: usize = 176;
/// The town below Miletos castle (177), the castle's grounds (178) and
/// its hall (179).
const MILETOS_TOWN: usize = 177;
const CASTLE_GROUNDS: usize = 178;
const CASTLE_HALL: usize = 179;
/// Hagen City (182), its bar (183), the Zoid institute's gate (186) and
/// its hall (188).
const HAGEN_CITY: usize = 182;
const HAGEN_BAR: usize = 183;
const INSTITUTE_GATE: usize = 186;
const INSTITUTE_HALL: usize = 188;
/// The ruins (194), and their copy the scene there loads (216).
const RUINS: usize = 194;
const RUINS_SCENE: usize = 216;
/// Gray Colony (195).
const GRAY_COLONY: usize = 195;
/// The hidden lab's door (198), its guarded passage (199) and its rooms
/// (200).
const LAB_DOOR: usize = 198;
const LAB_PASSAGE: usize = 199;
const LAB_ROOMS: usize = 200;
/// The Emperor's throne room (205), where chapter 5's end leaves the
/// party; the castle's portal room (213) and the room above the base's bar
/// (214) in the kingdom's time.
const THRONE_ROOM: usize = 205;
const PORTAL_ROOM: usize = 213;
const BASE_ROOM: usize = 214;
/// Chapter 7's first map, where the chapter's end leaves the party.
const CHAPTER_7_START: usize = 225;

/// The songs the chapter switches to.
const DANGER_MUSIC: u16 = 4;
const AFTERMATH_MUSIC: u16 = 5;
const SIGHTING_MUSIC: u16 = 6;
const ENEMY_MUSIC: u16 = 9;
/// The song the staged battle scenes play to (`0x08012040`).
const STAGED_MUSIC: u16 = 0x17;

const PORTAL_SOUND: u16 = 0x6F;
const PORTAL_ARRIVAL_SOUND: u16 = 0x49;
const BLAST_SOUND: u16 = 0x5B;
/// The blow Rosso takes at the castle.
const BLOW_SOUND: u16 = 0x7D;
/// Raven's charged-particle beam.
const BEAM_SOUND: u16 = 0x81;
const PORTAL_IDLE: usize = 0;
const PORTAL_OPENS: usize = 2;
const PORTAL_BRINGS_STEP: usize = 32;

/// Frames a task waits after the fade in before its first line.
const SETTLE: u32 = 32;
const OFF_THE_MAP: (usize, usize) = (0xFF, 0xFF);
const HIDDEN: (usize, usize) = (0xFFFF, 0xFFFF);
/// The prince standing again (`0x080089A0`).
const PRINCE_STANDING: usize = 0x98;
/// The characters of group 5, met as the chapter opens (`0x080099F0`).
const CHAPTER_GROUP: u8 = 5;
/// The Zi data Rosso gives (`0x08037A24` with `0x5B`).
pub(super) const ROSSO_GIFT: u8 = 0x5B;
/// The lists Rosso and Viola, then Van and Irvine, join with.
const ROSSO_LISTS: [u8; 2] = [0x16, 0x17];
const VAN_LISTS: [u8; 2] = [0x18, 0x19];

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

/// A scene's last beat once the screen is black (the field's hooks
/// cleared, the task ended, `0x08007188` and `0x080014A8` with 1): the
/// party is taken to `cell` of `map` with its own objects, which brightens
/// slowly.
const fn warp_to(map: usize, cell: (usize, usize)) -> [Op; 3] {
    [
        Op::Warp {
            map,
            cell,
            facing: None,
        },
        Op::FadeInHoldingSlow,
        Op::End,
    ]
}

/// The same, the screen darkened first (`0x08011E08`).
const fn back_to(map: usize, cell: (usize, usize)) -> [Op; 4] {
    let [warp, fade, end] = warp_to(map, cell);
    [Op::Call(SCENE_DARKEN), warp, fade, end]
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

/// The same over the player's cell moved by `by` cells, as the gas's
/// helpers read the player's entity.
const fn blast_near(actor: usize, by: (i32, i32)) -> [Op; 5] {
    [
        Op::PlaceNearPlayer(actor, by),
        Op::PlayOnce(actor, 0),
        Op::Sound(BLAST_SOUND),
        Op::AwaitAnimation(actor),
        Op::Place(actor, OFF_THE_MAP),
    ]
}

/// A story battle's hook: beaten, the party is taken to its return point
/// and the field brightens at once.
const BEATEN: &[Op] = &[Op::WarpHome, Op::FadeInHolding];

/// Scrolls the view a pixel, or two, a frame (`0x08008324` in a loop).
const PAN_DOWN: &[Op] = &[Op::Pan(0, PIXEL), Op::Wait(1)];
const PAN_DOWN_FAST: &[Op] = &[Op::Pan(0, 2 * PIXEL), Op::Wait(1)];
const PAN_UP: &[Op] = &[Op::Pan(0, -PIXEL), Op::Wait(1)];
const PAN_LEFT: &[Op] = &[Op::Pan(-PIXEL, 0), Op::Wait(1)];

/// The white flash's rise (`0x0801D93C`): `BLDCNT` brightens every layer
/// toward white, and `BLDY` goes up a level every other frame to 16.
const WHITEN: [Op; 33] = whiten(true);
/// Its fall (`0x0801D968`): back a level every other frame to 0, then the
/// blend is turned off a frame later.
const UNWHITEN: [Op; 33] = whiten(false);

const fn whiten(rise: bool) -> [Op; 33] {
    let mut ops = [Op::End; 33];
    let mut step: u8 = 0;
    while step < 16 {
        let level = if rise { step + 1 } else { 15 - step };
        ops[2 * step as usize] = Op::Whiten(level);
        ops[2 * step as usize + 1] = Op::Wait(2);
        step += 1;
    }
    ops[32] = if rise { Op::End } else { Op::Wait(1) };
    ops
}
const UNWHITEN_TASK: &[Op] = &[Op::Call(&UNWHITEN), Op::End];

/// Frames the flash's caller waits for `BLDY` to be 0 again.
const UNWHITEN_WAIT: u32 = 31;

/// Raven's beam (`0x0801DB44`): the field whitens while the beam's sound
/// plays, and fades back.
const BEAM_FLASH: &[Op] = &[
    Op::Spawn(MAP_TASK + 1, &WHITEN),
    Op::Sound(BEAM_SOUND),
    Op::AwaitSoundEnd(BEAM_SOUND),
    Op::Spawn(MAP_TASK + 1, UNWHITEN_TASK),
    Op::Wait(UNWHITEN_WAIT),
];

/// The throne room for the opening (ROM `0x08669888`): the Emperor, and
/// Gale, Blood and Opis below.
const THRONE_OBJECTS: u32 = 0x0866_9888;
const GALE: usize = 2;
const BLOOD: usize = 3;
const OPIS: usize = 4;
/// The portal room (ROM `0x086698EC`): two soldiers.
const PORTAL_ROOM_OBJECTS: u32 = 0x0866_98EC;
/// The room above the base's bar (ROM `0x08669928`): the prince seated,
/// Regina, and Jack and Earth off the map.
const BASE_OBJECTS: u32 = 0x0866_9928;
const REGINA: usize = 1;
const JACK: usize = 2;
const EARTH: usize = 3;

const GALE_STEPS_UP: &[Op] = &[glide16(GALE, (0x70, 0x30), 1, true), Op::End];
const BLOOD_STEPS_UP: &[Op] = &[glide16(BLOOD, (0x80, 0x30), 1, true), Op::End];
const BLOOD_LEAVES: &[Op] = &[
    glide16(BLOOD, (0x80, 0xC0), 1, true),
    Op::Place(BLOOD, OFF_THE_MAP),
    Op::End,
];
const JACK_COMES_IN: &[Op] = &[
    Op::Place(JACK, (0xE, 2)),
    glide16(JACK, (0x30, 0x20), 1, true),
    glide16(JACK, (0x30, 0x30), 1, true),
    Op::Face(JACK, Direction::Down),
    Op::End,
];

/// The chapter's opening (task at `0x0801C028`): Blood and Opis report
/// to the Emperor (`0x18D`); Gale is sent for the Death Saurer's data
/// (`0x18E`) and the Emperor laughs (`0x18F`); in the portal room the
/// soldiers sense the device in use again (`0x190`); in the room above the
/// bar Jack brings the news (`0x191`) and the party leaves.
const OPENING: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Spawn(MAP_TASK + 1, GALE_STEPS_UP),
    Op::Spawn(MAP_TASK + 2, BLOOD_STEPS_UP),
    glide16(OPIS, (0x90, 0x30), 1, true),
    Op::Dialogue(0x18D),
    Op::Spawn(MAP_TASK + 1, BLOOD_LEAVES),
    glide16(OPIS, (0x90, 0xC0), 1, true),
    Op::Place(OPIS, OFF_THE_MAP),
    glide16(GALE, (0x80, 0x30), 1, true),
    Op::Face(GALE, Direction::Up),
    Op::Dialogue(0x18E),
    glide16(GALE, (0x80, 0xC0), 1, true),
    Op::Place(GALE, OFF_THE_MAP),
    Op::Dialogue(0x18F),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PORTAL_ROOM,
        player: (0xA, 0xB),
        objects: PORTAL_ROOM_OBJECTS,
        count: 3,
    },
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    glide16(2, (0x80, 0xD0), 2, true),
    glide16(2, (0xA0, 0xD0), 2, true),
    Op::Face(2, Direction::Up),
    Op::Wait(60),
    Op::RestartMusic(DANGER_MUSIC),
    Op::Dialogue(0x190),
    glide16(1, (0xA0, 0xC0), 2, true),
    glide16(1, (0xD0, 0xC0), 2, true),
    Op::Place(1, OFF_THE_MAP),
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
    Op::Face(REGINA, Direction::Up),
    Op::Dialogue(0x191),
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

/// The throne room (`0x0801BF40`): the first time, it marks group 5 as met
/// and plays the opening.
const THRONE_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[CHAPTER_OPENED],
    then: &[
        Op::Meet(CHAPTER_GROUP),
        Op::LoadMap {
            map: THRONE_ROOM,
            player: (8, 2),
            objects: THRONE_OBJECTS,
            count: 5,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, OPENING),
        Op::Place(PLAYER, HIDDEN),
        Op::Flag(CHAPTER_OPENED, true),
    ],
    otherwise: &[],
}];

/// The plains with the portal (ROM `0x08669498`).
const ARRIVAL_OBJECTS: u32 = 0x0866_9498;
const PLAINS_PORTAL: usize = 1;
const PLAINS_PORTAL_CELL: (usize, usize) = (5, 0x15);
const PLAINS_ARRIVAL_CELL: (usize, usize) = (4, 0x15);
/// The plains' return point: the portal.
const PLAINS_RETURN_POINT: u8 = 0x10;

/// Into the Empire (task at `0x0801C35C`): the portal brings the Gustav,
/// and the party finds Miletos castle near (`0x193`).
const ARRIVAL: &[Op] = &[
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
    Op::Dialogue(0x193),
    Op::Call(&back_to(PLAINS, PLAINS_ARRIVAL_CELL)),
];

/// Stinger's ambush on the plains (ROM `0x08669C20`): Stinger's Zoid and
/// three gas clouds, all off the map.
const AMBUSH_OBJECTS: u32 = 0x0866_9C20;
const STINGER_ZOID: usize = 1;
const GAS_1: usize = 2;
const GAS_2: usize = 3;
const GAS_3: usize = 4;
const GAS_1_BLAST: [Op; 5] = blast_near(GAS_1, (-1, 1));
const GAS_2_BLAST: [Op; 5] = blast_near(GAS_2, (1, -1));
const GAS_3_BLAST: [Op; 5] = blast_near(GAS_3, (2, -2));
const GAS_3_BLAST_LEFT: [Op; 5] = blast_near(GAS_3, (-1, -2));
/// The gas's helpers (`0x0801D500`, `0x0801D52C`, `0x0801D558`).
const GAS_1_LOOP: &[Op] = &[Op::Loop(&[Op::Wait(12), Op::Call(&GAS_1_BLAST)])];
const GAS_2_LOOP: &[Op] = &[Op::Loop(&[Op::Wait(30), Op::Call(&GAS_2_BLAST)])];
const GAS_3_LOOP: &[Op] = &[Op::Loop(&[
    Op::Wait(8),
    Op::Call(&GAS_3_BLAST),
    Op::Wait(8),
    Op::Call(&GAS_3_BLAST_LEFT),
])];

/// Stinger's fight (story battle 26).
const STINGER_BATTLE_OPS: [Op; 4] = story_battle(26);
/// Stinger's battle (the hook at `0x0801D42C`): won, Stinger gets away
/// (task at `0x0801D4AC`, `0x1B3`) and the plains load again where the
/// party stands.
const STINGER_FIGHT: &[Op] = &[
    Op::Call(&STINGER_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::FadeInHolding,
            Op::Flag(STINGER_BEATEN, true),
            Op::Control(false),
            Op::Place(STINGER_ZOID, OFF_THE_MAP),
            Op::Spawn(MAP_TASK, STINGER_GETS_AWAY),
        ],
    },
    Op::End,
];
const STINGER_GETS_AWAY: &[Op] = &[Op::Dialogue(0x1B3), Op::Call(&back_to(PLAINS, HERE))];

/// The ambush (task at `0x0801D328`): the plains load again around the
/// party with Stinger's Zoid ten cells right, gas bursts around it while
/// the party wonders (`0x1B0`), and Stinger drives up (`0x1B1`, `0x1B2`).
const AMBUSH: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PLAINS,
        player: HERE,
        objects: AMBUSH_OBJECTS,
        count: 5,
    },
    Op::RestartMusic(ENEMY_MUSIC),
    Op::PlaceNearPlayer(STINGER_ZOID, (10, 0)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, GAS_1_LOOP),
    Op::Spawn(MAP_TASK + 2, GAS_2_LOOP),
    Op::Spawn(MAP_TASK + 3, GAS_3_LOOP),
    Op::Dialogue(0x1B0),
    Op::Stop(MAP_TASK + 1),
    Op::Stop(MAP_TASK + 2),
    Op::Stop(MAP_TASK + 3),
    Op::Place(GAS_1, OFF_THE_MAP),
    Op::Place(GAS_2, OFF_THE_MAP),
    Op::Place(GAS_3, OFF_THE_MAP),
    Op::Dialogue(0x1B1),
    Op::GlideNearPlayer {
        actor: STINGER_ZOID,
        by: (1, 0),
        speed: 2,
        camera: false,
    },
    Op::Dialogue(0x1B2),
    Op::Spawn(FIELD_HOOK, STINGER_FIGHT),
    Op::End,
];

/// Enemies no longer meet the party, which loses its controls, and
/// `task` starts (the hooks at `0x0801B45C`, `0x0801B4C4`, `0x0801B524`).
macro_rules! calm_and_run {
    ($task:expr) => {
        &[
            Op::Calm(true),
            Op::Control(false),
            Op::Spawn(MAP_TASK, $task),
        ]
    };
}

/// Where Stinger ambushes the party (the hook at `0x0801B45C`): on row 10
/// left of column 21, or on cell (20, 12).
const AMBUSH_WATCH: &[Op] = &[Op::Loop(&[
    Op::IfPlayerSprite {
        x: (0, 0x280),
        y: (0x140, 0x140),
        then: &[Op::Call(calm_and_run!(AMBUSH)), Op::End],
        otherwise: &[Op::IfPlayerSprite {
            x: (0x280, 0x280),
            y: (0x180, 0x180),
            then: &[Op::Call(calm_and_run!(AMBUSH)), Op::End],
            otherwise: &[],
        }],
    },
    Op::Wait(1),
])];

/// The wrecked Iron Kongs (task at `0x0801D594`): to the danger song the
/// party finds them (`0x1B4`); enemies meet it again.
const WRECKS: &[Op] = &[
    Op::RestartMusic(DANGER_MUSIC),
    Op::Dialogue(0x1B4),
    Op::Control(true),
    Op::Calm(false),
    Op::End,
];
/// Where the party finds them (the hook at `0x0801B4C4`): cell (27, 5).
const WRECKS_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x360, 0x360),
        y: (0xA0, 0xA0),
    },
    Op::Call(calm_and_run!(WRECKS)),
    Op::Flag(WRECKS_SEEN, true),
    Op::End,
];

/// Raven's attack on the plains (ROM `0x08669D4C`): four Command Wolves of
/// the Republic's intelligence, Van's Blade Liger, Raven's Genosaurer and
/// an explosion.
const RAVEN_OBJECTS: u32 = 0x0866_9D4C;
const WOLVES: [usize; 4] = [1, 2, 3, 4];
const AGENT: usize = 4;
const VAN_LIGER: usize = 5;
const GENOSAURER: usize = 6;
const EXPLOSION: usize = 7;
const WOLF_1_BLAST: [Op; 5] = blast(EXPLOSION, (24, 10));
const WOLF_2_BLAST: [Op; 5] = blast(EXPLOSION, (25, 10));
const WOLF_3_BLAST: [Op; 5] = blast(EXPLOSION, (26, 10));

/// Raven's attack (task at `0x0801D9A0`): the plains load by the fight,
/// blasts and beams sound beyond the ridge (`0x1BC`), the party drives up
/// and the view drops to the fight; the hook at `0x0801DB08` stages
/// Raven's shot at a Command Wolf (battle scene 20).
const RAVEN_ATTACK: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PLAINS,
        player: (0x16, 7),
        objects: RAVEN_OBJECTS,
        count: 8,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Wait(6),
    Op::Sound(BLAST_SOUND),
    Op::AwaitSoundEnd(BLAST_SOUND),
    Op::Wait(12),
    Op::Sound(BLAST_SOUND),
    Op::Wait(30),
    Op::Sound(BLAST_SOUND),
    Op::AwaitSoundEnd(BLAST_SOUND),
    Op::Wait(6),
    Op::Dialogue(0x1BC),
    Op::Sound(BEAM_SOUND),
    Op::AwaitSoundEnd(BEAM_SOUND),
    Op::Wait(6),
    Op::Sound(BLAST_SOUND),
    Op::AwaitSoundEnd(BLAST_SOUND),
    Op::Wait(6),
    Op::Sound(BLAST_SOUND),
    Op::AwaitSoundEnd(BLAST_SOUND),
    glide(PLAYER, (0x2E0, 0xE0), 1, true),
    glide(PLAYER, (0x2E0, 0x100), 1, true),
    glide(PLAYER, (0x320, 0x100), 1, true),
    glide(PLAYER, (0x320, 0x120), 1, true),
    Op::Face(PLAYER, Direction::Down),
    Op::Repeat(32, PAN_DOWN),
    Op::Spawn(FIELD_HOOK, RAVEN_SHOOTS),
    Op::End,
];
/// Where Raven's attack starts (the hook at `0x0801B524`): cell (22, 7).
const RAVEN_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x2C0, 0x2C0),
        y: (0xE0, 0xE0),
    },
    Op::Call(calm_and_run!(RAVEN_ATTACK)),
    Op::Flag(RAVEN_MET, true),
    Op::End,
];

/// A staged battle scene to its song (`0x08012040`): the song plays, the
/// scene runs, and the song that played comes back.
macro_rules! staged {
    ($scene:expr, $song:expr) => {
        &[
            Op::Music(STAGED_MUSIC),
            Op::Battle($scene),
            Op::Music($song),
        ]
    };
}

/// Raven's shot (the hook at `0x0801DB08`, battle scene 20): then the
/// Wolves are finished off (task at `0x0801DB44`).
const RAVEN_SHOOTS: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Call(staged!(20, DANGER_MUSIC)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Control(false),
    Op::Spawn(MAP_TASK, WOLVES_FALL),
    Op::End,
];

const ROSSO_GIFT_OPS: &[Op] = &[
    Op::Gift(Some(Reward::ZiData(ROSSO_GIFT))),
    Op::Dialogue(0x1F),
    Op::Call(ZI_DATA_REWARD),
    Op::Dialogue(0x22),
    Op::Gift(None),
];

/// The Wolves fall (task at `0x0801DB44`): three blow up; the party drives
/// to the last, the agent in it (`0x1BD`, `0x1BE`); with Rosso in the
/// party Rosso knows the Zoid, gives a Zi data and leaves with Viola
/// (`0x1BF`), else the beam flashes and the agent panics (`0x1C1`); the
/// agent flees, Raven's beam flashes after Rosso's words (`0x1C0`); to
/// the aftermath's song the view sweeps to Van facing Raven (`0x1C2`,
/// `0x1C3`), and the hook at `0x0801DD74` stages their fight.
const WOLVES_FALL: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Call(&WOLF_1_BLAST),
    Op::Place(WOLVES[0], OFF_THE_MAP),
    Op::Call(&WOLF_2_BLAST),
    Op::Place(WOLVES[1], OFF_THE_MAP),
    Op::Call(&WOLF_3_BLAST),
    Op::Place(WOLVES[2], OFF_THE_MAP),
    Op::Dialogue(0x1BD),
    glide(PLAYER, (0x320, 0x160), 1, true),
    Op::Face(PLAYER, Direction::Left),
    Op::Dialogue(0x1BE),
    Op::IfFlags {
        all: &[ROSSO_JOINED],
        none: &[],
        then: &[
            Op::Dialogue(0x1BF),
            Op::Call(ROSSO_GIFT_OPS),
            Op::Leave(ROSSO_LISTS[0]),
            Op::Leave(ROSSO_LISTS[1]),
        ],
        otherwise: &[Op::Call(BEAM_FLASH), Op::Dialogue(0x1C1)],
    },
    glide(AGENT, (0x300, 0x200), 2, true),
    Op::Place(AGENT, OFF_THE_MAP),
    Op::IfFlags {
        all: &[ROSSO_JOINED],
        none: &[],
        then: &[Op::Dialogue(0x1C0), Op::Call(BEAM_FLASH)],
        otherwise: &[],
    },
    Op::RestartMusic(AFTERMATH_MUSIC),
    Op::Dialogue(0x1C2),
    Op::Repeat(288, PAN_LEFT),
    Op::Repeat(64, PAN_UP),
    Op::Place(PLAYER, (0x14, 9)),
    glide(PLAYER, (0x260, 0x120), 1, false),
    Op::Dialogue(0x1C3),
    Op::Spawn(FIELD_HOOK, VAN_FIGHTS_RAVEN),
    Op::End,
];

/// Van's shot, which finds no one, and Raven's at him (the hook at
/// `0x0801DD74`, battle scenes 4 and 5 from the list at ROM
/// `0x08669494`): Van is outmatched (task at
/// `0x0801DDC8`, `0x1C4`, `0x1C5`) and his Liger's animation runs out; the
/// hook at `0x0801DE1C` stages Raven's last shot (scene 6).
const VAN_FIGHTS_RAVEN: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Music(STAGED_MUSIC),
    Op::Battle(4),
    Op::Battle(5),
    Op::Music(AFTERMATH_MUSIC),
    Op::RestartMusic(AFTERMATH_MUSIC),
    Op::Control(false),
    Op::Spawn(MAP_TASK, VAN_OUTMATCHED),
    Op::End,
];
const VAN_OUTMATCHED: &[Op] = &[
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x1C4),
    Op::Dialogue(0x1C5),
    Op::Once(VAN_LIGER),
    Op::Spawn(FIELD_HOOK, RAVEN_FINISHES),
    Op::End,
];

/// Van's Liger fallen (ROM `0x08669DEC`).
const FALLEN_LIGER_OBJECTS: u32 = 0x0866_9DEC;
const FALLEN_LIGER: usize = 1;

/// Raven's last shot (the hook at `0x0801DE1C`, battle scene 6): Fiene
/// cries out (task at `0x0801DE58`, `0x1C6`), Raven drives off to the
/// map's song, and Dr. D comes to the fallen Liger (`0x1C7`).
const RAVEN_FINISHES: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Call(staged!(6, AFTERMATH_MUSIC)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Control(false),
    Op::Spawn(MAP_TASK, RAVEN_LEAVES),
    Op::End,
];
const RAVEN_LEAVES: &[Op] = &[
    Op::Dialogue(0x1C6),
    Op::RestartMapMusic,
    glide(GENOSAURER, (0x180, 0x140), 1, true),
    Op::Place(GENOSAURER, OFF_THE_MAP),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PLAINS,
        player: (0x12, 0xA),
        objects: FALLEN_LIGER_OBJECTS,
        count: 2,
    },
    Op::Place(PLAYER, (0x13, 0xA)),
    Op::Once(FALLEN_LIGER),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x1C7),
    Op::Call(&back_to(PLAINS, (0x13, 0xA))),
];

/// Raven and Gale on the plains (ROM `0x086694C0`): the Genosaurer, Gale's
/// Zoid and a Zoid of the party's off the map.
const GENOSAURER_OBJECTS: u32 = 0x0866_94C0;
const RAVEN_ZOID: usize = 1;
const GALE_ZOID: usize = 2;
const SCOUT: usize = 3;

/// Raven finds Gale (task at `0x0801E044`): the party sees the
/// Genosaurer (`0x1D3`), a Zoid of the party's drives ahead and Raven
/// turns (`0x1D4`); the hook at `0x0801E0BC` stages Gale's smoke screen
/// (scene 7).
const GENOSAURER_SEEN_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Dialogue(0x1D3),
    Op::Place(SCOUT, (0x11, 0x15)),
    Op::Wait(15),
    glide(SCOUT, (0x220, 0x260), 2, false),
    Op::Face(SCOUT, Direction::Left),
    Op::Wait(15),
    Op::Dialogue(0x1D4),
    Op::Spawn(FIELD_HOOK, GALE_SMOKE),
    Op::End,
];
const GALE_SMOKE: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Call(staged!(7, ENEMY_MUSIC)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Control(false),
    Op::Spawn(MAP_TASK, GALE_ESCAPES),
    Op::End,
];
const SCOUT_LEAVES: &[Op] = &[
    glide(SCOUT, (0x220, 0x2A0), 2, false),
    Op::Place(SCOUT, OFF_THE_MAP),
    Op::End,
];
/// Gale gets away in the smoke (task at `0x0801E0F8`): he calls Jack to
/// Hagen City's institute (`0x1D5`) and goes, Raven chases him (`0x1D6`),
/// and the party makes for the institute (`0x1D7`, `0x1D8`).
const GALE_ESCAPES: &[Op] = &[
    Op::Dialogue(0x1D5),
    glide(GALE_ZOID, (0x240, 0x200), 2, false),
    Op::Place(GALE_ZOID, OFF_THE_MAP),
    Op::Dialogue(0x1D6),
    glide(RAVEN_ZOID, (0x1E0, 0x260), 2, false),
    glide(RAVEN_ZOID, (0x1E0, 0x200), 2, false),
    Op::Place(RAVEN_ZOID, OFF_THE_MAP),
    Op::Dialogue(0x1D7),
    Op::Wait(15),
    Op::Face(SCOUT, Direction::Down),
    Op::Wait(15),
    Op::Dialogue(0x1D8),
    Op::Spawn(MAP_TASK + 1, SCOUT_LEAVES),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::Call(&warp_to(PLAINS, (0x11, 0x15))),
];

/// The plains after the prototype (ROM `0x08669E78`, and `0x08669510`
/// the same): the prototype, Gale's Zoid, Van's and Irvine's, and the
/// portal.
const GALE_FIGHT_OBJECTS: u32 = 0x0866_9510;
const PROTOTYPE_OBJECTS: u32 = 0x0866_9E78;
const PROTOTYPE: usize = 1;
const VAN_ZOID: usize = 3;
const IRVINE_ZOID: usize = 4;

/// Gale's fight (story battle 28).
const GALE_BATTLE_OPS: [Op; 4] = story_battle(28);
/// Gale's battle (the hook at `0x0801E970`): won, the chapter's end.
const GALE_FIGHT: &[Op] = &[
    Op::Call(&GALE_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::FadeInHolding,
            Op::Control(false),
            Op::Spawn(MAP_TASK, FAREWELL),
            Op::Flag(GALE_BEATEN, true),
        ],
    },
    Op::End,
];

/// Gale comes (`0x0801E848`): the party beat the prototype (`0x1DF`) and
/// Gale drives up for the real thing (`0x1E0`); the first time Van and
/// Irvine come to fight along (`0x1E1`, lists 24 and 25), later they
/// stand by; the hook at `0x0801E970` starts the fight.
const GALE_COMES: &[Op] = &[
    Op::Dialogue(0x1DF),
    Op::Place(GALE_ZOID, (4, 0xE)),
    glide(GALE_ZOID, (0xE0, 0x1C0), 1, false),
    Op::Face(PLAYER, Direction::Left),
    Op::Dialogue(0x1E0),
    Op::IfFlags {
        all: &[],
        none: &[VAN_CAME],
        then: &[
            Op::Flag(VAN_CAME, true),
            Op::Place(VAN_ZOID, (0xE, 0xC)),
            glide(VAN_ZOID, (0x140, 0x1C0), 1, false),
            Op::Place(IRVINE_ZOID, (0xE, 0xD)),
            glide(IRVINE_ZOID, (0x140, 0x1A0), 1, false),
            Op::RestartMusic(ENEMY_MUSIC),
            Op::Dialogue(0x1E1),
            Op::Join(VAN_LISTS[0]),
            Op::Join(VAN_LISTS[1]),
        ],
        otherwise: &[
            Op::Place(IRVINE_ZOID, (9, 0xE)),
            glide(IRVINE_ZOID, (0x140, 0x1A0), 1, false),
            Op::Face(IRVINE_ZOID, Direction::Left),
            Op::Place(VAN_ZOID, (9, 0xE)),
            glide(VAN_ZOID, (0x140, 0x1C0), 1, false),
            Op::Face(VAN_ZOID, Direction::Left),
        ],
    },
    Op::Spawn(FIELD_HOOK, GALE_FIGHT),
    Op::End,
];
/// Back on the plains before Gale is beaten (task at `0x0801E828`).
const GALE_RETURNS: &[Op] = &[Op::Wait(SETTLE), Op::Call(GALE_COMES)];
/// The prototype beaten (task at `0x0801E838`).
const PROTOTYPE_FALLS: &[Op] = &[Op::Call(SCENE_BRIGHTEN), Op::Call(GALE_COMES)];

const PLAYER_DRIVES_OFF: &[Op] = &[glide(PLAYER, (0x1E0, 0x1C0), 1, false), Op::End];
/// Gale beaten (task at `0x0801E9E0`): Gale withdraws (`0x1E2`), Irvine
/// and Van go their own way (`0x1E3`, `0x1E4`, lists 24 and 25 leave),
/// Jack wonders about Gale (`0x1E5`); the hook at `0x0801EAA8` warps to
/// map 225, chapter 7's first.
const FAREWELL: &[Op] = &[
    Op::Dialogue(0x1E2),
    glide(GALE_ZOID, (0x80, 0x1C0), 2, false),
    Op::Place(GALE_ZOID, OFF_THE_MAP),
    Op::Dialogue(0x1E3),
    glide(IRVINE_ZOID, (0x1C0, 0x1A0), 1, false),
    Op::Place(IRVINE_ZOID, OFF_THE_MAP),
    Op::Dialogue(0x1E4),
    glide(VAN_ZOID, (0x1C0, 0x1C0), 2, false),
    Op::Place(VAN_ZOID, OFF_THE_MAP),
    Op::Dialogue(0x1E5),
    Op::Leave(VAN_LISTS[0]),
    Op::Leave(VAN_LISTS[1]),
    Op::Spawn(MAP_TASK + 1, PLAYER_DRIVES_OFF),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::Spawn(FIELD_HOOK, TO_CHAPTER_7),
    Op::End,
];
const TO_CHAPTER_7: &[Op] = &warp_to(CHAPTER_7_START, (8, 2));

/// The plains (`0x0801B284`): the portal brings the party the first time;
/// then, as the story goes, Stinger's ambush, the wrecks and Raven's
/// attack wait at their spots, Raven finds Gale on the way back from the
/// lab, and Gale waits until he is beaten.
const PLAINS_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CHAPTER_OPENED],
    none: &[ARRIVED],
    then: &[
        Op::LoadMap {
            map: PLAINS,
            player: PLAINS_PORTAL_CELL,
            objects: ARRIVAL_OBJECTS,
            count: 2,
        },
        Op::Place(PLAYER, OFF_THE_MAP),
        Op::Control(false),
        Op::Spawn(MAP_TASK, ARRIVAL),
        Op::ReturnPoint(PLAINS_RETURN_POINT),
        Op::Flag(ARRIVED, true),
    ],
    otherwise: &[Op::IfFlags {
        all: &[STINGER_MET],
        none: &[STINGER_BEATEN],
        then: &[Op::Spawn(FIELD_WATCH, AMBUSH_WATCH)],
        otherwise: &[Op::IfFlags {
            all: &[STINGER_BEATEN],
            none: &[WRECKS_SEEN],
            then: &[Op::Spawn(FIELD_WATCH, WRECKS_WATCH)],
            otherwise: &[Op::IfFlags {
                all: &[ROSSO_MET],
                none: &[RAVEN_MET],
                then: &[Op::Spawn(FIELD_WATCH, RAVEN_WATCH)],
                otherwise: &[Op::IfFlags {
                    all: &[LAB_ENTERED],
                    none: &[GENOSAURER_SEEN],
                    then: &[
                        Op::LoadMap {
                            map: PLAINS,
                            player: (0x11, 0x13),
                            objects: GENOSAURER_OBJECTS,
                            count: 4,
                        },
                        Op::RestartMusic(ENEMY_MUSIC),
                        Op::Place(PLAYER, (0x11, 0x15)),
                        Op::Control(false),
                        Op::Spawn(MAP_TASK, GENOSAURER_SEEN_SCENE),
                        Op::Flag(GENOSAURER_SEEN, true),
                    ],
                    otherwise: &[Op::IfFlags {
                        all: &[PROTOTYPE_BEATEN],
                        none: &[GALE_BEATEN],
                        then: &[
                            Op::LoadMap {
                                map: PLAINS,
                                player: (9, 0xE),
                                objects: GALE_FIGHT_OBJECTS,
                                count: 6,
                            },
                            Op::Control(false),
                            Op::Spawn(MAP_TASK, GALE_RETURNS),
                        ],
                        otherwise: &[],
                    }],
                }],
            }],
        }],
    }],
}];

/// The town below Miletos castle for its first view (ROM `0x08669588`)
/// and for Gale's sighting (ROM `0x08669978`: Regina, Jack, Earth and
/// Gale); the castle's grounds (ROM `0x086699DC`).
const TOWN_OBJECTS: u32 = 0x0866_9588;
const SIGHTING_OBJECTS: u32 = 0x0866_9978;
const CASTLE_GROUNDS_OBJECTS: u32 = 0x0866_99DC;
const TOWN_GALE: usize = 4;

/// The town's first view (task at `0x0801C47C`): the view sweeps down the
/// festive streets (`0x194`).
const TOWN_VIEW: &[Op] = &[
    Op::Repeat(192, PAN_DOWN_FAST),
    Op::Wait(SETTLE),
    Op::Dialogue(0x194),
    Op::Call(&back_to(MILETOS_TOWN, (0x13, 0x1C))),
];

const PLAYER_RUNS_UP: &[Op] = &[glide16(PLAYER, (0xC0, 0), 2, false), Op::End];
const REGINA_RUNS_UP: &[Op] = &[glide16(REGINA, (0xB0, 0), 2, true), Op::End];
const JACK_RUNS_UP: &[Op] = &[glide16(JACK, (0xD0, 0), 2, true), Op::End];
const EARTH_RUNS_UP: &[Op] = &[glide16(EARTH, (0xE0, 0), 2, true), Op::End];

/// Gale sighted (task at `0x0801C4E0`): to its song the party spots Gale
/// (`0x19A`, `0x19B`), who walks off north; the party follows (`0x19C`)
/// to the castle's grounds (`0x19D`).
const GALE_SIGHTING: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: MILETOS_TOWN,
        player: (0xC, 4),
        objects: SIGHTING_OBJECTS,
        count: 5,
    },
    Op::RestartMusic(SIGHTING_MUSIC),
    Op::Place(PLAYER, (0xC, 6)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x19A),
    Op::Face(TOWN_GALE, Direction::Down),
    Op::Wait(15),
    Op::Dialogue(0x19B),
    glide16(TOWN_GALE, (0xC0, 0), 1, true),
    Op::Place(TOWN_GALE, OFF_THE_MAP),
    Op::Dialogue(0x19C),
    Op::Spawn(MAP_TASK + 1, PLAYER_RUNS_UP),
    Op::Spawn(MAP_TASK + 2, REGINA_RUNS_UP),
    Op::Spawn(MAP_TASK + 3, JACK_RUNS_UP),
    Op::Spawn(MAP_TASK + 4, EARTH_RUNS_UP),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::Stop(MAP_TASK + 2),
    Op::Stop(MAP_TASK + 3),
    Op::Stop(MAP_TASK + 4),
    Op::LoadMap {
        map: CASTLE_GROUNDS,
        player: (0xD, 7),
        objects: CASTLE_GROUNDS_OBJECTS,
        count: 1,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x19D),
    Op::Call(&back_to(CASTLE_GROUNDS, (0xD, 7))),
];
/// Where the party spots Gale (the hook at `0x0801B618`): on row 6,
/// columns 10 to 14.
const SIGHTING_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x98, 0xD8),
        y: (0x60, 0x60),
    },
    Op::Control(false),
    Op::Spawn(MAP_TASK, GALE_SIGHTING),
    Op::Flag(GALE_SIGHTED, true),
    Op::End,
];

/// The town (`0x0801B584`): the first time the view sweeps its streets;
/// until Gale is sighted, the spot waits.
const TOWN_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[ARRIVED],
    none: &[TOWN_SEEN],
    then: &[
        Op::LoadMap {
            map: MILETOS_TOWN,
            player: (0x13, 5),
            objects: TOWN_OBJECTS,
            count: 14,
        },
        Op::Place(PLAYER, (0x13, 0x1C)),
        Op::Control(false),
        Op::Spawn(MAP_TASK, TOWN_VIEW),
        Op::Flag(TOWN_SEEN, true),
    ],
    otherwise: &[Op::IfFlags {
        all: &[],
        none: &[GALE_SIGHTED],
        then: &[Op::Spawn(FIELD_WATCH, SIGHTING_WATCH)],
        otherwise: &[],
    }],
}];

/// A walk of the castle's hall, in one of its two wings, before Rosso's
/// band strikes (ROM `0x086699F0` and `0x08669B08`: Regina, Jack and
/// Earth; ROM `0x08669A40` and `0x08669B58`: them, Rosso, Viola, Rudolph
/// and two of the band's men off the map).
const WEST_WING_OBJECTS: u32 = 0x0866_99F0;
const EAST_WING_OBJECTS: u32 = 0x0866_9B08;
const WEST_THRONE_OBJECTS: u32 = 0x0866_9A40;
const EAST_THRONE_OBJECTS: u32 = 0x0866_9B58;
const ROSSO: usize = 4;
const VIOLA: usize = 5;
const RUDOLPH: usize = 6;
const MAN_1: usize = 7;
const MAN_2: usize = 8;
/// The plains after the castle (ROM `0x08669AF4` and `0x08669C0C`).
const AFTER_CASTLE_OBJECTS: u32 = 0x0866_9AF4;
const AFTER_CASTLE_OBJECTS_EAST: u32 = 0x0866_9C0C;

const WEST_REGINA: &[Op] = &[
    glide16(REGINA, (0xB0, 0x30), 1, false),
    glide16(REGINA, (0xB0, 0x50), 1, false),
    glide16(REGINA, (0xD0, 0x50), 1, false),
    Op::End,
];
const WEST_JACK: &[Op] = &[
    glide16(JACK, (0xA0, 0x40), 1, false),
    glide16(JACK, (0xA0, 0x60), 1, false),
    glide16(JACK, (0xC0, 0x60), 1, false),
    Op::End,
];
const WEST_EARTH: &[Op] = &[
    glide16(EARTH, (0xA0, 0x30), 1, false),
    glide16(EARTH, (0xA0, 0x50), 1, false),
    glide16(EARTH, (0xC0, 0x50), 1, false),
    Op::End,
];
const WEST_MAN_2: &[Op] = &[glide16(MAN_2, (0x120, 0x70), 2, false), Op::End];
const WEST_VIOLA: &[Op] = &[
    glide16(VIOLA, (0xE0, 0x70), 2, false),
    glide16(VIOLA, (0x70, 0x70), 2, false),
    Op::End,
];
const WEST_RUDOLPH: &[Op] = &[
    glide16(RUDOLPH, (0xF0, 0x70), 2, false),
    glide16(RUDOLPH, (0x70, 0x70), 2, false),
    Op::End,
];
const WEST_PLAYER_OFF: &[Op] = &[glide16(PLAYER, (0x70, 0x60), 2, false), Op::End];
const WEST_REGINA_OFF: &[Op] = &[glide16(REGINA, (0x70, 0x50), 2, false), Op::End];
const WEST_JACK_OFF: &[Op] = &[glide16(JACK, (0x70, 0x60), 2, false), Op::End];
const WEST_EARTH_OFF: &[Op] = &[glide16(EARTH, (0x70, 0x50), 2, false), Op::End];

/// The castle's hall from its west wing (task at `0x0801C684`): the party
/// walks in, and to the danger song Rosso's band attacks Rudolph
/// (`0x19E`), an imperial shot strikes Rosso twice (`0x19F`), two of the
/// band's men drive in (`0x1A0`) and the band carries Rudolph off west;
/// the party gives chase (`0x1A1`) and on the plains wonders about the
/// boy (`0x1A2`).
const WEST_WING: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: CASTLE_HALL,
        player: (4, 4),
        objects: WEST_WING_OBJECTS,
        count: 4,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, WEST_REGINA),
    Op::Spawn(MAP_TASK + 2, WEST_JACK),
    Op::Spawn(MAP_TASK + 3, WEST_EARTH),
    glide16(PLAYER, (0xB0, 0x40), 1, false),
    glide16(PLAYER, (0xB0, 0x60), 1, false),
    glide16(PLAYER, (0xD0, 0x60), 1, false),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: CASTLE_HALL,
        player: (0xF, 6),
        objects: WEST_THRONE_OBJECTS,
        count: 9,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Place(PLAYER, (0xD, 6)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Call(ROSSO_STRUCK),
    Op::Place(MAN_1, (0x17, 6)),
    Op::Place(MAN_2, (0x17, 7)),
    Op::Spawn(MAP_TASK + 1, WEST_MAN_2),
    glide16(MAN_1, (0x120, 0x60), 2, false),
    Op::Dialogue(0x1A0),
    Op::Spawn(MAP_TASK + 1, WEST_VIOLA),
    Op::Spawn(MAP_TASK + 2, WEST_RUDOLPH),
    glide16(ROSSO, (0x100, 0x70), 2, false),
    glide16(ROSSO, (0x70, 0x70), 2, false),
    Op::Face(PLAYER, Direction::Left),
    Op::Face(REGINA, Direction::Left),
    Op::Face(JACK, Direction::Left),
    Op::Face(EARTH, Direction::Left),
    Op::Dialogue(0x1A1),
    Op::Spawn(MAP_TASK + 1, WEST_PLAYER_OFF),
    Op::Spawn(MAP_TASK + 2, WEST_REGINA_OFF),
    Op::Spawn(MAP_TASK + 3, WEST_JACK_OFF),
    Op::Spawn(MAP_TASK + 4, WEST_EARTH_OFF),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::Stop(MAP_TASK + 2),
    Op::Stop(MAP_TASK + 3),
    Op::Stop(MAP_TASK + 4),
    Op::LoadMap {
        map: PLAINS,
        player: (8, 0x13),
        objects: AFTER_CASTLE_OBJECTS,
        count: 1,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x1A2),
    Op::Call(&back_to(PLAINS, (8, 0x13))),
];

/// Rosso's band at the throne, three seconds after the fade in (`0x19E`),
/// and the two blows (`0x19F`).
const ROSSO_STRUCK: &[Op] = &[
    Op::Wait(180),
    Op::Dialogue(0x19E),
    Op::Sound(BLOW_SOUND),
    Op::AwaitSoundEnd(BLOW_SOUND),
    Op::Sound(BLOW_SOUND),
    Op::AwaitSoundEnd(BLOW_SOUND),
    Op::Dialogue(0x19F),
];

const EAST_REGINA: &[Op] = &[
    glide16(REGINA, (0x150, 0x30), 1, false),
    glide16(REGINA, (0x150, 0x50), 1, false),
    glide16(REGINA, (0x120, 0x50), 1, false),
    Op::End,
];
const EAST_JACK: &[Op] = &[
    glide16(JACK, (0x160, 0x40), 1, false),
    glide16(JACK, (0x160, 0x60), 1, false),
    glide16(JACK, (0x130, 0x60), 1, false),
    Op::End,
];
const EAST_EARTH: &[Op] = &[
    glide16(EARTH, (0x160, 0x30), 1, false),
    glide16(EARTH, (0x160, 0x50), 1, false),
    glide16(EARTH, (0x130, 0x50), 1, false),
    Op::End,
];
const EAST_MAN_2: &[Op] = &[glide16(MAN_2, (0xD0, 0x70), 2, false), Op::End];
const EAST_VIOLA: &[Op] = &[
    glide16(VIOLA, (0x110, 0x70), 2, false),
    glide16(VIOLA, (0x180, 0x70), 2, false),
    Op::End,
];
const EAST_RUDOLPH: &[Op] = &[
    glide16(RUDOLPH, (0x100, 0x70), 2, false),
    glide16(RUDOLPH, (0x180, 0x70), 2, false),
    Op::End,
];
const EAST_PLAYER_OFF: &[Op] = &[glide16(PLAYER, (0x180, 0x60), 2, false), Op::End];
const EAST_REGINA_OFF: &[Op] = &[glide16(REGINA, (0x180, 0x50), 2, false), Op::End];
const EAST_JACK_OFF: &[Op] = &[glide16(JACK, (0x180, 0x60), 2, false), Op::End];
const EAST_EARTH_OFF: &[Op] = &[glide16(EARTH, (0x180, 0x50), 2, false), Op::End];

/// The same from the east wing (task at `0x0801CA80`), mirrored; this one
/// keeps the map's song.
const EAST_WING: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: CASTLE_HALL,
        player: (0x1B, 4),
        objects: EAST_WING_OBJECTS,
        count: 4,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, EAST_REGINA),
    Op::Spawn(MAP_TASK + 2, EAST_JACK),
    Op::Spawn(MAP_TASK + 3, EAST_EARTH),
    glide16(PLAYER, (0x150, 0x40), 1, false),
    glide16(PLAYER, (0x150, 0x60), 1, false),
    glide16(PLAYER, (0x120, 0x60), 1, false),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: CASTLE_HALL,
        player: (0x10, 6),
        objects: EAST_THRONE_OBJECTS,
        count: 9,
    },
    Op::Place(PLAYER, (0x12, 6)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Call(ROSSO_STRUCK),
    Op::Place(MAN_1, (8, 6)),
    Op::Place(MAN_2, (8, 7)),
    Op::Spawn(MAP_TASK + 1, EAST_MAN_2),
    glide16(MAN_1, (0xD0, 0x60), 2, false),
    Op::Dialogue(0x1A0),
    Op::Spawn(MAP_TASK + 1, EAST_VIOLA),
    Op::Spawn(MAP_TASK + 2, EAST_RUDOLPH),
    glide16(ROSSO, (0xF0, 0x70), 2, false),
    glide16(ROSSO, (0x180, 0x70), 2, false),
    Op::Face(PLAYER, Direction::Right),
    Op::Face(REGINA, Direction::Right),
    Op::Face(JACK, Direction::Right),
    Op::Face(EARTH, Direction::Right),
    Op::Dialogue(0x1A1),
    Op::Spawn(MAP_TASK + 1, EAST_PLAYER_OFF),
    Op::Spawn(MAP_TASK + 2, EAST_REGINA_OFF),
    Op::Spawn(MAP_TASK + 3, EAST_JACK_OFF),
    Op::Spawn(MAP_TASK + 4, EAST_EARTH_OFF),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::Stop(MAP_TASK + 2),
    Op::Stop(MAP_TASK + 3),
    Op::Stop(MAP_TASK + 4),
    Op::LoadMap {
        map: PLAINS,
        player: (8, 0x13),
        objects: AFTER_CASTLE_OBJECTS_EAST,
        count: 1,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x1A2),
    Op::Call(&back_to(PLAINS, (8, 0x13))),
];

/// Where the hall's scene starts (the hook at `0x0801B6B4`): the wing's
/// doorway, column 2 or 27 on rows 3 to 5.
const CASTLE_WATCH: &[Op] = &[Op::Loop(&[
    Op::IfPlayerSprite {
        x: (0x28, 0x28),
        y: (0x30, 0x50),
        then: &[
            Op::Control(false),
            Op::Spawn(MAP_TASK, WEST_WING),
            Op::Flag(CASTLE_SEEN, true),
            Op::End,
        ],
        otherwise: &[Op::IfPlayerSprite {
            x: (0x1B8, 0x1B8),
            y: (0x30, 0x50),
            then: &[
                Op::Control(false),
                Op::Spawn(MAP_TASK, EAST_WING),
                Op::Flag(CASTLE_SEEN, true),
                Op::End,
            ],
            otherwise: &[],
        }],
    },
    Op::Wait(1),
])];

/// The castle's hall (`0x0801B680`): once Gale is sighted, until Rosso's
/// band has struck.
const CASTLE_HALL_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[GALE_SIGHTED],
    none: &[CASTLE_SEEN],
    then: &[Op::Spawn(FIELD_WATCH, CASTLE_WATCH)],
    otherwise: &[],
}];

/// Hagen City's bar with Stinger (ROM `0x086696A0`): Regina, Jack and
/// Earth off the map, Stinger, the barkeeper and a patron.
const BAR_OBJECTS: u32 = 0x0866_96A0;
const STINGER: usize = 4;

const BAR_PLAYER_ENTERS: &[Op] = &[
    Op::Place(PLAYER, (2, 8)),
    glide16(PLAYER, (0x20, 0x50), 1, true),
    glide16(PLAYER, (0x80, 0x50), 1, true),
    glide16(PLAYER, (0x80, 0x30), 1, true),
    Op::Face(PLAYER, Direction::Right),
    Op::End,
];
const BAR_REGINA_ENTERS: &[Op] = &[
    Op::Place(REGINA, (2, 8)),
    glide16(REGINA, (0x20, 0x50), 1, true),
    glide16(REGINA, (0x70, 0x50), 1, true),
    glide16(REGINA, (0x70, 0x30), 1, true),
    Op::Face(REGINA, Direction::Right),
    Op::End,
];
const BAR_JACK_ENTERS: &[Op] = &[
    Op::Place(JACK, (2, 8)),
    glide16(JACK, (0x20, 0x50), 1, true),
    glide16(JACK, (0x80, 0x50), 1, true),
    glide16(JACK, (0x80, 0x40), 1, true),
    Op::Face(JACK, Direction::Right),
    Op::End,
];
/// The party comes into the bar one by one (helpers `0x0801D0D8`,
/// `0x0801D128`, `0x0801D178`, Earth last) and meets Stinger's look.
const BAR_ENTRANCE: &[Op] = &[
    Op::Spawn(MAP_TASK + 1, BAR_PLAYER_ENTERS),
    Op::Wait(16),
    Op::Spawn(MAP_TASK + 2, BAR_REGINA_ENTERS),
    Op::Wait(16),
    Op::Spawn(MAP_TASK + 3, BAR_JACK_ENTERS),
    Op::Wait(16),
    Op::Place(EARTH, (2, 8)),
    glide16(EARTH, (0x20, 0x50), 1, true),
    glide16(EARTH, (0x70, 0x50), 1, true),
    glide16(EARTH, (0x70, 0x40), 1, true),
    Op::Face(EARTH, Direction::Right),
    Op::Wait(15),
    Op::Face(STINGER, Direction::Left),
    Op::Wait(15),
];

const BAR_EARTH_LEAVES: &[Op] = &[
    glide16(EARTH, (0x70, 0x50), 2, true),
    glide16(EARTH, (0x20, 0x50), 2, true),
    glide16(EARTH, (0x20, 0x80), 2, true),
    Op::Place(EARTH, OFF_THE_MAP),
    Op::End,
];
const BAR_JACK_LEAVES: &[Op] = &[
    glide16(JACK, (0x80, 0x50), 2, false),
    glide16(JACK, (0x20, 0x50), 2, false),
    glide16(JACK, (0x20, 0x80), 2, false),
    Op::Place(JACK, OFF_THE_MAP),
    Op::End,
];
const BAR_REGINA_LEAVES: &[Op] = &[
    glide16(REGINA, (0x70, 0x50), 2, true),
    glide16(REGINA, (0x20, 0x50), 2, true),
    glide16(REGINA, (0x20, 0x80), 2, true),
    Op::Place(REGINA, OFF_THE_MAP),
    Op::End,
];

/// Stinger in the bar (task at `0x0801CE90`): she sizes the party up
/// (`0x1AB`), the party leaves, and she means to meet again (`0x1AC`).
const STINGER_IN_THE_BAR: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Call(BAR_ENTRANCE),
    Op::Dialogue(0x1AB),
    Op::Wait(15),
    Op::Face(STINGER, Direction::Down),
    Op::Wait(15),
    Op::Spawn(MAP_TASK + 1, BAR_EARTH_LEAVES),
    Op::Spawn(MAP_TASK + 3, BAR_JACK_LEAVES),
    Op::Wait(8),
    Op::Spawn(MAP_TASK + 2, BAR_REGINA_LEAVES),
    glide16(PLAYER, (0x80, 0x50), 2, false),
    glide16(PLAYER, (0x20, 0x50), 2, false),
    glide16(PLAYER, (0x20, 0x80), 2, false),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Dialogue(0x1AC),
    Op::Call(&back_to(HAGEN_CITY, (0x15, 9))),
];

/// Stinger caught in the bar (task at `0x0801D1C8`): she orders her usual
/// (`0x1AD`), the party comes in and she flees (`0x1AE`); the party talks
/// it over (`0x1AF`).
const STINGER_CAUGHT_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Wait(SETTLE),
    Op::Dialogue(0x1AD),
    Op::Call(BAR_ENTRANCE),
    Op::Dialogue(0x1AE),
    glide16(STINGER, (0x90, 0x50), 2, false),
    glide16(STINGER, (0x20, 0x50), 2, false),
    glide16(STINGER, (0x20, 0x80), 2, false),
    Op::Place(STINGER, OFF_THE_MAP),
    Op::Wait(15),
    Op::Face(PLAYER, Direction::Down),
    Op::Face(REGINA, Direction::Down),
    Op::Face(JACK, Direction::Down),
    Op::Face(EARTH, Direction::Down),
    Op::Wait(15),
    Op::Dialogue(0x1AF),
    Op::Call(&back_to(HAGEN_BAR, (8, 3))),
];

/// The bar with its scene's list and the player off the map.
macro_rules! bar_scene {
    ($task:expr, $flag:expr) => {
        &[
            Op::LoadMap {
                map: HAGEN_BAR,
                player: (2, 8),
                objects: BAR_OBJECTS,
                count: 7,
            },
            Op::Place(PLAYER, OFF_THE_MAP),
            Op::Control(false),
            Op::Spawn(MAP_TASK, $task),
            Op::Flag($flag, true),
        ]
    };
}

/// Hagen City's bar (`0x0801B7A8`): once the castle is seen Stinger is
/// there, and once she is beaten the party catches her there again.
const BAR_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CASTLE_SEEN],
    none: &[STINGER_MET],
    then: &[
        Op::Place(1, (9, 3)),
        Op::Call(bar_scene!(STINGER_IN_THE_BAR, STINGER_MET)),
    ],
    otherwise: &[Op::IfFlags {
        all: &[STINGER_BEATEN],
        none: &[STINGER_CAUGHT],
        then: &[
            Op::Place(1, (9, 3)),
            Op::Call(bar_scene!(STINGER_CAUGHT_SCENE, STINGER_CAUGHT)),
        ],
        otherwise: &[],
    }],
}];

/// The ruins' copy with the party (ROM `0x086697E0`), then with Rosso and
/// Viola hurt (ROM `0x08669C84`), and further in (ROM `0x08669CFC`).
const RUINS_OBJECTS: u32 = 0x0866_97E0;
const ROSSO_OBJECTS: u32 = 0x0866_9C84;
const RUINS_DEPTH_OBJECTS: u32 = 0x0866_9CFC;
const HURT_ROSSO: usize = 4;
const HURT_VIOLA: usize = 5;

const RUINS_REGINA: &[Op] = &[glide16(REGINA, (0xB0, 0x110), 1, false), Op::End];
const RUINS_JACK: &[Op] = &[glide16(JACK, (0xC0, 0x120), 1, false), Op::End];
const RUINS_EARTH: &[Op] = &[glide16(EARTH, (0xB0, 0x120), 1, false), Op::End];
const VIOLA_FOLLOWS: &[Op] = &[
    glide16(HURT_VIOLA, (0xD0, 0xF0), 1, false),
    glide16(HURT_VIOLA, (0xD0, 0x100), 1, false),
    Op::End,
];
const VIOLA_LEAVES: &[Op] = &[
    glide16(HURT_VIOLA, (0xD0, 0x180), 1, false),
    Op::Place(HURT_VIOLA, OFF_THE_MAP),
    Op::End,
];

/// Rosso's band in the ruins (task at `0x0801D5E4`): to the enemy's song
/// the party finds people (`0x1B5`) and Rosso and Viola hurt (`0x1B6`);
/// Earth tends Rosso, who tells what attacked them and asks to come along
/// (`0x1B7`): taken, they join (`0x1B8`, lists 22 and 23); refused, they
/// leave with a tip (`0x1B9`, `0x1BA`). Further in, the party is too late
/// (`0x1BB`).
const ROSSO_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Face(PLAYER, Direction::Up),
    Op::Wait(15),
    Op::RestartMusic(ENEMY_MUSIC),
    Op::Dialogue(0x1B5),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: RUINS_SCENE,
        player: (0xC, 0x12),
        objects: ROSSO_OBJECTS,
        count: 6,
    },
    Op::RestartMusic(ENEMY_MUSIC),
    Op::Place(PLAYER, (0xC, 0x18)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Face(HURT_ROSSO, Direction::Down),
    Op::Face(HURT_VIOLA, Direction::Down),
    Op::Wait(15),
    Op::Spawn(MAP_TASK + 1, RUINS_REGINA),
    Op::Spawn(MAP_TASK + 2, RUINS_JACK),
    Op::Spawn(MAP_TASK + 3, RUINS_EARTH),
    glide16(PLAYER, (0xC0, 0x110), 1, false),
    Op::Dialogue(0x1B6),
    glide16(EARTH, (0xA0, 0x120), 1, false),
    glide16(EARTH, (0xA0, 0xF0), 1, false),
    glide16(EARTH, (0xB0, 0xF0), 1, false),
    Op::Face(HURT_ROSSO, Direction::Left),
    Op::Wait(60),
    glide16(EARTH, (0xA0, 0xF0), 1, false),
    glide16(EARTH, (0xA0, 0x120), 1, false),
    glide16(EARTH, (0xB0, 0x120), 1, false),
    Op::Face(EARTH, Direction::Up),
    Op::Face(HURT_ROSSO, Direction::Down),
    Op::Dialogue(0x1B7),
    Op::IfChoice {
        then: &[
            Op::Dialogue(0x1B8),
            Op::Join(ROSSO_LISTS[0]),
            Op::Join(ROSSO_LISTS[1]),
            Op::Flag(ROSSO_JOINED, true),
        ],
        otherwise: &[
            Op::Dialogue(0x1B9),
            glide16(HURT_ROSSO, (0xD0, 0x100), 1, false),
            Op::Spawn(MAP_TASK + 1, VIOLA_FOLLOWS),
            glide16(HURT_ROSSO, (0xD0, 0x110), 1, false),
            Op::Face(PLAYER, Direction::Down),
            Op::Face(REGINA, Direction::Down),
            Op::Face(JACK, Direction::Down),
            Op::Face(EARTH, Direction::Down),
            Op::Wait(15),
            Op::Dialogue(0x1BA),
            Op::Spawn(MAP_TASK + 1, VIOLA_LEAVES),
            glide16(HURT_ROSSO, (0xD0, 0x180), 1, false),
            Op::Place(HURT_ROSSO, OFF_THE_MAP),
        ],
    },
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: RUINS_SCENE,
        player: (0xA, 5),
        objects: RUINS_DEPTH_OBJECTS,
        count: 4,
    },
    Op::Place(PLAYER, (0xB, 5)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x1BB),
    Op::Call(&back_to(PLAINS, (0x1B, 3))),
];

/// The ruins (`0x0801BA40`): once the wrecks are seen, the first visit
/// finds Rosso's band.
const RUINS_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[WRECKS_SEEN],
    none: &[ROSSO_MET],
    then: &[
        Op::LoadMap {
            map: RUINS_SCENE,
            player: (0xF, 0x18),
            objects: RUINS_OBJECTS,
            count: 1,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, ROSSO_SCENE),
        Op::Flag(ROSSO_MET, true),
    ],
    otherwise: &[],
}];

/// Gray Colony with the party (ROM `0x086697F4`).
const COLONY_OBJECTS: u32 = 0x0866_97F4;
/// Gray Colony (`0x0801BAB8`): after Raven's attack, the first visit
/// talks of the mountains' lab (task at `0x0801DF24`, `0x1C8`).
const COLONY_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[RAVEN_MET],
    none: &[LAB_POINTED],
    then: &[
        Op::LoadMap {
            map: GRAY_COLONY,
            player: (0xF, 0x11),
            objects: COLONY_OBJECTS,
            count: 1,
        },
        Op::Control(false),
        Op::Spawn(
            MAP_TASK,
            &[
                Op::Wait(SETTLE),
                Op::Dialogue(0x1C8),
                Op::Call(&back_to(GRAY_COLONY, (0xF, 0x11))),
            ],
        ),
        Op::Flag(LAB_POINTED, true),
    ],
    otherwise: &[],
}];

/// The lab's door with the party (ROM `0x08669808`).
const LAB_DOOR_OBJECTS: u32 = 0x0866_9808;
/// The lab found (task at `0x0801DF6C`): the view climbs the mountain
/// while the party guesses this is the place (`0x1CE`).
const LAB_FOUND_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Spawn(MAP_TASK + 1, &[Op::Repeat(144, PAN_UP), Op::End]),
    Op::Dialogue(0x1CE),
    Op::Stop(MAP_TASK + 1),
    Op::Call(&back_to(LAB_DOOR, (0xE, 0x11))),
];
/// The lab's door before Gray Colony has spoken (task at `0x0801DFF8`):
/// a military site, and the party backs off (`0x1CD`).
const LAB_TURNED_AWAY: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Dialogue(0x1CD),
    Op::Call(&back_to(PLAINS, (0x11, 0x15))),
];
/// The lab's door (`0x0801BB40`).
const LAB_DOOR_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[LAB_POINTED],
    none: &[LAB_FOUND],
    then: &[
        Op::LoadMap {
            map: LAB_DOOR,
            player: (0xE, 0x11),
            objects: LAB_DOOR_OBJECTS,
            count: 1,
        },
        Op::RestartMusic(DANGER_MUSIC),
        Op::Control(false),
        Op::Spawn(MAP_TASK, LAB_FOUND_SCENE),
        Op::Flag(LAB_FOUND, true),
    ],
    otherwise: &[Op::IfFlags {
        all: &[],
        none: &[LAB_POINTED],
        then: &[
            Op::Control(false),
            Op::Face(PLAYER, Direction::Up),
            Op::Spawn(MAP_TASK, LAB_TURNED_AWAY),
        ],
        otherwise: &[],
    }],
}];

/// Caught by a guard (task at `0x0801BD9C`): once the guard and the player
/// have ended their steps, the player turns to the guard, the party is
/// found out (`0x1CF`) and put out at the lab's door.
const CAUGHT: &[Op] = &[
    Op::AwaitArrival(SEEN_GUARD),
    Op::AwaitArrival(PLAYER),
    Op::FaceSeenGuard,
    Op::Dialogue(0x1CF),
    Op::Call(SCENE_DARKEN),
    Op::Face(PLAYER, Direction::Up),
    Op::Call(&warp_to(LAB_DOOR, (0xE, 4))),
];

/// A guard seeing the player takes the controls and starts [`CAUGHT`].
const CAUGHT_START: &[Op] = &[Op::Control(false), Op::Spawn(MAP_TASK, CAUGHT), Op::End];

/// The guards of the passage and of the rooms (the lists at ROM
/// `0x0866981C` and `0x08669880`).
const PASSAGE_GUARDS: &[usize] = &[4, 5];
const ROOMS_GUARDS: &[usize] = &[1, 2, 3, 4, 5];

/// The passage's watch (`0x0801BC1C`): every frame, whether a guard sees
/// the player.
const PASSAGE_WATCH: &[Op] = &[Op::Loop(&[
    Op::IfSeen {
        guards: PASSAGE_GUARDS,
        then: CAUGHT_START,
        otherwise: &[],
    },
    Op::Wait(1),
])];
const ROOMS_WATCH: &[Op] = &[Op::Loop(&[
    Op::IfSeen {
        guards: ROOMS_GUARDS,
        then: CAUGHT_START,
        otherwise: &[],
    },
    Op::Wait(1),
])];

/// The wrecked room (ROM `0x08669E14`): Regina, Jack, Earth and a
/// scientist off the map.
const WRECKED_ROOM_OBJECTS: u32 = 0x0866_9E14;
const SCIENTIST: usize = 4;
const SCIENTIST_BACKS_OFF: &[Op] = &[
    Op::Repeat(16, &[Op::Nudge(SCIENTIST, (0, 1)), Op::Wait(1)]),
    Op::End,
];

/// The wrecked room (task at `0x0801E20C`): something has run wild here
/// (`0x1D0`); a scientist comes out and is calmed (`0x1D1`), and tells of
/// the Death Saurer, which the party blames on Gale (`0x1D2`); then the
/// party is back on the plains.
const WRECKED_ROOM: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: LAB_ROOMS,
        player: (0xF, 9),
        objects: WRECKED_ROOM_OBJECTS,
        count: 5,
    },
    Op::Place(PLAYER, (0xF, 6)),
    Op::Call(SCENE_BRIGHTEN),
    glide16(PLAYER, (0xF0, 0x70), 1, false),
    Op::Place(REGINA, (0xF, 7)),
    glide16(REGINA, (0xE0, 0x70), 1, true),
    Op::Face(REGINA, Direction::Down),
    Op::Place(JACK, (0xF, 7)),
    glide16(JACK, (0x100, 0x60), 1, true),
    Op::Face(JACK, Direction::Down),
    Op::Place(EARTH, (0xF, 7)),
    glide16(EARTH, (0x100, 0x70), 1, true),
    Op::Face(EARTH, Direction::Down),
    Op::Dialogue(0x1D0),
    glide16(REGINA, (0xD0, 0x70), 1, true),
    glide16(REGINA, (0xD0, 0x60), 1, true),
    Op::Face(REGINA, Direction::Right),
    glide16(JACK, (0x110, 0x60), 1, true),
    Op::Face(JACK, Direction::Left),
    glide16(EARTH, (0x110, 0x70), 1, true),
    Op::Face(EARTH, Direction::Left),
    glide16(PLAYER, (0xD0, 0x70), 1, false),
    Op::Face(PLAYER, Direction::Right),
    Op::Place(SCIENTIST, (0xF, 3)),
    glide16(SCIENTIST, (0xF0, 0x70), 1, true),
    glide16(JACK, (0xF0, 0x60), 1, true),
    Op::Face(JACK, Direction::Down),
    Op::Wait(15),
    Op::Face(SCIENTIST, Direction::Up),
    Op::Spawn(MAP_TASK + 1, SCIENTIST_BACKS_OFF),
    glide16(JACK, (0xF0, 0x70), 1, true),
    glide16(EARTH, (0x100, 0x70), 1, true),
    Op::Face(EARTH, Direction::Down),
    glide16(PLAYER, (0xE0, 0x70), 1, false),
    Op::Face(PLAYER, Direction::Down),
    Op::Face(REGINA, Direction::Down),
    Op::Dialogue(0x1D1),
    Op::Repeat(16, &[Op::Nudge(JACK, (0, -1)), Op::Wait(1)]),
    glide16(REGINA, (0xE0, 0x60), 1, true),
    Op::Face(REGINA, Direction::Right),
    Op::Face(PLAYER, Direction::Right),
    Op::Face(EARTH, Direction::Left),
    Op::Wait(15),
    Op::Dialogue(0x1D2),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, &warp_to(PLAINS, (0x11, 0x15))),
    Op::End,
];

/// The rooms' watch (`0x0801BED8`): the wrecked room's doorway, cell
/// (15, 6), starts its scene, and the guards look every frame.
const ROOMS_ENTRY_WATCH: &[Op] = &[Op::Loop(&[
    Op::IfPlayerSprite {
        x: (0xE8, 0xE8),
        y: (0x60, 0x60),
        then: &[
            Op::Control(false),
            Op::Spawn(MAP_TASK, WRECKED_ROOM),
            Op::Flag(LAB_ENTERED, true),
            Op::End,
        ],
        otherwise: &[],
    },
    Op::IfSeen {
        guards: ROOMS_GUARDS,
        then: CAUGHT_START,
        otherwise: &[],
    },
    Op::Wait(1),
])];

/// The lab's rooms (`0x0801BE94`): until the wrecked room is seen its
/// doorway waits; the guards always look.
const LAB_ROOMS_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[LAB_FOUND],
    none: &[LAB_ENTERED],
    then: &[Op::Spawn(FIELD_WATCH, ROOMS_ENTRY_WATCH)],
    otherwise: &[Op::Spawn(FIELD_WATCH, ROOMS_WATCH)],
}];
/// The lab's passage (`0x0801BC04`).
const LAB_PASSAGE_ARRIVAL: &[Op] = &[Op::Spawn(FIELD_WATCH, PASSAGE_WATCH)];

/// The institute's gate (ROM `0x0866972C`): Regina, Jack and Earth off the
/// map.
const INSTITUTE_OBJECTS: u32 = 0x0866_972C;
/// Its gate's soldier, gone once the party has been in.
const GATE_SOLDIER: usize = 2;

/// Into the institute (task at `0x0801E48C`): the party comes to the gate,
/// unguarded (`0x1DA`), and walks in; the hook at `0x0801E5B8` brings it
/// inside.
const INSTITUTE_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Place(EARTH, (0xC, 0x12)),
    glide16(EARTH, (0xC0, 0xD0), 1, true),
    Op::Place(PLAYER, (0xC, 0x12)),
    glide16(PLAYER, (0xC0, 0xE0), 1, false),
    glide16(PLAYER, (0xB0, 0xE0), 1, false),
    Op::Face(PLAYER, Direction::Up),
    Op::Place(REGINA, (0xC, 0x12)),
    glide16(REGINA, (0xC0, 0xE0), 1, true),
    glide16(REGINA, (0xD0, 0xE0), 1, true),
    Op::Face(REGINA, Direction::Up),
    Op::Place(JACK, (0xC, 0x12)),
    glide16(JACK, (0xC0, 0xE0), 1, true),
    Op::Face(EARTH, Direction::Down),
    Op::Dialogue(0x1DA),
    glide16(EARTH, (0xB0, 0xE0), 1, true),
    Op::Place(EARTH, OFF_THE_MAP),
    glide16(JACK, (0xB0, 0xE0), 1, true),
    Op::Place(JACK, OFF_THE_MAP),
    glide16(REGINA, (0xB0, 0xE0), 1, true),
    Op::Place(REGINA, OFF_THE_MAP),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, &warp_to(INSTITUTE_GATE, (0xB, 0xE))),
    Op::End,
];

/// The institute's gate (`0x0801B8C4`): after Raven has chased Gale, the
/// first visit walks in; from then on its soldier is gone.
const INSTITUTE_GATE_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[GENOSAURER_SEEN],
    none: &[INSTITUTE_ENTERED],
    then: &[
        Op::LoadMap {
            map: INSTITUTE_GATE,
            player: (0xC, 0x12),
            objects: INSTITUTE_OBJECTS,
            count: 4,
        },
        Op::Place(PLAYER, OFF_THE_MAP),
        Op::Control(false),
        Op::Spawn(MAP_TASK, INSTITUTE_SCENE),
        Op::Flag(INSTITUTE_ENTERED, true),
    ],
    otherwise: &[Op::IfFlags {
        all: &[GENOSAURER_SEEN],
        none: &[],
        then: &[Op::Place(GATE_SOLDIER, OFF_THE_MAP)],
        otherwise: &[],
    }],
}];

/// The institute's hall with Prozen (ROM `0x0866977C`): Regina, Jack and
/// Earth off the map.
const PROZEN_OBJECTS: u32 = 0x0866_977C;
const PROZEN: usize = 4;

/// The prototype's fight (story battle 27).
const PROTOTYPE_BATTLE_OPS: [Op; 4] = story_battle(27);
/// The prototype's battle (the hook at `0x0801E7BC`): won, it is gone and
/// Gale comes (task at `0x0801E838`).
const PROTOTYPE_FIGHT: &[Op] = &[
    Op::Call(&PROTOTYPE_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::Place(PROTOTYPE, OFF_THE_MAP),
            Op::Flag(PROTOTYPE_BEATEN, true),
            Op::Control(false),
            Op::Spawn(MAP_TASK, PROTOTYPE_FALLS),
        ],
    },
    Op::End,
];

const PLAYER_RUNS_OUT: &[Op] = &[glide16(PLAYER, (0x80, 0x140), 2, false), Op::End];
const REGINA_RUNS_OUT: &[Op] = &[
    glide16(REGINA, (0x80, 0xB0), 2, false),
    glide16(REGINA, (0x80, 0x140), 2, false),
    Op::End,
];
const JACK_RUNS_OUT: &[Op] = &[
    glide16(JACK, (0x80, 0xB0), 2, false),
    glide16(JACK, (0x80, 0x140), 2, false),
    Op::End,
];
const EARTH_RUNS_OUT: &[Op] = &[
    glide16(EARTH, (0x80, 0xB0), 2, false),
    glide16(EARTH, (0x80, 0x140), 2, false),
    Op::End,
];

/// Prozen (task at `0x0801E5E8`): the party comes in; Prozen shows the
/// Death Saurer's prototype (`0x1DB`, `0x1DC`) and leaves for the
/// coronation (`0x1DD`); Jack answers him (`0x1DE`) and the party runs
/// out to the plains, where the prototype stands; the hook at
/// `0x0801E7BC` starts its fight.
const PROZEN_SCENE: &[Op] = &[
    Op::Place(REGINA, (8, 0xB)),
    glide16(REGINA, (0x70, 0xB0), 1, true),
    Op::Face(REGINA, Direction::Up),
    Op::Place(EARTH, (8, 0xB)),
    glide16(EARTH, (0xA0, 0xB0), 1, true),
    Op::Face(EARTH, Direction::Up),
    Op::Place(JACK, (8, 0xB)),
    glide16(JACK, (0x90, 0xB0), 1, true),
    Op::Face(JACK, Direction::Up),
    Op::Dialogue(0x1DB),
    Op::Dialogue(0x1DC),
    Op::Dialogue(0x1DD),
    glide16(PROZEN, (0x60, 0x80), 2, true),
    glide16(PROZEN, (0x60, 0x40), 2, true),
    Op::Place(PROZEN, OFF_THE_MAP),
    glide16(EARTH, (0xA0, 0xA0), 1, true),
    glide16(EARTH, (0x80, 0xA0), 1, false),
    Op::Face(EARTH, Direction::Up),
    Op::Dialogue(0x1DE),
    Op::Spawn(MAP_TASK + 1, PLAYER_RUNS_OUT),
    Op::Wait(8),
    Op::Spawn(MAP_TASK + 2, REGINA_RUNS_OUT),
    Op::Wait(8),
    Op::Spawn(MAP_TASK + 3, JACK_RUNS_OUT),
    Op::Wait(8),
    Op::Spawn(MAP_TASK + 4, EARTH_RUNS_OUT),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::Stop(MAP_TASK + 2),
    Op::Stop(MAP_TASK + 3),
    Op::Stop(MAP_TASK + 4),
    Op::LoadMap {
        map: PLAINS,
        player: (9, 0xC),
        objects: PROTOTYPE_OBJECTS,
        count: 6,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Wait(15),
    glide(PLAYER, (0x120, 0x1C0), 1, false),
    Op::Face(PLAYER, Direction::Up),
    Op::Wait(15),
    Op::Place(PROTOTYPE, (9, 0xC)),
    Op::Wait(15),
    Op::Repeat(64, PAN_DOWN),
    Op::Wait(15),
    Op::Spawn(FIELD_HOOK, PROTOTYPE_FIGHT),
    Op::End,
];

/// Where Prozen waits (the hook at `0x0801B9B8`): cell (8, 11).
const PROZEN_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x78, 0x78),
        y: (0xB0, 0xB0),
    },
    Op::Control(false),
    Op::Spawn(MAP_TASK, PROZEN_SCENE),
    Op::End,
];

/// The institute's hall (`0x0801B960`): until the prototype is beaten it
/// loads with Prozen where the party stands, and his spot waits.
const INSTITUTE_HALL_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[INSTITUTE_ENTERED],
    none: &[PROTOTYPE_BEATEN],
    then: &[
        Op::LoadMap {
            map: INSTITUTE_HALL,
            player: HERE,
            objects: PROZEN_OBJECTS,
            count: 5,
        },
        Op::Spawn(FIELD_WATCH, PROZEN_WATCH),
    ],
    otherwise: &[],
}];

/// Hagen City's people (`0x08006BAC` on): until Rosso's band has struck at
/// the castle they talk of the coronation (`0x1A3` on), after of the boy
/// carried off (`0x36C` on).
macro_rules! townsperson {
    ($before:expr, $after:expr) => {
        &[Op::IfFlags {
            all: &[],
            none: &[CASTLE_SEEN],
            then: &[Op::Dialogue($before)],
            otherwise: &[Op::Dialogue($after)],
        }]
    };
}
const TOWNSPEOPLE: [(u32, &[Op]); 8] = [
    (0x0800_6BAC, townsperson!(0x1A3, 0x36C)),
    (0x0800_6BD0, townsperson!(0x1A4, 0x36D)),
    (0x0800_6BF4, townsperson!(0x1A5, 0x36E)),
    (0x0800_6C1C, townsperson!(0x1A6, 0x36F)),
    (0x0800_6C40, townsperson!(0x1A7, 0x370)),
    (0x0800_6C64, townsperson!(0x1A8, 0x371)),
    (0x0800_6C88, townsperson!(0x1A9, 0x372)),
    (0x0800_6CB0, townsperson!(0x1AA, 0x373)),
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

/// The teachers of area 6: deck commands 23 and 8 (`0x08006B98`,
/// `0x08006CD4`), and 3 (`0x080094A4` through `0x08009430`).
const TEACHERS: [(u32, &[Op]); 3] = [
    (0x0800_6B98, teacher!(0x17, 0x3E0, 0x3F4)),
    (0x0800_6CD4, teacher!(8, 0x3E1, 0x3F5)),
    (0x0800_94A4, teacher!(3, 0x2F7, 0x359)),
];

/// The keepers of area 6 (`0x080092AC` on): item shops 9 to 12,
/// armaments shops 17 to 19, and the labs 11 and 20.
const SHOPS: [(u32, &[Op]); 9] = [
    (0x0800_92AC, &shop(Shop::Items(9))),
    (0x0800_92B8, &shop(Shop::Items(0xA))),
    (0x0800_92C4, &shop(Shop::Items(0xB))),
    (0x0800_92D0, &shop(Shop::Items(0xC))),
    (0x0800_92DC, &shop(Shop::Arms(0x11))),
    (0x0800_92E8, &shop(Shop::Arms(0x12))),
    (0x0800_92F4, &shop(Shop::Arms(0x13))),
    (0x0800_9300, &shop(Shop::Lab(0xB))),
    (0x0800_930C, &shop(Shop::Lab(0x14))),
];

/// What speaking to an object of area 6 whose script is code runs.
pub(super) fn talk_handler(address: u32) -> Option<&'static [Op]> {
    TEACHERS
        .iter()
        .chain(SHOPS.iter())
        .chain(TOWNSPEOPLE.iter())
        .find(|(at, _)| *at == address)
        .map(|(_, program)| *program)
}

/// What map `map` of area 6 runs when it loads, when it runs anything.
pub(super) fn map_handler(map: usize) -> Option<&'static [Op]> {
    match map {
        THRONE_ROOM => Some(THRONE_ROOM_ARRIVAL),
        PLAINS => Some(PLAINS_ARRIVAL),
        MILETOS_TOWN => Some(TOWN_ARRIVAL),
        CASTLE_HALL => Some(CASTLE_HALL_ARRIVAL),
        HAGEN_BAR => Some(BAR_ARRIVAL),
        INSTITUTE_GATE => Some(INSTITUTE_GATE_ARRIVAL),
        INSTITUTE_HALL => Some(INSTITUTE_HALL_ARRIVAL),
        RUINS => Some(RUINS_ARRIVAL),
        GRAY_COLONY => Some(COLONY_ARRIVAL),
        LAB_DOOR => Some(LAB_DOOR_ARRIVAL),
        LAB_PASSAGE => Some(LAB_PASSAGE_ARRIVAL),
        LAB_ROOMS => Some(LAB_ROOMS_ARRIVAL),
        _ => None,
    }
}
