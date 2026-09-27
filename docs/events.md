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

Some calls hold the tasks until they finish: a blocking fade and a scene load hold the
whole game. A task's dialogue holds only that task: the script runner hands the frame
back to the kernel every frame (`0x0803E51C` calls `0x0805EF90`), so the entities go on
walking and animating while it shows (in Arcana the soldiers leave during dialogue
`0x4E`) and the other tasks go on too, from the frame it starts. The task that started
it continues in the frame it ends. Chapter 3 needs this: in the fortress's wing a helper
task walking an officer in ends its walk in the frame the main task starts dialogue
`0xD7`, and the officer's later walk out would otherwise meet the helper's unfinished
one. During a dialogue started by speaking to someone the port still holds every task,
which is not checked against the original.

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

The field's per-frame hook (RAM `0x02000000`) runs before the entities move. A hook that
watches where the player stands (chapter 4's yard, `0x08017208`) takes the control away
in the first field frame the player is on its spot, before a roaming enemy beside it
can step in. The port runs such a hook in its own slot (`FIELD_WATCH`) before the
field's update, and ends it when the player leaves the map; the hooks that only start a
battle or a warp after a scene stay in the slot before the map's task.

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
announced, and dialogue `0x22` closes it. The 30 frames are waited within the player's
update (`0x0800B938` calls `0x0805EF90` with `0x1E`), so nothing on the field moves or
animates and the keys go unread until the reward: pressing A again does not search the
chest a second time. Checked against a reference emulator with A pressed every other
frame at a chest of map 157: the party and the chest match picture for picture through
the wait. The routine checks the fields in this order and gives the first one there:

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
Area 1 is the talk above; areas 9 and 10 have a line each (`0x2C9`, `0x2CA`). In the
other areas the first that holds says its line: flag `0x140` set, `0x2C8`; a unit of
Zoid `0x90` owned (`0x0802AACC`), `0x2C7` and flag `0x140`; one of `0x8F`, `0x2C6`; the
game-state byte `+0x3320` not zero (what sets it is not traced), `0x2C5`; flag `0x141`
set, `0x2C4`; otherwise `0x2C3` and flag `0x141`.

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

## Chapter 3

Source of knowledge: own reading of the area's map handlers, the tasks and field hooks
they install, the objects' code and the routines named below; checked against a
reference emulator with saves patched to each scene's flags, comparing the frames of the
songs, sounds and dialogues and the pictures between them. Implemented in
`crates/game-core/src/story/chapter3.rs`.

Area 3 is maps 50 to 93: the desert around Mount Ossa (50), a Zoid map, with its town
(51 to 53), the Kronos fort (54 to 60), the Mount Ossa fortress (61 to 66), its rear
tunnels (67 to 71), the path to the crater (72 to 77) and a cave (78 to 80); the
Emperor's castle (81, 82), and the kingdom's base in the past, whose rooms reuse the
first chapter's (83 to 93). The base and the desert are joined by a pair of portals (map
86's and map 50's; see [field.md](field.md), Portals). The chapter's story is a chain of
flags:

