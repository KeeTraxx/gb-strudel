# gb-strudel

Compose Game Boy music as code. Writes hUGETracker `.uge` files that GB Studio
imports directly, and previews them through a small Game Boy APU model so you
can hear a change without opening the engine.

Inspired by [Strudel](https://strudel.cc) / TidalCycles pattern notation, but
targeting the Game Boy's four hardware channels instead of Web Audio.

## Install

Download a prebuilt binary for Linux, macOS or Windows from the
[releases page](https://github.com/KeeTraxx/gb-strudel/releases), or build
it with Cargo:

```sh
cargo install gb-strudel
```

On Linux, `play` needs ALSA, so install its headers first (`libasound2-dev`
on Debian/Ubuntu, `alsa-lib` on Arch, `alsa-lib-devel` on Fedora).

## Usage

```sh

gb-strudel build songs/coffee_break.gbs      # -> songs/coffee_break.uge
gb-strudel build songs/coffee_break.gbs ~/game/assets/music/coffee_break.uge
                                             # or -o <path>
gb-strudel play  songs/coffee_break.gbs      # preview through the speakers
gb-strudel wav   songs/coffee_break.gbs out.wav
gb-strudel info  songs/coffee_break.uge      # describe an existing file
gb-strudel instruments                       # list the template's instruments

gb-strudel write-basic-song                  # -> ./basic.gbs, one channel
gb-strudel write-advanced-song               # -> ./advanced.gbs, four channels and &sections
```

The `write-*` commands give you a commented song to start from, and take an
optional file name (`gb-strudel write-basic-song intro.gbs`). They never
overwrite an existing file.

While `play` runs it shows a live status: position and bar on top, one line
per channel (green when playing, red when muted), and the keys at the bottom:

```
Coffee Break
▶ 0:02 / 0:08   bar 1/4   repeat on

  1  pulse1  ● on
  2  pulse2  ○ muted
  3  wave    ● on
  4  noise   ● on

1-4 mute   r repeat   q quit
```

Single keys control it:

| key   | action                                   |
|-------|------------------------------------------|
| `1`–`4` | mute/unmute `pulse1`, `pulse2`, `wave`, `noise` |
| `r`   | toggle repeat (loop back to the start)   |
| `q`   | quit (also `Esc` or `ctrl-c`)            |

Muting only changes the preview mix; it never touches the song. When stdin
is not a terminal (piped, scripted), `play` just plays once without controls.

`play`, `wav` and `info` accept either a `.gbs` source or an existing `.uge`,
so you can audition files that GB Studio or hUGETracker produced.

## Song files

```
name   = Coffee Break
artist = gb-strudel
tempo  = 6              # ticks per row; the driver runs at 60Hz, so 60/6 = 10 rows/sec

pulse1 = 8 | c4 ~ a3 c4 | d4 ~ c4 ~
pulse2 = 6 | ~ e3 ~ a2  | ~ f3 ~ c3
wave   = 4 | f2 a2 c3 e3 | d3 c3 a2 f2
noise  = 1 | c4*4 | c4 ~ c4 c4
```

Each channel line is `<instrument> | <bar> | <bar> | ...`. The instrument is an
index into that channel's bank in the template (1–15). Bars cycle if one
channel is shorter than another, so a 1-bar drum pattern loops under a 4-bar
melody.

The four channels are the console's, not roles — `pulse1` has the hardware
sweep unit, `wave` plays a 32-sample wavetable, and `noise` is the LFSR:

| channel  | hardware            | typical use              |
|----------|---------------------|--------------------------|
| `pulse1` | square + sweep      | lead                     |
| `pulse2` | square              | harmony / counter-melody |
| `wave`   | 32-sample wavetable | bass                     |
| `noise`  | LFSR noise          | drums                    |

### Sections (`&name`)

A line whose key starts with `&` defines a named run of bars instead of a
channel. A channel line can then use `&name` anywhere a bar can go, mixed with
inline bars:

```
&intro = 8 | c4 ~ a3 c4 | d4 ~ c4 ~ | as3 d4 c4 as3 | a3 ~ g3 ~
&verse = 9 | f4 . a4 ~ | g4 ~ e4 ~

pulse1 = 6 | &intro | c4 e4 g4 c5 | &verse*2 &intro
```

- A section keeps its own instrument. Above, the `&intro` bars play on 8, the
  inline `c4 e4 g4 c5` bar on 6 and `&verse` on 9.
- A section without a leading number (`&riff = c4 e4 | g4`) takes the
  instrument of the line that uses it, which lets one riff be shared between
  channels with different sounds.
- A line made only of references can drop the instrument:
  `pulse1 = &intro &verse`. Any inline bar still needs one.
- Several references can share one `|` slot, separated by spaces, and
  `&name*N` repeats a section N times.
- Sections can reference other sections and can be defined after they are
  used. Undefined, duplicate or circular references are build errors.

`<a b>` alternation still counts song bars, not bars within the section, so a
section's alternation depends on where it lands in the song.

### Tempo

`tempo` is ticks per row, not BPM. The driver ticks at 60 Hz and every bar is
64 rows, so how fast a song feels depends on how many beats you write into a
bar:

```
bpm = 3600 / (tempo * rows_per_beat)        rows_per_beat = 64 / beats_per_bar
```

| `tempo` | 4 beats per bar (16 rows/beat) | 8 beats per bar (8 rows/beat) |
|---------|--------------------------------|-------------------------------|
| 1       | 225                            | 450                           |
| 2       | 112.5                          | 225                           |
| 3       | 75                             | 150                           |
| 4       | 56.25                          | 112.5                         |
| 5       | 45                             | 90                            |
| 6       | 37.5                           | 75                            |
| 8       | 28.1                           | 56.25                         |

`tempo` only takes whole numbers, so the available speeds are coarse. To get
something in between, change the meter instead of the tempo. For example, at
`tempo = 5`, writing eight beats per bar gives 90 BPM, which no whole `tempo`
value reaches with four beats per bar. The trade-off is resolution: a bar
always has 64 rows, so with eight beats per bar a sixteenth note is only 2
rows, and a 32nd note no longer fits.

These figures assume the 60 Hz tick that the preview uses. When the driver
runs off the frame interrupt, real hardware ticks at ~59.7 Hz, about half a
percent slower.

## Pattern notation

Steps split each bar evenly, so `c4 e4 g4 c5` is four quarter notes and
`c4 e4` is two half notes.

| syntax      | meaning                                        |
|-------------|------------------------------------------------|
| `c4 e4 g4`  | notes: letter, optional `#`/`b`, octave        |
| `~`         | rest                                           |
| `.`         | hold (sustain the previous note)               |
| `[c4 e4]`   | group: subdivides one step's slice             |
| `c4*4`      | repeat a step within its slice                 |
| `<c4 e4>`   | alternate: one per bar, in turn                |

Note range is **C2 to B7** (hUGE notes 0–71). Anything outside it is a build
error rather than a silent clamp, because a wrong octave is a composing
mistake worth hearing about.

Note naming follows scientific pitch, verified against the driver's own
`hUGE_note_table.inc`: A4 is 440 Hz. (hUGETracker's UI labels the same note
`A-5`, following the tracker convention of numbering one octave higher.)

## Instruments

This tool composes patterns; it does not design instruments. Every build copies
the instrument and wavetable definitions verbatim from a template `.uge` —
GB Studio's own `templates/template.uge`, which is compiled into the binary,
unless you pass `--template`. To use
custom instruments, design them in hUGETracker, save, and point `--template` at
that file.

The number before the first `|` on a channel line is an index into that
channel's bank below. Each bank has 15 slots, and **the banks are separate** —
instrument `7` means "12,5% Pulse" on `pulse1`/`pulse2` but "Snare Drum" on
`noise`.

Run `gb-strudel instruments [file.uge]` to list these for any template.

### `pulse1` / `pulse2` (both pulse channels share one bank)

| # | name | | # | name |
|---|------|-|---|------|
| 1 | Fade Out 25% Pulse | | 9 | 50% Pulse |
| 2 | Fade Out 50% Pulse | | 10 | 75% Pulse Custom Vibrato |
| 3 | Fade In 12,5% Pulse | | 11 | Bass Drum 50% Pulse (Duty 1 Only) |
| 4 | Short 12,5% Pulse | | 12 | Soft Sweep 50% Pulse (Duty 1 Only) |
| 5 | Short 25% Pulse | | 13 | Sweep 12,5% Pulse (Duty 1 Only) |
| 6 | Short 50% Pulse | | 14 | Sweep 25% Pulse (Duty 1 Only) |
| 7 | 12,5% Pulse | | 15 | *(empty)* |
| 8 | 25% Pulse | | | |

The "Duty 1 Only" instruments use the hardware sweep unit, which exists only on
`pulse1` — they will not sound as named on `pulse2`.

### `wave`

| # | name | | # | name |
|---|------|-|---|------|
| 1 | 12,5% Pulse | | 9 | 50% Pulse (Volume 1) |
| 2 | 25% Pulse | | 10 | Square Wave with added Square Wave (2 octaves higher) |
| 3 | 31.25% Pulse | | 11 | Triangular Wave |
| 4 | 37,50% Pulse | | 12 | Triangular with added Square Wave (2 octaves higher) |
| 5 | 43,75% Pulse | | 13 | Saw Wave |
| 6 | 50% Pulse | | 14 | Distorted Saw Wave |
| 7 | 50% Pulse (Volume 5) | | 15 | *(empty)* |
| 8 | 50% Pulse (Volume 3) | | | |

### `noise`

| # | name | | # | name |
|---|------|-|---|------|
| 1 | Closed Hi-Hat | | 9 | Snare Drum 3 |
| 2 | Closed Hi-Hat 2 | | 10 | Metallic Sound |
| 3 | Closed Hi-Hat 3 | | 11 | Bass Drum |
| 4 | Open Hi-Hat | | 12 | Bass Drum 2 |
| 5 | Crash | | 13 | Explosion |
| 6 | Crash 2 | | 14 | Explosion 2 |
| 7 | Snare Drum | | 15 | *(empty)* |
| 8 | Snare Drum 2 | | | |

Note that preview does **not** interpret these definitions — it approximates
each channel with a fixed timbre, so envelopes, sweeps and vibrato will be
audible in GB Studio but not in `play`/`wav`.

## The `.uge` format

`src/uge.rs` documents the v6 layout. It follows GB Studio's own loader
(`loadUGESong` in `src/shared/lib/uge/ugeHelper.ts`) and is checked against the
files that ship inside GB Studio 4.3.2. `tests/roundtrip.rs` asserts that all 12
bundled v6 files parse and re-serialise byte-for-byte, and that known cells
decode to the values GB Studio shows. v5 files (hUGETracker's older sample
songs) are not supported.

Details worth knowing if you touch the writer:

- The timer setting is a `u8` (enabled) followed by a `u32` (divider), not a
  single `u32`.
- Each pattern is a `u32` id followed by 64 cells. Order tables refer to
  patterns by that id.
- A cell is 17 bytes: four little-endian `u32`s (note, instrument, an unused
  column, effect code) plus a one-byte effect param. Note `90` means "no note".
- A byte-exact round trip is not enough on its own. A reader and writer that
  share the same wrong framing still reproduce their input, which is how an
  earlier version of this tool wrote files GB Studio rejected with "Song has
  too many patterns".

## Tests

```sh
cargo test
```

## Releasing

Releases are automated with [release-plz](https://release-plz.dev): merging
its release PR publishes to crates.io. See [RELEASE.md](RELEASE.md).

## License

MIT, see [LICENSE](LICENSE). `templates/template.uge` is from
[GB Studio](https://github.com/chrismaltby/gb-studio) and ships under its MIT
license, see [templates/LICENSE-GB-STUDIO](templates/LICENSE-GB-STUDIO).
