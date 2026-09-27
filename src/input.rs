//! The BOOT button's one job: a long hold opens (or leaves) Wi-Fi setup.
//! Pure logic (times in milliseconds), so it's tested on the PC in `tools/host_tests`.

pub const HOLD_MS: u64 = 3000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Waiting for the button to be released before anything counts. The BOOT pin is a
    /// strapping pin, so a press that started before boot isn't ours.
    Disarmed,
    Up,
    Down {
        since: u64,
    },
    /// Hold already reported; ignore the rest of this press.
    Held,
}

pub struct HoldButton {
    state: State,
}

impl Default for HoldButton {
    fn default() -> Self {
        Self::new()
    }
}

impl HoldButton {
    pub const fn new() -> Self {
        Self {
            state: State::Disarmed,
        }
    }

    /// Feed one sample (`pressed` = button down) taken at `now` ms. Returns true once per
    /// press, as soon as it has been held for [`HOLD_MS`] (without waiting for the release).
    pub fn update(&mut self, pressed: bool, now: u64) -> bool {
        let (state, held) = match (self.state, pressed) {
            (_, false) => (State::Up, false),
            (State::Up, true) => (State::Down { since: now }, false),
            (State::Down { since }, true) if now - since >= HOLD_MS => (State::Held, true),
            (state, true) => (state, false),
        };
        self.state = state;
        held
    }
}
