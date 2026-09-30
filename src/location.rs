//! Where the board is, from its public IP address (ip-api.com over plain HTTP, no API key).
//! Only used to pick the weather location; accurate to about the city.

use core::fmt;

/// A coordinate in hundredths of a degree (about 1 km: plenty for weather).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coordinate(pub i32);

impl fmt::Display for Coordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        write!(f, "{sign}{}.{:02}", abs / 100, abs % 100)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub latitude: Coordinate,
    pub longitude: Coordinate,
}

/// Used until the lookup works (Blacktown, NSW).
pub const FALLBACK: Location = Location {
    latitude: Coordinate(-3377),
    longitude: Coordinate(15091),
};

pub const HOST: &str = "ip-api.com";
pub const PATH: &str = "/json/?fields=status,city,lat,lon";

/// Parses the body of `http://ip-api.com/json/?fields=status,city,lat,lon`, e.g.
/// `{"status":"success","city":"Blacktown","lat":-33.7717,"lon":150.9083}`.
pub fn parse(body: &str) -> Option<Location> {
    if !body.contains("\"status\":\"success\"") {
        return None;
    }
    let latitude = coordinate(body, "\"lat\":")?;
    let longitude = coordinate(body, "\"lon\":")?;
    let in_range = latitude.0.abs() <= 9000 && longitude.0.abs() <= 18000;
    in_range.then_some(Location {
        latitude,
        longitude,
    })
}

/// The city name, for the log.
pub fn city(body: &str) -> Option<&str> {
    let rest = &body[body.find("\"city\":\"")? + 8..];
    Some(&rest[..rest.find('"')?])
}

/// Reads a decimal number after `key` as hundredths, dropping further digits.
fn coordinate(body: &str, key: &str) -> Option<Coordinate> {
    let rest = &body[body.find(key)? + key.len()..];
    let (negative, rest) = match rest.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, rest),
    };
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(rest.len());
    let (whole, fraction) = rest[..end].split_once('.').unwrap_or((&rest[..end], ""));
    let whole: i32 = whole.parse().ok()?;
    let mut hundredths = 0;
    for (i, digit) in fraction.bytes().chain(*b"00").take(2).enumerate() {
        if !digit.is_ascii_digit() {
            return None;
        }
        hundredths += i32::from(digit - b'0') * if i == 0 { 10 } else { 1 };
    }
    let value = whole.checked_mul(100)? + hundredths;
    Some(Coordinate(if negative { -value } else { value }))
}
