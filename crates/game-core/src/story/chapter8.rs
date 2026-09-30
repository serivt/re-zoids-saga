//! Chapter 8: the Emperor sends for the Death Stinger, which razes New
//! Helic City; the party meets Schwarz and the Ultrasaurus, holds off the
//! Death Stinger, gathers the Planetal Sites for Dr. D's Gravity Cannon,
//! sees Opis turn on Blood, defends the Ultrasaurus until the cannon
//! strikes, and faces the true Death Saurer at Eve Polis (area 8).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the map
//! handlers of area 8 (`0x08020F24` to `0x0802214C`), the tasks and field
//! hooks they install and the objects' code, named at each item. See
//! `docs/events.md`.

use super::{
    FIELD_HOOK, PLAYER, SCENE_BRIGHTEN, SCENE_DARKEN, glide, learn, shop, story_battle, through,
};
use crate::event::{FIELD_WATCH, HERE, MAP_TASK, Op, WarpRedirect};
use crate::field::{Direction, PIXEL};
use crate::menu::Shop;

/// Set once the Emperor has sent for the Death Stinger (`0x0802205C`).
const CHAPTER_OPENED: u16 = 0x1B2;
/// Set once the portal has brought the party near New Helic City.
const ARRIVED: u16 = 0x1B3;
/// Set once the party has first seen the city's ruins.
const RUINS_SEEN: u16 = 0x1B4;
/// Set once Blood has claimed the Death Stinger as his.
const BLOOD_WARNED: u16 = 0x1B5;
/// Set once Schwarz has joined.
const SCHWARZ_MET: u16 = 0x1B6;
/// Set once the Ultrasaurus has risen from the lake.
const ULTRASAURUS_SEEN: u16 = 0x1B7;
/// Set once the Death Stinger is held off (story battle 30).
const STINGER_REPELLED: u16 = 0x1B9;
/// Set once Dr. D has shown the Gravity Cannon, and on entering either
/// ruin's first room (the handlers test what `0x08000F88` leaves in `r0`,
/// the flag's own number).
const CANNON_SEEN: u16 = 0x1BA;
/// Set once Raven and Reese have joined.
const RAVEN_JOINED: u16 = 0x1BB;
/// The Planetal Sites found: three in the island's ruins, three in the
/// western ruins.
const SITES: [u16; 6] = [0x1BC, 0x1BD, 0x1BE, 0x1BF, 0x1C0, 0x1C1];
const ISLAND_SITES: [u16; 3] = [SITES[0], SITES[1], SITES[2]];
const WESTERN_SITES: [u16; 3] = [SITES[3], SITES[4], SITES[5]];
/// Set once Blood is beaten at Dr. D's dig (story battle 31).
const BLOOD_BEATEN: u16 = 0x1C2;
/// Set once the party is told to guard the Ultrasaurus.
const DEFENSE_BRIEFED: u16 = 0x1C3;
/// Set once the Gravity Cannon has struck the Death Stinger.
const STINGER_STRUCK: u16 = 0x1C4;
/// Set once Hermann sends the party to Eve Polis.
const EVE_POLIS_BOUND: u16 = 0x1C6;
/// Set once the true Death Saurer has fallen (story battle 34).
const SAURER_BEATEN: u16 = 0x1C7;
/// Set once the party has seen Opis in the city's ruins.
const OPIS_SEEN: u16 = 0x1C8;
/// Set once the party has come near Opis in the ruins.
const OPIS_NEAR: u16 = 0x1C9;
/// Set with the cannon's strike and cleared by Hermann; the chapter reads
/// it nowhere.
const STRIKE_NOTED: u16 = 0x199;

/// The world map (236); the plains around New Helic City (237), a Zoid
/// map with the portal; the city's ruins (238); Raven's hideout (242); the
/// Ultrasaurus's hold (243) and bridge (244); the ruins where Dr. D fits
/// his cannon (245, and 246 once Blood is beaten); the Planetal Sites'
/// rooms: the island's ruins (247, 250, 251) and the western ruins (252,
/// 253, 255); Eve Polis (258).
const WORLD: usize = 236;
const PLAINS: usize = 237;
const CITY: usize = 238;
const HIDEOUT: usize = 242;
const HOLD: usize = 243;
const BRIDGE: usize = 244;
const DIG: usize = 245;
const DIG_AFTER: usize = 246;
const ISLAND_RUINS: usize = 247;
const ISLAND_SECOND_ROOM: usize = 250;
const ISLAND_THIRD_ROOM: usize = 251;
const WESTERN_RUINS: usize = 252;
const WESTERN_SECOND_ROOM: usize = 253;
const WESTERN_THIRD_ROOM: usize = 255;
const EVE_POLIS: usize = 258;
/// The map Eve Polis's way leads to, which leads to Eve Polis itself
/// while the party is sent there.
const EVE_POLIS_WAY: usize = 257;
/// The Emperor's throne room (262), where chapter 7's end leaves the
/// party; the castle's portal room (270) and the room above the base's bar
/// (271) in the kingdom's time.
const THRONE_ROOM: usize = 262;
const PORTAL_ROOM: usize = 270;
const BASE_ROOM: usize = 271;
/// Chapter 9's first map, where the chapter's end leaves the party.
const CHAPTER_9_START: usize = 281;

/// The songs the chapter switches to.
const PEACE_MUSIC: u16 = 1;
const DANGER_MUSIC: u16 = 4;
const ENEMY_MUSIC: u16 = 9;
const FINAL_MUSIC: u16 = 0x13;
const RIVAL_MUSIC: u16 = 0x16;
/// The song the staged battle scenes play to (`0x08012040`).
const STAGED_MUSIC: u16 = 0x17;

const PORTAL_SOUND: u16 = 0x6F;
const PORTAL_ARRIVAL_SOUND: u16 = 0x49;
const BLAST_SOUND: u16 = 0x5B;
const BEAM_SOUND: u16 = 0x81;
const ALARM_SOUND: u16 = 0x80;
const SIREN_SOUND: u16 = 0x58;
const BLOCKADE_SOUND: u16 = 0x4B;
const GRAVITY_SOUND: u16 = 0x85;
const REVIVAL_SOUND: u16 = 0x66;
const PORTAL_IDLE: usize = 0;
const PORTAL_OPENS: usize = 2;
const PORTAL_BRINGS_STEP: usize = 32;

/// Frames a task waits after the fade in before its first line.
const SETTLE: u32 = 32;
const OFF_THE_MAP: (usize, usize) = (0xFF, 0xFF);
const HIDDEN: (usize, usize) = (0xFFFF, 0xFFFF);
/// The prince standing again (`0x080089A0`).
const PRINCE_STANDING: usize = 0x98;
/// The characters of group 7, met as the chapter opens (`0x08009A08`).
const CHAPTER_GROUP: u8 = 7;
/// The lists Schwarz; the two at Dr. D's dig; and Raven and Reese join
/// with.
const SCHWARZ_LIST: u8 = 0x1C;
const DR_D_LISTS: [u8; 2] = [0x1D, 0x1E];
const RAVEN_LISTS: [u8; 2] = [0x1F, 0x20];
/// The return points the chapter sets: the city's ruins, the
/// Ultrasaurus's hold, the plains and the world map.
const RUINS_RETURN_POINT: u8 = 0x12;
const HOLD_RETURN_POINT: u8 = 0x13;
const PLAINS_RETURN_POINT: u8 = 0x15;
const WORLD_RETURN_POINT: u8 = 0x16;
/// The dialogue that counts the Planetal Sites left in a ruin
/// (`0x08021D1C`).
const SITES_LEFT: u16 = 0x2D6;

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

/// The same over the player's cell moved by `by` cells.
const fn blast_near(actor: usize, by: (i32, i32)) -> [Op; 5] {
    [
        Op::PlaceNearPlayer(actor, by),
        Op::PlayOnce(actor, 0),
        Op::Sound(BLAST_SOUND),
        Op::AwaitAnimation(actor),
        Op::Place(actor, OFF_THE_MAP),
    ]
}

/// Staged battle scenes to their song (`0x08012040` with a scene, or with
/// an `0xFF`-ended list): the song playing is kept, theirs plays, the
/// scenes run and the song kept comes back.
macro_rules! staged {
    ($($scene:expr),+) => {
        &[
            Op::SaveMusic,
            Op::Music(STAGED_MUSIC),
            $(Op::Battle($scene),)+
            Op::RestoreMusic,
        ]
    };
}

/// A story battle's hook: beaten, the party is taken to its return point
/// and the field brightens at once.
const BEATEN: &[Op] = &[Op::WarpHome, Op::FadeInHolding];

/// Scrolls the view a pixel, or four, a frame (`0x08008324` in a loop).
const PAN_DOWN: &[Op] = &[Op::Pan(0, PIXEL), Op::Wait(1)];
const PAN_UP: &[Op] = &[Op::Pan(0, -PIXEL), Op::Wait(1)];
const PAN_LEFT: &[Op] = &[Op::Pan(-PIXEL, 0), Op::Wait(1)];
const PAN_DOWN_FAST: &[Op] = &[Op::Pan(0, 4 * PIXEL), Op::Wait(1)];
const PAN_UP_FAST: &[Op] = &[Op::Pan(0, -4 * PIXEL), Op::Wait(1)];
const PAN_LEFT_FAST: &[Op] = &[Op::Pan(-4 * PIXEL, 0), Op::Wait(1)];
const PAN_RIGHT_FAST: &[Op] = &[Op::Pan(4 * PIXEL, 0), Op::Wait(1)];
/// A pixel left every other frame.
const PAN_LEFT_SLOW: &[Op] = &[Op::Pan(-PIXEL, 0), Op::Wait(2)];

/// Actor `actor` flickers at `cell`, off and on every third frame four
/// times, and is left off.
const fn flicker(actor: usize, cell: (usize, usize)) -> [Op; 18] {
    let mut ops = [Op::Wait(3); 18];
    let mut step = 0;
    while step < 4 {
        ops[4 * step] = Op::Place(actor, OFF_THE_MAP);
        ops[4 * step + 2] = Op::Place(actor, cell);
        step += 1;
    }
    ops[16] = Op::Place(actor, OFF_THE_MAP);
    ops
}

/// The white flash's rise (`0x0801D93C`): `BLDCNT` brightens every layer
/// toward white, and `BLDY` goes up a level every other frame to 16.
const WHITEN: [Op; 32] = whiten_steps(true);
/// Its fall (`0x0801D968`): back a level every other frame to 0, then the
/// blend is turned off a frame later.
const UNWHITEN: [Op; 33] = unwhiten();

const fn whiten_steps(rise: bool) -> [Op; 32] {
    let mut ops = [Op::Wait(2); 32];
    let mut step: u8 = 0;
    while step < 16 {
        let level = if rise { step + 1 } else { 15 - step };
        ops[2 * step as usize] = Op::Whiten(level);
        step += 1;
    }
    ops
}

const fn unwhiten() -> [Op; 33] {
    let steps = whiten_steps(false);
    let mut ops = [Op::Wait(1); 33];
    let mut at = 0;
    while at < steps.len() {
        ops[at] = steps[at];
        at += 1;
    }
    ops
}

const BRIGHTENS: &[Op] = &[Op::Call(SCENE_BRIGHTEN), Op::End];

/// The throne room for the opening (ROM `0x0866A9A8`): the Emperor, Gale,
/// Opis, Blood, Fran and two guards.
const THRONE_OBJECTS: u32 = 0x0866_A9A8;
const GALE: usize = 2;
const OPIS: usize = 3;
const BLOOD: usize = 4;
const FRAN: usize = 5;
const FIRST_GUARD: usize = 6;
const SECOND_GUARD: usize = 7;
/// The portal room (ROM `0x0866AA48`): two soldiers.
const PORTAL_ROOM_OBJECTS: u32 = 0x0866_AA48;
/// The room above the base's bar (ROM `0x0866AA84`): the prince seated,
/// Regina, and Jack and Earth off the map.
const BASE_OBJECTS: u32 = 0x0866_AA84;
const REGINA: usize = 1;
const JACK: usize = 2;
const EARTH: usize = 3;

const FRAN_STEPS_UP: &[Op] = &[glide16(FRAN, (0x80, 0x40), 1, true), Op::End];
const OPIS_STEPS_ASIDE: &[Op] = &[
    glide16(OPIS, (0xA0, 0x30), 1, true),
    glide16(OPIS, (0xA0, 0x40), 1, true),
    Op::Face(OPIS, Direction::Left),
    Op::End,
];
const GALE_LEAVES: &[Op] = &[
    glide16(GALE, (0x80, 0xC0), 1, true),
    Op::Place(GALE, OFF_THE_MAP),
    Op::End,
];
const SECOND_GUARD_COMES: &[Op] = &[
    glide16(SECOND_GUARD, (0x80, 0x30), 1, true),
    Op::Face(SECOND_GUARD, Direction::Left),
    Op::End,
];
const FIRST_GUARD_LEAVES: &[Op] = &[
    glide16(FIRST_GUARD, (0x60, 0xC0), 1, true),
    Op::Place(FIRST_GUARD, OFF_THE_MAP),
    Op::End,
];
const SECOND_GUARD_LEAVES: &[Op] = &[
    glide16(SECOND_GUARD, (0x80, 0xC0), 1, true),
    Op::Place(SECOND_GUARD, OFF_THE_MAP),
    Op::End,
];
const JACK_COMES_IN: &[Op] = &[
    Op::Place(JACK, (0xE, 2)),
    glide16(JACK, (0x30, 0x20), 1, true),
    glide16(JACK, (0x30, 0x30), 1, true),
    Op::Face(JACK, Direction::Down),
    Op::End,
];

