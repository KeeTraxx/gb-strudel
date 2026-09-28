//! A small Game Boy APU model, enough to preview a song faithfully.
//!
//! This is not a cycle-accurate emulator. It reproduces the parts that decide
//! whether a tune *sounds right*: the four channels' timbres, the note table,
//! and the 60 Hz row clock the hUGE driver runs on. Pulse channels are square
//! waves with selectable duty, the wave channel reads a 32-sample table, and
//! noise uses the console's 15-bit LFSR.

use crate::uge::{CHANNELS, NO_NOTE, ROWS_PER_PATTERN, Song};

pub const SAMPLE_RATE: u32 = 48_000;
/// The driver ticks at the Game Boy's vertical blank rate.
pub const FRAME_RATE: f64 = 59.7275;

/// Frequency of a hUGE note number.
///
/// Verified against `hUGE_note_table.inc`: note 0 has period 44, i.e.
/// 131072 / (2048 - 44) = 65.41 Hz (C2), and note 33 is 439.84 Hz (A4).
fn note_hz(note: u32) -> f64 {
    440.0 * 2f64.powf((note as f64 - 33.0) / 12.0)
}

/// Per-sample multiplier that halves an amplitude every `seconds`.
fn half_life_decay(seconds: f64) -> f32 {
    0.5f64.powf(1.0 / (seconds * SAMPLE_RATE as f64)) as f32
}

const DUTY_TABLE: [[f32; 8]; 4] = [
    [1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0], // 12.5%
    [1.0, 1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0],  // 25%
    [1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0],    // 50%
    [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0],      // 75%
];

#[derive(Default, Clone, Copy)]
struct Pulse {
    hz: f64,
    phase: f64,
    duty: usize,
    level: f32,
}

impl Pulse {
    fn sample(&mut self, dt: f64) -> f32 {
        if self.level <= 0.0 {
            return 0.0;
        }
        self.phase = (self.phase + self.hz * dt).fract();
        let step = (self.phase * 8.0) as usize & 7;
        DUTY_TABLE[self.duty][step] * self.level
    }
}

#[derive(Clone)]
struct Wave {
    hz: f64,
    phase: f64,
    table: [f32; 32],
    level: f32,
}

impl Wave {
    fn sample(&mut self, dt: f64) -> f32 {
        if self.level <= 0.0 {
            return 0.0;
        }
        self.phase = (self.phase + self.hz * dt).fract();
        let idx = (self.phase * 32.0) as usize & 31;
        self.table[idx] * self.level
    }
}

#[derive(Clone, Copy)]
struct Noise {
    lfsr: u16,
    hz: f64,
    acc: f64,
    out: f32,
    level: f32,
}

impl Noise {
    fn sample(&mut self, dt: f64) -> f32 {
        if self.level <= 0.0 {
            return 0.0;
        }
        self.acc += self.hz * dt;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            // 15-bit LFSR, as on hardware.
            let bit = (self.lfsr ^ (self.lfsr >> 1)) & 1;
            self.lfsr = (self.lfsr >> 1) | (bit << 14);
            self.out = if self.lfsr & 1 == 0 { 1.0 } else { -1.0 };
        }
        self.out * self.level
    }
}

/// Renders a `Song` to f32 mono samples.
pub struct Renderer {
    pulse: [Pulse; 2],
    wave: Wave,
    noise: Noise,
    /// Per-sample amplitude decay for pitched channels, and a faster one for
    /// noise. Set from a half-life in [`Renderer::new`] so the fade is tied to
    /// wall-clock time rather than to the sample rate.
    decay: f32,
    noise_decay: f32,
}

impl Renderer {
    pub fn new() -> Renderer {
        // A mild sawtooth: a neutral default for the wave channel, since we
        // do not interpret hUGETracker instrument definitions.
        let mut table = [0.0f32; 32];
        for (i, v) in table.iter_mut().enumerate() {
            *v = (i as f32 / 31.0) * 2.0 - 1.0;
        }
        Renderer {
            pulse: [Pulse::default(); 2],
            wave: Wave {
                hz: 0.0,
                phase: 0.0,
                table,
                level: 0.0,
            },
            noise: Noise {
                lfsr: 0x7FFF,
                hz: 0.0,
                acc: 0.0,
                out: 1.0,
                level: 0.0,
            },
            // Pitched notes half-fade over ~0.6s so sustained notes ring
            // across a row but successive notes still articulate; percussion
            // is much shorter or every hit smears into the next.
            decay: half_life_decay(0.60),
            noise_decay: half_life_decay(0.05),
        }
    }

