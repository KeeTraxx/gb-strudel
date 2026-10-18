//! wasm bindings for the web editor: compile a `.gbs` song, render it through
//! the APU preview, and hand back the `.uge` bytes.

// The CLI's modules, shared rather than copied. Not everything in them is used
// here (the CLI-only helpers look dead to this crate).
#[allow(dead_code)]
#[path = "../../../src/apu.rs"]
mod apu;
#[allow(dead_code)]
#[path = "../../../src/pattern.rs"]
mod pattern;
#[allow(dead_code)]
#[path = "../../../src/song.rs"]
mod song;
#[allow(dead_code)]
#[path = "../../../src/uge.rs"]
mod uge;

use wasm_bindgen::prelude::*;

const BUNDLED_TEMPLATE: &[u8] = include_bytes!("../../../templates/template.uge");

fn template() -> Result<uge::Song, JsError> {
    uge::Song::parse(BUNDLED_TEMPLATE).map_err(|e| JsError::new(&format!("bundled template: {e}")))
}

/// A compiled song. Compiling is cheap, so the editor does it on every edit to
/// report errors; rendering audio only happens on [`Compiled::render`].
#[wasm_bindgen]
pub struct Compiled {
    song: uge::Song,
    bars: usize,
}

/// Parse and compile `.gbs` source against the bundled template.
#[wasm_bindgen]
pub fn compile(src: &str) -> Result<Compiled, JsError> {
    let def = song::parse_song_file(src).map_err(|e| JsError::new(&e))?;
    let song = def
        .compile(&template()?)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(Compiled {
        song,
        bars: def.bar_count(),
    })
}

#[wasm_bindgen]
impl Compiled {
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.song.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn bars(&self) -> usize {
        self.bars
    }

    #[wasm_bindgen(getter, js_name = samplesPerRow)]
    pub fn samples_per_row(&self) -> usize {
        apu::Renderer::samples_per_row(&self.song)
    }

    /// The four channel stems, planar: pulse1, pulse2, wave, noise, each the
    /// same length. Unscaled, like `Renderer::render_stems`; the player
    /// applies the same 0.6 master gain as `apu::mix`.
    pub fn render(&self) -> Vec<f32> {
        apu::Renderer::new().render_stems(&self.song).concat()
    }

    /// The `.uge` file, ready for GB Studio.
    pub fn uge(&self) -> Vec<u8> {
        self.song.write()
    }
}

#[wasm_bindgen(js_name = sampleRate)]
pub fn sample_rate() -> u32 {
    apu::SAMPLE_RATE
}

/// The bundled template's instrument names: 15 pulse, 15 wave, 15 noise.
#[wasm_bindgen(js_name = instrumentNames)]
pub fn instrument_names() -> Result<Vec<String>, JsError> {
    Ok(template()?.instrument_names())
}