| Where | Handler | Condition | What happens | Sets |
|---|---|---|---|---|
| 82 | `0x08014C68` | not `0x158` | The characters of group 2 are met; the throne room loads with the court (ROM `0x0866770C`) and the opening runs (task `0x08014D5C`): Fran is sent away (`0xA8`, `0xA9`), Opis brings the rare-hertz amplifier and takes the command, Blood and Gale are sent to guard the castle (`0xAA`, `0xAB`); in the hall (81) Blood hints that Gale let the prince go (`0xAC`, `0xAD`); at the base (92) a soldier reports the space-time device in use (`0xAE`) and the party leaves | `0x158` |
| 50 | `0x0801426C` | `0x158`, not `0x159` | Through the portal (task `0x08015000`): the portal brings the Gustav (`0xB1`) | `0x159` |
| 51 | `0x08014628` | | Entering the town | `0x15D` |
| 50 | | `0x15D`, not `0x15C` | Out of the town, to the danger song (4), three runaway Zoids close in (task `0x080152A4`, `0xBE`, `0xBF`); story battle 6 follows from the hook `0x08015338`. The handler clears `0x15D` first, so a lost fight waits for another visit to the town | `0x15C` on winning |
| 50 | | `0x15C`, not `0x15E` | Opis shows himself to his song (9) and heads for the fortress (task `0x08015400`, `0xC0` to `0xC2`) | `0x15E` |
| 61 | `0x080147F8` | `0x158`, not `0x161` | The fortress's gate loads with Dr. D and a guard (ROM `0x0866757C`). Before `0x15E` the guard sends the party away (task `0x0801574C`, `0xC3`); after it Dr. D takes the party in (task `0x080154FC`, `0xC6` to `0xCB`), and a beaten party is taken to the fortress's lab from then on (return point 10, `0x08006DFC`) | `0x161` |
| 68 | `0x08014A44` | not `0x15F` | The rear entrance's guard: the first time `0xC4`, later `0xC5` (task `0x08015498`) | `0x160` |
| 66 | `0x08014918` | not `0x162` | The command room (task `0x080157C0`, `0xCC` to `0xCF`): Colonel Ford, Captain Herman and Lieutenant O'Connell; Dr. D sends the party to measure the rare-hertz zone | `0x162` |
| 50 | | `0x159`, not `0x15A`, not `0x164` | The zone's watch (the hook `0x080144DC`, below) | `0x15B`, `0x163`, `0x164` |
| 50 | | `0x164` | The four amplifiers (objects 1 to 4, `0x08016A58` on) stand until each is destroyed: speaking to one runs `0xD9`, an explosion over it (object 5, sound `0x5B`) and four blinks of eight frames (`0x08016C38`, `0x08016CA8`, `0x08016BC8`) | `0x165` to `0x168` |
| 54 | `0x08014654` | `0x165` to `0x168`, not `0x169` | The Kronos fort (task `0x08015BEC`): the officers arrive (`0xDF`), Van, Fiene and Colonel Krueger come out (55, `0xE0` to `0xE2`), and in Krueger's room (57, `0xE3`) Van and Fiene join (lists 6, 7) and Irvine, Moonbay and Zeke leave (lists 4, 3, 5). Before that the rare hertz sends the party back (task `0x08015F18`, `0xDD` or, once the amplifiers are located, `0xDE`) | `0x169` |
| 60 | `0x08014780` | `0x169`, not `0x16A` | The amplifier's core (object 1, `0x080147B0`): speaking to it runs its scene (task `0x08015F74`): it blows up, but the runaway Zoids go on (`0xE7`, `0xE8`); the officers (57, `0xE9`), Dr. D's guess about the volcano (66, `0xEA`); list 8 joins, list 7 leaves | `0x15F`, `0x16A` |
| 71 | `0x08014AB4` | not `0x16B` | The rock-boring laser's container (object 1, `0x08014AD4`): it opens with sound `0x46`, the laser is found (`0xED`, task `0x080161C0`) and brought to Dr. D (`0xEE`) | `0x16B` |
| 77 | `0x08014B74` | `0x16B`, not `0x16C` | The crater's path reloads with Opis (ROM `0x086676BC`); the hook `0x08014BE0` waits for the Gustav at pixel (`0x440`, `0x280`); Opis (task `0x08016264`, `0xEF`); the first time Van and Irvine go on to the crater (`0xF0`, `0xF1`, lists 8 and 6 leave, flag `0x198`); story battle 7 follows from the hook `0x080168B0`. Once the laser is found the path's rock (object 1) is gone | `0x198`, `0x16C` on winning |
| 77 | | after the win | The chapter's end (task `0x08016378`): Opis flees (`0xF2`, `0xF3`), the laser opens the crater (`0xF4`, `0xF5`), the thanks in the command room (`0xF6` to `0xF8`), the Gustav through the desert's portal, the Emperor sends for Gale (82, `0xF9`, `0xFA`), the base's portal brings the party home (86, `0xFB`); the hook `0x08016880` warps to map 120, chapter 4's first | |

