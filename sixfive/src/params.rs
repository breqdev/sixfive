use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::{Arc, Mutex};

#[derive(PartialEq, Copy, Clone, Enum)]
pub enum RomBank {
    A,
    B,
    C,
    D,
}

impl RomBank {
    pub fn as_index(&self) -> usize {
        match self {
            RomBank::A => 0,
            RomBank::B => 1,
            RomBank::C => 2,
            RomBank::D => 3,
        }
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(RomBank::A),
            1 => Some(RomBank::B),
            2 => Some(RomBank::C),
            3 => Some(RomBank::D),
            _ => None,
        }
    }
}

#[derive(Params)]
pub struct TrampolineVectorParams {
    #[id = "state"]
    pub state: BoolParam,
}

#[derive(Params)]
pub struct SixFiveParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "rom-bank-select"]
    pub rom_bank_select: EnumParam<RomBank>,

    #[persist = "rom-banks"]
    pub rom_banks: Mutex<[Vec<u16>; 4]>,

    #[id = "clock-speed"]
    pub clock_speed: IntParam,

    // todo: instruction pointer overwrite as parameters?
    #[nested(array, group = "trampoline-vectors")]
    pub trampoline_vectors: [TrampolineVectorParams; 4],

    #[id = "square-1-enable"]
    pub square_wave_1_enable: BoolParam,

    #[id = "square-2-enable"]
    pub square_wave_2_enable: BoolParam,

    #[id = "triangle-enable"]
    pub triangle_wave_enable: BoolParam,

    #[id = "noise-enable"]
    pub noise_enable: BoolParam,
}

impl SixFiveParams {
    pub fn read_rom(&self, address: u8) -> u8 {
        let bank_index = self.rom_bank_select.value().as_index();
        let bank = &self.rom_banks.lock().unwrap()[bank_index];

        if address & 0b1 == 0 {
            (bank[address as usize / 2] >> 8) as u8
        } else {
            bank[address as usize / 2] as u8
        }
    }

    pub fn read_trampoline_vector(&self, address: u8) -> u8 {
        match address {
            0xFC => self.trampoline_vectors[0].state.value() as u8,
            0xFD => self.trampoline_vectors[1].state.value() as u8,
            0xFE => self.trampoline_vectors[2].state.value() as u8,
            0xFF => self.trampoline_vectors[3].state.value() as u8,
            _ => panic!("invalid trampoline vector address"),
        }
    }
}

impl Default for SixFiveParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(450, 500),

            rom_bank_select: EnumParam::new("ROM Bank Selection", RomBank::A),

            rom_banks: Mutex::new([vec![0; 64], vec![0; 64], vec![0; 64], vec![0; 64]]),

            clock_speed: IntParam::new(
                "Clock Speed",
                10,
                IntRange::Linear {
                    min: 1,
                    max: 1_000_000,
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(0.1)),

            // todo: instruction pointer overwrite?
            trampoline_vectors: [
                TrampolineVectorParams {
                    state: BoolParam::new("Trampoline Vector A", false),
                },
                TrampolineVectorParams {
                    state: BoolParam::new("Trampoline Vector B", false),
                },
                TrampolineVectorParams {
                    state: BoolParam::new("Trampoline Vector C", false),
                },
                TrampolineVectorParams {
                    state: BoolParam::new("Trampoline Vector D", false),
                },
            ],

            square_wave_1_enable: BoolParam::new("Square Wave 1 Enable", true),
            square_wave_2_enable: BoolParam::new("Square Wave 2 Enable", true),
            triangle_wave_enable: BoolParam::new("Triangle Wave Enable", true),
            noise_enable: BoolParam::new("Noise Enable", true),
        }
    }
}
