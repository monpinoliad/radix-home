//! Bin night: which bins to put out, from the local date and time.

const DAY: i64 = 24 * 60 * 60;

/// Days since 1970-01-01 of a calendar date (proleptic Gregorian).
pub const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let month_index = (month + 9) % 12; // March = 0
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// 0 = Sunday ... 6 = Saturday.
pub const fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7) // 1970-01-01 was a Thursday
}

pub struct Bin {
    /// Collected every this many weeks, on the weekday of `first_day`.
    every_weeks: i64,
    /// A reminder day (days since 1970-01-01); none are shown before it.
    first_day: i64,
}

/// Reminders show on the bin night from this hour (local time) until midnight.
pub const REMINDER_FROM_HOUR: i64 = 18;

/// Same order as `bin-images` in `ui/main.slint` and BIN_COLOURS in tools/gen_sprites.py.
pub const BINS: [Bin; 3] = [
    // Red: every Tuesday.
    Bin {
        every_weeks: 1,
        first_day: days_from_civil(2026, 9, 29),
    },
    // Green: every Tuesday.
    Bin {
        every_weeks: 1,
        first_day: days_from_civil(2026, 9, 29),
    },
    // Yellow: every second Tuesday, starting 29 September 2026.
    Bin {
        every_weeks: 2,
        first_day: days_from_civil(2026, 9, 29),
    },
];

/// Which of [`BINS`] to show right now: `true` on its reminder evening.
pub fn due(unix: i64, utc_offset_s: i32) -> [bool; 3] {
    let local = unix + i64::from(utc_offset_s);
    let today = local.div_euclid(DAY);
    let hour = local.rem_euclid(DAY) / 3600;
    BINS.map(|bin| {
        hour >= REMINDER_FROM_HOUR
            && today >= bin.first_day
            && (today - bin.first_day).rem_euclid(7 * bin.every_weeks) == 0
    })
}
