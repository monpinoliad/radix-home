//! Why joining the saved network failed, in words for the screen and the setup page.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum JoinProblem {
    WrongPassword = 1,
    NotFound = 2,
    Security = 3,
    WeakSignal = 4,
    /// Joined, but never got an address (or failed for another reason).
    Other = 5,
}

impl JoinProblem {
    /// For keeping it across a restart as a plain number.
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            1 => Self::WrongPassword,
            2 => Self::NotFound,
            3 => Self::Security,
            4 => Self::WeakSignal,
            5 => Self::Other,
            _ => return None,
        })
    }

    /// Short, in the uppercase pixel font (see `TEXT_CHARS` in `setup/mod.rs`), ≤ 15 characters.
    pub fn screen_text(self) -> &'static str {
        match self {
            Self::WrongPassword => "WRONG PASSWORD?",
            Self::NotFound => "WI-FI NOT FOUND",
            Self::Security => "USE WPA2 WI-FI",
            Self::WeakSignal => "WEAK WI-FI",
            Self::Other => "WI-FI FAILED",
        }
    }

    /// Shown on the setup page after the board gave up on the saved network.
    pub fn page_text(self) -> &'static str {
        match self {
            Self::WrongPassword => {
                "Couldn't join the saved network: the password looks wrong. Pick it and try again."
            }
            Self::NotFound => {
                "Couldn't find the saved network. Is the router on, in range, and using 2.4 GHz?"
            }
            Self::Security => {
                "The saved network's security isn't supported. Set the router to WPA2 or \
                 WPA2/WPA3 (not WPA3 only)."
            }
            Self::WeakSignal => {
                "The saved network's signal was too weak. Try moving the clock closer to the router."
            }
            Self::Other => "Couldn't join the saved network. Check the password and try again.",
        }
    }
}
