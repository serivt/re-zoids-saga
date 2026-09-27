# Sound

Source of knowledge: own reading of the sound driver in Zoids Saga (Japan, Rev 1): the
player's frame (`0x0805B724`), the note routine (`0x0805BA00`), the track volume and
pitch routine (`0x0805C690`), the key-to-rate routines (`0x0805BC90`, `0x0805C744`), the
programmable channels' frame (`0x0805C8A4`, `0x0805C83C`) and the mixer the driver copies
to RAM (from ROM `0x0805B18C`); its state read from the RAM in a reference emulator (its
`SoundInfo` block at `0x03006600`, the four music players and their table at ROM
`0x8567738`); frame-by-frame captures of the driver's own PCM buffer and channel
records (`research/tools/mgba_pcm.lua`); the public description of the GBA's common
sound driver's data (song headers, voice groups, samples, track bytecode); and public
hardware documentation of how the sound hardware mixes its channels. No code from any
emulator or decompilation was used; see [third-party.md](third-party.md). Implemented
in `crates/formats/src/m4a.rs` (parsing) and `crates/gba-runtime/src/apu/` (players,
channels, mixer); constants in `crates/extraction/src/saga.rs`; output through
`platform::AudioOut`.

## The driver in this game

| Setting | Value | Where it comes from |
|---|---|---|
| Mix rate | 31536 Hz, 528 samples per frame | `SoundInfo` (`freq` index 9) |
| Sampled channels | 8 | `SoundInfo.maxChans` |
| Master volume | 14 of 15 | `SoundInfo.masterVolume` |
| Output | mono: one buffer of 3 × 528 bytes at `SoundInfo + 0x350`, played by DMA sound A on both sides at full volume; B off; programmable channels at 100 %, master volumes 7 and 7 | the right half of the buffer is never written; `SOUNDCNT_H` `0x030E`, `SOUNDCNT_L` `0x..77` |
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
the previous ones otherwise; `0xCE` reads an optional key the same way. A bare argument
after a one-byte command repeats that command. Only the commands from `0xBD` on become
the running status: after `0xBA`, `0xBB` or `0xBC` a bare byte repeats the command
before them (sound `0x68` bends on past its tempo change). `0xB1` ends the track, `0xB2`
jumps, `0xB3`/`0xB4` call and return from a pattern, `0xBA`–`0xC8` set priority, tempo,
key shift, voice, volume, pan, bend, bend range, LFO speed and delay, modulation depth
and type, and tune; `0xCD` is an extended command the songs here do not use (the driver
treats it as `0xB1`).

## The sequencer

