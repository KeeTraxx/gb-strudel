# Test fixtures

`.uge` files written by hUGETracker, used by `tests/roundtrip.rs` as the
specification for `src/uge.rs`: every v6 file here must parse and re-serialise
byte for byte. They are test data only and are not part of the published crate
(see `include` in `Cargo.toml`).

## From GB Studio

Taken from [GB Studio](https://github.com/chrismaltby/gb-studio) 4.3.2 and
used under its MIT license, see [LICENSE-GB-STUDIO](LICENSE-GB-STUDIO). All
are hUGETracker v6.

| file | author | path in the GB Studio repository |
|------|--------|----------------------------------|
| `template.uge` | GB Studio | `appData/music/template.uge` |
| `Rulz_BattleTheme.uge` | Rulz | `appData/templates/gbs2/assets/music/` |
| `Rulz_FastPaceSpeedRace.uge` | Rulz | `appData/templates/gbs2/assets/music/` |
| `Rulz_GonaSpace.uge` | Rulz | `appData/templates/gbs2/assets/music/` |
| `Rulz_Into the woods.uge` | Rulz | `appData/templates/gbs2/assets/music/` |
| `Rulz_Intro.uge` | Rulz | `appData/templates/gbs2/assets/music/` |
| `Rulz_LightMood.uge` | Rulz | `appData/templates/gbs2/assets/music/` |
| `Rulz_Outside.uge` | Rulz | `appData/templates/gbs2/assets/music/` |
| `Rulz_Pause_Underground.uge` | Rulz | `appData/templates/gbs2/assets/music/` |
| `Rulz_SpaceEmergency.uge` | Rulz | `appData/templates/gbs2/assets/music/` |
| `Rulz_UndergroundCave.uge` | Rulz | `appData/templates/gbs2/assets/music/` |
| `Tronimal_DrumsExample.uge` | Tronimal | `appData/templates/gbs2/assets/music/` |
| `Tronimal_EchoExample.uge` | Tronimal | `appData/templates/gbs2/assets/music/` |

`template.uge` is identical to `templates/template.uge`, the template the
binary embeds.

## From hUGETracker

| file | author | source |
|------|--------|--------|
| `Coffee Bat - Wyrmhole.uge` | Coffee Bat | [hUGETracker](https://github.com/SuperDisk/hUGETracker) `sample-songs/` |

This one is a v5 file, which the round-trip test skips. It is **not** covered
by `LICENSE-GB-STUDIO`, and the hUGETracker repository has no license file.