/// The chapter's opening (task at `0x08022158`): the Emperor dismisses
/// Blood (`0x21A`, `0x21B`) and turns on Fran (`0x21C`), whom the guards
/// take away; he asks Opis for power and Opis offers the Death
/// Stinger (`0x21D`, `0x21E`); the Emperor curses the prince (`0x21F`); in
/// the portal room the soldiers sense the device in use (`0x220`); in the
/// room above the bar Jack brings the news (`0x221`) and the party leaves.
const OPENING: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Spawn(MAP_TASK + 1, FRAN_STEPS_UP),
    glide16(BLOOD, (0x80, 0x30), 1, true),
    Op::Wait(16),
    Op::Spawn(MAP_TASK + 1, OPIS_STEPS_ASIDE),
    glide16(GALE, (0x60, 0x30), 1, true),
    glide16(GALE, (0x60, 0x40), 1, true),
    Op::Face(GALE, Direction::Right),
    Op::Wait(16),
    glide16(FRAN, (0x70, 0x40), 1, true),
    glide16(FRAN, (0x70, 0x30), 1, true),
    Op::Dialogue(0x21A),
    glide16(GALE, (0x80, 0x40), 1, true),
    Op::Face(GALE, Direction::Up),
    Op::Face(FRAN, Direction::Right),
    Op::Dialogue(0x21B),
    Op::Spawn(MAP_TASK + 1, GALE_LEAVES),
    glide16(BLOOD, (0x80, 0xC0), 1, true),
    Op::Place(BLOOD, OFF_THE_MAP),
    Op::Face(FRAN, Direction::Up),
    Op::Dialogue(0x21C),
    Op::Spawn(MAP_TASK + 1, SECOND_GUARD_COMES),
    glide16(FIRST_GUARD, (0x60, 0x30), 1, true),
    Op::Face(FIRST_GUARD, Direction::Right),
    Op::Wait(15),
    Op::Face(FRAN, Direction::Down),
    Op::Wait(15),
    Op::Spawn(MAP_TASK + 1, FIRST_GUARD_LEAVES),
    Op::Spawn(MAP_TASK + 2, SECOND_GUARD_LEAVES),
    glide16(FRAN, (0x70, 0xC0), 1, true),
    Op::Place(FRAN, OFF_THE_MAP),
    glide16(OPIS, (0x80, 0x30), 1, true),
    Op::Face(OPIS, Direction::Up),
    Op::Dialogue(0x21D),
    Op::Dialogue(0x21E),
    glide16(OPIS, (0x80, 0xC0), 1, true),
    Op::Place(OPIS, OFF_THE_MAP),
    Op::Dialogue(0x21F),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PORTAL_ROOM,
        player: (10, 0xB),
        objects: PORTAL_ROOM_OBJECTS,
        count: 3,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    glide16(2, (0x80, 0xD0), 2, true),
    glide16(2, (0xA0, 0xD0), 2, true),
    Op::Face(2, Direction::Up),
    Op::Wait(60),
    Op::Dialogue(0x220),
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
    Op::Face(PLAYER, Direction::Up),
    Op::Dialogue(0x221),
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

/// The throne room (`0x0802205C`): the first time, it marks group 7 as
/// met, keeps the party off the sea again and plays the opening.
const THRONE_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[],
    none: &[CHAPTER_OPENED],
    then: &[
        Op::Meet(CHAPTER_GROUP),
        Op::LoadMap {
            map: THRONE_ROOM,
            player: (8, 2),
            objects: THRONE_OBJECTS,
            count: 8,
        },
        Op::Place(PLAYER, HIDDEN),
        Op::Control(false),
        Op::Spawn(MAP_TASK, OPENING),
        Op::Flag(CHAPTER_OPENED, true),
        Op::SeaCrossing(false),
    ],
    otherwise: &[],
}];

/// The plains with the portal (ROM `0x0866A76C`): an explosion off the
/// map and the portal.
const ARRIVAL_OBJECTS: u32 = 0x0866_A76C;
const EXPLOSION: usize = 1;
const PLAINS_PORTAL: usize = 2;
const PLAINS_PORTAL_CELL: (usize, usize) = (0x11, 7);
const PLAINS_ARRIVAL_CELL: (usize, usize) = (0x11, 8);
/// Raven's fight with Hiltz (ROM `0x0866AAD4`): Raven's Zoid, Reese's off
/// the map, and the Death Stinger.
const RAVEN_FIGHT_OBJECTS: u32 = 0x0866_AAD4;
const REESE_ZOID: usize = 2;

const PRINCE_DRIVES_ON: &[Op] = &[
    glide(PLAYER, (0x220, 0x120), 1, true),
    glide(PLAYER, (0x200, 0x120), 1, true),
    glide(PLAYER, (0x200, 0x140), 1, true),
    glide(PLAYER, (0x1C0, 0x140), 1, true),
    Op::End,
];

/// Near New Helic City (task at `0x08022628`): the portal brings the party,
/// which places itself (`0x223`); blasts go off around it (`0x224`) and it
/// drives on; Raven faces Hiltz (`0x225`); the hook at `0x080227B8`
/// stages Hiltz's shot.
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
    Op::Dialogue(0x223),
    Op::Call(&blast_near(EXPLOSION, (1, -1))),
    Op::Call(&blast_near(EXPLOSION, (-1, 0))),
    Op::Dialogue(0x224),
    Op::Spawn(MAP_TASK + 1, PRINCE_DRIVES_ON),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::LoadMap {
        map: PLAINS,
        player: (0xD, 0xC),
        objects: RAVEN_FIGHT_OBJECTS,
        count: 4,
    },
    Op::RestartMusic(ENEMY_MUSIC),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x225),
    Op::Spawn(FIELD_HOOK, RAVEN_SHOT),
    Op::End,
];

/// Hiltz's shot at Raven (the hook at `0x080227B8`, battle scene 10): then
/// Reese comes (task at `0x080227F0`).
const RAVEN_SHOT: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Call(staged!(10)),
    Op::Control(false),
    Op::Spawn(MAP_TASK, REESE_COMES),
    Op::End,
];

/// Reese (task at `0x080227F0`): Raven falls (`0x226`); Reese drives up
/// and Hiltz casts her off (`0x227`); the party drives up and sees the
/// Death Stinger charge its cannon (`0x228`); the hook at `0x080228B0`
/// stages the shot.
const REESE_COMES: &[Op] = &[
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x226),
    Op::Place(REESE_ZOID, (8, 0xB)),
    glide(REESE_ZOID, (0x180, 0x160), 1, true),
    Op::Dialogue(0x227),
    Op::Place(PLAYER, (0x10, 9)),
    glide(PLAYER, (0x200, 0x120), 1, false),
    glide(PLAYER, (0x200, 0x140), 1, false),
    glide(PLAYER, (0x1C0, 0x140), 1, false),
    Op::Face(PLAYER, Direction::Down),
    Op::Dialogue(0x228),
    Op::Spawn(FIELD_HOOK, CHARGED_BEAM),
    Op::End,
];

/// The charged particle beam (the hook at `0x080228B0`, battle scene 11):
/// then the city is razed (task at `0x080228E8`).
const CHARGED_BEAM: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Call(staged!(11)),
    Op::Control(false),
    Op::Spawn(MAP_TASK, CITY_RAZED),
    Op::End,
];

const TWO_BLASTS: &[Op] = &[
    Op::Sound(BLAST_SOUND),
    Op::AwaitSoundEnd(BLAST_SOUND),
    Op::Sound(BLAST_SOUND),
    Op::Wait(15),
    Op::Sound(BLAST_SOUND),
    Op::AwaitSoundEnd(BLAST_SOUND),
];

/// The city razed (task at `0x080228E8`): the beam whitens the field amid
/// blasts; when it fades the Zoids are gone and the party is unhurt
/// (`0x229`).
const CITY_RAZED: &[Op] = &[
    Op::Sound(BEAM_SOUND),
    Op::Call(SCENE_BRIGHTEN),
    Op::Call(&WHITEN),
    Op::Repeat(2, TWO_BLASTS),
    Op::Place(1, OFF_THE_MAP),
    Op::Place(2, OFF_THE_MAP),
    Op::Place(3, OFF_THE_MAP),
    Op::Call(&UNWHITEN),
    Op::Wait(60),
    Op::Dialogue(0x229),
    Op::Call(&back_to(PLAINS, (0xE, 0xA))),
];

/// The plains for Blood's claim (ROM `0x0866A7A8`): Blood's Zoid.
const BLOOD_CLAIM_OBJECTS: u32 = 0x0866_A7A8;
const BLOOD_ZOID: usize = 1;

/// Blood's claim (task at `0x08022A60`): Blood drives up and warns the
/// party off the Death Stinger (`0x230`) and leaves; the party resolves to
/// stop it (`0x231`).
const BLOOD_CLAIMS: &[Op] = &[
    Op::Wait(SETTLE),
    glide(BLOOD_ZOID, (0x180, 0x1A0), 2, true),
    Op::Dialogue(0x230),
    glide(BLOOD_ZOID, (0x120, 0x1A0), 2, true),
    Op::Place(BLOOD_ZOID, OFF_THE_MAP),
    Op::Dialogue(0x231),
    Op::Call(&back_to(PLAINS, (0xD, 0xD))),
];

/// The plains for Schwarz (ROM `0x0866AB24`): Schwarz's Zoid.
const SCHWARZ_OBJECTS: u32 = 0x0866_AB24;
const SCHWARZ_ZOID: usize = 1;

/// Schwarz (task at `0x08022AF0`): his Zoid drives up and he tells of his
/// division's fall (`0x232`), and joins.
const SCHWARZ: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PLAINS,
        player: (4, 0x14),
        objects: SCHWARZ_OBJECTS,
        count: 2,
    },
    Op::Call(SCENE_BRIGHTEN),
    glide(SCHWARZ_ZOID, (0x80, 0x260), 1, true),
    Op::Wait(15),
    Op::Face(PLAYER, Direction::Up),
    Op::Wait(15),
    Op::Dialogue(0x232),
    glide(SCHWARZ_ZOID, (0x80, 0x280), 1, true),
    Op::Place(SCHWARZ_ZOID, OFF_THE_MAP),
    Op::Join(SCHWARZ_LIST),
    Op::Call(&back_to(PLAINS, (4, 0x14))),
];

/// Where Schwarz waits (the hook at `0x080213F0`): at x `0x80`, y
/// `0x280`.
const SCHWARZ_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x80, 0x80),
        y: (0x280, 0x280),
    },
    Op::Calm(true),
    Op::Control(false),
    Op::Spawn(MAP_TASK, SCHWARZ),
    Op::Flag(SCHWARZ_MET, true),
    Op::End,
];

/// The lake (ROM `0x0866AB4C`): the Ultrasaurus off the map.
const LAKE_OBJECTS: u32 = 0x0866_AB4C;
const ULTRASAURUS: usize = 1;
/// The Ultrasaurus's hold (ROM `0x0866AB74`): Regina, Jack and Earth, and
/// Van and four of his friends off the map.
const HOLD_OBJECTS: u32 = 0x0866_AB74;
const HOLD_FRIENDS: [usize; 4] = [4, 5, 6, 8];
const HOLD_VAN: usize = 7;
/// The Ultrasaurus's bridge (ROM `0x0866AC28`): the party, Van's friends,
/// the crew, and the screen's Hiltz off the map.
const BRIDGE_OBJECTS: u32 = 0x0866_AC28;
const BRIDGE_SCREEN: usize = 14;
/// The plains as Van holds off the Hellcats (ROM `0x0866AD54`): the
/// Ultrasaurus and Van's Zoids.
const HOLD_OFF_OBJECTS: u32 = 0x0866_AD54;