The zone's watch (`0x080144DC`) runs every frame on the desert until the amplifiers are
located. The zone is the cells whose attribute has bit `0x1000` (`0x08008434` reads the
player's). On one, before the command room has sent the party, the player stops and the
task `0x08015118` turns it back: once its step is over, the first time the danger song
plays with `0xB2` (flag `0x15B`), later `0xB3`; the Gustav backs off a cell (command 11
toward the cell behind it, two pixels a frame). Once sent, the first time
`0x08015968` says the zone starts here (`0xD4`, flag `0x163`) and the count of roaming
battles won (the game state's `+0x0A`, which `0x0800B9CC` counts) starts again. While
measuring, the danger song plays on the zone and, once more than two battles have been
won there, the task `0x080159B4` runs: the party is called back (`0xD5`), Dr. D finds
the amplifiers (66, `0xD6`), and in the fortress's wing (64) Irvine and Moonbay burst
in after Van (`0xD7`, `0xD8`) and join with Zeke (lists 4, 3, 5; flag `0x164`). Off the
zone the count starts again and the map's song comes back. The tasks the watch starts
clear the hook while they run and set it again as they end; the port keeps the watch
running and has it skip its check while the map's task runs.

The town's people (`0x08006914` on; `0x08014618` tests `0x16A`) have a line before the
core is destroyed and one after. The teachers use `0x08012090`: deck commands 1 (the
town), 9 (the fortress's gate), 13 (its wing), 10 (its hall) and 11 (the command room,
whose reminder is dialogue `0xEC` in the original), and 21 (the base's bar, `0x0800957C`
through `0x08009430`). Dr. D in the command room (`0x080149C0`) has a line for each
stage (`0x2D7` to `0x2DB`). The keepers (`0x08009138` on): item shops 3 (the town) and 4
(the base), armaments shops 3 (the town), 4 (the fortress), 5 (the base) and 26 (the
Kronos fort), and the labs 3 (the fortress) and 4 (the base).

What the chapter needed of the engine:

- **Portals** (see [field.md](field.md)).
- **The return point** is the game state's byte 3, which the loader sets to the area's
  own only when the area changes (`0x08007328`), so `0x08006DFC` can move it until the
  party leaves the area.
- **Object states** keep the cells the stepping command writes back (`0x0800B764`); an
  object an event places (the amplifiers off the map before they are located) keeps
  its state.
- **A task's dialogue** holds only its own task (see Tasks above).
- **Loops, the player's cell attribute, the count of battles won, the song playing, the
  map's song again, a step back** and ending another task are new ops.

Checked against the original, scene by scene from patched saves (the frames of each
song, sound and dialogue, and the pictures between dialogues): the opening (every
interval between its lines, `0xA9` to `0xAE`, equal); the portal from the base (the
portal's sounds on the original's frames, and in the desert the two openings, the two
`0x49` and `0xB1` at its intervals, one of them a frame apart); the runaway Zoids to the
battle's first sounds; Opis; the gate, refused and with Dr. D; the rear entrance; the
command room (its lines on the original's frames); the zone's turning back and edge;
the measurements (every interval that no key bounds equal, and the pictures but for the
roaming enemies, which are random); an amplifier's destruction (identical pictures but
for a roaming enemy); the Kronos fort; the core; the laser; the crater to story battle 7; and the chapter's
end, with the battle's enemies kept at 1 hit point and the party's at full by writing the
battle's units (their hit points are the word at `+0x10` of the 0x2CC-byte units): every
interval no key bounds is equal from `0xF2` to `0xFB`, but for `0xF5`, a frame early. The
loads of the desert and the other Zoid maps take the
original 10 to 30 frames longer than the port's (see Not modeled yet); the songs the
loads start come earlier within the black in the port.

A driver in the research notes plays the game in the port alone from a new game to
chapter 4's first map in one run (438885 frames, the protagonist's attacks overpowered
in battle, six fights lost and retried after the lab's repair), setting every flag of
chapters 1 to 3 but the choices not taken and the zone's turning back, which it
avoids. On the way it found the tunnel's guard of chapter 2 (see
[field.md](field.md), Roaming enemies).

## Chapter 4

Source of knowledge: own reading of the area's map handlers, the tasks and field hooks
they install, the objects' code and the routines named below; checked against a
reference emulator with saves patched to each scene's flags, as chapter 3's. Implemented
in `crates/game-core/src/story/chapter4.rs`.

Area 4 is maps 94 to 132: the border's plains (94), a Zoid map with a portal, a town
(95 to 97), the Empire's Ark (101 to 103), Bego (104, 105), Zeta (106) and Deme (107 to
111, 131, 132) bases, and the Emperor's castle and the kingdom's base again (120 to
129). The chapter's story is a chain of flags:

