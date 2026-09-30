//! Chapter 7: Fran flees through the space-time transfer device and the
//! party follows her to a land where the Berserk Führer roams, with
//! Alster, Blue Gem, Parti and Solid (area 7).
//!
//! Source of knowledge: own reading of Zoids Saga (Japan, Rev 1): the map
//! handlers of area 7 (`0x0801EBB4` to `0x0801EFFC`), the tasks and field
//! hooks they install and the objects' code, named at each item. See
//! `docs/events.md`.

use extraction::saga::Reward;

use super::{
    FIELD_HOOK, PLAYER, SCENE_BRIGHTEN, SCENE_DARKEN, ZI_DATA_REWARD, glide, learn, shop,
    story_battle, through,
};
use crate::event::{FIELD_WATCH, MAP_TASK, Op};
use crate::field::{Direction, PIXEL};
use crate::menu::Shop;

/// Set once the Emperor has sent Blood after Fran (`0x0801EF1C`).
const CHAPTER_OPENED: u16 = 0x1AB;
/// Set once the portal has brought the party to the forest.
const ARRIVED: u16 = 0x1AC;
/// Set once Blue Gem has seen to the Liger Zero and Solid has taken the
/// party on.
const SOLID_MET: u16 = 0x1AD;
/// Set once the party has reached Solid's camp.
const CAMP_REACHED: u16 = 0x1AE;
/// Set once Fran has been found in the camp's command room.
const FRAN_FOUND: u16 = 0x1AF;
/// Set once the Berserk Führer is beaten (story battle 29).
const FUHRER_BEATEN: u16 = 0x1B0;
/// Read by the village before its farewell; nothing in the game sets it.
const FAREWELL_SEEN: u16 = 0x1B1;

/// The forest (217), a Zoid map with the portal; the village (218); Blue
/// Gem's house (220); Solid's camp (221) and its command room (222); the
/// old base where the Führer nests (224).
const FOREST: usize = 217;
const VILLAGE: usize = 218;
const BLUE_GEM_HOUSE: usize = 220;
const CAMP: usize = 221;
const COMMAND_ROOM: usize = 222;
const NEST: usize = 224;
/// The Emperor's throne room (225), where chapter 6's end leaves the
/// party; the castle's portal room (228) and the room above the base's bar
/// (234) in the kingdom's time.
const THRONE_ROOM: usize = 225;
const PORTAL_ROOM: usize = 228;
const BASE_ROOM: usize = 234;
/// Chapter 8's first map, where the chapter's end leaves the party.
const CHAPTER_8_START: usize = 262;

/// The songs the chapter switches to.
const PEACE_MUSIC: u16 = 1;
const DANGER_MUSIC: u16 = 4;
const SOLEMN_MUSIC: u16 = 6;
const VICTORY_MUSIC: u16 = 7;
const FRIENDS_MUSIC: u16 = 8;
const ENEMY_MUSIC: u16 = 9;
const RIVAL_MUSIC: u16 = 0x16;
/// The song the staged battle scenes play to (`0x08012040`).
const STAGED_MUSIC: u16 = 0x17;

const PORTAL_SOUND: u16 = 0x6F;
const PORTAL_ARRIVAL_SOUND: u16 = 0x49;
const BLAST_SOUND: u16 = 0x5B;
const CRASH_SOUND: u16 = 0x5C;
const ALARM_SOUND: u16 = 0x65;
const PORTAL_IDLE: usize = 0;
const PORTAL_OPENS: usize = 2;
const PORTAL_BRINGS_STEP: usize = 32;

/// Frames a task waits after the fade in before its first line.
const SETTLE: u32 = 32;
const OFF_THE_MAP: (usize, usize) = (0xFF, 0xFF);
const HIDDEN: (usize, usize) = (0xFFFF, 0xFFFF);
/// The prince standing again (`0x080089A0`).
const PRINCE_STANDING: usize = 0x98;
/// The characters of group 6, met as the chapter opens (`0x080099FC`).
const CHAPTER_GROUP: u8 = 6;
/// The lists Alster and Parti join with.
const ALSTER_LISTS: [u8; 2] = [0x1A, 0x1B];
/// The Zi data Blue Gem gives (`0x08037A24`): the Trinity Liger's
/// upgrade and three of his own.
const BLUE_GEM_GIFTS: [u8; 4] = [0x90, 0x43, 0x44, 0x45];
/// The return point the Führer's loss takes the party to: Blue Gem's
/// house.
const BLUE_GEM_RETURN_POINT: u8 = 0x11;

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

/// A story battle's hook: beaten, the party is taken to its return point
/// and the field brightens at once.
const BEATEN: &[Op] = &[Op::WarpHome, Op::FadeInHolding];

/// Scrolls the view a pixel a frame (`0x08008324` in a loop).
const PAN_DOWN: &[Op] = &[Op::Pan(0, PIXEL), Op::Wait(1)];

/// The throne room for the opening (ROM `0x0866A124`): the Emperor, Gale,
/// Opis and Blood, and a soldier below.
const THRONE_OBJECTS: u32 = 0x0866_A124;
const GALE: usize = 2;
const OPIS: usize = 3;
const BLOOD: usize = 4;
const MESSENGER: usize = 5;
/// The portal room (ROM `0x0866A19C`): three soldiers.
const PORTAL_ROOM_OBJECTS: u32 = 0x0866_A19C;
/// The room above the base's bar (ROM `0x0866A1EC`): the prince seated,
/// Regina, and Jack and Earth off the map.
const BASE_OBJECTS: u32 = 0x0866_A1EC;
const REGINA: usize = 1;
const JACK: usize = 2;
const EARTH: usize = 3;

const SOLDIER_RUNS_OFF: &[Op] = &[
    glide(1, (0x80, 0x60), 2, true),
    glide(1, (0x80, 0xE0), 2, true),
    Op::End,
];
const JACK_COMES_IN: &[Op] = &[
    Op::Place(JACK, (0xE, 2)),
    glide16(JACK, (0x30, 0x20), 1, true),
    glide16(JACK, (0x30, 0x30), 1, true),
    Op::Face(JACK, Direction::Down),
    Op::End,
];

/// The chapter's opening (task at `0x0801F008`): Gale reports on the Death
/// Saurer and Opis on its weak spot (`0x1E6`); a soldier reports that
/// Fran has used the device and the Emperor sends Blood after her
/// (`0x1E7`); Opis rages (`0x1E8`); in the portal room the soldiers
/// sense the device in use twice (`0x1E9`); in the room above the bar Jack
/// brings the news (`0x1EA`) and the party leaves.
const OPENING: &[Op] = &[
    Op::Wait(SETTLE),
    glide16(GALE, (0x70, 0x30), 1, true),
    Op::Dialogue(0x1E6),
    glide16(OPIS, (0x80, 0x50), 1, true),
    glide16(OPIS, (0x70, 0x50), 1, true),
    Op::Face(OPIS, Direction::Right),
    Op::Face(GALE, Direction::Right),
    Op::Face(BLOOD, Direction::Left),
    glide16(MESSENGER, (0x80, 0x40), 1, true),
    Op::Dialogue(0x1E7),
    Op::Face(OPIS, Direction::Up),
    Op::Face(GALE, Direction::Up),
    Op::Face(BLOOD, Direction::Up),
    glide16(BLOOD, (0x90, 0xC0), 1, true),
    Op::Place(BLOOD, HIDDEN),
    Op::Dialogue(0x1E8),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: PORTAL_ROOM,
        player: (4, 3),
        objects: PORTAL_ROOM_OBJECTS,
        count: 4,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x1E9),
    Op::Spawn(MAP_TASK + 1, SOLDIER_RUNS_OFF),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
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
    Op::Dialogue(0x1EA),
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

