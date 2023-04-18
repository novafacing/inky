use crate::eeprom::EEPROM;
use anyhow::{Context, Error, Result};
use derive_builder::Builder;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive as ConvertFromPrimitive, ToPrimitive as ConvertToPrimitive};
use rppal::{
    gpio::{Gpio, InputPin, OutputPin, Trigger},
    spi::{Bus, Mode, SlaveSelect as SecondarySelect, Spi},
};
use std::{thread::sleep, time::Duration};

/*
Inky Lookup Tables.

These lookup tables comprise of two sets of values.

The first set of values, formatted as binary, describe the voltages applied during the six update phases:

    Phase 0     Phase 1     Phase 2     Phase 3     Phase 4     Phase 5     Phase 6
    A B C D
0b01001000, 0b10100000, 0b00010000, 0b00010000, 0b00010011, 0b00000000, 0b00000000,  LUT0 - Black
0b01001000, 0b10100000, 0b10000000, 0b00000000, 0b00000011, 0b00000000, 0b00000000,  LUT1 - White
0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,  NOT USED BY HARDWARE
0b01001000, 0b10100101, 0b00000000, 0b10111011, 0b00000000, 0b00000000, 0b00000000,  LUT3 - Yellow or Red
0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,  LUT4 - VCOM

There are seven possible phases, arranged horizontally, and only the phases with duration/repeat information
(see below) are used during the update cycle.

Each phase has four steps: A, B, C and D. Each step is represented by two binary bits and these bits can
have one of four possible values representing the voltages to be applied. The default values follow:

0b00: VSS or Ground
0b01: VSH1 or 15V
0b10: VSL or -15V
0b11: VSH2 or 5.4V

During each phase the Black, White and Yellow (or Red) stages are applied in turn, creating a voltage
differential across each display pixel. This is what moves the physical ink particles in their suspension.

The second set of values, formatted as hex, describe the duration of each step in a phase, and the number
of times that phase should be repeated:

    Duration                Repeat
    A     B     C     D
0x10, 0x04, 0x04, 0x04, 0x04,  <-- Timings for Phase 0
0x10, 0x04, 0x04, 0x04, 0x04,  <-- Timings for Phase 1
0x04, 0x08, 0x08, 0x10, 0x10,      etc
0x00, 0x00, 0x00, 0x00, 0x00,
0x00, 0x00, 0x00, 0x00, 0x00,
0x00, 0x00, 0x00, 0x00, 0x00,
0x00, 0x00, 0x00, 0x00, 0x00,

The duration and repeat parameters allow you to take a single sequence of A, B, C and D voltage values and
transform them into a waveform that - effectively - wiggles the ink particles into the desired position.

In all of our LUT definitions we use the first and second phases to flash/pulse and clear the display to
mitigate image retention. The flashing effect is actually the ink particles being moved from the bottom to
the top of the display repeatedly in an attempt to reset them back into a sensible resting position.
 */

