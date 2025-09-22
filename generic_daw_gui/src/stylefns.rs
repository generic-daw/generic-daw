use iced::{
	Border, Theme,
	border::{self, Radius},
	overlay::menu,
	widget::{button, container, pick_list, progress_bar, slider},
};

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

pub fn button_with_radius(
	f: impl Fn(&Theme, button::Status) -> button::Style,
	r: impl Into<Radius>,
) -> impl Fn(&Theme, button::Status) -> button::Style {
	let r = r.into();
	move |t, s| {
		let mut style = f(t, s);
		style.border.radius = r;
		style
	}
}

pub fn pick_list_with_radius(
	f: impl Fn(&Theme, pick_list::Status) -> pick_list::Style,
	r: impl Into<Radius>,
) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
	let r = r.into();
	move |t, s| {
		let mut style = f(t, s);
		style.border.radius = r;
		style.placeholder_color = t.extended_palette().background.weak.text;
		style
	}
}

pub fn progress_bar_with_radius(
	f: impl Fn(&Theme) -> progress_bar::Style,
	r: impl Into<Radius>,
) -> impl Fn(&Theme) -> progress_bar::Style {
	let r = r.into();
	move |t| {
		let mut style = f(t);
		style.border.radius = r;
		style
	}
}

pub fn menu_with_border(
	f: impl Fn(&Theme) -> menu::Style,
	r: impl Into<Border>,
) -> impl Fn(&Theme) -> menu::Style {
	let r = r.into();
	move |t| {
		let mut style = f(t);
		style.border = r;
		style
	}
}

pub fn bordered_box_with_radius(r: impl Into<Radius>) -> impl Fn(&Theme) -> container::Style {
	let r = r.into();
	move |t| {
		container::background(t.extended_palette().background.weak.color).border(
			border::width(1)
				.color(t.extended_palette().background.strong.color)
				.rounded(r),
		)
	}
}
