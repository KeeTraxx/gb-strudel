//! Reading and writing hUGETracker `.uge` v6 files.
//!
//! The layout was recovered by round-tripping the `.uge` files that ship
//! inside GB Studio 4.3.2 (`tests/fixtures`), cross-checked against the
//! runtime semantics in `hUGEDriver.h`:
//!
//! ```text
//! u32   version (6)
//! [256] name, [256] artist, [256] comment   (length-prefixed, fixed width)
//! u32   0 in every bundled file; purpose unknown, preserved verbatim
//! 45 instruments (15 duty, 15 wave, 15 noise), 1385 bytes each except the
//!       last, which is 4 bytes shorter -- the region is 62321 bytes, not
//!       45 * 1385 = 62325
//! 16 x 32    wavetables (one sample per byte, 0-15)
//! u32   ticks per row
//! u32   timer flags
//! u32   row count (patterns * 256; 256 = 64 rows * 4 channels)
//! cells      (patterns * 64 + 1) * 17 bytes, one cell per row per pattern
//! 4 x order table (u32 len, then len * u32 pattern index)
//! [..]  trailing routine table, preserved verbatim
//! ```
//!
//! A cell is four little-endian u32s plus a byte: note, instrument, effect
//! code, effect parameter, and a trailing unused byte. Note 90 (`NO_NOTE`)
//! means "no note here"; playable notes are 0-71 = C-3 through B-8.

use std::io::{Error, ErrorKind, Result};

pub const NO_NOTE: u32 = 90;
pub const LAST_NOTE: u32 = 72;
pub const ROWS_PER_PATTERN: usize = 64;
pub const CHANNELS: usize = 4;

const VERSION: u32 = 6;
const STRING_FIELD: usize = 256;
const INSTRUMENT_SIZE: usize = 1385;
const INSTRUMENT_COUNT: usize = 45;
/// The final instrument record is 4 bytes shorter than the rest.
const INSTRUMENT_REGION: usize = INSTRUMENT_COUNT * INSTRUMENT_SIZE - 4;
const WAVE_COUNT: usize = 16;
const WAVE_SIZE: usize = 32;

/// One tracker cell: a note, the instrument playing it, and one effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub note: u32,
    pub instrument: u32,
    pub effect_code: u32,
    pub effect_param: u32,
    /// Final byte of the on-disk cell. Preserved so reads round-trip exactly.
    pub tail: u8,
}

impl Cell {
    pub const EMPTY: Cell = Cell {
        note: NO_NOTE,
        instrument: 0,
        effect_code: 0,
        effect_param: 0,
        tail: 0,
    };

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.note == NO_NOTE && self.instrument == 0 && self.effect_code == 0
    }
}

impl Default for Cell {
    fn default() -> Self {
        Cell::EMPTY
    }
}

/// A `.uge` song. Instrument, wavetable and routine blobs are kept as raw
/// bytes: this crate composes patterns, it does not design instruments, so
/// carrying them verbatim from a template is both simpler and safer.
#[derive(Clone)]
pub struct Song {
    pub name: String,
    pub artist: String,
    pub comment: String,
    /// A u32 sitting between the comment and the instruments. Zero in every
    /// bundled file; carried through so output stays byte-exact.
    pub unknown: u32,
    pub instruments: Vec<u8>,
    pub waves: Vec<u8>,
    /// Ticks per row. The hUGE driver runs at 60 Hz, so row rate = 60 / ticks.
    pub ticks_per_row: u32,
    pub timer_flags: u32,
    /// `patterns[p][row]` — one cell per row. Channels select patterns via
    /// the order tables, which is how hUGETracker shares a pattern across
    /// channels.
    pub patterns: Vec<[Cell; ROWS_PER_PATTERN]>,
    /// Trailing cell present in every observed file; kept for exact output.
    pub trailing_cell: Cell,
    /// Padding after the cell grid (16 bytes per pattern group past the first).
    pub padding: Vec<u8>,
    pub orders: [Vec<u32>; CHANNELS],
    pub routines: Vec<u8>,
}

struct Reader<'a> {
    d: &'a [u8],
    o: usize,
}

impl<'a> Reader<'a> {
    fn need(&self, n: usize) -> Result<()> {
        if self.o + n > self.d.len() {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                format!("truncated .uge at offset {} (wanted {} bytes)", self.o, n),
            ));
        }
        Ok(())
    }
    fn u32(&mut self) -> Result<u32> {
        self.need(4)?;
        let v = u32::from_le_bytes(self.d[self.o..self.o + 4].try_into().unwrap());
        self.o += 4;
        Ok(v)
    }
    fn u8(&mut self) -> Result<u8> {
        self.need(1)?;
        let v = self.d[self.o];
        self.o += 1;
        Ok(v)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        self.need(n)?;
        let v = &self.d[self.o..self.o + n];
        self.o += n;
        Ok(v)
    }
    /// Pascal-style string in a fixed 256-byte field.
    fn string(&mut self) -> Result<String> {
        let f = self.take(STRING_FIELD)?;
        let len = (f[0] as usize).min(STRING_FIELD - 1);
        Ok(f[1..1 + len].iter().map(|&b| b as char).collect())
    }
    fn cell(&mut self) -> Result<Cell> {
        Ok(Cell {
            note: self.u32()?,
            instrument: self.u32()?,
            effect_code: self.u32()?,
            effect_param: self.u32()?,
            tail: self.u8()?,
        })
    }
}