| Where | Handler | Condition | What happens | Sets |
|---|---|---|---|---|
| 120 | `0x080172F4` | not `0x16D` | The characters of group 3 are met (`0x080099D8`); the throne room loads with the Emperor and Gale (ROM `0x0866815C`) and the opening runs (task `0x080173E0`): Gale counsels the Emperor, who puts his loyalty to the test (`0xFC`, `0xFD`); in the portal room (123) the soldiers see the device in use (`0xFE`, after a 64-pixel pan); in the room above the base's bar (129) Jack brings the news (`0xFF`) and the party leaves | `0x16D` |
| 94 | `0x08016D20` | `0x16D`, not `0x16E` | Through the portal (task `0x08017648`): the portal brings the Gustav (`0x102`); three blasts (sound `0x5B`, each waited out) and `0x103`; the Ark base in ruins (101, `0x104`), someone slips away (`0x105`); on the plains Jack knows him, Gale, his old instructor (`0x106`, `0x107`) | `0x16E` |
| 103 | `0x08016E78` | `0x16E`, not `0x16F` | Fiene in the ruins (task `0x08017938`, `0x108` to `0x10D`); she joins (list 9) | `0x16F` |
| 104 | `0x08016EF4` | `0x16F`, not `0x170` | The Bego base (task `0x08017C3C`): the First Armoured Division surrounds the Gustav, three Zoids circling it while the scene goes on (`0x08017E90`, `0x08017F6C`, `0x08018048`), Schwarz calls on it to surrender and Fiene comes (`0x10E`, `0x10F`); in the hall (105) Schwarz joins (`0x110`, list 10). A beaten party is taken to the base from then on (return point 11) | `0x170` |
| 106 | `0x08016F74` | `0x170`, not `0x171` | The Zeta base: the view pans 256 pixels across the ruins (task `0x08018124`, `0x111`) | `0x171` |
| 106 | | `0x171`, not `0x172` | Van's Zoid stands by the base (object 1 at (20, 2)); speaking to it (`0x08017014`) runs task `0x08018188`: Van joins (`0x112`, list 11) | `0x172` |
| 97 | `0x08016DC4` | `0x172`, not `0x173` | The house in the town (task `0x08018224`): the party comes in one by one, Irvine and Moonbay tell of the raiders heading for Deme (`0x113`); Irvine gives Zi data `0x48`, announced as a chest's (`0x08037A24`: dialogue `0x1F`, the Zi data's line, `0x22`); the pair leaves and the party sets off (`0x114`) | `0x173` |
| 107 | `0x080170E8` (object 1) | | The Deme base's guard: before Schwarz joins `0x115`; after it `0x116` | `0x174` |
| 107 | `0x08017048` | `0x173`, not `0x175` | Thomas's Dibison at the gate, to Gale's song (task `0x0801851C`, `0x117`); story battle 8 follows from the hook `0x08018550`; won, the party is taken to the side road (131). Once Thomas is beaten the guard is gone | `0x175` on winning |
| 131 | `0x08017138` | | Thomas comes to (task `0x080185AC`, `0x118`) and leads the way; back to the gate | |
| 111 | `0x080171AC` | `0x175`, not `0x176` | The hook `0x08017208` waits for the player's sprite at x `0x540` to `0x560`, y `0x200`: in the passage (132) Hiltz and Reese mock the party and slip away (task `0x08018650`, `0x119`); Thomas, Van, Fiene and Schwarz give chase and leave the party (`0x11A`, `0x11B`, lists 9 to 12), and the others follow (`0x11C`); the hook `0x0801889C` brings the party back to the yard | `0x176` |
| 111 | | `0x176`, not `0x177` | On the same spot (hook `0x08017264`): Gale waits (task `0x08018AA8`, `0x11D`) and, to the duel's song (1), challenges Jack (`0x11E`); story battle 9 follows from the hook `0x08018DB0`; won, the chapter's end (task `0x08018B74`): Gale gives way (`0x11F`, `0x120`), Van, Thomas and Schwarz come back (`0x121`), the party goes home (`0x122`) through the plains' portal (ROM `0x08668454`), and the hook `0x08018D80` warps to map 165, chapter 5's first | `0x177` on winning |

The port runs the yard's hooks before the field's update (see Tasks above).

The teachers use `0x08012090`: deck commands 25 (`0x08006AC8`), 6, 27, 31, 28 and 14
(`0x08006AE0` to `0x08006B3C`), and 32 (`0x08009600` through `0x08009430`). The keepers
(`0x08009198` on): item shops 5, 6 and 7, armaments shops 6, 7 and 8, and the labs 5 and
19. Dr. T (`0x0802AB08`) has his own lab in the area (map 127); see Teachers above for
what he says outside area 1.

What the chapter needed of the engine: a gift announced as a chest's (the reward the
chest's announcement reads comes first from the event), and branches on a Zoid owned
(a unit slot whose half at `+6` is the Zoid) and on a game-state byte, for Dr. T.

Checked against the original, scene by scene from patched saves (the frames of each
song, sound and dialogue, the brightness, and the pictures between dialogues): the
opening, the portal and the plains, Fiene, the Bego base (the circling Zoids' positions
equal on every frame), the Zeta base's pan, Van, the house and the gift, Thomas to story
battle 8, the side road, Hiltz and Reese, Gale to story battle 9, and the chapter's end,
with Gale's hit points kept at 1 and the party's at full: every interval that no key
bounds is equal, and the scenes' darkenings fall on the same frames. What differs is the
loads (the original's continue loads a map twice when its handler loads a cutscene list,
and its warps take up to 20 frames longer, see Not modeled yet) and, after them or after a
dialogue that keys end, the phase of the animations that were already running. On the yard,
continuing a patched save on the spot, the hook takes the control from the party in the
first field frame, as the original's does, before the roaming enemy beside the spot can
step in.

