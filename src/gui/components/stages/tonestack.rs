use iced::widget::{column, container, pick_list, row, text};
use iced::{Element, Length};

use crate::amp::stages::tonestack::ToneStackModel;
use crate::gui::components::widgets::common::{labeled_slider, stage_header};
use crate::gui::config::ToneStackConfig;
use crate::gui::messages::{Message, StageMessage, ToneStackMessage};
use crate::tr;

const TONE_STACK_MODELS: [ToneStackModel; 4] = [
    ToneStackModel::Modern,
    ToneStackModel::British,
    ToneStackModel::American,
    ToneStackModel::Flat,
];

pub fn view(idx: usize, cfg: &ToneStackConfig, total_stages: usize) -> Element<'_, Message> {
    let header = stage_header(tr!(stage_tone_stack), idx, total_stages);

    let model_picker = row![
        text(tr!(model)).width(Length::FillPortion(3)),
        pick_list(TONE_STACK_MODELS, Some(cfg.model), move |m| {
            Message::Stage(
                idx,
                StageMessage::ToneStack(ToneStackMessage::ModelChanged(m)),
            )
        })
        .width(Length::FillPortion(7)),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let body = column![
        model_picker,
        labeled_slider(
            tr!(bass),
            0.0..=2.0,
            cfg.bass,
            move |v| Message::Stage(
                idx,
                StageMessage::ToneStack(ToneStackMessage::BassChanged(v))
            ),
            |v| format!("{v:.2}"),
            0.05
        ),
        labeled_slider(
            tr!(mid),
            0.0..=2.0,
            cfg.mid,
            move |v| Message::Stage(
                idx,
                StageMessage::ToneStack(ToneStackMessage::MidChanged(v))
            ),
            |v| format!("{v:.2}"),
            0.05
        ),
        labeled_slider(
            tr!(treble),
            0.0..=2.0,
            cfg.treble,
            move |v| Message::Stage(
                idx,
                StageMessage::ToneStack(ToneStackMessage::TrebleChanged(v))
            ),
            |v| format!("{v:.2}"),
            0.05
        ),
        labeled_slider(
            tr!(presence),
            0.0..=2.0,
            cfg.presence,
            move |v| Message::Stage(
                idx,
                StageMessage::ToneStack(ToneStackMessage::PresenceChanged(v))
            ),
            |v| format!("{v:.2}"),
            0.05
        ),
    ]
    .spacing(5);

    container(column![header, body].spacing(5).padding(10))
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(5))
        })
        .into()
}
