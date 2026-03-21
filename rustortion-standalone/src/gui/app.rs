use std::collections::HashMap;

use iced::widget::container;
use iced::{Element, Length, Subscription, Task, Theme, time, time::Duration};
use log::{debug, error};

use crate::audio::manager::Manager;
use crate::backend::StandaloneBackend;
use crate::gui::handlers::midi::MidiHandler;
use crate::gui::handlers::settings::SettingsHandler;
use crate::gui::handlers::tuner::TunerHandler;
use crate::midi::start_midi_manager;
use crate::settings::Settings;
use rustortion_ui::app::{SharedApp, UpdateResult};
use rustortion_ui::backend::ParamBackend;
use rustortion_ui::components::ir_cabinet_control::IrCabinetControl;
use rustortion_ui::components::peak_meter::PeakMeterDisplay;
use rustortion_ui::components::pitch_shift_control::PitchShiftControl;
use rustortion_ui::handlers::hotkey::HotkeyHandler;
use rustortion_ui::handlers::preset::PresetHandler;
use rustortion_ui::i18n;
use rustortion_ui::messages::{HotkeyMessage, Message, MidiMessage, PresetMessage, TunerMessage};
use rustortion_ui::stages::StageType;
use rustortion_ui::tabs::Tab;

const TUNER_POLL_INTERVAL: Duration = Duration::from_millis(20);
const MIDI_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub struct AmplifierApp {
    shared: SharedApp<StandaloneBackend>,
    settings: Settings,
    settings_handler: SettingsHandler,
    tuner_handler: TunerHandler,
    midi_handler: MidiHandler,
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
            audio_manager.request_ir_load(&ir_name);
        } else if let Some(first_ir) = ir_cabinet_control.get_selected_ir() {
            ir_cabinet_control.set_selected_ir(Some(first_ir.clone()));
            audio_manager.request_ir_load(&first_ir);
        }

        // Preload IRs referenced by presets
        {
            let mut preset_ir_names: Vec<String> = preset_handler
                .get_available_presets()
                .iter()
                .filter_map(|name| {
                    preset_handler
                        .get_preset_by_name(name)
                        .and_then(|p| p.ir_name.clone())
                })
                .collect();
            preset_ir_names.sort();
            preset_ir_names.dedup();
            audio_manager.preload_irs(&preset_ir_names);
        }

        // Initialize MIDI
        let midi_handle = start_midi_manager();
        let mut midi_handler = MidiHandler::new(midi_handle);

        // Load MIDI mappings from settings
        midi_handler.set_mappings(settings.midi.mappings.clone());

        // Try to connect to saved MIDI controller
        if let Some(controller_name) = &settings.midi.controller_name {
            midi_handler.connect(controller_name);
            debug!("Attempting to reconnect to MIDI controller: {controller_name}");
        }

        // Set the global language from settings
        i18n::set_language(settings.language);

        let hotkey_handler = HotkeyHandler::new(settings.hotkeys.clone());

        // Sync settings with the actually loaded preset so collapse keys stay consistent
        let mut settings = settings;
        settings.selected_preset = Some(preset.name.clone());

        let collapsed_stages = Self::restore_collapsed(
            &settings.collapsed_stages,
            &preset.name,
            preset.stages.len(),
        );

        let input_filter_config = preset.input_filters;

        // Build the standalone backend
        let backend = StandaloneBackend::new(audio_manager);

        // Send initial input filters to engine
        backend.set_input_filter(&input_filter_config);

        // Build and send initial chain
        backend.set_amp_chain(&preset.stages);

        let shared = SharedApp {
            backend,
            stages: preset.stages,
            collapsed_stages,
            dirty_params: HashMap::new(),
            active_tab: Tab::default(),
            selected_stage_type: StageType::default(),
            ir_cabinet_control,
            pitch_shift_control,
            preset_handler,
            peak_meter_display: PeakMeterDisplay::new(),
            hotkey_handler,
            input_filter_config,
            is_recording: false,
        };

        (
            Self {
                shared,
                settings,
                settings_handler,
                tuner_handler: TunerHandler::new(),
                midi_handler,
            },
            Task::none(),
        )
    }

    pub fn view(&self) -> Element<'_, Message> {
        let main_content = self.shared.view();

        let dialogs = [
            self.settings_handler.view(),
            self.tuner_handler.view(),
            self.midi_handler.view(),
            self.shared.hotkey_handler.view(),
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

    pub const fn theme(&self) -> Theme {
        Theme::TokyoNight
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let shared_sub = self.shared.subscription();

        let tuner_sub = if self.tuner_handler.is_enabled() {
            time::every(TUNER_POLL_INTERVAL).map(|_| Message::Tuner(TunerMessage::Update))
        } else {
            Subscription::none()
        };

        let midi_sub = if self.midi_handler.is_visible()
            || self.midi_handler.get_selected_controller().is_some()
        {
            time::every(MIDI_POLL_INTERVAL).map(|_| Message::Midi(MidiMessage::Update))
        } else {
            Subscription::none()
        };

        Subscription::batch(vec![shared_sub, tuner_sub, midi_sub])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Detect messages that need standalone-side persistence *before*
        // shared handles them (because shared may consume the variant).
        let needs_collapse_persist = matches!(
            message,
            Message::AddStage
                | Message::RemoveStage(_)
                | Message::MoveStageUp(_)
                | Message::MoveStageDown(_)
                | Message::ToggleStageCollapse(_)
                | Message::ToggleAllStagesCollapse
        );

        let needs_ir_bypass_persist = matches!(message, Message::IrBypassed(_));
        let ir_bypassed_value = if let Message::IrBypassed(b) = &message {
            Some(*b)
        } else {
            None
        };

        let needs_hotkey_save = matches!(
            message,
            Message::Hotkey(HotkeyMessage::ConfirmMapping | HotkeyMessage::RemoveMapping(_))
        );

        let is_preset_select_or_save = matches!(
            message,
            Message::Preset(PresetMessage::Select(_) | PresetMessage::Save(_))
        );
        let is_preset_delete = matches!(message, Message::Preset(PresetMessage::Delete(_)));

        // Clone preset name for persistence if needed
        let preset_name_for_persist = match &message {
            Message::Preset(PresetMessage::Select(name) | PresetMessage::Save(name)) => {
                Some(name.clone())
            }
            _ => None,
        };
        let deleted_preset_name = match &message {
            Message::Preset(PresetMessage::Delete(name)) => Some(name.clone()),
            _ => None,
        };

        // Block key events when standalone dialogs are open
        if matches!(message, Message::KeyPressed(..)) && self.any_dialog_visible() {
            return Task::none();
        }

        // Handle SetStages with collapse state restoration from settings
        if let Message::SetStages(ref stages) = message
            && let Some(preset_name) = self.settings.selected_preset.as_deref()
        {
            self.shared.collapsed_stages =
                Self::restore_collapsed(&self.settings.collapsed_stages, preset_name, stages.len());
        }

        // Try shared update first
        let task = match self.shared.update(message) {
            UpdateResult::Handled(task) => task,
            UpdateResult::Unhandled(msg) => self.handle_standalone(msg),
        };

        // Post-update persistence
        if needs_collapse_persist {
            self.persist_collapse_state();
        }

        if needs_ir_bypass_persist && let Some(bypassed) = ir_bypassed_value {
            self.settings.ir_bypassed = bypassed;
            self.save_settings();
        }

        if needs_hotkey_save {
            self.settings.hotkeys = self.shared.hotkey_handler.settings().clone();
            self.save_settings();
        }

        if is_preset_select_or_save && let Some(name) = preset_name_for_persist {
            self.settings.selected_preset = Some(name);
            self.save_settings();
        }

        if is_preset_delete && let Some(deleted_name) = deleted_preset_name {
            self.settings.collapsed_stages.remove(&deleted_name);
            if self.settings.selected_preset == Some(deleted_name) {
                self.settings.selected_preset = None;
            }
            self.save_settings();
        }

        task
    }

    /// Handle standalone-only messages.
    fn handle_standalone(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::StartRecording => {
                let sample_rate = self.shared.backend.manager().sample_rate();
                let recording_dir = &self.settings.recording_dir;
                if let Err(e) = self
                    .shared
                    .backend
                    .manager()
                    .engine()
                    .start_recording(sample_rate, recording_dir)
                {
                    error!("Failed to start recording: {e}");
                } else {
                    self.shared.is_recording = true;
                    debug!("Recording started");
                }
            }
            Message::StopRecording => {
                self.shared.backend.manager().engine().stop_recording();
                self.shared.is_recording = false;
                debug!("Recording stopped");
            }
            Message::Settings(msg) => {
                let old_oversampling = self.settings.audio.oversampling_factor;
                let task = self.settings_handler.handle(
                    msg,
                    &mut self.settings,
                    self.shared.backend.manager_mut(),
                );
                if self.settings.audio.oversampling_factor != old_oversampling {
                    self.shared.dirty_params.clear();
                    self.shared.backend.set_amp_chain(&self.shared.stages);
                }
                return task;
            }
            Message::Tuner(msg) => {
                return self
                    .tuner_handler
                    .handle(msg, self.shared.backend.manager());
            }
            Message::Midi(msg) => return self.handle_midi(msg),
            other => {
                debug!("Unhandled message: {other:?}");
            }
        }

        Task::none()
    }

    fn handle_midi(&mut self, msg: MidiMessage) -> Task<Message> {
        if matches!(msg, MidiMessage::Open) {
            let presets = self.shared.preset_handler.get_available_presets().to_vec();
            let mappings = self.settings.midi.mappings.clone();
            self.midi_handler.open(presets, mappings);
            return Task::none();
        }

        let controller_update = match &msg {
            MidiMessage::ControllerSelected(name) => Some(Some(name.clone())),
            MidiMessage::Disconnect => Some(None),
            _ => None,
        };
        let save_mappings = matches!(
            msg,
            MidiMessage::ConfirmMapping | MidiMessage::RemoveMapping(_)
        );

        let task = self.midi_handler.handle(msg);

        if let Some(name) = controller_update {
            self.settings.midi.controller_name = name;
            self.save_settings();
        } else if save_mappings {
            self.settings.midi.mappings = self.midi_handler.get_mappings();
            self.save_settings();
        }

        task
    }

    const fn any_dialog_visible(&self) -> bool {
        self.settings_handler.is_visible()
            || self.tuner_handler.is_visible()
            || self.midi_handler.is_visible()
            || self.shared.hotkey_handler.is_visible()
    }

    fn persist_collapse_state(&mut self) {
        let Some(key) = self.settings.selected_preset.clone() else {
            return;
        };
        let saved = self.settings.collapsed_stages.get(&key);
        if saved != Some(&self.shared.collapsed_stages) {
            self.settings
                .collapsed_stages
                .insert(key, self.shared.collapsed_stages.clone());
            self.save_settings();
        }
    }

    fn restore_collapsed(
        saved: &std::collections::HashMap<String, Vec<bool>>,
        preset_name: &str,
        stage_count: usize,
    ) -> Vec<bool> {
        let mut result = saved.get(preset_name).cloned().unwrap_or_default();
        result.resize(stage_count, false);
        result
    }

    fn save_settings(&self) {
        if let Err(e) = self.settings.save() {
            error!("Failed to save settings: {e}");
        }
    }
}
