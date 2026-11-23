use std::collections::HashMap;

use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, slider, space, text,
};
use iced::{Alignment, Element, Length, Subscription, Task, keyboard, time, time::Duration};

use crate::backend::{ExternalEvent, ParamBackend};
use crate::components::ir_cabinet_control::IrCabinetControl;
use crate::components::minimap;
use crate::components::nam_browser::selection::ChainEdit;
use crate::components::nam_browser::{NamBrowser, NamViewContext};
use crate::components::peak_meter::PeakMeterDisplay;
use crate::components::pitch_shift_control::PitchShiftControl;
use crate::components::widgets::common::{
    PADDING_LARGE, PADDING_NORMAL, SPACING_NORMAL, SPACING_TIGHT, StageViewState,
    TAB_BUTTON_PADDING, TEXT_SIZE_TAB, section_container, section_title,
};
use crate::handlers::hotkey::HotkeyHandler;
use crate::handlers::preset::PresetHandler;
use crate::messages::{HotkeyMessage, Message, NamTabMessage, PresetMessage};
use crate::stages::NamMessage;
use crate::stages::{
    ParamUpdate, StageCategory, StageConfig, StageMessage, StageType, apply_stage_config,
    view_stage_config,
};
use crate::tabs::Tab;
use crate::tr;
use rustortion_core::amp::chain::DEFAULT_CHAIN_CAPACITY;
use rustortion_core::amp::stages::nam::NamConfig;
use rustortion_core::preset::InputFilterConfig;

const REBUILD_INTERVAL: Duration = Duration::from_millis(100);
const PEAK_METER_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Result of `SharedApp::update()` — either handled (with a task) or unhandled
/// (the message is returned so the outer shell can process it).
pub enum UpdateResult {
    Handled(Task<Message>),
    Unhandled(Message),
}

