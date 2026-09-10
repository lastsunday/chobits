use alloc::{boxed::Box, vec, vec::Vec};
use embedded_hal::spi::SpiBus;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::i2c::master as i2c_master;
use esp_hal::peripherals::{FROM_CPU_INTR0, Peripherals};
use esp_hal::spi::master as spi_master;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use iot_core::drivers::board::Board as BoardTrait;
use iot_core::drivers::input::{BUTTON_SCAN_MS, ButtonScanner, PassThrough, PollEntry};

pub use crate::components::button::PullButton;
use crate::components::pca9557::Pca9557;
use crate::components::st7789::{SPI_FREQ_HZ, SPI_MODE, St7789};
pub use crate::virtual_components::DisplayLight;

const PCA9557_I2C_ADDR: u8 = 0x19;
const LCD_CS_BIT: u8 = 1 << 0;
const DVP_PWDN_BIT: u8 = 1 << 2;
const LCD_WIDTH: u16 = 240;
const LCD_HEIGHT: u16 = 320;

/// Board-level support for the LCKFB SZPI ESP32-S3 display board.
pub struct Board<'d> {
    light: Option<DisplayLight>,
    button: Option<PullButton<'d>>,
}

/// Completion of the chip-level wiring, handed to the application entry point.
pub type Startup<'a> = (
    Board<'a>,
    TimerGroup<'static, esp_hal::peripherals::TIMG0<'static>>,
    FROM_CPU_INTR0<'static>,
);

impl Board<'static> {
    #[allow(clippy::result_unit_err)]
    pub fn new(peripherals: Peripherals) -> Result<Startup<'static>, ()> {
        #[allow(non_snake_case)]
        let Peripherals {
            I2C0,
            SPI3,
            GPIO1,
            GPIO2,
            GPIO39,
            GPIO40,
            GPIO41,
            GPIO42,
            GPIO0,
            TIMG0,
            FROM_CPU_INTR0,
            ..
        } = peripherals;

        let i2c = i2c_master::I2c::new(I2C0, i2c_master::Config::default())
            .map_err(|_| ())?
            .with_sda(GPIO1)
            .with_scl(GPIO2);
        let mut pca9557 = Pca9557::new(i2c, PCA9557_I2C_ADDR);
        pca9557
            .init(LCD_CS_BIT | DVP_PWDN_BIT, 0xf8)
            .map_err(|_| ())?;

        // Configure the SPI/GPIO matrix while CS stays high: the IO_MUX remap
        // glitches during Spi::new/Output::new must not reach the panel. The
        // working legacy driver also asserted CS low only after all pin setup.
        let mut block_spi = spi_master::Spi::new(
            SPI3,
            spi_master::Config::default()
                .with_frequency(Rate::from_hz(SPI_FREQ_HZ))
                .with_mode(SPI_MODE),
        )
        .map_err(|_| ())?
        .with_sck(GPIO41)
        .with_mosi(GPIO40);

        let dc = Output::new(GPIO39, Level::Low, OutputConfig::default());
        let bl = Output::new(GPIO42, Level::Low, OutputConfig::default());

        // Consume the first SPI transfer while CS is still high: remapping the
        // IO_MUX for these pins glitches on the very first transfer (esp-idf
        // #15703), and the working legacy driver busied the bus in this window
        // before pulling CS low. The panel ignores the byte since CS is high.
        SpiBus::write(&mut block_spi, &[0x01]).map_err(|_| ())?;

        pca9557.set_output(DVP_PWDN_BIT).map_err(|_| ())?;

        let panel = St7789::new(block_spi, dc, LCD_WIDTH, LCD_HEIGHT)?;
        let light = DisplayLight::new(panel, bl);
        let button = PullButton::new(GPIO0);
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
    type Light = DisplayLight;

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
