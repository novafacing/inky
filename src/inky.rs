//! Control and draw to the Inky display

use crate::{
    eeprom::{DisplayVariant, EEPROM},
    lut::LUT_BLACK,
};
use anyhow::{ensure, Context, Error, Result};
use derive_builder::Builder;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive as ConvertFromPrimitive, ToPrimitive as ConvertToPrimitive};
use rppal::{
    gpio::{Gpio, InputPin, OutputPin, Trigger},
    spi::{Bus, Mode, SlaveSelect as SecondarySelect, Spi},
};
use std::{
    borrow::{Borrow, BorrowMut},
    fmt::Display,
    thread::sleep,
    time::Duration,
};

#[derive(Builder, Debug)]
/// Packet used to write to the SPI bus with a command, data, or both
pub struct SpiPacket {
    #[builder(setter(strip_option), default)]
    command: Option<Command>,
    #[builder(default)]
    data: Vec<u8>,
}

impl SpiPacket {
    /// Retrieve the SPI command
    pub fn command(&self) -> Option<u8> {
        self.command.clone().and_then(|c| c.try_into().ok())
    }

    /// Retrieve the SPI data
    pub fn data(&self) -> Vec<u8> {
        self.data.clone()
    }
}

#[derive(ToPrimitive, FromPrimitive, Debug, Clone)]
#[repr(u8)]
/// Enumertion of Inky display SPI commands according to the Inky Python library
/// there may be more commands, but I don't know what they are
pub enum Command {
    DataEntryMode = 0x11, // X/Y increment
    DisplayUpdateSequence = 0x22,
    DummyLinePeriod = 0x3a,
    EnterDeepSleep = 0x10,
    GSTransition = 0x3c,
    GateDrivingVoltage = 0x3,
    GateLineWidth = 0x3b,
    GateSetting = 0x1,
    SetAnalogBlockControl = 0x74,
    SetDigitalBlockControl = 0x7e,
    SetLUT = 0x32,
    SetRamXPointerStart = 0x4e,
    SetRamXStartEnd = 0x44,
    SetRamYPointerStart = 0x4f,
    SetRamYStartEnd = 0x45,
    SoftReset = 0x12,
    SourceDrivingVoltage = 0x4,
    TriggerDisplayUpdate = 0x20,
    VComRegister = 0x2c,
    SetBWBuffer = 0x24,
    SetRYBuffer = 0x26,
}

impl TryFrom<u8> for Command {
    type Error = Error;

    /// Convert a primitive u8 value to a Command
    fn try_from(value: u8) -> Result<Self> {
        ConvertFromPrimitive::from_u8(value).context("Invalid value for command")
    }
}

impl TryFrom<Command> for u8 {
    type Error = Error;

    /// Convert a command to a primitive u8 value
    fn try_from(value: Command) -> Result<Self> {
        value.to_u8().context("Not a valid u8 value")
    }
}

#[derive(Clone, Debug, Copy)]
/// Drawing colors, used on the `Canvas` to draw to the Inky screen
pub enum Color {
    Red,
    Yellow,
    Black,
    White,
}

impl Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Self::Red => "R",
                Self::Yellow => "Y",
                Self::Black => "B",
                Self::White => ".",
            }
        )
    }
}

impl Color {
    /// Convert the color to u8 for packing
    // TODO: Support additional displays
    fn as_u8(&self) -> u8 {
        if !matches!(*self, Color::Black) {
            1
        } else {
            0
        }
    }
}

impl From<u8> for Color {
    fn from(value: u8) -> Self {
        if value == 0 {
            Self::Black
        } else {
            Self::White
        }
    }
}

impl From<u32> for Color {
    fn from(value: u32) -> Self {
        if value == 0 {
            Self::Black
        } else {
            Self::White
        }
    }
}

pub trait Drawable {
    fn coordinates(&self) -> Vec<(usize, usize)>;
}

