//! Wi-Fi, clock sync (SNTP), location lookup (by IP) and weather fetching (Open-Meteo), all
//! over plain HTTP/UDP.
//!
//! Runs as embassy tasks next to the UI loop and publishes results into [`snapshot`].

use alloc::format;
use core::cell::Cell;

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
    join::JoinProblem,
    location::{self, Location},
    weather::{self, Weather},
};

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
}

impl Snapshot {
    /// Current Unix time in seconds, if the clock has been synced.
    pub fn unix_now(&self) -> Option<i64> {
        let offset = self.clock_offset_us?;
        Some((now_us() + offset).div_euclid(1_000_000))
    }
}

static SHARED: Mutex<Cell<Snapshot>> = Mutex::new(Cell::new(Snapshot {
    clock_offset_us: None,
    weather: None,
    join_problem: None,
}));

pub fn snapshot() -> Snapshot {
    critical_section::with(|cs| SHARED.borrow(cs).get())
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
        let forecast = fetch_weather(stack, located.unwrap_or(location::FALLBACK)).await;
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

async fn resolve(stack: Stack<'_>, host: &str) -> Result<IpAddress, &'static str> {
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
    let body = http_get(stack, location::HOST, location::PATH, &mut response).await?;
    let here = location::parse(body).ok_or("location parse")?;
    log::info!(
        "located near {} ({}, {})",
        location::city(body).unwrap_or("?"),
        here.latitude,
        here.longitude
    );
    Ok(here)
}

async fn fetch_weather(stack: Stack<'_>, at: Location) -> Result<Weather, &'static str> {
    // `timezone=auto`: the local time (and daylight saving) of the coordinates.
    let path = format!(
        "/v1/forecast?latitude={}&longitude={}\
         &current=temperature_2m,weather_code&daily=sunrise,sunset\
         &timezone=auto&timeformat=unixtime&forecast_days=1",
        at.latitude, at.longitude
    );
    let mut response = [0u8; 2048];
    let body = http_get(stack, WEATHER_HOST, &path, &mut response).await?;
    weather::parse(body).ok_or("weather parse")
}

/// A plain-HTTP GET; returns the body of a 200 reply, read into `response`.
async fn http_get<'a>(
    stack: Stack<'_>,
    host: &str,
    path: &str,
    response: &'a mut [u8],
) -> Result<&'a str, &'static str> {
    let server = resolve(stack, host).await?;

    let mut rx_buffer = [0u8; 1024];
    let mut tx_buffer = [0u8; 512];
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(NET_TIMEOUT));
    socket
        .connect((server, 80))
        .await
        .map_err(|_| "http connect")?;

    // HTTP/1.0 so the reply is never chunked and the server closes when done.
    let request = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n");
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
        return Err("http status");
    }
    Ok(text.split_once("\r\n\r\n").ok_or("http no body")?.1)
}
