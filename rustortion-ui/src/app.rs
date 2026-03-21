use std::collections::HashMap;

use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, slider, space, text,
};
use iced::{Alignment, Element, Length, Subscription, Task, keyboard, time, time::Duration};

use crate::backend::{ExternalEvent, ParamBackend};
use crate::components::ir_cabinet_control::IrCabinetControl;
use crate::components::minimap;
use crate::components::peak_meter::PeakMeterDisplay;
use crate::components::pitch_shift_control::PitchShiftControl;
use crate::components::widgets::common::{
    PADDING_LARGE, PADDING_NORMAL, SPACING_NORMAL, SPACING_TIGHT, StageViewState,
    TAB_BUTTON_PADDING, TEXT_SIZE_TAB, section_container, section_title,
};
use crate::handlers::hotkey::HotkeyHandler;
use crate::handlers::preset::PresetHandler;
use crate::messages::{HotkeyMessage, Message, PresetMessage};
use crate::stages::{
    ParamUpdate, StageCategory, StageConfig, StageType, apply_stage_config, view_stage_config,
};
use crate::tabs::Tab;
use crate::tr;
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
    /// Whether recording is active — set by standalone, displayed in header.
    pub is_recording: bool,
}

impl<B: ParamBackend> SharedApp<B> {
    pub fn update(&mut self, message: Message) -> UpdateResult {
        match message {
            Message::TabSelected(tab) => {
                self.active_tab = tab;
                self.sync_stage_type_with_tab(tab);
            }
            Message::SetStages(stages) => {
                self.collapsed_stages.resize(stages.len(), false);
                self.stages = stages;
                self.dirty_params.clear();
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
                self.flush_dirty_params();
                let new_stage = StageConfig::from(self.selected_stage_type);
                let category = new_stage.category();
                let insert_idx = self.category_end_index(category);
                self.stages.insert(insert_idx, new_stage);
                self.collapsed_stages.insert(insert_idx, false);
                self.backend.add_stage(insert_idx, &self.stages[insert_idx]);
                self.backend.persist_chain_state(&self.stages);
            }
            Message::RemoveStage(idx) => {
                if idx < self.stages.len() {
                    self.flush_dirty_params();
                    self.stages.remove(idx);
                    self.collapsed_stages.remove(idx);
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
        };
        let signal_minimap =
            minimap::view(&self.stages, &self.input_filter_config, self.active_tab);
        let footer =
            row![self.peak_meter_display.view_status(), signal_minimap,].align_y(Alignment::Center);

        column![
            header,
            self.preset_handler.view(!self.backend.capabilities().has_preset_management),
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

        let mut header_row = row![self.peak_meter_display.view(), space::horizontal(),]
            .spacing(SPACING_TIGHT)
            .align_y(Alignment::Center);

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
        let category_has_stages = self.stages.iter().any(|s| s.category() == category);
        let all_collapsed = category_has_stages
            && self
                .stages
                .iter()
                .enumerate()
                .filter(|(_, s)| s.category() == category)
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

    // -- Subscription --------------------------------------------------------

    pub fn subscription(&self) -> Subscription<Message> {
        let rebuild_sub = if self.dirty_params.is_empty() {
            Subscription::none()
        } else {
            time::every(REBUILD_INTERVAL).map(|_| Message::RebuildTick)
        };

        let peak_meter_sub =
            time::every(PEAK_METER_POLL_INTERVAL).map(|_| Message::PeakMeterUpdate);

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
