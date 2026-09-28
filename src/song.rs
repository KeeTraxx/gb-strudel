//! Song definitions: the declarative format a user actually writes.
//!
//! A song is four channel patterns plus tempo. Channels map onto Game Boy
//! hardware exactly as the console does, which is why the channel names are
//! the hardware's and not "lead/bass/etc":
//!
//! | channel | hardware      | typical use              |
//! |---------|---------------|--------------------------|
//! | pulse1  | square + sweep| lead                     |
//! | pulse2  | square        | harmony / counter-melody |
//! | wave    | 32-sample wave| bass                     |
//! | noise   | LFSR noise    | drums                    |

use crate::pattern::{self, Event, Step};
use crate::uge::{Cell, Song, CHANNELS, ROWS_PER_PATTERN};
use std::collections::BTreeMap;

/// One bar of a channel: mini-notation plus the instrument it plays with.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub src: String,
    /// Instrument index within the channel's bank (1-15, 0 = none).
    pub instrument: u32,
}

/// One channel's part.
#[derive(Clone, Debug, Default)]
pub struct Part {
    /// One entry per bar. Bars cycle if the song is longer. Each bar carries
    /// its own instrument because `&name` references can splice in bars
    /// written for a different instrument.
    pub bars: Vec<Bar>,
}

