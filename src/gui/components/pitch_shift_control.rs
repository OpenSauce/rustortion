use iced::widget::{pick_list, row, text};
use iced::{Alignment, Element};

use crate::gui::messages::Message;
use crate::tr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemitoneOption(pub i32);

impl std::fmt::Display for SemitoneOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == 0 {
            write!(f, "Off")
        } else {
            write!(f, "{:+}", self.0)
        }
    }
}

const SEMITONE_OPTIONS: [SemitoneOption; 25] = [
    SemitoneOption(-12),
    SemitoneOption(-11),
    SemitoneOption(-10),
    SemitoneOption(-9),
    SemitoneOption(-8),
    SemitoneOption(-7),
    SemitoneOption(-6),
    SemitoneOption(-5),
    SemitoneOption(-4),
    SemitoneOption(-3),
    SemitoneOption(-2),
    SemitoneOption(-1),
    SemitoneOption(0),
    SemitoneOption(1),
    SemitoneOption(2),
    SemitoneOption(3),
    SemitoneOption(4),
    SemitoneOption(5),
    SemitoneOption(6),
    SemitoneOption(7),
    SemitoneOption(8),
    SemitoneOption(9),
    SemitoneOption(10),
    SemitoneOption(11),
    SemitoneOption(12),
];

pub struct PitchShiftControl {
    semitones: i32,
}

impl PitchShiftControl {
    pub fn new(semitones: i32) -> Self {
        Self {
            semitones: semitones.clamp(-12, 12),
        }
    }

    pub fn set_semitones(&mut self, semitones: i32) {
        self.semitones = semitones.clamp(-12, 12);
    }

    pub fn get_semitones(&self) -> i32 {
        self.semitones
    }

    pub fn view(&self) -> Element<'static, Message> {
        row![
            text(format!("{}:", tr!(pitch_shift))).size(14),
            pick_list(
                &SEMITONE_OPTIONS[..],
                Some(SemitoneOption(self.semitones)),
                |opt| Message::PitchShiftChanged(opt.0)
            ),
        ]
        .spacing(5)
        .align_y(Alignment::Center)
        .into()
    }
}