/// The throne room (`0x0801EF1C`): the first time, it marks group 6 as met
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
            count: 6,
        },
        Op::Place(PLAYER, HIDDEN),
        Op::Control(false),
        Op::Spawn(MAP_TASK, OPENING),
        Op::Flag(CHAPTER_OPENED, true),
    ],
    otherwise: &[],
}];

/// The forest with the portal (ROM `0x08669EF4`): the Berserk Führer and
/// Alster's Liger Zero off the map, and the portal.
const ARRIVAL_OBJECTS: u32 = 0x0866_9EF4;
const FUHRER: usize = 1;
const LIGER_ZERO: usize = 2;
const FOREST_PORTAL: usize = 3;
const FOREST_PORTAL_CELL: (usize, usize) = (7, 1);
const FOREST_ARRIVAL_CELL: (usize, usize) = (7, 2);

/// Into the forest (task at `0x0801F2CC`): the portal brings the party,
/// which wonders where it is (`0x1EB`); the Berserk Führer comes at it
/// (`0x1EC`) and Alster's Liger Zero cuts in (`0x1ED`); the hook at
/// `0x0801F47C` stages their fight.
const ARRIVAL: &[Op] = &[
    Op::Wait(SETTLE),
    Op::PlayOnce(FOREST_PORTAL, PORTAL_OPENS),
    Op::Sound(PORTAL_SOUND),
    Op::AwaitStepEnd(FOREST_PORTAL, PORTAL_BRINGS_STEP),
    Op::Show(PLAYER),
    Op::Place(PLAYER, FOREST_PORTAL_CELL),
    through(PLAYER, FOREST_ARRIVAL_CELL, PIXEL, 1),
    Op::Sound(PORTAL_ARRIVAL_SOUND),
    Op::AwaitAnimation(FOREST_PORTAL),
    Op::Animate(FOREST_PORTAL, PORTAL_IDLE),
    Op::Dialogue(0x1EB),
    Op::Wait(60),
    Op::Place(FUHRER, (2, 2)),
    glide(FUHRER, (0xE0, 0x60), 4, true),
    Op::Face(FUHRER, Direction::Up),
    Op::RestartMusic(DANGER_MUSIC),
    Op::Dialogue(0x1EC),
    Op::Place(LIGER_ZERO, (0xC, 2)),
    glide(LIGER_ZERO, (0x120, 0x40), 2, true),
    Op::Dialogue(0x1ED),
    Op::Wait(15),
    Op::Face(FUHRER, Direction::Right),
    Op::Wait(15),
    glide(FUHRER, (0x100, 0x40), 4, true),
    Op::Face(FUHRER, Direction::Right),
    Op::Wait(15),
    Op::Spawn(FIELD_HOOK, LIGER_ZERO_FIGHT),
    Op::End,
];

/// The Liger Zero's fight with the Führer (the hook at `0x0801F47C`):
/// battle scenes 8 and 9 to their song (`0x08012040` with the list at
/// ROM `0x08669EF0`), then the party meets Alster (task at `0x0801F4B8`).
const LIGER_ZERO_FIGHT: &[Op] = &[
    Op::Call(SCENE_DARKEN),
    Op::Music(STAGED_MUSIC),
    Op::Battle(8),
    Op::Battle(9),
    Op::Music(DANGER_MUSIC),
    Op::Control(false),
    Op::Spawn(MAP_TASK, ALSTER_MET),
    Op::End,
];

/// Alster (task at `0x0801F4B8`): the Führer is gone, the Liger Zero is
/// hurt (`0x1EE`); Alster meets the party and tells of the Führer
/// (`0x1EF`), and they drive to the village; the hook at `0x0801F5A0`
/// takes the party there.
const ALSTER_MET: &[Op] = &[
    Op::Call(SCENE_BRIGHTEN),
    glide(FUHRER, (0x100, 0xC0), 4, true),
    Op::Place(FUHRER, OFF_THE_MAP),
    Op::RestartMusic(FRIENDS_MUSIC),
    Op::Dialogue(0x1EE),
    glide(LIGER_ZERO, (0x100, 0x40), 1, false),
    glide(PLAYER, (0xE0, 0x40), 1, false),
    Op::Face(PLAYER, Direction::Right),
    Op::Dialogue(0x1EF),
    glide(LIGER_ZERO, (0xE0, 0x40), 1, false),
    Op::Place(LIGER_ZERO, OFF_THE_MAP),
    Op::Wait(30),
    glide(PLAYER, (0x100, 0x40), 1, true),
    glide(PLAYER, (0x100, 0x160), 1, true),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, &warp_to(VILLAGE, (0x16, 0x11))),
    Op::End,
];

/// Where the party is sent to the camp first (the hook at `0x0801EC58`,
/// run every frame; its task swaps it for one that does nothing while it
/// runs and sets it again as it ends): on row 19 between x `0xA0` and
/// `0x140`, the forest's way south.
const CAMP_FIRST_WATCH: &[Op] = &[Op::Loop(&[
    Op::IfTask {
        slot: MAP_TASK,
        then: &[],
        otherwise: &[Op::IfPlayerSprite {
            x: (0xA0, 0x140),
            y: (0x260, 0x260),
            then: &[
                Op::Calm(true),
                Op::Control(false),
                Op::Spawn(MAP_TASK, CAMP_FIRST),
            ],
            otherwise: &[],
        }],
    },
    Op::Wait(1),
])];

/// Sent to the camp first (task at `0x0801FBDC`): once the step is over,
/// Solid says to head for his camp (`0x200`) and the player walks a cell
/// back north at two pixels a frame; then the player is in control again
/// and enemies meet the party again.
const CAMP_FIRST: &[Op] = &[
    Op::AwaitArrival(PLAYER),
    Op::Dialogue(0x200),
    Op::StepPlayer {
        by: (0, -1),
        speed: 2 * PIXEL,
        shift: 2,
    },
    Op::AwaitArrival(PLAYER),
    Op::Control(true),
    Op::Calm(false),
    Op::End,
];

/// The forest (`0x0801EBB4`): the portal brings the party the first time;
/// until Fran is found, the way south sends it to the camp first.
const FOREST_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CHAPTER_OPENED],
    none: &[ARRIVED],
    then: &[
        Op::LoadMap {
            map: FOREST,
            player: FOREST_PORTAL_CELL,
            objects: ARRIVAL_OBJECTS,
            count: 4,
        },
        Op::Place(PLAYER, OFF_THE_MAP),
        Op::Control(false),
        Op::Spawn(MAP_TASK, ARRIVAL),
        Op::Flag(ARRIVED, true),
    ],
    otherwise: &[Op::IfFlags {
        all: &[SOLID_MET],
        none: &[FRAN_FOUND],
        then: &[Op::Spawn(FIELD_WATCH, CAMP_FIRST_WATCH)],
        otherwise: &[],
    }],
}];

