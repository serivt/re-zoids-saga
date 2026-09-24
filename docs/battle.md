# Battle scenes

Source of knowledge: own reading of Zoids Saga (Japan, Rev 1) at the addresses named
below, and traces of the two battle scenes of the opening in a reference emulator:
- breakpoints on the game's LZ77 and `CpuSet` wrappers (`0x0805D3A0`, `0x0805D388`) and
  on the script runner (`0x0803E51C`);
- a write watchpoint on the scroll registers;
- per-frame logs of the scroll shadows, the brightness and the per-scanline table;
- VRAM, palette, OAM and RAM dumps, and screenshots every four frames.

Implemented in `crates/extraction/src/saga_battle.rs` (data) and
`crates/game-core/src/battle.rs` (the scene). The shots were traced from the entity
table (IWRAM `0x03004BBC`) and OAM frame by frame, and the sprite drawer at `0x08000560`
was read to place them.

## Staging a scene

`0x08008E4C(n)` stages battle scene `n`. It hands over to `0x08003590`, which starts the
battle module (task `0x0803DC54` in slot 4) on the 44-byte record `n` of the table at
ROM `0x66429C`, and waits until the module reports the end. The game then reloads the
map. The module is the real battle engine running a scripted attack: the enemy unit rolls
in, its pilot speaks, and it fires.

The opening uses these record fields:

| Offset | Content |
|---|---|
| `0x11` | Scenery |
| `0x14` | The enemy's Zoid |
| `0x15` | Its pilot |
| `0x16` | Its quote: string `172 + n` of the `battle` table (`0x08042916`) |

| Scene | Enemy | Pilot | Quote |
|---|---|---|---|
| 0 | Zoid `0x49`, コマンドウルフ市街戦用 | `0x3A`, 共和国軍人 | 173: 攻撃開始！ |
| 1 | Zoid `0x09`, レッドホーンＢＧ | `0x3B`, 帝国軍人 | 175: 共和国に遅れを取るな！ |

## Graphics

Battle images are 128×128 pixels: 256 tiles of 8bpp and 64 colors, LZ77-compressed.
The data below was checked byte for byte against the VRAM and palette RAM the scene
left.

| What | Tiles | Palette | Loaded to |
|---|---|---|---|
| Scenery | pointers at ROM `0x6F6968` by scenery | `0x6F6BF0` | char block 2, palette 64–127 |
| Zoid | first of three pointers per Zoid at ROM `0x6F8974` | `0x6F9100` | char block 1, palette 0–63 |
| Window frame | `0x3B9428`, the field's window skin | | char block 3 |
| Portrait | the dialogue portraits (see [formats/portrait.md](formats/portrait.md)), expression 0 | | OBJ tiles 988–1023, palette 15 |

The screen, from back to front:

- **BG2:** the scenery. The 128×128 image is drawn twice across a 256×256 map. Every map
  entry flips its tile and the columns run in reverse, so the image shows mirrored.
- **BG1:** the Zoid, the same way, once, in the corner of a 512×256 map.
- **BG0:** two light-framed windows. The portrait's is at tile (0, 14), 6×6. The
  message's is at (6, 16), 24×4, and the quote is typed into it a character a frame.
- **Sprites:** the pilot's 48×48 portrait at (0, 112), in four pieces (32×32, 32×16,
  16×32, 16×16), over its window.

## Motion

BG2 scrolls per scanline. An `HBlank` handler (IWRAM `0x03005BB0`) writes each line's
entry of a table of 16.16 values at EWRAM `0x0200DAAC` into the horizontal scroll. It
writes the entry at the end of a line, so a line shows the entry of the line above.
Each frame the module adds to each entry:

- the sky (entries 0–71): `0x1000`, 1/16 pixel;
- the forest (entries 72–112): `0x4000`;
- the ground: `0x6800` on entry 113, `0x800` more on each entry to 127;
- the rest: `0x1000`, like the sky.

The entries start at −`0x2000`.

BG1's scroll (the shadows at IWRAM `0x03004B9C`, copied to the registers each `VBlank`)
starts at 176. The Zoid then slides in as `176 − t²/8`, `t` counting frames. Each shot
pushes it back 4, 2 and 1 pixels, two frames apart. The screen shows the scroll and the
brightness the module set the frame before.

## Shots

Each shot spawns effect sprites, entities of the sprite system like the map's
characters. Their records are the 282 16-byte entries at ROM `0x6F77D4`:

- LZ77-compressed 4bpp tiles;
- an LZ77-compressed palette;
- a table of animations;
- a table of frames.