Tempo `t` adds `2t` to a counter each frame and every 150 is one tick, so 75 is one
tick per frame. Each tick, track by track, the track's notes count their gates down
(a note's key goes up when its gate reaches 0), then its commands run until a wait,
then its modulation steps. A player whose tracks have all ended sets its status's
pause bit in the next frame; that is when the game sees the song end (`0x08001A28`):
sound `0x68`, two tracks, takes 140 frames from its start. The players run from 0 to
3. A song's start (`0x0805C4A4`) stops its player's notes at once; the players' own
priority check is off in this game (their table's field is 0), so a song always
starts.

A track starts with volume 0, key 0, velocity 0, pan, bend and tune centered, bend
range 2, a volume scale of 64 and an LFO speed of `0x16`. From its volume, pan and
modulation it keeps a right and a left volume (`0x0805C690`): the volume times the
scale over 32, times the pan (−128 to 127, twice the command's offset from 64) plus
128 over 256 on the right, and 127 minus the pan over 256 on the left. Its pitch is
the bend times the bend range plus the tune, in quarters of 1/64 key, plus the key
shift, plus the pitch modulation in 16ths of a key, kept as whole keys and 256ths.
Commands that change these mark the track (volume and pan; bend, range, tune and key
shift; both for a new modulation type), and after the ticks the frame's changes pass
to the track's notes. A note clears the marks: the notes before it keep their volumes
and rates when a change and the note come in the same frame.

The modulation (LFO), per tick: after the delay (in ticks, counted again from each
note when the delay is not 0, which also clears the modulation), the phase advances by
the speed and the value is the depth times a triangle of the phase (−64 to 64) over 64;
a speed or depth of 0 clears it. Only pitch modulation is used here (no song sets the
type): the value adds its 16ths of a key.

## Notes and channels

A note's priority is its song's plus its track's (at most 255). A key split picks its
sub-voice by the note's key; a drum kit picks by the key and plays the drum at the
drum's own key, with the drum's pan (`(pan − 0xC0) × 2` when bit 7 is set); a sub-voice
that is itself a split or kit plays nothing. The note sounds at its key plus the track's
whole keys (not below 0) and fine pitch. Its right and left volumes are the velocity
times 128 plus the kit pan (right) or 127 minus it (left) times the track's volume,
over 2¹⁴, at most 255.

A sampled note takes the first free channel of the eight; else, among those letting go
(if any), else among all, the one of lowest priority, as long as it is not above the
note's; ties go to the note of the later track (players' tracks lie in player order in
RAM, and a note whose track has ended counts before any), then to the later channel.
With no such channel the note is dropped. A programmable note takes its channel (square
1, square 2, wave, noise, by the voice) when it is free, letting go, of lower priority,
or of the same priority and the same or a later track. `0xCE` lets go of the track's
latest note of its key still held; a track's end lets go of all its notes.

## The mixer

Each frame the mixer fills the next of the three frames of its buffer. It starts with
the reverb: every byte is the sum of the bytes the buffer held three and two frames
before times the reverb over 256, a negative result one closer to 0; without reverb
the frame starts silent. Then each sampled channel in turn steps its envelope (a new
note starts at its attack; the attack adds to 255, the decay multiplies by decay / 256
down to the sustain, the key's release multiplies by release / 256, and 0 frees the
channel) and adds its samples: its volume is the right plus the left volume times the
envelope times the master volume plus one over 16, over 512; each output sample is the
sample linearly interpolated between two bytes at a 23-bit fraction (voices marked
fixed play one byte per output sample), times the volume over 256, added to the
buffer's byte with wrapping. The rate advances the position by the note's rate
(`0x0805BC90`: the sample's rate times a key table's step, interpolated to the next
key by the fine pitch, in 32-bit fixed point) times 266 in 23-bit fractions. At a
sample's end it loops back to the loop start or the channel frees.

The programmable channels' frame (`0x0805C8A4`) runs before the mixer. Each channel's
envelope is the driver's own: at a note's start or when its counter runs out it derives
its goal and pan from the volumes (`0x0805C83C`: a side at least twice the other plays
alone; the goal is the sum over 16, at most 15 unless both sides play; the sustain
level is the sustain times the goal plus 15, over 16), then steps a level toward the
phase's goal, counting the attack, decay or release in frames (7 in the sustain);
every fifteenth frame (the driver's 14-to-0 counter at 0) counts one step more, to keep
pace with the hardware's 64 envelope steps a second. The tone channels' register comes
from a key table (`0x0805C744`: from key 36, interpolated by the fine pitch), the noise
channel's frequency byte from a table per key from 21, and the wave channel's volume
from a table per level (full, 75 %, half, quarter, off).

The hardware then mixes: the sampled buffer at full volume (a byte counts 4) on both
sides, and each programmable channel's level times the master volume plus one (8) on
the sides its pan allows, clipped to the 10-bit output (±512 around its bias). The port
spans that range over the whole 16-bit sample (×64), so the loudest songs peak just
under full scale; the launcher's volume only scales it down.

## Verified

Played from the frame the original starts them, the port's sampled buffer equals the
driver's byte for byte on every frame compared: the title's song 1 for 990 frames,
songs 5, 7, 12 and 22 for 1985, song 21 (with priority 32 and a modulation depth) for
3665, and song 1 with the menu's effects `0x3D`, `0x40` and `0x47` over it for 685. The
programmable channels' records (status, level, goal, counter, pan and frequency) equal
the driver's on every frame of the same runs, the 14-to-0 counter in the same phase.
The comparison scripts live in the research notes, not in the tree.

## Not modeled yet

The hardware's generators of the programmable channels and the final mix follow the
hardware documentation and have not been compared with a capture of the hardware's
output; the hardware's own envelope steps (the driver's level stands for them) and its
8-bit output resolution. The phase of the driver's 14-to-0 counter from power-on. The
driver's echo, fades, the sweep, the sound length and the programmable voices of fixed
pitch, which nothing in this game uses. The name-entry key sounds and the sounds of the
field beyond doors are not known.
