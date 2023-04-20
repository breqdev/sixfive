use gui::draw_editor;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use std::sync::{Arc, Mutex};

mod cpu;
mod gui;
mod params;

use cpu::Cpu;
use params::SixFiveParams;

pub struct SixFive {
    params: Arc<SixFiveParams>,
    sample_rate: f32,

    cpu: Arc<Mutex<Cpu>>,

    samples_until_execute: f64,
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
        draw_editor(
            self.params.editor_state.clone(),
            self.params.clone(),
            self.cpu.clone(),
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
