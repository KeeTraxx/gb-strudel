//! Tests for the mini-notation: note naming, timing, and the group forms.

#[path = "../src/uge.rs"]
mod uge;
#[path = "../src/pattern.rs"]
mod pattern;

use pattern::{parse, parse_note, render, Event, Step};

#[test]
fn note_names_match_the_hardware_table() {
    // Anchors verified against hUGE_note_table.inc: note 0 is 65.41 Hz (C2)
    // and note 33 is 439.84 Hz (A4).
    assert_eq!(parse_note("c2"), Some(0));
    assert_eq!(parse_note("a4"), Some(33));
    assert_eq!(parse_note("b7"), Some(71));
}

#[test]
fn accidentals_resolve_both_ways() {
    assert_eq!(parse_note("c#2"), Some(1));
    assert_eq!(parse_note("db2"), Some(1));
    assert_eq!(parse_note("bb2"), Some(10));
    assert_eq!(parse_note("as2"), Some(10));
}

#[test]
fn out_of_range_notes_are_rejected_not_clamped() {
    // A silent clamp would turn a typo into a wrong-sounding song.
    assert_eq!(parse_note("c1"), None);
    assert_eq!(parse_note("c8"), None);
    assert_eq!(parse_note("h3"), None);
}

#[test]
fn steps_divide_the_bar_evenly() {
    let steps = parse("c2 e2 g2 c3").unwrap();
    let ev = render(&steps, 64, 0);
    // Four steps across 64 rows lands on every 16th row.
    assert_eq!(ev[0], Event::Strike(0));
    assert_eq!(ev[16], Event::Strike(4));
    assert_eq!(ev[32], Event::Strike(7));
    assert_eq!(ev[48], Event::Strike(12));
    assert_eq!(ev[1], Event::Silence);
}

#[test]
fn rests_leave_rows_empty() {
    let ev = render(&parse("c2 ~ c2 ~").unwrap(), 64, 0);
    assert_eq!(ev[0], Event::Strike(0));
    assert_eq!(ev[16], Event::Silence);
    assert_eq!(ev[32], Event::Strike(0));
}

#[test]
fn groups_subdivide_their_own_slice() {
    // [c2 e2] shares the first half-bar, so the second note lands at row 16.
    let ev = render(&parse("[c2 e2] g2").unwrap(), 64, 0);
    assert_eq!(ev[0], Event::Strike(0));
    assert_eq!(ev[16], Event::Strike(4));
    assert_eq!(ev[32], Event::Strike(7));
}

#[test]
fn repeat_fills_its_slice() {
    let ev = render(&parse("c2*4").unwrap(), 64, 0);
    for row in [0, 16, 32, 48] {
        assert_eq!(ev[row], Event::Strike(0), "expected a hit at row {row}");
    }
}

#[test]
fn alternation_advances_with_the_cycle() {
    let steps = parse("<c2 e2>").unwrap();
    assert_eq!(render(&steps, 64, 0)[0], Event::Strike(0));
    assert_eq!(render(&steps, 64, 1)[0], Event::Strike(4));
    assert_eq!(render(&steps, 64, 2)[0], Event::Strike(0));
}

#[test]
fn unbalanced_brackets_are_an_error() {
    assert!(parse("[c2 e2").is_err());
    assert!(parse("c2 e2]").is_err());
}

#[test]
fn nested_groups_parse() {
    let steps = parse("[c2 [e2 g2]] c3").unwrap();
    assert!(matches!(steps[0], Step::Group(_)));
    let ev = render(&steps, 64, 0);
    assert_eq!(ev[0], Event::Strike(0));
    assert_eq!(ev[16], Event::Strike(4));
    assert_eq!(ev[24], Event::Strike(7));
}
