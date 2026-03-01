use crate::gui::messages::Message;
use iced::widget::{button, column, container, pick_list, row, rule, slider, text};
use iced::{Alignment, Color, Element, Length};

// ── Text sizes ──────────────────────────────────────────────────────────────
pub const TEXT_SIZE_SECTION_TITLE: f32 = 18.0;
pub const TEXT_SIZE_LABEL: f32 = 16.0;
pub const TEXT_SIZE_TAB: f32 = 15.0;
pub const TEXT_SIZE_INFO: f32 = 14.0;
pub const TEXT_SIZE_SMALL: f32 = 12.0;

// ── Spacing ─────────────────────────────────────────────────────────────────
pub const SPACING_TIGHT: f32 = 5.0;
pub const SPACING_NORMAL: f32 = 10.0;
pub const SPACING_WIDE: f32 = 15.0;
pub const SPACING_SECTION: f32 = 20.0;

// ── Padding ─────────────────────────────────────────────────────────────────
pub const PADDING_SMALL: f32 = 5.0;
pub const PADDING_NORMAL: f32 = 10.0;
pub const PADDING_LARGE: f32 = 20.0;

// ── Border radii ────────────────────────────────────────────────────────────
pub const BORDER_RADIUS_CARD: f32 = 5.0;
pub const BORDER_RADIUS_DIALOG: f32 = 10.0;

// ── Semantic colors ─────────────────────────────────────────────────────────
pub const COLOR_SUCCESS: Color = Color::from_rgb(0.3, 1.0, 0.3);
pub const COLOR_WARNING: Color = Color::from_rgb(1.0, 0.7, 0.3);
pub const COLOR_ERROR: Color = Color::from_rgb(1.0, 0.3, 0.3);
pub const COLOR_MUTED: Color = Color::from_rgb(0.5, 0.5, 0.5);
pub const COLOR_INACTIVE: Color = Color::from_rgb(0.4, 0.4, 0.4);
pub const COLOR_SUBTLE: Color = Color::from_rgb(0.7, 0.7, 0.7);

// ── Button dimensions ───────────────────────────────────────────────────────
pub const ICON_BUTTON_WIDTH: f32 = 30.0;
pub const TAB_BUTTON_PADDING: [f32; 2] = [8.0, 24.0];

pub fn labeled_slider<'a, F: 'a + Fn(f32) -> Message>(
    label: &'a str,
    range: std::ops::RangeInclusive<f32>,
    value: f32,
    on_change: F,
    format: impl Fn(f32) -> String + 'a,
    step: f32,
) -> Element<'a, Message> {
    row![
        text(label).width(Length::FillPortion(3)),
        slider(range, value, on_change)
            .width(Length::FillPortion(5))
            .step(step),
        text(format(value)).width(Length::FillPortion(2)),
    ]
    .spacing(SPACING_NORMAL)
    .align_y(Alignment::Center)
    .into()
}

pub fn icon_button(
    icon: &str,
    message: Option<Message>,
    style: fn(&iced::Theme, button::Status) -> iced::widget::button::Style,
) -> Element<'_, Message> {
    let btn = button(text(icon))
        .width(Length::Fixed(ICON_BUTTON_WIDTH))
        .style(style);

    if let Some(msg) = message {
        btn.on_press(msg).into()
    } else {
        btn.into()
    }
}

pub fn stage_header(
    stage_name: &str,
    idx: usize,
    is_collapsed: bool,
    can_move_up: bool,
    can_move_down: bool,
) -> Element<'_, Message> {
    let header_text = format!("{} {}", stage_name, idx + 1);

    let collapse_icon = if is_collapsed { "▶" } else { "▼" };
    let collapse_btn = icon_button(
        collapse_icon,
        Some(Message::ToggleStageCollapse(idx)),
        iced::widget::button::secondary,
    );

    let move_up_btn = if can_move_up {
        icon_button(
            "↑",
            Some(Message::MoveStageUp(idx)),
            iced::widget::button::primary,
        )
    } else {
        icon_button("↑", None, iced::widget::button::secondary)
    };

    let move_down_btn = if can_move_down {
        icon_button(
            "↓",
            Some(Message::MoveStageDown(idx)),
            iced::widget::button::primary,
        )
    } else {
        icon_button("↓", None, iced::widget::button::secondary)
    };

    let remove_btn = icon_button(
        "×",
        Some(Message::RemoveStage(idx)),
        iced::widget::button::danger,
    );

    row![
        collapse_btn,
        move_up_btn,
        move_down_btn,
        remove_btn,
        text(header_text)
    ]
    .spacing(SPACING_TIGHT)
    .align_y(Alignment::Center)
    .into()
}

pub fn stage_card<'a>(
    stage_name: &'a str,
    idx: usize,
    is_collapsed: bool,
    can_move_up: bool,
    can_move_down: bool,
    body: impl FnOnce() -> Element<'a, Message>,
) -> Element<'a, Message> {
    let header = stage_header(stage_name, idx, is_collapsed, can_move_up, can_move_down);

    let mut content = column![header].spacing(SPACING_TIGHT);

    if !is_collapsed {
        content = content.push(body());
    }

    let padding = if is_collapsed {
        PADDING_SMALL
    } else {
        PADDING_NORMAL
    };

    container(content.padding(padding))
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(BORDER_RADIUS_CARD))
        })
        .into()
}

// ── Shared helpers ──────────────────────────────────────────────────────────

/// Section title with a horizontal rule underneath.
/// Used for IO tab sections, IR cabinet, dialog sub-sections, etc.
pub fn section_title(label: &str) -> Element<'_, Message> {
    column![
        text(label)
            .size(TEXT_SIZE_SECTION_TITLE)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.palette().text),
            }),
        rule::horizontal(1),
    ]
    .spacing(SPACING_TIGHT)
    .into()
}

/// Standard card container with themed background, rounded border, and padding.
pub fn section_container(content: Element<'_, Message>) -> Element<'_, Message> {
    container(content)
        .width(Length::Fill)
        .padding(PADDING_NORMAL)
        .style(|theme: &iced::Theme| {
            container::Style::default()
                .background(theme.palette().background)
                .border(iced::Border::default().rounded(BORDER_RADIUS_CARD))
        })
        .into()
}

/// Labeled pick list — mirrors `labeled_slider` but for dropdowns.
/// Uses the same 3/7 fill-portion layout.
pub fn labeled_picker<'a, T>(
    label: &'a str,
    options: impl Into<Vec<T>>,
    selected: Option<T>,
    on_change: impl Fn(T) -> Message + 'a,
) -> Element<'a, Message>
where
    T: ToString + PartialEq + Clone + 'a,
{
    row![
        text(label).width(Length::FillPortion(3)),
        pick_list(options.into(), selected, on_change).width(Length::FillPortion(7)),
    ]
    .spacing(SPACING_NORMAL)
    .align_y(Alignment::Center)
    .into()
}
