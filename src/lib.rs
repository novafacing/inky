use anyhow::{Context, Error, Result};
use num::{FromPrimitive as ConvertFromPrimitive, ToPrimitive as ConvertToPrimitive};
use num_derive::{FromPrimitive, ToPrimitive};
use rppal::i2c::I2c;

// Inky devices all use Bus 1
pub const INKY_BUS: u8 = 1;

#[derive(Debug)]
struct PascalString {
    capacity: u8,
    data: Vec<u8>,
}

impl PascalString {
    fn with_capacity(capacity: u8) -> Self {
        Self {
            capacity,
            data: Vec::with_capacity(capacity as usize - 1),
        }
    }

    pub fn capacity(&self) -> u8 {
        self.capacity
    }

    pub fn set_data<I: IntoIterator<Item = u8>>(&mut self, data: I) {
        for (i, v) in data.into_iter().enumerate() {
            self.data[i] = v;
        }
    }
}

impl From<PascalString> for Vec<u8> {
    fn from(value: PascalString) -> Self {
        let mut v = vec![value.capacity];
        v.extend(value.data.iter());
        v
    }
}

impl TryFrom<&[u8]> for PascalString {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        let mut s = Self::with_capacity(value.len().try_into()?);
        if s.capacity > 1 {
            let data = &value[1..];
            s.set_data(data.to_vec());
        }
        Ok(s)
    }
}

#[derive(Debug, FromPrimitive, ToPrimitive)]
#[repr(u8)]
enum Color {
    Black = 1,
    Red = 2,
    Yellow = 3,
    SevenColor = 5,
}

impl TryFrom<Color> for u8 {
    type Error = Error;

    fn try_from(value: Color) -> Result<Self> {
        ConvertToPrimitive::to_u8(&value).context("Invalid Color value")
    }
}

impl TryFrom<u8> for Color {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        ConvertFromPrimitive::from_u8(value).context("Invalid Color value")
    }
}

#[derive(Debug, FromPrimitive, ToPrimitive)]
#[repr(u8)]
enum PcbVariant {
    V1 = 12,
}
impl TryFrom<PcbVariant> for u8 {
    type Error = Error;

    fn try_from(value: PcbVariant) -> Result<Self> {
        ConvertToPrimitive::to_u8(&value).context("Invalid PcbVariant value")
    }
}
impl TryFrom<u8> for PcbVariant {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        ConvertFromPrimitive::from_u8(value).context("Invalid PcbVariant value")
    }
}

#[derive(Debug, FromPrimitive, ToPrimitive)]
#[repr(u8)]
enum DisplayVariant {
    RedPHatHighTemp = 1,
    YellowWHat = 2,
    BlackWHat = 3,
    BlackPHat = 4,
    YellowPHat = 5,
    RedWHat = 6,
    RedWHatHighTemp = 7,
    RedWHatv2 = 8,
    BlackPHatSsd1608 = 10,
    RedPHatSsd1608 = 11,
    YellowPHatSsd1608 = 12,
    SevenColorUc8159 = 14,
    SevenColor640x400Uc8159 = 15,
    SevenColor640x400Uc8159v2 = 16,
    BlackWHatSsd1683 = 17,
    RedWHatSsd1683 = 18,
    YellowWHatSsd1683 = 19,
    SevenColor800x480Ac073Tc1A = 20,
}

impl TryFrom<DisplayVariant> for u8 {
    type Error = Error;

    fn try_from(value: DisplayVariant) -> Result<Self> {
        ConvertToPrimitive::to_u8(&value).context("Invalid DisplayVariant value")
    }
}

impl TryFrom<u8> for DisplayVariant {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        ConvertFromPrimitive::from_u8(value).context("Invalid DisplayVariant value")
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct EEPROM {
    width: u16,
    height: u16,
    color: Color,
    pcb_variant: PcbVariant,
    display_variant: DisplayVariant,
    eeprom_write_time: PascalString,
}

impl From<EEPROM> for Vec<u8> {
    fn from(value: EEPROM) -> Self {
        let mut v = Vec::new();
        v.extend_from_slice(&value.width.to_le_bytes());
        v.extend_from_slice(&value.height.to_le_bytes());
        v.push(value.color as u8);
        v.push(value.pcb_variant as u8);
        v.push(value.display_variant as u8);
        let write_time: Vec<u8> = value.eeprom_write_time.into();
        v.extend(write_time);
        v
    }
}

impl TryFrom<&[u8]> for EEPROM {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        let width = u16::from_le_bytes(value[..2].try_into()?);
        let height = u16::from_le_bytes(value[2..4].try_into()?);
        let color = Color::try_from(value[5])?;
        let pcb_variant = PcbVariant::try_from(value[6])?;
        let display_variant = DisplayVariant::try_from(value[7])?;
        let eeprom_write_time = PascalString::try_from(&value[8..])?;

        Ok(Self {
            width,
            height,
            color,
            pcb_variant,
            display_variant,
            eeprom_write_time,
        })
    }
}

impl EEPROM {
    pub const ADDRESS: u16 = 0x50;

    pub fn try_new() -> Result<Self> {
        let mut i2c_bus = I2c::with_bus(INKY_BUS)?;
        i2c_bus.set_slave_address(Self::ADDRESS)?;
        i2c_bus.block_write(0x00, &[0x00])?;
        let mut buffer = Vec::new();
        i2c_bus.block_read(0x00, &mut buffer)?;
        buffer.as_slice().try_into()
    }
}

#[cfg(test)]
mod tests {
    use crate::EEPROM;

    #[test]
    fn init_eeprom() {
        let eeprom = EEPROM::try_new().expect("Failed to initialize eeprom");
        eprintln!("Got eeprom: {:?}", eeprom);
        panic!("test fail");
    }
}
