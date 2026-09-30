#[path = "../../src/join.rs"]
mod join;

use join::*;

const ALL: [JoinProblem; 5] = [
    JoinProblem::WrongPassword,
    JoinProblem::NotFound,
    JoinProblem::Security,
    JoinProblem::WeakSignal,
    JoinProblem::Other,
];

/// Same as TEXT_CHARS in src/setup/mod.rs (the pixel font's glyphs).
const TEXT_CHARS: &str = " ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-.:!?/";

#[test]
fn survives_a_restart_as_a_number() {
    for problem in ALL {
        assert_eq!(JoinProblem::from_code(problem as u32), Some(problem));
    }
    assert_eq!(JoinProblem::from_code(0), None, "nothing stored");
    assert_eq!(JoinProblem::from_code(0xDEAD_BEEF), None, "garbage after power-on");
}

#[test]
fn screen_text_fits_the_round_screen_and_the_font() {
    for problem in ALL.iter().map(|p| p.screen_text()).chain(["JOINING WI-FI"]) {
        assert!(problem.len() <= 15, "{problem:?} is too wide");
        assert!(problem.chars().all(|c| TEXT_CHARS.contains(c)), "{problem:?} has a missing glyph");
    }
}