const fn comes_into_the_hold(actor: usize, cell: (usize, usize), x: i32) -> [Op; 3] {
    [
        Op::Place(actor, cell),
        glide16(actor, (x, 0x110), 1, true),
        Op::End,
    ]
}
const FIRST_FRIEND_COMES: &[Op] = &comes_into_the_hold(HOLD_FRIENDS[0], (9, 0xD), 0x90);
const SECOND_FRIEND_COMES: &[Op] = &comes_into_the_hold(HOLD_FRIENDS[1], (0xB, 0xD), 0xB0);
const THIRD_FRIEND_COMES: &[Op] = &comes_into_the_hold(HOLD_FRIENDS[2], (0xC, 0xD), 0xC0);
const fn leaves_the_hold(actor: usize, x: i32) -> [Op; 3] {
    [
        glide16(actor, (x, 0xD0), 1, true),
        Op::Place(actor, OFF_THE_MAP),
        Op::End,
    ]
}
const FIRST_FRIEND_LEAVES: &[Op] = &leaves_the_hold(HOLD_FRIENDS[0], 0x90);
const SECOND_FRIEND_LEAVES: &[Op] = &leaves_the_hold(HOLD_FRIENDS[1], 0xB0);
const THIRD_FRIEND_LEAVES: &[Op] = &leaves_the_hold(HOLD_FRIENDS[2], 0xC0);
const FOURTH_FRIEND_LEAVES: &[Op] = &leaves_the_hold(HOLD_FRIENDS[3], 0x90);
const VAN_LEAVES: &[Op] = &leaves_the_hold(HOLD_VAN, 0xA0);
const ALERT: &[Op] = &[
    Op::Sound(ALARM_SOUND),
    Op::AwaitSoundEnd(ALARM_SOUND),
    Op::Sound(SIREN_SOUND),
    Op::AwaitSoundEnd(SIREN_SOUND),
];
/// Hiltz on the bridge's screen (task at `0x08023058`): shown two frames,
/// hidden two.
const SCREEN_FLICKERS: &[Op] = &[Op::Loop(&[
    Op::Place(BRIDGE_SCREEN, (9, 9)),
    Op::Wait(2),
    Op::Place(BRIDGE_SCREEN, OFF_THE_MAP),
    Op::Wait(2),
])];

/// The Ultrasaurus (task at `0x08022BA0`): the lake drains (`0x233`) and
/// the Ultrasaurus rises (`0x234`); Schwarz leaves the party, which drives
/// aboard; in the hold Van (`0x235`) and his friends greet the party
/// (`0x236`) until the alarm sounds (`0x237`); on the bridge the crew
/// plots the course (`0x238`), Hiltz taunts them from the screen
/// (`0x239`), Hermann plans to stall him (`0x23A`) and the party joins in
/// (`0x23B`); outside the Hellcats strike and Van holds them off
/// (`0x23C`); the hook at `0x08022EE8` builds the plains' Zoids again.
const ULTRASAURUS_RISES: &[Op] = &[
    glide(PLAYER, (0x180, 0x2C0), 1, true),
    Op::Face(PLAYER, Direction::Right),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PLAINS,
        player: (0xE, 0x16),
        objects: LAKE_OBJECTS,
        count: 2,
    },
    Op::Place(PLAYER, (0xC, 0x16)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x233),
    Op::Place(ULTRASAURUS, (0xE, 0x16)),
    Op::Wait(15),
    Op::Dialogue(0x234),
    Op::Leave(SCHWARZ_LIST),
    glide(PLAYER, (0x1C0, 0x2C0), 1, false),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: HOLD,
        player: (10, 4),
        objects: HOLD_OBJECTS,
        count: 9,
    },
    Op::Place(PLAYER, (10, 0x13)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Repeat(0xF0, PAN_DOWN),
    Op::Dialogue(0x235),
    Op::Place(HOLD_VAN, (10, 0xD)),
    glide16(HOLD_VAN, (0xA0, 0x110), 1, true),
    Op::Spawn(MAP_TASK + 1, FIRST_FRIEND_COMES),
    Op::Spawn(MAP_TASK + 2, SECOND_FRIEND_COMES),
    Op::Spawn(MAP_TASK + 3, THIRD_FRIEND_COMES),
    Op::Place(HOLD_FRIENDS[3], (9, 0xC)),
    glide16(HOLD_FRIENDS[3], (0x90, 0x100), 1, true),
    Op::Dialogue(0x236),
    Op::Repeat(3, ALERT),
    Op::RestartMusic(DANGER_MUSIC),
    Op::Dialogue(0x237),
    Op::Spawn(MAP_TASK + 1, FIRST_FRIEND_LEAVES),
    Op::Spawn(MAP_TASK + 2, SECOND_FRIEND_LEAVES),
    Op::Spawn(MAP_TASK + 3, THIRD_FRIEND_LEAVES),
    Op::Spawn(MAP_TASK + 4, FOURTH_FRIEND_LEAVES),
    Op::Spawn(MAP_TASK + 5, VAN_LEAVES),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::Stop(MAP_TASK + 2),
    Op::Stop(MAP_TASK + 3),
    Op::Stop(MAP_TASK + 4),
    Op::Stop(MAP_TASK + 5),
    Op::LoadMap {
        map: BRIDGE,
        player: (9, 0x12),
        objects: BRIDGE_OBJECTS,
        count: 15,
    },
    Op::Place(PLAYER, (8, 0x14)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Repeat(0x80, PAN_UP),
    Op::RestartMusic(ENEMY_MUSIC),
    Op::Dialogue(0x238),
    Op::Spawn(MAP_TASK + 1, SCREEN_FLICKERS),
    Op::Wait(15),
    Op::Face(8, Direction::Right),
    Op::Face(9, Direction::Left),
    Op::Face(12, Direction::Left),
    Op::Wait(15),
    Op::Dialogue(0x239),
    Op::Stop(MAP_TASK + 1),
    Op::Place(BRIDGE_SCREEN, OFF_THE_MAP),
    Op::Wait(15),
    Op::Dialogue(0x23A),
    Op::Repeat(0x90, PAN_DOWN),
    Op::Wait(15),
    Op::Face(10, Direction::Down),
    Op::Face(13, Direction::Down),
    Op::Wait(15),
    glide16(PLAYER, (0x80, 0x130), 1, false),
    Op::Dialogue(0x23B),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PLAINS,
        player: (0xE, 0x16),
        objects: HOLD_OFF_OBJECTS,
        count: 4,
    },
    Op::Place(PLAYER, (0xD, 0x17)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x23C),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, AFTER_THE_ULTRASAURUS),
    Op::End,
];

/// Back on the plains (the hook at `0x08022EE8`): the area's Zoids are
/// built again (`0x08006E4C`) and the party is taken below the lake.
const AFTER_THE_ULTRASAURUS: &[Op] = &[Op::RebuildObjects, Op::Call(&warp_to(PLAINS, (0xD, 0x17)))];

/// Where the lake is (the hook at `0x0802144C`): at x `0x180`, y `0x280`
/// to `0x2E0`.
const ULTRASAURUS_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x180, 0x180),
        y: (0x280, 0x2E0),
    },
    Op::Calm(true),
    Op::Control(false),
    Op::Spawn(MAP_TASK, ULTRASAURUS_RISES),
    Op::Flag(ULTRASAURUS_SEEN, true),
    Op::End,
];

/// The plains' objects while the Death Stinger stands off: the
/// Ultrasaurus and Van's Zoids at the lake, the Death Stinger, and an
/// explosion.
const DEATH_STINGER: usize = 4;
const PLAINS_EXPLOSION: usize = 5;
/// The cells the explosions go off on (ROM `0x0866A7D0`).
const EXPLOSION_CELLS: &[(usize, usize)] = &[(0xD, 0x15), (0xD, 0x16), (0xF, 0x15), (0xD, 0x17)];

/// The Death Stinger's barrage around the lake (the hooks at `0x080215C8`
/// and `0x0802163C`): an explosion on one of four cells drawn at random,
/// then the next once it has played.
const BARRAGE: &[Op] = &[Op::Loop(&[
    Op::PlaceRandom(PLAINS_EXPLOSION, EXPLOSION_CELLS),
    Op::PlayOnce(PLAINS_EXPLOSION, 0),
    Op::Sound(BLAST_SOUND),
    Op::AwaitAnimation(PLAINS_EXPLOSION),
    Op::Place(PLAINS_EXPLOSION, OFF_THE_MAP),
    Op::Wait(2),
])];

/// The Death Stinger spoken to (`0x080214B4`): the barrage's hook gives
/// way to Hiltz's.
const DEATH_STINGER_TALK: &[Op] = &[
    Op::Stop(FIELD_WATCH),
    Op::Control(false),
    Op::Spawn(MAP_TASK, HILTZ_TAUNTS),
];

/// Hiltz (task at `0x0802307C`): no one escapes (`0x23D`); the hook at
/// `0x080230C4` starts the fight.
const HILTZ_TAUNTS: &[Op] = &[
    Op::RestartMusic(DANGER_MUSIC),
    Op::Dialogue(0x23D),
    Op::Spawn(FIELD_HOOK, STINGER_FIGHT),
    Op::End,
];

/// The Death Stinger's fight (story battle 30).
const STINGER_BATTLE_OPS: [Op; 4] = story_battle(30);
/// The Death Stinger's battle (the hook at `0x080230C4`): held off, it
/// takes aim at the Ultrasaurus (task at `0x08023134`).
const STINGER_FIGHT: &[Op] = &[
    Op::Call(&STINGER_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::FadeInHolding,
            Op::Flag(STINGER_REPELLED, true),
            Op::Control(false),
            Op::Spawn(MAP_TASK, STINGER_AIMS),
        ],
    },
    Op::End,
];

/// The Death Stinger takes aim (task at `0x08023134`): the view goes down
/// to the Ultrasaurus and back, and Hiltz fires (`0x23E`); the hook at
/// `0x0802319C` stages the shot.
const STINGER_AIMS: &[Op] = &[
    Op::Repeat(0x50, PAN_DOWN_FAST),
    Op::Wait(60),
    Op::Repeat(0x50, PAN_UP_FAST),
    Op::Dialogue(0x23E),
    Op::Spawn(FIELD_HOOK, STINGER_FIRES),
    Op::End,
];

/// The Death Stinger's shot (the hook at `0x0802319C`, battle scene 12):
/// then it leaves (task at `0x080231D8`).
const STINGER_FIRES: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Call(staged!(12)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Control(false),
    Op::Spawn(MAP_TASK, STINGER_LEAVES),
    Op::End,
];

/// The Death Stinger leaves (task at `0x080231D8`): the Ultrasaurus dodged
/// the shot (`0x23F`); the Death Stinger drives off north, the party
/// stepping out of its way when it stands there, and it was let go
/// (`0x240`).
const STINGER_LEAVES: &[Op] = &[
    Op::Dialogue(0x23F),
    Op::IfPlayerSprite {
        x: (0x1A0, 0x1A0),
        y: (0x140, 0x140),
        then: &[glide(PLAYER, (0x1A0, 0x160), 1, false)],
        otherwise: &[],
    },
    glide(DEATH_STINGER, (0x1A0, 0x140), 2, false),
    glide(DEATH_STINGER, (0x1A0, 0xE0), 2, false),
    Op::Place(DEATH_STINGER, OFF_THE_MAP),
    Op::Dialogue(0x240),
    Op::Call(&back_to(PLAINS, HERE)),
];

/// The lake for Dr. D's message (ROM `0x0866ADA4`): a soldier's Gustav.
const MESSENGER_OBJECTS: u32 = 0x0866_ADA4;
const MESSENGER: usize = 1;
/// The world map on the way to Dr. D (ROM `0x0866ADCC`): the party's Gustav
/// alone.
const TO_DR_D_OBJECTS: u32 = 0x0866_ADCC;
/// Dr. D's dig (ROM `0x0866ADE0`): Regina, Jack and Earth, two of Van's
/// friends and Dr. D.
const DIG_OBJECTS: u32 = 0x0866_ADE0;
const DR_D: usize = 6;
/// The world map after (ROM `0x0866AE6C`): the Gustav alone.
const AFTER_DR_D_OBJECTS: u32 = 0x0866_AE6C;

const MESSENGER_DRIVES_OFF: &[Op] = &[glide(MESSENGER, (0x260, 0x2C0), 2, false), Op::End];
const REGINA_WALKS_UP: &[Op] = &[glide16(REGINA, (0x150, 0x220), 1, false), Op::End];
const JACK_WALKS_UP: &[Op] = &[glide16(JACK, (0x160, 0x230), 1, false), Op::End];
const EARTH_WALKS_UP: &[Op] = &[glide16(EARTH, (0x150, 0x230), 1, false), Op::End];
const FIRST_FRIEND_WALKS_UP: &[Op] = &[glide16(4, (0x170, 0x200), 1, false), Op::End];
const SECOND_FRIEND_WALKS_UP: &[Op] = &[glide16(5, (0x150, 0x200), 1, false), Op::End];
/// Along the cannon (task at `0x080235D0`): a pixel left every other frame
/// for 528 frames.
const ALONG_THE_CANNON: &[Op] = &[Op::Repeat(0x210, PAN_LEFT_SLOW), Op::End];