/// Blue Gem's house for the Liger Zero's repair (ROM `0x08669FE4`): Alster
/// off the map, Blue Gem, Parti, and Regina off the map.
const REPAIR_OBJECTS: u32 = 0x0866_9FE4;
const HOUSE_ALSTER: usize = 1;
const HOUSE_BLUE_GEM: usize = 2;
const HOUSE_REGINA: usize = 4;
/// The village's gate (ROM `0x0866A23C`): Regina, and Jack and Earth off
/// the map.
const GATE_OBJECTS: u32 = 0x0866_A23C;
const GATE_JACK: usize = 2;
const GATE_EARTH: usize = 3;
/// The village's square (ROM `0x0866A28C`): the party, two soldiers,
/// Alster, and Solid and Parti off the map.
const SQUARE_OBJECTS: u32 = 0x0866_A28C;
const SQUARE_ALSTER: usize = 6;
const SQUARE_SOLID: usize = 7;
const SQUARE_PARTI: usize = 8;

const ALSTER_LEADS_OUT: &[Op] = &[
    glide16(HOUSE_ALSTER, (0x120, 0x90), 1, true),
    glide16(HOUSE_ALSTER, (0x50, 0x90), 1, true),
    Op::End,
];
const PRINCE_COMES_IN: &[Op] = &[
    glide16(PLAYER, (0x120, 0x50), 1, true),
    glide16(PLAYER, (0x140, 0x50), 1, true),
    Op::Face(PLAYER, Direction::Up),
    Op::End,
];
const ALSTER_COMES_BACK: &[Op] = &[
    glide16(HOUSE_ALSTER, (0x120, 0x90), 1, true),
    glide16(HOUSE_ALSTER, (0x120, 0x50), 1, true),
    Op::End,
];
const PRINCE_GOES_OUT: &[Op] = &[glide16(PLAYER, (0x120, 0xC0), 1, false), Op::End];
const EARTH_STEPS_UP: &[Op] = &[
    glide16(EARTH, (0x170, 0x50), 1, true),
    Op::Face(EARTH, Direction::Down),
    Op::End,
];
const REGINA_STEPS_UP: &[Op] = &[glide16(REGINA, (0x140, 0x50), 1, true), Op::End];
const JACK_STEPS_DOWN: &[Op] = &[glide16(JACK, (0x160, 0x60), 1, true), Op::End];

/// Blue Gem (task at `0x0801F5D0`): Alster brings the party to Blue Gem
/// and Parti (`0x1F5`); Blue Gem sees to the Liger Zero outside, Regina
/// greets them (`0x1F6`); Blue Gem offers to look at the party's Zoids
/// (`0x1F7`) and runs to the rare one (`0x1F8`); blasts shake the house;
/// at the village's gate the party wonders (`0x1F9`, `0x1FA`); in the
/// square the Empire's officer orders the forest burnt (`0x1FB`), Solid
/// holds to it (`0x1FC`, `0x1FD`) until the party offers to hunt the Führer
/// (`0x1FE`) and lends Alster the Liger Zero X (`0x1FF`); Alster and
/// Parti join, and the party is back in the forest.
const BLUE_GEM: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Place(HOUSE_ALSTER, (0x12, 0xC)),
    glide16(HOUSE_ALSTER, (0x120, 0x50), 1, true),
    Op::Face(HOUSE_ALSTER, Direction::Up),
    Op::RestartMusic(FRIENDS_MUSIC),
    Op::Dialogue(0x1F5),
    Op::Spawn(MAP_TASK + 1, ALSTER_LEADS_OUT),
    glide16(HOUSE_BLUE_GEM, (0x110, 0x50), 1, true),
    glide16(HOUSE_BLUE_GEM, (0x120, 0x50), 1, true),
    glide16(HOUSE_BLUE_GEM, (0x120, 0x90), 1, true),
    glide16(HOUSE_BLUE_GEM, (0x60, 0x90), 1, true),
    Op::Spawn(MAP_TASK + 1, PRINCE_COMES_IN),
    Op::Wait(16),
    Op::Place(HOUSE_REGINA, (0x12, 0xC)),
    glide16(HOUSE_REGINA, (0x120, 0x50), 1, true),
    glide16(HOUSE_REGINA, (0x130, 0x50), 1, true),
    Op::Face(HOUSE_REGINA, Direction::Up),
    Op::Dialogue(0x1F6),
    Op::Spawn(MAP_TASK + 1, ALSTER_COMES_BACK),
    glide16(HOUSE_BLUE_GEM, (0x120, 0x90), 1, true),
    glide16(HOUSE_BLUE_GEM, (0x120, 0x50), 1, true),
    glide16(HOUSE_BLUE_GEM, (0x110, 0x50), 1, true),
    glide16(HOUSE_BLUE_GEM, (0x110, 0x40), 1, true),
    Op::Face(HOUSE_BLUE_GEM, Direction::Down),
    Op::Dialogue(0x1F7),
    glide16(HOUSE_ALSTER, (0x100, 0x50), 1, true),
    Op::Face(HOUSE_ALSTER, Direction::Right),
    glide16(HOUSE_BLUE_GEM, (0x110, 0x50), 1, true),
    glide16(HOUSE_BLUE_GEM, (0x120, 0x50), 1, true),
    glide16(HOUSE_BLUE_GEM, (0x120, 0x90), 1, true),
    glide16(HOUSE_BLUE_GEM, (0x60, 0x90), 1, true),
    Op::Dialogue(0x1F8),
    Op::Sound(BLAST_SOUND),
    Op::Wait(30),
    Op::Sound(BLAST_SOUND),
    Op::Wait(30),
    Op::Sound(BLAST_SOUND),
    Op::AwaitSoundEnd(BLAST_SOUND),
    Op::Wait(30),
    Op::Sound(BLAST_SOUND),
    Op::Wait(45),
    Op::Sound(BLAST_SOUND),
    Op::AwaitSoundEnd(BLAST_SOUND),
    Op::RestartMusic(DANGER_MUSIC),
    Op::Face(PLAYER, Direction::Down),
    Op::Face(HOUSE_ALSTER, Direction::Down),
    Op::Face(HOUSE_BLUE_GEM, Direction::Down),
    Op::Face(3, Direction::Down),
    Op::Face(HOUSE_REGINA, Direction::Down),
    Op::Wait(15),
    glide16(HOUSE_REGINA, (0x110, 0x50), 1, true),
    Op::Face(HOUSE_REGINA, Direction::Right),
    glide16(PLAYER, (0x120, 0x50), 1, false),
    Op::Spawn(MAP_TASK + 1, PRINCE_GOES_OUT),
    Op::Call(SCENE_DARKEN),
    Op::Stop(MAP_TASK + 1),
    Op::LoadMap {
        map: VILLAGE,
        player: (4, 0xE),
        objects: GATE_OBJECTS,
        count: 4,
    },
    Op::Place(PLAYER, (4, 0x10)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Place(GATE_JACK, (4, 0xE)),
    glide16(GATE_JACK, (0x40, 0xF0), 1, true),
    glide16(GATE_JACK, (0x50, 0xF0), 1, true),
    Op::Face(GATE_JACK, Direction::Down),
    Op::Place(GATE_EARTH, (4, 0xE)),
    glide16(GATE_EARTH, (0x40, 0xF0), 1, true),
    Op::Dialogue(0x1F9),
    Op::Dialogue(0x1FA),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: VILLAGE,
        player: (0x15, 4),
        objects: SQUARE_OBJECTS,
        count: 9,
    },
    Op::RestartMusic(DANGER_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x1FB),
    Op::Repeat(32, PAN_DOWN),
    Op::Place(SQUARE_SOLID, (0x15, 0xC)),
    glide16(SQUARE_SOLID, (0x150, 0x70), 1, true),
    Op::Wait(15),
    Op::Face(PLAYER, Direction::Down),
    Op::Face(REGINA, Direction::Down),
    Op::Face(JACK, Direction::Down),
    Op::Face(EARTH, Direction::Down),
    Op::Face(4, Direction::Down),
    Op::Wait(15),
    glide16(SQUARE_ALSTER, (0x130, 0x30), 1, true),
    glide16(SQUARE_ALSTER, (0x130, 0x50), 1, true),
    Op::RestartMusic(RIVAL_MUSIC),
    Op::Dialogue(0x1FC),
    Op::Spawn(MAP_TASK + 1, EARTH_STEPS_UP),
    glide16(JACK, (0x160, 0x50), 1, true),
    Op::Face(JACK, Direction::Down),
    glide16(PLAYER, (0x150, 0x50), 1, false),
    Op::Dialogue(0x1FD),
    Op::Spawn(MAP_TASK + 1, REGINA_STEPS_UP),
    Op::Spawn(MAP_TASK + 2, JACK_STEPS_DOWN),
    glide16(EARTH, (0x170, 0x60), 1, true),
    Op::Dialogue(0x1FE),
    Op::Repeat(16, PAN_DOWN),
    Op::Place(SQUARE_PARTI, (0x14, 0xD)),
    glide16(SQUARE_PARTI, (0x140, 0x70), 1, true),
    Op::Dialogue(0x1FF),
    Op::Join(ALSTER_LISTS[0]),
    Op::Join(ALSTER_LISTS[1]),
    Op::Call(&back_to(FOREST, (8, 0xC))),
];

