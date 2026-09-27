#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use embassy_executor::Spawner;
use embassy_net::{Stack, StackResources};
use embassy_time::Timer;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{DriveMode, Input, InputConfig, Level, Output, OutputConfig, Pull},
    ledc::{
        LSGlobalClkSource, Ledc, LowSpeed,
        channel::{self, ChannelIFace},
        timer::{self, TimerIFace},
    },
    main,
    rng::Rng,
    spi::{Mode, master::Spi},
    time::{Duration, Instant, Rate},
    timer::timg::TimerGroup,
};
use esp_radio::wifi::{
    AuthenticationMethodConfig, Config as WifiConfig, ControllerConfig, WifiController,
    sta::StationConfig,
};
use esp_storage::FlashStorage;
use gc9a01::{
    Gc9a01, SPIDisplayInterface,
    mode::DisplayConfiguration,
    prelude::{DisplayResolution240x240, DisplayRotation},
};
use radix_home::{
    bins, credentials::Credentials, display::DrawBuffer, input::HoldButton, join::JoinProblem, net,
    setup, storage, weather,
};
use slint::{
    ModelRc, VecModel,
    platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel},
};
use static_cell::StaticCell;

slint::include_modules!();

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

/// Optional preset network from `wifi.env`, used until one is saved from the setup page.
const PRESET_SSID: Option<&str> = option_env!("WIFI_SSID");
const PRESET_PASSWORD: Option<&str> = option_env!("WIFI_PASSWORD");

const SCREEN_SIZE: u32 = 240;

/// Flip the picture 180° (upright when holding the board normally).
const ROTATE_180: bool = true;

/// How often to sample the BOOT button.
const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// How often to push clock, weather and bins into the UI.
const UI_REFRESH_INTERVAL: Duration = Duration::from_millis(250);

/// Used for the clock until the first forecast says otherwise (AEST, no daylight saving).
const FALLBACK_UTC_OFFSET_S: i32 = 10 * 60 * 60;

/// If the saved network hasn't given us an address this long after boot, open setup instead.
const CONNECT_GIVE_UP: Duration = Duration::from_secs(60);
/// Setup mode gives up and retries the saved network after this long (if there is one).
const SETUP_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Time to show "SAVED!" (and let the page reach the phone) before restarting.
const SAVED_RESTART_DELAY: Duration = Duration::from_secs(2);

/// Backlight brightness (percent). The screen never dims.
const BRIGHTNESS: u8 = 100;

/// `BINS_DEMO=1 ./scripts/flash.sh`: show every bin all the time (even before the clock has
/// synced), to see how they look.
const BINS_DEMO: bool = option_env!("BINS_DEMO").is_some();

/// Survives a software reset: tells the next boot to go straight into setup mode.
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut SETUP_REQUEST: u32 = 0;
const SETUP_REQUEST_MAGIC: u32 = 0x5345_5455; // "SETU"

fn take_setup_request() -> bool {
    // SAFETY: single-threaded access before any tasks run.
    unsafe {
        let flag = core::ptr::addr_of_mut!(SETUP_REQUEST);
        let requested = flag.read_volatile() == SETUP_REQUEST_MAGIC;
        flag.write_volatile(0);
        requested
    }
}

/// Survives the restart into setup: why the saved network couldn't be joined.
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut JOIN_PROBLEM: u32 = 0;

fn take_join_problem() -> Option<JoinProblem> {
    // SAFETY: single-threaded access before any tasks run.
    unsafe {
        let slot = core::ptr::addr_of_mut!(JOIN_PROBLEM);
        let code = slot.read_volatile();
        slot.write_volatile(0);
        JoinProblem::from_code(code)
    }
}

/// Opens setup, telling it why the saved network didn't work.
fn restart_into_setup_after(problem: JoinProblem) -> ! {
    // SAFETY: we reset immediately after this write.
    unsafe { core::ptr::addr_of_mut!(JOIN_PROBLEM).write_volatile(problem as u32) };
    restart(true)
}

