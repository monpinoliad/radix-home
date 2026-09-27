#[path = "../../src/location.rs"]
mod location;

use location::*;

#[test]
fn parses_real_response() {
    let body = r#"{"status":"success","city":"Blacktown","lat":-33.7717,"lon":150.9083}"#;
    let here = parse(body).unwrap();
    assert_eq!(here.latitude, Coordinate(-3377));
    assert_eq!(here.longitude, Coordinate(15090));
    assert_eq!(city(body), Some("Blacktown"));
    assert_eq!(here.latitude.to_string(), "-33.77");
    assert_eq!(here.longitude.to_string(), "150.90");
}

#[test]
fn formats_small_and_negative_values() {
    assert_eq!(Coordinate(-5).to_string(), "-0.05");
    assert_eq!(Coordinate(5).to_string(), "0.05");
    assert_eq!(Coordinate(-12000).to_string(), "-120.00");
    assert_eq!(FALLBACK.latitude.to_string(), "-33.77");
}

#[test]
fn short_and_whole_numbers() {
    let here = parse(r#"{"status":"success","lat":-0.5,"lon":7}"#).unwrap();
    assert_eq!(here.latitude, Coordinate(-50));
    assert_eq!(here.longitude, Coordinate(700));
    assert_eq!(city(r#"{"status":"success","lat":1,"lon":2}"#), None);
}

#[test]
fn rejects_failures_and_nonsense() {
    assert_eq!(parse(r#"{"status":"fail","message":"private range"}"#), None);
    assert_eq!(parse(r#"{"status":"success","lat":"x","lon":1}"#), None);
    assert_eq!(parse(r#"{"status":"success","lat":95.1,"lon":1}"#), None);
    assert_eq!(parse(r#"{"status":"success","lat":1}"#), None);
}