/// Blue Gem's house (`0x0801ED3C`): the first time, Alster brings the
/// party to Blue Gem.
const BLUE_GEM_HOUSE_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[ARRIVED],
    none: &[SOLID_MET],
    then: &[
        Op::LoadMap {
            map: BLUE_GEM_HOUSE,
            player: (0x12, 0xC),
            objects: REPAIR_OBJECTS,
            count: 5,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, BLUE_GEM),
        Op::ReturnPoint(BLUE_GEM_RETURN_POINT),
        Op::Flag(SOLID_MET, true),
    ],
    otherwise: &[],
}];

/// Solid's camp (ROM `0x0866A048`): the soldier at the gate.
const CAMP_OBJECTS: u32 = 0x0866_A048;
const CAMP_SOLDIER: usize = 1;

/// The camp (task at `0x0801FC8C`): the soldier salutes Captain Solid and
/// tells of a woman rescued from the Führer (`0x201`).
const CAMP_SCENE: &[Op] = &[
    glide(CAMP_SOLDIER, (0xE0, 0x100), 1, true),
    Op::Face(CAMP_SOLDIER, Direction::Down),
    Op::Wait(SETTLE),
    Op::Dialogue(0x201),
    Op::Call(&back_to(CAMP, (7, 9))),
];

/// The camp (`0x0801EDB0`): the first time, the soldier greets Solid.
const CAMP_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[SOLID_MET],
    none: &[CAMP_REACHED],
    then: &[
        Op::LoadMap {
            map: CAMP,
            player: (7, 9),
            objects: CAMP_OBJECTS,
            count: 2,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, CAMP_SCENE),
        Op::Flag(CAMP_REACHED, true),
    ],
    otherwise: &[],
}];

/// The command room (ROM `0x0866A070`): Regina, Jack, Earth, Solid, two
/// soldiers, Alster and Fran, all off the map.
const COMMAND_ROOM_OBJECTS: u32 = 0x0866_A070;
const ROOM_SOLID: usize = 4;
const ROOM_SOLDIER_1: usize = 5;
const ROOM_SOLDIER_2: usize = 6;
const ROOM_ALSTER: usize = 7;
const FRAN: usize = 8;

const PRINCE_WALKS_IN: &[Op] = &[glide16(PLAYER, (0x70, 0x40), 1, false), Op::End];
const REGINA_WALKS_IN: &[Op] = &[
    Op::Place(REGINA, (7, 8)),
    glide16(REGINA, (0x70, 0x50), 1, false),
    Op::End,
];
const JACK_WALKS_IN: &[Op] = &[
    Op::Place(JACK, (7, 8)),
    glide16(JACK, (0x70, 0x60), 1, false),
    glide16(JACK, (0x80, 0x60), 1, false),
    glide16(JACK, (0x80, 0x40), 1, false),
    Op::End,
];
const EARTH_WALKS_IN: &[Op] = &[
    Op::Place(EARTH, (7, 8)),
    glide16(EARTH, (0x70, 0x60), 1, false),
    glide16(EARTH, (0x80, 0x60), 1, false),
    glide16(EARTH, (0x80, 0x50), 1, false),
    Op::End,
];
const SOLID_WALKS_IN: &[Op] = &[
    Op::Place(ROOM_SOLID, (7, 8)),
    glide16(ROOM_SOLID, (0x70, 0x60), 1, false),
    glide16(ROOM_SOLID, (0xA0, 0x60), 1, false),
    glide16(ROOM_SOLID, (0xA0, 0x30), 1, false),
    Op::Face(ROOM_SOLID, Direction::Left),
    Op::End,
];
const FIRST_SOLDIER_BRINGS_FRAN: &[Op] = &[
    Op::Place(ROOM_SOLDIER_1, (7, 8)),
    glide16(ROOM_SOLDIER_1, (0x70, 0x70), 1, false),
    glide16(ROOM_SOLDIER_1, (0x60, 0x70), 1, false),
    Op::Face(ROOM_SOLDIER_1, Direction::Right),
    Op::End,
];
const SECOND_SOLDIER_LEAVES: &[Op] = &[
    glide16(ROOM_SOLDIER_2, (0x80, 0x70), 1, false),
    glide16(ROOM_SOLDIER_2, (0x80, 0x80), 1, false),
    Op::Place(ROOM_SOLDIER_2, OFF_THE_MAP),
    Op::End,
];
const FIRST_SOLDIER_LEAVES: &[Op] = &[
    glide16(ROOM_SOLDIER_1, (0x70, 0x70), 1, false),
    glide16(ROOM_SOLDIER_1, (0x70, 0x80), 1, false),
    Op::Place(ROOM_SOLDIER_1, OFF_THE_MAP),
    Op::End,
];

