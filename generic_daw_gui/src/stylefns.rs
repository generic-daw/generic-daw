use iced::{
	Color, Theme, border,
	overlay::menu,
	widget::{button, container, pick_list, progress_bar, scrollable, slider, text_input},
};
use sweeten::widget::{column, row};

pub fn button_warning_text(t: &Theme, s: button::Status) -> button::Style {
	let base = button::Style {
		text_color: t.palette().warning.base.color,
		..button::Style::default()
	};

	match s {
		button::Status::Active | button::Status::Pressed => base,
		button::Status::Hovered => button::Style {
			text_color: base.text_color.scale_alpha(0.8),
			..base
		},
		button::Status::Disabled => button::Style {
			text_color: base.text_color.scale_alpha(0.5),
			..base
		},
	}
}

pub fn button_with_radius(
	f: impl Fn(&Theme, button::Status) -> button::Style,
	r: impl Into<border::Radius>,
) -> impl Fn(&Theme, button::Status) -> button::Style {
	let r = r.into();
	move |t, s| {
		let mut style = f(t, s);
		style.border.radius = r;
		style
	}
}

pub fn container_with_radius(
	f: impl Fn(&Theme) -> container::Style,
	r: impl Into<border::Radius>,
) -> impl Fn(&Theme) -> container::Style {
	let r = r.into();
	move |t| {
		let mut style = f(t);
		style.border.radius = r;
		style
	}
}

pub fn menu_style(t: &Theme) -> menu::Style {
	let mut style = menu::default(t);
	style.border = border::width(0);
	style
}

pub fn pick_list_with_radius(
	r: impl Into<border::Radius>,
) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
	let r = r.into();
	move |t, s| {
		let mut style = pick_list::default(t, s);
		style.border.radius = r;
		style.placeholder_color = t.palette().background.weak.text;
		style
	}
}

pub fn progress_bar_with_radius(
	f: impl Fn(&Theme) -> progress_bar::Style,
	r: impl Into<border::Radius>,
) -> impl Fn(&Theme) -> progress_bar::Style {
	let r = r.into();
	move |t| {
		let mut style = f(t);
		style.border.radius = r;
		style
	}
}

pub fn scrollable_style(t: &Theme, s: scrollable::Status) -> scrollable::Style {
	let mut style = scrollable::default(t, s);
	style.vertical_rail.border.radius = 0.into();
	style.vertical_rail.scroller.border.radius = 0.into();
	style.horizontal_rail.border.radius = 0.into();
	style.horizontal_rail.scroller.border.radius = 0.into();
	style
}

pub fn selectable_box(
	f: impl Fn(&Theme) -> container::Style,
	s: bool,
) -> impl Fn(&Theme) -> container::Style {
	move |t| {
		if s {
			f(t).border(border::width(1.5).color(t.palette().primary.base.color))
		} else {
			f(t)
		}
	}
}

pub fn slider_with_radius(
	f: impl Fn(&Theme, slider::Status) -> slider::Style,
	r: impl Into<border::Radius>,
) -> impl Fn(&Theme, slider::Status) -> slider::Style {
	let r = r.into();
	move |t, s| {
		let mut style = f(t, s);
		style.handle.border.radius = r;
		style
	}
}

pub fn split_style(t: &Theme) -> iced_split::Style {
	let mut style = iced_split::default(t);
	style.focused = iced_split::StyleSheet {
		color: t.palette().background.strong.color,
		width: 3.0,
		radius: 1.5.into(),
	};
	style
}

pub fn sweeten_column_style(t: &Theme) -> column::Style {
	let mut style = column::default(t);
	style.scale = 1.0;
	style.moved_item_overlay = Color::TRANSPARENT;
	style
}

pub fn sweeten_column_with_radius(
	f: impl Fn(&Theme) -> column::Style,
	r: impl Into<border::Radius>,
) -> impl Fn(&Theme) -> column::Style {
	let r = r.into();
	move |t| {
		let mut style = f(t);
		style.ghost_border.radius = r;
		style
	}
}

pub fn sweeten_row_style(t: &Theme) -> row::Style {
	let mut style = row::default(t);
	style.scale = 1.0;
	style.moved_item_overlay = Color::TRANSPARENT;
	style
}

pub fn sweeten_row_with_radius(
	f: impl Fn(&Theme) -> row::Style,
	r: impl Into<border::Radius>,
) -> impl Fn(&Theme) -> row::Style {
	let r = r.into();
	move |t| {
		let mut style = f(t);
		style.ghost_border.radius = r;
		style
	}
}

pub fn text_input_transparent(t: &Theme, _s: text_input::Status) -> text_input::Style {
	let palette = t.palette();

	text_input::Style {
		background: Color::TRANSPARENT.into(),
		border: border::width(0),
		placeholder: palette.background.base.text.scale_alpha(0.8),
		value: palette.background.base.text,
		selection: palette.primary.weak.color,
	}
}

pub fn text_input_with_radius(
	f: impl Fn(&Theme, text_input::Status) -> text_input::Style,
	r: impl Into<border::Radius>,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
	let r = r.into();
	move |t, s| {
		let mut style = f(t, s);
		style.border.radius = r;
		style
	}
}

pub fn weak_bordered_box(t: &Theme) -> container::Style {
	container::background(t.palette().background.weak.color)
		.border(border::width(1).color(t.palette().background.strong.color))
}

pub fn weaker_bordered_box(t: &Theme) -> container::Style {
	container::background(t.palette().background.weaker.color)
		.border(border::width(1).color(t.palette().background.strong.color))
}

pub fn weakest_bordered_box(t: &Theme) -> container::Style {
	container::background(t.palette().background.weakest.color)
		.border(border::width(1).color(t.palette().background.strong.color))
}
