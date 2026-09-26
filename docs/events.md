# Events

Source of knowledge: own reading of the event code of Zoids Saga (Japan, Rev 1) at the
addresses named below, plus per-frame traces of the entity table (IWRAM `0x03004BBC`),
the brightness register, the task table and the game-state block in a reference
emulator while playing the opening chapter. Implemented in
`crates/game-core/src/event.rs` (the runtime) and `crates/game-core/src/story.rs` (the
programs), with the party in `crates/extraction/src/saga_party.rs`.

## Tasks

Map events are tasks of a small cooperative kernel: `0x08003D78` spawns one into a slot,
`0x08003DD8` ends one and `0x0805EF90` yields for a number of frames. Slot 3 holds the
map's own event and slots 4–7 its helpers (fades, flicker, walks that run beside the
main event). Each frame the entities update first, then every task runs in slot order
until it yields. A task spawned into a later slot runs in the same frame.

Some calls hold the tasks until they finish: a dialogue, a blocking fade and a scene
load. The task that started it continues in the frame the hold ends. A fade or a load
holds the whole game. A task's dialogue runs inside the task, after the entities'
update, so the entities go on walking and animating while it shows: in Arcana the
soldiers leave during dialogue `0x4E`.

Handlers the game calls directly run at once, until they yield or end. Those handlers
are the code an object runs when spoken to and the code a map runs when it loads.

- An object's code runs from within the player's update, in the frame it is spoken to,
  and nothing moves while it runs. Its scripts follow one another within the call: the
  next starts in the frame the last one ends.
- A map's handler runs within the loader. A scene load it starts adds its frames to the
  black before the fade in: six plus one per object. Arcana's handler reloads the town
  with eight objects, and the town brightens 14 frames later than a plain door.

The port writes each task as a program of `Op`s, one per call of the original, and
keeps the slot order, the holds and the frame costs.

## Entities

Each entity takes 0x88 bytes, and its number is the object index plus 1. The fields
events use:

| Offset | Field |
|---|---|
| `+0x08`, `+0x0C` | Position, 16.16 fixed point |
| `+0x10`, `+0x12` | The sprite's anchor, from its record (see [formats/sprite.md](formats/sprite.md)) |
| `+0x30`, `+0x34`, `+0x38` | Animation, frame, animation shift |
| `+0x4C` | Frames left in a step, or a wanderer's wait |
| `+0x4E`, `+0x50` | Cell; `+0x52`, `+0x54` the previous cell, which blocks others until it becomes the cell halfway through a step (`+0x58`) |
| `+0x60` | Command |
| `+0x6A`, `+0x6C` | Walk target cell |
| `+0x7C` | Speed |

Commands come from the table at ROM `0x08666EA8`:

| Command | Behavior |
|---|---|
| 0 | The player's input |
| 1 | Idle |
| 2 | Wander (see [field.md](field.md)) |
| 3 | Wander, and step away from the player while B is held (see [field.md](field.md)) |
| 10 | Walk to the target: the longer axis first, the horizontal one on ties (`0x0800A864`) |
| 11 | The same, through anything: collision (`0x080084D4`) reports free for it |
| 21 | A chest opening (`0x0800B938`) |

Flag `0x400` plays the animation once, and flag `0x4` marks that it ended.

`0x08011F18(entity − 1, x, y, cell, speed, camera)` moves along x a cell if the sprite
is not at `x` yet, then along y, and again, `speed` pixels a frame for `cell / speed`
frames a cell with the walking animation of that way (7 right, 6 left, 5 down, 4 up),
the first pixel in the call's own frame; the camera follows the player when `camera`
is set. Once both axes are there it plays the standing animation of the last way, or
the one it had. A cell of 16 moves to `x − 8` (not used by the ported scenes).

## Helpers

| Routine | Use |
|---|---|
| `0x08001040`, `0x08000F88` | Test and set a game flag |
| `0x08008B58` | Run a string of the `dialogue` table |
| `0x08000BD8` | Face an entity (animation) |
| `0x08008B70` | Place an object on a cell |
| `0x080079E8`, `0x080076C0` | Load a map for a cutscene, with its own object list |
| `0x08007188` | Warp to a map |
| `0x080019B4`, `0x080019EC` | Play a song, play a sound |
| `0x08008324` | Pan the camera |
| `0x08011F18` | Move an entity's sprite to a map pixel a cell at a time, its cell left as it was (below) |
| `0x08001A08` | Stop a song; with `0x02000B54` cleared, `0x080019B4` then plays it from its start |
| `0x08001A28` | Whether a song has ended: its player's status has the pause bit |
| `0x08011E08`, `0x08011E24` | Fade out from 0 or in from 31 within the task, a level every two frames |
| `0x080014A8` | Fade in holding the game: `n` = 1 gives two frames a level, after two at black |
| `0x08009938` | Flash the screen |
| `0x08008E4C` | Stage a battle scene (descriptors at ROM `0x0866429C`; see [battle.md](battle.md)) |
| `0x080378F0` | Learn a deck command: byte `+0x347B + n` of the game state |
| `0x080370DC` | Whether deck command `n` is learned (its byte is 1) |
| `0x08009430` | A teacher of deck command `n` (see below) |
| `0x08037858` | Meet the characters of a list at ROM `0x0866C8D0` |
| `0x08037098` | See a Zoid: byte `+0x33E2 + id` |

