# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A Rust CLI that compiles a Strudel/Tidal-style pattern language (`.gbs` files) into hUGETracker `.uge` v6 files for GB Studio, and previews songs through a simplified Game Boy APU model. The README documents the user-facing song format, pattern notation and instrument banks. Read it before changing syntax or behavior.

## Commands

```sh
cargo build --release
cargo test                                   # all tests
cargo test --test roundtrip                  # one test file (roundtrip | compile | pattern)
cargo test --test pattern repeat_fills       # one test by name substring
cargo run -- build songs/coffee_break.gbs    # -> songs/coffee_break.uge
cargo run -- info songs/coffee_break.uge
cargo run -- wav songs/coffee_break.gbs /tmp/out.wav   # audible check without speakers
```

## Architecture

Pipeline: `.gbs` text → `song::parse_song_file` → `SongDef` → `SongDef::compile(template)` → `uge::Song` → bytes. `apu::Renderer` turns a `uge::Song` into samples for `play`/`wav`.

- `src/pattern.rs`: mini-notation. `parse` produces a `Step` tree. `render(steps, rows, cycle)` samples it onto a fixed 64-row grid of `Event`s (`Strike`/`Sustain`/`Silence`). `cycle` is the bar index, which drives `<a b>` alternation.
- `src/song.rs`: the `key = value` song file, plus compilation. Each bar becomes one 64-row pattern per channel, and bar `b` of channel `c` is global pattern `b * 4 + c` (the same interleave GB Studio's bundled songs use). Order tables get one extra trailing slot, and `padding` is 16 bytes per bar after the first. Parts shorter than the song cycle.
- `src/uge.rs`: byte-exact reader and writer for the reverse-engineered v6 format. The module doc lists the layout. Unknown regions (the pre-instrument `u32`, instruments, wavetables, the trailing routine table) are kept as opaque bytes and written back verbatim.
- `src/apu.rs`: an approximate preview, not an emulator. It ignores instrument definitions (envelopes, sweep, vibrato) and uses a fixed timbre per channel.
- `src/main.rs`: hand-rolled argument parsing (no clap). Flags (`--template`, `-o`) are pulled out of the argument list before dispatch.

### Templates / instruments

The tool never creates instruments. `compile` clones a template `.uge` (default `templates/template.uge`, overridable with `--template`) and replaces only the name, artist, comment, tempo, patterns, orders and padding. An instrument number in a `.gbs` file is an index (1–15) into that channel's separate bank, and the pulse channels share one bank.

## Conventions and gotchas

- **There is no lib crate.** Integration tests pull modules in with `#[path = "../src/uge.rs"] mod uge;` and similar. Modules reference each other through `crate::uge`, so any test that includes `pattern.rs` or `song.rs` must also declare `mod uge`. A new module dependency means updating those `#[path]` lists in `tests/`. Items used only by some test crates need `#[allow(dead_code)]`.
- **`tests/roundtrip.rs` is the format spec.** Every `.uge` in `tests/fixtures/` must parse and re-serialize byte-for-byte. Any writer change must keep this test passing. v5 files are intentionally unsupported.
- **Note naming is scientific pitch:** hUGE note 0 = C2 and note 33 = A4 = 440 Hz; the valid range is C2–B7 (notes 0–71). hUGETracker's UI and the driver headers number octaves one higher. Some older doc comments in `pattern.rs` and `uge.rs` still use the tracker numbering ("octave 3-8", "C-3 through B-8"). The code and README are authoritative.
- Out-of-range notes and unknown song-file keys are hard errors, never silent clamps or ignores. Tests assert this.
- Cell note `90` (`NO_NOTE`) means an empty row.
