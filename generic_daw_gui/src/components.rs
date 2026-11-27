use crate::{
	icons::{Icon, LUCIDE_FONT, chevron_down, chevron_up, move_vertical, rotate_ccw},
	stylefns::{
		button_with_radius, container_with_radius, weaker_bordered_box, weakest_bordered_box,
	},
	widget::{LINE_HEIGHT, TEXT_HEIGHT},
};
use generic_daw_widget::context_menu::ContextMenu;
use iced::{
	Element, Fill, Font, Length, Rectangle, Renderer, Size, Theme, border,
	mouse::{self, Cursor, Interaction, ScrollDelta},
	padding,
	widget::{
		Action, Button, button, component, container, mouse_area, pick_list, right, row, sensor,
		space, text, text_input,
	},
};
use std::{cell::Cell, ops::RangeInclusive};

pub fn file_tree_entry<'a, Message: 'a>(
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
		.padding(1)
		.spacing(2)
		.width(Fill),
	)
	.padding(0)
	.height(LINE_HEIGHT + 2.0)
	.style(style)
}

pub fn icon_button<'a, Message: 'a>(
	i: Icon,
	style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
) -> Button<'a, Message> {
	button(i.size(13.0))
		.style(button_with_radius(style, 0))
		.padding(1)
}

pub fn menu_entry<'a, Message: 'a>(
	i: impl Into<Option<Icon>>,
	l: impl text::IntoFragment<'a>,
	r: impl text::IntoFragment<'a>,
) -> Button<'a, Message> {
	button(
		row![
			i.into().map(|i| i.size(TEXT_HEIGHT)),
			text(l).line_height(1.0),
			right(text(r).line_height(1.0).style(text::secondary))
		]
		.spacing(5),
	)
	.style(button::text)
	.padding(5)
}

pub fn number_input<'a>(
	range: RangeInclusive<usize>,
	value: usize,
	default: usize,
	radius: impl Into<border::Radius>,
) -> Element<'a, Option<usize>> {
	struct DragHandle {
		range: RangeInclusive<usize>,
		value: usize,
		default: usize,
		radius: border::Radius,
	}

	#[derive(Default)]
	struct State {
		dragging: Option<(usize, f32)>,
		scroll: f32,
	}

	#[derive(Clone)]
	enum Event {
		Default,
		Press(f32),
		Drag(f32),
		Release,
		Scroll(f32),
	}

	impl<'a> component::Component<'a, usize> for DragHandle {
		type State = State;
		type Event = Event;

		fn update(
			&mut self,
			state: &mut Self::State,
			event: Self::Event,
			_renderer: &Renderer,
		) -> Option<usize> {
			match event {
				Event::Default => Some(self.default),
				Event::Press(y) => {
					state.dragging = Some((self.value, y));
					state.scroll = 0.0;
					None
				}
				Event::Drag(y) => state.dragging.and_then(|(start_value, start_y)| {
					let new_value = ((start_value as f32
						+ (start_y - y) * (self.range.end() - self.range.start()) as f32 * 0.0001)
						.round() as usize)
						.clamp(*self.range.start(), *self.range.end());
					(new_value != self.value).then_some(new_value)
				}),
				Event::Release => {
					state.dragging = None;
					state.scroll = 0.0;
					None
				}
				Event::Scroll(y) => {
					let mut diff = y + state.scroll;
					state.scroll = diff - diff.round();
					diff = diff.round();
					let new_value = ((self.value as f32 + diff).round() as usize)
						.clamp(*self.range.start(), *self.range.end());
					(new_value != self.value).then_some(new_value)
				}
			}
		}

		fn view(&self, _state: &Self::State) -> Element<'a, Self::Event> {
			ContextMenu::new(
				mouse_area(
					container(move_vertical())
						.style(container_with_radius(weakest_bordered_box, self.radius))
						.padding(padding::vertical(5)),
				)
				.interaction(Interaction::ResizingVertically),
				move || {
					container(
						menu_entry(rotate_ccw(), "Reset", "Ctrl-Click").on_press(Event::Default),
					)
					.width(160)
					.style(container_with_radius(weaker_bordered_box, 5))
					.into()
				},
			)
			.into()
		}

		fn listen(
			&self,
			state: &Self::State,
			event: &iced::Event,
			bounds: Rectangle,
			cursor: Cursor,
		) -> Action<Self::Event> {
			match event {
				iced::Event::Mouse(mouse::Event::ButtonPressed {
					button: mouse::Button::Left,
					modifiers,
				}) if state.dragging.is_none()
					&& cursor.is_over(bounds)
					&& let Some(cursor) = cursor.position() =>
				{
					return Action::publish(if modifiers.control() || modifiers.command() {
						Event::Default
					} else {
						Event::Press(cursor.y)
					})
					.and_capture();
				}
				iced::Event::Mouse(mouse::Event::ButtonReleased {
					button: mouse::Button::Left,
					..
				}) if state.dragging.is_some() => return Action::publish(Event::Release),
				iced::Event::Mouse(mouse::Event::CursorMoved { .. })
					if state.dragging.is_some()
						&& let Some(cursor) = cursor.land().position() =>
				{
					return Action::publish(Event::Drag(cursor.y)).and_capture();
				}
				iced::Event::Mouse(mouse::Event::WheelScrolled { delta, modifiers })
					if state.dragging.is_none() && cursor.is_over(bounds) =>
				{
					return Action::publish(Event::Scroll(
						match delta {
							ScrollDelta::Lines { y, .. } => *y,
							ScrollDelta::Pixels { y, .. } => y / 60.0,
						} * if modifiers.command() { 10.0 } else { 1.0 },
					))
					.and_capture();
				}
				_ => {}
			}

			Action::none()
		}

		fn mouse_interaction(&self, state: &Self::State) -> Interaction {
			if state.dragging.is_some() {
				Interaction::ResizingVertically
			} else {
				Interaction::default()
			}
		}
	}

	let radius = radius.into();
	let max_digits = (range.end() + 1).ilog10();
	row![
		component(DragHandle {
			range,
			value,
			default,
			radius
		})
		.map(Some),
		text_input("", value)
			.style(move |t, s| {
				let mut style = text_input::default(t, s);
				style.border.radius = radius.left(0);
				style
			})
			.font(Font::MONOSPACE)
			.width(max_digits * 10 + 14)
			.on_input(|text| text.parse().ok())
	]
	.into()
}

