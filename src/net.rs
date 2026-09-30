//! Wi-Fi, clock sync (SNTP), location lookup (by IP), weather fetching (Open-Meteo), and Home
//! Assistant calendars and media player, all over plain HTTP/UDP.
//!
//! Runs as embassy tasks next to the UI loop and publishes results into [`snapshot`] (and the
//! calendar into [`calendar`], what's playing into [`now_playing`]).

use alloc::{format, vec::Vec};
use core::cell::{Cell, RefCell};

use critical_section::Mutex;
use embassy_net::{
    IpAddress, Runner, Stack,
    dns::DnsQueryType,
    tcp::TcpSocket,
    udp::{PacketMetadata, UdpSocket},
};
use embassy_time::{Duration, Timer, with_timeout};
use esp_hal::time::Instant;
use esp_radio::wifi::{ConnectionError, DisconnectReason, Interface, WifiController};

use crate::{
    calendar::{self, Calendar, Event},
    join::JoinProblem,
    location::{self, Location},
    media::{self, NowPlaying},
    weather::{self, Weather},
};

/// Home Assistant, from `calendar.env` at build time (see `calendar.env.example`).
/// Without a host and token, nothing is fetched from it.
const HA_HOST: &str = env_or_empty(option_env!("HA_HOST"));
const HA_PORT: Option<&str> = option_env!("HA_PORT");
const HA_TOKEN: &str = env_or_empty(option_env!("HA_TOKEN"));
/// Comma-separated entity IDs whose events are shown, e.g. `calendar.family,calendar.sydney`.
const HA_CALENDARS: &str = env_or_empty(option_env!("HA_CALENDARS"));
/// Entity ID whose events say which bins to put out (instead of `bins::BINS`).
const HA_BIN_CALENDAR: &str = env_or_empty(option_env!("HA_BIN_CALENDAR"));
/// Entity ID of the media player to show on the now-playing slide, e.g. Spotify's.
const HA_MEDIA_PLAYER: &str = env_or_empty(option_env!("HA_MEDIA_PLAYER"));
/// Room for one Home Assistant reply: a calendar's, or the media player's state (about 1 KiB).
/// A bigger one is a failed fetch.
const HA_RESPONSE_MAX: usize = 8 * 1024;
/// Room for the forecast (about 1.1 KiB).
const WEATHER_RESPONSE_MAX: usize = 3 * 1024;

const CALENDAR_EVERY: Duration = Duration::from_secs(5 * 60);
/// Also how often Home Assistant is checked at all.
const MEDIA_EVERY: Duration = Duration::from_secs(10);

const fn env_or_empty(value: Option<&'static str>) -> &'static str {
    match value {
        Some(value) => value,
        None => "",
    }
}

fn ha_configured() -> bool {
    !HA_HOST.is_empty() && !HA_TOKEN.is_empty()
}

const WEATHER_HOST: &str = "api.open-meteo.com";
const NTP_HOST: &str = "pool.ntp.org";
const NTP_LOCAL_PORT: u16 = 12_345;
/// Seconds between 1900-01-01 (NTP epoch) and 1970-01-01 (Unix epoch).
const NTP_TO_UNIX: i64 = 2_208_988_800;

const REFRESH_EVERY: Duration = Duration::from_secs(15 * 60);
const RETRY_AFTER: Duration = Duration::from_secs(30);
const NET_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, Default)]
pub struct Snapshot {
    /// Unix time in microseconds minus `Instant::now()` in microseconds, once synced.
    clock_offset_us: Option<i64>,
    pub weather: Option<Weather>,
    /// Why the last attempt to join the saved network failed (cleared once joined).
    pub join_problem: Option<JoinProblem>,
    /// Goes up each time a new [`calendar`] is fetched.
    pub calendar_version: u32,
    /// Goes up each time [`now_playing`] changes.
    pub media_version: u32,
}

impl Snapshot {
    /// Current Unix time in seconds, if the clock has been synced.
    pub fn unix_now(&self) -> Option<i64> {
        let offset = self.clock_offset_us?;
        Some((now_us() + offset).div_euclid(1_000_000))
    }

    /// Local time = UTC + this: the forecast's, until then Sydney's standard time.
    pub fn utc_offset_s(&self) -> i32 {
        self.weather
            .map_or(weather::FALLBACK_UTC_OFFSET_S, |w| w.utc_offset_s)
    }
}

static SHARED: Mutex<Cell<Snapshot>> = Mutex::new(Cell::new(Snapshot {
    clock_offset_us: None,
    weather: None,
    join_problem: None,
    calendar_version: 0,
    media_version: 0,
}));