/// Shared application state that is common across standalone and plugin GUIs.
/// Generic over the audio backend (`B: ParamBackend`).
pub struct SharedApp<B: ParamBackend> {
    pub backend: B,
    pub stages: Vec<StageConfig>,
    pub collapsed_stages: Vec<bool>,
    pub dirty_params: HashMap<(usize, &'static str), f32>,
    pub active_tab: Tab,
    pub selected_stage_type: StageType,
    pub ir_cabinet_control: IrCabinetControl,
    pub pitch_shift_control: PitchShiftControl,
    pub preset_handler: PresetHandler,
    pub peak_meter_display: PeakMeterDisplay,
    pub hotkey_handler: HotkeyHandler,
    pub input_filter_config: InputFilterConfig,
    pub oversampling_factor: u32,
    /// Whether recording is active — set by standalone, displayed in header.
    pub is_recording: bool,
    pub nam_browser: NamBrowser,
}

impl<B: ParamBackend> SharedApp<B> {
    pub fn update(&mut self, message: Message) -> UpdateResult {
        match message {
            Message::TabSelected(tab) => {
                self.active_tab = tab;
                self.sync_stage_type_with_tab(tab);
                // Pin the implicit "first NAM stage" so a later reorder can't change what is shown.
                if tab == Tab::Nam
                    && let Some(idx) = self.nam_browser.focused_stage(&self.stages)
                {
                    self.nam_browser.focus_stage(idx);
                }
            }
            Message::SetStages(stages) => {
                self.collapsed_stages.resize(stages.len(), false);
                self.stages = stages;
                self.dirty_params.clear();
                self.nam_browser.chain_edited(ChainEdit::Replaced);
                self.update_processor_chain();
                self.backend.persist_chain_state(&self.stages);
            }
            Message::SetInputFilters(config) => {
                self.input_filter_config = config;
                self.backend.set_input_filter(&self.input_filter_config);
            }
            Message::InputFilterHighpassToggle(enabled) => {
                self.input_filter_config.hp_enabled = enabled;
                self.backend.set_input_filter(&self.input_filter_config);
            }
            Message::InputFilterHighpassCutoff(cutoff) => {
                self.input_filter_config.hp_cutoff = cutoff;
                self.backend.set_input_filter(&self.input_filter_config);
            }
            Message::InputFilterLowpassToggle(enabled) => {
                self.input_filter_config.lp_enabled = enabled;
                self.backend.set_input_filter(&self.input_filter_config);
            }
            Message::InputFilterLowpassCutoff(cutoff) => {
                self.input_filter_config.lp_cutoff = cutoff;
                self.backend.set_input_filter(&self.input_filter_config);
            }
            Message::RebuildTick => self.flush_dirty_params(),
            Message::AddStage => {
                self.insert_stage(StageConfig::from(self.selected_stage_type));
            }
            Message::RemoveStage(idx) => {
                if idx < self.stages.len() {
                    self.flush_dirty_params();
                    self.stages.remove(idx);
                    self.collapsed_stages.remove(idx);
                    self.nam_browser.chain_edited(ChainEdit::Removed(idx));
                    self.backend.remove_stage(idx);
                    self.backend.persist_chain_state(&self.stages);
                }
            }
            Message::MoveStageUp(idx) => {
                if idx < self.stages.len() {
                    let category = self.stages[idx].category();
                    if let Some(prev) = (0..idx)
                        .rev()
                        .find(|&i| self.stages[i].category() == category)
                    {
                        self.flush_dirty_params();
                        self.stages.swap(prev, idx);
                        self.collapsed_stages.swap(prev, idx);
                        self.nam_browser.chain_edited(ChainEdit::Swapped(prev, idx));
                        self.backend.swap_stages(prev, idx);
                        self.backend.persist_chain_state(&self.stages);
                    }
                }
            }
            Message::MoveStageDown(idx) => {
                if idx < self.stages.len() {
                    let category = self.stages[idx].category();
                    if let Some(next) = (idx + 1..self.stages.len())
                        .find(|&i| self.stages[i].category() == category)
                    {
                        self.flush_dirty_params();
                        self.stages.swap(idx, next);
                        self.collapsed_stages.swap(idx, next);
                        self.nam_browser.chain_edited(ChainEdit::Swapped(idx, next));
                        self.backend.swap_stages(idx, next);
                        self.backend.persist_chain_state(&self.stages);
                    }
                }
            }
            Message::ToggleStageCollapse(idx) => {
                if let Some(collapsed) = self.collapsed_stages.get_mut(idx) {
                    *collapsed = !*collapsed;
                }
            }
            Message::ToggleAllStagesCollapse => {
                if let Some(cat) = self.active_tab.stage_category() {
                    let category_indices: Vec<usize> = self
                        .stages
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| s.category() == cat && !matches!(s, StageConfig::Nam(_)))
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
                }
            }
            Message::ToggleStageBypass(idx) => {
                if let Some(stage) = self.stages.get_mut(idx) {
                    let new_state = !stage.bypassed();
                    stage.set_bypassed(new_state);
                    self.backend.set_bypass(idx, new_state);
                    self.backend.persist_chain_state(&self.stages);
                }
            }
            Message::StageTypeSelected(stage_type) => {
                self.selected_stage_type = stage_type;
            }
            Message::IrSelected(ir_name) => {
                self.ir_cabinet_control
                    .set_selected_ir(Some(ir_name.clone()));
                self.backend.set_ir(&ir_name);
            }
            Message::IrBypassed(bypassed) => {
                self.ir_cabinet_control.set_bypassed(bypassed);
                self.backend.set_ir_bypass(bypassed);
            }
            Message::IrGainChanged(gain) => {
                self.ir_cabinet_control.set_gain(gain);
                self.backend.set_ir_gain(gain);
            }
            Message::PitchShiftChanged(semitones) => {
                self.pitch_shift_control.set_semitones(semitones);
                self.backend.set_pitch_shift(semitones);
            }
            Message::OversamplingChanged(factor) => {
                self.oversampling_factor = factor;
                self.backend.set_oversampling(factor);
                self.flush_dirty_params();
                self.backend.set_amp_chain(&self.stages);
            }
            Message::Stage(idx, stage_msg) => {
                if let Some(stage) = self.stages.get_mut(idx) {
                    match apply_stage_config(stage, stage_msg) {
                        Some(ParamUpdate::Changed(name, value)) => {
                            self.dirty_params.insert((idx, name), value);
                            self.backend.persist_chain_state(&self.stages);
                        }
                        Some(ParamUpdate::NeedsStageRebuild) => {
                            self.flush_dirty_params();
                            self.backend.rebuild_stage(idx, &self.stages[idx]);
                            self.backend.persist_chain_state(&self.stages);
                        }
                        None => {}
                    }
                }
            }
            Message::Nam(msg) => self.handle_nam(msg),
            Message::Hotkey(msg) => return self.handle_hotkey(msg),
            Message::KeyPressed(key, modifiers) => {
                return self.handle_key_pressed(&key, modifiers);
            }
            Message::PeakMeterUpdate => {
                if let Some(ExternalEvent::PeakMeterUpdate {
                    info,
                    xrun_count,
                    cpu_load,
                }) = self.backend.get_peak_meter_info()
                {
                    self.peak_meter_display.update(info, xrun_count, cpu_load);
                }
            }
            Message::Preset(msg) => {
                let task = self.preset_handler.handle(
                    msg,
                    self.stages.clone(),
                    self.ir_cabinet_control.get_selected_ir(),
                    self.ir_cabinet_control.get_gain(),
                    self.pitch_shift_control.get_semitones(),
                    self.input_filter_config,
                );
                // Notify backend of the new preset index for DAW state persistence
                if let Some(idx) = self.preset_handler.selected_preset_index() {
                    self.backend.set_preset_index(idx);
                }
                return UpdateResult::Handled(task);
            }
            other => return UpdateResult::Unhandled(other),
        }