## Timing

Brightness goes from 0 to 31, and every level of 16 or more shows black. The fade tasks
`0x0800C6A4` (out, from 0) and `0x0800C680` (in, from 31) change one level every two
frames.

| Moment | Measured |
|---|---|
| Entering a map | Held at 31 for a frame, then one level less each frame; the world runs in the frame the level reaches 0 |
| Taking an exit, arrival at frame R | Level 1 at R+2 … level 31 and the load at R+32; black until R+42; level 30 at R+43 … 0 at R+73; the world runs at R+74. The tasks the map's handler spawned and the actors' animations already run at R+73, unless the handler loaded the map again (Arcana), whose tasks run with the world |
| Cutscene load, event warp | 6 frames plus one per object |
| Exit onto the world map (map 1) | Level 30 six frames later than a room's: the load takes longer |
| Input | The game acts on the buttons of the frame before |
| Display | A frame shows the field, the windows and the brightness as the frame before left them (see [field.md](field.md), Drawing) |

A task's dialogue call runs the script's first step within the call, in the frame the
task makes it, and a task that goes on after one dialogue into another starts the next
in the frame the first ends. Closing a window that leaves another open redraws that
one: a dialogue started by speaking to someone takes a second frame for it, and so
does a task's call in the rooms and towns (the castle, Arcana) and on a Zoid map with
four sprites or more on the screen (the factory's hall once Blood's squad is in); on a
Zoid map with fewer (the world map, the factory's door, the hall before the squad
comes in) the redraw fits in the close's own frame, and the window being cleared
shows half gone on the screen. From the key to the call's return that makes 3 frames
for a box without a portrait and 4 or 5 for one with a portrait. The port takes the
map's kind and the sprites drawn as the measure of the time the frame has left.

Continuing a save runs the map's handler with the fade in already set, so a load the
handler makes holds the fade back, as entering by an exit does.

Script operations cost frames too (see [formats/script-text.md](formats/script-text.md)).
The original's real costs vary with the CPU time each frame takes. For example, a window
reset took 10 frames after a battle instead of the usual 3. The port uses the usual
costs.

## Chests

A chest is an object of behavior 4 whose script reference holds the chest number in its
low half-word (`0x80000000` is chest 0). Its opened flag is `0x1E + n`. The treasure
table at ROM `0x0866BCE4` has 12-byte records: money (4 bytes), a Zoid's Zi data (a
picture id, 0 for none), a part (2 bytes, `0xFFFF` for none), a consumable and a Zoid
core (a byte each, `0xFF` for none) and two unused bytes.

Opening one plays sound `0x46` for `tb00` or `0x48` for `tb01` and switches the chest to
its open animation (1). After 30 frames it sets the flag and runs the reward routine
(`0x080376A8`): dialogue `0x1F` opens the message box, the one reward the chest gives is
announced, and dialogue `0x22` closes it. The routine checks the fields in this order and
gives the first one there:

| Reward | What it adds | Message |
|---|---|---|
| Zoid core | its count at `+0x330C`, to 99 (`0x08037040`) | Ｚｉデータ用アイテム「`item` n」を手に入れた (`battle-text` 40, 9) |
| Zi data | sets its byte at `+0x33E2` (`0x08037098`) | Ｚｉデータ「`name` 1 + n」を手に入れた (42, 43); when the byte was set already, `battle-menu` 20 and だが、そのＺｉデータは既に持っていた・・・ (44) follow |
| Part | its stock at `+0x334C`, to 9 (`0x0803706C`) | 武装「`part` n」を手に入れた (41, 9) |
| Consumable | its count at `+0x3305`, to 99 (`0x08037014`) | アイテム「`name` 241 + n」を手に入れた (39, 9) |
| Money | the party's money, to 9,999,999 (`0x08037100`) | the amount and Ｇ手に入れた (10) |

Each message is framed by `battle-menu` 6 and 7 before and 5 after. The factory's
chests of chapter 1 hold Zi data (chests 2–4, 6, 7, 9 and 10: Zoids 46, 15, 21, 92, 2,
60 and 71) and チョバムアーマー (part 96: chests 5 and 8). The part chest and a Zi-data
chest held already were compared with the original frame by frame, and every picture
matched.

