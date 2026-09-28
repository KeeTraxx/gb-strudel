//! A Strudel-flavoured mini-notation for Game Boy channels.
//!
//! A pattern string is a sequence of steps separated by whitespace. Each step
//! takes an equal slice of the cycle (one bar), so `"c4 e4 g4 c5"` is four
//! quarter notes and `"c4 e4"` is two half notes.
//!
//! ```text
//!   c4 e4 g4      notes (letter, optional # or b, octave 3-8)
//!   ~             rest (leaves the row empty)
//!   .             hold (sustains the previous note)
//!   [c4 e4]       subdivide one step into a group
//!   c4*4          repeat a step 4 times within its slice
//!   <c4 e4>       alternate: pick one per cycle, in turn
//! ```
//!
//! `parse` turns a string into `Step`s; `render` samples them onto a fixed
//! grid of tracker rows, which is what a `.uge` pattern needs.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Rest,
    Hold,
    Note(u32),
    /// A group that subdivides its time slice among its children.
    Group(Vec<Step>),
    /// One child per cycle, chosen round-robin.
    Alternate(Vec<Step>),
    /// A step repeated `n` times inside its own slice.
    Repeat(Box<Step>, usize),
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at token {})", self.message, self.position)
    }
}

impl std::error::Error for ParseError {}

/// Parse a note name like `c4`, `f#3`, `bb5` into a hUGE note number.
///
/// The playable range is note 0 (C2, 65.41 Hz) to note 71 (B7); anything
/// outside it is a parse error rather than a silent clamp, since a wrong
/// octave is a composing mistake worth hearing about.
pub fn parse_note(s: &str) -> Option<u32> {
    let b = s.as_bytes();
    if b.is_empty() {
        return None;
    }
    let base = match b[0].to_ascii_lowercase() {
        b'c' => 0,
        b'd' => 2,
        b'e' => 4,
        b'f' => 5,
        b'g' => 7,
        b'a' => 9,
        b'b' => 11,
        _ => return None,
    };
    let mut i = 1;
    let mut semis: i32 = base;
    while i < b.len() {
        match b[i] {
            b'#' => {
                semis += 1;
                i += 1;
            }
            b's' => {
                semis += 1;
                i += 1;
            }
            // A 'b' directly after the letter is a flat; `bb5` is B-flat 5.
            b'b' if i == 1 || b[i - 1] == b'b' || b[i - 1] == b'#' => {
                semis -= 1;
                i += 1;
            }
            _ => break,
        }
    }
    let octave: i32 = s[i..].parse().ok()?;
    // hUGE note 0 is 65.41 Hz, i.e. C2 in scientific pitch notation. (The
    // driver's headers label it `C_3`, following the tracker convention of
    // numbering an octave higher; we take the names a musician would write,
    // verified against hUGE_note_table.inc: note 33 = A4 = 440 Hz.)
    let n = (octave - 2) * 12 + semis;
    if (0..crate::uge::LAST_NOTE as i32).contains(&n) {
        Some(n as u32)
    } else {
        None
    }
}

/// Split a pattern string into top-level tokens, honouring `[]` and `<>`.
fn tokenize(src: &str) -> Result<Vec<String>, ParseError> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in src.chars() {
        match c {
            '[' | '<' => {
                depth += 1;
                cur.push(c);
            }
            ']' | '>' => {
                depth -= 1;
                if depth < 0 {
                    return Err(ParseError {
                        message: format!("unmatched '{c}'"),
                        position: out.len(),
                    });
                }
                cur.push(c);
            }
            c if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if depth != 0 {
        return Err(ParseError {
            message: "unclosed group".into(),
            position: out.len(),
        });
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}

fn parse_token(tok: &str, index: usize) -> Result<Step, ParseError> {
    // `x*n` repeats a step inside its slice.
    if let Some((head, count)) = tok.rsplit_once('*') {
        let n: usize = count.parse().map_err(|_| ParseError {
            message: format!("'{count}' is not a repeat count"),
            position: index,
        })?;
        if n == 0 {
            return Err(ParseError {
                message: "repeat count must be at least 1".into(),
                position: index,
            });
        }
        return Ok(Step::Repeat(Box::new(parse_token(head, index)?), n));
    }
    if let Some(inner) = tok.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        return Ok(Step::Group(parse(inner)?));
    }
    if let Some(inner) = tok.strip_prefix('<').and_then(|t| t.strip_suffix('>')) {
        let alts = parse(inner)?;
        if alts.is_empty() {
            return Err(ParseError {
                message: "empty alternation".into(),
                position: index,
            });
        }
        return Ok(Step::Alternate(alts));
    }
    match tok {
        "~" => Ok(Step::Rest),
        "." | "-" => Ok(Step::Hold),
        _ => parse_note(tok).map(Step::Note).ok_or_else(|| ParseError {
            message: format!("'{tok}' is not a note, rest or group"),
            position: index,
        }),
    }
}

pub fn parse(src: &str) -> Result<Vec<Step>, ParseError> {
    tokenize(src)?
        .iter()
        .enumerate()
        .map(|(i, t)| parse_token(t, i))
        .collect()
}

/// What lands on a single tracker row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Start a new note.
    Strike(u32),
    /// Keep the previous note ringing (no new trigger).
    Sustain,
    /// Nothing here.
    Silence,
}

/// Sample `steps` onto `rows` tracker rows for the given cycle.
///
/// Rendering is by time slice rather than by row count, so a pattern whose
/// step count does not divide the row count still lands as close to the right
/// place as the grid allows, instead of being rejected or truncated.
pub fn render(steps: &[Step], rows: usize, cycle: usize) -> Vec<Event> {
    let mut out = vec![Event::Silence; rows];
    if steps.is_empty() || rows == 0 {
        return out;
    }
    emit(steps, 0.0, 1.0, rows, cycle, &mut out);
    out
}

fn emit(steps: &[Step], start: f64, end: f64, rows: usize, cycle: usize, out: &mut Vec<Event>) {
    let span = (end - start) / steps.len() as f64;
    for (i, step) in steps.iter().enumerate() {
        let s = start + span * i as f64;
        let e = s + span;
        emit_one(step, s, e, rows, cycle, out);
    }
}

fn emit_one(step: &Step, s: f64, e: f64, rows: usize, cycle: usize, out: &mut Vec<Event>) {
    let row = (s * rows as f64).round() as usize;
    if row >= rows {
        return;
    }
    match step {
        Step::Rest => {}
        Step::Hold => out[row] = Event::Sustain,
        Step::Note(n) => out[row] = Event::Strike(*n),
        Step::Group(children) => emit(children, s, e, rows, cycle, out),
        Step::Alternate(children) => {
            let pick = &children[cycle % children.len()];
            emit_one(pick, s, e, rows, cycle, out);
        }
        Step::Repeat(inner, n) => {
            let span = (e - s) / *n as f64;
            for k in 0..*n {
                let ks = s + span * k as f64;
                emit_one(inner, ks, ks + span, rows, cycle, out);
            }
        }
    }
}
