#[path = "../../src/slides.rs"]
mod slides;

use slides::*;

#[test]
fn goes_round_in_order_skipping_empty_slides() {
    let all = |_| true;
    assert_eq!(next(Slide::Clock, all), Slide::Agenda);
    assert_eq!(next(Slide::Agenda, all), Slide::Forecast);
    assert_eq!(next(Slide::Forecast, all), Slide::Music);
    assert_eq!(next(Slide::Music, all), Slide::Clock);

    let no_music = |s| s != Slide::Music;
    assert_eq!(next(Slide::Forecast, no_music), Slide::Clock);
    let only_music = |s| s == Slide::Music || s == Slide::Clock;
    assert_eq!(next(Slide::Clock, only_music), Slide::Music);
    // Nothing else to show: stay on the clock.
    assert_eq!(next(Slide::Clock, |s| s == Slide::Clock), Slide::Clock);
    // The music stopped while it was up: on to the clock.
    assert_eq!(next(Slide::Music, only_music), Slide::Clock);
}

#[test]
fn clock_stays_longest() {
    assert_eq!(Slide::Clock.stays_ms(), 120_000);
    for slide in [Slide::Agenda, Slide::Forecast, Slide::Music] {
        assert!(slide.stays_ms() <= 20_000, "{slide:?}");
    }
    for (i, slide) in ORDER.iter().enumerate() {
        assert_eq!(*slide as usize, i);
    }
}

#[test]
fn slides_smoothly_on_the_pixel_grid() {
    assert_eq!(shift(0), 0);
    assert_eq!(shift(SLIDE_MS), 240);
    assert_eq!(shift(SLIDE_MS * 2), 240);
    let mut last = 0;
    for ms in 0..=SLIDE_MS {
        let px = shift(ms);
        assert!(px % 2 == 0 && px >= last && px <= 240, "{ms} ms: {px}");
        last = px;
    }
    // Eased: slow at the ends, fast in the middle.
    assert!(shift(SLIDE_MS / 10) < 24);
    assert_eq!(shift(SLIDE_MS / 2), 120);
}