A map load shows chests whose flag is set already open.

## The opening chapter

| Where | What happens | Flags |
|---|---|---|
| Map 4, first room | The opening, dialogue 41 | `0x11F` |
| Ground floor | The soldier by the stairs (`0x0800C73C`, dialogue 43) | `0x120` |
| Second floor | The warrior (`0x0800C77C`, dialogue 42) | `0x121` |
| Throne room | Speaking to the king starts the cutscene: the attack, the first battle scene and the prince's command 王子のはげまし (deck command 19) | `0x122` |
| Gate | The emperor, then the walk to the hangar | |
| Hangar | The camera pans for 320 frames; Regina, Ace and Jack offer their Zoids (dialogues `0x3E`, `0x3F`, `0x40`) | `0x123` Shield Liger, `0x124` Saber Tiger, `0x125` Raynos |
| Departure | The party forms and the Gustav warps to map 9, cell (4, 1), facing down | |
| Map 10, the long tunnel | Stepping on column 23 plays dialogue `0x43` | `0x126` |
| Labyrinth chests | Chest 0: 4200 G (map `mq0158`, cell (18, 5)). Chest 1: 1400 G (`mq0159`, (4, 5)) | `0x1E`, `0x1F` |
| Factory chests | Maps 17–19: Zi data in chests 2–4, 6, 7, 9 and 10, part 96 in chests 5 and 8 (see Chests above) | `0x20`–`0x28` |
| Map 11, the exit | Reaching column 2 on row 2 or 3 walks the Gustav to (1, 3), plays dialogue `0x2C0` and sees the Trinity Liger (Zoid `0x8F`) | `0x127` |
| Map 1, the world map | The first time (handler `0x08010358`) the Gustav faces right and stands still; its task (`0x080103B4`) waits 60 frames from the end of the fade in, plays dialogue `0x44` (Regina: to the nearby town of Arcana), walks the Gustav at a pixel a frame to (11, 6), then (11, 7), then toward (14, 7), and ends 60 frames after starting that walk. The Gustav runs into the town's door at (14, 7) and takes it into map 24 at (23, 29) | `0x11E` |
| Map 24, Arcana | The first time (handler `0x0800E850`) the town reloads with the arrival's objects (ROM `0x0832AC54`: the prince, Regina, Ace, Jack, Roman and three soldiers), song 6 plays, and the arrival task (`0x0800E8B8`) starts: the party splits up (dialogue `0x49`; helper tasks `0x0800FBA0`, `0x0800FE14`, `0x08010088` walk Jack, Ace and Regina around town), the prince finds the bar (`0x4A`), the soldiers surround the party (`0x4B`), Roman comes out (`0x4C`, `0x4D`), the soldiers leave and everyone goes into the bar (`0x4E`) | `0x128` |
| Map 29, above the bar | The same task loads the room with its own list (ROM `0x0832ACF4`): Roman's account (`0x4F`); the prince and Regina leave by the stairs, the camera pans 64 pixels left, Jack and Ace follow (`0x50`); the task warps to map 28, the bar, at (13, 12) facing left | |
| Map 29, Roman | Teaches 包囲攻撃, deck command 26 | |
| Map 25, item shop | The keeper behind the counter opens item shop 1 (`0x080090F0`, see [shop.md](shop.md)) | |
| Map 26, armaments shop | The keeper opens armaments shop 1 (`0x080090FC`); the old man teaches 節電, deck command 22 | |
| Map 7, the castle grounds | The first time after Arcana (handler `0x080104A0`): song 4, the Gustav faces up and stands still, and its task (`0x08010514`) pans the camera up two pixels a frame for 100 frames and back down for 100, waits a second, plays dialogue `0x51` (Jack finds the guard thin, Regina recalls the underground factory's entrance), waits a second more and hands back | `0x129` |
| Map 16, the factory's door | Until it is opened the door (object 1, off the map otherwise) stands at (4, 0) (handler `0x08010588`). After Arcana its task (`0x080105F0`) waits for the Gustav on (4, 1): it stops, Regina gives the pass code (dialogue `0x52`), a second later the door plays its opening once with sound `0x6D`, a second later sound `0x82` and the Gustav drives through the door's cell at half a pixel a frame, whole animation ticks, the controls back a frame later. The door's cell is the exit to map 13 | `0x12A` |
| Maps 12 and 13, the factory's hall | Until Blood's squad is beaten, entering reloads map 12 with the player where it stands and Blood and three soldiers below the map (handler `0x080106C4`, list ROM `0x0832AD80`). The task (`0x08010754`) waits for the Gustav on (4, 1), brings it down a cell (dialogue `0x53`), pans the camera down 128 pixels, starts song 9 and walks the soldiers and Blood in two by two to their places; the camera comes back up 32 pixels, Blood speaks (`0x54`) and story battle 0 follows. Lost, the party goes to its return point. Won: song 4, the hall reloads as map 13 with the Gustav on (4, 3) and the four others (`0x0832ADD0`), the Gustav is placed on (4, 2) without the camera following, and after the fade in and two seconds the two wrecked soldiers play their wreck and explode (sprite `0xFD` through `0x080089A0`, sound `0x5A`); the camera goes up, dialogue `0x55`, the Gustav dashes out the top (two pixels a frame) and hides, `0x56`, Blood and the last soldier dash out the same way (the code moves Blood again where it meant the soldier), the screen darkens and the party warps to map 17 at (4, 6) | `0x12B` |
| Map 17, the factory's corridor | Its handler (`0x080110C8`) spawns the corridor's task (`0x0801114C`) until both its flags are set, as the hall's warp does: the first time the Gustav drives in to (14, 2) (dialogue `0x57`); at (37, 1) it stops for dialogue `0x59`. The warp runs the handler within the hall's task, and the task the handler spawns into that task's own slot is lost: the hall's task goes on after the load, waits a second after the fade in starts and ends setting the field's hook `0x0801112C`, which spawns the corridor's task the next frame. After the first part the task clears the field's bit 2 (`0x02000008`), which the handler set, and roaming enemies can meet the party again | `0x12C`, `0x12D` |
| Map 23, the space-time transfer device | Until Blood is beaten there, entering reloads the room with the party's four Zoids outside, Blood, his officer and the device (handler `0x080113A8`, list ROM `0x0832AE34`) and plays song 9. The task (`0x08011408`) drives the four Zoids in, moves the camera a pixel a frame 24 right and 32 up, and the officer takes the device: it runs once (animation 1, sound `0x6F`), the one on it leaves the map on the last frame of its 8th step and sound `0x44` plays on its 41st, then it stands again. Dialogue `0x5A`, Blood comes out (`0x5B`) and story battle 1 follows (song `0x1E` after it). Won: the fade in, `0x5C`, Blood backs off and is gone, `0x5D`, two Zoids take the device, `0x5E`, the other two drive off to the left, the Gustav takes the device last, and the party warps to map 48 (8, 1); the task then runs map 48's scene itself (`0x080129CC`), the one the map's handler spawns during the warp being lost | `0x12E` |
| Map 48, Arcadia castle's throne room | The flashback the device room's task runs once the party has warped in (`0x080129CC`; the map's handler `0x08012948` meets the guide's list 1, loads the room with the list at ROM `0x086671F8` and the Gustav off the map, and sets the flag). 32 frames later Fran walks up to the Emperor (dialogue `0x5F`) and back (`0x60`); the task fades out, loads map 30, past the Red River (list ROM `0x08667234`: the four Zoids, Fran and the portal), restarts song `0x1E` and fades in. The portal opens three times (animation 2, sound `0x6F`) and on the last frame of its 32nd step brings a Zoid, a second and the Gustav (sound `0x49`), each driving off a cell at a time (`0x08011F18`); dialogue `0x61`, the two Zoids drive into the Gustav, 30 frames, the map's song `0x11` again, the Gustav drives up two cells and `0x63`; song 9, Fran drives in at four pixels a frame with sound `0x68`, a second and the sound's end, `0x64`, he turns, a second, and drives off with the sound again; at its end `0x65`, the Gustav drives on, the task fades out and a hook (`0x08012ED4`) warps to map 31 (22, 29), Sand Colony, with the facing kept, and fades in holding the game (`0x080014A8` with 1) | `0x143` |
| Map 27, Dr. T's lab | Dr. T (`0x0802AB08`) talks with Regina about rebuilding the Trinity Liger (dialogue `0x2C1`), later `0x2C2`; the assistant on the left teaches データ収集, deck command 0; the keeper at (5, 5) opens Zoid lab 1 (`0x08009108`, see [shop.md](shop.md)) | `0x13F` |

The map record's byte `+8` names the map's song (11 for the castle, 22 for the
labyrinth). On maps with 32-pixel cells (the Zoid maps) the player is the carrier `mz10`
(sprite 0). Each cell has a single attribute, and the player's footing is its own cell.

Arcana's townsfolk are objects of kind 3 (see [field.md](field.md)).

## Teachers

The objects that teach deck commands run `0x08009430` with the command `n` (their code
is at `0x08009480 + 12n`). The routine reads a pair of dialogue indices at ROM
`0x08328EC4 + 4n`. While the command is not learned, it runs the first line, then the
learning message of `0x080378F0`:

- dialogue `0x1F`;
- battle-menu strings 6 and 7;
- battle-text string `0x39`;
- the command's name (string 77 + `n` of the item table);
- battle-text string `0x3A`;
- battle-menu string 5;
- dialogue `0x22`.

Once the command is learned, it runs the second line. Command 16 takes another path
(`0x0803795C`).

Dr. T's code reads the area (the low byte of the map record's id, RAM `0x0200000C`).
Area 1 is the talk above; areas 9 and 10 have a line each (`0x2C9`, `0x2CA`).