        UpdateResult::Handled(Task::none())
    }

    fn handle_nam(&mut self, msg: NamTabMessage) {
        match msg {
            NamTabMessage::FilterChanged(filter) => self.nam_browser.set_filter(filter),
            NamTabMessage::FolderToggled(path) => self.nam_browser.toggle_folder(path),
            NamTabMessage::StageFocused(idx) => self.nam_browser.focus_stage(idx),
            NamTabMessage::OpenStage(idx) => {
                self.nam_browser.focus_stage(idx);
                self.active_tab = Tab::Nam;
            }
            NamTabMessage::ModelClicked(key) => self.load_model(key),
            NamTabMessage::Rescan => {
                // The browser reads the registry live each frame, so only the
                // already-built stages need refreshing.
                match self.backend.rescan_nam_models() {
                    Ok(count) => {
                        log::info!("Rescanned NAM models: {count} found");
                        // Built NamStages own their model: rebuild them so an
                        // in-place `.nam` edit takes effect.
                        for idx in 0..self.stages.len() {
                            if self.stages[idx].stage_type() == StageType::Nam {
                                self.backend.rebuild_stage(idx, &self.stages[idx]);
                            }
                        }
                    }
                    Err(e) => log::error!("Failed to rescan NAM models: {e}"),
                }
            }
            NamTabMessage::OpenFolder => {
                if let Some(dir) = self.backend.nam_models_dir() {
                    crate::platform::open_directory(&dir);
                } else {
                    log::warn!("No NAM models directory to open");
                }
            }
        }
    }

    /// Load `key` into the selected NAM stage, or add a NAM stage holding it.
    fn load_model(&mut self, key: String) {
        if let Some(idx) = self.nam_browser.focused_stage(&self.stages) {
            let already_loaded = matches!(
                &self.stages[idx],
                StageConfig::Nam(cfg) if cfg.model_name.as_deref() == Some(key.as_str())
            );
            if !already_loaded {
                self.update(Message::Stage(
                    idx,
                    StageMessage::Nam(NamMessage::ModelSelected(key)),
                ));
            }
        } else if let Some(idx) = self.insert_stage(StageConfig::Nam(NamConfig {
            model_name: Some(key),
            ..NamConfig::default()
        })) {
            self.nam_browser.focus_stage(idx);
        }
    }

    /// Insert `stage` at the end of its category and tell the backend. Returns its
    /// index, or `None` when the chain is full.
    fn insert_stage(&mut self, stage: StageConfig) -> Option<usize> {
        // Cap the chain so the engine's stage list never has to grow on the RT
        // thread. See `DEFAULT_CHAIN_CAPACITY`.
        if self.stages.len() >= DEFAULT_CHAIN_CAPACITY {
            return None;
        }
        self.flush_dirty_params();
        let idx = self.category_end_index(stage.category());
        self.stages.insert(idx, stage);
        self.collapsed_stages.insert(idx, false);
        self.nam_browser.chain_edited(ChainEdit::Inserted(idx));
        self.backend.add_stage(idx, &self.stages[idx]);
        self.backend.persist_chain_state(&self.stages);
        Some(idx)
    }

