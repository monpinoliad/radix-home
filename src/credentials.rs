//! Wi-Fi credentials and the small record they're saved as in flash.

use alloc::string::{String, ToString};

pub const SSID_MAX: usize = 32;
/// WPA2 passphrases are 8..=63 characters; empty means an open network.
pub const PASSWORD_MIN: usize = 8;
pub const PASSWORD_MAX: usize = 63;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub ssid: String,
    pub password: String,
}

impl Credentials {
    /// Checks the lengths the Wi-Fi driver and WPA2 will insist on.
    /// The error is shown to the user on the setup page.
    pub fn new(ssid: &str, password: &str) -> Result<Self, &'static str> {
        if ssid.is_empty() {
            return Err("Pick a network, or type its name.");
        }
        if ssid.len() > SSID_MAX {
            return Err("That network name is too long (max 32 bytes).");
        }
        if !password.is_empty() && !(PASSWORD_MIN..=PASSWORD_MAX).contains(&password.len()) {
            return Err(
                "Wi-Fi passwords are 8 to 63 characters (leave it empty for an open network).",
            );
        }
        Ok(Self {
            ssid: ssid.to_string(),
            password: password.to_string(),
        })
    }

    pub fn is_open(&self) -> bool {
        self.password.is_empty()
    }
}

// Record layout (little-endian), 108 bytes so it's a multiple of the 4-byte flash word:
//   0..4    magic "PSET"
//   4       version
//   5       SSID length
//   6       password length
//   7       reserved
//   8..40   SSID
//   40..104 password
//   104..108 FNV-1a checksum of bytes 0..104
pub const RECORD_LEN: usize = 108;
const MAGIC: [u8; 4] = *b"PSET";
const VERSION: u8 = 1;
const SSID_AT: usize = 8;
const PASSWORD_AT: usize = SSID_AT + SSID_MAX;
const CHECKSUM_AT: usize = PASSWORD_AT + 64;

pub fn encode(credentials: &Credentials) -> [u8; RECORD_LEN] {
    let ssid = credentials.ssid.as_bytes();
    let password = credentials.password.as_bytes();
    let mut record = [0u8; RECORD_LEN];
    record[..4].copy_from_slice(&MAGIC);
    record[4] = VERSION;
    record[5] = ssid.len() as u8;
    record[6] = password.len() as u8;
    record[SSID_AT..SSID_AT + ssid.len()].copy_from_slice(ssid);
    record[PASSWORD_AT..PASSWORD_AT + password.len()].copy_from_slice(password);
    let checksum = fnv1a(&record[..CHECKSUM_AT]);
    record[CHECKSUM_AT..].copy_from_slice(&checksum.to_le_bytes());
    record
}

/// `None` for erased flash, another format, or a torn write.
pub fn decode(record: &[u8; RECORD_LEN]) -> Option<Credentials> {
    if record[..4] != MAGIC || record[4] != VERSION {
        return None;
    }
    let stored = u32::from_le_bytes(record[CHECKSUM_AT..].try_into().ok()?);
    if stored != fnv1a(&record[..CHECKSUM_AT]) {
        return None;
    }
    let (ssid_len, password_len) = (usize::from(record[5]), usize::from(record[6]));
    let ssid = core::str::from_utf8(record.get(SSID_AT..SSID_AT + ssid_len)?).ok()?;
    let password =
        core::str::from_utf8(record.get(PASSWORD_AT..PASSWORD_AT + password_len)?).ok()?;
    Credentials::new(ssid, password).ok()
}

fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811C_9DC5, |hash, &b| {
        (hash ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}
