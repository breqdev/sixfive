const CHANNEL_MAX_VOLUME: f32 = 15.0;

pub mod conversions {
    pub fn note_period_to_seconds(period: u16) -> f64 {
        //  fCPU / (16 × (t + 1))
        // TODO: We probably don't exactly have to match the NES here
        // (we might as well pick something easier to work with)
        let frequency = 1789773.0 / (16.0 * (period as f64 + 1.0));
        1.0 / frequency
    }

    pub fn note_length_to_seconds(length: u8) -> f64 {
        (length as f64) * 1.0 / 60.0 // TODO: this doesn't match NES (uses a lookup table). Do something similar?
    }

    pub fn envelope_length_to_seconds(length: u8) -> f64 {
        (length as f64) * 1.0 / 15.0
    }

    pub fn seconds_to_shift_steps(seconds: f64) -> u16 {
        (seconds / (60.0 * 2.0)) as u16
    }
}

pub struct ChannelRegisters {
    // Register 0 (envelope)
    pub duty_cycle: u8,
    pub looping: bool,
    pub envelope: bool,
    pub envelope_length: u8, // constant volume, if envelope not used

    // Register 1 (shift unit)
    pub shift_enabled: bool,
    pub shift_period: u8,
    pub shift_reverse: bool, // 0 = lengthen period / lower note, 1 = shorten period / higher note
    pub shift_speed: u8,     // this value is exponential

    // Register 2, 3 (period, note length)
    pub period: u16,
    pub note_length: u8,

    // Internal registers
    time_since_note: f64,
}

impl Default for ChannelRegisters {
    fn default() -> Self {
        Self {
            duty_cycle: 0,
            looping: false,
            envelope: false,
            envelope_length: 0,
            shift_enabled: false,
            shift_period: 0,
            shift_reverse: false,
            shift_speed: 0,
            period: 0,
            note_length: 0,

            time_since_note: 0.0,
        }
    }
}

impl ChannelRegisters {
    fn read(&self, register: u8) -> u8 {
        match register {
            0x00 => {
                let mut value = 0;
                value |= self.duty_cycle << 6;
                value |= (self.looping as u8) << 5;
                value |= (self.envelope as u8) << 4;
                value |= self.envelope_length;
                value
            }
            0x01 => {
                let mut value = 0;
                value |= (self.shift_enabled as u8) << 7;
                value |= self.shift_period << 4;
                value |= (self.shift_reverse as u8) << 3;
                value |= self.shift_speed;
                value
            }
            0x02 => {
                let mut value = 0;
                value |= self.period as u8;
                value
            }
            0x03 => {
                let mut value = 0;
                value |= (self.period >> 8) as u8;
                value |= self.note_length << 3;
                value
            }
            _ => panic!("Read from invalid register: {:02X}", register),
        }
    }

    fn write(&mut self, register: u8, value: u8) {
        match register {
            0x00 => {
                self.duty_cycle = (value >> 6) & 0b11;
                self.looping = (value >> 5) & 0b1 == 1;
                self.envelope = (value >> 4) & 0b1 == 1;
                self.envelope_length = value & 0b1111;
            }
            0x01 => {
                self.shift_enabled = (value >> 7) & 0b1 == 1;
                self.shift_period = (value >> 4) & 0b111;
                self.shift_reverse = (value >> 3) & 0b1 == 1;
                self.shift_speed = value & 0b111;
            }
            0x02 => {
                self.period = (self.period & 0b111_0000_0000) | value as u16;
            }
            0x03 => {
                self.period = (self.period & 0b000_1111_1111) | (((value & 0b0111) as u16) << 8);
                self.note_length = (value >> 3) & 0b1111_1;
                self.time_since_note = 0.0;
            }
            _ => panic!("Write to invalid register: {:02X}", register),
        };
    }

    fn tick(&mut self, sample_rate: f64) {
        self.time_since_note += 1.0 / sample_rate;
    }

    fn get_effective_volume(&self) -> f32 {
        // Are we outside a note?
        if self.time_since_note > conversions::note_length_to_seconds(self.note_length) {
            return 0.0;
        }

        // Is the envelope active?
        if self.envelope {
            let elapsed = self.time_since_note;
            let total = conversions::envelope_length_to_seconds(self.envelope_length);
            let relative = (elapsed % total) / total;
            let relative = 1.0 - relative;

            return relative as f32;
        }

        // Otherwise, use the constant volume
        self.envelope_length as f32 / CHANNEL_MAX_VOLUME
    }

    fn get_effective_period(&self) -> u16 {
        let steps_since_shift = conversions::seconds_to_shift_steps(self.time_since_note);
        let mut effective_period = self.period;
        if self.shift_enabled {
        for _ in 0..steps_since_shift {
            if self.shift_reverse {
                    effective_period -= effective_period >> self.shift_period;
                } else {
                    effective_period += effective_period >> self.shift_period;
                }
            }
        };
        effective_period
    }
}

pub trait WaveGenerator: Default {
    fn generate(&mut self, registers: &mut ChannelRegisters) -> f32;
}

