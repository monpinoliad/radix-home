extern crate alloc;

#[path = "../../src/bins.rs"]
mod bins;
#[path = "../../src/calendar.rs"]
mod calendar;
#[path = "../../src/json.rs"]
mod json;
#[path = "../../src/media.rs"]
mod media;

use media::*;

/// A real reply from Home Assistant's `/api/states/media_player.<spotify>`.
const PLAYING: &str = r#"{"entity_id":"media_player.living_room","state":"playing","attributes":{"source_list":["Speaker"],"volume_level":0.8,"media_content_id":"spotify:track:1SyaSFBojlBjLWI2raj340","media_content_type":"music","media_duration":222,"media_position":30,"media_position_updated_at":"2026-09-28T11:37:21.240370+00:00","media_title":"keep me low","media_artist":"Maya Vance","media_album_name":"keep me low","media_track":1,"source":"Speaker","shuffle":false,"repeat":"off","entity_picture":"/api/media_player_proxy/media_player.living_room?token=0000&cache=0000","friendly_name":"Living Room","supported_features":444983},"last_changed":"2026-09-28T10:21:03.270953+00:00","last_reported":"2026-09-28T11:37:21.240690+00:00","last_updated":"2026-09-28T11:37:21.240690+00:00","context":{"id":"00000000000000000000000000","parent_id":null,"user_id":null}}"#;

/// 2026-09-28 11:37:21 UTC.
const UPDATED_AT: i64 = 1_790_595_441;

#[test]
fn parses_real_reply() {
    let playing = parse(PLAYING).unwrap().unwrap();
    assert_eq!(playing.title, "keep me low");
    assert_eq!(playing.artist, "Maya Vance");
    assert_eq!(playing.duration_s, Some(222));
    assert_eq!(playing.position_s, Some(30));
    assert_eq!(playing.position_at, Some(UPDATED_AT));
}

#[test]
fn not_playing_or_not_a_player() {
    for state in ["paused", "idle", "off", "unavailable"] {
        let body = PLAYING.replace("\"state\":\"playing\"", &format!("\"state\":\"{state}\""));
        assert_eq!(parse(&body), Some(None), "{state}");
    }
    assert_eq!(parse(r#"{"message":"Entity not found."}"#), None);
    assert_eq!(parse("401: Unauthorized"), None);
}

#[test]
fn progress_moves_on_between_updates() {
    let playing = parse(PLAYING).unwrap().unwrap();
    assert_eq!(playing.elapsed_s(UPDATED_AT), Some(30));
    assert_eq!(playing.elapsed_s(UPDATED_AT + 60), Some(90));
    // Never past the end, nor before the last update.
    assert_eq!(playing.elapsed_s(UPDATED_AT + 1000), Some(222));
    assert_eq!(playing.elapsed_s(UPDATED_AT - 100), Some(30));
    assert_eq!(playing.progress(UPDATED_AT + 81), Some(0.5));
}

#[test]
fn missing_position_or_length() {
    let body = PLAYING
        .replace("\"media_duration\":222,", "")
        .replace("\"media_position\":30,", "")
        .replace("\"media_artist\":\"Maya Vance\",", "\"media_artist\":null,");
    let playing = parse(&body).unwrap().unwrap();
    assert_eq!((playing.duration_s, playing.position_s), (None, None));
    assert_eq!(playing.artist, "");
    assert_eq!(playing.elapsed_s(UPDATED_AT), None);
    assert_eq!(playing.progress(UPDATED_AT), None);
    // Fractional seconds, as some players send.
    let body = PLAYING.replace("\"media_duration\":222", "\"media_duration\":222.46");
    assert_eq!(parse(&body).unwrap().unwrap().duration_s, Some(222));
}

#[test]
fn durations() {
    assert_eq!(duration_text(0), "0:00");
    assert_eq!(duration_text(83), "1:23");
    assert_eq!(duration_text(222), "3:42");
    assert_eq!(duration_text(3723), "1:02:03");
}
