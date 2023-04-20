use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use std::sync::{Arc, Mutex};

struct Cpu {
    pub accumulator: u8,
    pub instruction_pointer: u8,
    pub status_register: (bool, bool),

    pub clock_running: bool,
    pub beats_waiting: u8,
    pub cycles_waiting: u8,

    pub memory: Memory,
    pub params: Arc<SixFiveParams>,
}

impl Cpu {
    fn new(params: &Arc<SixFiveParams>) -> Self {
        Self {
            accumulator: 0,
            instruction_pointer: 0,
            status_register: (false, false),

            clock_running: false,
            beats_waiting: 0,
            cycles_waiting: 0,

            memory: Memory::default(),
            params: params.clone(),
        }
    }

    fn reset(&mut self) {
        self.instruction_pointer = 0;
        self.accumulator = 0;
        self.status_register = (false, false);
        self.memory = Memory::default();
    }

    fn set_status_register(&mut self, value: u8) {
        self.status_register = (value == 0, value & 0b1000_0000 != 0)
    }

    fn read(&mut self, address: u8) -> u8 {
        match address {
            0x00..=0x7F => self.params.read_rom(address),
            0x80..=0x9F => self.memory.ram[address as usize - 0x80],
            0xA0..=0xAF => self.memory.audio[address as usize - 0xA0],
            0xB0..=0xEF => panic!("unimplemented memory read"),
            0xF0..=0xFF => self.params.read_trampoline_vector(address),
        }
    }

    fn write(&mut self, address: u8, value: u8) {
        println!("writing {:02X} to {:02X}", value, address);
        match address {
            0x00..=0x7F => panic!("ROM not writable"),
            0x80..=0x9F => self.memory.ram[address as usize - 0x80] = value,
            0xA0..=0xAF => self.memory.audio[address as usize - 0xA0] = value,
            0xB0..=0xEF => panic!("unimplemented memory write"),
            0xF0..=0xFF => panic!("trampoline vectors not writable"),
        }
    }

    fn execute(&mut self) {
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
                let index = (opcode & 0x0F) as usize;
                self.memory.audio[index] = operand;
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

struct Memory {
    pub ram: [u8; 0x20],
    pub audio: [u8; 0x10],
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            ram: [0; 0x20],
            audio: [0; 0x10],
        }
    }
}

pub struct SixFive {
    params: Arc<SixFiveParams>,
    sample_rate: f32,

    cpu: Arc<Mutex<Cpu>>,

    samples_until_execute: f64,
}

#[derive(PartialEq, Copy, Clone, Enum)]
enum RomBank {
    A,
    B,
    C,
    D,
}

impl RomBank {
    fn as_index(&self) -> usize {
        match self {
            RomBank::A => 0,
            RomBank::B => 1,
            RomBank::C => 2,
            RomBank::D => 3,
        }
    }