## The party

After the choice in the hangar, the event runs three routines:

1. `0x08037644` gives the prince a unit of the chosen Zoid: `0x0F` for choice 1,
   `0x5C` for choice 2, `0x39` otherwise. It puts that unit in formation slot 1.
2. `0x080374B8` goes through the first starting list, at the first pointer of ROM
   `0x0867E380`. The list gives characters 1–3 a unit each.
3. `0x08037684` puts characters 1, 2 and 3 in formation slots 4, 0 and 2, then
   `0x080368BC` sets the three warriors' pilot bonuses to the party level times their
   growth (ROM `0x66BB38`, five half-words each), after their units' statistics.

A unit's statistics (`0x08036CB0`) start from its Zoid's record. Each step adds:

- half the unit's training level, in percent (`0x080346C0` rounds to nearest below
  `0x10000`);
- the bonuses of its active parts, from the part records at ROM `0x0866C8F8`;
- its pilot's bonuses in percent: from the member records for characters 0–3, from ROM
  `0x0867B35C` by chapter for the others.

The first statistic is capped at 9999, the second at 999, and the two half-words at
9999 and 999. The half-words are stored after every step, so they wrap to 16 bits. The
layout of units and formation slots is in [formats/save.md](formats/save.md).

The port's party matched the original's game-state block byte for byte after the
Shield Liger choice: 131 bytes of member records, units, count, character table and
formation.

