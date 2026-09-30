//! Events and bin nights from Home Assistant calendars (its REST API, over plain HTTP on the
//! local network): parsing, and what to show when. `net.rs` does the fetching.

use alloc::{format, string::String, vec::Vec};

use crate::{
    bins::{self, days_from_civil},
    json::{self, after},
};

const DAY: i64 = 24 * 60 * 60;

/// Each fetch asks for this many days from local midnight: today, the agenda's days, and
/// that still covers today for a while past midnight.
pub const DAYS_FETCHED: i64 = AGENDA_DAYS + 1;

/// The agenda runs from tomorrow to this many days ahead. Today is never in it, so a week
/// ahead still has weekday names that are unique ("MON" is next Monday).
const AGENDA_DAYS: i64 = 7;
/// At most this many agenda lines.
pub const AGENDA_LINES: usize = 5;

/// Titles longer than this (in pixel-font characters) are cut off.
const MAX_TEXT: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    /// Local days since 1970-01-01; `end` is the day after the last one.
    AllDay { start: i64, end: i64 },
    /// Unix seconds.
    Timed { start: i64, end: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub when: When,
    pub summary: String,
}

impl Event {
    fn is_on(&self, day: i64, utc_offset_s: i32) -> bool {
        match self.when {
            When::AllDay { start, end } => start <= day && day < end,
            When::Timed { start, .. } => local_day(start, utc_offset_s) == day,
        }
    }
}

/// One event as shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// In the pixel font's characters, e.g. "7:00PM DINNER".
    pub text: String,
    /// Shown with a red "!" badge.
    pub important: bool,
}

/// Title starts with one of these (after any spaces): the event is important. The marker isn't shown.
const IMPORTANT_MARKERS: [char; 4] = ['!', '❗', '❕', '‼'];

/// Whether `summary` is marked important ("! Dentist", "❗Pay rent"), and the title without the marker.
pub fn important(summary: &str) -> (bool, &str) {
    let title = summary.trim_start();
    let rest = title.trim_start_matches(|c: char| {
        IMPORTANT_MARKERS.contains(&c) || c == '\u{fe0f}' || c.is_whitespace()
    });
    if rest.len() == title.len() {
        (false, summary)
    } else {
        (true, rest)
    }
}

/// The result of one fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calendar {
    /// Local day the fetch started from (days since 1970-01-01).
    pub day: i64,
    /// From the event calendars.
    pub events: Vec<Event>,
    /// From the bin calendar, if one is set.
    pub bin_events: Option<Vec<Event>>,
}

impl Calendar {
    fn covers(&self, day: i64) -> bool {
        self.day <= day && day < self.day + DAYS_FETCHED
    }

    /// Lines to show now: today's all-day events, then the timed ones that haven't ended (as
    /// "7:00PM DINNER"), by start time; important ones (see [`important`]) before the rest.
    pub fn lines(&self, now: i64, utc_offset_s: i32) -> Vec<Line> {
        let today = local_day(now, utc_offset_s);
        if !self.covers(today) {
            return Vec::new();
        }
        let tonight = (today + 1) * DAY - i64::from(utc_offset_s);
        let mut lines: Vec<(i64, Line)> = self
            .events
            .iter()
            .filter_map(|event| {
                let (important, title) = important(&event.summary);
                let title = display_text(title);
                let (start, text) = match event.when {
                    When::AllDay { .. } => {
                        if !event.is_on(today, utc_offset_s) || title.is_empty() {
                            return None;
                        }
                        (i64::MIN, title)
                    }
                    When::Timed { start, end } => {
                        if end <= now || start >= tonight {
                            return None;
                        }
                        let line = format!("{} {title}", clock_12h(start, utc_offset_s));
                        (start, String::from(line.trim_end()))
                    }
                };
                Some((start, Line { text, important }))
            })
            .collect();
        lines.sort_by_key(|(start, line)| (!line.important, *start));
        lines.into_iter().map(|(_, line)| line).collect()
    }

