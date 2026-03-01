use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, slider, space, text,
};
use iced::{Alignment, Element, Length, Subscription, Task, Theme, keyboard, time, time::Duration};
use log::{debug, error};

use crate::amp::chain::AmplifierChain;
use crate::amp::stages::filter::{FilterStage, FilterType};
use crate::audio::manager::Manager;
use crate::gui::components::input_filter_control::InputFilterConfig;
use crate::gui::components::ir_cabinet_control::IrCabinetControl;
use crate::gui::components::minimap;
use crate::gui::components::peak_meter::PeakMeterDisplay;
use crate::gui::components::pitch_shift_control::PitchShiftControl;
use crate::gui::components::widgets::common::{
    COLOR_ERROR, PADDING_LARGE, PADDING_NORMAL, SPACING_NORMAL, SPACING_TIGHT, TAB_BUTTON_PADDING,
    TEXT_SIZE_TAB, section_container, section_title,
};
use crate::gui::handlers::hotkey::HotkeyHandler;
use crate::gui::handlers::midi::MidiHandler;
use crate::gui::handlers::preset::PresetHandler;
use crate::gui::handlers::settings::SettingsHandler;
use crate::gui::handlers::tuner::TunerHandler;
use crate::gui::messages::{
    HotkeyMessage, Message, MidiMessage, PresetMessage, SettingsMessage, TunerMessage,
};
use crate::gui::stages::{StageCategory, StageConfig, StageType};
use crate::gui::tabs::Tab;
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
    active_tab: Tab,
    input_filter_config: InputFilterConfig,
    selected_stage_type: StageType,
    settings: Settings,
    settings_handler: SettingsHandler,
    collapsed_stages: Vec<bool>,
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

        // Send initial input filters to engine before constructing app
        {
            let sample_rate = audio_manager.sample_rate() as f32;
            let hp: Option<Box<dyn crate::amp::stages::Stage>> = if input_filter_config.hp_enabled {
                Some(Box::new(FilterStage::new(
                    FilterType::Highpass,
                    input_filter_config.hp_cutoff,
                    sample_rate,
                )))
            } else {
                None
            };
            let lp: Option<Box<dyn crate::amp::stages::Stage>> = if input_filter_config.lp_enabled {
                Some(Box::new(FilterStage::new(
                    FilterType::Lowpass,
                    input_filter_config.lp_cutoff,
                    sample_rate,
                )))
            } else {
                None
            };
            audio_manager.engine().set_input_filters(hp, lp);
        }

        (
            Self {
                audio_manager,
                stages: preset.stages,
                is_recording: false,
                active_tab: Tab::default(),
                input_filter_config,
                selected_stage_type: StageType::default(),
                settings,
                settings_handler,
                collapsed_stages,
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
        let header = self.view_header();
        let tab_bar = self.view_tab_bar();
        let tab_content = match self.active_tab {
            Tab::Amp => self.view_amp_tab(),
            Tab::Effects => self.view_effects_tab(),
            Tab::Cabinet => self.view_cabinet_tab(),
            Tab::Io => self.view_io_tab(),
        };
        let signal_minimap =
            minimap::view(&self.stages, &self.input_filter_config, self.active_tab);
        let footer =
            row![self.peak_meter_display.view_status(), signal_minimap,].align_y(Alignment::Center);

        let main_content = column![
            header,
            self.preset_handler.view(),
            tab_bar,
            tab_content,
            footer,
        ]
        .spacing(SPACING_NORMAL)
        .padding(PADDING_LARGE);

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

    fn view_header(&self) -> Element<'_, Message> {
        let record_button = if self.is_recording {
            button(text(tr!(stop_recording)))
                .on_press(Message::StopRecording)
                .style(iced::widget::button::danger)
        } else {
            button(text(tr!(start_recording)))
                .on_press(Message::StartRecording)
                .style(iced::widget::button::success)
        };

        let recording_status = if self.is_recording {
            text(tr!(recording)).style(|_| iced::widget::text::Style {
                color: Some(COLOR_ERROR),
            })
        } else {
            text("").style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            })
        };

        row![
            self.peak_meter_display.view(),
            space::horizontal(),
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
            record_button,
            recording_status,
        ]
        .spacing(SPACING_TIGHT)
        .align_y(Alignment::Center)
        .into()
    }

    fn view_tab_bar(&self) -> Element<'_, Message> {
        let tabs = [
            (Tab::Io, tr!(tab_io)),
            (Tab::Amp, tr!(tab_amp)),
            (Tab::Effects, tr!(tab_effects)),
            (Tab::Cabinet, tr!(tab_cabinet)),
        ];

        let mut tab_row = row![].spacing(SPACING_TIGHT);

        for (tab, label) in tabs {
            let is_active = self.active_tab == tab;
            let btn = button(text(label).size(TEXT_SIZE_TAB))
                .on_press(Message::TabSelected(tab))
                .padding(TAB_BUTTON_PADDING)
                .style(if is_active {
                    tab_button_active
                } else {
                    tab_button_inactive
                });
            tab_row = tab_row.push(btn);
        }

        container(tab_row)
            .width(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    fn view_amp_tab(&self) -> Element<'_, Message> {
        self.view_stage_tab(StageCategory::Amp)
    }

    fn view_effects_tab(&self) -> Element<'_, Message> {
        self.view_stage_tab(StageCategory::Effect)
    }

    fn view_stage_tab(&self, category: StageCategory) -> Element<'_, Message> {
        let category_indices: Vec<usize> = self
            .stages
            .iter()
            .enumerate()
            .filter(|(_, s)| s.category() == category)
            .map(|(i, _)| i)
            .collect();

        let total_in_category = category_indices.len();

        // Collapse/expand toggle above stages
        let collapse_toggle = self.view_collapse_toggle(category);

        let mut stage_col = column![].width(Length::Fill).spacing(SPACING_TIGHT);
        for (pos, &abs_idx) in category_indices.iter().enumerate() {
            let is_collapsed = self.collapsed_stages.get(abs_idx).copied().unwrap_or(false);
            let can_move_up = pos > 0;
            let can_move_down = pos < total_in_category.saturating_sub(1);
            stage_col = stage_col.push(self.stages[abs_idx].view(
                abs_idx,
                is_collapsed,
                can_move_up,
                can_move_down,
            ));
        }

        // Add-stage picker at the bottom
        let add_bar = self.view_add_stage_bar(category);

        let content = column![
            collapse_toggle,
            scrollable(stage_col.padding(PADDING_NORMAL)).height(Length::Fill),
            add_bar,
        ]
        .spacing(SPACING_TIGHT);

        view_tab_panel(content.into())
    }

    fn view_collapse_toggle(&self, category: StageCategory) -> Element<'_, Message> {
        let category_has_stages = self.stages.iter().any(|s| s.category() == category);
        let all_collapsed = category_has_stages
            && self
                .stages
                .iter()
                .enumerate()
                .filter(|(_, s)| s.category() == category)
                .all(|(i, _)| self.collapsed_stages.get(i).copied().unwrap_or(false));

        let label = if all_collapsed {
            format!("▼ {}", tr!(expand_all))
        } else {
            format!("▶ {}", tr!(collapse_all))
        };

        row![
            button(text(label))
                .on_press(Message::ToggleAllStagesCollapse)
                .style(iced::widget::button::secondary),
        ]
        .into()
    }

    fn view_add_stage_bar(&self, category: StageCategory) -> Element<'_, Message> {
        let available_types = StageType::for_category(category);
        let selected = if self.selected_stage_type.category() == category {
            Some(self.selected_stage_type)
        } else {
            available_types.first().copied()
        };

        row![
            pick_list(available_types, selected, Message::StageTypeSelected),
            button(tr!(add_stage)).on_press(Message::AddStage),
        ]
        .spacing(SPACING_NORMAL)
        .align_y(Alignment::Center)
        .into()
    }

    fn view_cabinet_tab(&self) -> Element<'_, Message> {
        let content = scrollable(
            column![self.ir_cabinet_control.view()]
                .width(Length::Fill)
                .padding(PADDING_NORMAL),
        )
        .height(Length::Fill);

        view_tab_panel(content.into())
    }

    fn view_io_tab(&self) -> Element<'_, Message> {
        let hp_section = column![
            checkbox(self.input_filter_config.hp_enabled)
                .label(tr!(highpass))
                .on_toggle(Message::InputFilterHighpassToggle),
            row![
                text(tr!(cutoff)).width(Length::FillPortion(3)),
                slider(
                    0.0..=1000.0,
                    self.input_filter_config.hp_cutoff,
                    Message::InputFilterHighpassCutoff
                )
                .width(Length::FillPortion(5))
                .step(1.0),
                text(format!(
                    "{:.0} {}",
                    self.input_filter_config.hp_cutoff,
                    tr!(hz)
                ))
                .width(Length::FillPortion(2)),
            ]
            .spacing(SPACING_NORMAL)
            .align_y(Alignment::Center),
        ]
        .spacing(SPACING_TIGHT);

        let lp_section = column![
            checkbox(self.input_filter_config.lp_enabled)
                .label(tr!(lowpass))
                .on_toggle(Message::InputFilterLowpassToggle),
            row![
                text(tr!(cutoff)).width(Length::FillPortion(3)),
                slider(
                    1000.0..=20000.0,
                    self.input_filter_config.lp_cutoff,
                    Message::InputFilterLowpassCutoff
                )
                .width(Length::FillPortion(5))
                .step(1.0),
                text(format!(
                    "{:.0} {}",
                    self.input_filter_config.lp_cutoff,
                    tr!(hz)
                ))
                .width(Length::FillPortion(2)),
            ]
            .spacing(SPACING_NORMAL)
            .align_y(Alignment::Center),
        ]
        .spacing(SPACING_TIGHT);

        let input_filters_section = section_container(
            column![section_title(tr!(input_filters)), hp_section, lp_section,]
                .spacing(SPACING_NORMAL)
                .into(),
        );

        let pitch_section = section_container(
            column![
                section_title(tr!(pitch_shift)),
                self.pitch_shift_control.view(),
            ]
            .spacing(SPACING_NORMAL)
            .into(),
        );

        let content = scrollable(
            column![input_filters_section, pitch_section,]
                .spacing(SPACING_NORMAL)
                .padding(PADDING_NORMAL),
        )
        .height(Length::Fill);

        view_tab_panel(content.into())
    }

    pub const fn theme(&self) -> Theme {
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
            Message::TabSelected(tab) => {
                self.active_tab = tab;
                self.sync_stage_type_with_tab(tab);
            }
            Message::SetStages(stages) => {
                if let Some(preset_name) = self.settings.selected_preset.as_deref() {
                    self.collapsed_stages = Self::restore_collapsed(
                        &self.settings.collapsed_stages,
                        preset_name,
                        stages.len(),
                    );
                } else {
                    self.collapsed_stages.resize(stages.len(), false);
                }
                self.stages = stages;
                self.mark_stages_dirty();
            }
            Message::SetInputFilters(config) => {
                self.input_filter_config = config;
                self.send_input_filters_to_engine();
            }
            Message::InputFilterHighpassToggle(enabled) => {
                self.input_filter_config.hp_enabled = enabled;
                self.send_input_filters_to_engine();
            }
            Message::InputFilterHighpassCutoff(cutoff) => {
                self.input_filter_config.hp_cutoff = cutoff;
                self.send_input_filters_to_engine();
            }
            Message::InputFilterLowpassToggle(enabled) => {
                self.input_filter_config.lp_enabled = enabled;
                self.send_input_filters_to_engine();
            }
            Message::InputFilterLowpassCutoff(cutoff) => {
                self.input_filter_config.lp_cutoff = cutoff;
                self.send_input_filters_to_engine();
            }
            Message::RebuildTick => self.rebuild_if_dirty(),
            Message::AddStage => {
                let new_stage = StageConfig::from(self.selected_stage_type);
                let category = new_stage.category();

                // Insert at end of same-category section
                let insert_idx = self.category_end_index(category);
                self.stages.insert(insert_idx, new_stage);
                self.collapsed_stages.insert(insert_idx, false);
                self.mark_stages_dirty();
                self.persist_collapse_state();
            }
            Message::RemoveStage(idx) => {
                if idx < self.stages.len() {
                    self.stages.remove(idx);
                    self.collapsed_stages.remove(idx);
                    self.mark_stages_dirty();
                    self.persist_collapse_state();
                }
            }
            Message::MoveStageUp(idx) => {
                if idx < self.stages.len() {
                    let category = self.stages[idx].category();
                    // Find previous stage of same category
                    if let Some(prev) = (0..idx)
                        .rev()
                        .find(|&i| self.stages[i].category() == category)
                    {
                        self.stages.swap(prev, idx);
                        self.collapsed_stages.swap(prev, idx);
                        self.mark_stages_dirty();
                        self.persist_collapse_state();
                    }
                }
            }
            Message::MoveStageDown(idx) => {
                if idx < self.stages.len() {
                    let category = self.stages[idx].category();
                    // Find next stage of same category
                    if let Some(next) = (idx + 1..self.stages.len())
                        .find(|&i| self.stages[i].category() == category)
                    {
                        self.stages.swap(idx, next);
                        self.collapsed_stages.swap(idx, next);
                        self.mark_stages_dirty();
                        self.persist_collapse_state();
                    }
                }
            }
            Message::ToggleStageCollapse(idx) => {
                if let Some(collapsed) = self.collapsed_stages.get_mut(idx) {
                    *collapsed = !*collapsed;
                }
                self.persist_collapse_state();
            }
            Message::ToggleAllStagesCollapse => {
                // Only affect stages of the current tab's category
                if let Some(cat) = self.active_tab.stage_category() {
                    let category_indices: Vec<usize> = self
                        .stages
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| s.category() == cat)
                        .map(|(i, _)| i)
                        .collect();

                    let any_expanded = category_indices
                        .iter()
                        .any(|&i| !self.collapsed_stages.get(i).copied().unwrap_or(false));

                    for &i in &category_indices {
                        if let Some(c) = self.collapsed_stages.get_mut(i) {
                            *c = any_expanded;
                        }
                    }
                    self.persist_collapse_state();
                }
            }
            Message::StageTypeSelected(stage_type) => {
                self.selected_stage_type = stage_type;
            }
            Message::StartRecording => {
                let sample_rate = self.audio_manager.sample_rate();
                let recording_dir = &self.settings.recording_dir;
                if let Err(e) = self
                    .audio_manager
                    .engine()
                    .start_recording(sample_rate, recording_dir)
                {
                    error!("Failed to start recording: {e}");
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
            Message::Midi(msg) => return self.handle_midi(msg),
            Message::Hotkey(msg) => return self.handle_hotkey(msg),
            Message::KeyPressed(key, modifiers) => {
                return self.handle_key_pressed(&key, modifiers);
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
                        self.settings.selected_preset = Some(name);
                        self.save_settings();
                    }
                    PresetMessage::Delete(deleted_name) => {
                        self.settings.collapsed_stages.remove(&deleted_name);
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
                    self.input_filter_config,
                );
            }
        }

        Task::none()
    }

    fn handle_midi(&mut self, msg: MidiMessage) -> Task<Message> {
        if matches!(msg, MidiMessage::Open) {
            let presets = self.preset_handler.get_available_presets();
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

    fn handle_hotkey(&mut self, msg: HotkeyMessage) -> Task<Message> {
        if matches!(msg, HotkeyMessage::Open) {
            let presets = self.preset_handler.get_available_presets();
            self.hotkey_handler.open(presets);
            return Task::none();
        }

        let needs_save = matches!(
            msg,
            HotkeyMessage::ConfirmMapping | HotkeyMessage::RemoveMapping(_)
        );

        let task = self.hotkey_handler.handle(msg);

        if needs_save {
            self.settings.hotkeys = self.hotkey_handler.settings().clone();
            self.save_settings();
        }

        task
    }

    fn handle_key_pressed(
        &mut self,
        key: &iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    ) -> Task<Message> {
        if self.hotkey_handler.is_learning() {
            self.hotkey_handler.on_key_input(key, modifiers);
            return Task::none();
        }

        if self.any_dialog_visible() {
            return Task::none();
        }

        if let Some(preset_name) = self.hotkey_handler.check_mapping(key, modifiers) {
            debug!("Hotkey triggered preset: {preset_name}");
            return Task::done(Message::Preset(PresetMessage::Select(preset_name)));
        }

        Task::none()
    }

    /// Reset `selected_stage_type` to the first stage of the new tab's category.
    fn sync_stage_type_with_tab(&mut self, tab: Tab) {
        let Some(category) = tab.stage_category() else {
            return;
        };

        if self.selected_stage_type.category() == category {
            return;
        }

        if let Some(first) = StageType::ALL
            .iter()
            .copied()
            .find(|s| s.category() == category)
        {
            self.selected_stage_type = first;
        }
    }

    /// Find the index after the last stage of the given category.
    fn category_end_index(&self, category: StageCategory) -> usize {
        match category {
            StageCategory::Amp => {
                // Amp stages come first; find the last amp stage
                self.stages
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.category() == StageCategory::Amp)
                    .map(|(i, _)| i + 1)
                    .next_back()
                    .unwrap_or(0)
            }
            StageCategory::Effect => {
                // Effect stages come after amp stages; append at end
                self.stages.len()
            }
        }
    }

    const fn any_dialog_visible(&self) -> bool {
        self.settings_handler.is_visible()
            || self.tuner_handler.is_visible()
            || self.midi_handler.is_visible()
            || self.hotkey_handler.is_visible()
    }

    fn persist_collapse_state(&mut self) {
        let Some(key) = self.settings.selected_preset.clone() else {
            return;
        };
        let saved = self.settings.collapsed_stages.get(&key);
        if saved != Some(&self.collapsed_stages) {
            self.settings
                .collapsed_stages
                .insert(key, self.collapsed_stages.clone());
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

    fn rebuild_if_dirty(&mut self) {
        if !self.dirty_chain {
            return;
        }
        self.update_processor_chain();
        self.dirty_chain = false;
    }

    const fn mark_stages_dirty(&mut self) {
        self.dirty_chain = true;
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

    fn send_input_filters_to_engine(&self) {
        let sample_rate = self.audio_manager.sample_rate() as f32;

        let hp: Option<Box<dyn crate::amp::stages::Stage>> = if self.input_filter_config.hp_enabled
        {
            Some(Box::new(FilterStage::new(
                FilterType::Highpass,
                self.input_filter_config.hp_cutoff,
                sample_rate,
            )))
        } else {
            None
        };

        let lp: Option<Box<dyn crate::amp::stages::Stage>> = if self.input_filter_config.lp_enabled
        {
            Some(Box::new(FilterStage::new(
                FilterType::Lowpass,
                self.input_filter_config.lp_cutoff,
                sample_rate,
            )))
        } else {
            None
        };

        self.audio_manager.engine().set_input_filters(hp, lp);
    }
}

/// Shared container for all tab content panels — consistent sizing and structure.
fn view_tab_panel(content: Element<'_, Message>) -> Element<'_, Message> {
    use iced::widget::container;

    container(content)
        .width(Length::Fill)
        .height(Length::FillPortion(9))
        .into()
}

fn tab_button_active(
    theme: &iced::Theme,
    _status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let palette = theme.palette();
    iced::widget::button::Style {
        text_color: palette.text,
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            palette.primary.r,
            palette.primary.g,
            palette.primary.b,
            0.3,
        ))),
        border: iced::Border {
            color: palette.primary,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..iced::widget::button::Style::default()
    }
}

fn tab_button_inactive(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let palette = theme.palette();
    let bg_alpha = if matches!(status, iced::widget::button::Status::Hovered) {
        0.15
    } else {
        0.05
    };
    iced::widget::button::Style {
        text_color: iced::Color::from_rgba(palette.text.r, palette.text.g, palette.text.b, 0.6),
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, bg_alpha,
        ))),
        border: iced::Border {
            color: iced::Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..iced::widget::button::Style::default()
    }
}
