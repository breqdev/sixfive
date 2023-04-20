use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use std::sync::{Arc, Mutex};

use crate::{
    cpu::Cpu,
    params::{RomBank, SixFiveParams},
};

const OVERWRITE_INSTRUCTION_POINTER_VALUES: [u8; 8] =
    [0x00, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70];
const TRAMPOLINE_VECTOR_JUMP_ADDRESSES: [(u8, u8); 4] =
    [(0x28, 0x2C), (0x28, 0x3C), (0x30, 0x34), (0x38, 0x3C)];

struct GuiUserState {
    rom_bank: Vec<String>,
    clock_speed: String,
}

impl GuiUserState {
    fn new(params: &Arc<SixFiveParams>) -> Self {
        let mut rom_bank = Vec::new();
        for i in 0x00..0x40 {
            let high = params.read_rom(i * 2) as u16;
            let low = params.read_rom(i * 2 + 1) as u16;
            rom_bank.push(format!("{:04X}", (high << 8) | low));
        }

        Self {
            rom_bank,
            clock_speed: params.clock_speed.value().to_string(),
        }
    }
}

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

                        draw_rom_location(
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
            for row in 0..2 {
                ui.horizontal_top(|ui| {
                    ui.label(
                        egui::RichText::from(format!("0x{:02X}", 0x80 + row * 0x10)).monospace(),
                    );

                    ui.add_space(15.0);

                    for col in 0..16 {
                        ui.label(
                            egui::RichText::from(format!("{:02X}", cpu.ram[row * 0x10 + col]))
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

                for register in 0xA0..=0xAF {
                    ui.label(
                        egui::RichText::from(format!("{:02X}", cpu.sound.read(register)))
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
    input: &mut String,
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
                    egui::RichText::from(format!("0x{:02X}", cpu.instruction_pointer)).monospace(),
                );
                ui.label("Clock Speed");

                egui::Frame::default().show(ui, |ui| {
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
                        if ui.input().key_pressed(egui::Key::Enter) {
                            let value = input.parse().unwrap();

                            setter.begin_set_parameter(&params.clock_speed);
                            setter.set_parameter(&params.clock_speed, value);
                            setter.end_set_parameter(&params.clock_speed);

                            *input = format!("{}", value);
                        } else {
                            *input = format!("{}", params.clock_speed.value());
                        }
                    }
                });

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
                        ui.label(egui::RichText::from(format!("0x{:02X}", 0xFC + i)).monospace());
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

pub fn draw_editor(
    editor_state: Arc<EguiState>,
    params: Arc<SixFiveParams>,
    cpu: Arc<Mutex<Cpu>>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        editor_state,
        GuiUserState::new(&params),
        |_, _| {},
        move |egui_ctx, setter, state| {
            egui_ctx.set_visuals(egui::Visuals::light());

            egui::CentralPanel::default().show(egui_ctx, |ui| {
                ui.vertical_centered_justified(|ui| {
                    let mut cpu = cpu.lock().unwrap();

                    draw_rom(ui, &params, setter, state, &cpu);

                    draw_ram(ui, &cpu);

                    draw_audio_registers(ui, &cpu);

                    draw_instruction_pointer(ui, &params, setter, &mut cpu, &mut state.clock_speed);

                    ui.columns(2, |columns| {
                        columns[0].vertical(|ui| {
                            draw_overwrite_instruction_pointer(ui, &mut cpu);

                            draw_enable_voices(ui, &params, setter);
                        });

                        columns[1].vertical(|ui| {
                            draw_trampoline_vectors(ui, &params, setter);

                            draw_register_view(ui, &mut cpu);
                        });
                    });
                });
            });
        },
    )
}
