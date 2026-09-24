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
| Taking an exit, arrival at frame R | Level 1 at R+2 … level 31 and the load at R+32; black until R+42; level 30 at R+43 … 0 at R+73; the world runs at R+74 |
| Cutscene load, event warp | 6 frames plus one per object |
| Exit onto the world map (map 1) | Level 30 six frames later than a room's: the load takes longer |
| Input | The game acts on the buttons of the frame before |
| Display | A frame shows the field, the windows and the brightness as the frame before left them (see [field.md](field.md), Drawing) |

Script operations cost frames too (see [formats/script-text.md](formats/script-text.md)).
The original's real costs vary with the CPU time each frame takes. For example, a window
reset took 10 frames after a battle instead of the usual 3. The port uses the usual
costs.

## Chests

A chest is an object of behavior 4 whose script reference holds the chest number in its
low half-word (`0x80000000` is chest 0). Its opened flag is `0x1E + n`. The treasure
table at ROM `0x0866BCE4` has 12-byte records: money (4 bytes), a Zoid (2 bytes), an
item (2 bytes) and two more bytes.

Opening one plays sound `0x46` for `tb00` or `0x48` for `tb01` and switches the chest to
its open animation (1). After 30 frames it sets the flag and shows the message:

- dialogue `0x1F`;
- battle-menu strings 6 and 7;
- the amount;
- battle-text string 10 (Ｇ手に入れた);
- battle-menu string 5;
- dialogue `0x22`.

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
| Map 11, the exit | Reaching column 2 on row 2 or 3 walks the Gustav to (1, 3), plays dialogue `0x2C0` and sees the Trinity Liger (Zoid `0x8F`) | `0x127` |
| Map 1, the world map | The first time (handler `0x08010358`) the Gustav faces right and stands still; its task (`0x080103B4`) waits 60 frames from the end of the fade in, plays dialogue `0x44` (Regina: to the nearby town of Arcana), walks the Gustav at a pixel a frame to (11, 6), then (11, 7), then toward (14, 7), and ends 60 frames after starting that walk. The Gustav runs into the town's door at (14, 7) and takes it into map 24 at (23, 29) | `0x11E` |
| Map 24, Arcana | The first time (handler `0x0800E850`) the town reloads with the arrival's objects (ROM `0x0832AC54`: the prince, Regina, Ace, Jack, Roman and three soldiers), song 6 plays, and the arrival task (`0x0800E8B8`) starts: the party splits up (dialogue `0x49`; helper tasks `0x0800FBA0`, `0x0800FE14`, `0x08010088` walk Jack, Ace and Regina around town), the prince finds the bar (`0x4A`), the soldiers surround the party (`0x4B`), Roman comes out (`0x4C`, `0x4D`), the soldiers leave and everyone goes into the bar (`0x4E`) | `0x128` |
| Map 29, above the bar | The same task loads the room with its own list (ROM `0x0832ACF4`): Roman's account (`0x4F`); the prince and Regina leave by the stairs, the camera pans 64 pixels left, Jack and Ace follow (`0x50`); the task warps to map 28, the bar, at (13, 12) facing left | |
| Map 29, Roman | Teaches 包囲攻撃, deck command 26 | |
| Map 25, item shop | The keeper behind the counter opens item shop 1 (`0x080090F0`, see [shop.md](shop.md)) | |
| Map 26, armaments shop | The keeper opens armaments shop 1 (`0x080090FC`); the old man teaches 節電, deck command 22 | |
| Map 27, Dr. T's lab | Dr. T (`0x0802AB08`) talks with Regina about rebuilding the Trinity Liger (dialogue `0x2C1`), later `0x2C2`; the assistant on the left teaches データ収集, deck command 0 | `0x13F` |

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
- The black after an exit's load depends on the scene. The port uses one length for
  every room: leaving Arcana's bar for the streets, the original stays black six frames
  longer, and going up to the room above, two frames shorter.

## Not modeled yet

- The fighting in battles, and the story battles (`0x08008D28`); the roaming enemies,
  meeting them, the battle screen's opening, its menu and retreating are described in
  [combat.md](combat.md).
- Dr. T's Zoid lab (`0x08009108`, kind 2 of the shops, `0x080090C8`) and the other
  towns' shops: speaking to their keepers does nothing yet (see [shop.md](shop.md)).
- Dr. T in the other areas: whether the party has the Zoids `0x90` or `0x8F`, and the
  game-state byte `+0x3320` (flags `0x140`, `0x141`).
- The field's per-frame hooks (RAM `0x02000000`, `0x02000004`), which the world map's
  and Arcana's handlers set. The ones seen so far (`0x08008024`, `0x08008028`,
  `0x0800C4D0`, `0x0800C708`) return at once; the world map's battles come from its
  roaming enemies instead (see [combat.md](combat.md)). The world map's handler swaps
  them for empty ones during the drive and leaves the second one empty. Arcana's
  arrival sets its own and puts the field's back when it ends; after it, Arcana's
  handler (`0x0800BEE4`) only sets them.
- The port runs the tasks a map handler spawns two frames after its fade in ends,
  where the original runs them in that frame; the drive to Arcana makes up for it.
- The CPU-time variance of script operations.
- The object-state overlay.
- A one-frame drift of the backdrop.