    /// Flatten the order tables into the actual sequence of rows to play.
    fn rows(song: &Song) -> Vec<[Option<u32>; CHANNELS]> {
        let seq = song.sequence_len();
        let mut out = Vec::new();
        for step in 0..seq {
            for row in 0..ROWS_PER_PATTERN {
                let mut slot = [None; CHANNELS];
                for (note, order) in slot.iter_mut().zip(&song.orders) {
                    let Some(&pat) = order.get(step) else {
                        continue;
                    };
                    let Some(pattern) = song.pattern(pat) else {
                        continue;
                    };
                    let cell = pattern.rows[row];
                    if cell.note < NO_NOTE {
                        *note = Some(cell.note);
                    }
                }
                out.push(slot);
            }
        }
        out
    }

    /// How many output samples one tracker row lasts.
    pub fn samples_per_row(song: &Song) -> usize {
        let rows_per_sec = FRAME_RATE / song.ticks_per_row.max(1) as f64;
        (SAMPLE_RATE as f64 / rows_per_sec) as usize
    }

    /// Render the whole song to mono f32 samples at [`SAMPLE_RATE`].
    pub fn render(&mut self, song: &Song) -> Vec<f32> {
        let stems = self.render_stems(song);
        (0..stems[0].len())
            .map(|i| mix(&stems, i, [true; CHANNELS]))
            .collect()
    }

    /// Render each channel separately, unscaled, so a player can mute
    /// channels live. [`mix`] combines them into what [`Renderer::render`]
    /// produces.
    pub fn render_stems(&mut self, song: &Song) -> [Vec<f32>; CHANNELS] {
        let rows = Self::rows(song);
        let samples_per_row = Self::samples_per_row(song);
        let dt = 1.0 / SAMPLE_RATE as f64;

        let mut stems: [Vec<f32>; CHANNELS] =
            std::array::from_fn(|_| Vec::with_capacity(rows.len() * samples_per_row));
        for slot in rows {
            if let Some(n) = slot[0] {
                self.pulse[0].hz = note_hz(n);
                self.pulse[0].duty = 2;
                self.pulse[0].level = 0.22;
            }
            if let Some(n) = slot[1] {
                self.pulse[1].hz = note_hz(n);
                self.pulse[1].duty = 1;
                self.pulse[1].level = 0.18;
            }
            if let Some(n) = slot[2] {
                self.wave.hz = note_hz(n);
                self.wave.level = 0.20;
            }
            if let Some(n) = slot[3] {
                // Noise has no pitch as such; map the note onto the LFSR rate
                // so higher notes read as brighter hits.
                self.noise.hz = note_hz(n) * 24.0;
                self.noise.level = 0.15;
            }

            for _ in 0..samples_per_row {
                stems[0].push(self.pulse[0].sample(dt));
                stems[1].push(self.pulse[1].sample(dt));
                stems[2].push(self.wave.sample(dt));
                stems[3].push(self.noise.sample(dt));
                for p in self.pulse.iter_mut() {
                    p.level *= self.decay;
                }
                self.wave.level *= self.decay;
                self.noise.level *= self.noise_decay;
            }
        }
        stems
    }
}

/// Mix sample `i` of the enabled stems into one output sample.
pub fn mix(stems: &[Vec<f32>; CHANNELS], i: usize, enabled: [bool; CHANNELS]) -> f32 {
    let mut s = 0.0;
    for (stem, on) in stems.iter().zip(enabled) {
        if on {
            s += stem[i];
        }
    }
    (s * 0.6).clamp(-1.0, 1.0)
}

impl Default for Renderer {
    fn default() -> Self {
        Renderer::new()
    }
}