A frame is a list of 20-byte pieces, ended by a tile of `0xFFFF`:

| Offset | Content |
|---|---|
| 0 | First tile, relative to the sprite's; the top four bits add to the palette |
| 2 | Flips (low two bits) and a rotation (high byte) |
| 4, 6 | Offset of the piece's top-left corner from the anchor |
| 8, 10 | Width and height; the drawer picks the OAM shape and size from them |
| 12, 14 | Horizontal and vertical scale, 8.8 |
| 16 | `0xFF` for a plain piece; otherwise affine, with `0x200` for the double-size box |

The drawer (`0x08000560`) builds the OAM entries from these pieces:

- **Position:** the entity's anchor minus its layer's scroll, plus the piece's offset.
  The enemy's entities are mirrored (flag `0x100`), so the offset becomes
  `−x − width` (less another width for a double-size box).
- **Plain pieces:** flipped once more when the piece asks for it.
- **Affine pieces:** the drawer sets `ObjAffineSet` with the reciprocal of each scale
  and the piece's rotation, then negates the first entry for the mirroring.
- **Culling:** it hides an anchor more than 264 pixels right or 160 below.

The OAM copy, like the scroll, shows on the next frame. The scenes' effects are all
semi-transparent, drawn under the windows with 15/16 of the sprite over 8/16 of the
layer below (`BLDCNT` `0x1610`, `BLDALPHA` `0x080F`).

| Scene | Sprite | Kind |
|---|---|---|
| 0 | 188 | Muzzle flash, two 32×8 pieces, 28 frames |
| 0 | 189 | Ring, affine, its scale growing frame by frame, 36 frames |
| 0 | 167 | Bullet, 16×8 |
| 1 | 183 | Muzzle flash |
| 1 | 156 | Round |

Effect anchors are on BG1, so they follow the Zoid's recoil.

A flash and a ring stay until their animation ends, and the animation starts a step
in. A round flies right 24 pixels a frame:

- in scene 0 it flies from the frame it appears, and lasts 9 frames;
- in scene 1 it waits a frame first, and lasts 10.

Each shot plays a sound effect in the frame its flash spawns (the module calls
`0x080019EC`): 123 for the Command Wolf, 89 for the Red Horn. Nothing else sounds
during the scenes; the song is the cutscene's.

The Command Wolf fires four times: at frames 180 and 191 from (91, 55) and (91, 66),
and at 216 and 227 from (69, 54) and (69, 65). Each shot is a flash, then a ring a
frame later, then a bullet a frame after that. The Red Horn fires ten times, every 11
or 12 frames from frame 186, its barrel moving between four positions. Each shot is a
flash, then a round a frame later, 2 pixels right and 5 up. The port transcribes these
spawns from the entity table.

## Timeline

Frames are counted from the first frame the scene shows. Scene 1 runs a frame behind
scene 0 from the fade-in on.

| Frame | Scene 0 | Scene 1 |
|---|---|---|
| 0 | The scene replaces the map; black | Same |
| 11 | The scenery's table starts moving | 12 |
| 20–27 | Fade in, two levels a frame | 21–28 |
| 29 | The Zoid starts sliding in | 30 |
| 159 | The quote starts | 160 |
| 164 | The module loads the shot's graphics and the scenery's table stays put for a frame | |
| 180 | First shot | 186 |
| 181, 192, 217, 228 | Shots, each with its recoil | (none) |
| 298 | Fade out, two levels a frame | 288 |
| 310 | The map returns; 9 frames of reload follow | 300 |

## Checked against the original

Frames sampled every four frames compare pixel for pixel with the original:

- **Scene 0:** all 77 are identical.
- **Scene 1:** 83 of 84 are identical, once shifted by the one frame its start drifts.
  That drift comes from the dialogue before it: the script operations' frame costs vary
  in the original.

Compared frame by frame, 223 of scene 0's 241 frames are identical. The others differ
by a line or two in the scenery's bands. The module rewrites the scroll table while the
frame is drawn, so the lines drawn before it show the frame before's values:

- **Normally:** the rewrite lands about 16 lines in, which the port models.
- **During the shots and when the quote starts:** the extra CPU load moves the rewrite
  lower, which the port does not model.

The port requests the shots' sounds in the same frames as the original: 3076, 3087, 3112
and 3123 of the traced run for scene 0, and the ten of scene 1 at the same frames from
its start.

## Not modeled yet

- The rewrite line of the scroll table under CPU load.
- The battle engine itself: the port transcribes these two scenes' timelines and shots
  rather than running the module's attack logic.
