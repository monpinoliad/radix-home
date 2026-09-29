//! Open-Meteo response parsing, plus the "what should the sky look like" logic.

use crate::json::{after, nth_number, number};

const DAY: i64 = 24 * 60 * 60;
/// Dawn/dusk colours are shown for this long either side of sunrise/sunset.
const TWILIGHT: i64 = 30 * 60;

/// Local time until the first forecast says otherwise (AEST, no daylight saving).
pub const FALLBACK_UTC_OFFSET_S: i32 = 10 * 60 * 60;

/// Hours of hourly forecast fetched, from the current hour (`forecast_hours` in the request).
pub const HOURS_FETCHED: usize = 13;

/// One hour of the hourly forecast.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Hour {
    /// Unix seconds, on the hour.
    pub time: i64,
    pub temperature_c: f32,
    /// WMO weather code.
    pub code: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct Weather {
    pub temperature_c: f32,
    /// WMO weather code.
    pub code: u16,
    /// Local time = UTC + this. Follows daylight saving.
    pub utc_offset_s: i32,
    /// Unix seconds, for the local day the forecast was fetched.
    pub sunrise: i64,
    pub sunset: i64,
    /// That day's highest and lowest temperature.
    pub high_c: f32,
    pub low_c: f32,
    /// That day's highest chance of rain (%), if known.
    pub rain_chance: Option<u8>,
    /// From the hour the forecast was fetched in; missing ones (short reply) have `time` 0.
    pub hours: [Hour; HOURS_FETCHED],
}

/// Parses the body of `/v1/forecast?current=temperature_2m,weather_code
/// &daily=sunrise,sunset,temperature_2m_max,temperature_2m_min,precipitation_probability_max
/// &hourly=temperature_2m,weather_code&forecast_hours=13&timeformat=unixtime&forecast_days=1`.
///
/// The response is small and flat, so this just scans for keys rather than pulling in a JSON parser.
pub fn parse(body: &str) -> Option<Weather> {
    // `current` etc. also appear as `current_units` etc.; the `":{` tells them apart.
    let current = after(body, "\"current\":{")?;
    let hourly = after(body, "\"hourly\":{")?;
    let daily = after(body, "\"daily\":{")?;
    let mut hours = [Hour::default(); HOURS_FETCHED];
    for (i, hour) in hours.iter_mut().enumerate() {
        let item = |key| nth_number(hourly, key, i);
        let (Some(time), Some(temperature), Some(code)) = (
            item("\"time\":["),
            item("\"temperature_2m\":["),
            item("\"weather_code\":["),
        ) else {
            break;
        };
        *hour = Hour {
            time: time.parse().ok()?,
            temperature_c: temperature.parse().ok()?,
            code: code.parse().ok()?,
        };
    }
    Some(Weather {
        temperature_c: number(current, "\"temperature_2m\":")?.parse().ok()?,
        code: number(current, "\"weather_code\":")?.parse().ok()?,
        utc_offset_s: number(body, "\"utc_offset_seconds\":")?.parse().ok()?,
        sunrise: nth_number(daily, "\"sunrise\":[", 0)?.parse().ok()?,
        sunset: nth_number(daily, "\"sunset\":[", 0)?.parse().ok()?,
        high_c: nth_number(daily, "\"temperature_2m_max\":[", 0)?
            .parse()
            .ok()?,
        low_c: nth_number(daily, "\"temperature_2m_min\":[", 0)?
            .parse()
            .ok()?,
        rain_chance: nth_number(daily, "\"precipitation_probability_max\":[", 0)
            .and_then(|chance| chance.parse().ok()),
        hours,
    })
}

/// Rounded to the nearest degree.
pub fn round_c(t: f32) -> i32 {
    if t >= 0.0 {
        (t + 0.5) as i32
    } else {
        (t - 0.5) as i32
    }
}

/// Index into the icon list in `ui/main.slint` / `tools/gen_sprites.py`.
pub fn icon_for(code: u16, night: bool) -> i32 {
    match code {
        0 | 1 => i32::from(night), // clear / mainly clear
        2 => 2 + i32::from(night), // partly cloudy
        3 => 4,                    // overcast
        45 | 48 => 5,              // fog
        51..=67 | 80..=82 => 6,    // drizzle, rain, showers
        71..=77 | 85 | 86 => 7,    // snow
        95..=99 => 8,              // thunderstorm
        _ => 4,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sky {
    Night = 0,
    Dawn = 1,
    Day = 2,
    Dusk = 3,
}

impl Weather {
    pub fn temperature_rounded(&self) -> i32 {
        round_c(self.temperature_c)
    }

    /// The hourly forecast for hours starting after `now`.
    pub fn hours_after(&self, now: i64) -> impl Iterator<Item = Hour> + '_ {
        self.hours
            .iter()
            .copied()
            .filter(move |hour| hour.time > now)
    }

    /// Icon for an hour of the forecast: night icons between sunset and sunrise.
    pub fn hour_icon(&self, hour: Hour) -> i32 {
        icon_for(hour.code, self.is_night(hour.time))
    }

    /// Sunrise/sunset moved onto `now`'s local day, so the sky stays right even if
    /// the forecast is from yesterday (e.g. just after midnight, or Wi-Fi has been down).
    fn sun_times(&self, now: i64) -> (i64, i64) {
        let local_day = |t: i64| (t + i64::from(self.utc_offset_s)).div_euclid(DAY);
        let shift = (local_day(now) - local_day(self.sunrise)) * DAY;
        (self.sunrise + shift, self.sunset + shift)
    }

    pub fn is_night(&self, now: i64) -> bool {
        let (sunrise, sunset) = self.sun_times(now);
        now < sunrise || now >= sunset
    }

    pub fn sky(&self, now: i64) -> Sky {
        let (sunrise, sunset) = self.sun_times(now);
        if (now - sunrise).abs() < TWILIGHT {
            Sky::Dawn
        } else if (now - sunset).abs() < TWILIGHT {
            Sky::Dusk
        } else if self.is_night(now) {
            Sky::Night
        } else {
            Sky::Day
        }
    }

    /// Icon for the current weather.
    pub fn icon(&self, now: i64) -> i32 {
        icon_for(self.code, self.is_night(now))
    }
}

/// Hours, minutes, seconds of the local time.
pub fn local_hms(unix: i64, utc_offset_s: i32) -> (i32, i32, i32) {
    let secs = (unix + i64::from(utc_offset_s)).rem_euclid(DAY) as i32;
    (secs / 3600, secs / 60 % 60, secs % 60)
}
