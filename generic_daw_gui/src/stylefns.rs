use iced::{
    Color, Theme,
    widget::{radio, slider},
};

pub fn radio_secondary(theme: &Theme, status: radio::Status) -> radio::Style {
    let color = match status {
        radio::Status::Active { .. } => Color::TRANSPARENT,
        radio::Status::Hovered { .. } => theme.extended_palette().secondary.weak.color,
    };

    let mut style = radio::default(theme, status);
    style.border_color = theme.extended_palette().secondary.strong.color;
    style.dot_color = theme.extended_palette().secondary.strong.color;
    style.background = color.into();

    style
}

pub fn slider_secondary(theme: &Theme, status: slider::Status) -> slider::Style {
    let color = match status {
        slider::Status::Active => theme.extended_palette().secondary.base.color,
        slider::Status::Hovered => theme.extended_palette().secondary.strong.color,
        slider::Status::Dragged => theme.extended_palette().secondary.weak.color,
    };

    let mut style = slider::default(theme, status);
    style.rail.backgrounds.0 = color.into();
    style.handle.background = color.into();

    style
}