pub struct Line {
    start: (isize, isize),
    end: (isize, isize),
}

impl Line {
    pub fn new(start: (isize, isize), end: (isize, isize)) -> Self {
        Self { start, end }
    }

    // Returns a vector of coordinates along the line using Bresenham's algorithm
    fn line_coordinates(&self) -> Vec<(usize, usize)> {
        let mut result = Vec::new();

        let (mut x0, mut y0) = self.start;
        let (x1, y1) = self.end;

        let dx = x1 - x0;
        let dy = -(y1 - y0);

        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };

        let mut err = dx + dy;

        loop {
            result.push((x0 as usize, y0 as usize));
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }

        result
    }
}

impl Drawable for Line {
    fn coordinates(&self) -> Vec<(usize, usize)> {
        self.line_coordinates()
    }
}

pub struct Rectangle {
    top_left: (usize, usize),
    bottom_right: (usize, usize),
}

impl Rectangle {
    pub fn new(top_left: (usize, usize), bottom_right: (usize, usize)) -> Self {
        Self {
            top_left,
            bottom_right,
        }
    }

    // Returns a vector of coordinates inside the rectangle
    fn rectangle_coordinates(&self) -> Vec<(usize, usize)> {
        let mut result = Vec::new();

        for row in self.top_left.0..=self.bottom_right.0 {
            for col in self.top_left.1..=self.bottom_right.1 {
                result.push((row, col));
            }
        }

        result
    }
}

impl Drawable for Rectangle {
    fn coordinates(&self) -> Vec<(usize, usize)> {
        self.rectangle_coordinates()
    }
}

pub struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<Vec<Color>>,
}

impl Canvas {
    /// Create a new drawing canvas with a width and height
    fn new(width: usize, height: usize) -> Canvas {
        Canvas {
            width,
            height,
            pixels: vec![vec![Color::White; height]; width],
        }
    }

    /// Get the color of a given pixel
    fn get_pixel(&self, col: usize, row: usize) -> Color {
        self.pixels[col][row].clone()
    }

    /// Set the color of a given pixel
    fn set_pixel(&mut self, col: usize, row: usize, color: Color) {
        self.pixels[col][row] = color;
    }

    pub fn draw<D: Drawable>(&mut self, drawable: D) {
        for (row, col) in drawable.coordinates() {
            self.set_pixel(col, row, Color::Black);
        }
    }

    /// Get the height of the canvas
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get the width of the canvas
    pub fn width(&self) -> usize {
        self.width
    }

    /// Bitpack the canvas into bits representing (color|no color) from colored byte pixels
    pub fn pack(&self) -> Vec<u8> {
        let mut packed: Vec<u8> = Vec::new();
        let mut bit_pos: u8 = 0;
        let mut cur_byte: u8 = 0;
        for row in &self.pixels {
            for b in row {
                cur_byte |= (b.as_u8()) << bit_pos;
                bit_pos += 1;
                if bit_pos == 8 {
                    packed.push(cur_byte);
                    cur_byte = 0;
                    bit_pos = 0;
                }
            }
        }
        if bit_pos != 0 {
            packed.push(cur_byte);
        }
        packed
    }
}

#[derive(Builder)]
#[builder(pattern = "owned")]
/// The main display structure, used to control the Inky screen
pub struct Inky {
    color: Color,
    h_flip: bool,
    v_flip: bool,
    spi: Spi,
    // i2c is only used to read EEPROM
    // i2c: I2c,
    dc: OutputPin,
    reset: OutputPin,
    busy: InputPin,
    eeprom: EEPROM,
    canvas: Canvas,
}

impl TryFrom<EEPROM> for Inky {
    type Error = Error;

