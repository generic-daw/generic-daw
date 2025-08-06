use crate::{
	icons::{LUCIDE_FONT, move_vertical, plus},
	stylefns::{button_with_radius, menu_with_border},
	widget::{DragHandle, LINE_HEIGHT},
};
use iced::{
	Alignment, Element, Font,
	Length::Fill,
	Shrink, Theme,
	border::{self, Radius},
	overlay::menu,
	widget::{
		Button, PickList, Scrollable, Space, Text, button, container, pick_list, row,
		scrollable::{self, Direction},
		text::Shaping,
		text_input,
	},
};
use std::borrow::Borrow;

pub fn space() -> Space {
	Space::new(Shrink, Shrink)
}

pub fn icon_button<'a, Message>(
	t: Text<'a>,
	style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
) -> Button<'a, Message>
where
	Message: Clone + 'a,
{
	button(
		container(t.size(13).line_height(1.0))
			.width(13)
			.align_x(Alignment::Center),
	)
	.style(button_with_radius(style, 0))
	.padding(0)
}

pub fn number_input<'a, Message>(
	current: usize,
	default: usize,
	max_digits: usize,
	drag_update: fn(usize) -> Message,
	text_update: fn(String) -> Message,
) -> Element<'a, Message>
where
	Message: Clone + 'a,
{
	row![
		DragHandle::new(
			container(move_vertical())
				.style(|t| {
					container::background(t.extended_palette().background.weakest.color).border(
						border::width(1)
							.color(t.extended_palette().background.strong.color)
							.rounded(border::left(5)),
					)
				})
				.padding([5, 0])
				.height(Fill),
			current,
			default,
			drag_update
		),
		text_input("", &current.to_string())
			.style(|t, s| {
				let mut style = text_input::default(t, s);
				style.border.radius = border::right(5);
				style
			})
			.font(Font::MONOSPACE)
			.width((max_digits as f32).mul_add(10.0, 14.0))
			.on_input(text_update)
	]
	.height(Shrink)
	.into()
}

pub fn circle_plus<'a, Message>() -> Button<'a, Message>
where
	Message: Clone + 'a,
{
	button(
		container(plus().size(LINE_HEIGHT + 6.0))
			.width(LINE_HEIGHT + 6.0)
			.align_x(Alignment::Center),
	)
	.style(|t, s| {
		let mut style = button::primary(t, s);
		style.border.radius = f32::INFINITY.into();
		style
	})
	.padding(5)
}

pub fn pick_list_custom_handle<'a, T, L, V, Message>(
	options: L,
	selected: Option<V>,
	on_selected: impl Fn(T) -> Message + 'a,
) -> PickList<'a, T, L, V, Message>
where
	T: ToString + PartialEq + Clone + 'a,
	L: Borrow<[T]> + 'a,
	V: Borrow<T> + 'a,
	Message: Clone,
{
	pick_list(options, selected, on_selected)
		.handle(pick_list::Handle::Dynamic {
			closed: pick_list::Icon {
				font: LUCIDE_FONT,
				code_point: const { char::from_u32(57457).unwrap() },
				size: None,
				line_height: 1.0.into(),
				shaping: Shaping::Advanced,
			},
			open: pick_list::Icon {
				font: LUCIDE_FONT,
				code_point: const { char::from_u32(57460).unwrap() },
				size: None,
				line_height: 1.0.into(),
				shaping: Shaping::Advanced,
			},
		})
		.menu_style(menu_with_border(menu::default, border::width(0)))
}

pub fn styled_scrollable_with_direction<'a, Message>(
	content: impl Into<Element<'a, Message>>,
	direction: impl Into<Direction>,
) -> Scrollable<'a, Message> {
	Scrollable::with_direction(content, direction)
		.spacing(5)
		.style(|t, s| {
			let mut style = scrollable::default(t, s);
			style.vertical_rail.border.radius = Radius::default();
			style.vertical_rail.scroller.border.radius = Radius::default();
			style.horizontal_rail.border.radius = Radius::default();
			style.horizontal_rail.scroller.border.radius = Radius::default();
			style
		})
}