/// The last calendar fetched (kept out of [`Snapshot`], which is copied every UI refresh).
static CALENDAR: Mutex<RefCell<Option<Calendar>>> = Mutex::new(RefCell::new(None));
/// What the media player is playing, if anything.
static NOW_PLAYING: Mutex<RefCell<Option<NowPlaying>>> = Mutex::new(RefCell::new(None));

pub fn snapshot() -> Snapshot {
    critical_section::with(|cs| SHARED.borrow(cs).get())
}

pub fn calendar() -> Option<Calendar> {
    critical_section::with(|cs| CALENDAR.borrow_ref(cs).clone())
}

pub fn now_playing() -> Option<NowPlaying> {
    critical_section::with(|cs| NOW_PLAYING.borrow_ref(cs).clone())
}

fn publish(f: impl FnOnce(&mut Snapshot)) {
    critical_section::with(|cs| {
        let cell = SHARED.borrow(cs);
        let mut snapshot = cell.get();
        f(&mut snapshot);
        cell.set(snapshot);
    });
}

fn now_us() -> i64 {
    Instant::now().duration_since_epoch().as_micros() as i64
}

/// A first attempt failing and the next one working is normal, so a problem is only reported
/// after this many failures in a row.
const FAILURES_BEFORE_REPORTING: u32 = 3;
/// Below this (dBm, as measured by the failed attempt) the signal really is weak.
const WEAK_SIGNAL_DBM: i8 = -80;

/// Keeps the station connected, reconnecting after drops.
#[embassy_executor::task]
pub async fn wifi_task(mut controller: WifiController<'static>) {
    let mut failures = 0;
    loop {
        match controller.connect_async().await {
            Ok(info) => {
                log::info!("wifi connected: {info:?}");
                failures = 0;
                publish(|s| s.join_problem = None);
                let reason = controller.wait_for_disconnect_async().await.ok();
                log::warn!("wifi disconnected: {reason:?}");
            }
            Err(e) => {
                failures += 1;
                log::warn!("wifi connect failed ({failures} in a row): {e:?}");
                let problem = match e {
                    ConnectionError::Failed(info) => join_problem(info.reason, info.rssi),
                    _ => JoinProblem::Other,
                };
                if failures >= FAILURES_BEFORE_REPORTING {
                    publish(|s| s.join_problem = Some(problem));
                }
            }
        }
        Timer::after(Duration::from_secs(5)).await;
    }
}

/// `rssi` is the signal strength the failed attempt saw (0 if it never heard the network).
fn join_problem(reason: DisconnectReason, rssi: i8) -> JoinProblem {
    use DisconnectReason as R;
    if reason == R::NoAccessPointFoundInRssiThreshold || (rssi != 0 && rssi < WEAK_SIGNAL_DBM) {
        return JoinProblem::WeakSignal;
    }
    match reason {
        R::FourWayHandshakeTimeout | R::HandshakeTimeout | R::AuthenticationFailed => {
            JoinProblem::WrongPassword
        }
        R::NoAccessPointFound => JoinProblem::NotFound,
        R::NoAccessPointFoundWithCompatibleSecurity | R::NoAccessPointFoundInAuthmodeThreshold => {
            JoinProblem::Security
        }
        _ => JoinProblem::Other,
    }
}

#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, Interface>) {
    runner.run().await
}

/// Finds where the board is (once), then syncs the clock and fetches the weather every
/// 15 minutes (every 30 s until everything works).
#[embassy_executor::task]
pub async fn sync_task(stack: Stack<'static>) {
    // Part of the task, so in static memory, not taken from the heap on every fetch (where it
    // could fail to fit when Wi-Fi is busy).
    let mut response = [0u8; WEATHER_RESPONSE_MAX];
    let mut located: Option<Location> = None;
    loop {
        stack.wait_config_up().await;

        if located.is_none() {
            match locate(stack).await {
                Ok(here) => located = Some(here),
                Err(e) => log::warn!("location lookup failed: {e}"),
            }
        }

        let clock = sntp_offset_us(stack).await;
        match clock {
            Ok(offset) => {
                publish(|s| s.clock_offset_us = Some(offset));
                log::info!("clock synced");
            }
            Err(e) => log::warn!("time sync failed: {e}"),
        }

        // Until the lookup works, show the fallback location's weather rather than none.
        let forecast =
            fetch_weather(stack, located.unwrap_or(location::FALLBACK), &mut response).await;
        match forecast {
            Ok(weather) => {
                log::info!("weather: {weather:?}");
                publish(|s| s.weather = Some(weather));
            }
            Err(e) => log::warn!("weather fetch failed: {e}"),
        }

        let all_ok = located.is_some() && clock.is_ok() && forecast.is_ok();
        Timer::after(if all_ok { REFRESH_EVERY } else { RETRY_AFTER }).await;
    }
}