## Chapter 5

Source of knowledge: own reading of the area's map handlers, the tasks and field hooks
they install, the objects' code, the match task at `0x0801A3DC` and the routines named
below; checked against a reference emulator with saves patched to each scene's flags,
as the earlier chapters'. Implemented in `crates/game-core/src/story/chapter5.rs` and,
for the colosseum's data, `crates/extraction/src/saga_arena.rs`.

Area 5 is maps 133 to 175: the colosseum's district (133), a Zoid map with the portal,
Dr. Tros's rooms (134 to 136), the South, East and Main domes' halls (143, 146, 149) and
desks (144, 147, 150), the arena (152), and the Emperor's castle and the kingdom's base
again (165 to 175). The chapter's story:

| Where | Handler | Condition | What happens | Sets |
|---|---|---|---|---|
| 165 | `0x08019B04` | not `0x178` | The characters of group 4 are met (`0x080099E4`); the throne room loads with the Emperor and Blood (ROM `0x08668558`) and the opening runs (task `0x08019BEC`): Blood hears of Zoid battles (`0x123`); the soldiers sense the device (168, `0x124`); Jack brings the news (174, `0x125`) | `0x178` |
| 133 | `0x08018EEC` | `0x178`, not `0x179` | Into the battle field (task `0x08019DE4`): the portal brings the Gustav (`0x126`); shells fall three times around it (`0x08016CA8`, sound `0x5B`) and three more as it drives on (`0x127`); the judge calls the battle off (`0x128`); Blood's team (`0x129`, `0x12A`) and Team Blitz (`0x12B`) drive up; at Dr. Tros's (135, 136) the party hears of the colosseum and of the missing rankers, and Team Blitz joins (`0x12C`, `0x12D`, lists 13 to 15) | `0x179` |
| 133 | | `0x179`, not `0x17A` | Team Blitz tells how to reach the champion's final (task `0x0801A2DC`, `0x12F`); a beaten party is taken to Dr. Tros's from then on (return point 12) | `0x17A` |
| 133 | `0x08018FCC` (object 1) | | The Zoid Federation's Ultrasaurus orders the party away (`0x16C`); any answer but the first starts story battle 25 from the hook `0x08018FFC` | |
| 144, 147 | `0x080191E4`, `0x08019518` (the desks) | | Before entering, the desk offers its dome's tournament (`0x130`, `0x14D`); entered, the dome keeps the party in and is the return point (13, 14); then it starts the next match, or explains its regulation; once the dome is won it sends the party elsewhere (`0x131`, `0x14E`) | `0x17B`, `0x183` |
| 143, 146, 149 | `0x0801907C`, `0x080193B4`, `0x080196E8` | entered, dome not won | The hooks `0x080190B0`, `0x080193E8`, `0x080197B0` watch the hall's doorway (x `0x158` to `0x178`, `0x148` to `0x188` in the Main hall, y `0x1B0`): the desk's warning (`0x132`, `0x14F`, `0x170`); leaving clears the tournament's flags and takes the party outside, staying brings it back in | |
| 150 | `0x080198E0` (the desk) | | Once the South and East domes are won, the Main dome's tournament (`0x16D`), the first time with Bit and Ballad (task `0x0801AEDC`, `0x16F`: Bit joins, list 20, and Ballad if taken, list 21, flag `0x190`); return point 15; otherwise `0x16E` | `0x193`, `0x189` |
| 149 | `0x0801973C` (object 1) | entered, `0x190` not set | Ballad offers his services (`0x171`); taken, he joins and the hall reloads without him (hook `0x0801977C`) | `0x190` |

The matches (task `0x0801A3DC` with `n`, table at ROM `0x08668E7C`): the desk asks
(`question`); the South dome's last match brings Naomi first (`0x144` to `0x146`), the
East dome's Harry's team (`0x161` to `0x164`), the final Leena's thanks (`0x184`; the
original sets flag `0x190` there where it means to test it, so Ballad has joined from
then on); the arena (map 152) loads with the match's enemies and judge and a Zoid for
each member of the formation (see `saga_arena`); the judge opens; story battle 10 + `n`
follows from the hook `0x0801AE30`. Lost, the party goes back to the desk; won, the
match's flag is set (`0x17D`–`0x181`, `0x184`–`0x188`, `0x18A`–`0x18E`) and the judge
closes (ROM `0x086690AC`). The South dome's last win brings Naomi's team (`0x149`): she
joins (`0x14C`, list 16) unless the East dome was won first, when Harry shows up
(`0x14A`, `0x14B`); the East dome's brings Harry's team (`0x167`, `0x168`): they join
(`0x16B`, lists 17 to 19) unless the South dome was won first, when his friends take him
away (`0x169`, `0x16A`). Flags `0x182` and `0x17C` keep which dome was won first, and
`0x191`, `0x192` that each scene has run.

