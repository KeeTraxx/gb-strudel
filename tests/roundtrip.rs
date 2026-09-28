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

    assert!(failures.is_empty(), "round-trip failures:\n{}", failures.join("\n"));
    assert!(checked >= 12, "expected to check the bundled fixtures, got {checked}");
}
