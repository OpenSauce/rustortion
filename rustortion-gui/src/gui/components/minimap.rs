use iced::widget::{button, container, row, text};
use iced::{Alignment, Element, Length};

use crate::gui::messages::Message;
use crate::gui::stages::{StageCategory, StageConfig};
use crate::gui::tabs::Tab;
use rustortion_core::preset::InputFilterConfig;

const fn stage_abbreviation(cfg: &StageConfig) -> &'static str {
    match cfg {
        StageConfig::Preamp(_) => "Pre",
        StageConfig::Compressor(_) => "Cmp",
        StageConfig::ToneStack(_) => "TS",
        StageConfig::PowerAmp(_) => "PA",
        StageConfig::Level(_) => "Lvl",
        StageConfig::NoiseGate(_) => "NG",
        StageConfig::MultibandSaturator(_) => "MBS",
        StageConfig::Delay(_) => "Dly",
        StageConfig::Reverb(_) => "Rev",
        StageConfig::Eq(_) => "EQ",
    }
}

fn block_style(
    is_active_tab: bool,
    bypassed: bool,
) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style {
    move |theme: &iced::Theme, status| {
        let palette = theme.palette();
        let alpha = if bypassed { 0.5 } else { 1.0 };
        let mut style = iced::widget::button::Style {
            text_color: iced::Color::from_rgba(
                palette.text.r,
                palette.text.g,
                palette.text.b,
                alpha,
            ),
            border: iced::Border {
                color: if is_active_tab {
                    iced::Color::from_rgba(
                        palette.primary.r,
                        palette.primary.g,
                        palette.primary.b,
                        alpha,
                    )
                } else {
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.2 * alpha)
                },
                width: if is_active_tab { 2.0 } else { 1.0 },
                radius: 4.0.into(),
            },
            background: Some(iced::Background::Color(if is_active_tab {
                iced::Color::from_rgba(
                    palette.primary.r,
                    palette.primary.g,
                    palette.primary.b,
                    0.15 * alpha,
                )
            } else {
                iced::Color::from_rgba(0.2, 0.2, 0.3, 0.3 * alpha)
            })),
            ..iced::widget::button::Style::default()
        };

        if !bypassed && matches!(status, iced::widget::button::Status::Hovered) {
            style.background = Some(iced::Background::Color(iced::Color::from_rgba(
                palette.primary.r,
                palette.primary.g,
                palette.primary.b,
                0.25,
            )));
        }

        style
    }
}

fn fixed_block_style(
    _theme: &iced::Theme,
    _status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
        text_color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.5),
        border: iced::Border {
            color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.1),
            width: 1.0,
            radius: 4.0.into(),
        },
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.15, 0.15, 0.2, 0.3,
        ))),
        ..iced::widget::button::Style::default()
    }
}

pub fn view<'a>(
    stages: &'a [StageConfig],
    input_filters: &InputFilterConfig,
    active_tab: Tab,
) -> Element<'a, Message> {
    let mut chain = row![].spacing(2).align_y(Alignment::Center);

    // IN block
    chain = chain.push(
        button(text("IN").size(11))
            .style(fixed_block_style)
            .padding([2, 6]),
    );

    chain = chain.push(text("\u{2192}").size(11));

    // HP/LP block
    let filter_label = match (input_filters.hp_enabled, input_filters.lp_enabled) {
        (true, true) => "HP/LP",
        (true, false) => "HP",
        (false, true) => "LP",
        (false, false) => "-",
    };
    let io_active = active_tab == Tab::Io;
    chain = chain.push(
        button(text(filter_label).size(11))
            .on_press(Message::TabSelected(Tab::Io))
            .style(block_style(io_active, false))
            .padding([2, 6]),
    );

    // Stage blocks
    for stage in stages {
        chain = chain.push(text("\u{2192}").size(11));
        let abbr = stage_abbreviation(stage);
        let cat = stage.category();
        let tab = match cat {
            StageCategory::Amp => Tab::Amp,
            StageCategory::Effect => Tab::Effects,
        };
        let is_active = active_tab == tab;
        let bypassed = stage.bypassed();
        chain = chain.push(
            button(text(abbr).size(11))
                .on_press(Message::TabSelected(tab))
                .style(block_style(is_active, bypassed))
                .padding([2, 6]),
        );
    }

    chain = chain.push(text("\u{2192}").size(11));

    // CAB block
    let cab_active = active_tab == Tab::Cabinet;
    chain = chain.push(
        button(text("CAB").size(11))
            .on_press(Message::TabSelected(Tab::Cabinet))
            .style(block_style(cab_active, false))
            .padding([2, 6]),
    );

    chain = chain.push(text("\u{2192}").size(11));

    // OUT block
    chain = chain.push(
        button(text("OUT").size(11))
            .style(fixed_block_style)
            .padding([2, 6]),
    );

    container(chain)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .padding([5, 10])
        .into()
}
