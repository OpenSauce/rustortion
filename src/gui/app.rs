use iced::{Element, Length, Subscription, Task, Theme, keyboard, time, time::Duration};
use log::{debug, error};

use crate::amp::chain::AmplifierChain;
use crate::audio::manager::Manager;
use crate::gui::components::ir_cabinet_control::IrCabinetControl;
use crate::gui::components::peak_meter::PeakMeterDisplay;
use crate::gui::components::pitch_shift_control::PitchShiftControl;
use crate::gui::components::{control::Control, stage_list::StageList};
use crate::gui::config::{StageConfig, StageType};
use crate::gui::handlers::hotkey::HotkeyHandler;
use crate::gui::handlers::midi::MidiHandler;
use crate::gui::handlers::preset::PresetHandler;
use crate::gui::handlers::settings::SettingsHandler;
use crate::gui::handlers::tuner::TunerHandler;
use crate::gui::messages::{
    HotkeyMessage, Message, MidiMessage, PresetMessage, SettingsMessage, TunerMessage,
};
use crate::i18n;
use crate::midi::start_midi_manager;
use crate::settings::Settings;
use crate::tr;

const REBUILD_INTERVAL: Duration = Duration::from_millis(100);
const TUNER_POLL_INTERVAL: Duration = Duration::from_millis(20);
const MIDI_POLL_INTERVAL: Duration = Duration::from_millis(10);
const PEAK_METER_POLL_INTERVAL: Duration = Duration::from_millis(20);

pub struct AmplifierApp {
    audio_manager: Manager,
    stages: Vec<StageConfig>,
    is_recording: bool,
    stage_list: StageList,
    control_bar: Control,
    settings: Settings,
    settings_handler: SettingsHandler,
    dirty_chain: bool,
    ir_cabinet_control: IrCabinetControl,
    pitch_shift_control: PitchShiftControl,
    tuner_handler: TunerHandler,
    preset_handler: PresetHandler,
    peak_meter_display: PeakMeterDisplay,
    midi_handler: MidiHandler,
    hotkey_handler: HotkeyHandler,
}

impl AmplifierApp {
    pub fn boot(settings: Settings) -> (Self, Task<Message>) {
        let audio_manager = Manager::new(settings.clone()).unwrap();
        let mut preset_handler = PresetHandler::new(&settings.preset_dir).unwrap();

        // Try and load the last opened preset
        if let Some(last_opened_preset) = settings.selected_preset.as_deref() {
            preset_handler.load_preset_by_name(last_opened_preset);
        }

        let preset = preset_handler.get_selected_preset().unwrap_or_default();

        let stage_list = StageList::new(preset.stages.clone());
        let control_bar = Control::new(StageType::default());
        let settings_handler = SettingsHandler::new(&settings.audio);

        let mut ir_cabinet_control = IrCabinetControl::new(settings.ir_bypassed, preset.ir_gain);
        ir_cabinet_control.set_available_irs(audio_manager.get_available_irs());

        let pitch_shift_control = PitchShiftControl::new(preset.pitch_shift_semitones);

        if settings.ir_bypassed {
            audio_manager.engine().set_ir_bypass(true);
        }

        audio_manager.engine().set_ir_gain(preset.ir_gain);

        audio_manager
            .engine()
            .set_pitch_shift(preset.pitch_shift_semitones);

        if let Some(ir_name) = preset.ir_name {
            ir_cabinet_control.set_selected_ir(Some(ir_name.clone()));
            audio_manager.engine().set_ir_cabinet(Some(ir_name));
        } else if let Some(first_ir) = ir_cabinet_control.get_selected_ir() {
            ir_cabinet_control.set_selected_ir(Some(first_ir.clone()));
            audio_manager.engine().set_ir_cabinet(Some(first_ir));
        }

        // Initialize MIDI
        let midi_handle = start_midi_manager();
        let mut midi_handler = MidiHandler::new(midi_handle);

        // Load MIDI mappings from settings
        midi_handler.set_mappings(settings.midi.mappings.clone());

        // Try to connect to saved MIDI controller
        if let Some(controller_name) = &settings.midi.controller_name {
            midi_handler.connect(controller_name);
            debug!(
                "Attempting to reconnect to MIDI controller: {}",
                controller_name
            );
        }

        // Set the global language from settings
        i18n::set_language(settings.language);

        let hotkey_handler = HotkeyHandler::new(settings.hotkeys.clone());

        (
            Self {
                audio_manager,
                stages: preset.stages,
                is_recording: false,
                stage_list,
                control_bar,
                settings,
                settings_handler,
                // Set dirty chain to true to trigger initial rebuild
                dirty_chain: true,
                ir_cabinet_control,
                pitch_shift_control,
                tuner_handler: TunerHandler::new(),
                preset_handler,
                peak_meter_display: PeakMeterDisplay::new(),
                midi_handler,
                hotkey_handler,
            },
            Task::none(),
        )
    }

    pub fn view(&self) -> Element<'_, Message> {
        use iced::widget::{button, column, container, row, space};

