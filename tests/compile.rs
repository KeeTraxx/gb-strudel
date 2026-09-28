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
    assert_eq!(def.pulse1.bars[0].src, "f#4 c4");
    assert_eq!(def.wave.bars[0].src, "c3#not a comment");

    let def = song::parse_song_file("pulse1 = 1 | f#4 c4 # comment\n").expect("parse");
    let compiled = def.compile(&template()).expect("f#4 is a valid note");
    assert_eq!(compiled.patterns[0][0].note, 30); // f#4
}

#[test]
fn unknown_keys_are_rejected() {
    assert!(song::parse_song_file("wobble = 1 | c4\n").is_err());
}

fn bar_list(part: &song::Part) -> Vec<(&str, u32)> {
    part.bars.iter().map(|b| (b.src.as_str(), b.instrument)).collect()
}

#[test]
fn references_splice_in_bars_with_their_own_instrument() {
    let src = "\
&intro = 8 | c4 ~ a3 c4 | d4 ~ c4 ~
pulse1 = 9 | &intro | c4 e4 | &intro
";
    let def = song::parse_song_file(src).expect("parse");
    assert_eq!(
        bar_list(&def.pulse1),
        vec![
            ("c4 ~ a3 c4", 8),
            ("d4 ~ c4 ~", 8),
            ("c4 e4", 9),
            ("c4 ~ a3 c4", 8),
            ("d4 ~ c4 ~", 8),
        ]
    );
    let compiled = def.compile(&template()).expect("compile");
    assert_eq!(compiled.orders[0].len(), 6);
    assert_eq!(compiled.patterns[2 * 4][0].instrument, 9);
    assert_eq!(compiled.patterns[3 * 4][0].instrument, 8);
}

#[test]
fn a_line_can_be_only_references_with_repeats() {
    let src = "\
pulse1 = &a &b*2 &a
&a = 1 | c4
&b = 2 | d4 | e4
";
    let def = song::parse_song_file(src).expect("forward references resolve");
    assert_eq!(
        bar_list(&def.pulse1),
        vec![("c4", 1), ("d4", 2), ("e4", 2), ("d4", 2), ("e4", 2), ("c4", 1)]
    );
}

#[test]
fn instrument_less_definitions_inherit_from_the_using_line() {
    let src = "\
&riff = c4 e4 | g4
&inner = 3 | &riff
pulse1 = 8 | &riff
pulse2 = 6 | &riff | &inner
";
    let def = song::parse_song_file(src).expect("parse");
    assert_eq!(bar_list(&def.pulse1), vec![("c4 e4", 8), ("g4", 8)]);
    assert_eq!(
        bar_list(&def.pulse2),
        vec![("c4 e4", 6), ("g4", 6), ("c4 e4", 3), ("g4", 3)]
    );
}

#[test]
fn bad_references_are_rejected() {
    let cases = [
        "pulse1 = 1 | &missing\n",
        "&a = 1 | &b\n&b = 1 | &a\npulse1 = &a\n",
        "&a = 1 | c4\n&a = 1 | d4\n",
        "&a = 1 | c4\npulse1 = &a*0\n",
        "&a = 1 | c4\npulse1 = 1 | &a c4\n",
        "&riff = c4\npulse1 = &riff\n",
        "pulse1 = c4 e4\n",
        "& = 1 | c4\n",
    ];
    for src in cases {
        assert!(song::parse_song_file(src).is_err(), "expected an error for {src:?}");
    }
}
