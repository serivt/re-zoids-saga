# Maps and warps

Source of knowledge: own analysis of Zoids Saga (Japan, Rev 1) in a reference emulator
(RAM dumps before and after walking through the first room's exit, and a write watchpoint
on the decompressed map buffer that led to the scene loader) plus a read of the loader,
the exit handler and the warp lookup in the game's code. Implemented in
`crates/extraction/src/saga.rs` (`map_record`, `warp`, `Scene::exit`).

## Map records

A map is a scene plus the data the game attaches to it. 343 records of 28 bytes at ROM
`0x31B27C`, terminated by a record whose id half-word is `0xFFFF`:

| Offset | Field |
|---|---|
| 0 | Scene index (see [scene.md](scene.md)) |
| 2 | Map id with bit 15 set on some records; the game searches records by `id & 0x7FFF` |
| 4 | Map tiles per attribute cell side: 2 for rooms and towns, 4 for the world map |
| 6 | Unknown half-word |
| 8 | Unknown word |
| 12 | Code pointer: a per-map handler (not modeled) |
| 16 | ASCII name, zero-padded to 12 bytes: `START`, `mq0100`, `md0151`… |

The first room of the game is record 4, `md0153`, scene 3. Its neighbours `md0151`–`md0155`
are the other rooms of the same building.

## Warp tables

At ROM `0x31FD84` one pointer per map record, in map order, to that map's warp table: a
list of 12-byte entries indexed by the exit number carried in the scene attributes.

| Offset | Field |
|---|---|
| 0 | Always `0xFFFF` |
| 2 | Destination map record |
| 4 | Arrival metatile column |
| 6 | Arrival metatile row |
| 8 | Sound: `0` plays the default door sound, `0x44` plays nothing, anything else is a sound id |
| 10 | Facing on arrival in sprite sheet order (0 up, 1 down, 2 left, 3 right), `0xFFFF` keeps the current one |

The tables have no length field; the number of entries is whatever the attributes
reference. Map 4 has two exits: exit 0 (metatiles (23–24, 14)) leads to map 5 `md0154` at
(8, 16) and exit 1 (metatiles (23–24, 6)) to map 3 `md0152` at (23, 5).

## Exit attributes

A scene attribute whose bits 15–14 are `01` (`0x4000 | n`) is exit `n`: finishing a step
onto that metatile warps. Attributes with both bits set (`0xC000 | n`) are doors the game
only takes when the player presses A facing them; those are not modeled yet.

## How the game warps

When a step completes, the handler samples the attribute under the player's footing
(the metatile below the one the player stands on). For a `0x4000` attribute it looks up
the entry in the current map's warp table, plays the sound, and calls the scene loader
with the destination map, the arrival metatile and the facing. The loader reads the map
record, decompresses the scene, stands the player on the arrival metatile (sprite top-left
at `(16 × column − 8, 16 × row)`) and points the camera at `(16 × column − 112,
16 × row − 64)` clamped to the map.
