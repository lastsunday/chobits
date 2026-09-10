use alloc::boxed::Box;
use alloc::vec::Vec;
use esp_hal::Blocking;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, Output, OutputConfig, Pull};
use esp_hal::i2c::master as i2c_master;
use esp_hal::peripherals::FROM_CPU_INTR0;
use esp_hal::peripherals::Peripherals;
use esp_hal::spi::Mode;
use esp_hal::spi::master as spi_master;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use iot_core::drivers::board::Board as BoardTrait;
use iot_core::drivers::input::{BUTTON_SCAN_MS, Button, ButtonScanner, PassThrough, PollEntry};
use iot_core::drivers::light::{Rgb, RgbLight};

const LCD_WIDTH: u16 = 240;
const LCD_HEIGHT: u16 = 320;
const SPI_FREQ_HZ: u32 = 80_000_000;
const REPAINT_STEP: u8 = 12;
const FRAME_BYTES: usize = LCD_WIDTH as usize * LCD_HEIGHT as usize * 2;
const PCA9557_I2C_ADDR: u8 = 0x19;
const PCA9557_REG_OUTPUT: u8 = 0x01;
const PCA9557_REG_CONFIG: u8 = 0x03;
const LCD_CS_BIT: u8 = 1 << 0;
const DVP_PWDN_BIT: u8 = 1 << 2;

fn pca9557_init(i2c: &mut i2c_master::I2c<'_, Blocking>) -> Result<(), i2c_master::Error> {
    i2c.write(
        PCA9557_I2C_ADDR,
        &[PCA9557_REG_OUTPUT, LCD_CS_BIT | DVP_PWDN_BIT],
    )?;
    i2c.write(PCA9557_I2C_ADDR, &[PCA9557_REG_CONFIG, 0xf8])?;
    Ok(())
}

fn pca9557_assert_cs_low(i2c: &mut i2c_master::I2c<'_, Blocking>) -> Result<(), i2c_master::Error> {
    i2c.write(PCA9557_I2C_ADDR, &[PCA9557_REG_OUTPUT, DVP_PWDN_BIT])?;
    Ok(())
}

pub struct DisplayLight {
    spi: spi_master::Spi<'static, Blocking>,
    dc: Output<'static>,
    #[allow(dead_code)]
    bl: Output<'static>,
    frame: Vec<u8>,
    frame_color: Option<Rgb>,
}

impl DisplayLight {
    fn write_cmd(spi: &mut spi_master::Spi<'static, Blocking>, cmd: u8, dc: &mut Output<'static>) {
        dc.set_low();
        spi.write(&[cmd]).ok();
    }

    fn write_data(
        spi: &mut spi_master::Spi<'static, Blocking>,
        data: &[u8],
        dc: &mut Output<'static>,
    ) {
        dc.set_high();
        spi.write(data).ok();
    }
}

impl RgbLight for DisplayLight {
    fn repaint_step(&self) -> u8 {
        REPAINT_STEP
    }

    fn set_rgb(&mut self, color: Rgb) {
        let r = color.0 as u16;
        let g = color.1 as u16;
        let b = color.2 as u16;
        let rgb565 = ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3);
        let hi = (rgb565 >> 8) as u8;
        let lo = rgb565 as u8;

        if self.frame_color != Some(color) {
            self.frame_color = Some(color);
            self.frame.resize(FRAME_BYTES, 0);
            for px in self.frame.chunks_exact_mut(2) {
                px[0] = hi;
                px[1] = lo;
            }
        }

        let x_end = LCD_WIDTH - 1;
        let y_end = LCD_HEIGHT - 1;
        Self::write_cmd(&mut self.spi, 0x2A, &mut self.dc);
        Self::write_data(
            &mut self.spi,
            &[0x00, 0x00, (x_end >> 8) as u8, x_end as u8],
            &mut self.dc,
        );
        Self::write_cmd(&mut self.spi, 0x2B, &mut self.dc);
        Self::write_data(
            &mut self.spi,
            &[0x00, 0x00, (y_end >> 8) as u8, y_end as u8],
            &mut self.dc,
        );
        Self::write_cmd(&mut self.spi, 0x2C, &mut self.dc);

        self.dc.set_high();
        self.spi.write(&self.frame).ok();
    }
}

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

pub struct Board<'d> {
    light: Option<DisplayLight>,
    button: Option<BootButton<'d>>,
}

pub type Startup<'a> = (
    Board<'a>,
    TimerGroup<'static, esp_hal::peripherals::TIMG0<'static>>,
    FROM_CPU_INTR0<'static>,
);

impl Board<'static> {
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

        let mut i2c = i2c_master::I2c::new(I2C0, i2c_master::Config::default())
            .map_err(|_| ())?
            .with_sda(GPIO1)
            .with_scl(GPIO2);
        pca9557_init(&mut i2c).map_err(|_| ())?;

        let mut spi = spi_master::Spi::new(
            SPI3,
            spi_master::Config::default()
                .with_frequency(Rate::from_hz(SPI_FREQ_HZ))
                .with_mode(Mode::_2),
        )
        .map_err(|_| ())?
        .with_sck(GPIO41)
        .with_mosi(GPIO40);

        let mut dc = Output::new(GPIO39, esp_hal::gpio::Level::Low, OutputConfig::default());
        let bl = Output::new(GPIO42, esp_hal::gpio::Level::Low, OutputConfig::default());

        DisplayLight::write_cmd(&mut spi, 0x01, &mut dc);
        pca9557_assert_cs_low(&mut i2c).map_err(|_| ())?;
        DisplayLight::write_cmd(&mut spi, 0x11, &mut dc);
        Delay::new().delay_millis(150);
        DisplayLight::write_cmd(&mut spi, 0x36, &mut dc);
        DisplayLight::write_data(&mut spi, &[0x00], &mut dc);
        DisplayLight::write_cmd(&mut spi, 0x3A, &mut dc);
        DisplayLight::write_data(&mut spi, &[0x55], &mut dc);
        DisplayLight::write_cmd(&mut spi, 0xB0, &mut dc);
        DisplayLight::write_data(&mut spi, &[0x00, 0xF0], &mut dc);
        DisplayLight::write_cmd(&mut spi, 0x21, &mut dc);
        DisplayLight::write_cmd(&mut spi, 0x29, &mut dc);

        let light = DisplayLight {
            spi,
            dc,
            bl,
            frame: Vec::new(),
            frame_color: None,
        };
        let button = BootButton::new(GPIO0);
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
    fn take_input(&mut self) -> Option<alloc::vec::Vec<PollEntry>> {
        self.button.take().map(|button| {
            alloc::vec![PollEntry::new(
                Box::new(ButtonScanner::new(button)),
                Box::new(PassThrough),
                BUTTON_SCAN_MS,
            )]
        })
    }
}