pub const LUT_BLACK: &[u8] = &[
    0b01001000, 0b10100000, 0b00010000, 0b00010000, 0b00010011, 0b00000000, 0b00000000, 0b01001000,
    0b10100000, 0b10000000, 0b00000000, 0b00000011, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
    0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b01001000, 0b10100101, 0b00000000,
    0b10111011, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
    0b00000000, 0b00000000, 0b00000000, 0x10, 0x04, 0x04, 0x04, 0x04, 0x10, 0x04, 0x04, 0x04, 0x04,
    0x04, 0x08, 0x08, 0x10, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

pub const LUT_RED: &[u8] = &[
    0b01001000, 0b10100000, 0b00010000, 0b00010000, 0b00010011, 0b00000000, 0b00000000, 0b01001000,
    0b10100000, 0b10000000, 0b00000000, 0b00000011, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
    0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b01001000, 0b10100101, 0b00000000,
    0b10111011, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
    0b00000000, 0b00000000, 0b00000000, 0x40, 0x0C, 0x20, 0x0C, 0x06, 0x10, 0x08, 0x04, 0x04, 0x06,
    0x04, 0x08, 0x08, 0x10, 0x10, 0x02, 0x02, 0x02, 0x40, 0x20, 0x02, 0x02, 0x02, 0x02, 0x02, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

pub const LUT_RED_HIGHTEMP: &[u8] = &[
    0b01001000, 0b10100000, 0b00010000, 0b00010000, 0b00010011, 0b00010000, 0b00010000, 0b01001000,
    0b10100000, 0b10000000, 0b00000000, 0b00000011, 0b10000000, 0b10000000, 0b00000000, 0b00000000,
    0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b01001000, 0b10100101, 0b00000000,
    0b10111011, 0b00000000, 0b01001000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000,
    0b00000000, 0b00000000, 0b00000000, 0x43, 0x0A, 0x1F, 0x0A, 0x04, 0x10, 0x08, 0x04, 0x04, 0x06,
    0x04, 0x08, 0x08, 0x10, 0x0B, 0x02, 0x04, 0x04, 0x40, 0x10, 0x06, 0x06, 0x06, 0x02, 0x02, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

pub const LUT_YELLOW: &[u8] = &[
    0b11111010, 0b10010100, 0b10001100, 0b11000000, 0b11010000, 0b00000000, 0b00000000, 0b11111010,
    0b10010100, 0b00101100, 0b10000000, 0b11100000, 0b00000000, 0b00000000, 0b11111010, 0b00000000,
    0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b11111010, 0b10010100, 0b11111000,
    0b10000000, 0b01010000, 0b00000000, 0b11001100, 0b10111111, 0b01011000, 0b11111100, 0b10000000,
    0b11010000, 0b00000000, 0b00010001, 0x40, 0x10, 0x40, 0x10, 0x08, 0x08, 0x10, 0x04, 0x04, 0x10,
    0x08, 0x08, 0x03, 0x08, 0x20, 0x08, 0x04, 0x00, 0x00, 0x10, 0x10, 0x08, 0x08, 0x00, 0x20, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[derive(Builder, Debug)]
pub struct SpiPacket {
    #[builder(setter(strip_option), default)]
    command: Option<Command>,
    #[builder(default)]
    data: Vec<u8>,
}

impl SpiPacket {
    pub fn command(&self) -> Option<u8> {
        self.command.clone().and_then(|c| c.try_into().ok())
    }

    pub fn data(&self) -> Vec<u8> {
        self.data.clone()
    }
}

#[derive(ToPrimitive, FromPrimitive, Debug, Clone)]
#[repr(u8)]
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

    fn try_from(value: u8) -> Result<Self> {
        ConvertFromPrimitive::from_u8(value).context("Invalid value for command")
    }
}

impl TryFrom<Command> for u8 {
    type Error = Error;

    fn try_from(value: Command) -> Result<Self> {
        value.to_u8().context("Not a valid u8 value")
    }
}

#[derive(Clone, Debug)]
pub enum Color {
    Red,
    Yellow,
    Black,
    White,
}

impl Color {
    fn as_u8(&self) -> u8 {
        if !matches!(*self, Color::Black) {
            1
        } else {
            0
        }
    }
}

pub struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<Vec<Color>>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Canvas {
        Canvas {
            width,
            height,
            pixels: vec![vec![Color::White; height]; width],
        }
    }

    fn get_pixel(&self, col: usize, row: usize) -> Color {
        self.pixels[col][row].clone()
    }

    fn set_pixel(&mut self, col: usize, row: usize, color: Color) {
        self.pixels[col][row] = color;
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }

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
pub struct Inky {
    width: u16,
    height: u16,
    color: Color,
    cs_channel: u8,
    dc_pin: u8,
    reset_pin: u8,
    busy_pin: u8,
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
        let mut gpio = Gpio::new()?;

        let cs_channel = 0;
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
            .width(value.width())
            .height(value.height())
            .color(value.color().try_into()?)
            .cs_channel(cs_channel)
            .dc_pin(dc_pin)
            .reset_pin(reset_pin)
            .busy_pin(busy_pin)
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

    pub fn wait(&mut self) -> Result<()> {
        self.busy.set_interrupt(Trigger::FallingEdge)?;
        self.busy.poll_interrupt(false, None)?;
        self.busy.clear_interrupt()?;
        Ok(())
    }

    pub fn spi_send(&mut self, packet: SpiPacket) -> Result<()> {
        if let Some(command) = packet.command() {
            println!("Sending command: {:#x}", command);
            self.dc.set_low();
            self.spi.write(&[command])?;
        }

        if !packet.data().is_empty() {
            if packet.data().len() < 64 {
                println!("Data: {:?}", packet.data());
            }
            self.dc.set_high();
            for chunk in packet.data().chunks(4096) {
                println!("Sending data (len {})", chunk.len());
                self.spi.write(chunk)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Inky;
    use crate::eeprom::EEPROM;
    use anyhow::Result;

    #[test]
    fn test_blank() -> Result<()> {
        let eeprom = EEPROM::try_new().expect("Failed to initialize eeprom");
        let mut inky = Inky::try_from(eeprom)?;
        inky.update()?;
        Ok(())
    }
}