    /// Coming days (tomorrow through [`AGENDA_DAYS`] ahead) in time order, at most [`AGENDA_LINES`]:
    /// "TUE 3:30PM DENTIST", "MON LABOUR DAY", and "TUE BIN NIGHT" for a day with bins to put out.
    pub fn agenda(&self, now: i64, utc_offset_s: i32) -> Vec<Line> {
        let today = local_day(now, utc_offset_s);
        let days = today + 1..=today + AGENDA_DAYS;
        let mut lines: Vec<((i64, i64), Line)> = Vec::new();
        for event in &self.events {
            let (important, title) = important(&event.summary);
            let title = display_text(title);
            let (day, start, time) = match event.when {
                When::AllDay { start, .. } => (start, i64::MIN, None),
                When::Timed { start, .. } => (
                    local_day(start, utc_offset_s),
                    start,
                    Some(clock_12h(start, utc_offset_s)),
                ),
            };
            if !days.contains(&day) || !self.covers(day) || (title.is_empty() && time.is_none()) {
                continue;
            }
            let mut text = String::from(weekday_name(day));
            for part in [time.as_deref(), Some(title.as_str())]
                .into_iter()
                .flatten()
            {
                if !part.is_empty() {
                    text.push(' ');
                    text.push_str(part);
                }
            }
            lines.push(((day, start), Line { text, important }));
        }
        if let Some(bin_events) = &self.bin_events {
            for day in days.clone().filter(|&day| self.covers(day)) {
                let bin_night = bin_events.iter().any(|event| {
                    event.is_on(day, utc_offset_s) && bins_named(&event.summary).contains(&true)
                });
                if bin_night {
                    let text = format!("{} BIN NIGHT", weekday_name(day));
                    lines.push((
                        (day, i64::MIN),
                        Line {
                            text,
                            important: false,
                        },
                    ));
                }
            }
        }
        lines.sort_by_key(|(when, _)| *when);
        lines.truncate(AGENDA_LINES);
        lines.into_iter().map(|(_, line)| line).collect()
    }

    /// Which bins (same order as [`bins::BINS`]) to show now, or `None` without a bin calendar
    /// (or data for today). Like the built-in schedule, from [`bins::REMINDER_FROM_HOUR`] on the day.
    pub fn bins_due(&self, now: i64, utc_offset_s: i32) -> Option<[bool; bins::BINS.len()]> {
        let events = self.bin_events.as_ref()?;
        let today = local_day(now, utc_offset_s);
        if !self.covers(today) {
            return None;
        }
        let mut due = [false; bins::BINS.len()];
        let hour = (now + i64::from(utc_offset_s)).rem_euclid(DAY) / 3600;
        if hour >= bins::REMINDER_FROM_HOUR {
            for event in events.iter().filter(|e| e.is_on(today, utc_offset_s)) {
                for (due, named) in due.iter_mut().zip(bins_named(&event.summary)) {
                    *due |= named;
                }
            }
        }
        Some(due)
    }
}

/// Colour word or coloured circle for each of [`bins::BINS`].
const BIN_NAMES: [(&str, char); bins::BINS.len()] =
    [("RED", '🔴'), ("GREEN", '🟢'), ("YELLOW", '🟡')];

/// The bins an event is about, e.g. "🔴🟢 Put out Red + Green Bins" names red and green.
pub fn bins_named(summary: &str) -> [bool; bins::BINS.len()] {
    let text = clean(summary);
    BIN_NAMES.map(|(word, circle)| text.split(' ').any(|w| w == word) || summary.contains(circle))
}

/// Local time of day, 12-hour: "7:00PM", "12:05AM".
pub fn clock_12h(unix: i64, utc_offset_s: i32) -> String {
    let minutes = (unix + i64::from(utc_offset_s)).rem_euclid(DAY) / 60;
    let (hour, minute) = (minutes / 60, minutes % 60);
    let am_pm = if hour < 12 { "AM" } else { "PM" };
    format!("{}:{minute:02}{am_pm}", (hour + 11) % 12 + 1)
}

const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// "MON" for a local day (days since 1970-01-01).
pub fn weekday_name(day: i64) -> &'static str {
    WEEKDAYS[bins::weekday(day) as usize]
}

/// "MON 28 SEP" for a local day (days since 1970-01-01).
pub fn date_text(day: i64) -> String {
    let (_, month, day_of_month) = civil_from_days(day);
    format!(
        "{} {day_of_month} {}",
        weekday_name(day),
        MONTHS[month as usize - 1]
    )
}

pub fn local_day(unix: i64, utc_offset_s: i32) -> i64 {
    (unix + i64::from(utc_offset_s)).div_euclid(DAY)
}

