#[path = "../../src/bins.rs"]
mod bins;

use bins::*;

/// Sydney in spring: AEST until 4 Oct 2026, AEDT (+11) after.
const AEST: i32 = 10 * 3600;
const AEDT: i32 = 11 * 3600;

/// Unix time of a local date and time.
fn at(year: i64, month: i64, day: i64, hour: i64, minute: i64, offset: i32) -> i64 {
    days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 - i64::from(offset)
}

const NONE: [bool; 3] = [false, false, false];
const RED_GREEN: [bool; 3] = [true, true, false];
const ALL: [bool; 3] = [true, true, true];

#[test]
fn calendar_basics() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    assert_eq!(days_from_civil(2026, 9, 29), 20_725);
    assert_eq!(weekday(days_from_civil(2026, 9, 29)), 2); // Tuesday
    assert_eq!(weekday(days_from_civil(2026, 9, 27)), 0); // Sunday
}

#[test]
fn first_bin_night_has_all_three_from_6pm_to_midnight() {
    assert_eq!(due(at(2026, 9, 29, 17, 59, AEST), AEST), NONE);
    assert_eq!(due(at(2026, 9, 29, 18, 0, AEST), AEST), ALL);
    assert_eq!(due(at(2026, 9, 29, 23, 59, AEST), AEST), ALL);
    assert_eq!(due(at(2026, 9, 30, 0, 0, AEST), AEST), NONE);
}

#[test]
fn yellow_every_second_week() {
    // 6 Oct is after daylight saving starts.
    assert_eq!(due(at(2026, 10, 6, 19, 0, AEDT), AEDT), RED_GREEN);
    assert_eq!(due(at(2026, 10, 13, 19, 0, AEDT), AEDT), ALL);
    assert_eq!(due(at(2026, 10, 20, 19, 0, AEDT), AEDT), RED_GREEN);
    assert_eq!(due(at(2026, 10, 27, 19, 0, AEDT), AEDT), ALL);
    // Next year, across the new year.
    assert_eq!(due(at(2027, 1, 5, 20, 0, AEDT), AEDT), ALL);
    assert_eq!(due(at(2027, 1, 12, 20, 0, AEDT), AEDT), RED_GREEN);
}

#[test]
fn nothing_on_other_evenings_or_before_the_start() {
    for day in 23..=31 {
        let (month, day) = if day > 30 { (10, day - 30) } else { (9, day) };
        if (month, day) == (9, 29) {
            continue;
        }
        assert_eq!(due(at(2026, month, day, 20, 0, AEST), AEST), NONE, "{month}/{day}");
    }
}

#[test]
fn uses_local_time_not_utc() {
    // 18:30 in Sydney is 08:30 UTC.
    let t = at(2026, 9, 29, 18, 30, AEST);
    assert_eq!(due(t, AEST), ALL);
    assert_eq!(due(t, 0), NONE);
}