pub struct SquareWave {}

impl Default for SquareWave {
    fn default() -> Self {
        Self {}
    }
}

impl WaveGenerator for SquareWave {
    fn generate(&mut self, registers: &mut ChannelRegisters) -> f32 {
        let elapsed = registers.time_since_note;
        let period = conversions::note_period_to_seconds(registers.get_effective_period());
        let relative = (elapsed % period) / period;

        let value = match registers.duty_cycle {
            0b00 => {
                if relative < 0.125 {
                    0.0
                } else {
                    1.0
                }
            }
            0b01 => {
                if relative < 0.25 {
                    0.0
                } else {
                    1.0
                }
            }
            0b10 => {
                if relative < 0.5 {
                    0.0
                } else {
                    1.0
                }
            }
            0b11 => {
                if relative < 0.75 {
                    0.0
                } else {
                    1.0
                }
            }
            _ => panic!("Invalid duty cycle: {}", registers.duty_cycle),
        };

        value * registers.get_effective_volume()
    }
}

pub struct TriangleWave {}

impl Default for TriangleWave {
    fn default() -> Self {
        Self {}
    }
}

impl WaveGenerator for TriangleWave {
    fn generate(&mut self, registers: &mut ChannelRegisters) -> f32 {
        let elapsed = registers.time_since_note;
        let period = conversions::note_period_to_seconds(registers.get_effective_period());
        let relative = (elapsed % period) / period;

        let value = if relative < 0.5 {
            (relative * 2.0) as f32
        } else {
            ((1.0 - relative) * 2.0) as f32
        };

        value * registers.get_effective_volume()
    }
}

pub struct Noise {
    shift_register: u16,
}

impl Default for Noise {
    fn default() -> Self {
        Self { shift_register: 1 }
    }
}

impl WaveGenerator for Noise {
    fn generate(&mut self, registers: &mut ChannelRegisters) -> f32 {
        let feedback = (self.shift_register & 0b1) ^ ((self.shift_register >> 1) & 0b1);
        self.shift_register >>= 1;
        self.shift_register |= feedback << 14;

        if feedback == 0 {
            1.0 * registers.get_effective_volume()
        } else {
            0.0
        }
    }
}

pub struct Channel<T: WaveGenerator> {
    registers: ChannelRegisters,
    generator: T,
}

impl<T: WaveGenerator> Default for Channel<T> {
    fn default() -> Self {
        Self {
            registers: ChannelRegisters::default(),
            generator: T::default(),
        }
    }
}

impl<T: WaveGenerator> Channel<T> {
    fn read(&self, register: u8) -> u8 {
        self.registers.read(register)
    }

    fn write(&mut self, register: u8, value: u8) {
        self.registers.write(register, value)
    }

    pub fn generate(&mut self, sample_rate: f64) -> f32 {
        self.registers.tick(sample_rate);
        self.generator.generate(&mut self.registers)
    }

    pub fn registers(&self) -> &ChannelRegisters {
        &self.registers
    }
}

pub struct SoundChip {
    pub square_wave_1: Channel<SquareWave>,
    pub square_wave_2: Channel<SquareWave>,
    pub triangle_wave: Channel<TriangleWave>,
    pub noise: Channel<Noise>,
}

impl Default for SoundChip {
    fn default() -> Self {
        Self {
            square_wave_1: Channel::default(),
            square_wave_2: Channel::default(),
            triangle_wave: Channel::default(),
            noise: Channel::default(),
        }
    }
}

impl SoundChip {
    pub fn read(&self, register: u8) -> u8 {
        match register {
            0xA0..=0xA3 => self.square_wave_1.read(register - 0xA0),
            0xA4..=0xA7 => self.square_wave_2.read(register - 0xA4),
            0xA8..=0xAB => self.triangle_wave.read(register - 0xA8),
            0xAC..=0xAF => self.noise.read(register - 0xAC),
            _ => panic!("Read from invalid sound register: {:02X}", register),
        }
    }

    pub fn write(&mut self, register: u8, value: u8) {
        match register {
            0xA0..=0xA3 => self.square_wave_1.write(register - 0xA0, value),
            0xA4..=0xA7 => self.square_wave_2.write(register - 0xA4, value),
            0xA8..=0xAB => self.triangle_wave.write(register - 0xA8, value),
            0xAC..=0xAF => self.noise.write(register - 0xAC, value),
            _ => panic!("Write to invalid sound register: {:02X}", register),
        }
    }

    pub fn generate(&mut self, sample_rate: f64) -> f32 {
        // Taken from https://www.nesdev.org/wiki/APU_Mixer#Linear_Approximation
        // Note that the * 16 is because our generate() calls return from 0.0 - 1.0, not 0 - 15
        (0.00376 * 16.0) * self.square_wave_1.generate(sample_rate)
            + (0.00376 * 16.0) * self.square_wave_2.generate(sample_rate)
            + (0.00851 * 16.0) * self.triangle_wave.generate(sample_rate)
            + (0.00494 * 16.0) * self.noise.generate(sample_rate)
    }
}
