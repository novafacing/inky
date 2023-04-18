use std::{thread::sleep, time::Duration};

use crate::inky::Color as InkyColor;
use anyhow::{bail, ensure, Context, Error, Result};
use chrono::NaiveDateTime;
use num::{FromPrimitive as ConvertFromPrimitive, ToPrimitive as ConvertToPrimitive};
use num_derive::{FromPrimitive, ToPrimitive};
use rppal::i2c::I2c;

// Inky devices all use Bus 1
pub const INKY_BUS: u8 = 1;

#[derive(Debug)]
pub struct PascalString {
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

    pub fn data(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity as u8 + 1;
        self.data.reserve(capacity);
    }

    pub fn set_data<I: IntoIterator<Item = u8>>(&mut self, data: I) {
        self.data.clear();
        self.data.extend(data);
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
            s.set_capacity(data.len());
            s.set_data(data.to_vec());
        }
        Ok(s)
    }
}

#[derive(Debug, FromPrimitive, ToPrimitive, Clone)]
#[repr(u8)]
pub enum Color {
    Black = 1,
    Red = 2,
    Yellow = 3,
    SevenColor = 5,
}

impl TryFrom<Color> for InkyColor {
    type Error = Error;

    fn try_from(value: Color) -> Result<Self> {
        Ok(match value {
            Color::Black => InkyColor::Black,
            Color::Red => InkyColor::Red,
            Color::Yellow => InkyColor::Yellow,
            Color::SevenColor => bail!("Cannot convert EEPROM color to Inky Color"),
        })
    }
}

impl TryFrom<Color> for u8 {
    type Error = Error;

    fn try_from(value: Color) -> Result<Self> {
        ConvertToPrimitive::to_u8(&value).context(format!("Invalid Color value {:?}", value))
    }
}

impl TryFrom<u8> for Color {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        ConvertFromPrimitive::from_u8(value).context(format!("Invalid Color value {}", value))
    }
}

#[derive(Debug, FromPrimitive, ToPrimitive, Clone)]
#[repr(u8)]
pub enum PcbVariant {
    V1 = 12,
}
impl TryFrom<PcbVariant> for u8 {
    type Error = Error;

    fn try_from(value: PcbVariant) -> Result<Self> {
        ConvertToPrimitive::to_u8(&value).context(format!("Invalid PcbVariant value {:?}", value))
    }
}
impl TryFrom<u8> for PcbVariant {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        ConvertFromPrimitive::from_u8(value).context(format!("Invalid PcbVariant value {}", value))
    }
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum DisplayVariant {
    // RedPHatHighTemp = 1,
    // YellowWHat = 2,
    // BlackWHat = 3,
    // BlackPHat = 4,
    // YellowPHat = 5,
    // RedWHat = 6,
    // RedWHatHighTemp = 7,
    // RedWHatv2 = 8,
    // BlackPHatSsd1608 = 10,
    // RedPHatSsd1608 = 11,
    // YellowPHatSsd1608 = 12,
    // SevenColorUc8159 = 14,
    // SevenColor640x400Uc8159 = 15,
    // SevenColor640x400Uc8159v2 = 16,
    // BlackWHatSsd1683 = 17,
    // RedWHatSsd1683 = 18,
    // YellowWHatSsd1683 = 19,
    // SevenColor800x480Ac073Tc1A = 20,
    Phat,
    PhatSsd1608,
    What,
    Uc8159600448,
    Uc8159640400,
    WhatSsd1683,
    Ac073Tc1A,
}