## Checked against the original

- The whole of chapter 1 plays in the port from a new game: a driver in the research
  notes presses A through the scenes and battles, walks to the next goal over the maps'
  exits, heals at the lab after a loss and fights roaming Zoids before Blood. It sets
  every flag of the chapter (`0x11E`–`0x12E`, `0x143`) and reaches Sand Colony (map 31)
  after 230464 frames and 19 battles; each scene on the way was compared with the
  original on its own (below).
- The opening matches frame by frame. A known one-frame drift appears after dialogue 41.
- The title, continuing and the notices match frame by frame.
- Map exits match frame by frame.
- The throne-room cutscene matches frame by frame, its two battle scenes included (see
  [battle.md](battle.md)).
- The later segments match as sequences of entity states: the throne room, the ground
  floor, the gate, the hangar, the choice, the departure, and the arrival in `mq0157`
  at pixel (128, 32).
- `mq0157` has no random battles.
- Arcana's arrival matches as a sequence of entity states from the door to the warp
  into the bar. It also matches pixel for pixel on frames sampled every 100, and on
  every frame of a walk, of the fade into the bar and of the warp back down. Roman's
  lesson matches frame by frame.
- Arcana's shops: see [shop.md](shop.md).
- The first room's opening matches pixel for pixel on 332 of 343 frames sampled every
  7. Before the drawing order and the frame of display delay were modeled, 64 did (279
  differed); before the prompt's blink was 21 frames, 327.
- The drive to Arcana runs a frame late after dialogue `0x44` closes, and the town
  brightens a frame early.
- The castle grounds' view, entered from map 14 (`mq0161`) with the party formed and
  Arcana done: every frame from the door to the view's end is identical once shifted
  by one, the black after the load being a frame longer in the port (see below), but
  for two frames of a page turn of dialogue `0x51`.
- The factory's door, from two cells below it: every frame from the first step to the
  door's opening is identical; the sounds fall on the original's frames. The door's
  opening and the Gustav driving through it show their new pictures a frame early
  (see [field.md](field.md), Drawing): they are near the top of the screen. The last
  close of dialogue `0x52` shows one frame differently (the original's half-cleared
  window).
- The space-time transfer device, entered from map 21: every event falls two frames
  after the original's, the black after the exit being longer in the port.
- The space-time transfer device, entered by continuing a save in map 23: every event
  falls a frame after the original's before the battle and on its frame after it (the
  battle's enemy set to 1 hit point in a copy of the ROM for both); the frames differ
  in the device's idle animation, where sprites overlap at the screen's left edge, and
  after some page turns of the long dialogue `0x5B`, whose portraits take the original
  a frame longer with this many sprites.