    fn from_index(index: usize) -> Option<Self> {
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
struct TrampolineVectorParams {
    #[id = "state"]
    pub state: BoolParam,
}

#[derive(Params)]
struct SixFiveParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[id = "rom-bank-select"]
    pub rom_bank_select: EnumParam<RomBank>,

    #[persist = "rom-banks"]
    rom_banks: Mutex<[Vec<u16>; 4]>,

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
    fn read_rom(&self, address: u8) -> u8 {
        let bank_index = self.rom_bank_select.value().as_index();
        let bank = &self.rom_banks.lock().unwrap()[bank_index];

        if address & 0b1 == 0 {
            (bank[address as usize / 2] >> 8) as u8
        } else {
            bank[address as usize / 2] as u8
        }
    }

    fn read_trampoline_vector(&self, address: u8) -> u8 {
        match address {
            0xFC => self.trampoline_vectors[0].state.value() as u8,
            0xFD => self.trampoline_vectors[1].state.value() as u8,
            0xFE => self.trampoline_vectors[2].state.value() as u8,
            0xFF => self.trampoline_vectors[3].state.value() as u8,
            _ => panic!("invalid trampoline vector address"),
        }
    }
}

struct GuiUserState {
    rom_bank: Vec<String>,
}

impl Default for GuiUserState {
    fn default() -> Self {
        Self {
            rom_bank: vec!["0000".to_string(); 64],
        }
    }
}

impl Default for SixFive {
    fn default() -> Self {
        let params = Arc::new(SixFiveParams::default());

        Self {
            params: params.clone(),
            sample_rate: 1.0,

            cpu: Arc::new(Mutex::new(Cpu::new(&params))),

            samples_until_execute: 0.0,
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

impl SixFive {
    // fn calculate_sine(&mut self, frequency: f32) -> f32 {
    //     let phase_delta = frequency / self.sample_rate;
    //     let sine = (self.phase * consts::TAU).sin();

    //     self.phase += phase_delta;
    //     if self.phase >= 1.0 {
    //         self.phase -= 1.0;
    //     }

    //     sine
    // }
}

const OVERWRITE_INSTRUCTION_POINTER_VALUES: [u8; 8] =
    [0x00, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70];
const TRAMPOLINE_VECTOR_JUMP_ADDRESSES: [(u8, u8); 4] =
    [(0x28, 0x2C), (0x28, 0x3C), (0x30, 0x34), (0x38, 0x3C)];

struct SixFiveEditor;

impl SixFiveEditor {
    fn draw_rom_location(ui: &mut egui::Ui, active: bool, input: &mut String, value: &mut u16) {
        egui::Frame::none()
            .stroke(egui::Stroke::new(
                2.0,
                if active {
                    egui::Color32::RED
                } else {
                    egui::Color32::BLACK
                },
            ))
            .show(ui, |ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(input)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(30.0)
                        .frame(false),
                );

                if response.gained_focus() {
                    *input = "".to_string();
                }

                if response.lost_focus() {
                    if ui.input().key_pressed(egui::Key::Enter)
                        || ui.input().key_pressed(egui::Key::Tab)
                    {
                        *value = u16::from_str_radix(input, 16).unwrap_or(0);
                    }

                    *input = format!("{:04X}", value);
                }
            });
    }

    fn draw_rom(
        ui: &mut egui::Ui,
        params: &SixFiveParams,
        setter: &ParamSetter,
        state: &mut GuiUserState,
        cpu: &Cpu,
    ) {
        ui.group(|ui| {
            ui.vertical_centered_justified(|ui| {
                let mut selected_index = params.rom_bank_select.value().as_index();

                ui.horizontal(|ui| {
                    ui.add_space(50.0);

                    ui.label("ROM Bank: ");

                    if egui::ComboBox::from_label("")
                        .show_index(ui, &mut selected_index, 4, |i| {
                            ["A", "B", "C", "D"][i].to_string()
                        })
                        .changed()
                    {
                        setter.begin_set_parameter(&params.rom_bank_select);
                        setter.set_parameter(
                            &params.rom_bank_select,
                            RomBank::from_index(selected_index).unwrap(),
                        );
                        setter.end_set_parameter(&params.rom_bank_select);

                        state.rom_bank = params.rom_banks.lock().unwrap()[selected_index]
                            .iter()
                            .map(|b| format!("{:04X}", b))
                            .collect();
                    }
                });

                ui.separator();

                for row in 0..8 {
                    ui.horizontal_top(|ui| {
                        ui.label(egui::RichText::from(format!("0x{:02X}", row * 16)).monospace());

                        ui.add_space(10.0);

                        for col in 0..8 {
                            let index = row * 8 + col;

                            SixFiveEditor::draw_rom_location(
                                ui,
                                cpu.instruction_pointer == (index as u8 * 2),
                                &mut state.rom_bank[index],
                                &mut params.rom_banks.lock().unwrap()[selected_index][index],
                            )
                        }
                    });
                }
            });
        });
    }

    fn draw_ram(ui: &mut egui::Ui, cpu: &Cpu) {
        ui.group(|ui| {
            ui.vertical_centered_justified(|ui| {
                let ram = cpu.memory.ram;

                for row in 0..2 {
                    ui.horizontal_top(|ui| {
                        ui.label(
                            egui::RichText::from(format!("0x{:02X}", 0x80 + row * 0x10))
                                .monospace(),
                        );

                        ui.add_space(15.0);

                        for col in 0..16 {
                            ui.label(
                                egui::RichText::from(format!("{:02X}", ram[row * 0x10 + col]))
                                    .monospace(),
                            );
                        }

                        ui.add_space(ui.available_width());
                    });
                }
            });
        });
    }

    fn draw_audio_registers(ui: &mut egui::Ui, cpu: &Cpu) {
        ui.group(|ui| {
            ui.vertical_centered_justified(|ui| {
                // TODO: register-specific readouts

                ui.horizontal_top(|ui| {
                    ui.label(egui::RichText::from("0xA0").monospace());

                    ui.add_space(15.0);

                    let audio_registers = cpu.memory.audio;

                    for col in 0..16 {
                        ui.label(
                            egui::RichText::from(format!("{:02X}", audio_registers[col]))
                                .monospace(),
                        );
                    }

                    ui.add_space(ui.available_width());
                });
            });
        });
    }

    fn draw_instruction_pointer(
        ui: &mut egui::Ui,
        params: &SixFiveParams,
        setter: &ParamSetter,
        cpu: &mut Cpu,
    ) {
        ui.group(|ui| {
            ui.vertical_centered_justified(|ui| {
                ui.horizontal_top(|ui| {
                    if ui.button("↺").clicked() {
                        cpu.reset();
                    }

                    if ui.button("▶").clicked() {
                        cpu.clock_running = true;
                    }

                    if ui.button("⏹").clicked() {
                        cpu.reset();
                        cpu.clock_running = false;
                    }

                    if ui.button("⏸").clicked() {
                        cpu.clock_running = false;
                    }

                    ui.add_space(20.0);

                    ui.label("Instruction Pointer");
                    ui.label(
                        egui::RichText::from(format!("0x{:02X}", cpu.instruction_pointer))
                            .monospace(),
                    );
                    ui.label("Clock Speed");
                    ui.label(
                        egui::RichText::from(format!("{} Hz", params.clock_speed.value()))
                            .monospace(),
                    );

                    ui.add_space(ui.available_width());
                });
            });
        });
    }

    fn draw_overwrite_instruction_pointer(ui: &mut egui::Ui, cpu: &mut Cpu) {
        ui.group(|ui| {
            ui.label("Overwrite Instruction Pointer");
            for chunk in OVERWRITE_INSTRUCTION_POINTER_VALUES.chunks(4) {
                ui.horizontal(|ui| {
                    for i in chunk {
                        if ui
                            .button(egui::RichText::from(format!("0x{:02X}", i)).monospace())
                            .clicked()
                        {
                            cpu.instruction_pointer = i.clone();
                            cpu.clock_running = true;
                        }
                    }

                    ui.add_space(ui.available_width());
                });
            }
        });
    }

    fn draw_trampoline_vectors(ui: &mut egui::Ui, params: &SixFiveParams, setter: &ParamSetter) {
        ui.group(|ui| {
            ui.label("Toggle Trampoline Vectors");
            ui.horizontal(|ui| {
                for (i, (off, on)) in TRAMPOLINE_VECTOR_JUMP_ADDRESSES.iter().enumerate() {
                    ui.vertical(|ui| {
                        let param = &params.trampoline_vectors[i].state;

                        let mut job = egui::text::LayoutJob::default();

                        job.append(
                            format!("0x{:02X}\n", off).as_str(),
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::new(14.0, egui::FontFamily::Monospace),
                                color: if param.value() {
                                    egui::Color32::GRAY
                                } else {
                                    egui::Color32::BLACK
                                },
                                ..Default::default()
                            },
                        );

                        job.append(
                            format!("0x{:02X}", on).as_str(),
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::new(14.0, egui::FontFamily::Monospace),
                                color: if param.value() {
                                    egui::Color32::BLACK
                                } else {
                                    egui::Color32::GRAY
                                },
                                ..Default::default()
                            },
                        );

                        if ui.button(job).clicked() {
                            setter.begin_set_parameter(param);
                            setter.set_parameter(param, !param.value());
                            setter.end_set_parameter(param);
                        }

                        ui.horizontal(|ui| {
                            ui.add_space(5.0);
                            ui.label(
                                egui::RichText::from(format!("0x{:02X}", 0xFC + i)).monospace(),
                            );
                        })
                    });
                }

                ui.add_space(ui.available_width());
            })
        });
    }