fn restart(into_setup: bool) -> ! {
    // SAFETY: we reset immediately after this write.
    unsafe {
        let value = if into_setup { SETUP_REQUEST_MAGIC } else { 0 };
        core::ptr::addr_of_mut!(SETUP_REQUEST).write_volatile(value);
    }
    esp_hal::system::software_reset()
}

fn preset_credentials() -> Option<Credentials> {
    Credentials::new(PRESET_SSID?, PRESET_PASSWORD.unwrap_or("")).ok()
}

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger(log::LevelFilter::Info);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // Wi-Fi alone wants ~100 KiB of heap; Slint takes the rest.
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);
    esp_alloc::heap_allocator!(size: 112 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

    let mut delay = Delay::new();

    // ---- Display: GC9A01 over SPI ----
    // ESP32-S3-LCD-1.28(-B) pinout (differs from the touch board: RST and backlight moved).
    let sck = peripherals.GPIO10; // LCD_CLK
    let mosi = peripherals.GPIO11; // LCD_DIN
    let cs = peripherals.GPIO9; // LCD_CS
    let dc = peripherals.GPIO8; // LCD_DC
    let lcd_reset = peripherals.GPIO12; // LCD_RST
    let backlight = peripherals.GPIO40; // LCD_BL

    let mut lcd_reset = Output::new(lcd_reset, Level::Low, OutputConfig::default());
    let cs = Output::new(cs, Level::High, OutputConfig::default());
    let dc = Output::new(dc, Level::Low, OutputConfig::default());

    // Backlight via PWM so its brightness is adjustable (20 kHz: no visible flicker or audible whine).
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
    let mut backlight_timer = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    backlight_timer
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty10Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(20),
        })
        .unwrap();
    let mut backlight = ledc.channel(channel::Number::Channel0, backlight);
    backlight
        .configure(channel::config::Config {
            timer: &backlight_timer,
            duty_pct: BRIGHTNESS,
            drive_mode: DriveMode::PushPull,
        })
        .unwrap();

    let spi = Spi::new(
        peripherals.SPI2,
        esp_hal::spi::master::Config::default()
            .with_mode(Mode::_0)
            .with_frequency(Rate::from_mhz(40)),
    )
    .unwrap()
    .with_sck(sck)
    .with_mosi(mosi);
    let spi = ExclusiveDevice::new_no_delay(spi, cs).unwrap();

    let rotation = if ROTATE_180 {
        DisplayRotation::Rotate180
    } else {
        DisplayRotation::Rotate0
    };
    let mut display = Gc9a01::new(
        SPIDisplayInterface::new(spi, dc),
        DisplayResolution240x240,
        rotation,
    );
    display.reset(&mut lcd_reset, &mut delay).unwrap();
    display.init(&mut delay).unwrap();
    display.clear_fit().unwrap();
    log::info!("display ready");

    // ---- BOOT button: hold to open (or leave) Wi-Fi setup ----
    // Active-low; `HoldButton` ignores a press that was already held at boot.
    let boot_button = Input::new(
        peripherals.GPIO0,
        InputConfig::default().with_pull(Pull::Up),
    );
    let mut button = HoldButton::new();

    // ---- Wi-Fi: normal (join the saved network) or setup (be a hotspot) ----
    let mut flash = FlashStorage::new(peripherals.FLASH);
    let saved = storage::load(&mut flash);
    let credentials = saved.or_else(preset_credentials);
    let setup_requested = take_setup_request();
    let last_join_problem = take_join_problem();
    let has_credentials = credentials.is_some();
    let station = credentials.filter(|_| !setup_requested);

    let wifi_config = match &station {
        Some(credentials) => {
            let authentication = if credentials.is_open() {
                AuthenticationMethodConfig::Open
            } else {
                AuthenticationMethodConfig::Wpa2Personal(
                    credentials.password.as_str().try_into().unwrap(),
                )
            };
            WifiConfig::Station(
                StationConfig::default()
                    .with_ssid(credentials.ssid.as_str().try_into().unwrap())
                    .with_authentication(authentication),
            )
        }
        None => setup::wifi_config(),
    };
    let controller = WifiController::new(
        peripherals.WIFI,
        ControllerConfig::default().with_initial_config(wifi_config),
    )
    .unwrap();

    let rng = Rng::new();
    let seed = (u64::from(rng.random()) << 32) | u64::from(rng.random());
    static STACK_RESOURCES: StaticCell<StackResources<6>> = StaticCell::new();
    let resources = STACK_RESOURCES.init(StackResources::new());

    let setup_mode = station.is_none();
    let stack: Stack<'static> = match &station {
        Some(credentials) => {
            let (stack, runner) = embassy_net::new(
                esp_radio::wifi::Interface::station(),
                embassy_net::Config::dhcpv4(Default::default()),
                resources,
                seed,
            );
            spawner.spawn(net::wifi_task(controller).unwrap());
            spawner.spawn(net::net_task(runner).unwrap());
            spawner.spawn(net::sync_task(stack).unwrap());
            log::info!("joining wifi {:?}", credentials.ssid);
            stack
        }
        None => {
            let (stack, runner) = embassy_net::new(
                esp_radio::wifi::Interface::access_point(),
                setup::net_config(),
                resources,
                seed,
            );
            setup::give_flash(flash);
            if let Some(problem) = last_join_problem {
                setup::set_notice(problem.page_text());
            }
            spawner.spawn(setup::wifi_task(controller).unwrap());
            spawner.spawn(net::net_task(runner).unwrap());
            spawner.spawn(setup::dhcp_task(stack).unwrap());
            spawner.spawn(setup::dns_task(stack).unwrap());
            spawner.spawn(setup::http_task(stack).unwrap());
            spawner.spawn(setup::http_task(stack).unwrap());
            log::info!("setup mode (saved network: {has_credentials})");
            stack
        }
    };

    // ---- Slint ----
    // The panel keeps what we drew, so Slint only needs to redraw what changed.
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    window.set_size(slint::PhysicalSize::new(SCREEN_SIZE, SCREEN_SIZE));
    slint::platform::set_platform(Box::new(EspBackend {
        window: window.clone(),
    }))
    .unwrap();

    let app = AppWindow::new().unwrap();
    app.set_setup_mode(setup_mode);
    app.set_bin_label(pixel_text("BIN NIGHT"));
    if BINS_DEMO {
        show_bins(&app, [true; bins::BINS.len()]);
    }
    if setup_mode {
        show_setup_text(
            &app,
            ["WI-FI SETUP", "JOIN", setup::AP_SSID, "ON YOUR PHONE"],
            "192.168.4.1",
        );
    }

    let booted = Instant::now();
    let mut ever_connected = false;
    let mut restart_at: Option<Instant> = None;

    let mut line_buffer = [Rgb565Pixel(0); SCREEN_SIZE as usize];
    let mut next_input_poll = Instant::now();
    let mut next_ui_refresh = Instant::now();
    let mut shown_bins = [false; bins::BINS.len()];
    let mut shown_status = "";

    loop {
        slint::platform::update_timers_and_animations();

        if Instant::now() >= next_ui_refresh {
            next_ui_refresh = Instant::now() + UI_REFRESH_INTERVAL;

            if setup_mode {
                if restart_at.is_none() && setup::state() == setup::State::Saved {
                    show_setup_text(&app, ["SAVED!", "", "RESTARTING", ""], "");
                    restart_at = Some(Instant::now() + SAVED_RESTART_DELAY);
                }
                if has_credentials && booted.elapsed() > SETUP_TIMEOUT && restart_at.is_none() {
                    log::info!("setup timed out, retrying the saved network");
                    restart(false);
                }
            } else {
                refresh_clock_weather_and_bins(&app, &mut shown_bins);
                ever_connected |= stack.is_config_up();
                let join_problem = net::snapshot().join_problem;
                if !ever_connected && booted.elapsed() > CONNECT_GIVE_UP {
                    let problem = join_problem.unwrap_or(JoinProblem::Other);
                    log::warn!("couldn't join wifi ({problem:?}), opening setup");
                    restart_into_setup_after(problem);
                }

                // Until the first connection: what's happening, in words.
                let status = match join_problem {
                    _ if ever_connected => "",
                    Some(problem) => problem.screen_text(),
                    None => "JOINING WI-FI",
                };
                if status != shown_status {
                    shown_status = status;
                    app.set_status(pixel_text(status));
                }
            }

            if restart_at.is_some_and(|at| Instant::now() >= at) {
                restart(false);
            }
        }

        if Instant::now() >= next_input_poll {
            next_input_poll = Instant::now() + INPUT_POLL_INTERVAL;
            let now_ms = Instant::now().duration_since_epoch().as_millis();
            if button.update(boot_button.is_low(), now_ms) {
                restart(!setup_mode);
            }
        }

        window.draw_if_needed(|renderer| {
            renderer.render_by_line(DrawBuffer {
                display: &mut display,
                line_buffer: &mut line_buffer,
            });
        });

        // Let the Wi-Fi / network tasks run.
        Timer::after_millis(5).await;
    }
}

