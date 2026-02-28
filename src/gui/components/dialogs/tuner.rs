use iced::widget::{button, column, container, row, rule, space, text};
use iced::{Alignment, Color, Element, Length};

use crate::gui::messages::TunerMessage;
use crate::tr;
use crate::tuner::TunerInfo;

pub struct TunerDisplay {
    info: TunerInfo,
    show_dialog: bool,
}

impl Default for TunerDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl TunerDisplay {
    pub fn new() -> Self {
        Self {
            info: TunerInfo::default(),
            show_dialog: false,
        }
    }

    pub fn show(&mut self) {
        self.show_dialog = true;
        self.info = TunerInfo::default();
    }

    pub fn hide(&mut self) {
        self.show_dialog = false;
    }

    pub fn is_visible(&self) -> bool {
        self.show_dialog
    }

    pub fn update(&mut self, info: TunerInfo) {
        self.info = info;
    }

    pub fn view(&self) -> Option<Element<'_, TunerMessage>> {
        if !self.show_dialog {
            return None;
        }

        let title = text(tr!(tuner_title))
            .size(28)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            });

        let note_display = if let Some(ref note) = self.info.note {
            text(note)
                .size(96)
                .style(move |_: &iced::Theme| iced::widget::text::Style {
                    color: Some(if self.info.in_tune {
                        Color::from_rgb(0.2, 1.0, 0.2) // Bright green
                    } else {
                        Color::from_rgb(0.9, 0.9, 0.9)
                    }),
                })
        } else {
            text("--")
                .size(96)
                .style(|_: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.4, 0.4, 0.4)),
                })
        };

        let freq_text = if let Some(freq) = self.info.frequency {
            format!("{:.1} {}", freq, tr!(hz))
        } else {
            format!("--.- {}", tr!(hz))
        };

        let freq_display =
            text(freq_text)
                .size(20)
                .style(|_: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.7, 0.7, 0.7)),
                });

        let cents_indicator = self.cents_display();

        let status_text = if self.info.in_tune {
            text(format!("{} ✓", tr!(in_tune)))
                .size(24)
                .style(|_: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.2, 1.0, 0.2)),
                })
        } else if self.info.cents_off.is_some() {
            text(tr!(adjust))
                .size(20)
                .style(|_: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(1.0, 0.7, 0.3)),
                })
        } else {
            text(tr!(play_a_note))
                .size(20)
                .style(|_: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                })
        };

        let close_button = button(tr!(close))
            .on_press(TunerMessage::Toggle) // Toggles off since it's already open
            .style(iced::widget::button::primary)
            .padding(10);

        let dialog_content = column![
            title,
            rule::horizontal(1),
            note_display,
            freq_display,
            cents_indicator,
            status_text,
            close_button,
        ]
        .spacing(10)
        .padding(40)
        .width(Length::Fixed(800.0))
        .align_x(Alignment::Center);

        let dialog = container(dialog_content).style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(10).width(2))
        });

        let centered = container(dialog)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_theme: &iced::Theme| {
                container::Style::default().background(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7))
            });

        Some(centered.into())
    }

    fn cents_display(&self) -> Element<'static, TunerMessage> {
        if let Some(cents) = self.info.cents_off {
            let width: usize = 50;
            let center = width / 2;

            let pos = ((cents / 50.0).clamp(-1.0, 1.0) * (width / 2) as f32) as i32;
            let marker = (center as i32 + pos).clamp(0, width as i32 - 1) as usize;

            let mut bar = vec![' '; width];
            bar[center] = '│';
            bar[marker] = '●';

            let bar_str: String = bar.iter().collect();

            let cents_text = if cents >= 0.0 {
                format!("+{:.0}¢", cents)
            } else {
                format!("{:.0}¢", cents)
            };

            let color = if cents.abs() < 5.0 {
                Color::from_rgb(0.2, 1.0, 0.2) // Green
            } else if cents.abs() < 20.0 {
                Color::from_rgb(1.0, 0.8, 0.2) // Yellow
            } else {
                Color::from_rgb(1.0, 0.3, 0.3) // Red
            };

            let flat_label = format!("♭ {}", tr!(flat));
            let sharp_label = format!("{} ♯", tr!(sharp));

            column![
                text(bar_str)
                    .font(iced::Font::MONOSPACE)
                    .size(24)
                    .style(move |_: &iced::Theme| iced::widget::text::Style { color: Some(color) }),
                row![
                    text(flat_label)
                        .size(14)
                        .style(|_: &iced::Theme| iced::widget::text::Style {
                            color: Some(Color::from_rgb(0.6, 0.6, 0.6)),
                        }),
                    space::horizontal(),
                    text(cents_text).size(22).style(move |_: &iced::Theme| {
                        iced::widget::text::Style { color: Some(color) }
                    }),
                    space::horizontal(),
                    text(sharp_label)
                        .size(14)
                        .style(|_: &iced::Theme| iced::widget::text::Style {
                            color: Some(Color::from_rgb(0.6, 0.6, 0.6)),
                        }),
                ]
                .spacing(10)
                .width(Length::Fill)
            ]
            .spacing(5)
            .align_x(Alignment::Center)
            .into()
        } else {
            column![
                text("│")
                    .font(iced::Font::MONOSPACE)
                    .size(24)
                    .style(|_: &iced::Theme| iced::widget::text::Style {
                        color: Some(Color::from_rgb(0.3, 0.3, 0.3)),
                    }),
                text("--¢")
                    .size(22)
                    .style(|_: &iced::Theme| iced::widget::text::Style {
                        color: Some(Color::from_rgb(0.4, 0.4, 0.4)),
                    }),
            ]
            .spacing(5)
            .align_x(Alignment::Center)
            .into()
        }
    }
}