        let top_bar = row![
            self.peak_meter_display.view(),
            space::horizontal(),
            self.pitch_shift_control.view(),
            button(tr!(hotkeys))
                .on_press(Message::Hotkey(HotkeyMessage::Open))
                .style(iced::widget::button::secondary),
            button(tr!(midi))
                .on_press(Message::Midi(MidiMessage::Open))
                .style(iced::widget::button::secondary),
            button(tr!(tuner))
                .on_press(Message::Tuner(TunerMessage::Toggle))
                .style(iced::widget::button::secondary),
            button(tr!(settings)).on_press(Message::Settings(SettingsMessage::Open)),
        ]
        .spacing(5);

        let main_content = column![
            top_bar,
            self.preset_handler.view(),
            self.stage_list.view(),
            self.ir_cabinet_control.view(),
            self.control_bar.view(self.is_recording),
        ]
        .spacing(10)
        .padding(20);

        let dialogs = [
            self.settings_handler.view(),
            self.tuner_handler.view(),
            self.midi_handler.view(),
            self.hotkey_handler.view(),
        ];

        if let Some(dialog) = dialogs.into_iter().flatten().next() {
            dialog
        } else {
            container(main_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::TokyoNight
    }

    // subscription handles all the periodic tasks that happen in the UI
    // this is usually polling for updates from the tuner, audio engine etc
    pub fn subscription(&self) -> Subscription<Message> {
        let rebuild_sub = if self.dirty_chain {
            time::every(REBUILD_INTERVAL).map(|_| Message::RebuildTick)
        } else {
            Subscription::none()
        };

        let tuner_sub = if self.tuner_handler.is_enabled() {
            time::every(TUNER_POLL_INTERVAL).map(|_| Message::Tuner(TunerMessage::Update))
        } else {
            Subscription::none()
        };

        let peak_meter_sub =
            time::every(PEAK_METER_POLL_INTERVAL).map(|_| Message::PeakMeterUpdate);

        let midi_sub = if self.midi_handler.is_visible()
            || self.midi_handler.get_selected_controller().is_some()
        {
            time::every(MIDI_POLL_INTERVAL).map(|_| Message::Midi(MidiMessage::Update))
        } else {
            Subscription::none()
        };

        let keyboard_sub = keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed {
                key,
                modifiers,
                repeat: false,
                ..
            } => Some(Message::KeyPressed(key, modifiers)),
            _ => None,
        });

