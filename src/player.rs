//! Live playback through the speakers, with keyboard controls.
//!
//! The song is pre-rendered into one stem per channel, and the audio callback
//! mixes them on the fly, so muting a channel takes effect immediately. The
//! callback and the terminal UI share state through atomics only.

use crate::apu;
use crate::uge::{CHANNELS, ROWS_PER_PATTERN};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::Stylize;
use crossterm::{cursor, execute, queue, terminal};
use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

const CHANNEL_NAMES: [&str; CHANNELS] = ["pulse1", "pulse2", "wave", "noise"];

/// State shared between the audio callback and the UI thread.
struct Shared {
    muted: [AtomicBool; CHANNELS],
    repeat: AtomicBool,
    /// Current position, in source samples.
    position: AtomicUsize,
    finished: AtomicBool,
}

/// Puts the terminal back the way it was, even if we bail out early.
struct RawMode;

impl RawMode {
    fn enable() -> Result<RawMode, String> {
        terminal::enable_raw_mode().map_err(|e| format!("could not set up the terminal: {e}"))?;
        let _ = execute!(std::io::stdout(), cursor::Hide);
        Ok(RawMode)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = execute!(std::io::stdout(), cursor::Show);
        let _ = terminal::disable_raw_mode();
    }
}

/// Play `stems` and block until the song ends or the user quits.
///
/// When stdin is not a terminal there is nothing to read keys from, so the
/// song plays through once without controls.
pub fn play(
    title: &str,
    stems: [Vec<f32>; CHANNELS],
    samples_per_row: usize,
) -> Result<(), String> {
    let len = stems[0].len();
    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let shared = Arc::new(Shared {
        muted: Default::default(),
        repeat: AtomicBool::new(false),
        position: AtomicUsize::new(0),
        finished: AtomicBool::new(false),
    });

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no audio output device available")?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("no default output config: {e}"))?;
    let config: cpal::StreamConfig = supported.into();
    let channels = config.channels as usize;
    // Resample if the device will not run at our render rate.
    let ratio = apu::SAMPLE_RATE as f64 / config.sample_rate as f64;

    let state = Arc::clone(&shared);
    // Device frames since the (last) start of the song.
    let mut frame = 0usize;
    let stream = device
        .build_output_stream::<f32, _, _>(
            config,
            move |buf: &mut [f32], _| {
                let enabled: [bool; CHANNELS] =
                    std::array::from_fn(|ch| !state.muted[ch].load(Ordering::Relaxed));
                for out in buf.chunks_mut(channels) {
                    let mut src = (frame as f64 * ratio) as usize;
                    if src >= len && state.repeat.load(Ordering::Relaxed) {
                        frame = 0;
                        src = 0;
                    }
                    let v = if src < len {
                        frame += 1;
                        apu::mix(&stems, src, enabled)
                    } else {
                        state.finished.store(true, Ordering::Relaxed);
                        0.0
                    };
                    out.fill(v);
                }
                state.position.store(
                    ((frame as f64 * ratio) as usize).min(len),
                    Ordering::Relaxed,
                );
            },
            |e| eprintln!("audio error: {e}"),
            None,
        )
        .map_err(|e| format!("could not open audio stream: {e}"))?;
    stream
        .play()
        .map_err(|e| format!("could not start playback: {e}"))?;

    let total = duration(len);
    if !interactive {
        println!("playing {title} ({total}), ctrl-c to stop");
        while !shared.finished.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(50));
        }
        return Ok(());
    }

    let bars = len / (samples_per_row * ROWS_PER_PATTERN).max(1);
    let raw = RawMode::enable()?;
    let mut shown: Vec<String> = Vec::new();
    loop {
        let screen = status_screen(title, &shared, len, bars, samples_per_row);
        if screen != shown {
            draw(&screen, shown.len());
            shown = screen;
        }
        if shared.finished.load(Ordering::Relaxed) {
            break;
        }
        if !event::poll(Duration::from_millis(50)).map_err(|e| e.to_string())? {
            continue;
        }
        let Event::Key(key) = event::read().map_err(|e| e.to_string())? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Char(c @ '1'..='4') => {
                let ch = c as usize - '1' as usize;
                shared.muted[ch].fetch_xor(true, Ordering::Relaxed);
            }
            KeyCode::Char('r') => {
                shared.repeat.fetch_xor(true, Ordering::Relaxed);
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
            KeyCode::Char('q') | KeyCode::Esc => break,
            _ => {}
        }
    }
    drop(raw);
    println!();
    Ok(())
}

/// The status block, one string per terminal line:
/// title and position on top, one line per channel, keys at the bottom.
fn status_screen(
    title: &str,
    shared: &Shared,
    len: usize,
    bars: usize,
    samples_per_row: usize,
) -> Vec<String> {
    let pos = shared.position.load(Ordering::Relaxed);
    let bar = (pos / (samples_per_row * ROWS_PER_PATTERN).max(1) + 1).min(bars.max(1));
    let finished = shared.finished.load(Ordering::Relaxed);
    let repeat = shared.repeat.load(Ordering::Relaxed);

    let state = if finished {
        "■".dark_grey()
    } else {
        "▶".green()
    };
    let repeat = if repeat {
        "on".green()
    } else {
        "off".dark_grey()
    };
    let mut lines = vec![
        title.bold().to_string(),
        format!(
            "{state} {} / {}   bar {}   repeat {repeat}",
            duration(pos).bold(),
            duration(len),
            format!("{bar}/{bars}").bold(),
        ),
        String::new(),
    ];
    for (ch, name) in CHANNEL_NAMES.iter().enumerate() {
        let line = if shared.muted[ch].load(Ordering::Relaxed) {
            format!("  {}  {name:<6}  ○ muted", ch + 1).red()
        } else {
            format!("  {}  {name:<6}  ● on", ch + 1).green()
        };
        lines.push(line.to_string());
    }
    lines.push(String::new());
    // Styled pieces end in a full reset, so each one is styled on its own
    // rather than nested inside a grey line.
    let keys: Vec<String> = [("1-4", "mute"), ("r", "repeat"), ("q", "quit")]
        .iter()
        .map(|(key, action)| format!("{} {}", key.bold(), action.dark_grey()))
        .collect();
    lines.push(keys.join("   "));
    lines
}

/// Draw `screen`, replacing the `previous` lines drawn before it.
///
/// Raw mode turns off newline translation, so lines are joined with `\r\n`.
fn draw(screen: &[String], previous: usize) {
    let mut out = std::io::stdout();
    if previous > 1 {
        let _ = queue!(out, cursor::MoveUp(previous as u16 - 1));
    }
    let _ = queue!(
        out,
        cursor::MoveToColumn(0),
        terminal::Clear(terminal::ClearType::FromCursorDown)
    );
    let _ = write!(out, "{}", screen.join("\r\n"));
    let _ = out.flush();
}

/// `m:ss` for a sample count.
fn duration(samples: usize) -> String {
    let secs = samples / apu::SAMPLE_RATE as usize;
    format!("{}:{:02}", secs / 60, secs % 60)
}