/// `2026-09-27T14:00:00Z`, for the API's `start`/`end`.
pub fn iso_utc(unix: i64) -> String {
    let (year, month, day) = civil_from_days(unix.div_euclid(DAY));
    let secs = unix.rem_euclid(DAY);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

/// Year, month, day of days since 1970-01-01 (the inverse of [`days_from_civil`]).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153; // March = 0
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// What the pixel font can draw of `text`: accents dropped, uppercase, anything else (emoji, `+`,
/// `'`...) left out, spaces collapsed, cut to [`MAX_TEXT`].
pub fn display_text(text: &str) -> String {
    let mut text = clean(text);
    if let Some((cut, _)) = text.char_indices().nth(MAX_TEXT) {
        text.truncate(cut);
        text.truncate(text.trim_end().len());
    }
    text
}

fn clean(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        // The pixel font's characters (TEXT_CHARS in src/setup/mod.rs).
        let c = without_accent(c).to_ascii_uppercase();
        if c.is_ascii_uppercase() || c.is_ascii_digit() || "-.:!?/%°".contains(c) {
            out.push(c);
        } else if c.is_whitespace() && !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out.truncate(out.trim_end().len());
    out
}

fn without_accent(c: char) -> char {
    match c {
        'À'..='Å' | 'à'..='å' => 'A',
        'Ç' | 'ç' => 'C',
        'È'..='Ë' | 'è'..='ë' => 'E',
        'Ì'..='Ï' | 'ì'..='ï' => 'I',
        'Ñ' | 'ñ' => 'N',
        'Ò'..='Ö' | 'Ø' | 'ò'..='ö' | 'ø' => 'O',
        'Ù'..='Ü' | 'ù'..='ü' => 'U',
        'Ý' | 'ý' | 'ÿ' => 'Y',
        _ => c,
    }
}

/// Parses the body of `/api/calendars/<entity>?start=...&end=...`, e.g.
/// `[{"start":{"dateTime":"2026-09-28T19:00:00+10:00"},"end":{...},"summary":"Dinner",...}]`
/// (all-day events have `"date":"2026-10-05"` instead, with `end` the day after).
///
/// Like the other parsers, this scans for keys rather than pulling in a JSON parser.
pub fn parse(body: &str) -> Option<Vec<Event>> {
    const START: &str = "\"start\":{";
    let mut rest = body.trim_start().strip_prefix('[')?;
    let mut events = Vec::new();
    while let Some(i) = rest.find(START) {
        let (start, after_start) = stamp(&rest[i + START.len()..])?;
        let (end, after_end) = stamp(after(after_start, "\"end\":{")?)?;
        let after_key = after(after_end, "\"summary\":")?;
        let (summary, after_summary) = match after_key.strip_prefix('"') {
            Some(string) => json::string(string)?,
            None => (String::new(), after_key), // null
        };
        let when = match (start, end) {
            (Stamp::Date(start), Stamp::Date(end)) => When::AllDay {
                start,
                end: end.max(start + 1),
            },
            (Stamp::DateTime(start), Stamp::DateTime(end)) => When::Timed {
                start,
                end: end.max(start),
            },
            _ => return None,
        };
        events.push(Event { when, summary });
        rest = after_summary;
    }
    Some(events)
}

enum Stamp {
    /// Days since 1970-01-01.
    Date(i64),
    /// Unix seconds.
    DateTime(i64),
}

/// `"date":"2026-10-05"` or `"dateTime":"2026-09-28T19:00:00+10:00"`, and what follows.
fn stamp(s: &str) -> Option<(Stamp, &str)> {
    if let Some(s) = s.strip_prefix("\"dateTime\":\"") {
        let (value, rest) = s.split_once('"')?;
        Some((Stamp::DateTime(date_time(value)?), rest))
    } else {
        let (value, rest) = s.strip_prefix("\"date\":\"")?.split_once('"')?;
        Some((Stamp::Date(date(value)?), rest))
    }
}

/// `2026-10-05` as days since 1970-01-01.
fn date(s: &str) -> Option<i64> {
    if s.len() != 10 || &s[4..5] != "-" || &s[7..8] != "-" {
        return None;
    }
    Some(days_from_civil(
        digits(&s[..4])?,
        digits(&s[5..7])?,
        digits(&s[8..10])?,
    ))
}

/// `2026-09-28T19:00:00+10:00` (or with fractional seconds, or `Z`) as Unix seconds.
pub fn date_time(s: &str) -> Option<i64> {
    let day = date(s.get(..10)?)?;
    if s.get(10..11)? != "T" || s.get(13..14)? != ":" || s.get(16..17)? != ":" {
        return None;
    }
    let hms = digits(s.get(11..13)?)? * 3600 + digits(&s[14..16])? * 60 + digits(s.get(17..19)?)?;
    let zone = s[19..].trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = match zone {
        "" | "Z" => 0,
        _ => {
            let sign = match zone.get(..1)? {
                "+" => 1,
                "-" => -1,
                _ => return None,
            };
            let (hours, minutes) = zone[1..].split_once(':')?;
            sign * (digits(hours)? * 3600 + digits(minutes)? * 60)
        }
    };
    Some(day * DAY + hms - offset)
}

fn digits(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}
