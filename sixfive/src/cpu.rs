use std::sync::Arc;

use crate::{params::SixFiveParams, sound::SoundChip};

pub struct Cpu {
    pub accumulator: u8,
    pub instruction_pointer: u8,
    pub status_register: (bool, bool),

    pub clock_running: bool,
    pub beats_waiting: u8,
    pub cycles_waiting: u8,

    pub ram: [u8; 0x20],
    pub sound: SoundChip,
    pub params: Arc<SixFiveParams>,
}

impl Cpu {
    pub fn new(params: &Arc<SixFiveParams>) -> Self {
        Self {
            accumulator: 0,
            instruction_pointer: 0,
            status_register: (false, false),

            clock_running: false,
            beats_waiting: 0,
            cycles_waiting: 0,

            ram: [0; 0x20],
            sound: SoundChip::default(),

            params: params.clone(),
        }
    }

    pub fn reset(&mut self) {
        self.instruction_pointer = 0;
        self.accumulator = 0;
        self.status_register = (false, false);
        self.ram = [0; 0x20];
        self.sound = SoundChip::default();
    }

    fn set_status_register(&mut self, value: u8) {
        self.status_register = (value == 0, value & 0b1000_0000 != 0)
    }

    fn read(&mut self, address: u8) -> u8 {
        match address {
            0x00..=0x7F => self.params.read_rom(address),
            0x80..=0x9F => self.ram[address as usize - 0x80],
            0xA0..=0xAF => self.sound.read(address),
            0xB0..=0xEF => panic!("unimplemented memory read"),
            0xF0..=0xFF => self.params.read_trampoline_vector(address),
        }
    }

    fn write(&mut self, address: u8, value: u8) {
        println!("writing {:02X} to {:02X}", value, address);
        match address {
            0x00..=0x7F => panic!("ROM not writable"),
            0x80..=0x9F => self.ram[address as usize - 0x80] = value,
            0xA0..=0xAF => self.sound.write(address, value),
            0xB0..=0xEF => panic!("unimplemented memory write"),
            0xF0..=0xFF => panic!("trampoline vectors not writable"),
        }
    }

    pub fn execute(&mut self) {
        if self.cycles_waiting > 0 {
            self.cycles_waiting -= 1;
            return;
        }

        let opcode = self.read(self.instruction_pointer);

        // The last bit of the opcode determines the addressing mode:
        // 0: immediate
        // 1: absolute

        // With the exception of 0xA0..=0xAF, which are always immediate
        // And 0xB0..=0xBF, which are always absolute
        // (so that the last 4 bits of the instruction match the last 4 bits of the address being written)
        let operand = match opcode {
            0xA0..=0xAF => self.read(self.instruction_pointer.wrapping_add(1)),
            0xB0..=0xBF => {
                let address = self.read(self.instruction_pointer.wrapping_add(1));
                self.read(address)
            }
            _ => {
                if opcode & 0b0000_0001 == 0 {
                    self.read(self.instruction_pointer.wrapping_add(1))
                } else {
                    let address = self.read(self.instruction_pointer.wrapping_add(1));
                    self.read(address)
                }
            }
        };

        println!(
            "{:02X}: {:02X} ({:02X})",
            self.instruction_pointer, opcode, operand
        );

        // we update this now in order to avoid messing up jumps
        self.instruction_pointer = self.instruction_pointer.wrapping_add(2);

        match opcode {
            // Halt
            0x00 | 0x01 => {
                // HALT
                self.clock_running = false;
            }

            // Loading and Storing
            0x10 | 0x11 => {
                // LOAD
                self.accumulator = operand;
                self.set_status_register(self.accumulator);
            }
            0x12 | 0x13 => {
                // STOR
                self.write(operand, self.accumulator);
                self.set_status_register(self.accumulator);
            }

            // Arithmetic

            // Add
            0x20 | 0x21 => {
                // ADD
                self.accumulator = self.accumulator.wrapping_add(operand);
                self.set_status_register(self.accumulator);
            }

            // Set Status Register With Value
            0x22 | 0x23 => {
                // SSR
                self.set_status_register(operand);
            }

            // Subtraction
            0x24 | 0x25 => {
                // SUB
                self.set_status_register(self.accumulator);
            }

            // Comparison
            0x26 | 0x27 => {
                // CMP
                let value = self.accumulator.wrapping_sub(operand);
                self.set_status_register(value);
            }

            // Branching
            0x30 | 0x31 => {
                // BREQ
                if self.status_register.0 {
                    self.instruction_pointer = operand;
                }
            }
            0x32 | 0x33 => {
                // BRNE
                if !self.status_register.0 {
                    self.instruction_pointer = operand;
                }
            }
            0x34 | 0x35 => {
                // BRLT
                if self.status_register.1 {
                    self.instruction_pointer = operand;
                }
            }
            0x36 | 0x37 => {
                // BRGE
                if !self.status_register.1 {
                    self.instruction_pointer = operand;
                }
            }

            // Jumping
            0x40 | 0x41 => {
                // JMP
                self.instruction_pointer = operand;
            }

            // Bitwise
            0x50 | 0x51 => {
                // AND
                self.accumulator = self.accumulator & operand;
                self.set_status_register(self.accumulator);
            }
            0x52 | 0x53 => {
                // BIT
                let value = self.accumulator & operand;
                self.set_status_register(value);
            }
            0x54 | 0x55 => {
                // OR
                self.accumulator = self.accumulator | operand;
                self.set_status_register(self.accumulator);
            }
            0x56 | 0x57 => {
                // XOR
                self.accumulator = self.accumulator ^ operand;
                self.set_status_register(self.accumulator);
            }
            0x58 | 0x59 => {
                // LSL
                self.accumulator = self.accumulator << operand;
                self.set_status_register(self.accumulator);
            }
            0x5A | 0x5B => {
                // LSR
                self.accumulator = self.accumulator >> operand;
                self.set_status_register(self.accumulator);
            }
            0x5C | 0x5D => {
                // ROL
                self.accumulator = self.accumulator.rotate_left(operand as u32);
                self.set_status_register(self.accumulator);
            }
            0x5E | 0x5F => {
                // ROR
                self.accumulator = self.accumulator.rotate_right(operand as u32);
                self.set_status_register(self.accumulator);
            }

            // Operations directly on memory
            0x60 | 0x61 => {
                // ZERO
                self.write(operand, 0);
                self.set_status_register(0);
            }
            0x62 | 0x63 => {
                // INC
                let value = self.read(operand).wrapping_add(1);
                self.write(operand, value);
                self.set_status_register(value);
            }
            0x64 | 0x65 => {
                // DEC
                let value = self.read(operand).wrapping_sub(1);
                self.write(operand, value);
                self.set_status_register(value);
            }

            // Audio Register Manipulation
            0xA0..=0xAF | 0xB0..=0xBF => {
                // AWI0 ... AWIF
                let index = (opcode & 0x0F) | 0xA0;
                self.sound.write(index, operand);
            }

            // No-ops and Waits
            0xF0 | 0xF1 => {
                // BEAT
                self.beats_waiting = operand;
            }
            0xF2 | 0xF3 => {
                // NOOP
                self.cycles_waiting = operand;
            }

            // Unimplemented
            _ => {
                println!("Unimplemented opcode: {:02X}", opcode);
                self.clock_running = false;
            }
        };
    }
}