The desks check the matches' regulations (`0x08038654`, routines at ROM `0x0867E404`):
the formation's filled slots at most 4 (matches 2 and 7), 3 (12 and 14) or 2 (13); at
most 3, all flying (3), all small (4), all medium (8), one large (9), or all of the
Liger, Tiger and Wolf types listed at ROM `0x0867E440` (11); anything for the rest.
The size class and the flying flag are the Zoid record's `+4` and `+0` bit 2.

The final won (`0x0801AD18`): to the duel's song Blood gives up the fight but not the
Trinity Liger (`0x187` to `0x189`); the hooks stage battle scenes 2 and 3 to the
Liger's song (`0x08012040`, see [battle.md](battle.md)) between the broken Liger and
Blood's escape (`0x18A`); the farewells follow (task `0x0801B108`: `0x18B`, `0x18C`,
Dr. Tros's Zoid core `0x14` announced as a chest's, `0x080379B4`), lists 13 to 21
leave, and the hook `0x0801B224` warps to map 205, chapter 6's first.

The teachers use `0x08012090`: deck commands 18, 5 and 30 (`0x08006B50` on), and 20
(`0x08009570` through `0x08009430`). The keepers (`0x080091F8` on): item shops 8 and 20,
armaments shops 9 to 16, and the labs 6 to 10.

What the chapter needed of the engine: the regulation check and the arena's load
(`Op::IfRegulation`, `Op::LoadArena`), a warp that keeps the player's cell and facing,
and staged battle scenes played as attacks.

Checked against the original, scene by scene from patched saves: the opening, the
arrival with its shelling, Team Blitz's talk, a desk's offer and a match to its battle
(the arena's load and the judge's lines on the original's frames), the regulations'
refusals, the South and East domes' last matches and their scenes, the leaving hook,
the Ultrasaurus, Ballad, and the final to chapter 6's first map, with the enemies'
hit points kept at 1 in the original and the protagonist overpowered in the port, the
formation cut to three and its units given 9999 hit points on both sides. Every
interval that no key bounds is equal but for the loads and the staged scenes' first
shots (see [battle.md](battle.md)).

## Chapter 6

Source of knowledge: own reading of the area's map handlers (`0x0801B284` to
`0x0801BF90`), the tasks and field hooks they install, the objects' code and the
routines named below; checked against a reference emulator with saves patched to each
scene's flags, as the earlier chapters'. Implemented in
`crates/game-core/src/story/chapter6.rs`.

Area 6 is maps 176 to 216: the plains (176), a Zoid map with the portal; the town below
Miletos castle (177), the castle's grounds (178) and hall (179); Hagen City (182), its
bar (183), the Zoid institute's gate (186) and hall (188); the ruins (194, whose scene
loads their copy 216); Gray Colony (195); the hidden lab's door (198), passage (199) and
rooms (200); and the Emperor's castle and the kingdom's base again (205 to 215). The
chapter's story:

| Where | Handler | Condition | What happens | Sets |
|---|---|---|---|---|
| 205 | `0x0801BF40` | not `0x194` | The characters of group 5 are met (`0x080099F0`); the opening (task `0x0801C028`): Blood and Obscura report (`0x18D`), Gale is sent for the Death Saurer's data (`0x18E`, `0x18F`); the soldiers sense the device (213, `0x190`); Jack brings the news (214, `0x191`) | `0x194` |
| 176 | `0x0801B284` | `0x194`, not `0x195` | The portal brings the Gustav (task `0x0801C35C`, `0x193`); return point 16 | `0x195` |
| 177 | `0x0801B584` | `0x195`, not `0x196` | The view sweeps the festive streets, two pixels a frame for 192 frames (task `0x0801C47C`, `0x194`) | `0x196` |
| 177 | | not `0x19C` | The hook `0x0801B618` (x `0x98` to `0xD8`, y `0x60`): to song 6 the party spots Gale (task `0x0801C4E0`, `0x19A` to `0x19C`) and follows him to the castle's grounds (178, `0x19D`) | `0x19C` |
| 179 | `0x0801B680` | `0x19C`, not `0x19D` | The hook `0x0801B6B4`: from the west doorway (x `0x28`, y `0x30` to `0x50`, task `0x0801C684`) or the east one (x `0x1B8`, task `0x0801CA80`, which keeps the map's song) the party walks to the throne, where Rosso's band attacks Rudolph (`0x19E`), a shot strikes Rosso twice (sound `0x7D`, `0x19F`) and the band carries the boy off (`0x1A0`, `0x1A1`); on the plains the party wonders about him (`0x1A2`) | `0x19D` |
| 182 | `0x08006BAC` on (objects) | | Hagen City's people: `0x1A3` to `0x1AA` until `0x19D`, `0x36C` to `0x373` after | |
| 183 | `0x0801B7A8` | `0x19D`, not `0x19E` | Stinger sizes the party up in the bar (task `0x0801CE90`, `0x1AB`, `0x1AC`) | `0x19E` |
| 176 | | `0x19E`, not `0x19F` | The hook `0x0801B45C` (x up to `0x280` on y `0x140`, or (`0x280`, `0x180`)): the ambush (task `0x0801D328`): the plains load again around the party, gas bursts around it from three helpers (`0x0801D500`, `0x0801D52C`, `0x0801D558`, `0x08016CA8` near the player's cell) while it wonders (`0x1B0`), Stinger drives up (`0x1B1`, `0x1B2`) and story battle 26 follows (hook `0x0801D42C`); won, she gets away (`0x1B3`) | `0x19F` |
| 183 | | `0x19F`, not `0x1A0` | Stinger caught in the bar (task `0x0801D1C8`, `0x1AD` to `0x1AF`) | `0x1A0` |
| 176 | | `0x19F`, not `0x1A1` | The hook `0x0801B4C4` (`0x360`, `0xA0`): the wrecked Iron Kongs, to the danger song (task `0x0801D594`, `0x1B4`) | `0x1A1` |
| 194 | `0x0801BA40` | `0x1A1`, not `0x1A2` | Rosso's band in the ruins (task `0x0801D5E4`): Rosso and Viola hurt (`0x1B5`, `0x1B6`); Rosso asks to come along (`0x1B7`): taken, they join (`0x1B8`, lists 22 and 23, flag `0x1A4`), refused, they leave (`0x1B9`, `0x1BA`); further in, too late (`0x1BB`) | `0x1A2` |
| 176 | | `0x1A2`, not `0x1A3` | The hook `0x0801B524` (`0x2C0`, `0xE0`): Raven's attack (task `0x0801D9A0`): blasts beyond the ridge (`0x1BC`), battle scene 20, the Wolves blow up (`0x1BD`, `0x1BE`); with Rosso, he gives a Zi data (`0x5B`) and leaves with Viola (`0x1BF`), else the beam flashes (`0x1C1`); the agent flees; to song 5 the view sweeps to Van facing Raven (`0x1C2`, `0x1C3`), battle scenes 4 and 5, `0x1C4`, `0x1C5`, battle scene 6, Fiene cries out (`0x1C6`) and Dr. D comes (`0x1C7`) | `0x1A3` |
| 195 | `0x0801BAB8` | `0x1A3`, not `0x1A5` | Gray Colony talks of a lab in the mountains (task `0x0801DF24`, `0x1C8`) | `0x1A5` |
| 198 | `0x0801BB40` | `0x1A5`, not `0x1A6` | To the danger song the view climbs to the lab (task `0x0801DF6C`, `0x1CE`); before `0x1A5`, the party backs off to the plains (task `0x0801DFF8`, `0x1CD`) | `0x1A6` |
| 199, 200 | `0x0801BC04`, `0x0801BE94` | | The guards (objects 4 and 5 of 199, 1 to 5 of 200) look every frame (`0x0801BC2C`, see [field.md](field.md), Guards); a guard that sees the player stops, the player turns to it and the party is put out at the door (task `0x0801BD9C`, `0x1CF`) | |
| 200 | | `0x1A6`, not `0x1A7` | The hook `0x0801BED8` (`0xE8`, `0x60`): the wrecked room (task `0x0801E20C`): a scientist tells of the Death Saurer (`0x1D0` to `0x1D2`; `0x08011EAC` moves an object a pixel a frame) | `0x1A7` |
| 176 | | `0x1A7`, not `0x1A8` | To the enemy's song Raven finds Gale (task `0x0801E044`, `0x1D3`, `0x1D4`), battle scene 7 (Gale's smoke), and Gale calls Jack to Hagen City's institute as Raven chases him (task `0x0801E0F8`, `0x1D5` to `0x1D8`) | `0x1A8` |
| 186 | `0x0801B8C4` | `0x1A8`, not `0x1A9` | The party walks into the institute (task `0x0801E48C`, `0x1DA`); from then on the gate's soldier is gone | `0x1A9` |
| 188 | `0x0801B960` | `0x1A9`, not `0x1AA` | The hall loads where the party stands with Prozen; the hook `0x0801B9B8` (`0x78`, `0xB0`): Prozen shows the Death Saurer's prototype and leaves (task `0x0801E5E8`, `0x1DB` to `0x1DE`); on the plains story battle 27 (hook `0x0801E7BC`) | `0x1AA` |
| 176 | | `0x1AA`, not `0x19B` | Gale comes (`0x0801E848`, `0x1DF`, `0x1E0`), the first time with Van and Irvine (`0x1E1`, lists 24 and 25, flag `0x19A`); story battle 28 (hook `0x0801E970`); won, they part (task `0x0801E9E0`, `0x1E2` to `0x1E5`, lists 24 and 25 leave) and the hook `0x0801EAA8` warps to map 225, chapter 7's first | `0x19B` |

Raven's beam flashes the field white (`0x0801D93C`, `0x0801D968`): a callback sets
`BLDCNT` to `0xBE`, every layer brightened toward white, and `BLDY` rises a level every
other frame to 16 while the beam's sound (`0x81`) plays, then falls back to 0; the task
waits for it. The staged scenes run through `0x08012040`, which plays the Liger's song,
the scene or the `0xFF`-ended list of scenes, and the song that played before (see
[battle.md](battle.md)).

The hooks at `0x0801B45C`, `0x0801B4C4` and `0x0801B524` set bit 1 of the field's state
halfword (`0x02000008`), so no roaming enemy meets the party while their scenes run;
the wrecks' task clears it, and otherwise the next map's handler does: every handler
starts with `0x0800BEE4`, which clears the halfword, or `0x0800802C`, which clears it
unless its bit 0 is set.

The teachers use `0x08012090`: deck commands 23 and 8 (`0x08006B98`, `0x08006CD4`), and
3 (`0x080094A4` through `0x08009430`). The keepers (`0x080092AC` on): item shops 9 to
12, armaments shops 17 to 19, and the labs 11 and 20.

What the chapter needed of the engine: the enemies kept away (`Op::Calm`), the white
flash (`Op::Whiten`), the guards' sight (`Op::IfSeen`, `Op::FaceSeenGuard`), objects
placed and glided relative to the player's cell (`Op::PlaceNearPlayer`,
`Op::GlideNearPlayer`), animations stopped at their end (`Op::Once`), and a staged
scene's reaction line.