/// Fran (task at `0x0801FCF4`): the party comes into the command room;
/// the soldiers bring Fran, whom Earth claims as his (`0x202`); Solid
/// leaves them (`0x203`), Fran tells she came alone and cannot go back;
/// Solid comes back to set off, and Fran leads them to the Führer's lair
/// (`0x204`).
const FRAN_SCENE: &[Op] = &[
    Op::Wait(SETTLE),
    Op::Spawn(MAP_TASK + 1, PRINCE_WALKS_IN),
    Op::Wait(16),
    Op::Spawn(MAP_TASK + 2, REGINA_WALKS_IN),
    Op::Wait(16),
    Op::Spawn(MAP_TASK + 3, JACK_WALKS_IN),
    Op::Wait(16),
    Op::Spawn(MAP_TASK + 4, EARTH_WALKS_IN),
    Op::Wait(16),
    Op::Spawn(MAP_TASK + 5, SOLID_WALKS_IN),
    Op::Wait(16),
    Op::Place(ROOM_ALSTER, (7, 8)),
    glide16(ROOM_ALSTER, (0x70, 0x60), 1, false),
    glide16(ROOM_ALSTER, (0x50, 0x60), 1, false),
    glide16(ROOM_ALSTER, (0x50, 0x30), 1, false),
    Op::Face(ROOM_ALSTER, Direction::Right),
    Op::Wait(60),
    Op::Spawn(MAP_TASK + 1, FIRST_SOLDIER_BRINGS_FRAN),
    Op::Place(ROOM_SOLDIER_2, (8, 8)),
    glide16(ROOM_SOLDIER_2, (0x80, 0x70), 1, false),
    glide16(ROOM_SOLDIER_2, (0x90, 0x70), 1, false),
    Op::Face(ROOM_SOLDIER_2, Direction::Left),
    Op::Place(FRAN, (7, 8)),
    Op::Wait(15),
    glide16(FRAN, (0x70, 0x60), 1, false),
    Op::Spawn(MAP_TASK + 1, SECOND_SOLDIER_LEAVES),
    Op::Spawn(MAP_TASK + 2, FIRST_SOLDIER_LEAVES),
    Op::Face(PLAYER, Direction::Down),
    Op::Face(REGINA, Direction::Down),
    Op::Face(JACK, Direction::Down),
    Op::Face(EARTH, Direction::Down),
    Op::Face(ROOM_SOLID, Direction::Down),
    Op::Face(ROOM_ALSTER, Direction::Down),
    Op::Dialogue(0x202),
    Op::RestartMusic(SOLEMN_MUSIC),
    glide16(ROOM_SOLID, (0xA0, 0x70), 1, false),
    glide16(ROOM_SOLID, (0x80, 0x70), 1, false),
    glide16(ROOM_SOLID, (0x80, 0x80), 1, false),
    Op::Place(ROOM_SOLID, OFF_THE_MAP),
    Op::Dialogue(0x203),
    Op::RestartMusic(RIVAL_MUSIC),
    Op::Wait(15),
    Op::Place(ROOM_SOLID, (8, 8)),
    Op::Face(ROOM_SOLID, Direction::Up),
    Op::Wait(15),
    Op::Face(FRAN, Direction::Down),
    Op::Wait(15),
    Op::Dialogue(0x204),
    Op::Call(&back_to(CAMP, (7, 8))),
];

/// The command room (`0x0801EE1C`): the first time, Fran is found.
const COMMAND_ROOM_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[CAMP_REACHED],
    none: &[FRAN_FOUND],
    then: &[
        Op::LoadMap {
            map: COMMAND_ROOM,
            player: (7, 8),
            objects: COMMAND_ROOM_OBJECTS,
            count: 9,
        },
        Op::Control(false),
        Op::Spawn(MAP_TASK, FRAN_SCENE),
        Op::Flag(FRAN_FOUND, true),
    ],
    otherwise: &[],
}];

/// The Führer's lair (ROM `0x0866A340`): Alster's Liger Zero X and Solid's
/// Zoid, and the Führer off the map.
const LAIR_OBJECTS: u32 = 0x0866_A340;
const LAIR_FUHRER: usize = 3;

const LAIR_BRIGHTENS: &[Op] = &[Op::Call(SCENE_BRIGHTEN), Op::End];
const LIGER_ZERO_X_DRIVES_IN: &[Op] = &[glide(1, (0x100, 0x60), 1, true), Op::End];
const SOLID_DRIVES_IN: &[Op] = &[glide(2, (0x120, 0x80), 1, true), Op::End];
const FUHRER_BACKS_OFF: &[Op] = &[glide(LAIR_FUHRER, (0x160, 0x80), 1, true), Op::End];

/// The Führer's fight (story battle 29).
const FUHRER_BATTLE_OPS: [Op; 4] = story_battle(29);
/// The Führer's battle (the hook at `0x08020208`): won, it is gone and
/// the party talks it over (task at `0x0802028C`).
const FUHRER_FIGHT: &[Op] = &[
    Op::Call(&FUHRER_BATTLE_OPS),
    Op::IfLost {
        then: BEATEN,
        otherwise: &[
            Op::RestartMusic(VICTORY_MUSIC),
            Op::Place(LAIR_FUHRER, OFF_THE_MAP),
            Op::Flag(FUHRER_BEATEN, true),
            Op::Control(false),
            Op::Spawn(MAP_TASK, FUHRER_FALLS),
        ],
    },
    Op::End,
];

/// The Führer beaten (task at `0x0802028C`): the Empire's plan is called
/// off, and Alster heads home (`0x207`).
const FUHRER_FALLS: &[Op] = &[
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x207),
    Op::Call(&back_to(FOREST, (3, 0x16))),
];

/// The lair (task at `0x08020118`): Fran points to the lair (`0x205`); the
/// party drives in with Alster and Solid (`0x206`), the Führer comes out
/// and the hook at `0x08020208` starts its fight.
const LAIR: &[Op] = &[
    Op::RestartMusic(DANGER_MUSIC),
    Op::Dialogue(0x205),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: NEST,
        player: (8, 4),
        objects: LAIR_OBJECTS,
        count: 4,
    },
    Op::Spawn(MAP_TASK + 1, LAIR_BRIGHTENS),
    Op::Calm(false),
    Op::Spawn(MAP_TASK + 2, LIGER_ZERO_X_DRIVES_IN),
    Op::Spawn(MAP_TASK + 3, SOLID_DRIVES_IN),
    glide(PLAYER, (0x140, 0x80), 1, true),
    Op::Dialogue(0x206),
    Op::Place(LAIR_FUHRER, (0xC, 3)),
    Op::Wait(15),
    glide(LAIR_FUHRER, (0x180, 0x80), 1, true),
    Op::Wait(15),
    Op::Face(LAIR_FUHRER, Direction::Left),
    Op::Spawn(MAP_TASK + 1, FUHRER_BACKS_OFF),
    Op::Stop(MAP_TASK + 1),
    Op::Spawn(FIELD_HOOK, FUHRER_FIGHT),
    Op::End,
];

/// Where the lair is (the hook at `0x0801EEC8`): at x `0xA0`, y `0x40`.
const LAIR_WATCH: &[Op] = &[
    Op::AwaitPlayerSprite {
        x: (0xA0, 0xA0),
        y: (0x40, 0x40),
    },
    Op::Calm(true),
    Op::Control(false),
    Op::Spawn(MAP_TASK, LAIR),
    Op::End,
];

/// The old base (`0x0801EE94`): once Fran is found and until the Führer
/// is beaten, its lair's spot waits.
const NEST_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[FRAN_FOUND],
    none: &[FUHRER_BEATEN],
    then: &[Op::Spawn(FIELD_WATCH, LAIR_WATCH)],
    otherwise: &[],
}];

