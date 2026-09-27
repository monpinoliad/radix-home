#[path = "../../src/weather.rs"]
mod weather;

use weather::*;

const BODY: &str = r#"{"latitude":-33.77856,"longitude":150.89552,"generationtime_ms":0.09,"utc_offset_seconds":36000,"timezone":"Australia/Sydney","timezone_abbreviation":"GMT+10","elevation":57.0,"current_units":{"time":"unixtime","interval":"seconds","temperature_2m":"°C","weather_code":"wmo code"},"current":{"time":1790432100,"interval":900,"temperature_2m":16.7,"weather_code":3},"daily_units":{"time":"unixtime","sunrise":"unixtime","sunset":"unixtime"},"daily":{"time":[1790431200],"sunrise":[1790451582],"sunset":[1790495759]}}"#;

#[test]
fn parses_real_response() {
    let w = parse(BODY).unwrap();
    assert_eq!(w.temperature_c, 16.7);
    assert_eq!(w.code, 3);
    assert_eq!(w.utc_offset_s, 36000);
    assert_eq!(w.sunrise, 1790451582);
    assert_eq!(w.sunset, 1790495759);
    assert_eq!(w.temperature_rounded(), 17);
}

#[test]
fn sky_through_the_day() {
    let w = parse(BODY).unwrap();
    let (sr, ss) = (w.sunrise, w.sunset);
    // 00:15 local that morning (fetch time was ~00:15)
    assert_eq!(w.sky(1790432100), Sky::Night);
    assert!(w.is_night(1790432100));
    assert_eq!(w.sky(sr - 20 * 60), Sky::Dawn);
    assert!(w.is_night(sr - 20 * 60));
    assert_eq!(w.sky(sr + 20 * 60), Sky::Dawn);
    assert!(!w.is_night(sr + 20 * 60));
    assert_eq!(w.sky(sr + 3 * 3600), Sky::Day);
    assert_eq!(w.sky(ss - 10 * 60), Sky::Dusk);
    assert_eq!(w.sky(ss + 10 * 60), Sky::Dusk);
    assert!(w.is_night(ss + 10 * 60));
    assert_eq!(w.sky(ss + 3 * 3600), Sky::Night);
    // Stale forecast: next day's noon must still be day, and 3am night.
    assert_eq!(w.sky(sr + 86400 + 5 * 3600), Sky::Day);
    assert_eq!(w.sky(sr + 86400 - 3 * 3600), Sky::Night);
    // Just before local midnight is night.
    assert_eq!(w.sky(1790431200 + 86400 - 60), Sky::Night);
}

#[test]
fn icons() {
    let mut w = parse(BODY).unwrap();
    let noon = w.sunrise + 5 * 3600;
    let night = w.sunset + 3 * 3600;
    w.code = 0;
    assert_eq!((w.icon(noon), w.icon(night)), (0, 1));
    w.code = 2;
    assert_eq!((w.icon(noon), w.icon(night)), (2, 3));
    for (code, icon) in [(3, 4), (45, 5), (61, 6), (81, 6), (53, 6), (73, 7), (86, 7), (95, 8)] {
        w.code = code;
        assert_eq!(w.icon(noon), icon, "code {code}");
    }
}

#[test]
fn local_clock_and_negative_temps() {
    // 1790432100 = 2026-09-26 14:15:00 UTC -> 00:15 AEST
    assert_eq!(local_hms(1790432100, 36000), (0, 15, 0));
    assert_eq!(local_hms(1790432100 + 59, 39600), (1, 15, 59));
    let mut w = parse(BODY).unwrap();
    w.temperature_c = -2.5;
    assert_eq!(w.temperature_rounded(), -3);
    w.temperature_c = -0.4;
    assert_eq!(w.temperature_rounded(), 0);
}
