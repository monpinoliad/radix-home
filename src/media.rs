//! What's playing, from a Home Assistant media player such as Spotify (`/api/states/<entity>`).

use alloc::{format, string::String};

use crate::{calendar, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    /// Track length in seconds, if known.
    pub duration_s: Option<i64>,
    /// Position in seconds at `position_at` (Unix seconds), if known.
    pub position_s: Option<i64>,
    pub position_at: Option<i64>,
}

/// Parses the body of `/api/states/media_player.<name>`: `Some(None)` when it isn't playing
/// (paused, idle, off...), `None` if the reply isn't a media player's state.
pub fn parse(body: &str) -> Option<Option<NowPlaying>> {
    let state = json::string_value(body, "\"state\":")?;
    if state != "playing" {
        return Some(None);
    }
    let attributes = json::after(body, "\"attributes\":{")?;
    let seconds = |key| {
        json::number(attributes, key)
            .and_then(|n| n.split('.').next()?.parse::<i64>().ok())
            .filter(|&n| n >= 0)
    };
    Some(Some(NowPlaying {
        title: json::string_value(attributes, "\"media_title\":").unwrap_or_default(),
        artist: json::string_value(attributes, "\"media_artist\":").unwrap_or_default(),
        duration_s: seconds("\"media_duration\":").filter(|&d| d > 0),
        position_s: seconds("\"media_position\":"),
        position_at: json::string_value(attributes, "\"media_position_updated_at\":")
            .and_then(|at| calendar::date_time(&at)),
    }))
}

impl NowPlaying {
    /// Seconds into the track at `now` (Home Assistant only updates the position now and then),
    /// no further than its end.
    pub fn elapsed_s(&self, now: i64) -> Option<i64> {
        let elapsed = self.position_s? + (now - self.position_at?).max(0);
        Some(self.duration_s.map_or(elapsed, |d| elapsed.min(d)))
    }

    /// How far through the track (0 to 1), if known.
    pub fn progress(&self, now: i64) -> Option<f32> {
        Some(self.elapsed_s(now)? as f32 / self.duration_s? as f32)
    }
}

/// "3:42", "1:02:03".
pub fn duration_text(seconds: i64) -> String {
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