/// The village for the farewell (ROM `0x08669F44`): the party, Solid,
/// Fran, Parti and Alster, all off the map but Parti.
const FAREWELL_OBJECTS: u32 = 0x0866_9F44;
const FAREWELL_SOLID: usize = 4;
const FAREWELL_FRAN: usize = 5;
const FAREWELL_PARTI: usize = 6;
const FAREWELL_ALSTER: usize = 7;
/// Blue Gem's house when Solid calls (ROM `0x0866A390`): Solid, Blue Gem,
/// Alster, and Regina off the map.
const CALL_OBJECTS: u32 = 0x0866_A390;
const CALL_REGINA: usize = 4;
/// The village's gate again (ROM `0x0866A3F4`): Alster, and Solid and
/// Earth off the map.
const RAID_GATE_OBJECTS: u32 = 0x0866_A3F4;
/// The square for Blood's raid (ROM `0x0866A444`): the party, Alster,
/// Solid, Blood, three soldiers, Fran, Parti and Blue Gem, all off the
/// map.
const RAID_OBJECTS: u32 = 0x0866_A444;
const RAID_ALSTER: usize = 4;
const RAID_SOLID: usize = 5;
const RAID_BLOOD: usize = 6;
const RAID_SOLDIER_1: usize = 7;
const RAID_SOLDIER_2: usize = 8;
const RAID_SOLDIER_3: usize = 9;
const RAID_FRAN: usize = 10;
const RAID_PARTI: usize = 11;
const RAID_BLUE_GEM: usize = 12;
/// Blue Gem's house for his gifts (ROM `0x0866A548`).
const GIFTS_OBJECTS: u32 = 0x0866_A548;
/// The square for the goodbyes (ROM `0x0866A5E8`).
const GOODBYE_OBJECTS: u32 = 0x0866_A5E8;
/// The forest with the portal (ROM `0x0866A688`).
const DEPARTURE_OBJECTS: u32 = 0x0866_A688;

const PARTI_TAKES_FRAN_HOME: &[Op] = &[
    glide16(FAREWELL_PARTI, (0x1F0, 0xF0), 1, false),
    Op::Place(FAREWELL_PARTI, OFF_THE_MAP),
    Op::End,
];
const PRINCE_STEPS_IN: &[Op] = &[glide16(PLAYER, (0x120, 0x60), 1, true), Op::End];
const BLOOD_COMES: &[Op] = &[
    Op::Place(RAID_BLOOD, (0x16, 0x12)),
    glide16(RAID_BLOOD, (0x160, 0xE0), 1, true),
    Op::End,
];
const SECOND_SOLDIER_COMES: &[Op] = &[
    Op::Place(RAID_SOLDIER_2, (0x17, 0x12)),
    glide16(RAID_SOLDIER_2, (0x170, 0xE0), 1, true),
    Op::End,
];
const FIRST_SOLDIER_COMES: &[Op] = &[
    Op::Place(RAID_SOLDIER_1, (0x16, 0x12)),
    glide16(RAID_SOLDIER_1, (0x160, 0xF0), 1, true),
    Op::End,
];
const PRINCE_RUNS_UP: &[Op] = &[
    Op::Place(PLAYER, (0xE, 0xF)),
    glide16(PLAYER, (0x130, 0xF0), 1, false),
    glide16(PLAYER, (0x130, 0xD0), 1, false),
    Op::Face(PLAYER, Direction::Right),
    Op::End,
];
const REGINA_RUNS_UP: &[Op] = &[
    Op::Place(REGINA, (0xE, 0xF)),
    glide16(REGINA, (0x120, 0xF0), 1, false),
    glide16(REGINA, (0x120, 0xD0), 1, false),
    Op::Face(REGINA, Direction::Right),
    Op::End,
];
const JACK_RUNS_UP: &[Op] = &[
    Op::Place(JACK, (0xE, 0xF)),
    glide16(JACK, (0x130, 0xF0), 1, false),
    glide16(JACK, (0x130, 0xE0), 1, false),
    Op::Face(JACK, Direction::Right),
    Op::End,
];
const ALSTER_RUNS_UP: &[Op] = &[
    Op::Place(RAID_ALSTER, (0xE, 0xF)),
    glide16(RAID_ALSTER, (0x140, 0xF0), 1, false),
    glide16(RAID_ALSTER, (0x140, 0xE0), 1, false),
    Op::Face(RAID_ALSTER, Direction::Right),
    Op::End,
];
const FRAN_COMES_OUT: &[Op] = &[
    Op::Place(RAID_FRAN, (0x1F, 0xF)),
    glide16(RAID_FRAN, (0x190, 0xF0), 1, false),
    glide16(RAID_FRAN, (0x190, 0xE0), 1, false),
    Op::Face(RAID_FRAN, Direction::Left),
    Op::End,
];
/// Fran's knife at Parti (`0x08011E40` on object 11, a pixel left a frame
/// for 32 frames).
const PARTI_IS_TAKEN: &[Op] = &[
    Op::Repeat(32, &[Op::Nudge(RAID_PARTI, (-1, 0)), Op::Wait(1)]),
    Op::End,
];
const BLOOD_GOES_FOR_THE_CORE: &[Op] = &[
    glide16(RAID_BLOOD, (0xE0, 0x100), 1, true),
    Op::Place(RAID_BLOOD, OFF_THE_MAP),
    Op::End,
];
const BLOOD_COMES_BACK: &[Op] = &[
    Op::Place(RAID_BLOOD, (0xE, 0x10)),
    glide16(RAID_BLOOD, (0x170, 0x100), 1, true),
    Op::Face(RAID_BLOOD, Direction::Up),
    Op::End,
];
const BLOOD_LEAVES: &[Op] = &[
    glide16(RAID_BLOOD, (0x170, 0x120), 1, true),
    Op::Place(RAID_BLOOD, OFF_THE_MAP),
    Op::End,
];
const FIRST_SOLDIER_LEAVES_THE_SQUARE: &[Op] = &[
    glide16(RAID_SOLDIER_1, (0x160, 0x120), 1, true),
    Op::Place(RAID_SOLDIER_1, OFF_THE_MAP),
    Op::End,
];
const THIRD_SOLDIER_LEAVES: &[Op] = &[
    glide16(RAID_SOLDIER_3, (0x180, 0x120), 1, true),
    Op::Place(RAID_SOLDIER_3, OFF_THE_MAP),
    Op::End,
];
const FRAN_LEAVES: &[Op] = &[
    glide16(RAID_FRAN, (0x180, 0x120), 1, false),
    Op::Place(RAID_FRAN, OFF_THE_MAP),
    Op::End,
];
const DEPARTURE_BRIGHTENS: &[Op] = &[Op::Call(SCENE_BRIGHTEN), Op::End];

const BLUE_GEM_GIFT_1: [Op; 5] = zi_gift(BLUE_GEM_GIFTS[0]);
const BLUE_GEM_GIFT_2: [Op; 5] = zi_gift(BLUE_GEM_GIFTS[1]);
const BLUE_GEM_GIFT_3: [Op; 5] = zi_gift(BLUE_GEM_GIFTS[2]);
const BLUE_GEM_GIFT_4: [Op; 5] = zi_gift(BLUE_GEM_GIFTS[3]);