    fn handle_hotkey(&mut self, msg: HotkeyMessage) -> UpdateResult {
        if matches!(msg, HotkeyMessage::Open) {
            let presets = self.preset_handler.get_available_presets().to_vec();
            self.hotkey_handler.open(presets);
            return UpdateResult::Handled(Task::none());
        }

        let task = self.hotkey_handler.handle(msg);
        UpdateResult::Handled(task)
    }

    fn handle_key_pressed(
        &mut self,
        key: &iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    ) -> UpdateResult {
        if self.hotkey_handler.is_learning() {
            self.hotkey_handler.on_key_input(key, modifiers);
            return UpdateResult::Handled(Task::none());
        }

        // If the outer shell has dialogs open, it should intercept KeyPressed
        // before calling SharedApp::update(). But as a safety net, hotkey
        // mapping check still runs here.
        if let Some(preset_name) = self.hotkey_handler.check_mapping(key, modifiers) {
            return UpdateResult::Handled(Task::done(Message::Preset(PresetMessage::Select(
                preset_name,
            ))));
        }

        UpdateResult::Handled(Task::none())
    }

    // -- View methods --------------------------------------------------------

    /// Main content view (header, preset bar, tab bar, tab content, footer).
    /// Does NOT include dialog overlays — those are added by the outer shell.
    pub fn view(&self) -> Element<'_, Message> {
        let header = self.view_header();
        let tab_bar = self.view_tab_bar();
        let tab_content = match self.active_tab {
            Tab::Amp => self.view_amp_tab(),
            Tab::Effects => self.view_effects_tab(),
            Tab::Cabinet => self.view_cabinet_tab(),
            Tab::Io => self.view_io_tab(),
            Tab::Nam => self.view_nam_tab(),
        };
        let signal_minimap =
            minimap::view(&self.stages, &self.input_filter_config, self.active_tab);
        let mut footer = row![].align_y(Alignment::Center);
        if self.backend.capabilities().has_peak_meter {
            footer = footer.push(self.peak_meter_display.view_status());
        }
        footer = footer.push(signal_minimap);