fn push_string(out: &mut Vec<u8>, s: &str) {
    let bytes: Vec<u8> = s.chars().map(|c| c as u8).collect();
    let len = bytes.len().min(STRING_FIELD - 1);
    let mut field = vec![0u8; STRING_FIELD];
    field[0] = len as u8;
    field[1..1 + len].copy_from_slice(&bytes[..len]);
    out.extend_from_slice(&field);
}

fn push_cell(out: &mut Vec<u8>, c: &Cell) {
    out.extend_from_slice(&c.note.to_le_bytes());
    out.extend_from_slice(&c.instrument.to_le_bytes());
    out.extend_from_slice(&c.effect_code.to_le_bytes());
    out.extend_from_slice(&c.effect_param.to_le_bytes());
    out.push(c.tail);
}

impl Song {
    pub fn parse(d: &[u8]) -> Result<Song> {
        let mut r = Reader { d, o: 0 };
        let version = r.u32()?;
        if version != VERSION {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("unsupported .uge version {version} (only v6 is supported)"),
            ));
        }
        let name = r.string()?;
        let artist = r.string()?;
        let comment = r.string()?;
        let unknown = r.u32()?;
        let instruments = r.take(INSTRUMENT_REGION)?.to_vec();
        let waves = r.take(WAVE_COUNT * WAVE_SIZE)?.to_vec();
        let ticks_per_row = r.u32()?;
        let timer_flags = r.u32()?;
        let rows = r.u32()? as usize;
        let pattern_count = rows / (ROWS_PER_PATTERN * CHANNELS);

        let mut patterns = Vec::with_capacity(pattern_count);
        for _ in 0..pattern_count {
            let mut pat = [Cell::EMPTY; ROWS_PER_PATTERN];
            for row in pat.iter_mut() {
                *row = r.cell()?;
            }
            patterns.push(pat);
        }
        let trailing_cell = r.cell()?;
        // Each group of 4 patterns past the first is followed by 16 bytes of
        // padding. Derived from every bundled fixture; kept verbatim so files
        // round-trip.
        let pad_groups = (pattern_count / CHANNELS).saturating_sub(1);
        let padding = r.take(pad_groups * 16)?.to_vec();

        let mut orders: [Vec<u32>; CHANNELS] = Default::default();
        for ch in orders.iter_mut() {
            let n = r.u32()? as usize;
            let mut list = Vec::with_capacity(n);
            for _ in 0..n {
                list.push(r.u32()?);
            }
            *ch = list;
        }
        let routines = d[r.o..].to_vec();

        Ok(Song {
            name,
            artist,
            comment,
            unknown,
            instruments,
            waves,
            ticks_per_row,
            timer_flags,
            patterns,
            trailing_cell,
            padding,
            orders,
            routines,
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(70_000);
        out.extend_from_slice(&VERSION.to_le_bytes());
        push_string(&mut out, &self.name);
        push_string(&mut out, &self.artist);
        push_string(&mut out, &self.comment);
        out.extend_from_slice(&self.unknown.to_le_bytes());
        out.extend_from_slice(&self.instruments);
        out.extend_from_slice(&self.waves);
        out.extend_from_slice(&self.ticks_per_row.to_le_bytes());
        out.extend_from_slice(&self.timer_flags.to_le_bytes());
        let rows = (self.patterns.len() * ROWS_PER_PATTERN * CHANNELS) as u32;
        out.extend_from_slice(&rows.to_le_bytes());
        for pat in &self.patterns {
            for cell in pat.iter() {
                push_cell(&mut out, cell);
            }
        }
        push_cell(&mut out, &self.trailing_cell);
        out.extend_from_slice(&self.padding);
        for ch in &self.orders {
            out.extend_from_slice(&(ch.len() as u32).to_le_bytes());
            for v in ch {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(&self.routines);
        out
    }

    /// Names of the 45 instruments, in bank order: 15 duty, 15 wave,
    /// 15 noise. Each record starts with a Pascal string in a 256-byte field.
    pub fn instrument_names(&self) -> Vec<String> {
        (0..INSTRUMENT_COUNT)
            .map(|i| {
                let rec = &self.instruments[i * INSTRUMENT_SIZE..];
                let len = (rec[0] as usize).min(STRING_FIELD - 1);
                rec[1..1 + len].iter().map(|&b| b as char).collect()
            })
            .collect()
    }

    /// Number of order entries actually played. hUGETracker stores a trailing
    /// slot past the end of the sequence, so the last entry is not played.
    pub fn sequence_len(&self) -> usize {
        self.orders
            .iter()
            .map(|o| o.len().saturating_sub(1))
            .max()
            .unwrap_or(0)
    }
}
