use crate::{
	icons::move_vertical,
	stylefns::{container_with_radius, weakest_bordered_box},
};
use iced::{
	Element, Point, Rectangle, Renderer, border,
	mouse::{self, Cursor, Interaction, ScrollDelta},
	padding,
	widget::{Action, component, container, mouse_area},
};
use std::ops::RangeInclusive;

pub fn drag_handle<'a>(
	range: RangeInclusive<usize>,
	value: usize,
	default: usize,
	radius: impl Into<border::Radius>,
) -> Element<'a, usize> {
	#[derive(Default)]
	struct State {
		dragging: Option<(usize, f32)>,
		scroll: f32,
	}

	#[derive(Clone)]
	enum Event {
		Default(f32),
		Press(f32),
		Drag(f32),
		Release,
		Scroll(f32),
	}

	struct DragHandle {
		range: RangeInclusive<usize>,
		value: usize,
		default: usize,
		radius: border::Radius,
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
				Event::Default(y) => {
					state.dragging = Some((self.default, y));
					Some(self.default)
				}
				Event::Press(y) => {
					state.dragging = Some((self.value, y));
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
			mouse_area(
				container(move_vertical())
					.style(container_with_radius(weakest_bordered_box, self.radius))
					.padding(padding::vertical(5)),
			)
			.interaction(Interaction::ResizingVertically)
			.into()
		}

		fn listen(
			&self,
			state: &Self::State,
			event: &iced::Event,
			bounds: Rectangle,
			cursor: Cursor,
		) -> Action<Self::Event> {
			if let iced::Event::Mouse(event) = event {
				match event {
					mouse::Event::ButtonPressed {
						button: mouse::Button::Left,
						modifiers,
					} if state.dragging.is_none() && cursor.is_over(bounds) => {
						let pos = cursor.position().unwrap();
						return Action::publish(if modifiers.control() || modifiers.command() {
							Event::Default
						} else {
							Event::Press
						}(pos.y))
						.and_capture();
					}
					mouse::Event::ButtonReleased {
						button: mouse::Button::Left,
						..
					} if state.dragging.is_some() => return Action::publish(Event::Release).and_capture(),
					mouse::Event::CursorMoved {
						position: Point { y, .. },
						..
					} if state.dragging.is_some() => return Action::publish(Event::Drag(*y)).and_capture(),
					mouse::Event::WheelScrolled {
						delta, modifiers, ..
					} if state.dragging.is_none() && cursor.is_over(bounds) => {
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

	component(DragHandle {
		range,
		value,
		default,
		radius: radius.into(),
	})
}