/// Dr. D (task at `0x080232A4`): a soldier brings Hermann's message
/// (`0x241`) and drives off; the party drives north to Dr. D's dig, where
/// he tells of his cannon (`0x242`) and shows it (`0x243`), and lists
/// `0x1D` and `0x1E` join; on the world map he calls about the Planetal Sites (`0x244`).
const DR_D_SCENE: &[Op] = &[
    glide(PLAYER, (0x1C0, 0x2C0), 1, true),
    Op::Face(PLAYER, Direction::Right),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PLAINS,
        player: (0xE, 0x16),
        objects: MESSENGER_OBJECTS,
        count: 2,
    },
    Op::Call(SCENE_BRIGHTEN),
    glide(MESSENGER, (0x1E0, 0x2C0), 2, false),
    Op::Dialogue(0x241),
    glide(PLAYER, (0x1E0, 0x2C0), 1, false),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Face(MESSENGER, Direction::Right),
    Op::Wait(30),
    Op::Spawn(MAP_TASK + 1, MESSENGER_DRIVES_OFF),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::LoadMap {
        map: WORLD,
        player: (0x11, 1),
        objects: TO_DR_D_OBJECTS,
        count: 1,
    },
    Op::Place(PLAYER, (0x11, 5)),
    Op::Call(SCENE_BRIGHTEN),
    glide(PLAYER, (0x220, 0x20), 2, false),
    Op::Wait(15),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: DIG,
        player: (0x16, 0x25),
        objects: DIG_OBJECTS,
        count: 7,
    },
    Op::Place(PLAYER, (0x16, 0x25)),
    Op::Spawn(MAP_TASK + 1, REGINA_WALKS_UP),
    Op::Spawn(MAP_TASK + 2, JACK_WALKS_UP),
    Op::Spawn(MAP_TASK + 3, EARTH_WALKS_UP),
    Op::Spawn(MAP_TASK + 4, BRIGHTENS),
    glide16(PLAYER, (0x160, 0x220), 1, false),
    Op::Wait(1),
    Op::Spawn(MAP_TASK + 1, FIRST_FRIEND_WALKS_UP),
    Op::Spawn(MAP_TASK + 2, SECOND_FRIEND_WALKS_UP),
    glide16(DR_D, (0x160, 0x200), 1, false),
    Op::Dialogue(0x242),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: DIG,
        player: (0x28, 10),
        objects: DIG_OBJECTS,
        count: 7,
    },
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, ALONG_THE_CANNON),
    Op::Dialogue(0x243),
    Op::Join(DR_D_LISTS[0]),
    Op::Join(DR_D_LISTS[1]),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::LoadMap {
        map: WORLD,
        player: (0x12, 2),
        objects: AFTER_DR_D_OBJECTS,
        count: 1,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x244),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, &warp_to(WORLD, (0x12, 3))),
    Op::End,
];

/// Where Hermann's messenger waits (the hook at `0x080214E0`): at x
/// `0x180`, y `0x280` to `0x2E0`; the party may cross the sea from then on.
const DR_D_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x180, 0x180),
        y: (0x280, 0x2E0),
    },
    Op::Calm(true),
    Op::Control(false),
    Op::Spawn(MAP_TASK, DR_D_SCENE),
    Op::ReturnPoint(PLAINS_RETURN_POINT),
    Op::Flag(CANNON_SEEN, true),
    Op::SeaCrossing(true),
    Op::End,
];

/// The plains for Opis's last stand (ROM `0x0866B3E4`): Opis's Zoid
/// off the map and the portal.
const CORNERED_OBJECTS: u32 = 0x0866_B3E4;
const OPIS_ZOID: usize = 1;

/// Opis cornered (task at `0x080250FC`): the party drives up to the
/// portal, Opis follows and gloats (`0x26E`); the hook at `0x080251F8`
/// starts the fight.
const OPIS_CORNERED: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PLAINS,
        player: (0x10, 10),
        objects: CORNERED_OBJECTS,
        count: 3,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    glide(PLAYER, (0x200, 0x120), 1, true),
    glide(PLAYER, (0x220, 0x120), 1, true),
    glide(PLAYER, (0x220, 0x100), 1, true),
    Op::Face(PLAYER, Direction::Up),
    Op::Place(OPIS_ZOID, (0x10, 10)),
    glide(OPIS_ZOID, (0x200, 0x120), 1, true),
    glide(OPIS_ZOID, (0x220, 0x120), 1, true),
    Op::Face(OPIS_ZOID, Direction::Up),
    Op::Repeat(0x20, PAN_DOWN),
    Op::Dialogue(0x26E),
    Op::Spawn(FIELD_HOOK, OPIS_FIGHT),
    Op::End,
];

/// Opis's fight (story battle 35).
const OPIS_BATTLE_OPS: [Op; 4] = story_battle(35);
/// Opis's battle (the hook at `0x080251F8`): won, the party questions
/// him (task at `0x08025260`).
const OPIS_FIGHT: &[Op] = &[
    Op::Call(&OPIS_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::FadeInHolding,
            Op::Control(false),
            Op::Spawn(MAP_TASK, OPIS_BEATEN),
        ],
    },
    Op::End,
];

/// Opis beaten (task at `0x08025260`): the Death Saurer's Zoid factor
/// is already the Emperor's (`0x26F`); Opis flees, and the party
/// hurries home (`0x270`), to chapter 9.
const OPIS_BEATEN: &[Op] = &[
    Op::Dialogue(0x26F),
    Op::Call(SCENE_DARKEN),
    Op::Place(OPIS_ZOID, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x270),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, &warp_to(CHAPTER_9_START, (1, 1))),
    Op::End,
];

/// Where Opis waits (the hook at `0x08021574`): at x `0x200`, y
/// `0x140`, by the portal.
const OPIS_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x200, 0x200),
        y: (0x140, 0x140),
    },
    Op::Calm(true),
    Op::Control(false),
    Op::Spawn(MAP_TASK, OPIS_CORNERED),
    Op::End,
];

/// The plains' watches by the chapter's progress (`0x080211B8`): Schwarz,
/// the lake, the barrage, Dr. D's messenger; Eve Polis's way while the
/// party is sent there (the second hook at `0x0802155C`); Opis once the
/// Death Saurer is beaten.
const PLAINS_PHASES: &[Op] = &[Op::IfFlags {
    all: &[BLOOD_WARNED],
    none: &[SCHWARZ_MET],
    then: &[Op::Spawn(FIELD_WATCH, SCHWARZ_WATCH)],
    otherwise: &[Op::IfFlags {
        all: &[SCHWARZ_MET],
        none: &[ULTRASAURUS_SEEN],
        then: &[Op::Spawn(FIELD_WATCH, ULTRASAURUS_WATCH)],
        otherwise: &[Op::IfFlags {
            all: &[ULTRASAURUS_SEEN],
            none: &[STINGER_REPELLED],
            then: &[Op::Spawn(FIELD_WATCH, BARRAGE)],
            otherwise: &[Op::IfFlags {
                all: &[STINGER_REPELLED],
                none: &[CANNON_SEEN],
                then: &[Op::Spawn(FIELD_WATCH, DR_D_WATCH)],
                otherwise: &[Op::IfFlags {
                    all: &[EVE_POLIS_BOUND],
                    none: &[SAURER_BEATEN],
                    then: &[Op::RedirectWarps(WarpRedirect {
                        from: (EVE_POLIS_WAY, EVE_POLIS_WAY),
                        to: EVE_POLIS,
                        cell: None,
                    })],
                    otherwise: &[Op::IfFlags {
                        all: &[SAURER_BEATEN],
                        none: &[],
                        then: &[Op::Spawn(FIELD_WATCH, OPIS_WATCH)],
                        otherwise: &[],
                    }],
                }],
            }],
        }],
    }],
}];

/// The plains (`0x080211B8`): the portal brings the party the first time;
/// once Opis is seen, Blood claims the Death Stinger; then the watches
/// by progress, and the lake's Zoids and the Death Stinger stand only
/// while the Death Stinger stands off.
const PLAINS_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CHAPTER_OPENED],
    none: &[ARRIVED],
    then: &[
        Op::LoadMap {
            map: PLAINS,
            player: PLAINS_PORTAL_CELL,
            objects: ARRIVAL_OBJECTS,
            count: 3,
        },
        Op::Place(PLAYER, OFF_THE_MAP),
        Op::Control(false),
        Op::Spawn(MAP_TASK, PORTAL_ARRIVAL),
        Op::Flag(ARRIVED, true),
    ],
    otherwise: &[Op::IfFlags {
        all: &[OPIS_SEEN],
        none: &[BLOOD_WARNED],
        then: &[
            Op::LoadMap {
                map: PLAINS,
                player: (0xD, 0xD),
                objects: BLOOD_CLAIM_OBJECTS,
                count: 2,
            },
            Op::RestartMusic(ENEMY_MUSIC),
            Op::Control(false),
            Op::Spawn(MAP_TASK, BLOOD_CLAIMS),
            Op::Flag(BLOOD_WARNED, true),
        ],
        otherwise: &[
            Op::Call(PLAINS_PHASES),
            Op::IfFlags {
                all: &[ULTRASAURUS_SEEN],
                none: &[STINGER_REPELLED],
                then: &[],
                otherwise: &[
                    Op::Place(1, HIDDEN),
                    Op::Place(2, HIDDEN),
                    Op::Place(3, HIDDEN),
                    Op::Place(DEATH_STINGER, HIDDEN),
                ],
            },
        ],
    }],
}];

/// The city's ruins the first time (ROM `0x0866A7F0`): the prince and
/// Regina.
const RUINS_OBJECTS: u32 = 0x0866_A7F0;

/// The ruins (task at `0x08022A00`): the party mourns and looks for
/// survivors (`0x22A`).
const RUINS_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Dialogue(0x22A),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, &warp_to(CITY, (0x19, 0x1B))),
    Op::End,
];

/// The ruins for Opis (ROM `0x0866B420`): Regina, Jack, Earth and
/// Opis.
const OPIS_OBJECTS: u32 = 0x0866_B420;
const RUINS_OPIS: usize = 4;

const REGINA_RUNS_DOWN: &[Op] = &[glide16(REGINA, (0x180, 0x110), 2, true), Op::End];
const PRINCE_RUNS_DOWN: &[Op] = &[glide16(PLAYER, (0x190, 0x110), 2, false), Op::End];
const JACK_RUNS_DOWN: &[Op] = &[glide16(JACK, (0x1A0, 0x110), 2, false), Op::End];

/// Opis in the ruins (task at `0x080252E0`): the party spots him
/// (`0x22B`) and runs after him (`0x22C`); Regina calls out (`0x22D`),
/// Opis mocks them (`0x22E`) and leaves.
const OPIS_IN_THE_RUINS: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: CITY,
        player: (0x19, 0xF),
        objects: OPIS_OBJECTS,
        count: 5,
    },
    Op::RestartMusic(ENEMY_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x22B),
    Op::Spawn(MAP_TASK + 1, REGINA_RUNS_DOWN),
    Op::Spawn(MAP_TASK + 2, PRINCE_RUNS_DOWN),
    Op::Repeat(0x30, PAN_DOWN),
    Op::Dialogue(0x22C),
    Op::Spawn(MAP_TASK + 1, JACK_RUNS_DOWN),
    glide16(EARTH, (0x1A0, 0x100), 2, true),
    Op::Face(RUINS_OPIS, Direction::Up),
    Op::Dialogue(0x22D),
    Op::Dialogue(0x22E),
    glide16(RUINS_OPIS, (0x190, 0x1A0), 2, true),
    Op::Place(RUINS_OPIS, OFF_THE_MAP),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, &warp_to(CITY, (0x19, 0x11))),
    Op::End,
];

/// Where Opis is seen (the hooks at `0x0802185C` and, before, at
/// `0x08021814`): between x `0x178` and `0x198`, y `0xF0`; the row above
/// counts as near.
const OPIS_SIGHTED: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x178, 0x198),
        y: (0xF0, 0xF0),
    },
    Op::Control(false),
    Op::Spawn(MAP_TASK, OPIS_IN_THE_RUINS),
    Op::Flag(OPIS_SEEN, true),
    Op::End,
];
const OPIS_APPROACHED: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x178, 0x198),
        y: (0xE0, 0xE0),
    },
    Op::Flag(OPIS_NEAR, true),
    Op::Call(OPIS_SIGHTED),
];

/// Raven by the ruins (ROM `0x0866AF20`): Raven.
const RAVEN_OBJECTS: u32 = 0x0866_AF20;
const RUINS_RAVEN: usize = 1;
/// Raven's hideout (ROM `0x0866AE80`): Regina, Jack and Earth, Raven,
/// Reese and her friends.
const HIDEOUT_OBJECTS: u32 = 0x0866_AE80;
const HIDEOUT_RAVEN: usize = 4;

