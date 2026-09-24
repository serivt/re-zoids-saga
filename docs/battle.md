# Battle scenes

Source of knowledge: own reading of Zoids Saga (Japan, Rev 1) at the addresses named
below, and traces of the two battle scenes of the opening in a reference emulator:
- breakpoints on the game's LZ77 and `CpuSet` wrappers (`0x0805D3A0`, `0x0805D388`) and
  on the script runner (`0x0803E51C`);
- a write watchpoint on the scroll registers;
- per-frame logs of the scroll shadows, the brightness and the per-scanline table;
- VRAM, palette, OAM and RAM dumps, and screenshots every four frames.

Implemented in `crates/extraction/src/saga_battle.rs` (data) and
`crates/game-core/src/battle.rs` (the scene).

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
| 181, 192, 217, 228 | Shots, each with its recoil | (none) |
| 298 | Fade out, two levels a frame | 288 |
| 310 | The map returns; 9 frames of reload follow | 300 |

## Checked against the original

Frames sampled every four frames compare pixel for pixel with the original:

- **Scene 0:** 63 of 77 are identical. The other 14 are the two bursts of shots.
- **Scene 1:** all are identical but for the shots, once shifted by the one frame its
  start drifts. That drift comes from the dialogue before it: the script operations'
  frame costs vary in the original.

## Not modeled yet

- The shots and their effects (sprites with alpha blending over the scene).
- The shots' sounds.
- The battle engine itself: the port transcribes these two scenes' timelines rather
  than running the module's attack logic.
