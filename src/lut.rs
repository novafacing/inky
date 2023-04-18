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
