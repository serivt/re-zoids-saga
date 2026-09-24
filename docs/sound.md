# Sound

Source of knowledge: the sound driver's state read from the RAM of Zoids Saga (Japan,
Rev 1) in a reference emulator (its `SoundInfo` block at `0x03006600`, the four music
players and their table at ROM `0x8567738`), a log of every song start from power-on
to the first room, a frame-by-frame capture of the driver's own PCM buffer
(`research/tools/mgba_pcm.lua`), and the public description of the GBA's common
sound driver's data (song headers, voice groups, samples, track bytecode). No code
from any emulator or decompilation was used; see [third-party.md](third-party.md).
Implemented in `crates/formats/src/m4a.rs` (parsing) and
`crates/gba-runtime/src/apu/` (players, voices, mixer); constants in
`crates/extraction/src/saga.rs`; output through `platform::AudioOut`.

## The driver in this game

| Setting | Value | Where it comes from |
|---|---|---|
| Mix rate | 31536 Hz, 528 samples per frame | `SoundInfo` (`freq` index 9) |
| Sampled channels | 8 | `SoundInfo.maxChans` |
| Master volume | 14 of 15 | `SoundInfo.masterVolume` |
| Output | mono: one buffer of 3 × 528 bytes at `SoundInfo + 0x350`, read by both sound DMAs | the right half of the buffer is never written |
| Players | 0: music, 12 tracks; 1: 8 tracks; 2 and 3: 4 tracks (effects) | player table `0x8567738` |
| Song table | `0x567768`, 134 entries of `{header, player, unused}` | `m4aSongNumStart` at `0x805BDB8` indexes it from there |

Songs 0–39 and 50–52 are music (priority 128 or 192), 60–133 are effects (priority
255); the rest are empty. Two voice groups exist: `0x5664FC` for music (43 samples,
67 square waves, 8 wave, 4 square-2, 4 key splits, 2 drum kits) and `0x566AFC` for
effects. Scripts play effects by song number (`0x3C + n` in the sound opcode).

Songs the game starts, from the log: 1 on the title (frame 409), effect `0x3D` on
START, 3 behind the name entry, the empty song 134 when the room loads (silence), 11
for the opening cutscene, 7 when control begins in the first room, `0x47` on menu
confirmations, `0x82` on doors. Every map names its song in the word at offset 8 of
its record (see [formats/map.md](formats/map.md)); the loader starts it unless it is
already playing, as the traced walk from the first room into `md0154` shows (song 7
gives way to the castle's 11 while the screen is black). The opening switches the first
room to song 7 when control begins.

## Format

A song header is `{tracks u8, blocks u8, priority u8, reverb u8, voice group ptr,
track ptrs…}`. Voice entries are 12 bytes: type (`0x00` sample, `0x08` sample at a
fixed rate, `0x01`/`0x02` square, `0x03` wave, `0x04` noise, `0x40` key split with a
sub group and a 128-byte key table, `0x80` drum kit indexed by key), base key, length,
pan or sweep, a pointer or PSG parameter, then attack, decay, sustain, release. Samples
are `{flags (bit 30 loops), rate × 1024, loop start, length, 8-bit signed data}`.

Track bytes: `0x80`–`0xB0` waits and `0xCF`–`0xFF` notes take their tick counts from
one 49-entry table (0–24, then 28, 30, 32, 36, 40, 42, 44, 48, 52, 54, 56, 60, 64, 66,
68, 72, 76, 78, 80, 84, 88, 90, 92, 96; `0xCF` is a tie held until `0xCE`). A note
reads up to three arguments below `0x80` (key, velocity, extra gate ticks) and repeats
the previous ones otherwise; a bare argument after a one-byte command repeats that
command. `0xB1` ends the track, `0xB2` jumps, `0xB3`/`0xB4` call and return from a
pattern, `0xBA`–`0xC8` set priority, tempo, key shift, voice, volume, pan, bend, bend
range, LFO speed and delay, modulation depth and type, and tune; `0xCD` is an extended
command the songs here do not use.

Tempo `t` adds `2t` to a counter each frame and every 150 is one tick, so 75 is one
tick per frame. A note's volume is velocity × track volume / 128 (0–126). Sampled
envelopes add the attack per frame to 255, multiply by decay/256 down to the sustain,
and multiply by release/256 after the key goes up; PSG envelopes rise to volume / 8
(at most 15) and step one level every attack, decay or release frames (0 is at once),
holding at goal × sustain / 16. A sample's rate field is its rate at key 60 whatever
the voice's base key says; a drum's own key replaces the note's, which is how kits
pitch their samples. Pitch is `rate × 2^((key − 60 + bend × range / 64 + tune / 64 +
vibrato) / 12)` with the vibrato a triangle of ±modulation / 256 semitones after the
LFO delay. Reverb adds to every sample the one the buffer held three frames earlier
times the song's reverb value over 256.

## Verified

The driver's sound channel records, read every frame during the title, show the
envelope rule above exactly (255, then 244, then 121, 60, 29, 14, 6, 2 for a voice
with decay 245 and release 127), the volume rule (velocity 92 × volume 44 / 128 = 31)
and the PSG goal (volume 20 gives 2). Playing songs 1 and 7 from the frames the
original started them and mixing the sampled channels gives waveforms whose
correlation with the driver's buffer averages 0.75 and 0.90 frame by frame, with the
notes landing on the same frames and the gain matching after scaling by the master
volume over 16. The comparison scripts live in the research notes, not in the tree.

## Not modeled yet

Pan (moot in mono), the volume and pan modulation types, the priority rules for
stealing channels, the PSG sweep and the exact PSG frequency register, the PSG
loudness relative to the sampled mix (no capture of the hardware registers exists
yet), and the remaining correlation dips on some notes.
The name-entry key sounds and the sounds of the field beyond doors are not known.
