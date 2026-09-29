extern crate alloc;

#[path = "../../src/json.rs"]
mod json;
#[path = "../../src/weather.rs"]
mod weather;

use weather::*;

const BODY: &str = r#"{"latitude":-33.77856,"longitude":150.89552,"generationtime_ms":0.09,"utc_offset_seconds":36000,"timezone":"Australia/Sydney","timezone_abbreviation":"GMT+10","elevation":57.0,"current_units":{"time":"unixtime","interval":"seconds","temperature_2m":"°C","weather_code":"wmo code"},"current":{"time":1790432100,"interval":900,"temperature_2m":16.7,"weather_code":3},"hourly_units":{"time":"unixtime","temperature_2m":"°C","weather_code":"wmo code"},"hourly":{"time":[1790431200,1790434800,1790438400,1790442000,1790445600,1790449200,1790452800,1790456400,1790460000,1790463600,1790467200,1790470800,1790474400],"temperature_2m":[12.0,11.5,11.0,10.4,9.9,9.8,10.6,12.9,15.1,16.8,18.2,19.0,19.4],"weather_code":[3,3,2,1,0,0,0,1,2,61,61,3,3]},"daily_units":{"time":"unixtime","sunrise":"unixtime","sunset":"unixtime","temperature_2m_max":"°C","temperature_2m_min":"°C","precipitation_probability_max":"%"},"daily":{"time":[1790431200],"sunrise":[1790451582],"sunset":[1790495759],"temperature_2m_max":[19.4],"temperature_2m_min":[9.8],"precipitation_probability_max":[35]}}"#;

/// A real reply to the full request (28 Sep 2026, 9:30 PM in Blacktown).
const REAL_BODY: &str = r#"{"latitude":-33.77856,"longitude":150.89552,"generationtime_ms":0.19633769989013672,"utc_offset_seconds":36000,"timezone":"Australia/Sydney","timezone_abbreviation":"GMT+10","elevation":57.0,"current_units":{"time":"unixtime","interval":"seconds","temperature_2m":"°C","weather_code":"wmo code"},"current":{"time":1790595000,"interval":900,"temperature_2m":13.4,"weather_code":0},"hourly_units":{"time":"unixtime","temperature_2m":"°C","weather_code":"wmo code"},"hourly":{"time":[1790593200,1790596800,1790600400,1790604000,1790607600,1790611200,1790614800,1790618400,1790622000,1790625600,1790629200,1790632800,1790636400],"temperature_2m":[13.5,13.3,12.6,12.0,11.3,11.1,11.3,11.2,10.6,10.2,11.4,13.8,16.1],"weather_code":[1,0,0,0,1,0,0,0,1,1,1,0,1]},"daily_units":{"time":"unixtime","sunrise":"unixtime","sunset":"unixtime","temperature_2m_max":"°C","temperature_2m_min":"°C","precipitation_probability_max":"%"},"daily":{"time":[1790517600],"sunrise":[1790537898],"sunset":[1790582201],"temperature_2m_max":[17.3],"temperature_2m_min":[12.2],"precipitation_probability_max":[76]}}"#;

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

#[test]
fn parses_forecast() {
    let w = parse(BODY).unwrap();
    assert_eq!((w.high_c, w.low_c, w.rain_chance), (19.4, 9.8, Some(35)));
    assert_eq!(w.hours[0], Hour { time: 1790431200, temperature_c: 12.0, code: 3 });
    assert_eq!(w.hours[12], Hour { time: 1790431200 + 12 * 3600, temperature_c: 19.4, code: 3 });

    let real = parse(REAL_BODY).unwrap();
    assert_eq!(real.temperature_c, 13.4);
    assert_eq!((real.high_c, real.low_c, real.rain_chance), (17.3, 12.2, Some(76)));
    assert_eq!(real.hours[1], Hour { time: 1790596800, temperature_c: 13.3, code: 0 });
    assert_eq!(real.hours[12], Hour { time: 1790636400, temperature_c: 16.1, code: 1 });
}

#[test]
fn rain_chance_can_be_missing_and_hours_short() {
    let body = REAL_BODY.replace("\"precipitation_probability_max\":[76]", "\"precipitation_probability_max\":[null]");
    assert_eq!(parse(&body).unwrap().rain_chance, None);
    // Only 3 hours in the reply: the rest are left empty and never come after `now`.
    let short = REAL_BODY
        .replace("[1790593200,1790596800,1790600400,1790604000,1790607600,1790611200,1790614800,1790618400,1790622000,1790625600,1790629200,1790632800,1790636400]", "[1790593200,1790596800,1790600400]")
        .replace("[13.5,13.3,12.6,12.0,11.3,11.1,11.3,11.2,10.6,10.2,11.4,13.8,16.1]", "[13.5,13.3,12.6]")
        .replace("[1,0,0,0,1,0,0,0,1,1,1,0,1]", "[1,0,0]");
    let w = parse(&short).unwrap();
    assert_eq!(w.hours[3].time, 0);
    assert_eq!(w.hours_after(1790595000).count(), 2);
}

#[test]
fn next_hours_every_three() {
    let w = parse(REAL_BODY).unwrap();
    // 9:30 PM: 10 PM, 1 AM, 4 AM, 7 AM.
    let times: Vec<i64> = w.hours_after(1790595000).step_by(3).take(4).map(|h| h.time).collect();
    assert_eq!(times, [1790596800, 1790607600, 1790618400, 1790629200]);
    let hours: Vec<i32> = times.iter().map(|&t| local_hms(t, w.utc_offset_s).0).collect();
    assert_eq!(hours, [22, 1, 4, 7]);
    // Clear at night shows the night icon; mainly clear by day the day one.
    assert_eq!(w.hour_icon(w.hours[1]), 1);
    assert_eq!(w.hour_icon(Hour { time: w.sunrise + 3600, temperature_c: 0.0, code: 1 }), 0);
    assert_eq!(icon_for(61, true), 6);
    assert_eq!((round_c(12.5), round_c(-0.5), round_c(9.49)), (13, -1, 9));
}
