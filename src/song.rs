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

/// One channel's part.
#[derive(Clone, Debug, Default)]
pub struct Part {
    /// Mini-notation, one string per bar. Bars cycle if the song is longer.
    pub bars: Vec<String>,
    /// Instrument index within this channel's bank (1-15, 0 = none).
    pub instrument: u32,
}

impl Part {
    /// Build a part in code rather than from a song file.
    #[allow(dead_code)]
    pub fn new(instrument: u32, bars: &[&str]) -> Part {
        Part {
            bars: bars.iter().map(|s| s.to_string()).collect(),
            instrument,
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
        let src = &part.bars[bar % part.bars.len()];
        let steps: Vec<Step> = pattern::parse(src)?;
        let events = pattern::render(&steps, ROWS_PER_PATTERN, bar);

        for (row, ev) in events.iter().enumerate() {
            match ev {
                Event::Silence | Event::Sustain => {}
                Event::Strike(note) => {
                    cells[row] = Cell {
                        note: *note,
                        instrument: part.instrument,
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

/// Parse the simple `key = value` song file format.
///
/// ```text
/// name   = Coffee Break
/// tempo  = 6            # ticks per row
/// pulse1 = 3 | c4 e4 g4 c5 | <a4 f4> ~ c5 ~
/// wave   = 2 | c2*4
/// ```
///
/// Bars are separated by `|`; the number before the first `|` is the
/// instrument. Blank lines and `#` comments are ignored; a comment's `#` must
/// start a word, so it cannot be confused with a sharp.
pub fn parse_song_file(src: &str) -> Result<SongDef, String> {
    let mut def = SongDef {
        ticks_per_row: 6,
        ..Default::default()
    };
    let mut seen: BTreeMap<&str, ()> = BTreeMap::new();

    for (lineno, raw) in src.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("line {}: expected 'key = value'", lineno + 1))?;
        let key = key.trim();
        let value = value.trim();

        let part = || -> Result<Part, String> {
            let mut chunks = value.split('|');
            let head = chunks.next().unwrap_or("").trim();
            let instrument: u32 = head
                .parse()
                .map_err(|_| format!("line {}: '{head}' is not an instrument number", lineno + 1))?;
            let bars: Vec<String> = chunks.map(|c| c.trim().to_string()).collect();
            if bars.is_empty() {
                return Err(format!("line {}: no bars after instrument", lineno + 1));
            }
            Ok(Part { bars, instrument })
        };

        match key {
            "name" => def.name = value.to_string(),
            "artist" => def.artist = value.to_string(),
            "tempo" | "ticks" => {
                def.ticks_per_row = value
                    .parse()
                    .map_err(|_| format!("line {}: '{value}' is not a tick count", lineno + 1))?
            }
            "bars" => {
                def.bars = Some(
                    value
                        .parse()
                        .map_err(|_| format!("line {}: '{value}' is not a bar count", lineno + 1))?,
                )
            }
            "pulse1" => def.pulse1 = part()?,
            "pulse2" => def.pulse2 = part()?,
            "wave" => def.wave = part()?,
            "noise" => def.noise = part()?,
            other => return Err(format!("line {}: unknown key '{other}'", lineno + 1)),
        }
        seen.insert(key, ());
    }
    Ok(def)
}
