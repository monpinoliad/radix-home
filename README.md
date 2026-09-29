# radix-home

A pixel-art home clock on the [ESP32-S3-LCD-1.28-B](https://www.waveshare.com/wiki/ESP32-S3-LCD-1.28)
(the non-touch board in the metal case), rendered with [Slint](https://slint.dev).

- Today's date and weekday (`MON 28 SEP`) above the time in 12-hour AM/PM (synced over the internet) and the current weather where the board is, in a pixel font
  with pixel weather icons. The location is found automatically from the internet connection.
- **Today's events** from your Home Assistant calendars under the weather, e.g. `7:00PM DINNER`
  or `LABOUR DAY`, one at a time; long ones scroll.
- **Bin night**: on bin day from 6 PM until midnight, the bins to put out stand on the grass as
  pixel wheelie bins in their colours, with **BIN NIGHT** underneath. Nothing shows otherwise.
- The sky follows the real day: dawn, day, dusk and night colours from today's sunrise/sunset.
  From sunset to sunrise, stars twinkle and a small moon circles the edge of the screen.
- The screen stays at full brightness; it never dims or turns off.

## Slides

The clock stays up for 2 minutes, then the other slides slide in from the right for 15 s each
(20 s for music) and it's back to the clock. Every slide has the time at the top. A slide with
nothing to show is skipped.

| Slide | Shows |
| --- | --- |
| Clock | Date, time, weather, today's events, bins on bin night |
| Coming up | The next 7 days' events (up to 5), e.g. `THU 3:30PM DENTIST`, and bin nights |
| Forecast | Today's high/low and chance of rain, then every 3 hours for the next 12 |
| Now playing | Only while the media player plays: title, artist, progress bar |

How long each stays is `stays_ms` in `src/slides.rs`, the slide speed `SLIDE_MS`.

## Calendar (Home Assistant)

Copy `calendar.env.example` to `calendar.env` (git-ignored), fill it in and rebuild:

- `HA_HOST`: Home Assistant's **local** address (e.g. `192.168.1.10`). A Tailscale (`100.x`) or
  remote address won't work: the board only reaches the local network, over plain HTTP.
- `HA_TOKEN`: a long-lived access token (your HA profile → **Security** →
  **Long-lived access tokens**).
- `HA_CALENDARS`: the calendars whose events to show, comma-separated (e.g.
  `calendar.family,calendar.sydney`). Find the IDs under Settings → Devices & services →
  Entities, searching for `calendar.`.
- `HA_BIN_CALENDAR` (optional): the calendar that says which bins to put out; see below.
- `HA_MEDIA_PLAYER` (optional): the media player for the now-playing slide, e.g. Spotify's
  `media_player.living_room` (find it under Entities, searching for `media_player.`).

Today's all-day events show first, then timed ones by start time until they end.

**Important events**: start the title with `!` (or ❗), e.g. `! Dentist`, in any calendar. It shows
first, with a red `!` badge in front (the marker itself isn't shown), and stays up twice as long. Titles are
shown in the pixel font: accents are dropped and emoji and other symbols left out. Calendars are
fetched every 5 minutes, so a new event can take that long to appear; the media player every
10 seconds.

The token is built into the firmware, so anyone with the board could read it off the flash.
Give it its own HA user if that matters to you.

## Bins

With `HA_BIN_CALENDAR` set, the bins come from that calendar: an event on the day that names a
colour (`Red`, `Green`, `Yellow`, or 🔴 🟢 🟡) puts that bin out from 6 PM, e.g.
"🔴🟢 Put out Red + Green Bins". A day with no bin events shows no bins.

Without it, or while Home Assistant can't be reached, the built-in schedule is used:

| Bin | When |
| --- | --- |
| Red | Every Tuesday |
| Green | Every Tuesday |
| Yellow | Every second Tuesday, from 29 September 2026 |

Change the built-in schedule in `BINS` in `src/bins.rs` (how many weeks apart, and a first
reminder date), and the start time (for both) in `REMINDER_FROM_HOUR`.

## Controls

**Hold BOOT for 3 seconds** to open Wi-Fi setup (or leave it). That's the only control.
A press that is already held when the board starts is ignored (holding BOOT while resetting
enters download mode instead).

## Wi-Fi setup

Pick the network from your phone, no rebuild needed:

1. The first time (or whenever it has no network), the screen shows **WI-FI SETUP**.
2. On your phone, join the open Wi-Fi network **Radix-Setup**. The setup page should pop up
   by itself; if not, open <http://192.168.4.1>.
3. Pick your network, type the password, **Save & connect**. The clock restarts and joins it.

The choice is saved in flash, so it survives power-off and re-flashing (unless you erase the
whole chip). To change networks later, **hold BOOT for 3 seconds**; hold it again to leave setup
without changing anything.

While joining, the screen is black with a Wi-Fi logo lighting up arc by arc and says
**JOINING WI-FI**, or (logo dimmed) why it isn't working: **WRONG PASSWORD?**,
**WI-FI NOT FOUND**, **USE WPA2 WI-FI** (WPA3-only routers aren't supported), **WEAK WI-FI** or
**WI-FI FAILED**. If the saved network can't be joined within a minute of starting, setup opens
by itself and the setup page says what went wrong. Setup gives up after 10 minutes and retries
the saved network.

The ESP32-S3 only does 2.4 GHz Wi-Fi (open or WPA2; WPA3-only networks won't work).

Optional: to bake a network into the firmware instead, copy `wifi.env.example` to `wifi.env`
(git-ignored) and fill it in. A network saved from the phone always wins over it.

Once joined, the sky comes up with a spinner under the clock until the time and weather arrive;
until then the clock shows `--:--`. Time and weather refresh every 15 minutes (every 30 s
while failing); the serial log says what went wrong.

## Build & flash

From the browser (works from WSL without USB passthrough):

```sh
./scripts/flash.sh           # or: ./scripts/flash.sh debug
```

Then open <http://localhost:8000> in Chrome or Edge, click **Connect**, pick the board's COM port
and install. If the port doesn't show up, hold **BOOT**, tap **RESET**, release **BOOT** and retry.

Build only: `./scripts/build.sh [release|debug]`.

Or over USB from WSL, which also shows the serial log (recommended). Once, in Windows
PowerShell as administrator: `winget install usbipd`, `usbipd list` (note the CH343's BUSID),
`usbipd bind --busid <id>`. Then, each time the board is plugged in:
`usbipd attach --wsl --busid <id>`, and in WSL:

```sh
./scripts/run.sh             # build, flash with espflash, show the log (Ctrl+C to stop)
```

Neither way erases the saved Wi-Fi network.

## Where things come from

| What | Source |
| --- | --- |
| Time | SNTP from `pool.ntp.org` |
| Location | [ip-api.com](https://ip-api.com) from the internet connection's address, once per start (plain HTTP, no key). City-level; a VPN moves it. Until it works, Blacktown, NSW (`FALLBACK` in `src/location.rs`) |
| Weather, forecast, sunrise/sunset, time zone (incl. daylight saving) | [Open-Meteo](https://open-meteo.com) for that location, over plain HTTP, no API key |
| Events, bin nights, what's playing | Home Assistant's REST API (`/api/calendars/...`) on the local network, over plain HTTP with a token. Bin nights fall back to `src/bins.rs` without it |

## Layout

| Path | What |
| --- | --- |
| `ui/main.slint` | Sky, moon, the slides: clock, weather row, events line, bins; agenda; forecast; music |
| `ui/music/` | Music note on the now-playing slide |
| `ui/wifi/` | Wi-Fi logo frames for the connecting screen |
| `ui/font/`, `ui/weather/`, `ui/sky/`, `ui/bins/` | Clock/temperature/text glyphs, weather icons, moon, wheelie bins |
| `ui/web/pixel.ttf` | The pixel font as a web font, served on the setup page |
| `tools/gen_sprites.py` | Regenerates all the art: `python3 tools/gen_sprites.py` |
| `tools/host_tests/run.sh` | Tests the weather, location, bins, calendar, Wi-Fi record, DNS, setup-page and button logic on the PC |
| `scripts/build.sh`, `scripts/flash.sh` | Build, then serve the firmware for browser flashing (`web-flash`) |
| `src/bin/main.rs` | Board bring-up, Wi-Fi start, BOOT hold, UI updates, render loop |
| `src/net.rs` | Wi-Fi reconnect, SNTP, location lookup, weather fetch; calendar and media player from Home Assistant |
| `src/calendar.rs` | Parses Home Assistant's events; picks today's lines, the agenda and the bins |
| `src/media.rs` | Parses Home Assistant's media player: what's playing, how far in |
| `src/slides.rs` | Which slide is up, for how long, and the slide animation |
| `src/json.rs` | The few JSON helpers the parsers share |
| `src/location.rs` | Parses the location lookup |
| `src/weather.rs` | Parses the forecast; picks sky colours, icon, day/night |
| `src/bins.rs` | Built-in bin schedule: which bins are due tonight |
| `src/input.rs` | The BOOT 3-second hold |
| `src/setup/` | Setup hotspot: scan, DHCP, DNS, the setup web page |
| `src/credentials.rs`, `src/storage.rs` | Saved Wi-Fi network and its record in the `nvs` flash partition |
| `src/display.rs` | Streams Slint's scanlines to the GC9A01 |

## Tweaking

In `ui/main.slint`:

- `sky-colors`, `ground-colors`: the 5 sky bands and ground for night, dawn, day, dusk.
- `moon-lap-time`: how long the moon takes to go once round the screen.
- `stars`: star positions; `blink: true` ones twinkle.
- `clock-y`, `weather-y`, `text-row-y`: where the clock, weather row and events line sit.
- `event-max-width`: how wide the events line gets before it scrolls; `Marquee` has its speed and
  pauses.

To see the bins without waiting for Tuesday: `BINS_DEMO=1 ./scripts/flash.sh`.

Bin colours: `BIN_COLOURS` in `tools/gen_sprites.py` (then rerun it).

Dawn and dusk last 30 minutes either side of sunrise/sunset (`TWILIGHT` in `src/weather.rs`).

Backlight brightness is `BRIGHTNESS` in `src/bin/main.rs` (100% by default).

## Pinout

| Function | GPIO |
| --- | --- |
| LCD SCLK / MOSI / CS / DC / RST / BL | 10 / 11 / 9 / 8 / 12 / 40 |
| BOOT button (active low) | 0 |

The picture is flipped 180° (`ROTATE_180` in `src/bin/main.rs`) so it's upright when holding the
board normally.