pub fn pick_list_handle() -> pick_list::Handle {
	pick_list::Handle::Dynamic {
		closed: pick_list::Icon {
			font: LUCIDE_FONT,
			code_point: chevron_down().glyph(),
			size: None,
			line_height: None,
			shaping: text::Shaping::Basic,
		},
		open: pick_list::Icon {
			font: LUCIDE_FONT,
			code_point: chevron_up().glyph(),
			size: None,
			line_height: None,
			shaping: text::Shaping::Basic,
		},
	}
}

pub fn text_icon_button<'a, Message: 'a>(
	i: impl text::IntoFragment<'a>,
	style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
) -> Button<'a, Message> {
	button(
		text(i)
			.size(13)
			.line_height(1.0)
			.width(13)
			.height(13)
			.center(),
	)
	.style(button_with_radius(style, 0))
	.padding(1)
}

pub fn virtualized<'a, Message: 'static>(
	f: impl Fn() -> Element<'a, Message> + 'a,
) -> Element<'a, Message> {
	struct Virtualized<'a, Message> {
		cache: Cell<Option<Element<'a, Message>>>,
		size: Size<Length>,
		f: Box<dyn Fn() -> Element<'a, Message> + 'a>,
	}

	impl<'a, Message: 'static> component::Component<'a, Message> for Virtualized<'a, Message> {
		type State = bool;
		type Event = Option<Message>;

		fn update(
			&mut self,
			state: &mut Self::State,
			event: Self::Event,
			_renderer: &Renderer,
		) -> Option<Message> {
			*state ^= event.is_none();
			event
		}

		fn view(&self, state: &Self::State) -> Element<'a, Self::Event> {
			sensor(if *state {
				self.cache.take().unwrap_or_else(&self.f).map(Some)
			} else {
				space()
					.width(self.size.width)
					.height(self.size.height)
					.into()
			})
			.on_show(|_| None)
			.on_hide(None)
			.into()
		}
	}

	let cache = f();
	component(Virtualized {
		size: cache.as_widget().size(),
		cache: Cell::new(Some(cache)),
		f: Box::new(f),
	})
}
