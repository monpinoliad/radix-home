extern crate alloc;

#[path = "../../src/bins.rs"]
mod bins;
#[path = "../../src/calendar.rs"]
mod calendar;
#[path = "../../src/json.rs"]
mod json;

use bins::days_from_civil;
use calendar::*;

const AEST: i32 = 10 * 3600;

/// Real replies from Home Assistant's `/api/calendars/<entity>`.
const BINS_BODY: &str = r#"[{"start":{"date":"2026-09-29"},"end":{"date":"2026-09-30"},"summary":"🔴🟢 Put out Red + Green Bins","description":"","location":null,"uid":"97be81b0-b989-11f1-b88d-e454e872c6e2","recurrence_id":"20260929","rrule":"FREQ=WEEKLY;BYDAY=TU"},{"start":{"date":"2026-09-29"},"end":{"date":"2026-09-30"},"summary":"🟡 Put out Yellow Bin","description":"","location":null,"uid":"a60e33fa-b989-11f1-b88d-e454e872c6e2","recurrence_id":"20260929","rrule":"FREQ=WEEKLY;INTERVAL=2;BYDAY=TU"},{"start":{"date":"2026-10-06"},"end":{"date":"2026-10-07"},"summary":"🔴🟢 Put out Red + Green Bins","description":"","location":null,"uid":"97be81b0-b989-11f1-b88d-e454e872c6e2","recurrence_id":"20261006","rrule":"FREQ=WEEKLY;BYDAY=TU"}]"#;
const DINNER_BODY: &str = r#"[{"start":{"dateTime":"2026-09-28T19:00:00+10:00"},"end":{"dateTime":"2026-09-28T20:00:00+10:00"},"summary":"Dinner","description":"Dinner time!","location":"31 Beaufort Rd","uid":"78de0748-bb1e-11f1-b88d-e454e872c6e2","recurrence_id":null,"rrule":null}]"#;
const HOLIDAY_BODY: &str = r#"[{"start":{"date":"2026-10-05"},"end":{"date":"2026-10-06"},"summary":"Labour Day","description":"Public holiday in NSW","location":"NSW, Australia","uid":"20261005-Labour-Day-NSW@simplecalendar.com.au","recurrence_id":null,"rrule":null}]"#;

/// Unix time of a local date and time (AEST).
fn at(year: i64, month: i64, day: i64, hour: i64, minute: i64) -> i64 {
    days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 - i64::from(AEST)
}

fn day(year: i64, month: i64, day: i64) -> i64 {
    days_from_civil(year, month, day)
}

/// Just the text of each line.
fn texts(lines: &[Line]) -> Vec<String> {
    lines.iter().map(|line| line.text.clone()).collect()
}

fn calendar(events: &[&str], bins: Option<&str>, from: i64) -> Calendar {
    Calendar {
        day: from,
        events: events.iter().flat_map(|body| parse(body).unwrap()).collect(),
        bin_events: bins.map(|body| parse(body).unwrap()),
    }
}

#[test]
fn parses_real_replies() {
    let dinner = parse(DINNER_BODY).unwrap();
    assert_eq!(
        dinner,
        [Event {
            when: When::Timed {
                start: at(2026, 9, 28, 19, 0),
                end: at(2026, 9, 28, 20, 0)
            },
            summary: "Dinner".into()
        }]
    );
    let bins = parse(BINS_BODY).unwrap();
    assert_eq!(bins.len(), 3);
    assert_eq!(bins[1].summary, "🟡 Put out Yellow Bin");
    assert_eq!(
        bins[2].when,
        When::AllDay {
            start: day(2026, 10, 6),
            end: day(2026, 10, 7)
        }
    );
    assert_eq!(parse("[]").unwrap(), []);
    assert_eq!(parse("  [ ]\n").unwrap(), []);
}

