// The .uge layout in src/uge.rs was reverse-engineered, so the guarantee we
// actually care about is byte-exact round-tripping of files written by
// hUGETracker itself. These fixtures ship inside GB Studio 4.3.2.
use std::fs;
use std::path::Path;

#[path = "../src/uge.rs"]
mod uge;

#[test]
fn roundtrips_every_v6_fixture() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut checked = 0;
    let mut failures = Vec::new();

    for entry in fs::read_dir(&dir).expect("fixtures dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("uge") {
            continue;
        }
        let data = fs::read(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        // v5 files (hUGETracker's own sample song) are out of scope.
        if u32::from_le_bytes(data[0..4].try_into().unwrap()) != 6 {
            continue;
        }

        match uge::Song::parse(&data) {
            Ok(song) => {
                let out = song.write();
                if out != data {
                    failures.push(format!(
                        "{name}: {} bytes in, {} bytes out",
                        data.len(),
                        out.len()
                    ));
                } else {
                    checked += 1;
                }
            }
            Err(e) => failures.push(format!("{name}: parse failed: {e}")),
        }
    }

    assert!(
        failures.is_empty(),
        "round-trip failures:\n{}",
        failures.join("\n")
    );
    assert!(
        checked >= 12,
        "expected to check the bundled fixtures, got {checked}"
    );
}

fn fixture(name: &str) -> uge::Song {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    uge::Song::parse(&fs::read(path).unwrap()).unwrap()
}

// Round-tripping alone can't catch a misframed layout: a reader and writer
// that share the same wrong framing still reproduce the input. These check
// decoded values against what GB Studio shows for the same file.
#[test]
fn decodes_known_cells() {
    let song = fixture("Rulz_Outside.uge");
    assert_eq!(song.ticks_per_row, 8);
    assert_eq!(song.patterns.len(), 4);
    let ids: Vec<u32> = song.patterns.iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![0, 1, 2, 3]);

    let row0 = song.patterns[0].rows[0];
    assert_eq!(row0.note, 37); // C#5
    assert_eq!(row0.instrument, 1);
    assert_eq!(row0.effect_code, 14);
    assert_eq!(row0.effect_param, 3);
    assert_eq!(song.patterns[0].rows[1].note, uge::NO_NOTE);
}

#[test]
fn every_order_entry_names_an_existing_pattern() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let data = fs::read(&path).unwrap();
        if path.extension().and_then(|e| e.to_str()) != Some("uge")
            || u32::from_le_bytes(data[0..4].try_into().unwrap()) != 6
        {
            continue;
        }
        let song = uge::Song::parse(&data).unwrap();
        for order in &song.orders {
            for &id in order {
                assert!(song.pattern(id).is_some(), "{path:?}: no pattern {id}");
            }
        }
        let names = song.instrument_names();
        assert!(names.iter().all(|n| n.is_ascii()), "{path:?}: {names:?}");
    }
}