        Subscription::batch(vec![
            rebuild_sub,
            tuner_sub,
            peak_meter_sub,
            midi_sub,
            keyboard_sub,
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetStages(stages) => {
                self.stages = stages;
                self.mark_stages_dirty();
            }
            Message::RebuildTick => self.rebuild_if_dirty(),
            Message::AddStage => {
                let new_stage = StageConfig::from(self.control_bar.selected());
                self.stages.push(new_stage);
                self.mark_stages_dirty();
            }
            Message::RemoveStage(idx) => {
                if idx < self.stages.len() {
                    self.stages.remove(idx);
                    self.mark_stages_dirty();
                }
            }
            Message::MoveStageUp(idx) => {
                if idx > 0 && idx < self.stages.len() {
                    self.stages.swap(idx - 1, idx);
                    self.mark_stages_dirty();
                }
            }
            Message::MoveStageDown(idx) => {
                if idx + 1 < self.stages.len() {
                    self.stages.swap(idx, idx + 1);
                    self.mark_stages_dirty();
                }
            }
            Message::StageTypeSelected(stage_type) => {
                self.control_bar.set_selected_stage_type(stage_type);
            }
            Message::StartRecording => {
                let sample_rate = self.audio_manager.sample_rate();
                let recording_dir = &self.settings.recording_dir;
                if let Err(e) = self
                    .audio_manager
                    .engine()
                    .start_recording(sample_rate, recording_dir)
                {
                    error!("Failed to start recording: {}", e);
                } else {
                    self.is_recording = true;
                    debug!("Recording started");
                }
            }
            Message::StopRecording => {
                self.audio_manager.engine().stop_recording();
                self.is_recording = false;
                debug!("Recording stopped");
            }
            Message::Settings(msg) => {
                return self.settings_handler.handle(
                    msg,
                    &mut self.settings,
                    &mut self.audio_manager,
                );
            }
            Message::IrSelected(ir_name) => {
                self.ir_cabinet_control
                    .set_selected_ir(Some(ir_name.clone()));
                self.audio_manager.engine().set_ir_cabinet(Some(ir_name));
            }
            Message::IrBypassed(bypassed) => {
                self.ir_cabinet_control.set_bypassed(bypassed);
                self.audio_manager.engine().set_ir_bypass(bypassed);
                self.settings.ir_bypassed = bypassed;
                self.save_settings();
            }
            Message::IrGainChanged(gain) => {
                self.ir_cabinet_control.set_gain(gain);
                self.audio_manager.engine().set_ir_gain(gain);
            }
            Message::PitchShiftChanged(semitones) => {
                self.pitch_shift_control.set_semitones(semitones);
                self.audio_manager.engine().set_pitch_shift(semitones);
            }
            Message::Stage(idx, stage_msg) => {
                if let Some(stage) = self.stages.get_mut(idx)
                    && stage.apply(stage_msg)
                {
                    self.mark_stages_dirty();
                }
            }
            Message::Tuner(msg) => {
                return self.tuner_handler.handle(msg, &self.audio_manager);
            }
            Message::Midi(ref msg) => {
                // Cross-cutting: persist settings changes for certain messages
                let needs_save = matches!(
                    msg,
                    MidiMessage::ControllerSelected(_)
                        | MidiMessage::Disconnect
                        | MidiMessage::ConfirmMapping
                        | MidiMessage::RemoveMapping(_)
                );

                let presets = self.preset_handler.get_available_presets();
                let mappings = self.settings.midi.mappings.clone();
                let task = self.midi_handler.handle(msg.clone(), presets, &mappings);

                if needs_save {
                    match msg {
                        MidiMessage::ControllerSelected(name) => {
                            self.settings.midi.controller_name = Some(name.clone());
                        }
                        MidiMessage::Disconnect => {
                            self.settings.midi.controller_name = None;
                        }
                        MidiMessage::ConfirmMapping | MidiMessage::RemoveMapping(_) => {
                            self.settings.midi.mappings = self.midi_handler.get_mappings();
                        }
                        _ => {}
                    }
                    self.save_settings();
                }

                return task;
            }
            Message::Hotkey(msg) => {
                let needs_save = matches!(
                    msg,
                    HotkeyMessage::ConfirmMapping | HotkeyMessage::RemoveMapping(_)
                );

                let presets = self.preset_handler.get_available_presets();
                let task = self.hotkey_handler.handle(msg, presets);

                if needs_save {
                    self.settings.hotkeys = self.hotkey_handler.settings().clone();
                    self.save_settings();
                }

                return task;
            }
            Message::KeyPressed(key, modifiers) => {
                // If hotkey dialog is in learning mode, capture the key
                if self.hotkey_handler.is_learning() {
                    self.hotkey_handler.on_key_input(&key, modifiers);
                    return Task::none();
                }

                // If any dialog is open, don't trigger hotkeys
                if self.any_dialog_visible() {
                    return Task::none();
                }

                // Check against hotkey mappings
                if let Some(preset_name) = self.hotkey_handler.check_mapping(&key, modifiers) {
                    debug!("Hotkey triggered preset: {}", preset_name);
                    return Task::done(Message::Preset(PresetMessage::Select(preset_name)));
                }
            }
            Message::PeakMeterUpdate => {
                let info = self.audio_manager.peak_meter().get_info();
                let xrun_count = self.audio_manager.xrun_count();
                let cpu_load = self.audio_manager.cpu_load();
                self.peak_meter_display.update(info, xrun_count, cpu_load);
            }
            Message::Preset(msg) => {
                match msg.clone() {
                    PresetMessage::Select(name) | PresetMessage::Save(name) => {
                        self.settings.selected_preset = Some(name.clone());
                        self.save_settings();
                    }
                    PresetMessage::Delete(deleted_name) => {
                        if self.settings.selected_preset == Some(deleted_name) {
                            self.settings.selected_preset = None;
                        }
                        self.save_settings();
                    }
                    _ => {}
                }

                return self.preset_handler.handle(
                    msg,
                    self.stages.clone(),
                    self.ir_cabinet_control.get_selected_ir(),
                    self.ir_cabinet_control.get_gain(),
                    self.pitch_shift_control.get_semitones(),
                );
            }
        }

        Task::none()
    }

    fn any_dialog_visible(&self) -> bool {
        self.settings_handler.is_visible()
            || self.tuner_handler.is_visible()
            || self.midi_handler.is_visible()
            || self.hotkey_handler.is_visible()
    }

    fn save_settings(&self) {
        if let Err(e) = self.settings.save() {
            error!("Failed to save settings: {e}");
        }
    }

    fn rebuild_if_dirty(&mut self) {
        if !self.dirty_chain {
            return;
        }
        self.update_processor_chain();
        self.dirty_chain = false;
    }

    fn mark_stages_dirty(&mut self) {
        self.dirty_chain = true;
        self.stage_list.set_stages(&self.stages);
    }

    fn update_processor_chain(&self) {
        let sample_rate = self.audio_manager.sample_rate();
        let chain = self.build_amplifier_chain(sample_rate);
        self.audio_manager.engine().set_amp_chain(chain);
    }

    fn build_amplifier_chain(&self, sample_rate: usize) -> AmplifierChain {
        let mut chain = AmplifierChain::new();

        let effective_sample_rate = sample_rate * self.settings.audio.oversampling_factor as usize;

        for cfg in &self.stages {
            chain.add_stage(cfg.to_runtime(effective_sample_rate as f32));
        }

        chain
    }
}
