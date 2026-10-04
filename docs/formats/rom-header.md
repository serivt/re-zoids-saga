# Cartridge header

Source of knowledge: public hardware documentation (GBATEK, "GBA Cartridge Header").
Implemented in `crates/formats/src/rom_header.rs`.

| Offset | Size | Field | Notes |
|---|---|---|---|
| `0x000` | 4 | Entry point | ARM branch, not parsed |
| `0x004` | 156 | Nintendo logo | Not parsed |
| `0x0A0` | 12 | Title | ASCII, NUL/space padded |
| `0x0AC` | 4 | Game code | `ATZJ` Zoids Saga, `BZFJ` Zoids Saga Fuzors |
| `0x0B0` | 2 | Maker code | `DA` = Tomy |
| `0x0B2` | 1 | Fixed value | Must be `0x96` |
| `0x0BC` | 1 | Software version | `0` for Zoids Saga's first release, `1` for Rev 1 ([revisions](revisions.md)) |
| `0x0BD` | 1 | Complement | `-(sum(bytes 0xA0..0xBD) + 0x19) & 0xFF` |

Identification (`crates/extraction/src/identify.rs`) maps the game code to a title and
compares the SHA-1 of the whole image against the validated dumps listed in the README.