/// Home Assistant: the calendars every 5 minutes (every 30 s until it works), and the media
/// player every 10 s. Does nothing without a host and token.
#[embassy_executor::task]
pub async fn ha_task(stack: Stack<'static>) {
    if !ha_configured() {
        return;
    }
    // Shared by every fetch here (they take turns). In the task, so in static memory, not
    // taken from the heap on every fetch (where it could fail to fit when Wi-Fi is busy).
    let mut response = [0u8; HA_RESPONSE_MAX];
    let mut next_calendar = embassy_time::Instant::now();
    loop {
        stack.wait_config_up().await;

        if embassy_time::Instant::now() >= next_calendar {
            let s = snapshot();
            let fetched = match s.unix_now() {
                Some(now) => fetch_calendar(stack, now, s.utc_offset_s(), &mut response).await,
                None => Err("clock not synced yet"),
            };
            let wait = match fetched {
                Ok(fetched) => {
                    log::info!(
                        "calendar: {} events, bins from HA: {}",
                        fetched.events.len(),
                        fetched.bin_events.is_some()
                    );
                    critical_section::with(|cs| *CALENDAR.borrow_ref_mut(cs) = Some(fetched));
                    publish(|s| s.calendar_version = s.calendar_version.wrapping_add(1));
                    CALENDAR_EVERY
                }
                Err(e) => {
                    log::warn!("calendar fetch failed: {e}");
                    RETRY_AFTER
                }
            };
            next_calendar = embassy_time::Instant::now() + wait;
        }

        if !HA_MEDIA_PLAYER.trim().is_empty() {
            let playing = fetch_now_playing(stack, &mut response).await.unwrap_or_else(|e| {
                // Don't leave a song up that may have stopped long ago.
                log::warn!("media player fetch failed: {e}");
                None
            });
            let changed = critical_section::with(|cs| {
                let mut shown = NOW_PLAYING.borrow_ref_mut(cs);
                let changed = *shown != playing;
                *shown = playing;
                changed
            });
            if changed {
                log::info!("now playing: {:?}", now_playing().map(|p| p.title));
                publish(|s| s.media_version = s.media_version.wrapping_add(1));
            }
        }

        Timer::after(MEDIA_EVERY).await;
    }
}

async fn resolve(stack: Stack<'_>, host: &str) -> Result<IpAddress, &'static str> {
    if let Ok(address) = host.parse::<core::net::Ipv4Addr>() {
        return Ok(IpAddress::Ipv4(address));
    }
    let addresses = stack
        .dns_query(host, DnsQueryType::A)
        .await
        .map_err(|_| "dns")?;
    addresses.first().copied().ok_or("dns: no address")
}

/// One SNTP round trip; returns the Unix-minus-`Instant` offset in microseconds.
async fn sntp_offset_us(stack: Stack<'_>) -> Result<i64, &'static str> {
    let server = resolve(stack, NTP_HOST).await?;

    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut rx_buffer = [0u8; 64];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_buffer = [0u8; 64];
    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    socket.bind(NTP_LOCAL_PORT).map_err(|_| "ntp bind")?;

    let mut packet = [0u8; 48];
    packet[0] = 0x1B; // version 3, client mode
    socket
        .send_to(&packet, (server, 123))
        .await
        .map_err(|_| "ntp send")?;
    let (len, _) = with_timeout(NET_TIMEOUT, socket.recv_from(&mut packet))
        .await
        .map_err(|_| "ntp timeout")?
        .map_err(|_| "ntp recv")?;
    let received_us = now_us();
    if len < 48 {
        return Err("ntp short reply");
    }

    // Transmit timestamp: seconds + 32-bit fraction since 1900.
    let secs = i64::from(u32::from_be_bytes([
        packet[40], packet[41], packet[42], packet[43],
    ]));
    let frac = i64::from(u32::from_be_bytes([
        packet[44], packet[45], packet[46], packet[47],
    ]));
    let unix_us = (secs - NTP_TO_UNIX) * 1_000_000 + ((frac * 1_000_000) >> 32);
    Ok(unix_us - received_us)
}

async fn locate(stack: Stack<'_>) -> Result<Location, &'static str> {
    let mut response = [0u8; 512];
    let body = http_get(stack, location::HOST, 80, location::PATH, "", &mut response).await?;
    let here = location::parse(body).ok_or("location parse")?;
    log::info!(
        "located near {} ({}, {})",
        location::city(body).unwrap_or("?"),
        here.latitude,
        here.longitude
    );
    Ok(here)
}

async fn fetch_weather(
    stack: Stack<'_>,
    at: Location,
    response: &mut [u8],
) -> Result<Weather, &'static str> {
    // `timezone=auto`: the local time (and daylight saving) of the coordinates.
    let path = format!(
        "/v1/forecast?latitude={}&longitude={}\
         &current=temperature_2m,weather_code\
         &daily=sunrise,sunset,temperature_2m_max,temperature_2m_min,precipitation_probability_max\
         &hourly=temperature_2m,weather_code&forecast_hours={}\
         &timezone=auto&timeformat=unixtime&forecast_days=1",
        at.latitude,
        at.longitude,
        weather::HOURS_FETCHED
    );
    let body = http_get(stack, WEATHER_HOST, 80, &path, "", response).await?;
    weather::parse(body).ok_or("weather parse")
}

/// The next [`calendar::DAYS_FETCHED`] days' events (local days, from today) from each of
/// `HA_CALENDARS`, and from `HA_BIN_CALENDAR` if set. Any calendar failing fails the lot, so the
/// last good one stays up.
async fn fetch_calendar(
    stack: Stack<'_>,
    now: i64,
    utc_offset_s: i32,
    response: &mut [u8],
) -> Result<Calendar, &'static str> {
    let day = calendar::local_day(now, utc_offset_s);
    let midnight = |day: i64| day * 24 * 60 * 60 - i64::from(utc_offset_s);
    let from = calendar::iso_utc(midnight(day));
    let to = calendar::iso_utc(midnight(day + calendar::DAYS_FETCHED));

    let mut events = Vec::new();
    for entity in HA_CALENDARS
        .split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
    {
        events.extend(fetch_events(stack, entity, &from, &to, response).await?);
    }
    let bin_events = match HA_BIN_CALENDAR.trim() {
        "" => None,
        entity => Some(fetch_events(stack, entity, &from, &to, response).await?),
    };
    Ok(Calendar {
        day,
        events,
        bin_events,
    })
}

async fn fetch_events(
    stack: Stack<'_>,
    entity: &str,
    from: &str,
    to: &str,
    response: &mut [u8],
) -> Result<Vec<Event>, &'static str> {
    let path = format!("/api/calendars/{entity}?start={from}&end={to}");
    let body = ha_get(stack, &path, response).await?;
    calendar::parse(body).ok_or("calendar parse")
}