    fn draw_enable_voices(ui: &mut egui::Ui, params: &SixFiveParams, setter: &ParamSetter) {
        ui.group(|ui| {
            ui.label("Enable Synth Voices");
            ui.horizontal(|ui| {
                let square_wave_1_enable = params.square_wave_1_enable.value();
                let square_wave_2_enable = params.square_wave_2_enable.value();
                let triangle_wave_enable = params.triangle_wave_enable.value();
                let noise_enable = params.noise_enable.value();

                if ui
                    .button(
                        egui::RichText::new("Pulse 1").color(if square_wave_1_enable {
                            egui::Color32::BLACK
                        } else {
                            egui::Color32::GRAY
                        }),
                    )
                    .clicked()
                {
                    setter.begin_set_parameter(&params.square_wave_1_enable);
                    setter.set_parameter(&params.square_wave_1_enable, !square_wave_1_enable);
                    setter.end_set_parameter(&params.square_wave_1_enable);
                }

                if ui
                    .button(
                        egui::RichText::new("Pulse 2").color(if square_wave_2_enable {
                            egui::Color32::BLACK
                        } else {
                            egui::Color32::GRAY
                        }),
                    )
                    .clicked()
                {
                    setter.begin_set_parameter(&params.square_wave_2_enable);
                    setter.set_parameter(&params.square_wave_2_enable, !square_wave_2_enable);
                    setter.end_set_parameter(&params.square_wave_2_enable);
                }

                if ui
                    .button(egui::RichText::new("Tri").color(if triangle_wave_enable {
                        egui::Color32::BLACK
                    } else {
                        egui::Color32::GRAY
                    }))
                    .clicked()
                {
                    setter.begin_set_parameter(&params.triangle_wave_enable);
                    setter.set_parameter(&params.triangle_wave_enable, !triangle_wave_enable);
                    setter.end_set_parameter(&params.triangle_wave_enable);
                }

                if ui
                    .button(egui::RichText::new("Noise").color(if noise_enable {
                        egui::Color32::BLACK
                    } else {
                        egui::Color32::GRAY
                    }))
                    .clicked()
                {
                    setter.begin_set_parameter(&params.noise_enable);
                    setter.set_parameter(&params.noise_enable, !noise_enable);
                    setter.end_set_parameter(&params.noise_enable);
                }

                ui.add_space(ui.available_width());
            })
        });
    }

