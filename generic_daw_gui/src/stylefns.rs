use iced::{
    Color, Theme,
    border::Radius,
    widget::{button, radio, slider, svg},
};

pub fn button_with_enabled(theme: &Theme, status: button::Status, enabled: bool) -> button::Style {
    let mut style = if enabled {
        button::primary(theme, status)
    } else {
        button::secondary(theme, status)
    };
    style.border.radius = Radius::default();
    style
}

pub fn radio_with_enabled(theme: &Theme, status: radio::Status, enabled: bool) -> radio::Style {
    let (strong, weak) = if enabled {
        (
            theme.extended_palette().primary.strong.color,
            match status {
                radio::Status::Active { .. } => Color::TRANSPARENT,
                radio::Status::Hovered { .. } => theme.extended_palette().primary.weak.color,
            },
        )
    } else {
        (
            theme.extended_palette().secondary.strong.color,
            match status {
                radio::Status::Active { .. } => Color::TRANSPARENT,
                radio::Status::Hovered { .. } => theme.extended_palette().secondary.weak.color,
            },
        )
    };

    let mut style = radio::default(theme, status);
    style.border_color = strong;
    style.dot_color = strong;
    style.background = weak.into();

    style
}

pub fn slider_with_enabled(theme: &Theme, status: slider::Status, enabled: bool) -> slider::Style {
    let color = if enabled {
        match status {
            slider::Status::Active => theme.extended_palette().primary.base.color,
            slider::Status::Hovered => theme.extended_palette().primary.strong.color,
            slider::Status::Dragged => theme.extended_palette().primary.weak.color,
        }
    } else {
        match status {
            slider::Status::Active => theme.extended_palette().secondary.base.color,
            slider::Status::Hovered => theme.extended_palette().secondary.strong.color,
            slider::Status::Dragged => theme.extended_palette().secondary.weak.color,
        }
    };

    let mut style = slider::default(theme, status);
    style.rail.backgrounds.0 = color.into();
    style.handle.background = color.into();

    style
}

pub fn svg_with_enabled(theme: &Theme, status: svg::Status, enabled: bool) -> svg::Style {
    let color = match status {
        svg::Status::Idle => {
            if enabled {
                theme.extended_palette().primary.base.text
            } else {
                theme.extended_palette().secondary.base.text
            }
        }
        svg::Status::Hovered => {
            if enabled {
                theme.extended_palette().primary.strong.text
            } else {
                theme.extended_palette().secondary.strong.text
            }
        }
    };

    svg::Style { color: Some(color) }
}