async fn fetch_now_playing(
    stack: Stack<'_>,
    response: &mut [u8],
) -> Result<Option<NowPlaying>, &'static str> {
    let path = format!("/api/states/{}", HA_MEDIA_PLAYER.trim());
    let body = ha_get(stack, &path, response).await?;
    media::parse(body).ok_or("media player parse")
}

/// A GET from Home Assistant's REST API, with the token.
async fn ha_get<'a>(
    stack: Stack<'_>,
    path: &str,
    response: &'a mut [u8],
) -> Result<&'a str, &'static str> {
    let port = HA_PORT.and_then(|p| p.trim().parse().ok()).unwrap_or(8123);
    let auth = format!("Authorization: Bearer {HA_TOKEN}\r\n");
    http_get(stack, HA_HOST, port, path, &auth, response).await
}

/// A plain-HTTP GET, with `headers` (each ending in `\r\n`) added to the request; returns the
/// body of a 200 reply, read into `response`.
async fn http_get<'a>(
    stack: Stack<'_>,
    host: &str,
    port: u16,
    path: &str,
    headers: &str,
    response: &'a mut [u8],
) -> Result<&'a str, &'static str> {
    let server = resolve(stack, host).await?;

    let mut rx_buffer = [0u8; 1024];
    let mut tx_buffer = [0u8; 512];
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(NET_TIMEOUT));
    socket
        .connect((server, port))
        .await
        .map_err(|_| "http connect")?;

    // HTTP/1.0 so the reply is never chunked and the server closes when done.
    let request =
        format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n{headers}\r\n");
    let mut sent = 0;
    while sent < request.len() {
        sent += socket
            .write(&request.as_bytes()[sent..])
            .await
            .map_err(|_| "http write")?;
    }

    let mut len = 0;
    loop {
        if len == response.len() {
            return Err("http response too large");
        }
        match socket.read(&mut response[len..]).await {
            Ok(0) => break,
            Ok(n) => len += n,
            Err(_) => return Err("http read"),
        }
    }
    socket.close();

    let text = core::str::from_utf8(&response[..len]).map_err(|_| "http utf8")?;
    let status_ok = text.split(' ').nth(1).is_some_and(|status| status == "200");
    if !status_ok {
        // e.g. "HTTP/1.0 401 Unauthorized" (wrong token) or "404 Not Found" (no such calendar).
        log::warn!("{host}: {}", text.lines().next().unwrap_or(""));
        return Err("http status");
    }
    Ok(text.split_once("\r\n\r\n").ok_or("http no body")?.1)
}