impl TryFrom<u8> for DisplayVariant {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        Ok(match value {
            1 | 4 | 5 => Self::Phat,
            10 | 11 | 12 => Self::PhatSsd1608,
            2 | 3 | 6 | 7 | 8 => Self::What,
            14 => Self::Uc8159600448,
            15 | 16 => Self::Uc8159640400,
            17 | 18 | 19 => Self::WhatSsd1683,
            20 => Self::Ac073Tc1A,
            _ => bail!("Invalid value {} for DisplayVariant", value),
        })
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
        println!("Got width: {}", width);
        let height = u16::from_le_bytes(value[2..4].try_into()?);
        println!("Got height: {}", height);
        let color = Color::try_from(value[4])?;
        println!("Got color: {:?}", color);
        let pcb_variant = PcbVariant::try_from(value[5])?;
        println!("Got pcb variant: {:?}", pcb_variant);
        let display_variant = DisplayVariant::try_from(value[6])?;
        println!("Got display variant: {:?}", display_variant);
        let eeprom_write_time_bytes = value[7..]
            .iter()
            .filter(|v| **v != 255)
            .cloned()
            .collect::<Vec<_>>();

        let eeprom_write_time = PascalString::try_from(eeprom_write_time_bytes.as_slice())?;
        println!("Got EEPROM write time: {:?}", eeprom_write_time);

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
    // Address of the i2c device
    pub const ADDRESS: u16 = 0x50;
    // Give up by default after 10 attempts to read the EEPROM
    pub const DEFAULT_TRIES: usize = 10;

    pub fn try_new() -> Result<Self> {
        Self::try_new_tries(Self::DEFAULT_TRIES)
    }

    pub fn try_new_tries(max_tries: usize) -> Result<Self> {
        let mut i2c_bus = I2c::with_bus(INKY_BUS)?;

        for _ in 0..max_tries {
            i2c_bus.set_slave_address(Self::ADDRESS)?;
            i2c_bus.write(&[0x00; 2])?;
            // sleep(Duration::from_millis(1000));
            let buffer = &mut [0x00; 29];
            i2c_bus.set_slave_address(Self::ADDRESS)?;
            let read = i2c_bus.read(buffer)?;
            ensure!(read >= 29, "Read length {} is too small", read);
            eprintln!("Got buffer: {:?}", buffer);
            match buffer.as_slice().try_into() {
                Ok(eeprom) => {
                    return Ok(eeprom);
                }
                Err(e) => {
                    eprintln!("Failed to initialize eeprom, retrying: {}", e);
                }
            }
            sleep(Duration::from_secs_f32(0.1));
        }

        bail!("Failed to initialize eeprom in {} tries", max_tries);
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn color(&self) -> Color {
        self.color.clone()
    }

    pub fn pcb_variant(&self) -> PcbVariant {
        self.pcb_variant.clone()
    }

    pub fn display_variant(&self) -> DisplayVariant {
        self.display_variant.clone()
    }

    pub fn eeprom_write_time(&self) -> Result<NaiveDateTime> {
        let string = String::from_utf8_lossy(&self.eeprom_write_time.data);
        Ok(NaiveDateTime::parse_from_str(
            &string,
            "%Y-%m-%d %H:%M:%S%.1f",
        )?)
    }
}

#[cfg(test)]
mod tests {
    use crate::eeprom::EEPROM;
    // A buffer retrieved with this code:
    // 144, 1, 44, 1, 1, 12, 3, 21, 50, 48, 50, 48, 45, 49, 48, 45, 48, 49, 32, 49, 53, 58, 53, 49, 58, 52, 51, 46, 51, 255, 255, 255
    // A buffer retrieved with smbus2:
    // 144, 1, 44, 1, 1, 12, 3, 21, 50, 48, 50, 48, 45, 49, 48, 45, 48, 49, 32, 49, 53, 58, 53, 49, 58, 52, 51, 46, 51

    #[test]
    /// Tests that EEPROM can be initialized by reading it from the device
    /// no specific device is tested for, because you should be able to run
    /// this test on any device with an Inky e-ink display plugged into it.
    /// However, only the Black wHat is tested.
    fn init_eeprom() {
        _ = EEPROM::try_new().expect("Failed to initialize eeprom");
    }
}
