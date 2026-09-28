//! End-to-end: a song source compiles to a .uge that re-parses identically
//! and matches the structure hUGETracker itself writes.

use std::fs;
use std::path::Path;

#[path = "../src/uge.rs"]
mod uge;
#[path = "../src/pattern.rs"]
mod pattern;
#[path = "../src/song.rs"]
mod song;

fn template() -> uge::Song {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/template.uge");
    uge::Song::parse(&fs::read(p).expect("template")).expect("parse template")
}

const SRC: &str = "\
name   = Test
tempo  = 6
pulse1 = 8 | c4 e4 g4 c5 | d4 ~ c4 ~
wave   = 4 | c2*4 | f2 f2 c2 c2
";

#[test]
fn compiled_song_reparses_to_the_same_bytes() {
    let def = song::parse_song_file(SRC).expect("parse source");
    let compiled = def.compile(&template()).expect("compile");
    let bytes = compiled.write();

    let reparsed = uge::Song::parse(&bytes).expect("re-parse our own output");
    assert_eq!(reparsed.write(), bytes, "writer and parser disagree");
    assert_eq!(reparsed.name, "Test");
    assert_eq!(reparsed.ticks_per_row, 6);
}

#[test]
fn channels_interleave_patterns_like_the_shipped_songs() {
    // GB Studio's own .uge files order channel c bar b as pattern b*4+c.
    let def = song::parse_song_file(SRC).expect("parse");
    let compiled = def.compile(&template()).expect("compile");
    assert_eq!(compiled.orders[0], vec![0, 4, 0]);
    assert_eq!(compiled.orders[1], vec![1, 5, 0]);
    assert_eq!(compiled.orders[2], vec![2, 6, 0]);
    assert_eq!(compiled.orders[3], vec![3, 7, 0]);
    assert_eq!(compiled.patterns.len(), 8);
}

#[test]
fn notes_land_on_the_right_rows_with_the_right_instrument() {
    let def = song::parse_song_file(SRC).expect("parse");
    let compiled = def.compile(&template()).expect("compile");

    // Bar 0 of pulse1 is pattern 0: "c4 e4 g4 c5" over 64 rows.
    let p = &compiled.patterns[0];
    assert_eq!(p[0].note, 24); // c4
    assert_eq!(p[0].instrument, 8);
    assert_eq!(p[16].note, 28); // e4
    assert_eq!(p[32].note, 31); // g4
    assert_eq!(p[48].note, 36); // c5
    assert_eq!(p[1].note, uge::NO_NOTE);
}

#[test]
fn a_bad_note_fails_the_build_instead_of_going_silent() {
    let bad = "name = X\npulse1 = 1 | c4 zz g4\n";
    let def = song::parse_song_file(bad).expect("file parses");
    assert!(def.compile(&template()).is_err(), "expected a pattern error");
}

#[test]
fn sharps_are_not_mistaken_for_comments() {
    let src = "# header\npulse1 = 1 | f#4 c4 # trailing comment\nwave = 1 | c3#not a comment\n";
    let def = song::parse_song_file(src).expect("parse");
    assert_eq!(def.pulse1.bars, vec!["f#4 c4"]);
    assert_eq!(def.wave.bars, vec!["c3#not a comment"]);

    let def = song::parse_song_file("pulse1 = 1 | f#4 c4 # comment\n").expect("parse");
    let compiled = def.compile(&template()).expect("f#4 is a valid note");
    assert_eq!(compiled.patterns[0][0].note, 30); // f#4
}

#[test]
fn unknown_keys_are_rejected() {
    assert!(song::parse_song_file("wobble = 1 | c4\n").is_err());
}