Checked against the original, scene by scene from patched saves: the opening, the
arrival, the town's view, Gale's sighting, both ways into the castle's hall, both bar
scenes, the ambush to Stinger's battle (her escape on each side's own run, the battle
lasting longer in the port), the wrecks, Rosso's band, Raven's
attack with and without Rosso in the party, Gray Colony, the lab's door both ways, the
wrecked room, a guard's catch, Raven and Gale, the institute, Prozen and the prototype's
battle, and Gale's battle to chapter 7's first map, with the enemies' hit points kept at
1 in the original and the protagonist overpowered in the port, the units given 9999 hit
points on both sides for Stinger's battle. Every interval that no key bounds is equal
but for the loads, the staged scenes' first shots, and the guards' wandering after a
continue (the random numbers' state then differs, see [field.md](field.md)); a guard
sees the player on the frame after it stands, as in the original.

## The end of the demo (a port feature)

Source of knowledge: this project's own design. The port's story stops where chapter 6
does, in chapter 7's first map (225) once Gale is beaten (flag `0x19B`).
When
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
| 27, Dr. T's lab | `0x08009108` | The Zoid lab (kind 2 of the shops): revival, development, pilot change and sale, and the repair when it closes (see [shop.md](shop.md)) | |

## Not modeled yet

- The fighting in battles, and the story battles (`0x08008D28`); the roaming enemies,
  meeting them, the battle screen's opening, its menu and retreating are described in
  [combat.md](combat.md).
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
- Chapters 2 to 6: the time the original's loader takes for each map (the cutscene
  loads and warps of Sand Colony, the deserts, the ruins and the later chapters' Zoid
  maps take 10 to 30 frames longer than the port's estimate, see "Chapter 2" to
  "Chapter 6" above), and the page turns of a message under keys pressed every other frame, which
  the original takes two frames longer to accept.
- A portal's wait for the cell below it to clear before it brings someone out (see
  [field.md](field.md), Portals).
- A one-frame drift of the backdrop.