    fn draw_register_view(ui: &mut egui::Ui, cpu: &mut Cpu) {
        ui.group(|ui| {
            ui.label("Register View");

            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label("Accumulator");

                ui.add_space(5.0);

                ui.label(egui::RichText::from(format!("{:02X}", cpu.accumulator)).monospace());

                ui.add_space(ui.available_width());
            });
        });
    }
}

impl Plugin for SixFive {
    const NAME: &'static str = "SixFive: 8-Bit Inspired Musical State Machine";
    const VENDOR: &'static str = "Brooke Chalmers";
    const URL: &'static str = "https://breq.dev/";
    const EMAIL: &'static str = "breq@breq.dev";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;

        true
    }

    fn reset(&mut self) {
        self.cpu.lock().unwrap().reset();
    }

    fn editor(&self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let cpu = self.cpu.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            GuiUserState::default(),
            |_, _| {},
            move |egui_ctx, setter, state| {
                egui_ctx.set_visuals(egui::Visuals::light());

                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    ui.vertical_centered_justified(|ui| {
                        let mut cpu = cpu.lock().unwrap();

                        SixFiveEditor::draw_rom(ui, &params, setter, state, &cpu);

                        SixFiveEditor::draw_ram(ui, &cpu);

                        SixFiveEditor::draw_audio_registers(ui, &cpu);

                        SixFiveEditor::draw_instruction_pointer(ui, &params, setter, &mut cpu);

                        ui.columns(2, |columns| {
                            columns[0].vertical(|ui| {
                                SixFiveEditor::draw_overwrite_instruction_pointer(ui, &mut cpu);

                                SixFiveEditor::draw_enable_voices(ui, &params, setter);
                            });

                            columns[1].vertical(|ui| {
                                SixFiveEditor::draw_trampoline_vectors(ui, &params, setter);

                                SixFiveEditor::draw_register_view(ui, &mut cpu);
                            });
                        });
                    });
                });
            },
        )
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut next_event = context.next_event();
        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            {
                let mut cpu = self.cpu.lock().unwrap();

                if cpu.clock_running {
                    if self.samples_until_execute <= 0.0 {
                        cpu.execute();

                        self.samples_until_execute +=
                            (self.sample_rate as f64) / (self.params.clock_speed.value() as f64);
                    }

                    self.samples_until_execute -= 1.0;
                } else {
                    self.samples_until_execute = 0.0;
                }
            }

            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }

                match event {
                    NoteEvent::NoteOn { note, velocity, .. } => {}
                    NoteEvent::NoteOff { note, .. } => {}
                    NoteEvent::MidiCC { cc, value, .. } => {}
                    _ => (),
                }

                next_event = context.next_event();
            }
        }

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for SixFive {
    const CLAP_ID: &'static str = "dev.breq.plugins.sixfive";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A 8-bit inspired musical state machine");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for SixFive {
    const VST3_CLASS_ID: [u8; 16] = *b"breqsixfivesound";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Synth,
        Vst3SubCategory::Tools,
    ];
}

nih_export_clap!(SixFive);
nih_export_vst3!(SixFive);
