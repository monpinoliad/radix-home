//! Which slide is up: the clock most of the time, then each other slide that has something to
//! show for a little while, sliding in from the right.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slide {
    /// Big clock, date, weather, today's events, bins.
    Clock = 0,
    /// The coming days' events.
    Agenda = 1,
    /// Today's high/low, rain and the next hours.
    Forecast = 2,
    /// What's playing on Spotify (only while it plays).
    Music = 3,
}

/// Order they come round in; `Slide as usize` indexes this.
pub const ORDER: [Slide; 4] = [Slide::Clock, Slide::Agenda, Slide::Forecast, Slide::Music];

/// How long a slide takes to slide in (ms).
pub const SLIDE_MS: u64 = 600;

/// Screen width: how far a slide moves.
const WIDTH: i32 = 240;

impl Slide {
    /// How long it stays up (ms).
    pub fn stays_ms(self) -> u64 {
        match self {
            Slide::Clock => 2 * 60 * 1000,
            Slide::Music => 20 * 1000,
            Slide::Agenda | Slide::Forecast => 15 * 1000,
        }
    }
}

/// The slide after `current` that has something to show, or the clock after the last one.
pub fn next(current: Slide, has_content: impl Fn(Slide) -> bool) -> Slide {
    ORDER[current as usize + 1..]
        .iter()
        .copied()
        .find(|&slide| has_content(slide))
        .unwrap_or(Slide::Clock)
}

/// How far (px, 0 to 240) the new slide has come in `elapsed_ms` into a slide: eased in and out,
/// on the 2px pixel grid.
pub fn shift(elapsed_ms: u64) -> i32 {
    if elapsed_ms >= SLIDE_MS {
        return WIDTH;
    }
    let t = elapsed_ms as f32 / SLIDE_MS as f32;
    let eased = t * t * (3.0 - 2.0 * t);
    (eased * (WIDTH / 2) as f32) as i32 * 2
}
