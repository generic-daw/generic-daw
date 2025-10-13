use crate::{
	icons::{Icon, LUCIDE_FONT, move_vertical},
	stylefns::{bordered_box_with_radius, button_with_radius},
};
use generic_daw_widget::drag_handle::DragHandle;
use iced::{
	Element, Font,
	Length::Fill,
	Shrink, Theme, border,
	widget::{Button, Text, button, container, pick_list, row, text, text_input},
};

pub fn icon_button<'a, Message>(
	t: Icon,
	style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
) -> Button<'a, Message>
where
	Message: Clone + 'a,
{
	button(t.size(13.0))
		.style(button_with_radius(style, 0))
		.padding(1)
}

pub fn text_icon_button<'a, Message>(
	t: impl Into<Text<'a>>,
	style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
) -> Button<'a, Message>
where
	Message: Clone + 'a,
{
	button(container(t.into().size(13).line_height(1.0)).center_x(13))
		.style(button_with_radius(style, 0))
		.padding(1)
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
				.style(|t| bordered_box_with_radius(border::left(5))(t)
					.background(t.extended_palette().background.weakest.color))
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

pub const PICK_LIST_HANDLE: pick_list::Handle<Font> = pick_list::Handle::Dynamic {
	closed: pick_list::Icon {
		font: LUCIDE_FONT,
		code_point: char::from_u32(57453).unwrap(),
		size: None,
		line_height: text::LineHeight::Relative(1.0),
		shaping: text::Shaping::Basic,
	},
	open: pick_list::Icon {
		font: LUCIDE_FONT,
		code_point: char::from_u32(57456).unwrap(),
		size: None,
		line_height: text::LineHeight::Relative(1.0),
		shaping: text::Shaping::Basic,
	},
};