    fn try_from(value: EEPROM) -> Result<Self> {
        // TODO: Support additional displays
        ensure!(
            matches!(value.display_variant(), DisplayVariant::What),
            "Only the Inky wHat is supported!"
        );
        let gpio = Gpio::new()?;

        let dc_pin = 22;
        let reset_pin = 27;
        let busy_pin = 17;

        let dc = gpio.get(dc_pin)?;
        let dc = dc.into_output_low();
        let reset = gpio.get(reset_pin)?;
        let reset = reset.into_output_high();
        let busy = gpio.get(busy_pin)?;
        let busy = busy.into_input();

        let mut inky = InkyBuilder::default()
            .color(value.color().try_into()?)
            .h_flip(false)
            .v_flip(false)
            .spi(Spi::new(
                Bus::Spi0,
                SecondarySelect::Ss0,
                488_000,
                Mode::Mode0,
            )?)
            .dc(dc)
            .reset(reset)
            .busy(busy)
            .canvas(Canvas::new(value.width() as usize, value.height() as usize))
            .eeprom(value)
            .build()?;

        inky.reset()?;

        Ok(inky)
    }
}

impl Inky {
    /// Reset the display
    pub fn reset(&mut self) -> Result<()> {
        self.reset.set_low();
        // Sleep time from inky library
        sleep(Duration::from_millis(100));
        self.reset.set_high();
        sleep(Duration::from_millis(100));
        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::SoftReset)
                .build()?,
        )?;
        self.wait()?;
        Ok(())
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    pub fn canvas_mut(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    /// Update the display to show the contents of the canvas
    pub fn update(&mut self) -> Result<()> {
        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::SetAnalogBlockControl)
                .data(vec![0x54])
                .build()?,
        )?;

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::SetDigitalBlockControl)
                .data(vec![0x3b])
                .build()?,
        )?;

        let mut gate_setting_data = (self.canvas.height() as u16).to_le_bytes().to_vec();
        gate_setting_data.push(0x00);

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::GateSetting)
                .data(gate_setting_data)
                .build()?,
        )?;
        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::GateDrivingVoltage)
                .data(vec![0x17])
                .build()?,
        )?;

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::SourceDrivingVoltage)
                .data(vec![0x41, 0xAC, 0x32])
                .build()?,
        )?;
        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::DummyLinePeriod)
                .data(vec![0x07])
                .build()?,
        )?;
        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::GateLineWidth)
                .data(vec![0x04])
                .build()?,
        )?;
        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::DataEntryMode)
                .data(vec![0x03])
                .build()?,
        )?;
        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::VComRegister)
                .data(vec![0x3c])
                .build()?,
        )?;

        // TODO: Make this depend on color:
        // if self.border_colour == self.BLACK:
        //     self._send_command(0x3c, 0b00000000)  # GS Transition Define A + VSS + LUT0
        // elif self.border_colour == self.RED and self.colour == 'red':
        //     self._send_command(0x3c, 0b01110011)  # Fix Level Define A + VSH2 + LUT3
        // elif self.border_colour == self.YELLOW and self.colour == 'yellow':
        //     self._send_command(0x3c, 0b00110011)  # GS Transition Define A + VSH2 + LUT3
        // elif self.border_colour == self.WHITE:
        //     self._send_command(0x3c, 0b00110001)  # GS Transition Define A + VSH2 + LUT1
        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::GSTransition)
                .data(vec![0b00110001])
                .build()?,
        )?;

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::SetLUT)
                .data(LUT_BLACK.to_vec())
                .build()?,
        )?;

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::SetRamXStartEnd)
                .data(vec![0x00, ((self.canvas.width() / 8) - 1) as u8])
                .build()?,
        )?;

        let mut data = vec![0x00, 0x00];
        data.extend_from_slice(&(self.canvas.height() as u16).to_le_bytes());

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::SetRamYStartEnd)
                .data(data)
                .build()?,
        )?;

        let bw_buf = self.canvas.pack();
        // 0 because nothing == RED
        // let ry_buf = vec![0; bw_buf.len()];

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::SetRamXPointerStart)
                .data(vec![0x00])
                .build()?,
        )?;

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::SetRamYPointerStart)
                .data(vec![0x00, 0x00])
                .build()?,
        )?;

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::SetBWBuffer)
                .data(bw_buf)
                .build()?,
        )?;

        // TODO: Support additional displays
        // self.spi_send(
        //     SpiPacketBuilder::default()
        //         .command(Command::SetRamXPointerStart)
        //         .data(vec![0x00])
        //         .build()?,
        // )?;

        // self.spi_send(
        //     SpiPacketBuilder::default()
        //         .command(Command::SetRamYPointerStart)
        //         .data(vec![0x00, 0x00])
        //         .build()?,
        // )?;

        // self.spi_send(
        //     SpiPacketBuilder::default()
        //         .command(Command::SetRYBuffer)
        //         .data(ry_buf)
        //         .build()?,
        // )?;

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::DisplayUpdateSequence)
                .data(vec![0xc7])
                .build()?,
        )?;

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::TriggerDisplayUpdate)
                .build()?,
        )?;

        // Defined by inky
        sleep(Duration::from_secs_f32(0.05));

        self.wait()?;

        self.spi_send(
            SpiPacketBuilder::default()
                .command(Command::EnterDeepSleep)
                .data(vec![0x01])
                .build()?,
        )?;

        Ok(())
    }

    /// Wait for the display to update
    pub fn wait(&mut self) -> Result<()> {
        self.busy.set_interrupt(Trigger::FallingEdge)?;
        self.busy.poll_interrupt(false, None)?;
        self.busy.clear_interrupt()?;
        Ok(())
    }

    /// Send a packet over the SPI bus
    pub fn spi_send(&mut self, packet: SpiPacket) -> Result<()> {
        if let Some(command) = packet.command() {
            self.dc.set_low();
            self.spi.write(&[command])?;
        }

        if !packet.data().is_empty() {
            self.dc.set_high();
            for chunk in packet.data().chunks(4096) {
                self.spi.write(chunk)?;
            }
        }

        Ok(())
    }
}