/// The farewell (task at `0x0802033C`): back in the village Parti welcomes
/// the party and Solid leaves (`0x208`, `0x209`); Fran is tired and Parti
/// takes her home (`0x20A`); the party heads to Blue Gem's (`0x20B`),
/// where Solid asks Blue Gem to study the Führer's core (`0x20C`); an
/// alarm and a crash, and Solid denies it is his doing (`0x20D`); at the
/// gate Blood is sighted (`0x20E`); in the square Blood looks for Fran
/// (`0x20F` to `0x211`), finds her (`0x212`, `0x213`), and Fran takes
/// Parti hostage for the core (`0x214`); she says goodbye to Earth
/// (`0x215`) and leaves with Blood; the party frees Parti (`0x216`) and
/// promises Blue Gem to get the core back (`0x217`); Blue Gem gives four
/// Zi data (`0x218`); Solid, Blue Gem and Alster see the party off
/// (`0x219`) and Alster and Parti leave the party; the hook at
/// `0x08020B98` takes it through the portal to chapter 8.
const FAREWELL: &[Op] = &[
    Op::Wait(SETTLE),
    glide16(PLAYER, (0x160, 0xE0), 1, false),
    Op::Place(REGINA, (0x16, 0xE)),
    glide16(REGINA, (0x150, 0xE0), 1, false),
    Op::Face(REGINA, Direction::Up),
    Op::Place(EARTH, (0x16, 0xE)),
    glide16(EARTH, (0x180, 0xE0), 1, false),
    Op::Face(EARTH, Direction::Up),
    Op::Place(JACK, (0x16, 0xE)),
    glide16(JACK, (0x170, 0xE0), 1, false),
    Op::Face(JACK, Direction::Up),
    Op::Place(FAREWELL_SOLID, (0x16, 0xE)),
    glide16(FAREWELL_SOLID, (0x160, 0xF0), 1, false),
    glide16(FAREWELL_SOLID, (0x150, 0xF0), 1, false),
    Op::Face(FAREWELL_SOLID, Direction::Up),
    Op::Place(FAREWELL_FRAN, (0x16, 0xE)),
    glide16(FAREWELL_FRAN, (0x160, 0xF0), 1, false),
    glide16(FAREWELL_FRAN, (0x170, 0xF0), 1, false),
    Op::Face(FAREWELL_FRAN, Direction::Up),
    Op::Place(FAREWELL_ALSTER, (0x16, 0xE)),
    glide16(FAREWELL_ALSTER, (0x160, 0xF0), 1, false),
    Op::Face(FAREWELL_ALSTER, Direction::Up),
    Op::Dialogue(0x208),
    glide16(FAREWELL_SOLID, (0xE0, 0xF0), 1, false),
    glide16(FAREWELL_SOLID, (0xE0, 0xE0), 1, false),
    Op::Place(FAREWELL_SOLID, OFF_THE_MAP),
    Op::Dialogue(0x209),
    Op::Face(PLAYER, Direction::Down),
    Op::Face(REGINA, Direction::Down),
    Op::Face(JACK, Direction::Down),
    Op::Face(EARTH, Direction::Down),
    Op::Face(FAREWELL_ALSTER, Direction::Right),
    Op::Dialogue(0x20A),
    glide16(FAREWELL_PARTI, (0x190, 0xB0), 1, false),
    glide16(FAREWELL_PARTI, (0x190, 0xF0), 1, false),
    Op::Spawn(MAP_TASK + 1, PARTI_TAKES_FRAN_HOME),
    glide16(FAREWELL_FRAN, (0x1F0, 0xF0), 1, false),
    Op::Place(FAREWELL_FRAN, OFF_THE_MAP),
    Op::Face(FAREWELL_ALSTER, Direction::Up),
    Op::Dialogue(0x20B),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: BLUE_GEM_HOUSE,
        player: (0x12, 0xC),
        objects: CALL_OBJECTS,
        count: 5,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, PRINCE_STEPS_IN),
    Op::Wait(16),
    Op::Place(CALL_REGINA, (0x12, 0xC)),
    glide16(CALL_REGINA, (0x120, 0x70), 1, false),
    Op::Dialogue(0x20C),
    Op::Sound(ALARM_SOUND),
    Op::AwaitSoundEnd(ALARM_SOUND),
    Op::Sound(CRASH_SOUND),
    Op::AwaitSoundEnd(CRASH_SOUND),
    Op::RestartMusic(DANGER_MUSIC),
    Op::Dialogue(0x20D),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: VILLAGE,
        player: (4, 0xE),
        objects: RAID_GATE_OBJECTS,
        count: 4,
    },
    Op::Place(PLAYER, (4, 0x10)),
    Op::Call(SCENE_BRIGHTEN),
    Op::Place(2, (4, 0xE)),
    glide16(2, (0x40, 0xF0), 1, true),
    glide16(2, (0x50, 0xF0), 1, true),
    Op::Face(2, Direction::Down),
    Op::Place(3, (4, 0xE)),
    glide16(3, (0x40, 0xF0), 1, true),
    Op::Dialogue(0x20E),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: VILLAGE,
        player: (0x16, 0xE),
        objects: RAID_OBJECTS,
        count: 13,
    },
    Op::RestartMusic(ENEMY_MUSIC),
    Op::Place(PLAYER, OFF_THE_MAP),
    Op::Call(SCENE_BRIGHTEN),
    Op::Spawn(MAP_TASK + 1, BLOOD_COMES),
    Op::Spawn(MAP_TASK + 3, SECOND_SOLDIER_COMES),
    Op::Wait(16),
    Op::Spawn(MAP_TASK + 2, FIRST_SOLDIER_COMES),
    Op::Place(RAID_SOLDIER_3, (0x17, 0x12)),
    glide16(RAID_SOLDIER_3, (0x170, 0xF0), 1, true),
    Op::Dialogue(0x20F),
    Op::Spawn(MAP_TASK + 1, PRINCE_RUNS_UP),
    Op::Wait(16),
    Op::Spawn(MAP_TASK + 2, REGINA_RUNS_UP),
    Op::Wait(16),
    Op::Spawn(MAP_TASK + 3, JACK_RUNS_UP),
    Op::Wait(16),
    Op::Place(EARTH, (0xE, 0xF)),
    glide16(EARTH, (0x120, 0xF0), 1, false),
    glide16(EARTH, (0x120, 0xE0), 1, false),
    Op::Face(EARTH, Direction::Right),
    Op::Spawn(MAP_TASK + 1, ALSTER_RUNS_UP),
    Op::Wait(16),
    Op::Place(RAID_SOLID, (0xE, 0xF)),
    glide16(RAID_SOLID, (0x140, 0xF0), 1, false),
    Op::Face(RAID_SOLID, Direction::Right),
    Op::Wait(15),
    Op::Face(RAID_BLOOD, Direction::Left),
    Op::Face(RAID_SOLDIER_1, Direction::Left),
    Op::Face(RAID_SOLDIER_2, Direction::Left),
    Op::Face(RAID_SOLDIER_3, Direction::Left),
    Op::Wait(15),
    Op::Dialogue(0x210),
    Op::Dialogue(0x211),
    Op::Spawn(MAP_TASK + 1, FRAN_COMES_OUT),
    Op::Wait(16),
    Op::Place(RAID_PARTI, (0x1F, 0xF)),
    glide16(RAID_PARTI, (0x190, 0xF0), 1, false),
    Op::Face(RAID_PARTI, Direction::Left),
    Op::Wait(16),
    Op::Wait(15),
    Op::Face(RAID_BLOOD, Direction::Right),
    Op::Face(RAID_SOLDIER_1, Direction::Right),
    Op::Face(RAID_SOLDIER_2, Direction::Right),
    Op::Face(RAID_SOLDIER_3, Direction::Right),
    Op::Wait(15),
    Op::Dialogue(0x212),
    Op::RestartMusic(SOLEMN_MUSIC),
    Op::Dialogue(0x213),
    glide16(RAID_SOLDIER_3, (0x180, 0xF0), 1, true),
    glide16(RAID_SOLDIER_3, (0x180, 0x100), 1, true),
    glide16(RAID_SOLDIER_3, (0x1A0, 0x100), 1, true),
    glide16(RAID_SOLDIER_3, (0x1A0, 0xF0), 1, true),
    Op::Face(RAID_SOLDIER_3, Direction::Left),
    Op::Wait(15),
    Op::Spawn(MAP_TASK + 1, PARTI_IS_TAKEN),
    glide16(RAID_SOLDIER_3, (0x180, 0xF0), 1, true),
    Op::Face(RAID_SOLDIER_3, Direction::Right),
    Op::RestartMusic(ENEMY_MUSIC),
    Op::Dialogue(0x214),
    glide16(RAID_BLOOD, (0x150, 0xE0), 1, true),
    glide16(RAID_BLOOD, (0x150, 0x100), 1, true),
    Op::Spawn(MAP_TASK + 1, BLOOD_GOES_FOR_THE_CORE),
    glide16(RAID_SOLDIER_1, (0x150, 0xF0), 1, true),
    glide16(RAID_SOLDIER_1, (0x150, 0x100), 1, true),
    glide16(RAID_SOLDIER_1, (0xE0, 0x100), 1, true),
    Op::Place(RAID_SOLDIER_1, OFF_THE_MAP),
    Op::Dialogue(0x215),
    Op::Spawn(MAP_TASK + 1, BLOOD_COMES_BACK),
    Op::Wait(16),
    Op::Place(RAID_SOLDIER_1, (0xE, 0x10)),
    glide16(RAID_SOLDIER_1, (0x160, 0x100), 1, true),
    Op::Face(RAID_SOLDIER_1, Direction::Up),
    Op::Wait(15),
    Op::Spawn(MAP_TASK + 1, BLOOD_LEAVES),
    Op::Spawn(MAP_TASK + 2, FIRST_SOLDIER_LEAVES_THE_SQUARE),
    Op::Spawn(MAP_TASK + 3, THIRD_SOLDIER_LEAVES),
    glide16(RAID_FRAN, (0x180, 0xE0), 1, false),
    Op::Spawn(MAP_TASK + 4, FRAN_LEAVES),
    Op::Wait(16),
    glide16(RAID_SOLDIER_2, (0x180, 0xE0), 1, true),
    glide16(RAID_SOLDIER_2, (0x180, 0x120), 1, true),
    Op::Place(RAID_SOLDIER_2, OFF_THE_MAP),
    glide16(RAID_ALSTER, (0x160, 0xE0), 1, false),
    glide16(RAID_ALSTER, (0x160, 0xF0), 1, false),
    Op::Face(RAID_ALSTER, Direction::Right),
    Op::Dialogue(0x216),
    Op::Place(RAID_BLUE_GEM, (0xE, 0x10)),
    glide16(RAID_BLUE_GEM, (0x160, 0x100), 1, false),
    Op::Face(RAID_BLUE_GEM, Direction::Up),
    Op::Wait(15),
    Op::Face(PLAYER, Direction::Down),
    Op::Face(REGINA, Direction::Down),
    Op::Face(JACK, Direction::Down),
    Op::Face(EARTH, Direction::Down),
    Op::Face(RAID_ALSTER, Direction::Down),
    Op::Face(RAID_SOLID, Direction::Down),
    Op::Face(RAID_PARTI, Direction::Down),
    Op::Wait(15),
    Op::Dialogue(0x217),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: BLUE_GEM_HOUSE,
        player: (7, 7),
        objects: GIFTS_OBJECTS,
        count: 8,
    },
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x218),
    Op::Call(&BLUE_GEM_GIFT_1),
    Op::Call(&BLUE_GEM_GIFT_2),
    Op::Call(&BLUE_GEM_GIFT_3),
    Op::Call(&BLUE_GEM_GIFT_4),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: VILLAGE,
        player: (0x16, 0xE),
        objects: GOODBYE_OBJECTS,
        count: 8,
    },
    Op::RestartMusic(PEACE_MUSIC),
    Op::Call(SCENE_BRIGHTEN),
    Op::Dialogue(0x219),
    Op::Leave(ALSTER_LISTS[0]),
    Op::Leave(ALSTER_LISTS[1]),
    Op::Call(SCENE_DARKEN),
    Op::LoadMap {
        map: FOREST,
        player: (5, 2),
        objects: DEPARTURE_OBJECTS,
        count: 2,
    },
    Op::RestartMusic(PEACE_MUSIC),
    Op::Spawn(MAP_TASK + 1, DEPARTURE_BRIGHTENS),
    glide(PLAYER, (0xE0, 0x20), 1, false),
    Op::Call(SCENE_DARKEN),
    Op::Spawn(FIELD_HOOK, &warp_to(CHAPTER_8_START, (8, 2))),
    Op::End,
];

