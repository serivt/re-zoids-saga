# Text font

Source of knowledge: own analysis of the Zoids Saga (Japan, Rev 1) glyph lookup
(ROM `0x0804178C`, range search at `0x08041864`). Implemented in
`crates/formats/src/font.rs`; ROM locations in `crates/extraction/src/saga.rs`.

## Glyphs

Glyphs are **8×16 pixels, 4 bits per pixel**, stored as GBA tiles: one 8×8 tile for
the top half and one for the bottom half, 32 bytes each, two pixels per byte with the
left pixel in the low nibble. Strokes are anti-aliased with palette indices:

| Index | Meaning |
|---|---|
| 1 | Background (transparent when drawn over a text box) |
| 2, 3, 4 | Anti-aliasing shades, darkest to lightest |
| 15 | Solid stroke |

Indices 0 and 5–14 never occur in the font.

## Sheets

The 3,419 glyphs are laid out in sheets of 32 glyphs per row. Within a row the 32 top
tiles are contiguous (`0x400` bytes) and the 32 bottom tiles follow, so:

```
top    = sheet + (i / 32) * 0x800 + (i % 32) * 0x20
bottom = top + 0x400
```

## Range table

A table of 111 entries at `0x086BFAC8` maps Shift-JIS codes to sheets. Each entry is
8 bytes: `u16 first_code`, `u16 count`, `u32 sheet_address`. A code `c` in
`[first_code, first_code + count)` is glyph `i = c - first_code` of that sheet. The
game searches the table linearly and draws a fallback glyph (tiles at `0x08683D58` and
`0x08683D78`) when no range matches.

The ranges cover full-width digits and Latin (`0x824F`–`0x829A`), hiragana, katakana,
symbols and the JIS level-1 kanji (up to `0x9872`). Half-width ASCII has no glyphs;
the game only uses full-width characters.
