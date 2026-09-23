# Field scenes

Source of knowledge: own analysis of Zoids Saga (Japan, Rev 1) in a reference emulator
with write watchpoints on VRAM and on the IWRAM tilemap buffer while the first room was
loaded, plus the BIOS decompression log. Implemented in `crates/extraction/src/saga.rs`
(`scene`), `crates/formats/src/tilemap.rs` and `crates/gba-runtime/src/ppu/background.rs`.

## Scene table

47 records of 24 bytes at ROM `0x1E70BC`, one per scene:

| Offset | Field |
|---|---|
| 0 | Pointer to the backdrop: a raw 32×32 tilemap (2048 bytes) shown on BG3 |
| 4 | Map size: low half-word width, high half-word height, in cells |
| 8 | Pointer to an LZ77 block of background palettes (480 bytes = palettes 0–14) |
| 12 | Pointer to an LZ77 block of 4bpp tiles shared by the map and the backdrop |
| 16 | Pointer to an LZ77 block with the map: `width × height` 16-bit tilemap entries |
| 20 | Pointer to an LZ77 block with one nibble per map cell (collision-like; not modeled yet) |

The first room of the game is record 2: backdrop `0x0806B60C`, 96×40 cells, palettes
`0x0806EA68`, tiles `0x0806EB18` (416 tiles), map `0x080713F0`, attributes `0x08071DC4`.
Records 0 and 1 are the title screen, which shares the tiles and palettes.

## How the game shows a scene

Tiles are decompressed to VRAM character block 2 (`0x06008000`) and the palettes to
palette RAM. The map is decompressed to EWRAM (`0x0200E8A4` for the first room); every
frame the engine copies the visible 30×20 window of it into an IWRAM buffer at
`0x0300239C` (offset `0x1000` for BG2) and DMAs the buffer to VRAM `0x06000000`, so
scrolling never touches the ROM. The backdrop tilemap is copied to VRAM `0x1800` (BG3,
palette 12) and repeats behind the map; the map is drawn on BG2 with palette index 0
transparent.

Tilemap entries are standard GBA text-mode entries: bits 0–9 tile, 10 horizontal flip,
11 vertical flip, 12–15 palette.
