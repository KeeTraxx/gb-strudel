//! gb-strudel — compose Game Boy music as code.
//!
//! Writes hUGETracker `.uge` files that GB Studio can import, and previews
//! them through a small Game Boy APU model so you can hear a change without
//! launching the engine.

mod apu;
mod pattern;
mod player;
mod song;
mod uge;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
gb-strudel — compose Game Boy music as code

USAGE:
    gb-strudel build <song.gbs> [-o out.uge]   compile to a .uge
    gb-strudel play  <song.gbs|song.uge>       preview through the speakers
                                               (keys: 1-4 mute, r repeat, q quit)
    gb-strudel wav   <song.gbs|song.uge> <out.wav>
    gb-strudel info  <song.uge>                describe an existing .uge
    gb-strudel instruments [file.uge]          list a template's instruments

The template supplying instruments and wavetables is taken from
--template <file.uge>, or the bundled one if not given.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Pull `--template <path>` out of the argument list.
fn take_template(args: &mut Vec<String>) -> Option<PathBuf> {
    let i = args.iter().position(|a| a == "--template")?;
    if i + 1 >= args.len() {
        return None;
    }
    let path = PathBuf::from(args.remove(i + 1));
    args.remove(i);
    Some(path)
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    if i + 1 >= args.len() {
        return None;
    }
    let v = args.remove(i + 1);
    args.remove(i);
    Some(v)
}

/// The instrument/wavetable donor. Every generated song inherits hUGETracker's
/// stock instruments this way, so output is playable without designing any.
fn load_template(explicit: Option<PathBuf>) -> Result<uge::Song, String> {
    let candidates: Vec<PathBuf> = match explicit {
        Some(p) => vec![p],
        None => vec![
            PathBuf::from("templates/template.uge"),
            Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/template.uge"),
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/template.uge"),
        ],
    };
    for c in &candidates {
        if c.exists() {
            let data = fs::read(c).map_err(|e| format!("{}: {e}", c.display()))?;
            return uge::Song::parse(&data).map_err(|e| format!("{}: {e}", c.display()));
        }
    }
    Err(format!(
        "no template .uge found (looked in {})",
        candidates
            .iter()
            .map(|c| c.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Load either a song source file or an existing `.uge`.
fn load_any(path: &Path, template: Option<PathBuf>) -> Result<uge::Song, String> {
    if path.extension().and_then(|e| e.to_str()) == Some("uge") {
        let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        return uge::Song::parse(&data).map_err(|e| format!("{}: {e}", path.display()));
    }
    let src = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let def = song::parse_song_file(&src).map_err(|e| format!("{}: {e}", path.display()))?;
    let base = load_template(template)?;
    def.compile(&base)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn run(args: &[String]) -> Result<(), String> {
    let mut args: Vec<String> = args.to_vec();
    let template = take_template(&mut args);
    let out_flag = take_flag(&mut args, "-o");

    let Some(cmd) = args.first().cloned() else {
        println!("{USAGE}");
        return Ok(());
    };
    let rest = &args[1..];

    match cmd.as_str() {
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        "build" => {
            let input = rest.first().ok_or("build needs a song file")?;
            let path = Path::new(input);
            let src = fs::read_to_string(path).map_err(|e| format!("{input}: {e}"))?;
            let def = song::parse_song_file(&src).map_err(|e| format!("{input}: {e}"))?;
            let base = load_template(template)?;
            let compiled = def.compile(&base).map_err(|e| format!("{input}: {e}"))?;
            let out = out_flag
                .map(PathBuf::from)
                .unwrap_or_else(|| path.with_extension("uge"));
            fs::write(&out, compiled.write()).map_err(|e| format!("{}: {e}", out.display()))?;
            println!(
                "wrote {} ({} bars, {} patterns, {} ticks/row)",
                out.display(),
                def.bar_count(),
                compiled.patterns.len(),
                compiled.ticks_per_row
            );
            Ok(())
        }
        "play" => {
            let input = rest.first().ok_or("play needs a song or .uge file")?;
            let s = load_any(Path::new(input), template)?;
            let stems = apu::Renderer::new().render_stems(&s);
            if stems[0].is_empty() {
                return Err("song has no rows to play".into());
            }
            let title = if s.name.is_empty() { input.clone() } else { s.name.clone() };
            player::play(&title, stems, apu::Renderer::samples_per_row(&s))
        }
        "wav" => {
            let input = rest.first().ok_or("wav needs an input file")?;
            let out = rest.get(1).ok_or("wav needs an output path")?;
            let s = load_any(Path::new(input), template)?;
            let samples = apu::Renderer::new().render(&s);
            fs::write(out, encode_wav(&samples)).map_err(|e| format!("{out}: {e}"))?;
            println!(
                "wrote {out} ({:.1}s)",
                samples.len() as f32 / apu::SAMPLE_RATE as f32
            );
            Ok(())
        }
        "info" => {
            let input = rest.first().ok_or("info needs a .uge file")?;
            let s = load_any(Path::new(input), template)?;
            println!("name      {}", s.name);
            println!("artist    {}", s.artist);
            println!("ticks/row {}", s.ticks_per_row);
            println!("patterns  {}", s.patterns.len());
            println!("sequence  {} steps", s.sequence_len());
            for (i, o) in s.orders.iter().enumerate() {
                println!("  ch{i} order {o:?}");
            }
            Ok(())
        }
        "instruments" => {
            let song = match rest.first() {
                Some(p) => load_any(Path::new(p), template)?,
                None => load_template(template)?,
            };
            let names = song.instrument_names();
            for (bank, label) in ["pulse1 / pulse2", "wave", "noise"].iter().enumerate() {
                println!("{label}:");
                for slot in 0..15 {
                    let name = &names[bank * 15 + slot];
                    println!("  {:2}  {}", slot + 1, name);
                }
            }
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n\n{USAGE}")),
    }
}

/// 16-bit mono PCM WAV.
fn encode_wav(samples: &[f32]) -> Vec<u8> {
    let bytes = samples.len() * 2;
    let mut out = Vec::with_capacity(44 + bytes);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + bytes) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&apu::SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(apu::SAMPLE_RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(bytes as u32).to_le_bytes());
    for &s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}
