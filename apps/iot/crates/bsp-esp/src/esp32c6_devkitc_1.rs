use alloc::{boxed::Box, vec, vec::Vec};
use esp_hal::Blocking;
use esp_hal::gpio::interconnect::PeripheralOutput;
use esp_hal::gpio::{Input, InputConfig, Pull};
use esp_hal::peripherals::{FROM_CPU_INTR0, Peripherals};
use esp_hal::rmt::{Rmt, TxChannelCreator};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal_smartled::{RmtSmartLeds, WS2812_TIMING, buffer_size, color_order};
use iot_core::drivers::board::Board as BoardTrait;
use iot_core::drivers::input::{BUTTON_SCAN_MS, Button, ButtonScanner, PassThrough, PollEntry};
use iot_core::drivers::light::{Rgb, RgbLight};
use smart_leds::{RGB8, SmartLedsWrite};

/// How many RGB blocks are on the board (DevKitC-1 has a single addressable LED).
const LEDS: usize = 1;

/// RMT driver configuration, in blocking mode.
const RMT_FREQ_HZ: u32 = 80_000_000;

type Driver<'d> = RmtSmartLeds<'d, { buffer_size::<RGB8>(LEDS) }, Blocking, RGB8, color_order::Grb>;

/// The WS2812 addressable RGB LED found on the ESP32-C6-DevKitC-1 (GPIO8).
pub struct Ws2812RgbLed<'d> {
    driver: Driver<'d>,
}

impl<'d> Ws2812RgbLed<'d> {
    pub fn new<Ch, P>(channel: Ch, pin: P) -> Result<Self, esp_hal_smartled::Error>
    where
        Ch: TxChannelCreator<'d, Blocking>,
        P: PeripheralOutput<'d>,
    {
        let clock = Rate::from_hz(RMT_FREQ_HZ);
        let driver = Driver::new_with_memsize(WS2812_TIMING, channel, pin, 2, clock)?;
        Ok(Self { driver })
    }
}

impl RgbLight for Ws2812RgbLed<'_> {
    fn set_rgb(&mut self, color: Rgb) {
        let data = RGB8::new(color.0, color.1, color.2);
        self.driver
            .write([data])
            .expect("failed to write WS2812 LED");
    }
}

/// The onboard boot button of the ESP32-C6-DevKitC-1 (GPIO9, active-low with
/// an internal pull-up).
pub struct BootButton<'d> {
    input: Input<'d>,
}

impl BootButton<'static> {
    fn new(pin: impl esp_hal::gpio::InputPin + 'static) -> Self {
        let config = InputConfig::default().with_pull(Pull::Up);
        Self {
            input: Input::new(pin, config),
        }
    }
}

impl Button for BootButton<'_> {
    fn is_pressed(&self) -> bool {
        self.input.is_low()
    }
}

/// Board-level support for the ESP32-C6-DevKitC-1.
pub struct Board<'d> {
    /// The onboard WS2812 addressable RGB LED.
    pub light: Option<Ws2812RgbLed<'d>>,
    /// The onboard boot button.
    pub button: Option<BootButton<'d>>,
}

/// Completion of the chip-level wiring, handed to the application entry point.
pub type Startup<'a> = (
    Board<'a>,
    TimerGroup<'static, esp_hal::peripherals::TIMG0<'static>>,
    FROM_CPU_INTR0<'static>,
);

impl Board<'static> {
    /// Wire up the board from the chip's remaining `Peripherals`.
    ///
    /// The wiring (LED on GPIO8, boot button on GPIO9, timer group, interrupt
    /// source) is fixed inside this module: pin changes only ever touch this
    /// board implementation and are invisible to the application.
    pub fn new(peripherals: Peripherals) -> Result<Startup<'static>, esp_hal_smartled::Error> {
        #[allow(non_snake_case)]
        let Peripherals {
            RMT,
            GPIO8,
            GPIO9,
            TIMG0,
            FROM_CPU_INTR0,
            ..
        } = peripherals;

        let rmt = Rmt::new(RMT, Rate::from_hz(RMT_FREQ_HZ)).expect("failed to initialize RMT");
        let light = Ws2812RgbLed::new(rmt.channel0, GPIO8)?;
        let button = BootButton::new(GPIO9);
        let timg0 = TimerGroup::new(TIMG0);

        Ok((
            Self {
                light: Some(light),
                button: Some(button),
            },
            timg0,
            FROM_CPU_INTR0,
        ))
    }
}

impl BoardTrait for Board<'static> {}

impl iot_core::drivers::board::HasLight for Board<'static> {
    type Light = Ws2812RgbLed<'static>;

    fn take_light(&mut self) -> Option<Self::Light> {
        self.light.take()
    }
}

impl iot_core::drivers::board::HasInput for Board<'static> {
    fn take_input(&mut self) -> Option<Vec<PollEntry>> {
        self.button.take().map(|button| {
            vec![PollEntry::new(
                Box::new(ButtonScanner::new(button)),
                Box::new(PassThrough),
                BUTTON_SCAN_MS,
            )]
        })
    }
}
