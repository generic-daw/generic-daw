use iced::{
	Theme, border,
	overlay::menu,
	widget::{button, container, pick_list, progress_bar, scrollable, slider},
};

pub fn bordered_box_with_radius(
	r: impl Into<border::Radius>,
) -> impl Fn(&Theme) -> container::Style {
	let r = r.into();
	move |t| {
		container::background(t.extended_palette().background.weak.color).border(
			border::width(1)
				.color(t.extended_palette().background.strong.color)
				.rounded(r),
		)
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
		style.placeholder_color = t.extended_palette().background.weak.text;
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

pub fn slider_secondary(theme: &Theme, status: slider::Status) -> slider::Style {
	let palette = theme.extended_palette();

	let color = match status {
		slider::Status::Active => palette.secondary.base.color,
		slider::Status::Hovered => palette.secondary.strong.color,
		slider::Status::Dragged => palette.secondary.weak.color,
	};

	let mut style = slider::default(theme, status);
	style.rail.backgrounds.0 = color.into();
	style.handle.background = color.into();

	style
}

pub fn split_style(t: &Theme) -> iced_split::Style {
	let mut style = iced_split::default(t);
	style.focused = iced_split::Styled {
		color: t.extended_palette().background.strong.color,
		width: 3.0,
		radius: 1.5.into(),
	};
	style
}