impl Part {
    /// Build a part in code rather than from a song file.
    #[allow(dead_code)]
    pub fn new(instrument: u32, bars: &[&str]) -> Part {
        Part {
            bars: bars
                .iter()
                .map(|s| Bar {
                    src: s.to_string(),
                    instrument,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SongDef {
    pub name: String,
    pub artist: String,
    /// Ticks per row: the driver runs at 60 Hz, so rows/sec = 60 / ticks.
    pub ticks_per_row: u32,
    pub pulse1: Part,
    pub pulse2: Part,
    pub wave: Part,
    pub noise: Part,
    /// How many bars long the song is. Defaults to the longest part.
    pub bars: Option<usize>,
}

impl SongDef {
    fn parts(&self) -> [&Part; CHANNELS] {
        [&self.pulse1, &self.pulse2, &self.wave, &self.noise]
    }

    pub fn bar_count(&self) -> usize {
        self.bars.unwrap_or_else(|| {
            self.parts()
                .iter()
                .map(|p| p.bars.len())
                .max()
                .unwrap_or(0)
                .max(1)
        })
    }

    /// Compile into a `.uge`, taking instruments and wavetables from `base`.
    ///
    /// Each bar becomes one 64-row pattern per channel. hUGETracker indexes
    /// patterns globally and channels select them through order tables, so
    /// bar `b` of channel `c` is pattern `b * 4 + c` — the same interleave the
    /// bundled GB Studio songs use.
    pub fn compile(&self, base: &Song) -> Result<Song, pattern::ParseError> {
        let bars = self.bar_count();
        let mut patterns = Vec::with_capacity(bars * CHANNELS);
        let mut orders: [Vec<u32>; CHANNELS] = Default::default();

        for bar in 0..bars {
            for (ch, part) in self.parts().iter().enumerate() {
                let index = (bar * CHANNELS + ch) as u32;
                patterns.push(self.render_bar(part, bar)?);
                orders[ch].push(index);
            }
        }
        // hUGETracker stores one slot past the end of the sequence.
        for ch in orders.iter_mut() {
            ch.push(0);
        }

        let mut out = base.clone();
        out.name = self.name.clone();
        out.artist = self.artist.clone();
        out.comment = String::new();
        out.ticks_per_row = self.ticks_per_row.max(1);
        out.patterns = patterns;
        out.orders = orders;
        out.padding = vec![0u8; (bars.saturating_sub(1)) * 16];
        Ok(out)
    }

    fn render_bar(&self, part: &Part, bar: usize) -> Result<[Cell; ROWS_PER_PATTERN], pattern::ParseError> {
        let mut cells = [Cell::EMPTY; ROWS_PER_PATTERN];
        if part.bars.is_empty() {
            return Ok(cells);
        }
        let Bar { src, instrument } = &part.bars[bar % part.bars.len()];
        let steps: Vec<Step> = pattern::parse(src)?;
        let events = pattern::render(&steps, ROWS_PER_PATTERN, bar);

        for (row, ev) in events.iter().enumerate() {
            match ev {
                Event::Silence | Event::Sustain => {}
                Event::Strike(note) => {
                    cells[row] = Cell {
                        note: *note,
                        instrument: *instrument,
                        ..Cell::EMPTY
                    };
                }
            }
        }
        Ok(cells)
    }
}

/// Cut a trailing `#` comment. A `#` only starts a comment at the beginning of
/// a word, so sharps like `f#4` survive.
fn strip_comment(line: &str) -> &str {
    let mut prev_is_space = true;
    for (i, c) in line.char_indices() {
        if c == '#' && prev_is_space {
            return &line[..i];
        }
        prev_is_space = c.is_whitespace();
    }
    line
}

/// A bar before instruments are settled: `None` means "use the instrument of
/// whichever line references me".
type RawBar = (String, Option<u32>);

/// A channel or `&name` line, kept unexpanded until every definition is known
/// so references may point forward.
struct RawLine<'a> {
    lineno: usize,
    value: &'a str,
}

/// Expand one line's value into bars, resolving `&name` references.
///
/// The value is `[instrument |] item | item ...`. An item is either
/// mini-notation for one bar or a whitespace-separated list of references
/// (`&intro &verse*2`). A referenced bar keeps its definition's instrument if
/// it has one, and otherwise takes this line's.
fn expand<'a>(
    line: &RawLine<'a>,
    defs: &BTreeMap<&'a str, RawLine<'a>>,
    stack: &mut Vec<&'a str>,
) -> Result<Vec<RawBar>, String> {
    let at = line.lineno;
    let mut chunks = line.value.split('|').map(str::trim).peekable();
    let instrument = match chunks.peek().map(|h| h.parse::<u32>()) {
        Some(Ok(n)) => {
            chunks.next();
            Some(n)
        }
        _ => None,
    };

    let mut bars = Vec::new();
    for chunk in chunks {
        if !chunk.starts_with('&') {
            bars.push((chunk.to_string(), instrument));
            continue;
        }
        for token in chunk.split_whitespace() {
            let reference = token
                .strip_prefix('&')
                .ok_or_else(|| format!("line {at}: '{token}' is in a bar of references; put notes in their own bar"))?;
            let (name, times) = match reference.split_once('*') {
                Some((name, n)) => match n.parse::<usize>() {
                    Ok(n) if n > 0 => (name, n),
                    _ => return Err(format!("line {at}: '{token}' needs a positive repeat count")),
                },
                None => (reference, 1),
            };
            let (&name, def) = defs
                .get_key_value(name)
                .ok_or_else(|| format!("line {at}: '&{name}' is not defined"))?;
            if stack.contains(&name) {
                let chain: Vec<String> = stack.iter().chain([&name]).map(|n| format!("&{n}")).collect();
                return Err(format!("line {at}: circular reference {}", chain.join(" -> ")));
            }
            stack.push(name);
            let expanded = expand(def, defs, stack)?;
            stack.pop();
            for _ in 0..times {
                bars.extend(
                    expanded
                        .iter()
                        .map(|(src, inst)| (src.clone(), inst.or(instrument))),
                );
            }
        }
    }
    if bars.is_empty() {
        return Err(format!("line {at}: no bars"));
    }
    Ok(bars)
}

/// Parse the simple `key = value` song file format.
///
/// ```text
/// name   = Coffee Break
/// tempo  = 6            # ticks per row
/// &hook  = 3 | c4 e4 g4 c5 | <a4 f4> ~ c5 ~
/// pulse1 = 5 | &hook*2 | g4 . . ~
/// wave   = 2 | c2*4
/// ```
///
/// Bars are separated by `|`; the number before the first `|` is the
/// instrument. `&name = ...` defines a reusable run of bars, which any
/// channel or definition can splice in as a bar item. Blank lines and `#`
/// comments are ignored; a comment's `#` must start a word, so it cannot be
/// confused with a sharp.
pub fn parse_song_file(src: &str) -> Result<SongDef, String> {
    let mut def = SongDef {
        ticks_per_row: 6,
        ..Default::default()
    };
    let mut defs: BTreeMap<&str, RawLine> = BTreeMap::new();
    let mut channels: Vec<(&str, RawLine)> = Vec::new();

    for (lineno, raw) in src.lines().enumerate() {
        let lineno = lineno + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("line {lineno}: expected 'key = value'"))?;
        let key = key.trim();
        let value = value.trim();

        if let Some(name) = key.strip_prefix('&') {
            let valid = !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            if !valid {
                return Err(format!("line {lineno}: '&{name}' is not a valid name"));
            }
            if let Some(prev) = defs.insert(name, RawLine { lineno, value }) {
                return Err(format!("line {lineno}: '&{name}' is already defined on line {}", prev.lineno));
            }
            continue;
        }

        match key {
            "name" => def.name = value.to_string(),
            "artist" => def.artist = value.to_string(),
            "tempo" | "ticks" => {
                def.ticks_per_row = value
                    .parse()
                    .map_err(|_| format!("line {lineno}: '{value}' is not a tick count"))?
            }
            "bars" => {
                def.bars = Some(
                    value
                        .parse()
                        .map_err(|_| format!("line {lineno}: '{value}' is not a bar count"))?,
                )
            }
            "pulse1" | "pulse2" | "wave" | "noise" => channels.push((key, RawLine { lineno, value })),
            other => return Err(format!("line {lineno}: unknown key '{other}'")),
        }
    }

    for (key, line) in channels {
        let bars = expand(&line, &defs, &mut Vec::new())?
            .into_iter()
            .enumerate()
            .map(|(i, (src, instrument))| {
                let instrument = instrument.ok_or_else(|| {
                    format!(
                        "line {}: bar {} of {key} has no instrument; start the line with one, e.g. '{key} = 8 | ...'",
                        line.lineno,
                        i + 1
                    )
                })?;
                Ok(Bar { src, instrument })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let part = Part { bars };
        match key {
            "pulse1" => def.pulse1 = part,
            "pulse2" => def.pulse2 = part,
            "wave" => def.wave = part,
            _ => def.noise = part,
        }
    }
    Ok(def)
}