pub struct BufferWrapper(Vec<u32>);

impl Borrow<[u8]> for BufferWrapper {
    fn borrow(&self) -> &[u8] {
        // Safe for alignment: align_of(u8) <= align_of(u32)
        // Safe for cast: u32 can be thought of as being transparent over [u8; 4]
        unsafe { std::slice::from_raw_parts(self.0.as_ptr() as *const u8, self.0.len() * 4) }
    }
}
impl BorrowMut<[u8]> for BufferWrapper {
    fn borrow_mut(&mut self) -> &mut [u8] {
        // Safe for alignment: align_of(u8) <= align_of(u32)
        // Safe for cast: u32 can be thought of as being transparent over [u8; 4]
        unsafe { std::slice::from_raw_parts_mut(self.0.as_mut_ptr() as *mut u8, self.0.len() * 4) }
    }
}
impl Borrow<[u32]> for BufferWrapper {
    fn borrow(&self) -> &[u32] {
        self.0.as_slice()
    }
}
impl BorrowMut<[u32]> for BufferWrapper {
    fn borrow_mut(&mut self) -> &mut [u32] {
        self.0.as_mut_slice()
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::{Borrow, BorrowMut};

    use super::{BufferWrapper, Color, Inky, Rectangle};
    use crate::eeprom::EEPROM;
    use anyhow::Result;

    #[test]
    fn test_blank() -> Result<()> {
        let eeprom = EEPROM::try_new().expect("Failed to initialize eeprom");
        let mut inky = Inky::try_from(eeprom)?;
        inky.update()?;
        Ok(())
    }

    #[test]
    fn test_draw_box() -> Result<()> {
        let eeprom = EEPROM::try_new().expect("Failed to initialize eeprom");
        let mut inky = Inky::try_from(eeprom)?;

        inky.canvas_mut().draw(Rectangle::new((0, 0), (299, 399)));

        inky.update()?;
        Ok(())
    }
}
