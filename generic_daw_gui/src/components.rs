use crate::{
	icons::{Icon, LUCIDE_FONT, chevron_down, chevron_up, move_vertical},
	stylefns::{button_with_radius, container_with_radius, weakest_bordered_box},
	widget::LINE_HEIGHT,
};
use generic_daw_widget::drag_handle::DragHandle;
use iced::{
	Element, Font, Shrink, Theme, border, padding,
	widget::{Button, button, container, pick_list, row, text, text_input},
};
use std::ops::RangeInclusive;

pub fn icon_button<'a, Message: 'a>(
	i: Icon,
	style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
) -> Button<'a, Message> {
	button(i.size(13.0))
		.style(button_with_radius(style, 0))
		.padding(1)
}

pub fn text_icon_button<'a, Message: 'a>(
	i: impl text::IntoFragment<'a>,
	style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
) -> Button<'a, Message> {
	button(container(text(i).size(13).line_height(1.0)).center_x(13))
		.style(button_with_radius(style, 0))
		.padding(1)
}

pub fn labeled_icon_button<'a, Message: 'a>(
	i: Icon,
	l: impl text::IntoFragment<'a>,
	style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
) -> Button<'a, Message> {
	button(
		row![
			i.size(LINE_HEIGHT),
			text(l)
				.wrapping(text::Wrapping::None)
				.ellipsis(text::Ellipsis::End)
		]
		.spacing(2),
	)
	.style(button_with_radius(style, 0))
	.padding(1)
}

pub fn number_input<'a, Message: Clone + 'a>(
	range: RangeInclusive<usize>,
	value: usize,
	default: usize,
	drag_update: fn(usize) -> Message,
	text_update: fn(String) -> Message,
	radius: impl Into<border::Radius>,
) -> Element<'a, Message> {
	let radius = radius.into();
	let max_digits = (range.end() + 1).ilog10();
	row![
		DragHandle::new(
			container(move_vertical())
				.style(container_with_radius(weakest_bordered_box, radius.right(0)))
				.padding(padding::vertical(5)),
			range,
			value,
			drag_update
		)
		.default(default),
		text_input("", &value.to_string())
			.style(move |t, s| {
				let mut style = text_input::default(t, s);
				style.border.radius = radius.left(0);
				style
			})
			.font(Font::MONOSPACE)
			.width(max_digits * 10 + 14)
			.on_input(text_update)
	]
	.height(Shrink)
	.into()
}

pub const PICK_LIST_HANDLE: pick_list::Handle<Font> = pick_list::Handle::Dynamic {
	closed: pick_list::Icon {
		font: LUCIDE_FONT,
		code_point: chevron_down().glyph(),
		size: None,
		line_height: text::LineHeight::Relative(1.0),
		shaping: text::Shaping::Basic,
	},
	open: pick_list::Icon {
		font: LUCIDE_FONT,
		code_point: chevron_up().glyph(),
		size: None,
		line_height: text::LineHeight::Relative(1.0),
		shaping: text::Shaping::Basic,
	},
};