        column![
            header,
            self.preset_handler
                .view(!self.backend.capabilities().has_preset_management),
            tab_bar,
            tab_content,
            footer,
        ]
        .spacing(SPACING_NORMAL)
        .padding(PADDING_LARGE)
        .into()
    }

    fn view_header(&self) -> Element<'_, Message> {
        let caps = self.backend.capabilities();

        let mut header_row = row![].spacing(SPACING_TIGHT).align_y(Alignment::Center);

        if caps.has_peak_meter {
            header_row = header_row.push(self.peak_meter_display.view());
        }
        header_row = header_row.push(space::horizontal());

        // Standalone-only buttons are guarded by capabilities
        if caps.has_midi_config {
            header_row = header_row
                .push(
                    button(tr!(hotkeys))
                        .on_press(Message::Hotkey(HotkeyMessage::Open))
                        .style(iced::widget::button::secondary),
                )
                .push(
                    button(tr!(midi))
                        .on_press(Message::Midi(crate::messages::MidiMessage::Open))
                        .style(iced::widget::button::secondary),
                );
        }
        if caps.has_tuner {
            header_row = header_row.push(
                button(tr!(tuner))
                    .on_press(Message::Tuner(crate::messages::TunerMessage::Toggle))
                    .style(iced::widget::button::secondary),
            );
        }
        if caps.has_settings_dialog {
            header_row = header_row.push(
                button(tr!(settings))
                    .on_press(Message::Settings(crate::messages::SettingsMessage::Open)),
            );
        }

        if caps.has_recorder {
            let record_button = if self.is_recording {
                button(text(tr!(stop_recording)))
                    .on_press(Message::StopRecording)
                    .style(iced::widget::button::danger)
            } else {
                button(text(tr!(start_recording)))
                    .on_press(Message::StartRecording)
                    .style(iced::widget::button::success)
            };
            header_row = header_row.push(record_button);
            if self.is_recording {
                header_row =
                    header_row.push(text(tr!(recording)).style(|_| iced::widget::text::Style {
                        color: Some(crate::components::widgets::common::COLOR_ERROR),
                    }));
            }
        }

        header_row.into()
    }

    fn view_tab_bar(&self) -> Element<'_, Message> {
        let tabs = [
            (Tab::Io, tr!(tab_io)),
            (Tab::Amp, tr!(tab_amp)),
            (Tab::Nam, tr!(tab_nam)),
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

        let collapse_toggle = self.view_collapse_toggle(category);

        let mut stage_col = column![].width(Length::Fill).spacing(SPACING_TIGHT);
        for (pos, &abs_idx) in category_indices.iter().enumerate() {
            let is_collapsed = self.collapsed_stages.get(abs_idx).copied().unwrap_or(false);
            let can_move_up = pos > 0;
            let can_move_down = pos < total_in_category.saturating_sub(1);
            let bypassed = self.stages[abs_idx].bypassed();
            stage_col = stage_col.push(view_stage_config(
                &self.stages[abs_idx],
                abs_idx,
                StageViewState {
                    is_collapsed,
                    can_move_up,
                    can_move_down,
                    bypassed,
                },
            ));
        }

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
        let category_has_stages = self
            .stages
            .iter()
            .any(|s| s.category() == category && !matches!(s, StageConfig::Nam(_)));
        let all_collapsed = category_has_stages
            && self
                .stages
                .iter()
                .enumerate()
                .filter(|(_, s)| s.category() == category && !matches!(s, StageConfig::Nam(_)))
                .all(|(i, _)| self.collapsed_stages.get(i).copied().unwrap_or(false));

        let label = if all_collapsed {
            format!("\u{25bc} {}", tr!(expand_all))
        } else {
            format!("\u{25b6} {}", tr!(collapse_all))
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

        // Disable "Add Stage" once the chain hits its capacity cap.
        let add_msg = (self.stages.len() < DEFAULT_CHAIN_CAPACITY).then_some(Message::AddStage);
        row![
            pick_list(available_types, selected, Message::StageTypeSelected),
            button(tr!(add_stage)).on_press_maybe(add_msg),
        ]
        .spacing(SPACING_NORMAL)
        .align_y(Alignment::Center)
        .into()
    }

    fn view_nam_tab(&self) -> Element<'_, Message> {
        let ctx = NamViewContext {
            stages: &self.stages,
            device_sample_rate: self.backend.sample_rate(),
            oversampling: self.backend.oversampling_factor(),
            models_dir: self.backend.nam_models_dir(),
            ir_bypassed: self.ir_cabinet_control.is_bypassed(),
            chain_full: self.stages.len() >= DEFAULT_CHAIN_CAPACITY,
        };
        view_tab_panel(self.nam_browser.view(&ctx))
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
                .step(1.0_f32),
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
                .step(1.0_f32),
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

        let oversampling_factors = vec![1u32, 2, 4, 8, 16];
        let oversampling_section = section_container(
            column![
                section_title(tr!(oversampling_factor)),
                row![
                    pick_list(
                        oversampling_factors,
                        Some(self.oversampling_factor),
                        Message::OversamplingChanged,
                    ),
                    text(format!("({}x)", self.oversampling_factor)),
                ]
                .spacing(SPACING_NORMAL)
                .align_y(Alignment::Center),
            ]
            .spacing(SPACING_NORMAL)
            .into(),
        );

        let content = scrollable(
            column![input_filters_section, pitch_section, oversampling_section,]
                .spacing(SPACING_NORMAL)
                .padding(PADDING_NORMAL),
        )
        .height(Length::Fill);

        view_tab_panel(content.into())
    }

    // -- Subscription --------------------------------------------------------

    pub fn subscription(&self) -> Subscription<Message> {
        let rebuild_sub = if self.dirty_params.is_empty() {
            Subscription::none()
        } else {
            time::every(REBUILD_INTERVAL).map(|_| Message::RebuildTick)
        };

        let peak_meter_sub = if self.backend.capabilities().has_peak_meter {
            time::every(PEAK_METER_POLL_INTERVAL).map(|_| Message::PeakMeterUpdate)
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

        Subscription::batch(vec![rebuild_sub, peak_meter_sub, keyboard_sub])
    }

    // -- Helpers -------------------------------------------------------------

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
            StageCategory::Amp => self
                .stages
                .iter()
                .enumerate()
                .filter(|(_, s)| s.category() == StageCategory::Amp)
                .map(|(i, _)| i + 1)
                .next_back()
                .unwrap_or(0),
            StageCategory::Effect => self.stages.len(),
        }
    }

    pub fn flush_dirty_params(&mut self) {
        for ((idx, name), value) in self.dirty_params.drain() {
            self.backend.begin_edit(idx, name);
            self.backend.set_parameter(idx, name, value);
            self.backend.end_edit(idx, name);
        }
    }

    fn update_processor_chain(&self) {
        self.backend.set_amp_chain(&self.stages);
    }
}