const PRINCE_FOLLOWS_RAVEN: &[Op] = &[glide16(PLAYER, (0x220, 0x40), 1, false), Op::End];
const PRINCE_COMES_IN: &[Op] = &[
    glide16(PLAYER, (0xC0, 0x30), 1, false),
    glide16(PLAYER, (0xB0, 0x30), 1, false),
    Op::End,
];
const REGINA_COMES_IN: &[Op] = &[
    glide16(REGINA, (0xC0, 0x40), 1, false),
    glide16(REGINA, (0xB0, 0x40), 1, false),
    Op::End,
];
const JACK_COMES_IN_THE_HIDEOUT: &[Op] = &[
    glide16(JACK, (0xC0, 0x30), 1, false),
    Op::Face(JACK, Direction::Left),
    Op::End,
];
const fn leaves_the_hideout(actor: usize) -> [Op; 3] {
    [
        glide16(actor, (0xB0, 0x30), 1, false),
        Op::Place(actor, OFF_THE_MAP),
        Op::End,
    ]
}
const REGINA_LEAVES: &[Op] = &leaves_the_hideout(REGINA);
const JACK_LEAVES: &[Op] = &leaves_the_hideout(JACK);
const RAVEN_LEAVES: &[Op] = &leaves_the_hideout(HIDEOUT_RAVEN);
const SIXTH_LEAVES: &[Op] = &leaves_the_hideout(6);
const SEVENTH_LEAVES: &[Op] = &leaves_the_hideout(7);

/// Raven (task at `0x080235F8`): the party finds Raven by the ruins
/// (`0x246`) and, as he walks off, asks about the Death Stinger (`0x247`,
/// `0x248`); it follows him to his hideout, where Reese is (`0x249`), and the two join.
const RAVEN_SCENE: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: CITY,
        player: (0x22, 8),
        objects: RAVEN_OBJECTS,
        count: 2,
    },
    Op::Place(PLAYER, (0x22, 10)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Face(RUINS_RAVEN, Direction::Down),
    Op::Wait(15),
    Op::Dialogue(0x246),
    glide16(RUINS_RAVEN, (0x220, 0x60), 1, false),
    Op::Dialogue(0x247),
    Op::Face(RUINS_RAVEN, Direction::Down),
    Op::Dialogue(0x248),
    glide16(RUINS_RAVEN, (0x220, 0x40), 1, false),
    Op::Place(RUINS_RAVEN, OFF_THE_MAP),
    Op::Spawn(MAP_TASK + 1, PRINCE_FOLLOWS_RAVEN),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::LoadMap {
        map: HIDEOUT,
        player: (0xC, 8),
        objects: HIDEOUT_OBJECTS,
        count: 8,
    },
    Op::Spawn(MAP_TASK + 1, BRIGHTENS),
    Op::Spawn(MAP_TASK + 2, PRINCE_COMES_IN),
    Op::Spawn(MAP_TASK + 3, REGINA_COMES_IN),
    Op::Spawn(MAP_TASK + 4, JACK_COMES_IN_THE_HIDEOUT),
    glide16(EARTH, (0xC0, 0x40), 1, false),
    Op::Face(EARTH, Direction::Left),
    Op::Face(HIDEOUT_RAVEN, Direction::Right),
    Op::Wait(15),
    Op::Face(5, Direction::Right),
    Op::Face(6, Direction::Right),
    Op::Face(7, Direction::Right),
    Op::Wait(15),
    Op::Dialogue(0x249),
    Op::Spawn(MAP_TASK + 1, REGINA_LEAVES),
    Op::Spawn(MAP_TASK + 2, JACK_LEAVES),
    glide16(EARTH, (0xB0, 0x30), 1, false),
    Op::Place(EARTH, OFF_THE_MAP),
    Op::Wait(1),
    Op::Spawn(MAP_TASK + 1, RAVEN_LEAVES),
    Op::Spawn(MAP_TASK + 2, SIXTH_LEAVES),
    Op::Spawn(MAP_TASK + 3, SEVENTH_LEAVES),
    glide16(5, (0xB0, 0x30), 1, false),
    Op::Place(5, OFF_THE_MAP),
    Op::Wait(1),
    Op::Join(RAVEN_LISTS[0]),
    Op::Join(RAVEN_LISTS[1]),
    Op::Call(&back_to(HIDEOUT, (0xB, 3))),
];

/// Where Raven stands (the hook at `0x080217C0`): at x `0x218`, y `0xC0`.
const RAVEN_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0x218, 0x218),
        y: (0xC0, 0xC0),
    },
    Op::Control(false),
    Op::Spawn(MAP_TASK, RAVEN_SCENE),
    Op::Flag(RAVEN_JOINED, true),
    Op::End,
];

/// The city's ruins (`0x08021670`): the first time, the party mourns;
/// until Opis is seen his spot waits, and later, until the cannon
/// strikes, Raven's; once Schwarz has joined, the survivors are gone.
const CITY_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[ARRIVED],
    none: &[RUINS_SEEN],
    then: &[
        Op::LoadMap {
            map: CITY,
            player: (0x19, 0x1B),
            objects: RUINS_OBJECTS,
            count: 2,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, RUINS_SCENE),
        Op::ReturnPoint(RUINS_RETURN_POINT),
        Op::Flag(RUINS_SEEN, true),
    ],
    otherwise: &[
        Op::IfFlags {
            all: &[RUINS_SEEN],
            none: &[OPIS_SEEN],
            then: &[Op::IfFlags {
                all: &[OPIS_NEAR],
                none: &[],
                then: &[Op::Spawn(FIELD_WATCH, OPIS_SIGHTED)],
                otherwise: &[Op::Spawn(FIELD_WATCH, OPIS_APPROACHED)],
            }],
            otherwise: &[Op::IfFlags {
                all: &[ARRIVED],
                none: &[STINGER_STRUCK, RAVEN_JOINED],
                then: &[Op::Spawn(FIELD_WATCH, RAVEN_WATCH)],
                otherwise: &[],
            }],
        },
        Op::IfFlags {
            all: &[SCHWARZ_MET],
            none: &[],
            then: &[
                Op::Place(1, HIDDEN),
                Op::Place(2, HIDDEN),
                Op::Place(3, HIDDEN),
                Op::Place(4, HIDDEN),
                Op::Place(5, HIDDEN),
                Op::Place(6, HIDDEN),
                Op::Place(7, HIDDEN),
                Op::Place(8, HIDDEN),
            ],
            otherwise: &[],
        },
    ],
}];

/// A Planetal Site found (task at `0x08021DA0`): it opens, Earth reports
/// it (`0x245`) and Jack counts those left in the ruin (`0x2D6`); with all
/// six, Dr. D calls: Blood has the last one (`0x24A`).
macro_rules! site_found {
    ($group:expr) => {
        &[
            Op::Face(1, Direction::Down),
            Op::Dialogue(0x245),
            Op::DialogueCounting {
                index: SITES_LEFT,
                flags: &$group,
            },
            Op::IfFlags {
                all: &SITES,
                none: &[],
                then: &[Op::Dialogue(0x24A)],
                otherwise: &[],
            },
            Op::Control(true),
            Op::Calm(false),
            Op::End,
        ]
    };
}
const ISLAND_SITE_FOUND: &[Op] = site_found!(ISLAND_SITES);
const WESTERN_SITE_FOUND: &[Op] = site_found!(WESTERN_SITES);

/// A Planetal Site spoken to (`0x08021B00` to `0x08021C84`): the first
/// time, it is found.
macro_rules! site {
    ($flag:expr, $found:expr) => {
        &[Op::IfFlags {
            all: &[],
            none: &[$flag],
            then: &[
                Op::Control(false),
                Op::Calm(true),
                Op::Flag($flag, true),
                Op::Spawn(MAP_TASK, $found),
            ],
            otherwise: &[],
        }]
    };
}

/// Jack's count on entering a ruin (tasks at `0x08021AC0` and
/// `0x08021F48`).
macro_rules! count_sites {
    ($group:expr) => {
        &[
            Op::Wait(SETTLE),
            Op::DialogueCounting {
                index: SITES_LEFT,
                flags: &$group,
            },
            Op::Control(true),
            Op::Calm(false),
            Op::End,
        ]
    };
}

/// A ruin's room (`0x08021E64` and the like): its Planetal Site stands
/// open once found.
macro_rules! site_room {
    ($flag:expr) => {
        &[Op::IfFlags {
            all: &[$flag],
            none: &[],
            then: &[Op::Face(1, Direction::Down)],
            otherwise: &[],
        }]
    };
}

/// A ruin's first room (`0x08021A1C` and `0x08021EA4`): its Planetal Site
/// stands open once found; it sets the cannon's flag; until Blood is
/// beaten, entering on `cell` with some of the ruin's sites left, Jack
/// counts them.
macro_rules! ruin_entrance {
    ($group:expr, $cell:expr) => {
        &[
            Op::IfFlags {
                all: &[$group[0]],
                none: &[],
                then: &[Op::Face(1, Direction::Down)],
                otherwise: &[],
            },
            Op::Flag(CANNON_SEEN, true),
            Op::IfFlags {
                all: &[],
                none: &[BLOOD_BEATEN],
                then: &[Op::IfFlags {
                    all: &$group,
                    none: &[],
                    then: &[],
                    otherwise: &[Op::IfPlayer {
                        columns: Some(($cell.0, $cell.0)),
                        rows: Some(($cell.1, $cell.1)),
                        then: &[
                            Op::Calm(true),
                            Op::Control(false),
                            Op::Spawn(MAP_TASK, count_sites!($group)),
                        ],
                        otherwise: &[],
                    }],
                }],
                otherwise: &[],
            },
        ]
    };
}

const ISLAND_SECOND_ARRIVAL: &[Op] = site_room!(SITES[1]);
const ISLAND_THIRD_ARRIVAL: &[Op] = site_room!(SITES[2]);
const WESTERN_SECOND_ARRIVAL: &[Op] = site_room!(SITES[4]);
const WESTERN_THIRD_ARRIVAL: &[Op] = site_room!(SITES[5]);
const ISLAND_RUINS_ARRIVAL: &[Op] = ruin_entrance!(ISLAND_SITES, (0x17_usize, 0x17_usize));
const WESTERN_RUINS_ARRIVAL: &[Op] = ruin_entrance!(WESTERN_SITES, (0x14_usize, 0x17_usize));

/// Dr. D's dig with Blood waiting (ROM `0x0866A818`): Regina, Jack and
/// Earth, Blood's Zoid, and Opis's off the map.
const BLOOD_WAITS_OBJECTS: u32 = 0x0866_A818;
const DIG_BLOOD: usize = 4;
/// The dig once Blood is beaten (ROM `0x0866AF48`): the same, and the
/// gravity's pull off the map.
const GRAVITY_OBJECTS: u32 = 0x0866_AF48;
const DIG_OPIS: usize = 5;
const GRAVITY: usize = 6;
/// The dig for the cannon (ROM `0x0866AFD4`): the party and Dr. D.
const CANNON_OBJECTS: u32 = 0x0866_AFD4;
/// The dig's own objects once Blood is beaten (ROM `0x08326108`).
const DIG_AFTER_OBJECTS: u32 = 0x0832_6108;

const REGINA_HURRIES_UP: &[Op] = &[glide16(REGINA, (0x130, 0x200), 2, true), Op::End];
const JACK_HURRIES_UP: &[Op] = &[glide16(JACK, (0x170, 0x200), 2, true), Op::End];
const EARTH_HURRIES_UP: &[Op] = &[glide16(EARTH, (0x190, 0x200), 2, true), Op::End];

/// Blood at the dig (task at `0x080239F0`): the party hurries up and Blood
/// demands the Planetal Sites (`0x24B`); the hook at `0x08023A74` starts
/// the fight.
const BLOOD_WAITS: &[Op] = &[
    Op::Spawn(MAP_TASK + 1, REGINA_HURRIES_UP),
    Op::Spawn(MAP_TASK + 2, JACK_HURRIES_UP),
    Op::Spawn(MAP_TASK + 3, EARTH_HURRIES_UP),
    glide16(PLAYER, (0x150, 0x200), 2, false),
    Op::RestartMusic(DANGER_MUSIC),
    Op::Dialogue(0x24B),
    Op::Spawn(FIELD_HOOK, BLOOD_FIGHT),
    Op::End,
];

/// Blood's fight (story battle 31).
const BLOOD_BATTLE_OPS: [Op; 4] = story_battle(31);
/// Blood's battle (the hook at `0x08023A74`): won, the dig loads again and
/// Opis comes (task at `0x08023B18`).
const BLOOD_FIGHT: &[Op] = &[
    Op::Call(&BLOOD_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::LoadMap {
                map: DIG,
                player: (0x15, 0x20),
                objects: GRAVITY_OBJECTS,
                count: 7,
            },
            Op::RestartMusic(ENEMY_MUSIC),
            Op::Place(PLAYER, (0x15, 0x20)),
            Op::ReturnPoint(HOLD_RETURN_POINT),
            Op::Flag(BLOOD_BEATEN, true),
            Op::Control(false),
            Op::Spawn(MAP_TASK, OPIS_BETRAYS),
        ],
    },
    Op::End,
];

