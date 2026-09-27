#[path = "../../src/input.rs"]
mod input;

use input::*;

/// Feeds `(pressed, until_ms)` spans in 20 ms samples (like the firmware); returns hold times.
fn run(button: &mut HoldButton, spans: &[(bool, u64)]) -> Vec<u64> {
    let mut holds = Vec::new();
    let mut now = 0;
    for &(pressed, until) in spans {
        while now < until {
            if button.update(pressed, now) {
                holds.push(now);
            }
            now += 20;
        }
    }
    holds
}

#[test]
fn hold_fires_while_held_and_only_once() {
    let mut b = HoldButton::new();
    let holds = run(&mut b, &[(false, 100), (true, 6000), (false, 7000)]);
    assert_eq!(holds.len(), 1);
    assert!(holds[0] >= 100 + HOLD_MS && holds[0] < 100 + HOLD_MS + 40);
}

#[test]
fn short_presses_do_nothing() {
    let mut b = HoldButton::new();
    let holds = run(
        &mut b,
        &[(false, 100), (true, 2900), (false, 3000), (true, 5000), (false, 6000)],
    );
    assert!(holds.is_empty());
}

#[test]
fn press_held_since_boot_is_ignored() {
    let mut b = HoldButton::new();
    assert!(run(&mut b, &[(true, 5000), (false, 6000)]).is_empty());
    // ...but the next hold works.
    assert!(!b.update(true, 6000));
    assert!(b.update(true, 6000 + HOLD_MS));
}