- The factory's hall: every event of the ambush falls on the original's frame
  (dialogue `0x53` at the same frame, song 9, dialogue `0x54`, the battle's sound),
  and 1223 of its 1310 frames are identical; the others are a typewriter's character
  a frame early after some page turns and the soldiers' animation near the bottom
  edge. After story battle 0 (its enemies set to 1 hit point in the original to
  shorten it, the frames aligned on the song that follows it) the explosion, dialogue
  `0x55` and the camera come a frame late, the reload of map 13 taking 11 frames
  where the original's took 10; dialogue `0x56` and the dash out match, and the
  corridor's dialogue `0x57` falls on the original's frame.
- The throne room's flashback and the landing past the Red River, continuing a save in
  map 21 (with the enemy of story battle 1 set to 1 hit point in both): from map 48's
  load every interval between events not bounded by a key falls on the original's
  (the portal's sounds 123 and 114 frames apart, dialogue `0x61` 82 frames after the
  last, song `0x11` 96 frames before `0x63`, sound `0x68` 140 frames before `0x64` and
  before `0x65`), and the last fade out and the fade in into Sand Colony take their 60
  frames each. The black between them is 22 frames in the port and 35 in the
  original: the load of map 31 is longer (see below).
  Compared by picture as well, with the frames aligned on each dialogue: the portal,
  the Zoids it brings, the Gustav's drive up, Fran's coming and going and the last
  drive are identical, but for the portal's idle bobbing, whose phase follows how long
  the dialogues before were held. This comparison found the camera running at twice
  the Gustav's speed when it drove up, which left it below the screen: the glide
  (`0x08011F18`) moves the entity and scrolls the camera by the same amount
  (`0x08008324`), and the camera never follows a gliding sprite by itself, which the
  port's camera, anchored to the player, did.
- The black after an exit's load depends on the scene. The port uses one length for
  every room: leaving Arcana's bar for the streets, the original stays black six frames
  longer, and going up to the room above, two frames shorter.

## Chapter 2

Source of knowledge: own reading of the area's map handlers (map record `+12`), the
tasks and field hooks they install, the objects' code and the party routines named
below; checked against a reference emulator with saves patched to each scene's flags
(and, for the party's changes, with the lists joined), frame by frame and by the game
state after each change. Implemented in `crates/game-core/src/story/chapter2.rs`.

Area 2 is maps 30 to 49: the desert past the Red River (30), Sand Colony (31) with its
bar (32), Dr. T's lab (33) and a house (34), the thieves' tunnels (35 to 47, with
chests 11 to 25), the throne room (48) and the ruins (49). Its story is a chain of
flags, each scene setting the next:

| Where | Handler | Condition | What happens | Sets |
|---|---|---|---|---|
| 31 | `0x08012350` | `0x143`, not `0x144` | The town loads with the party's six (ROM `0x08666F50`) and the arrival runs (task `0x08012EF8`): the party walks in, Regina and Jack talk (`0x66`), Van crashes into the prince to song 8 and runs off after Fran with Zeke (`0x67`), the town's song comes back (`0x68`) and the town loads again | `0x144` |
| 32 | `0x08012408` | not `0x145` | The bar loads with Moonbay, Irvine and a stranger (ROM `0x08666FC8`); speaking to Moonbay (`0x08012444`) starts the scene (task `0x080133DC`, dialogues `0x75`, `0x76`): everyone files out and Irvine and Moonbay join (list 1) | `0x145` |
| 31 | `0x08012350` | `0x145`, not `0x146` | The party plans its search (task `0x080131B0`, `0x77`) | `0x146` |
| 30 | `0x080120C0` | `0x146`, not `0x147` | A field hook (`0x080122F0`) waits for the Gustav at x `0x340`, y `0x60` to `0xA0`; Van fights the thieves (task `0x080131EC`, `0x7C`, `0x7D`) and story battle 2 follows from the next hook (`0x0801328C`) | `0x147` on winning |
| 30 | | `0x147`, not `0x148` | The talk after the ambush (task `0x08013304`, `0x7E`) | `0x148` |
| 34 | `0x08012498` | `0x148`, not `0x149` | The house's talk (task `0x0801334C`, `0x7F`); its six people each tell a piece (`0x80` to `0x85`, before `0x87` to `0x8C`) | `0x149`, then `0x14A` to `0x14F` |
| 30 | | `0x14A` to `0x14F`, not `0x150` | The party puts it together (task `0x08013620`, `0x86`) | `0x150` |
| 37 | `0x080125A4` | `0x150`, not `0x151` | The hideout: the Zoids gather, the camera goes up, Van and Zeke join (list 2, flag `0x197`, `0x8D`), story battle 3 (hook `0x080137D0`) | `0x151` on winning |
| 37 | | `0x151`, not `0x152` | Everyone drives out (task `0x08013840`, `0x8E`, `0x90`) | `0x152` |
| 38 | `0x080126E0` | `0x152`, the Gustav at x `0x200` or less, not `0x153` | A hook (`0x08012888`) waits for it at y `0xA0`, x `0x80` or less; Raven's ambush (task `0x08013964`, `0x98`), story battle 4 (hook `0x08013AD0`) | `0x153` on winning |
| 38 | | `0x153`, not `0x154` | Raven drives off (task `0x08013B2C`, `0x99` to `0x9C`); Irvine, Moonbay, Van and Zeke leave (lists 1 and 2) and a hook (`0x08013CCC`) takes the prince to the desert | `0x154`, `0x157` |
| 30 | | `0x154`, `0x157`, not `0x155` | Raven alone (task `0x08013D70`, `0x9D`), story battle 5 (hook `0x08013DD8`); the handler clears `0x157` | `0x155` on winning |
| 30 | | `0x155`, not `0x156` | The chapter's end (task `0x08013E34`): the throne room (`0xA0` to `0xA2`), the ruins (`0xA3`), the Gustav into the portal past the Red River (`0xA4`) and out of the ruins' portal (`0xA5` to `0xA7`); a hook (`0x0801423C`) warps to map 82 | `0x156` |

A lost story battle takes the party to its return point (`0x08006E08` with 1) and the
field brightens at once; the flags stay, so the scene runs again. The townsfolk of Sand
Colony and the bar speak by progress (`0x0801247C` tests `0x145`, `0x080126D0` tests
`0x151`); the town has item shop 2 (`0x08009114`), armaments shop 2 (`0x08009120`), Dr.
T's lab 2 in map 33 (`0x0800912C`) and a teacher of deck command 2 (`0x08006824`); the
house has one of deck command `0x18` (`0x080068FC`); both use `0x08012090`.

What the chapter's code needed of the event engine:

- **Field hooks.** Several tasks end by storing a routine in the field's per-frame hook
  (RAM `0x02000000`) and ending: the hook runs the story battle or warps the frame
  after, outside the tasks, so the map's handler it causes can start its own task in
  the map's slot. The port runs these in task slot 2, before the map's (3). A map
  handler that sets a hook to wait for the player runs as the map's task instead.
- **Entities and objects.** Turns and animations (`0x08000BD8`) and sprites
  (`0x080089A0`) take an entity, one past the object; placing (`0x08008B70`) and
  gliding (`0x08011F18`) take the object.
- **Scene fades.** `0x08011E08` and `0x08011E24` set the level and wait a frame before
  stepping it, a frame more than the fades the port had.
- **The cutscene loader** (`0x080076C0`) starts the map's song unless it plays, as its
  last step: a scene that changes the song right after never lets the map's be heard.
- **Glides in rooms** use cells of 16 and move their target 8 pixels left.
- **Position tests** of the hooks read the entity's pixel position (`+0x08`, `+0x0C`),
  which a step reaches at its end, rather than its cell.
- **The party.** A list of ROM `0x0867E380` joins (`0x080374B8`, as the hangar's) or
  leaves (`0x080374E4`): a leaving character comes out of the formation (`0x08037B1C`),
  its own unit (character bit `0x08`) is cleared, another unit loses its pilot
  (`0x08036C2C`: values worked out again without one, what is left capped), and bits
  `0x04` and `0x02` go; if no member then has a working unit, the prince's is repaired
  (`0x08037510`). The characters, units and formation after the bar's join and after the
  canyon's parting matched the original byte for byte.

Checked against the original: the arrival matched frame for frame from the walk in to
the town's reload, the bar's scene in 1643 of its 1650 frames (the others a message's
key timing), the town's talk, the ambush, the chase's talk and the shops' screens;
the chapter's end event for event, its sounds and songs on the same frames relative to
each load. The loads themselves take longer in the original, 10 to 23 frames each at
Sand Colony, the desert and the ruins.

## The end of the demo (a port feature)

Source of knowledge: this project's own design. The port's story stops where chapter 2
does, in chapter 3's first map (82) once the chapter's end has run (flag `0x156`). When
the player walks freely there in full light (after the scene's fade in, or after
continuing a save made there), the game waits a second and ends the demo (`crates/game-core/src/demo.rs`): a story box, window 0 at (0, 12) 30×8, thanks the
player (`port/demo/thanks`) and waits for A with the prompt blinking; it then asks
`port/demo/question` over the original's はい/いいえ window (`pause-menu` 61). はい saves:
with several save slots their list comes first, with the title's layout, B going back
to the question; `port/demo/saved` follows. いいえ, B, or a notice dismissed closes the
windows, stops the music and fades to black over 16 frames, and 30 frames later the
title starts again. The field keeps moving behind the windows. `Game::set_demo_end`
moves the end, or with `None` lets the game go on.

## Story inventory

The chapters follow the areas: the low byte of a map record's id (see
[formats/map.md](formats/map.md)), which entering a map writes to the game state. The
343 maps split into ten areas, named after them (`mq0100`–`md0198` area 1, `mq0200`…
area 2 and so on). Their code: 247 distinct map handlers (record `+12`; many maps share
one; most only set the field's hooks) and 169 objects whose script is code (the other
objects just say a dialogue string). A static
reading of each, of the tasks it spawns (`0x08003D78`, `0x08003DF8`) and of the routines
it calls, with the constants passed to the flag (`0x08001040`, `0x08000F88`), dialogue
(`0x08008B58`), story battle (`0x08008D28`) and staged scene (`0x08008E4C`) routines,
gives per area:

| Area | Maps | Handlers with events | Objects with code | Dialogues their code runs | Story battles | Shop keepers | Teachers |
|---:|---:|---:|---:|---:|---|---:|---:|
| 1 | 30 | 12 | 16 | 67 | 0, 1 | 3 | 3 |
| 2 | 20 | 7 | 19 | 68 | 2–5 | 3 | 0 |
| 3 | 44 | 12 | 27 | 93 | 6, 7 | 8 | 1 |
| 4 | 39 | 9 | 12 | 45 | 8, 9 | 8 | 1 |
| 5 | 43 | 5 | 22 | 60 | 25 | 15 | 1 |
| 6 | 41 | 12 | 19 | 94 | 26–28 | 9 | 1 |
| 7 | 19 | 7 | 8 | 55 | 29 | 6 | 1 |
| 8 | 37 | 13 | 19 | 93 | 30, 31, 33–35 | 9 | 1 |
| 9 | 24 | 6 | 5 | 73 | 36–40 | 3 | 1 |
| 10 | 46 | 2 | 14 | 16 | 41 | 9 | 1 |

Story battles 10–24 and 32 are not called with a constant: they come from elsewhere
(the arena, tables), still to be traced. Area 1 after Arcana:

| Map | Handler | What it does | Flags |
|---|---|---|---|
| 7, the castle grounds | `0x080104A0` | The view above | `0x129` |
| 16, `mq0163` | `0x08010588` | The factory's door (above) | `0x12A` |
| 12 and 13, `mq0160` | `0x080106C4` | Blood's ambush and story battle 0 (above) | `0x12B` |
| 17, `mq0164` | `0x080110C8` | The corridor (above) | `0x12C`, `0x12D` |
| 23, `mq0192` | `0x080113A8` | The space-time transfer device and story battle 1 (above); its task warps to map 48 and runs that map's scene | `0x12E` |
| 48, `md0288` | `0x08012948` | The throne room's flashback and the landing past the Red River (map 30) (above); the chapter ends with the warp to map 31, Sand Colony, in area 2 | `0x143` |
| 27, Dr. T's lab | `0x08009108` | The Zoid lab (kind 2 of the shops): revival, and the repair when it closes (see [shop.md](shop.md)) | |

## Not modeled yet

- The fighting in battles, and the story battles (`0x08008D28`); the roaming enemies,
  meeting them, the battle screen's opening, its menu and retreating are described in
  [combat.md](combat.md).
- The Zoid lab's development, pilot change and sale, and the other towns' shops (see
  [shop.md](shop.md)).
- Dr. T in the other areas: whether the party has the Zoids `0x90` or `0x8F`, and the
  game-state byte `+0x3320` (flags `0x140`, `0x141`).
- The field's per-frame hooks (RAM `0x02000000`, `0x02000004`), which the world map's
  and Arcana's handlers set. The ones seen so far (`0x08008024`, `0x08008028`,
  `0x0800C4D0`, `0x0800C708`) return at once; the world map's battles come from its
  roaming enemies instead (see [combat.md](combat.md)). The world map's handler swaps
  them for empty ones during the drive and leaves the second one empty. Arcana's
  arrival sets its own and puts the field's back when it ends; after it, Arcana's
  handler (`0x0800BEE4`) only sets them.
- The CPU-time variance of script operations.
- The door of map 20 (object `0x117`, an object state, on its exit to map 23): what
  takes it away. The chapter's way into map 23 does not need it: the corridor (17)
  leads through maps 18, 19 and 21 to map 23's left entrance, (0, 3), where its
  handler stands the player; map 20 is the factory's other door, from the world map.
- Chapter 2: the time the original's loader takes for each map (the cutscene loads
  and warps of Sand Colony, the desert and the ruins take 10 to 23 frames longer than
  the port's estimate, see "Chapter 2" above), and the page turns of a message under
  keys pressed every other frame, which the original takes two frames longer to accept.
- A one-frame drift of the backdrop.
