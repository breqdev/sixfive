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
        0.0
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
        0.0
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
            1.0
        } else {
            0.0
        }
    }
}

pub struct ChannelRegisters {
    // Register 0 (envelope)
    duty_cycle: u8,
    looping: bool,
    envelope: bool,
    envelope_period: u8, // constant volume, if envelope not used

    // Register 1 (shift unit)
    shift_enabled: bool,
    shift_period: u8,
    shift_reverse: bool, // 0 = lengthen period / lower note, 1 = shorten period / higher note
    shift_speed: u8,     // this value is exponential

    // Register 2, 3 (period, note length)
    period: u16,
    note_length: u8,
}

impl Default for ChannelRegisters {
    fn default() -> Self {
        Self {
            duty_cycle: 0,
            looping: false,
            envelope: false,
            envelope_period: 0,
            shift_enabled: false,
            shift_period: 0,
            shift_reverse: false,
            shift_speed: 0,
            period: 0,
            note_length: 0,
        }
    }
}

impl ChannelRegisters {
    fn read(&self, register: u8) -> u8 {
        0
    }

    fn write(&mut self, register: u8, value: u8) {}
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

    pub fn generate(&mut self) -> f32 {
        self.generator.generate(&mut self.registers)
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
        0
    }

    pub fn write(&mut self, register: u8, value: u8) {}

    pub fn generate(&mut self) -> f32 {
        // Taken from https://www.nesdev.org/wiki/APU_Mixer#Linear_Approximation
        // Note that the * 16 is because our generate() calls return from 0.0 - 1.0, not 0 - 15
        (0.00376 * 16.0) * self.square_wave_1.generate()
            + (0.00376 * 16.0) * self.square_wave_2.generate()
            + (0.00851 * 16.0) * self.triangle_wave.generate()
            + (0.00494 * 16.0) * self.noise.generate()
    }
}
