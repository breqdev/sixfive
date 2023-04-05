use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use std::sync::{Arc, Mutex};

pub struct SixFive {
    params: Arc<SixFiveParams>,
    sample_rate: f32,

    instruction_pointer: Arc<Mutex<u8>>,
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
    rom_banks: Mutex<[Vec<u8>; 4]>,

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

struct GuiUserState {
    rom_bank: Vec<String>,
}

impl Default for GuiUserState {
    fn default() -> Self {
        Self {
            rom_bank: vec!["00".to_string(); 64],
        }
    }
}

impl Default for SixFive {
    fn default() -> Self {
        Self {
            params: Arc::new(SixFiveParams::default()),
            sample_rate: 1.0,

            instruction_pointer: Arc::new(Mutex::new(0)),
        }
    }
}

impl Default for SixFiveParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(600, 300),

            rom_bank_select: EnumParam::new("ROM Bank Selection", RomBank::A),

            rom_banks: Mutex::new([vec![0; 64], vec![0; 64], vec![0; 64], vec![0; 64]]),

            clock_speed: IntParam::new(
                "Clock Speed",
                1,
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

const OVERWRITE_INSTRUCTION_POINTER_VALUES: [u8; 4] = [0x00, 0x10, 0x20, 0x30];
const TRAMPOLINE_VECTOR_JUMP_ADDRESSES: [(u8, u8); 4] =
    [(0x28, 0x2C), (0x28, 0x3C), (0x30, 0x34), (0x38, 0x3C)];

struct SixFiveEditor;

impl SixFiveEditor {
    fn draw_rom_location(ui: &mut egui::Ui, active: bool, input: &mut String, value: &mut u8) {
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
                        .desired_width(16.0)
                        .frame(false),
                );

                if response.lost_focus() {
                    if ui.input().key_pressed(egui::Key::Enter)
                        || ui.input().key_pressed(egui::Key::Tab)
                    {
                        *value = u8::from_str_radix(input, 16).unwrap_or(0);
                    }

                    *input = format!("{:02X}", value);
                }
            });
    }

    fn draw_rom(
        ui: &mut egui::Ui,
        params: &SixFiveParams,
        setter: &ParamSetter,
        state: &mut GuiUserState,
        instruction_pointer: &Arc<Mutex<u8>>,
    ) {
        ui.group(|ui| {
            ui.vertical_centered_justified(|ui| {
                let mut selected_index = params.rom_bank_select.value().as_index();

                if egui::ComboBox::from_label("ROM Bank")
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
                        .map(|b| format!("{:02X}", b))
                        .collect();
                }

                ui.separator();

                for row in 0..8 {
                    ui.horizontal_top(|ui| {
                        ui.label(egui::RichText::from(format!("0x{:02X}", row * 8)).monospace());

                        for col in 0..8 {
                            let index = row * 8 + col;

                            SixFiveEditor::draw_rom_location(
                                ui,
                                instruction_pointer.lock().unwrap().clone() == index as u8,
                                &mut state.rom_bank[index],
                                &mut params.rom_banks.lock().unwrap()[selected_index][index],
                            )
                        }
                    });
                }
            });
        });
    }

    fn draw_instruction_pointer(ui: &mut egui::Ui, instruction_pointer: &Arc<Mutex<u8>>) {
        ui.group(|ui| {
            ui.horizontal_top(|ui| {
                ui.label("Instruction Pointer");
                ui.label(
                    egui::RichText::from(format!(
                        "0x{:02X}",
                        instruction_pointer.lock().unwrap().clone()
                    ))
                    .monospace(),
                );
                ui.label("Clock Speed");
                ui.label(egui::RichText::from("10 Hz").monospace());
            });
        });
    }

    fn draw_overwrite_instruction_pointer(ui: &mut egui::Ui, instruction_pointer: &Arc<Mutex<u8>>) {
        ui.group(|ui| {
            ui.label("Overwrite Instruction Pointer");
            ui.horizontal(|ui| {
                for i in OVERWRITE_INSTRUCTION_POINTER_VALUES {
                    if ui
                        .button(egui::RichText::from(format!("0x{:02X}", i)).monospace())
                        .clicked()
                    {
                        *instruction_pointer.lock().unwrap() = i;
                    }
                }
            })
        });
    }

    fn draw_trampoline_vectors(ui: &mut egui::Ui, params: &SixFiveParams, setter: &ParamSetter) {
        ui.group(|ui| {
            ui.label("Toggle Trampoline Vectors");
            ui.horizontal(|ui| {
                for (i, (off, on)) in TRAMPOLINE_VECTOR_JUMP_ADDRESSES.iter().enumerate() {
                    ui.vertical(|ui| {
                        ui.label(format!(" 0x{:02X}", 0xFC + i));

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
                    });
                }
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
                        egui::RichText::new("Square 1").color(if square_wave_1_enable {
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
                        egui::RichText::new("Square 2").color(if square_wave_2_enable {
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
                    .button(
                        egui::RichText::new("Triangle").color(if triangle_wave_enable {
                            egui::Color32::BLACK
                        } else {
                            egui::Color32::GRAY
                        }),
                    )
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
            })
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
        *self.instruction_pointer.lock().unwrap() = 0;
    }

    fn editor(&self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let instruction_pointer = self.instruction_pointer.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            GuiUserState::default(),
            |_, _| {},
            move |egui_ctx, setter, state| {
                egui_ctx.set_visuals(egui::Visuals::light());

                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    ui.columns(2, |columns| {
                        columns[0].vertical_centered_justified(|ui| {
                            SixFiveEditor::draw_rom(
                                ui,
                                &params,
                                setter,
                                state,
                                &instruction_pointer,
                            );

                            SixFiveEditor::draw_instruction_pointer(ui, &instruction_pointer);
                        });

                        columns[1].vertical_centered_justified(|ui| {
                            SixFiveEditor::draw_overwrite_instruction_pointer(
                                ui,
                                &instruction_pointer,
                            );

                            SixFiveEditor::draw_trampoline_vectors(ui, &params, setter);

                            SixFiveEditor::draw_enable_voices(ui, &params, setter);
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