/// Puts the bins marked `true` on the grass.
fn show_bins(app: &AppWindow, due: [bool; bins::BINS.len()]) {
    let indices: Vec<i32> = (0..due.len() as i32).filter(|&i| due[i as usize]).collect();
    app.set_bins(ModelRc::new(VecModel::from(indices)));
}

fn pixel_text(text: &str) -> ModelRc<i32> {
    ModelRc::new(VecModel::from(setup::glyphs(text)))
}

fn show_setup_text(app: &AppWindow, lines: [&str; 4], footer: &str) {
    app.set_setup_line_1(pixel_text(lines[0]));
    app.set_setup_line_2(pixel_text(lines[1]));
    app.set_setup_line_3(pixel_text(lines[2]));
    app.set_setup_line_4(pixel_text(lines[3]));
    app.set_setup_footer(pixel_text(footer));
}

/// Pushes the latest time, sky and weather from the network tasks into the UI, and the bins
/// when they change (`shown` is what's on screen).
fn refresh_clock_weather_and_bins(app: &AppWindow, shown: &mut [bool; bins::BINS.len()]) {
    let snapshot = net::snapshot();
    let Some(now) = snapshot.unix_now() else {
        app.set_time_valid(false);
        return;
    };

    let utc_offset = snapshot
        .weather
        .map_or(FALLBACK_UTC_OFFSET_S, |w| w.utc_offset_s);
    let (hours, minutes, seconds) = weather::local_hms(now, utc_offset);
    app.set_time_valid(true);
    app.set_hours(hours);
    app.set_minutes(minutes);
    app.set_colon_on(seconds % 2 == 0);

    let due = bins::due(now, utc_offset);
    if !BINS_DEMO && due != *shown {
        *shown = due;
        show_bins(app, due);
    }

    match snapshot.weather {
        Some(forecast) => {
            app.set_sky_phase(forecast.sky(now) as i32);
            app.set_moon_visible(forecast.is_night(now));
            app.set_weather_valid(true);
            app.set_weather_icon(forecast.icon(now));
            app.set_temperature(forecast.temperature_rounded());
        }
        None => {
            // No forecast yet: guess day/night from the clock.
            let night = !(6..19).contains(&hours);
            let sky = if night {
                weather::Sky::Night
            } else {
                weather::Sky::Day
            };
            app.set_sky_phase(sky as i32);
            app.set_moon_visible(night);
        }
    }
}

struct EspBackend {
    window: Rc<MinimalSoftwareWindow>,
}

impl slint::platform::Platform for EspBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    // Real elapsed time - Slint's Timer and animations are driven from this.
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_micros(Instant::now().duration_since_epoch().as_micros())
    }
}