#[test]
fn rejects_non_lists() {
    assert_eq!(parse(r#"{"message":"Entity not found"}"#), None);
    assert_eq!(parse("401: Unauthorized"), None);
    assert_eq!(parse(""), None);
    // Cut off mid-event.
    assert_eq!(parse(&DINNER_BODY[..60]), None);
}

#[test]
fn parses_escapes_utc_and_null_summary() {
    let body = r#"[{"start":{"dateTime":"2026-09-28T09:00:00.500Z"},"end":{"dateTime":"2026-09-28T10:00:00-02:30"},"summary":"Say \"hi\" {u}00e9\\ {u}d83d{u}de00","description":"\"start\":{\"date\":\"2020-01-01\"}"},{"start":{"date":"2026-09-28"},"end":{"date":"2026-09-29"},"summary":null}]"#
        .replace("{u}", "\\u"); // JSON \u escapes
    let events = parse(&body).unwrap();
    assert_eq!(events.len(), 2);
    let utc = days_from_civil(2026, 9, 28) * 86_400;
    assert_eq!(
        events[0].when,
        When::Timed {
            start: utc + 9 * 3600,
            end: utc + 12 * 3600 + 30 * 60
        }
    );
    assert_eq!(events[0].summary, "Say \"hi\" é\\ ");
    assert_eq!(events[1].summary, "");
}

#[test]
fn display_text_fits_the_font() {
    assert_eq!(display_text("🔴🟢 Put out Red + Green Bins"), "PUT OUT RED GREEN BINS");
    assert_eq!(display_text("José Rizal's birthday"), "JOSE RIZALS BIRTHDAY");
    assert_eq!(
        display_text("Birthday of Muhammad (Mawlid)"),
        "BIRTHDAY OF MUHAMMAD MAWLID"
    );
    assert_eq!(display_text("  Dinner\n\ttime!  "), "DINNER TIME!");
    assert_eq!(display_text("Ñandú 3:30-4 w/ Zoë"), "NANDU 3:30-4 W/ ZOE");
    assert_eq!(display_text("🎉"), "");
    let long = display_text(&"word ".repeat(20));
    assert!(long.len() <= 48 && !long.ends_with(' '), "{long:?}");
}

#[test]
fn bins_named_by_word_or_circle() {
    assert_eq!(bins_named("🔴🟢 Put out Red + Green Bins"), [true, true, false]);
    assert_eq!(bins_named("🟡 Put out Yellow Bin"), [false, false, true]);
    assert_eq!(bins_named("Yellow bin"), [false, false, true]);
    assert_eq!(bins_named("🔴"), [true, false, false]);
    assert_eq!(bins_named("Reduce, reuse"), [false, false, false]);
}

#[test]
fn bin_night_from_the_calendar() {
    let cal = calendar(&[], Some(BINS_BODY), day(2026, 9, 29));
    // Tuesday: nothing before 6 PM, then all three.
    assert_eq!(cal.bins_due(at(2026, 9, 29, 17, 59), AEST), Some([false; 3]));
    assert_eq!(cal.bins_due(at(2026, 9, 29, 18, 0), AEST), Some([true; 3]));
    assert_eq!(cal.bins_due(at(2026, 9, 29, 23, 59), AEST), Some([true; 3]));
    // Wednesday (still covered by the fetch): none.
    assert_eq!(cal.bins_due(at(2026, 9, 30, 20, 0), AEST), Some([false; 3]));
    // Still covered by the fetch (a week and a day): none.
    assert_eq!(cal.bins_due(at(2026, 10, 1, 20, 0), AEST), Some([false; 3]));
    // Not covered: fall back to the built-in schedule.
    assert_eq!(cal.bins_due(at(2026, 10, 7, 20, 0), AEST), None);
    assert_eq!(cal.bins_due(at(2026, 9, 28, 20, 0), AEST), None);
    // No bin calendar set.
    let no_bins = calendar(&[DINNER_BODY], None, day(2026, 9, 29));
    assert_eq!(no_bins.bins_due(at(2026, 9, 29, 20, 0), AEST), None);
    // Set, but nothing on: no bins (not the built-in ones).
    let empty = calendar(&[], Some("[]"), day(2026, 9, 29));
    assert_eq!(empty.bins_due(at(2026, 9, 29, 20, 0), AEST), Some([false; 3]));
}

#[test]
fn todays_lines() {
    let cal = calendar(&[DINNER_BODY, HOLIDAY_BODY], None, day(2026, 9, 28));
    assert_eq!(texts(&cal.lines(at(2026, 9, 28, 8, 0), AEST)), ["7:00PM DINNER"]);
    assert_eq!(texts(&cal.lines(at(2026, 9, 28, 19, 59), AEST)), ["7:00PM DINNER"]);
    assert!(texts(&cal.lines(at(2026, 9, 28, 20, 0), AEST)).is_empty());
    assert!(texts(&cal.lines(at(2026, 9, 29, 9, 0), AEST)).is_empty());

    let cal = calendar(&[DINNER_BODY, HOLIDAY_BODY], None, day(2026, 10, 5));
    assert_eq!(texts(&cal.lines(at(2026, 10, 5, 0, 0), AEST)), ["LABOUR DAY"]);
    assert!(texts(&cal.lines(at(2026, 10, 6, 0, 0), AEST)).is_empty());
    // Outside what was fetched: nothing rather than stale events.
    assert!(texts(&cal.lines(at(2026, 10, 7, 12, 0), AEST)).is_empty());
}

#[test]
fn lines_sorted_all_day_first() {
    let body = r#"[{"start":{"dateTime":"2026-10-05T15:30:00+10:00"},"end":{"dateTime":"2026-10-05T16:00:00+10:00"},"summary":"Pickup"},{"start":{"dateTime":"2026-10-05T08:05:00+10:00"},"end":{"dateTime":"2026-10-05T09:00:00+10:00"},"summary":"Gym"},{"start":{"dateTime":"2026-10-06T08:00:00+10:00"},"end":{"dateTime":"2026-10-06T09:00:00+10:00"},"summary":"Tomorrow"}]"#;
    let cal = calendar(&[body, HOLIDAY_BODY], None, day(2026, 10, 5));
    assert_eq!(
        texts(&cal.lines(at(2026, 10, 5, 7, 0), AEST)),
        ["LABOUR DAY", "8:05AM GYM", "3:30PM PICKUP"]
    );
}

#[test]
fn iso_utc_round_trips() {
    assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(iso_utc(at(2026, 9, 28, 0, 0)), "2026-09-27T14:00:00Z");
    assert_eq!(iso_utc(at(2028, 3, 1, 10, 0)), "2028-03-01T00:00:00Z");
    assert_eq!(iso_utc(days_from_civil(2000, 2, 29) * 86_400 + 86_399), "2000-02-29T23:59:59Z");
    for d in (-800..80_000).step_by(7) {
        let s = iso_utc(d * 86_400);
        let (y, m, dd) = (s[..4].parse().unwrap(), s[5..7].parse().unwrap(), s[8..10].parse().unwrap());
        assert_eq!(days_from_civil(y, m, dd), d, "{s}");
    }
}

#[test]
fn event_times_are_12_hour() {
    let at_time = |hour: i64, minute: i64| {
        format!(
            r#"[{{"start":{{"dateTime":"2026-10-05T{hour:02}:{minute:02}:00+10:00"}},"end":{{"dateTime":"2026-10-05T23:59:00+10:00"}},"summary":"X"}}]"#
        )
    };
    for (hour, minute, want) in [
        (0, 5, "12:05AM X"),
        (9, 0, "9:00AM X"),
        (11, 59, "11:59AM X"),
        (12, 0, "12:00PM X"),
        (13, 30, "1:30PM X"),
        (23, 0, "11:00PM X"),
    ] {
        let cal = calendar(&[&at_time(hour, minute)], None, day(2026, 10, 5));
        assert_eq!(texts(&cal.lines(at(2026, 10, 5, 0, 0), AEST)), [want]);
    }
}

#[test]
fn important_marker() {
    assert_eq!(important("! Dentist"), (true, "Dentist"));
    assert_eq!(important("  !Dentist"), (true, "Dentist"));
    assert_eq!(important("❗Pay rent"), (true, "Pay rent"));
    assert_eq!(important("❗\u{fe0f} Pay rent"), (true, "Pay rent"));
    assert_eq!(important("!!! ‼ Taxes"), (true, "Taxes"));
    assert_eq!(important("Dinner!"), (false, "Dinner!"));
    assert_eq!(important("Dinner"), (false, "Dinner"));
    assert_eq!(important("🔴🟢 Put out bins"), (false, "🔴🟢 Put out bins"));
}

#[test]
fn important_lines_first_without_marker() {
    let body = r#"[{"start":{"dateTime":"2026-10-05T08:00:00+10:00"},"end":{"dateTime":"2026-10-05T09:00:00+10:00"},"summary":"Gym"},{"start":{"dateTime":"2026-10-05T15:30:00+10:00"},"end":{"dateTime":"2026-10-05T16:00:00+10:00"},"summary":"! Dentist"},{"start":{"date":"2026-10-05"},"end":{"date":"2026-10-06"},"summary":"❗Pay rent"},{"start":{"dateTime":"2026-10-05T12:00:00+10:00"},"end":{"dateTime":"2026-10-05T13:00:00+10:00"},"summary":"Lunch!"}]"#;
    let cal = calendar(&[body, HOLIDAY_BODY], None, day(2026, 10, 5));
    let lines = cal.lines(at(2026, 10, 5, 7, 0), AEST);
    assert_eq!(
        texts(&lines),
        ["PAY RENT", "3:30PM DENTIST", "LABOUR DAY", "8:00AM GYM", "12:00PM LUNCH!"]
    );
    let flags: Vec<bool> = lines.iter().map(|line| line.important).collect();
    assert_eq!(flags, [true, true, false, false, false]);
    // A marker alone isn't a title.
    let bare = r#"[{"start":{"date":"2026-10-05"},"end":{"date":"2026-10-06"},"summary":"!"}]"#;
    let cal = calendar(&[bare], None, day(2026, 10, 5));
    assert!(cal.lines(at(2026, 10, 5, 7, 0), AEST).is_empty());
}

#[test]
fn dates_and_weekdays() {
    assert_eq!(date_text(day(2026, 9, 28)), "MON 28 SEP");
    assert_eq!(date_text(day(2026, 10, 5)), "MON 5 OCT");
    assert_eq!(date_text(day(2027, 1, 1)), "FRI 1 JAN");
    assert_eq!(date_text(day(2028, 2, 29)), "TUE 29 FEB");
    assert_eq!(weekday_name(day(2026, 9, 29)), "TUE");
    assert_eq!(weekday_name(day(2026, 10, 4)), "SUN");
}

#[test]
fn agenda_of_coming_days() {
    let body = r#"[{"start":{"dateTime":"2026-09-28T19:00:00+10:00"},"end":{"dateTime":"2026-09-28T20:00:00+10:00"},"summary":"Dinner"},{"start":{"dateTime":"2026-10-01T15:30:00+10:00"},"end":{"dateTime":"2026-10-01T16:00:00+10:00"},"summary":"! Dentist"},{"start":{"dateTime":"2026-09-30T08:05:00+10:00"},"end":{"dateTime":"2026-09-30T09:00:00+10:00"},"summary":"Gym"},{"start":{"date":"2026-10-06"},"end":{"date":"2026-10-07"},"summary":"Too far"},{"start":{"dateTime":"2026-10-05T09:00:00+10:00"},"end":{"dateTime":"2026-10-05T10:00:00+10:00"},"summary":"Work"}]"#;
    let cal = calendar(&[body, HOLIDAY_BODY], Some(BINS_BODY), day(2026, 9, 28));
    let lines = cal.agenda(at(2026, 9, 28, 12, 0), AEST);
    // Today's dinner is on the clock slide, not here; 6 Oct is over a week away.
    assert_eq!(
        texts(&lines),
        ["TUE BIN NIGHT", "WED 8:05AM GYM", "THU 3:30PM DENTIST", "MON LABOUR DAY", "MON 9:00AM WORK"]
    );
    let flags: Vec<bool> = lines.iter().map(|line| line.important).collect();
    assert_eq!(flags, [false, false, true, false, false]);
}

#[test]
fn agenda_is_capped_and_needs_fetched_days() {
    let many: Vec<String> = (0..8)
        .map(|h| {
            format!(r#"{{"start":{{"dateTime":"2026-09-29T{:02}:00:00+10:00"}},"end":{{"dateTime":"2026-09-29T{:02}:30:00+10:00"}},"summary":"E{h}"}}"#, 8 + h, 8 + h)
        })
        .collect();
    let body = format!("[{}]", many.join(","));
    let cal = calendar(&[&body], None, day(2026, 9, 28));
    let lines = texts(&cal.agenda(at(2026, 9, 28, 12, 0), AEST));
    assert_eq!(lines.len(), AGENDA_LINES);
    assert_eq!(lines[0], "TUE 8:00AM E0");
    // Fetched long ago: only what's still covered.
    let stale = calendar(&[HOLIDAY_BODY], Some(BINS_BODY), day(2026, 9, 20));
    assert!(texts(&stale.agenda(at(2026, 9, 28, 12, 0), AEST)).is_empty());
}