/// Opis's betrayal (task at `0x08023B18`): Opis drives up and turns
/// on Blood (`0x24C`); the Planetal Site he gave Blood collapses into a
/// gravity well (`0x24D`); the prince pushes Blood's Zoid clear (`0x24E`);
/// the hook at `0x08023C50` stages the Gravity Cannon's shot.
const OPIS_BETRAYS: &[Op] = &[
    Op::Call(SCENE_BRIGHTEN),
    Op::Place(DIG_OPIS, (0xD, 0x1E)),
    glide16(DIG_OPIS, (0x110, 0x1E0), 2, true),
    Op::Dialogue(0x24C),
    Op::Place(GRAVITY, (0x15, 0x1D)),
    Op::PlayOnce(GRAVITY, 0),
    Op::Sound(GRAVITY_SOUND),
    Op::AwaitAnimation(GRAVITY),
    Op::Animate(GRAVITY, 1),
    Op::Wait(1),
    Op::AwaitSoundEnd(GRAVITY_SOUND),
    Op::Dialogue(0x24D),
    glide16(PLAYER, (0x150, 0x1F0), 2, false),
    Op::Face(PLAYER, Direction::Left),
    Op::Wait(15),
    Op::Face(PLAYER, Direction::Right),
    Op::Repeat(0x14, &[Op::Nudge(DIG_BLOOD, (4, 0)), Op::Wait(1)]),
    Op::Face(DIG_BLOOD, Direction::Left),
    Op::Wait(15),
    Op::Face(PLAYER, Direction::Left),
    Op::Wait(15),
    Op::Dialogue(0x24E),
    Op::Spawn(FIELD_HOOK, GRAVITY_SHOT),
    Op::End,
];

/// The Gravity Cannon's shot (the hook at `0x08023C50`, battle scene 13):
/// the well is gone and Opis flees (task at `0x08023CA8`).
const GRAVITY_SHOT: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Call(staged!(13)),
    Op::RestartMusic(RIVAL_MUSIC),
    Op::Place(GRAVITY, OFF_THE_MAP),
    Op::Control(false),
    Op::Spawn(MAP_TASK, OPIS_FLEES),
    Op::End,
];

/// Opis flees (task at `0x08023CA8`): his controls are hit (`0x24F`)
/// and he drives off; Regina runs to the prince (`0x250`); Blood asks why
/// he was saved (`0x251`) and leaves; the prince comes to (`0x252`); Dr. D
/// finishes his cannon with the Sites (`0x253`) and lists `0x1D` and
/// `0x1E` leave the party; on the bridge Dr. D tracks the Death Stinger (`0x255`).
const OPIS_FLEES: &[Op] = &[
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x24F),
    glide16(DIG_OPIS, (0xC0, 0x1E0), 1, true),
    Op::Place(DIG_OPIS, OFF_THE_MAP),
    Op::Dialogue(0x250),
    glide16(REGINA, (0x130, 0x1F0), 2, false),
    Op::Face(REGINA, Direction::Right),
    Op::Dialogue(0x251),
    glide16(DIG_BLOOD, (0x1A0, 0x1A0), 1, true),
    Op::Place(DIG_BLOOD, OFF_THE_MAP),
    Op::Dialogue(0x252),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: DIG,
        player: (0x18, 0x11),
        objects: CANNON_OBJECTS,
        count: 5,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Place(PLAYER, (0x18, 0x11)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x253),
    Op::Leave(DR_D_LISTS[0]),
    Op::Leave(DR_D_LISTS[1]),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: BRIDGE,
        player: (9, 10),
        objects: BRIDGE_OBJECTS_AFTER,
        count: 15,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Place(PLAYER, (8, 0x14)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x255),
    Op::Call(&back_to(HOLD, (10, 5))),
];
/// The bridge after the dig (ROM `0x0866B038`).
const BRIDGE_OBJECTS_AFTER: u32 = 0x0866_B038;

/// Dr. D's dig (`0x08021940`): with the six Planetal Sites and until Blood
/// is beaten, Blood waits; after, the dig has its own objects.
const DIG_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &SITES,
    none: &[BLOOD_BEATEN],
    then: &[
        Op::LoadMap {
            map: DIG,
            player: (0x15, 0x20),
            objects: BLOOD_WAITS_OBJECTS,
            count: 6,
        },
        Op::Place(PLAYER, (0x15, 0x25)),
        Op::Control(false),
        Op::Spawn(MAP_TASK, BLOOD_WAITS),
    ],
    otherwise: &[Op::IfFlags {
        all: &[BLOOD_BEATEN],
        none: &[],
        then: &[Op::LoadMap {
            map: DIG_AFTER,
            player: HERE,
            objects: DIG_AFTER_OBJECTS,
            count: 6,
        }],
        otherwise: &[],
    }],
}];

/// The Ultrasaurus's hold (`0x080218E8`): once Blood is beaten and until
/// the party is sent to Eve Polis, leaving for the world map or the plains
/// leads to the Ultrasaurus's cell of the world map (the second hook at
/// `0x08021918`).
const HOLD_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[BLOOD_BEATEN],
    none: &[EVE_POLIS_BOUND],
    then: &[Op::RedirectWarps(WarpRedirect {
        from: (WORLD, PLAINS),
        to: WORLD,
        cell: Some(ULTRASAURUS_CELL),
    })],
    otherwise: &[],
}];

/// The Ultrasaurus's cell on the world map.
const ULTRASAURUS_CELL: (usize, usize) = (0xC, 0x10);
/// The world map for the defense (ROM `0x0866A6B8`): the Ultrasaurus, and
/// Van's Zoids off the map.
const DEFENSE_OBJECTS: u32 = 0x0866_A6B8;
/// The world map for the cannon's strike (ROM `0x0866B164`): Van's Zoids,
/// the Blockade's field off the map, the Ultrasaurus, the Death Stinger
/// and its beam off the map.
const STRIKE_OBJECTS: u32 = 0x0866_B164;
const BLOCKADE: usize = 4;
const STINGER_BEAM: usize = 7;
/// The world map with the Death Stinger's wreck (ROM `0x0866B204`): the
/// wreck, and Opis's and Raven's Zoids off the map.
const WRECK_OBJECTS: u32 = 0x0866_B204;
const WRECK: usize = 1;
const WRECK_OPIS: usize = 2;
const WRECK_RAVEN: usize = 3;
/// The world map with Hermann (ROM `0x0866A71C`).
const HERMANN_OBJECTS: u32 = 0x0866_A71C;
const HERMANN: usize = 2;

const FIRST_ZOID_DRIVES_OUT: &[Op] = &[
    Op::Place(2, (0xE, 0x10)),
    glide(2, (0x100, 0x200), 2, true),
    Op::End,
];
const SECOND_ZOID_DRIVES_OUT: &[Op] = &[
    Op::Place(3, (0xE, 0x10)),
    glide(3, (0x100, 0x200), 2, true),
    Op::End,
];

/// The defense (task at `0x08023E78`): Van's Zoids drive out of the
/// Ultrasaurus, then the party's, and Jack gives the orders (`0x256`).
const DEFENSE_BRIEFING: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Spawn(MAP_TASK + 1, FIRST_ZOID_DRIVES_OUT),
    Op::Wait(32),
    Op::Spawn(MAP_TASK + 2, SECOND_ZOID_DRIVES_OUT),
    Op::Wait(32),
    Op::Place(4, (0xE, 0x10)),
    glide(4, (0x100, 0x200), 1, true),
    Op::Place(PLAYER, (0xE, 0x10)),
    glide(PLAYER, (0x160, 0x200), 1, false),
    Op::Dialogue(0x256),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, &warp_to(WORLD, ULTRASAURUS_CELL)),
    Op::End,
];

/// Back aboard (task at `0x080210F4` and the hook at `0x0802113C`): once
/// the step is over the screen darkens and the party is taken into the
/// hold, facing up.
const BACK_ABOARD: &[Op] = &[
    Op::AwaitArrival(PLAYER),
    Op::Control(false),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(
        FIELD_HOOK,
        &[
            Op::Face(PLAYER, Direction::Up),
            Op::Call(&warp_to(HOLD, (10, 0x16))),
        ],
    ),
    Op::End,
];

const VAN_TAKES_POSITION: &[Op] = &[
    glide(1, (0xC0, 0x1E0), 2, true),
    glide(1, (0x80, 0x1E0), 2, true),
    Op::Face(1, Direction::Up),
    Op::End,
];
const SECOND_ZOID_TAKES_POSITION: &[Op] = &[
    glide(2, (0xA0, 0x1A0), 2, true),
    glide(2, (0xA0, 0x1C0), 2, true),
    Op::Face(2, Direction::Left),
    Op::End,
];
const VIEW_GOES_DOWN: &[Op] = &[Op::Repeat(0x20, PAN_DOWN), Op::End];
const VAN_DRIVES_OFF: &[Op] = &[
    glide(1, (0x120, 0x1E0), 4, true),
    Op::Place(1, OFF_THE_MAP),
    Op::End,
];
const SECOND_ZOID_DRIVES_OFF_AGAIN: &[Op] = &[
    glide(2, (0x120, 0x1C0), 4, true),
    Op::Place(2, OFF_THE_MAP),
    Op::End,
];

/// The cannon's strike (task at `0x08023F94`): the Gravity Cannon's first
/// shot was stopped (`0x257`); Hiltz spares the party for now (`0x258`);
/// Van's Zoids take position, the Death Stinger fires and their Blockade
/// holds (`0x259`); they drive off, the view goes to the Ultrasaurus and
/// Hermann fires (`0x25A`); the hook at `0x08024190` stages the shot.
const CANNON_STRIKES: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Dialogue(0x257),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: WORLD,
        player: (4, 0xE),
        objects: STRIKE_OBJECTS,
        count: 8,
    },
    Op::RestartMusic(PEACE_MUSIC),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x258),
    Op::Spawn(MAP_TASK + 1, VAN_TAKES_POSITION),
    Op::Spawn(MAP_TASK + 2, SECOND_ZOID_TAKES_POSITION),
    Op::Spawn(MAP_TASK + 3, VIEW_GOES_DOWN),
    glide(3, (0xE0, 0x1A0), 1, true),
    glide(3, (0x60, 0x1A0), 1, true),
    glide(3, (0x60, 0x1C0), 1, true),
    Op::Face(3, Direction::Right),
    Op::Wait(15),
    Op::Place(STINGER_BEAM, (4, 0xF)),
    Op::Sound(BEAM_SOUND),
    Op::AwaitSoundEnd(BEAM_SOUND),
    Op::Place(BLOCKADE, (4, 0xF)),
    Op::PlayOnce(BLOCKADE, 0),
    Op::Sound(BLOCKADE_SOUND),
    Op::AwaitAnimation(BLOCKADE),
    Op::Wait(1),
    Op::AwaitSoundEnd(BLOCKADE_SOUND),
    Op::Dialogue(0x259),
    Op::Place(STINGER_BEAM, OFF_THE_MAP),
    Op::Spawn(MAP_TASK + 1, VAN_DRIVES_OFF),
    Op::Spawn(MAP_TASK + 2, SECOND_ZOID_DRIVES_OFF_AGAIN),
    glide(3, (0x60, 0x1A0), 1, true),
    glide(3, (0x120, 0x1A0), 2, true),
    Op::Place(3, OFF_THE_MAP),
    Op::Repeat(0x48, PAN_RIGHT_FAST),
    Op::Repeat(0x10, PAN_DOWN_FAST),
    Op::Dialogue(0x25A),
    Op::Spawn(FIELD_HOOK, CANNON_SHOT),
    Op::End,
];

/// The cannon's second shot (the hook at `0x08024190`, battle scene 14):
/// then the Death Stinger's wreck (task at `0x080241C8`).
const CANNON_SHOT: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Call(staged!(14)),
    Op::Control(false),
    Op::Spawn(MAP_TASK, STINGER_REVIVES),
    Op::End,
];

const WRECK_FLICKERS: [Op; 18] = flicker(WRECK, (4, 0xE));

/// The Death Stinger revives (task at `0x080241C8`): the party has held
/// (`0x25B`) and drives to the wreck (`0x25C`), which stirs (`0x25D`) and
/// is gone; Opis drives up and tells of Zoid Eve (`0x25E`) and leaves;
/// Regina tells of Eve Polis (`0x25F`); with Raven along, he and Reese go
/// after the Death Stinger (`0x260`) and leave the party.
const STINGER_REVIVES: &[Op] = &[
    Op::Place(PLAYER, ULTRASAURUS_CELL),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x25B),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: WORLD,
        player: (4, 0xE),
        objects: WRECK_OBJECTS,
        count: 4,
    },
    Op::Place(PLAYER, (9, 0xE)),
    Op::Call(SCENE_BRIGHTEN),
    glide(PLAYER, (0xA0, 0x1C0), 1, false),
    Op::Dialogue(0x25C),
    Op::Sound(REVIVAL_SOUND),
    Op::AwaitSoundEnd(REVIVAL_SOUND),
    Op::RestartMusic(DANGER_MUSIC),
    Op::Dialogue(0x25D),
    Op::Call(&WRECK_FLICKERS),
    Op::Place(WRECK_OPIS, (0, 0xE)),
    glide(WRECK_OPIS, (0x60, 0x1C0), 1, true),
    Op::Dialogue(0x25E),
    glide(WRECK_OPIS, (0, 0x1C0), 1, true),
    Op::Place(WRECK_OPIS, OFF_THE_MAP),
    Op::Dialogue(0x25F),
    Op::IfFlags {
        all: &[RAVEN_JOINED],
        none: &[],
        then: &[
            Op::Dialogue(0x260),
            Op::Place(WRECK_RAVEN, (5, 0xE)),
            glide(WRECK_RAVEN, (0xA0, 0x1A0), 1, true),
            glide(WRECK_RAVEN, (0x80, 0x1A0), 1, true),
            Op::Wait(1),
            glide(WRECK_RAVEN, (0, 0x1A0), 2, true),
            Op::Place(WRECK_RAVEN, OFF_THE_MAP),
            Op::Leave(RAVEN_LISTS[0]),
            Op::Leave(RAVEN_LISTS[1]),
        ],
        otherwise: &[],
    },
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, &warp_to(WORLD, (5, 0xE))),
    Op::End,
];

