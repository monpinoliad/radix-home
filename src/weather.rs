//! Open-Meteo response parsing, plus the "what should the sky look like" logic.

const DAY: i64 = 24 * 60 * 60;
/// Dawn/dusk colours are shown for this long either side of sunrise/sunset.
const TWILIGHT: i64 = 30 * 60;

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
}

/// Parses the body of
/// `/v1/forecast?current=temperature_2m,weather_code&daily=sunrise,sunset&timeformat=unixtime`.
///
/// The response is small and flat, so this just scans for keys rather than pulling in a JSON parser.
pub fn parse(body: &str) -> Option<Weather> {
    // `current` / `daily` also appear as `current_units` / `daily_units`; the `":{` tells them apart.
    let current = after(body, "\"current\":{")?;
    let daily = after(body, "\"daily\":{")?;
    Some(Weather {
        temperature_c: number(current, "\"temperature_2m\":")?.parse().ok()?,
        code: number(current, "\"weather_code\":")?.parse().ok()?,
        utc_offset_s: number(body, "\"utc_offset_seconds\":")?.parse().ok()?,
        sunrise: number(daily, "\"sunrise\":[")?.parse().ok()?,
        sunset: number(daily, "\"sunset\":[")?.parse().ok()?,
    })
}

fn after<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    s.find(key).map(|i| &s[i + key.len()..])
}

fn number<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let rest = after(s, key)?;
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.'))
        .unwrap_or(rest.len());
    Some(&rest[..end])
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
        let t = self.temperature_c;
        if t >= 0.0 {
            (t + 0.5) as i32
        } else {
            (t - 0.5) as i32
        }
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

    /// Index into the icon list in `ui/main.slint` / `tools/gen_sprites.py`.
    pub fn icon(&self, now: i64) -> i32 {
        let night = self.is_night(now);
        match self.code {
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
}

/// Hours, minutes, seconds of the local time.
pub fn local_hms(unix: i64, utc_offset_s: i32) -> (i32, i32, i32) {
    let secs = (unix + i64::from(utc_offset_s)).rem_euclid(DAY) as i32;
    (secs / 3600, secs / 60 % 60, secs % 60)
}
