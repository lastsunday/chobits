use alloc::boxed::Box;
use alloc::vec::Vec;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{ErrorType, OutputPin};
use embedded_hal::spi::SpiBus;
use esp_hal::Blocking;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
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
use mipidsi::Builder;
use mipidsi::interface::{Interface, InterfaceKind};
use mipidsi::models::ST7789;
use mipidsi::options::ColorInversion;

const LCD_WIDTH: u16 = 240;
const LCD_HEIGHT: u16 = 320;
const SPI_FREQ_HZ: u32 = 80_000_000;
const REPAINT_STEP: u8 = 12;
const SPI_SCRATCH_BYTES: usize = 8192;
const FRAME_BYTES: usize = LCD_WIDTH as usize * LCD_HEIGHT as usize * 2;
const PCA9557_I2C_ADDR: u8 = 0x19;
const PCA9557_REG_OUTPUT: u8 = 0x01;
const PCA9557_REG_CONFIG: u8 = 0x03;
const LCD_CS_BIT: u8 = 1 << 0;
const DVP_PWDN_BIT: u8 = 1 << 2;

/// No-op reset driver: the panel keeps RST unconnected, but `Builder::init`
/// only suppresses its software reset when a pin is provided. Driving a
/// no-op pin expresses "do not soft-reset" without touching a real GPIO.
#[derive(Clone, Copy)]
struct DummyReset;

impl ErrorType for DummyReset {
    type Error = core::convert::Infallible;
}

impl OutputPin for DummyReset {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// No-op delay source: mipidsi parks ~300ms with CS low before its first byte
/// and another ~120ms after DISPON; the working legacy driver started the first
/// command immediately after CS assert and the first frame right after DISPON.
#[derive(Clone, Copy)]
struct NoDelay;

impl DelayNs for NoDelay {
    fn delay_ns(&mut self, _ns: u32) {}
}

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

/// 4-wire SPI transport for mipidsi. esp-hal's blocking `Spi` is a `SpiBus`,
/// not a shared-bus `SpiDevice`, so we adapt it directly to mipidsi's
/// `Interface` trait to avoid pulling in a bus crate for a single client.
struct Spi4 {
    spi: spi_master::Spi<'static, Blocking>,
    dc: Output<'static>,
    buffer: &'static mut [u8],
}

impl Spi4 {
    fn push_word<const N: usize>(
        &mut self,
        word: &[u8; N],
        used: &mut usize,
    ) -> Result<(), esp_hal::spi::Error> {
        if *used + N > self.buffer.len() {
            SpiBus::write(&mut self.spi, &self.buffer[..*used])?;
            *used = 0;
        }
        self.buffer[*used..*used + N].copy_from_slice(word);
        *used += N;
        Ok(())
    }
}

impl Spi4 {
    fn write_data(&mut self, data: &[u8]) -> Result<(), esp_hal::spi::Error> {
        self.dc.set_high();
        SpiBus::write(&mut self.spi, data)
    }

    fn emit_command(&mut self, command: u8, args: &[u8]) -> Result<(), esp_hal::spi::Error> {
        self.dc.set_low();
        SpiBus::write(&mut self.spi, &[command])?;
        // Mirror the legacy driver's DC semantics: the line only goes high for
        // parameter bytes and stays low after argument-less commands.
        if !args.is_empty() {
            self.dc.set_high();
            SpiBus::write(&mut self.spi, args)?;
        }
        Ok(())
    }
}

impl Interface for Spi4 {
    type Word = u8;
    type Error = esp_hal::spi::Error;

    const KIND: InterfaceKind = InterfaceKind::Serial4Line;

    fn send_command(&mut self, command: u8, args: &[u8]) -> Result<(), Self::Error> {
        // Translate mipidsi's init stream (11→36→21→3A→13→29 at 10ms cadence)
        // into the exact legacy bring-up sequence that works on this panel
        // batch: sleep out must settle 150ms before any further command and
        // NORON/duplicate COLMOD must not reach the panel.
        match command {
            0x11 => {
                self.emit_command(command, args)?;
                Delay::new().delay_ms(150);
            }
            0x36 => {
                self.emit_command(0x36, &[0x00])?;
                self.emit_command(0x3A, &[0x55])?;
                self.emit_command(0xB0, &[0x00, 0xF0])?;
            }
            0x3A | 0x13 => {}
            _ => self.emit_command(command, args)?,
        }
        Ok(())
    }

    fn send_pixels<const N: usize>(
        &mut self,
        pixels: impl IntoIterator<Item = [Self::Word; N]>,
    ) -> Result<(), Self::Error> {
        self.dc.set_high();
        let mut used = 0usize;
        for word in pixels {
            self.push_word(&word, &mut used)?;
        }
        if used > 0 {
            SpiBus::write(&mut self.spi, &self.buffer[..used])?;
        }
        Ok(())
    }

    fn send_repeated_pixel<const N: usize>(
        &mut self,
        pixel: [Self::Word; N],
        count: u32,
    ) -> Result<(), Self::Error> {
        self.dc.set_high();
        let mut used = 0usize;
        for _ in 0..count {
            self.push_word(&pixel, &mut used)?;
        }
        if used > 0 {
            SpiBus::write(&mut self.spi, &self.buffer[..used])?;
        }
        Ok(())
    }
}

pub struct DisplayLight {
    spi: Spi4,
    #[allow(dead_code)]
    bl: Output<'static>,
    frame: Vec<u8>,
    frame_color: Option<Rgb>,
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
        self.spi.emit_command(0x2A, &[]).unwrap();
        self.spi
            .write_data(&[0x00, 0x00, (x_end >> 8) as u8, x_end as u8])
            .unwrap();
        self.spi.emit_command(0x2B, &[]).unwrap();
        self.spi
            .write_data(&[0x00, 0x00, (y_end >> 8) as u8, y_end as u8])
            .unwrap();
        self.spi.emit_command(0x2C, &[]).unwrap();
        self.spi.write_data(&self.frame).unwrap();
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

        let mut i2c = i2c_master::I2c::new(I2C0, i2c_master::Config::default())
            .map_err(|_| ())?
            .with_sda(GPIO1)
            .with_scl(GPIO2);
        pca9557_init(&mut i2c).map_err(|_| ())?;

        // Configure the SPI/GPIO matrix while CS stays high: the IO_MUX remap
        // glitches during Spi::new/Output::new must not reach the panel. The
        // working legacy driver also asserted CS low only after all pin setup.
        let mut block_spi = spi_master::Spi::new(
            SPI3,
            spi_master::Config::default()
                .with_frequency(Rate::from_hz(SPI_FREQ_HZ))
                .with_mode(Mode::_2),
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

        pca9557_assert_cs_low(&mut i2c).map_err(|_| ())?;

        let scratch: &'static mut [u8] =
            Box::leak(alloc::vec![0u8; SPI_SCRATCH_BYTES].into_boxed_slice());
        let iface = Spi4 {
            spi: block_spi,
            dc,
            buffer: scratch,
        };

        // RAMCTRL/COLMOD/MADCTL are emitted by the init shim at the same spot
        // the working legacy driver used; the panel must receive SLPOUT first.
        let display = Builder::new(ST7789, iface)
            .display_size(240, 320)
            .invert_colors(ColorInversion::Inverted)
            .reset_pin(DummyReset)
            .init(&mut NoDelay)
            .map_err(|_| ())?;
        let (iface, _, _) = display.release();

        let light = DisplayLight {
            spi: iface,
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