/// The defense's watch (the hook at `0x08021050`, run every frame; the
/// tasks it starts clear it): once more than two roaming battles are won,
/// the cannon strikes; stepping onto the Ultrasaurus's cell before, the
/// party goes back aboard.
const DEFENSE_WATCH: &[Op] = &[Op::Loop(&[
    Op::IfBattlesWon {
        more_than: 2,
        then: &[
            Op::Calm(true),
            Op::Control(false),
            Op::Spawn(MAP_TASK, CANNON_STRIKES),
            Op::Flag(STINGER_STRUCK, true),
            Op::Flag(STRIKE_NOTED, true),
            Op::End,
        ],
        otherwise: &[Op::IfPlayerWalking {
            then: &[Op::IfPlayer {
                columns: Some((ULTRASAURUS_CELL.0, ULTRASAURUS_CELL.0)),
                rows: Some((ULTRASAURUS_CELL.1, ULTRASAURUS_CELL.1)),
                then: &[
                    Op::Calm(true),
                    Op::Control(false),
                    Op::Spawn(MAP_TASK, BACK_ABOARD),
                    Op::End,
                ],
                otherwise: &[],
            }],
            otherwise: &[],
        }],
    },
    Op::Wait(1),
])];

/// The plains on the way to Eve Polis (ROM `0x0866B254`): Hermann's Zoid.
const TO_EVE_POLIS_OBJECTS: u32 = 0x0866_B254;

const HERMANN_DRIVES_OFF: &[Op] = &[glide(HERMANN, (0x180, 0x2C0), 2, true), Op::End];

/// To Eve Polis (task at `0x08024488`): the party drives off as Hermann's
/// Zoid leaves; on the plains Hermann's Zoid leads the way north.
const TO_EVE_POLIS: &[Op] = &[
    glide(PLAYER, (0x180, 0x200), 1, false),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Wait(15),
    Op::Spawn(MAP_TASK + 1, HERMANN_DRIVES_OFF),
    Op::Wait(32),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::Wait(1),
    Op::LoadMap {
        map: PLAINS,
        player: (0xD, 0xB),
        objects: TO_EVE_POLIS_OBJECTS,
        count: 2,
    },
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Spawn(MAP_TASK + 1, BRIGHTENS),
    glide(1, (0x1A0, 0x140), 1, true),
    Op::Place(PLAYER, (0xD, 10)),
    glide(PLAYER, (0x1A0, 0x160), 1, false),
    Op::Wait(15),
    glide(1, (0x1A0, 0xE0), 1, true),
    Op::Call(&back_to(PLAINS, (0xD, 0xB))),
];

/// Hermann spoken to (`0x08021168`): the Ultrasaurus cannot move, Van went
/// after the Death Stinger, and the party is sent to Eve Polis (`0x261`).
const HERMANN_TALK: &[Op] = &[
    Op::Dialogue(0x261),
    Op::Control(false),
    Op::Spawn(MAP_TASK, TO_EVE_POLIS),
    Op::ReturnPoint(WORLD_RETURN_POINT),
    Op::Flag(EVE_POLIS_BOUND, true),
    Op::Flag(STRIKE_NOTED, false),
];

/// The world map (`0x08020F24`): once Blood is beaten, the defense's
/// briefing the first time; then the defense's watch, the count of
/// battles starting again; once the cannon strikes, Hermann waits. The
/// Ultrasaurus stands through the defense, Hermann until he sends the
/// party on.
const WORLD_ARRIVAL: &[Op] = &[
    Op::IfFlags {
        all: &[BLOOD_BEATEN],
        none: &[DEFENSE_BRIEFED],
        then: &[
            Op::LoadMap {
                map: WORLD,
                player: (0xD, 0x10),
                objects: DEFENSE_OBJECTS,
                count: 5,
            },
            Op::Place(PLAYER, OFF_THE_MAP),
            Op::Control(false),
            Op::Spawn(MAP_TASK, DEFENSE_BRIEFING),
            Op::Flag(DEFENSE_BRIEFED, true),
        ],
        otherwise: &[Op::IfFlags {
            all: &[DEFENSE_BRIEFED],
            none: &[STINGER_STRUCK],
            then: &[Op::ForgetBattlesWon, Op::Spawn(FIELD_WATCH, DEFENSE_WATCH)],
            otherwise: &[Op::IfFlags {
                all: &[STINGER_STRUCK],
                none: &[EVE_POLIS_BOUND],
                then: &[Op::LoadMap {
                    map: WORLD,
                    player: HERE,
                    objects: HERMANN_OBJECTS,
                    count: 4,
                }],
                otherwise: &[],
            }],
        }],
    },
    Op::IfFlags {
        all: &[DEFENSE_BRIEFED],
        none: &[EVE_POLIS_BOUND],
        then: &[],
        otherwise: &[Op::Place(1, OFF_THE_MAP)],
    },
    Op::IfFlags {
        all: &[STINGER_STRUCK],
        none: &[EVE_POLIS_BOUND],
        then: &[],
        otherwise: &[Op::Place(HERMANN, OFF_THE_MAP)],
    },
];

/// Eve Polis (ROM `0x0866A890`): the prince, Regina, Jack and Earth off
/// the map, Opis's Zoid, the Death Stinger, the Death Saurer, four
/// soldiers, and Van's and Raven's Zoids off the map.
const EVE_POLIS_OBJECTS: u32 = 0x0866_A890;
const EVE_PRINCE: usize = 1;
const EVE_REGINA: usize = 2;
const EVE_JACK: usize = 3;
const EVE_EARTH: usize = 4;
const EVE_OPIS: usize = 5;
const EVE_STINGER: usize = 6;
const EVE_VAN: usize = 12;
const EVE_RAVEN: usize = 13;
const EVE_STINGER_FLICKERS: [Op; 18] = flicker(EVE_STINGER, (5, 4));

const fn arrives_at_eve_polis(actor: usize, to: (i32, i32)) -> [Op; 3] {
    [
        Op::Place(actor, (0xE, 4)),
        glide(actor, to, 1, true),
        Op::End,
    ]
}
const PRINCE_ARRIVES: &[Op] = &arrives_at_eve_polis(EVE_PRINCE, (0x180, 0x80));
const REGINA_ARRIVES: &[Op] = &arrives_at_eve_polis(EVE_REGINA, (0x1A0, 0x80));
const JACK_ARRIVES: &[Op] = &[
    Op::Place(EVE_JACK, (0xE, 4)),
    glide(EVE_JACK, (0x1C0, 0x60), 1, true),
    glide(EVE_JACK, (0x180, 0x60), 1, true),
    Op::End,
];
const fn drives_to(actor: usize, to: (i32, i32)) -> [Op; 2] {
    [glide(actor, to, 2, true), Op::End]
}
const PRINCE_STEPS_FORWARD: &[Op] = &drives_to(EVE_PRINCE, (0x140, 0x80));
const JACK_STEPS_FORWARD: &[Op] = &drives_to(EVE_JACK, (0x140, 0x60));
const REGINA_STEPS_FORWARD: &[Op] = &drives_to(EVE_REGINA, (0x160, 0x80));
const fn soldier_closes_in(actor: usize, to: (i32, i32), late: bool) -> [Op; 4] {
    [
        Op::Wait(if late { 32 } else { 0 }),
        glide(actor, to, 2, true),
        Op::Face(actor, Direction::Right),
        Op::End,
    ]
}
const FIRST_SOLDIER_CLOSES_IN: &[Op] = &soldier_closes_in(8, (0xE0, 0x60), true);
const SECOND_SOLDIER_CLOSES_IN: &[Op] = &soldier_closes_in(9, (0x100, 0x60), true);
const THIRD_SOLDIER_CLOSES_IN: &[Op] = &soldier_closes_in(10, (0xE0, 0x80), false);

/// Eve Polis (task at `0x080245B4`): the party drives in and Opis
/// greets it (`0x262`); the Death Stinger comes to the Death Saurer and
/// merges with it (`0x263`); the true Death Saurer rises (`0x264`) and
/// Opis drives off; the party stands firm (`0x265`) and the soldiers
/// close in; the hook at `0x080247B0` starts their fight.
const EVE_POLIS_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Spawn(MAP_TASK + 1, PRINCE_ARRIVES),
    Op::Spawn(MAP_TASK + 2, JACK_ARRIVES),
    Op::Wait(32),
    Op::Spawn(MAP_TASK + 3, REGINA_ARRIVES),
    Op::Place(EVE_EARTH, (0xE, 4)),
    glide(EVE_EARTH, (0x1C0, 0x60), 1, true),
    glide(EVE_EARTH, (0x1A0, 0x60), 1, true),
    Op::Dialogue(0x262),
    Op::Repeat(0x30, PAN_LEFT_FAST),
    glide(EVE_STINGER, (0xC0, 0x80), 1, true),
    Op::Wait(15),
    Op::Face(EVE_STINGER, Direction::Left),
    Op::Wait(15),
    glide(EVE_STINGER, (0xA0, 0x80), 1, true),
    Op::Call(&EVE_STINGER_FLICKERS),
    Op::Dialogue(0x263),
    Op::Repeat(0x30, PAN_RIGHT_FAST),
    Op::Dialogue(0x264),
    glide(EVE_OPIS, (0x120, 0x100), 1, true),
    Op::Place(EVE_OPIS, OFF_THE_MAP),
    Op::Spawn(MAP_TASK + 1, PRINCE_STEPS_FORWARD),
    Op::Repeat(0x40, PAN_LEFT),
    Op::Dialogue(0x265),
    Op::Spawn(MAP_TASK + 1, JACK_STEPS_FORWARD),
    Op::Spawn(MAP_TASK + 2, REGINA_STEPS_FORWARD),
    glide(EVE_EARTH, (0x160, 0x60), 2, true),
    Op::Wait(1),
    Op::Spawn(MAP_TASK + 1, FIRST_SOLDIER_CLOSES_IN),
    Op::Spawn(MAP_TASK + 2, SECOND_SOLDIER_CLOSES_IN),
    Op::Spawn(MAP_TASK + 3, THIRD_SOLDIER_CLOSES_IN),
    glide(11, (0x100, 0x80), 2, true),
    Op::Face(11, Direction::Right),
    Op::Spawn(FIELD_HOOK, SOLDIERS_FIGHT),
    Op::End,
];

/// The soldiers' fight (story battle 33).
const SOLDIERS_BATTLE_OPS: [Op; 4] = story_battle(33);
/// The soldiers' battle (the hook at `0x080247B0`): won, the Death Saurer
/// comes on (task at `0x08024820`).
const SOLDIERS_FIGHT: &[Op] = &[
    Op::Call(&SOLDIERS_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::RestartMusic(FINAL_MUSIC),
            Op::Control(false),
            Op::Spawn(MAP_TASK, SAURER_COMES_ON),
        ],
    },
    Op::End,
];

const PRINCE_FALLS_BACK: &[Op] = &drives_to(EVE_PRINCE, (0x100, 0x80));
const REGINA_FALLS_BACK: &[Op] = &drives_to(EVE_REGINA, (0x120, 0x80));
const JACK_FALLS_BACK: &[Op] = &drives_to(EVE_JACK, (0x100, 0x60));
const EARTH_FALLS_BACK: &[Op] = &drives_to(EVE_EARTH, (0x120, 0x60));
const JACK_CLOSES_IN: &[Op] = &drives_to(EVE_JACK, (0xC0, 0x60));
const EARTH_CLOSES_IN: &[Op] = &drives_to(EVE_EARTH, (0xE0, 0x60));