// -- Shared view helpers -----------------------------------------------------

/// Shared container for all tab content panels — consistent sizing and structure.
pub fn view_tab_panel(content: Element<'_, Message>) -> Element<'_, Message> {
    container(content)
        .width(Length::Fill)
        .height(Length::FillPortion(9))
        .into()
}

pub fn tab_button_active(
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

pub fn tab_button_inactive(
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

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use rustortion_core::amp::stages::nam::NamConfig;

    use super::*;
    use crate::backend::Capabilities;
    use crate::components::nam_browser::NamBrowser;
    use crate::hotkey::HotkeySettings;
    use crate::messages::NamTabMessage;

    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        AddStage(usize),
        RemoveStage(usize),
        RebuildStage(usize),
        SetParameter(usize, &'static str),
        Rescan,
    }

    struct MockBackend {
        calls: Mutex<Vec<Call>>,
        capabilities: Capabilities,
    }

    impl MockBackend {
        fn record(&self, call: Call) {
            self.calls.lock().unwrap().push(call);
        }
    }

    impl ParamBackend for MockBackend {
        fn set_parameter(&self, stage_idx: usize, name: &'static str, _value: f32) {
            self.record(Call::SetParameter(stage_idx, name));
        }
        fn rebuild_stage(&self, stage_idx: usize, _config: &StageConfig) {
            self.record(Call::RebuildStage(stage_idx));
        }
        fn set_amp_chain(&self, _stages: &[StageConfig]) {}
        fn set_bypass(&self, _stage_idx: usize, _bypassed: bool) {}
        fn add_stage(&self, idx: usize, _config: &StageConfig) {
            self.record(Call::AddStage(idx));
        }
        fn remove_stage(&self, idx: usize) {
            self.record(Call::RemoveStage(idx));
        }
        fn swap_stages(&self, _a: usize, _b: usize) {}
        fn set_ir(&self, _path: &str) {}
        fn set_ir_bypass(&self, _bypassed: bool) {}
        fn set_ir_gain(&self, _gain: f32) {}
        fn set_input_filter(&self, _filter: &InputFilterConfig) {}
        fn set_pitch_shift(&self, _semitones: i32) {}
        fn set_oversampling(&self, _factor: u32) {}
        fn sample_rate(&self) -> u32 {
            48_000
        }
        fn oversampling_factor(&self) -> u32 {
            1
        }
        fn capabilities(&self) -> &Capabilities {
            &self.capabilities
        }
        fn get_available_irs(&self) -> Vec<String> {
            Vec::new()
        }
        fn get_peak_meter_info(&self) -> Option<ExternalEvent> {
            None
        }
        fn nam_models_dir(&self) -> Option<std::path::PathBuf> {
            None
        }
        fn rescan_nam_models(&self) -> Result<usize, String> {
            self.record(Call::Rescan);
            Ok(0)
        }
    }

    fn app(stages: Vec<StageConfig>) -> SharedApp<MockBackend> {
        SharedApp {
            backend: MockBackend {
                calls: Mutex::new(Vec::new()),
                capabilities: Capabilities::plugin(),
            },
            collapsed_stages: vec![false; stages.len()],
            stages,
            dirty_params: HashMap::new(),
            active_tab: Tab::default(),
            selected_stage_type: StageType::default(),
            ir_cabinet_control: IrCabinetControl::default(),
            pitch_shift_control: PitchShiftControl::new(0),
            preset_handler: PresetHandler::new_from_presets(Vec::new()),
            peak_meter_display: PeakMeterDisplay::default(),
            hotkey_handler: HotkeyHandler::new(HotkeySettings::default()),
            input_filter_config: InputFilterConfig::default(),
            oversampling_factor: 1,
            is_recording: false,
            nam_browser: NamBrowser::default(),
        }
    }

    fn calls(app: &SharedApp<MockBackend>) -> Vec<Call> {
        app.backend.calls.lock().unwrap().clone()
    }

    fn nam(model: Option<&str>) -> StageConfig {
        StageConfig::Nam(NamConfig {
            model_name: model.map(str::to_owned),
            ..NamConfig::default()
        })
    }

    fn stage(kind: StageType) -> StageConfig {
        StageConfig::from(kind)
    }

    fn click(app: &mut SharedApp<MockBackend>, key: &str) {
        app.update(Message::Nam(NamTabMessage::ModelClicked(key.to_owned())));
    }

    fn model_at(app: &SharedApp<MockBackend>, idx: usize) -> Option<&str> {
        match &app.stages[idx] {
            StageConfig::Nam(cfg) => cfg.model_name.as_deref(),
            other => panic!("stage {idx} is {:?}, not NAM", other.stage_type()),
        }
    }

    #[test]
    fn clicking_a_model_with_no_nam_stage_inserts_one_after_the_amp_stages() {
        let mut app = app(vec![
            stage(StageType::NoiseGate),
            stage(StageType::Level),
            stage(StageType::Delay),
        ]);
        app.collapsed_stages = vec![true, false, true];
        click(&mut app, "Pack/model");
        assert_eq!(app.stages.len(), 4);
        assert_eq!(model_at(&app, 2), Some("Pack/model"));
        assert_eq!(app.stages[3].stage_type(), StageType::Delay);
        assert_eq!(app.collapsed_stages, [true, false, false, true]);
        assert_eq!(calls(&app), [Call::AddStage(2)]);
        assert_eq!(app.nam_browser.focused_stage(&app.stages), Some(2));
    }

    #[test]
    fn clicking_a_model_swaps_the_selected_stages_model() {
        let mut app = app(vec![nam(None)]);
        click(&mut app, "a");
        assert_eq!(app.stages.len(), 1);
        assert_eq!(model_at(&app, 0), Some("a"));
        assert_eq!(calls(&app), [Call::RebuildStage(0)]);
    }

    #[test]
    fn collapse_all_ignores_nam_rows() {
        let mut app = app(vec![stage(StageType::Level), nam(Some("a"))]);
        app.collapsed_stages = vec![true, false];
        app.update(Message::ToggleAllStagesCollapse);
        assert!(
            !app.collapsed_stages[0],
            "the only real card was collapsed, so the toggle expands it"
        );
    }

    #[test]
    fn clicking_the_loaded_model_does_nothing() {
        let mut app = app(vec![nam(Some("a"))]);
        click(&mut app, "a");
        assert!(calls(&app).is_empty());
    }

    #[test]
    fn clicking_a_model_loads_into_the_selected_stage_when_there_are_two() {
        let mut app = app(vec![nam(Some("a")), nam(Some("b"))]);
        app.update(Message::Nam(NamTabMessage::StageFocused(1)));
        click(&mut app, "c");
        assert_eq!(model_at(&app, 0), Some("a"));
        assert_eq!(model_at(&app, 1), Some("c"));
    }

    #[test]
    fn clicking_a_model_on_a_full_chain_with_no_nam_stage_does_nothing() {
        let mut app = app(vec![stage(StageType::Level); DEFAULT_CHAIN_CAPACITY]);
        click(&mut app, "a");
        assert_eq!(app.stages.len(), DEFAULT_CHAIN_CAPACITY);
        assert!(
            app.stages
                .iter()
                .all(|s| s.stage_type() == StageType::Level)
        );
        assert!(calls(&app).is_empty());
        assert_eq!(app.collapsed_stages.len(), DEFAULT_CHAIN_CAPACITY);
        assert_eq!(app.nam_browser.focused_stage(&app.stages), None);
    }

    #[test]
    fn a_pending_slider_edit_is_sent_before_the_insert_shifts_indices() {
        let mut app = app(vec![stage(StageType::Level), stage(StageType::Delay)]);
        app.dirty_params.insert((1, "feedback"), 0.5);
        click(&mut app, "a");
        assert_eq!(
            calls(&app),
            [Call::SetParameter(1, "feedback"), Call::AddStage(1)]
        );
        assert!(app.dirty_params.is_empty());
    }

    #[test]
    fn removing_the_selected_nam_stage_selects_the_first_remaining_one() {
        let mut app = app(vec![
            nam(Some("a")),
            stage(StageType::Level),
            nam(Some("b")),
        ]);
        app.update(Message::Nam(NamTabMessage::StageFocused(2)));
        app.update(Message::RemoveStage(2));
        assert_eq!(app.nam_browser.focused_stage(&app.stages), Some(0));
    }

    #[test]
    fn removing_an_earlier_stage_keeps_the_same_nam_stage_selected() {
        let mut app = app(vec![
            stage(StageType::Level),
            nam(Some("a")),
            nam(Some("b")),
        ]);
        app.update(Message::Nam(NamTabMessage::StageFocused(2)));
        app.update(Message::RemoveStage(0));
        assert_eq!(app.nam_browser.focused_stage(&app.stages), Some(1));
        assert_eq!(model_at(&app, 1), Some("b"));
    }

    #[test]
    fn moving_the_selected_nam_stage_keeps_it_selected() {
        let mut app = app(vec![nam(Some("a")), nam(Some("b"))]);
        app.update(Message::Nam(NamTabMessage::StageFocused(1)));
        app.update(Message::MoveStageUp(1));
        assert_eq!(app.nam_browser.focused_stage(&app.stages), Some(0));
        assert_eq!(model_at(&app, 0), Some("b"));
    }

    #[test]
    fn the_stage_shown_when_the_nam_tab_opens_stays_shown_after_a_reorder() {
        let mut app = app(vec![nam(Some("a")), nam(Some("b"))]);
        app.update(Message::TabSelected(Tab::Nam));
        app.update(Message::TabSelected(Tab::Amp));
        app.update(Message::MoveStageUp(1));
        let shown = app
            .nam_browser
            .focused_stage(&app.stages)
            .expect("a NAM stage is selected");
        assert_eq!(model_at(&app, shown), Some("a"));
    }

    #[test]
    fn moving_the_selected_nam_stage_down_keeps_it_selected() {
        let mut app = app(vec![nam(Some("a")), nam(Some("b"))]);
        app.update(Message::Nam(NamTabMessage::StageFocused(0)));
        app.update(Message::MoveStageDown(0));
        assert_eq!(app.nam_browser.focused_stage(&app.stages), Some(1));
        assert_eq!(model_at(&app, 1), Some("a"));
    }

    #[test]
    fn loading_a_preset_selects_its_first_nam_stage() {
        let mut app = app(vec![
            stage(StageType::Level),
            stage(StageType::Level),
            nam(Some("a")),
        ]);
        app.update(Message::Nam(NamTabMessage::StageFocused(2)));
        app.update(Message::SetStages(vec![
            stage(StageType::Level),
            nam(Some("c")),
            nam(Some("d")),
        ]));
        assert_eq!(app.nam_browser.focused_stage(&app.stages), Some(1));
    }

    #[test]
    fn open_stage_selects_it_and_switches_to_the_nam_tab() {
        let mut app = app(vec![nam(Some("a")), nam(Some("b"))]);
        app.update(Message::Nam(NamTabMessage::OpenStage(1)));
        assert_eq!(app.active_tab, Tab::Nam);
        assert_eq!(app.nam_browser.focused_stage(&app.stages), Some(1));
    }

    #[test]
    fn rescan_works_with_no_nam_stage_and_rebuilds_the_live_ones() {
        let mut empty = app(Vec::new());
        empty.update(Message::Nam(NamTabMessage::Rescan));
        assert_eq!(calls(&empty), [Call::Rescan]);

        let mut two = app(vec![nam(Some("a")), stage(StageType::Level), nam(None)]);
        two.update(Message::Nam(NamTabMessage::Rescan));
        assert_eq!(
            calls(&two),
            [Call::Rescan, Call::RebuildStage(0), Call::RebuildStage(2)]
        );
    }

    #[test]
    fn add_stage_still_adds_an_empty_nam_stage_from_the_amp_tab() {
        let mut app = app(vec![stage(StageType::Level)]);
        app.update(Message::StageTypeSelected(StageType::Nam));
        app.update(Message::AddStage);
        assert_eq!(model_at(&app, 1), None);
        assert_eq!(calls(&app), [Call::AddStage(1)]);
    }
}