/// The village (`0x0801ECB4`): once the Führer is beaten, coming in on
/// cell (22, 17) plays the farewell.
const VILLAGE_ARRIVAL: &[Op] = &[Op::IfFlags {
    all: &[FUHRER_BEATEN],
    none: &[FAREWELL_SEEN],
    then: &[Op::IfPlayer {
        columns: Some((0x16, 0x16)),
        rows: Some((0x11, 0x11)),
        then: &[
            Op::LoadMap {
                map: VILLAGE,
                player: (0x16, 0x11),
                objects: FAREWELL_OBJECTS,
                count: 8,
            },
            Op::Control(false),
            Op::Spawn(MAP_TASK, FAREWELL),
        ],
        otherwise: &[],
    }],
    otherwise: &[],
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

/// The teachers of area 7: deck commands 12 (`0x08006CEC`) and 4
/// (`0x080094B0` through `0x08009430`).
const TEACHERS: [(u32, &[Op]); 2] = [
    (0x0800_6CEC, teacher!(0xC, 0x3E2, 0x3F6)),
    (0x0800_94B0, teacher!(4, 0x390, 0x391)),
];

/// The keepers of area 7 (`0x08009318` on): item shops 13 and 14,
/// armaments shops 20 and 21, and the labs 12 and 13.
const SHOPS: [(u32, &[Op]); 6] = [
    (0x0800_9318, &shop(Shop::Items(0xD))),
    (0x0800_9324, &shop(Shop::Items(0xE))),
    (0x0800_9330, &shop(Shop::Arms(0x14))),
    (0x0800_933C, &shop(Shop::Arms(0x15))),
    (0x0800_9348, &shop(Shop::Lab(0xC))),
    (0x0800_9354, &shop(Shop::Lab(0xD))),
];

/// What speaking to an object of area 7 whose script is code runs.
pub(super) fn talk_handler(address: u32) -> Option<&'static [Op]> {
    TEACHERS
        .iter()
        .chain(SHOPS.iter())
        .find(|(at, _)| *at == address)
        .map(|(_, program)| *program)
}

/// What map `map` of area 7 runs when it loads, when it runs anything.
pub(super) fn map_handler(map: usize) -> Option<&'static [Op]> {
    match map {
        THRONE_ROOM => Some(THRONE_ROOM_ARRIVAL),
        FOREST => Some(FOREST_ARRIVAL),
        VILLAGE => Some(VILLAGE_ARRIVAL),
        BLUE_GEM_HOUSE => Some(BLUE_GEM_HOUSE_ARRIVAL),
        CAMP => Some(CAMP_ARRIVAL),
        COMMAND_ROOM => Some(COMMAND_ROOM_ARRIVAL),
        NEST => Some(NEST_ARRIVAL),
        _ => None,
    }
}
