use esp_hal::Blocking;
use esp_hal::i2c::master as i2c_master;

pub const PCA9557_REG_OUTPUT: u8 = 0x01;
pub const PCA9557_REG_CONFIG: u8 = 0x03;

/// PCA9557 8-bit I2C GPIO expander.
pub struct Pca9557<'d> {
    i2c: i2c_master::I2c<'d, Blocking>,
    addr: u8,
}

impl<'d> Pca9557<'d> {
    pub fn new(i2c: i2c_master::I2c<'d, Blocking>, addr: u8) -> Self {
        Self { i2c, addr }
    }

    /// Bring the expander up: set the output register before releasing the
    /// pins (so the CS line stays high during board bring-up) then mark the
    /// pins as outputs.
    pub fn init(&mut self, output: u8, config: u8) -> Result<(), i2c_master::Error> {
        self.i2c.write(self.addr, &[PCA9557_REG_OUTPUT, output])?;
        self.i2c.write(self.addr, &[PCA9557_REG_CONFIG, config])?;
        Ok(())
    }

    /// Rewrite the output register, e.g. to drop the LCD chip-select bit.
    pub fn set_output(&mut self, output: u8) -> Result<(), i2c_master::Error> {
        self.i2c.write(self.addr, &[PCA9557_REG_OUTPUT, output])?;
        Ok(())
    }
}