/// The Death Saurer comes on (task at `0x08024820`): the soldiers are gone
/// and the party falls back before it (`0x266`); Van drives up with a plan
/// (`0x267`) and off to the Gravity Cannon (`0x268`); the party trusts him
/// (`0x269`) and closes in; the hook at `0x08024974` starts the fight.
const SAURER_COMES_ON: &[Op] = &[
    Op::Place(8, OFF_THE_MAP),
    Op::Place(9, OFF_THE_MAP),
    Op::Place(10, OFF_THE_MAP),
    Op::Place(11, OFF_THE_MAP),
    Op::Place(EVE_VAN, (6, 4)),
    Op::Place(EVE_RAVEN, (5, 3)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x266),
    Op::Spawn(MAP_TASK + 1, PRINCE_FALLS_BACK),
    Op::Spawn(MAP_TASK + 2, REGINA_FALLS_BACK),
    Op::Spawn(MAP_TASK + 3, JACK_FALLS_BACK),
    Op::Spawn(MAP_TASK + 4, EARTH_FALLS_BACK),
    Op::Repeat(0x80, PAN_LEFT),
    glide(EVE_VAN, (0xE0, 0x80), 1, true),
    Op::Dialogue(0x267),
    glide(EVE_VAN, (0xE0, 0xA0), 4, true),
    glide(EVE_VAN, (0x160, 0xA0), 4, true),
    Op::Place(EVE_VAN, OFF_THE_MAP),
    Op::Dialogue(0x268),
    glide(EVE_PRINCE, (0xC0, 0x80), 2, true),
    Op::Dialogue(0x269),
    Op::Spawn(MAP_TASK + 1, JACK_CLOSES_IN),
    Op::Spawn(MAP_TASK + 2, EARTH_CLOSES_IN),
    glide(EVE_REGINA, (0xE0, 0x80), 2, true),
    Op::Spawn(FIELD_HOOK, SAURER_FIGHT),
    Op::End,
];

/// The Death Saurer's fight (story battle 34).
const SAURER_BATTLE_OPS: [Op; 4] = story_battle(34);
/// The Death Saurer's battle (the hook at `0x08024974`): won, Hiltz
/// charges his cannon (task at `0x080249E4`).
const SAURER_FIGHT: &[Op] = &[
    Op::Call(&SAURER_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::FadeInHolding,
            Op::Control(false),
            Op::Spawn(MAP_TASK, CANNON_CHARGES),
            Op::Flag(SAURER_BEATEN, true),
        ],
    },
    Op::End,
];

/// The plains with the Ultrasaurus (ROM `0x0866B27C`).
const CHARGE_OBJECTS: u32 = 0x0866_B27C;

/// The last charge (task at `0x080249E4`): Hiltz charges to end it all and
/// the party holds him (`0x26A`); on the Ultrasaurus Schwarz readies the
/// Gravity Cannon (`0x26B`); the hook at `0x08024A5C` stages the shots.
const CANNON_CHARGES: &[Op] = &[
    Op::Dialogue(0x26A),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PLAINS,
        player: (3, 0xD),
        objects: CHARGE_OBJECTS,
        count: 1,
    },
    Op::RestartMusic(PEACE_MUSIC),
    Op::Place(PLAYER, (5, 0xD)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x26B),
    Op::Spawn(FIELD_HOOK, LAST_SHOTS),
    Op::End,
];

/// The last shots (the hook at `0x08024A5C`, battle scenes 15 to 18 from
/// the list at ROM `0x0866A6B0`): then the Death Saurer falls (task at
/// `0x08024A98`).
const LAST_SHOTS: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Call(staged!(15, 16, 17, 18)),
    Op::Control(false),
    Op::Spawn(MAP_TASK, SAURER_FALLS),
    Op::End,
];

/// Eve Polis after (ROM `0x0866B290`): Regina, Jack and Earth, the Death
/// Saurer, Raven's Zoid and an explosion.
const FALL_OBJECTS: u32 = 0x0866_B290;
const FALL_EXPLOSION: usize = 6;
const SAURER_CELL: (usize, usize) = (5, 4);
/// Eve Polis on foot (ROM `0x0866B31C`): the party, Van and his friends,
/// all off the map.
const VICTORY_OBJECTS: u32 = 0x0866_B31C;

/// The Death Saurer's blasts (task at `0x080250C8`).
const SAURER_BLASTS: &[Op] = &[Op::Loop(&[
    Op::Call(&blast(FALL_EXPLOSION, SAURER_CELL)),
    Op::Wait(10),
    Op::Call(&blast(FALL_EXPLOSION, SAURER_CELL)),
    Op::Wait(4),
    Op::Call(&blast(FALL_EXPLOSION, SAURER_CELL)),
    Op::Wait(4),
])];

const fn walks_up(
    actor: usize,
    from: (usize, usize),
    to: (i32, i32),
    facing: Direction,
) -> [Op; 4] {
    [
        Op::Place(actor, from),
        glide16(actor, to, 1, actor != PLAYER),
        Op::Face(actor, facing),
        Op::End,
    ]
}
const fn walks_in(actor: usize, from: (usize, usize), to: (i32, i32)) -> [Op; 3] {
    [Op::Place(actor, from), glide16(actor, to, 1, true), Op::End]
}
const fn walks_off(actor: usize, y: i32) -> [Op; 3] {
    [
        glide16(actor, (0xE0, y), 2, true),
        Op::Place(actor, OFF_THE_MAP),
        Op::End,
    ]
}
const VAN_WALKS_UP: &[Op] = &walks_up(4, (0xD, 6), (0x50, 0x60), Direction::Down);
const FIFTH_WALKS_UP: &[Op] = &walks_up(5, (0xE, 6), (0x60, 0x60), Direction::Down);
const PRINCE_WALKS_UP: &[Op] = &walks_up(PLAYER, (0xD, 8), (0x50, 0x80), Direction::Up);
const REGINA_WALKS_UP_TO_VAN: &[Op] = &walks_up(REGINA, (0xE, 8), (0x60, 0x80), Direction::Up);
const JACK_WALKS_UP_TO_VAN: &[Op] = &walks_up(JACK, (0xF, 8), (0x70, 0x80), Direction::Up);
const SEVENTH_WALKS_IN: &[Op] = &walks_in(7, (0xD, 7), (0x90, 0x70));
const EIGHTH_WALKS_IN: &[Op] = &walks_in(8, (0xE, 7), (0xA0, 0x70));
const VAN_WALKS_OFF: &[Op] = &walks_off(4, 0x60);
const FIFTH_WALKS_OFF: &[Op] = &walks_off(5, 0x60);
const SIXTH_WALKS_OFF: &[Op] = &walks_off(6, 0x60);
const SEVENTH_WALKS_OFF: &[Op] = &walks_off(7, 0x70);
const EIGHTH_WALKS_OFF: &[Op] = &walks_off(8, 0x70);

/// The Death Saurer falls (task at `0x08024A98`): it blows up and the party
/// cannot believe it won (`0x26C`); on foot Van and his friends meet the
/// party (`0x26D`) and leave; the party is back on the plains.
const SAURER_FALLS: &[Op] = &[
    Op::LoadMap {
        map: EVE_POLIS,
        player: (5, 4),
        objects: FALL_OBJECTS,
        count: 7,
    },
    Op::RestartMusic(FINAL_MUSIC),
    Op::Place(PLAYER, (6, 4)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, SAURER_BLASTS),
    Op::Dialogue(0x26C),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::LoadMap {
        map: EVE_POLIS,
        player: (3, 4),
        objects: VICTORY_OBJECTS,
        count: 10,
    },
    Op::CellTiles(2),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, VAN_WALKS_UP),
    Op::Spawn(MAP_TASK + 2, FIFTH_WALKS_UP),
    Op::Place(6, (0xF, 6)),
    glide16(6, (0x70, 0x60), 1, true),
    Op::Face(6, Direction::Down),
    Op::Wait(1),
    Op::Spawn(MAP_TASK + 1, PRINCE_WALKS_UP),
    Op::Spawn(MAP_TASK + 2, REGINA_WALKS_UP_TO_VAN),
    Op::Spawn(MAP_TASK + 3, JACK_WALKS_UP_TO_VAN),
    Op::Place(EARTH, (0x10, 8)),
    glide16(EARTH, (0x80, 0x80), 1, true),
    Op::Face(EARTH, Direction::Up),
    Op::Wait(1),
    Op::Spawn(MAP_TASK + 1, SEVENTH_WALKS_IN),
    Op::Spawn(MAP_TASK + 2, EIGHTH_WALKS_IN),
    Op::Place(9, (0xF, 7)),
    glide16(9, (0xB0, 0x70), 1, true),
    Op::Wait(1),
    Op::Dialogue(0x26D),
    Op::Spawn(MAP_TASK + 1, VAN_WALKS_OFF),
    Op::Spawn(MAP_TASK + 2, FIFTH_WALKS_OFF),
    Op::Spawn(MAP_TASK + 3, SIXTH_WALKS_OFF),
    Op::Spawn(MAP_TASK + 4, SEVENTH_WALKS_OFF),
    Op::Spawn(MAP_TASK + 5, EIGHTH_WALKS_OFF),
    glide16(9, (0xE0, 0x70), 2, true),
    Op::Place(9, OFF_THE_MAP),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, &warp_to(PLAINS, (3, 0xD))),
    Op::End,
];

/// Eve Polis (`0x08021FEC`): while the party is sent there, the scene.
const EVE_POLIS_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[EVE_POLIS_BOUND],
    none: &[SAURER_BEATEN],
    then: &[
        Op::LoadMap {
            map: EVE_POLIS,
            player: (0xE, 4),
            objects: EVE_POLIS_OBJECTS,
            count: 14,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, EVE_POLIS_SCENE),
    ],
    otherwise: &[],
}];

/// Where the port's story stops: chapter 9's first map once the Death
/// Saurer is beaten.
pub(super) const STORY_END: (usize, u16) = (CHAPTER_9_START, SAURER_BEATEN);

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

/// The teachers of area 8: deck commands 7 (`0x08006D04`), 15
/// (`0x08006D1C`) and 17 (`0x0800954C` through `0x08009430`).
const TEACHERS: [(u32, &[Op]); 3] = [
    (0x0800_6D04, teacher!(7, 0x3E3, 0x3F7)),
    (0x0800_6D1C, teacher!(0xF, 0x3E4, 0x3F8)),
    (0x0800_954C, teacher!(0x11, 0x3AB, 0x3AC)),
];

/// The keepers of area 8 (`0x08009360` on): item shops 15 to 17,
/// armaments shops 22 and 23, and the labs 14 to 16 and 21.
const SHOPS: [(u32, &[Op]); 9] = [
    (0x0800_9360, &shop(Shop::Items(0xF))),
    (0x0800_936C, &shop(Shop::Items(0x10))),
    (0x0800_9378, &shop(Shop::Items(0x11))),
    (0x0800_9384, &shop(Shop::Arms(0x16))),
    (0x0800_9390, &shop(Shop::Arms(0x17))),
    (0x0800_939C, &shop(Shop::Lab(0xE))),
    (0x0800_93A8, &shop(Shop::Lab(0xF))),
    (0x0800_93B4, &shop(Shop::Lab(0x10))),
    (0x0800_93C0, &shop(Shop::Lab(0x15))),
];

/// The objects of area 8 whose script is code: the Death Stinger, Hermann
/// and the six Planetal Sites.
const OBJECTS: [(u32, &[Op]); 8] = [
    (0x0802_14B4, DEATH_STINGER_TALK),
    (0x0802_1168, HERMANN_TALK),
    (0x0802_1B00, site!(SITES[0], ISLAND_SITE_FOUND)),
    (0x0802_1B4C, site!(SITES[1], ISLAND_SITE_FOUND)),
    (0x0802_1B9C, site!(SITES[2], ISLAND_SITE_FOUND)),
    (0x0802_1BE8, site!(SITES[3], WESTERN_SITE_FOUND)),
    (0x0802_1C38, site!(SITES[4], WESTERN_SITE_FOUND)),
    (0x0802_1C84, site!(SITES[5], WESTERN_SITE_FOUND)),
];

/// What speaking to an object of area 8 whose script is code runs.
pub(super) fn talk_handler(address: u32) -> Option<&'static [Op]> {
    TEACHERS
        .iter()
        .chain(SHOPS.iter())
        .chain(OBJECTS.iter())
        .find(|(at, _)| *at == address)
        .map(|(_, program)| *program)
}

/// What map `map` of area 8 runs when it loads, when it runs anything.
pub(super) fn map_handler(map: usize) -> Option<&'static [Op]> {
    match map {
        THRONE_ROOM => Some(THRONE_ROOM_ARRIVAL),
        WORLD => Some(WORLD_ARRIVAL),
        PLAINS => Some(PLAINS_ARRIVAL),
        CITY => Some(CITY_ARRIVAL),
        HOLD => Some(HOLD_ARRIVAL),
        DIG | DIG_AFTER => Some(DIG_ARRIVAL),
        ISLAND_RUINS => Some(ISLAND_RUINS_ARRIVAL),
        ISLAND_SECOND_ROOM => Some(ISLAND_SECOND_ARRIVAL),
        ISLAND_THIRD_ROOM => Some(ISLAND_THIRD_ARRIVAL),
        WESTERN_RUINS => Some(WESTERN_RUINS_ARRIVAL),
        WESTERN_SECOND_ROOM => Some(WESTERN_SECOND_ARRIVAL),
        WESTERN_THIRD_ROOM => Some(WESTERN_THIRD_ARRIVAL),
        EVE_POLIS => Some(EVE_POLIS_ARRIVAL),
        _ => None,
    }
}
