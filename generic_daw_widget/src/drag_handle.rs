use iced_widget::{
	Renderer,
	core::{
		Element, Event, Layout, Length, Point, Rectangle, Shell, Size, Theme, Vector, Widget,
		layout::{Limits, Node},
		mouse::{self, Cursor, Interaction, ScrollDelta},
		overlay,
		renderer::Style,
		widget::{Operation, Tree, tree},
	},
};
use std::ops::RangeInclusive;

#[derive(Default)]
struct State {
	dragging: Option<(usize, f32)>,
	hovering: bool,
	scroll: f32,
}

pub struct DragHandle<'a, Message> {
	child: Element<'a, Message, Theme, Renderer>,
	range: RangeInclusive<usize>,
	value: usize,
	default: usize,
	f: Box<dyn Fn(usize) -> Message + 'a>,
}

impl<'a, Message> DragHandle<'a, Message> {
	#[must_use]
	pub fn new(
		child: impl Into<Element<'a, Message, Theme, Renderer>>,
		range: RangeInclusive<usize>,
		value: usize,
		f: impl Fn(usize) -> Message + 'a,
	) -> Self {
		Self {
			child: child.into(),
			range: range.clone(),
			value,
			default: *range.end(),
			f: Box::from(f),
		}
	}

	#[must_use]
	pub fn default(mut self, default: usize) -> Self {
		self.default = default;
		self
	}
}

impl<Message> Widget<Message, Theme, Renderer> for DragHandle<'_, Message> {
	fn size(&self) -> Size<Length> {
		self.child.as_widget().size()
	}

	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State::default())
	}

	fn diff(&mut self, tree: &mut Tree) {
		tree.diff_children(&mut [&mut self.child]);
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		self.child
			.as_widget_mut()
			.layout(&mut tree.children[0], renderer, limits)
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		cursor: Cursor,
		renderer: &Renderer,
		shell: &mut Shell<'_, Message>,
		viewport: &Rectangle,
	) {
		self.child.as_widget_mut().update(
			&mut tree.children[0],
			event,
			layout,
			cursor,
			renderer,
			shell,
			viewport,
		);

		if shell.is_event_captured() {
			return;
		}

		if let Event::Mouse(event) = event {
			let state = tree.state.downcast_mut::<State>();

			match event {
				mouse::Event::ButtonPressed {
					button: mouse::Button::Left,
					modifiers,
				} if state.dragging.is_none() && state.hovering => {
					let pos = cursor.position().unwrap();
					state.dragging = Some((self.value, pos.y));
					state.scroll = 0.0;

					if modifiers.control() || modifiers.command() {
						shell.publish((self.f)(self.default));
					}

					shell.capture_event();
				}
				mouse::Event::ButtonReleased {
					button: mouse::Button::Left,
					..
				} if state.dragging.is_some() => {
					state.dragging = None;
					state.scroll = 0.0;
					shell.capture_event();
				}
				mouse::Event::CursorMoved {
					position: Point { y, .. },
					..
				} => {
					if let Some((start_value, start_y)) = state.dragging {
						let new_value = ((start_value as f32
							+ (start_y - y)
								* (self.range.end() - self.range.start()) as f32
								* 0.0001)
							.round() as usize)
							.clamp(*self.range.start(), *self.range.end());

						if new_value != self.value {
							shell.publish((self.f)(new_value));
						}

						shell.capture_event();
					}

					state.hovering = cursor.is_over(layout.bounds());
				}
				mouse::Event::WheelScrolled {
					delta, modifiers, ..
				} if state.dragging.is_none() && state.hovering => {
					let mut diff = match delta {
						ScrollDelta::Lines { y, .. } => *y,
						ScrollDelta::Pixels { y, .. } => y / 60.0,
					} * if modifiers.command() { 10.0 } else { 1.0 }
						+ state.scroll;

					state.scroll = diff - diff.round();
					diff = diff.round();

					let new_value = ((self.value as f32 + diff).round() as usize)
						.clamp(*self.range.start(), *self.range.end());
					if new_value != self.value {
						shell.publish((self.f)(new_value));
						shell.capture_event();
					}
				}
				_ => {}
			}
		}
	}

	fn draw(
		&self,
		tree: &Tree,
		renderer: &mut Renderer,
		theme: &Theme,
		style: &Style,
		layout: Layout<'_>,
		cursor: Cursor,
		viewport: &Rectangle,
	) {
		self.child.as_widget().draw(
			&tree.children[0],
			renderer,
			theme,
			style,
			layout,
			cursor,
			viewport,
		);
	}

	fn mouse_interaction(
		&self,
		tree: &Tree,
		layout: Layout<'_>,
		cursor: Cursor,
		viewport: &Rectangle,
		renderer: &Renderer,
	) -> Interaction {
		let state = tree.state.downcast_ref::<State>();

		if state.dragging.is_some() {
			Interaction::ResizingVertically
		} else if state.hovering {
			let interaction = self.child.as_widget().mouse_interaction(
				&tree.children[0],
				layout,
				cursor,
				viewport,
				renderer,
			);

			if interaction == Interaction::default() {
				Interaction::ResizingVertically
			} else {
				interaction
			}
		} else {
			Interaction::default()
		}
	}

	fn overlay<'a>(
		&'a mut self,
		tree: &'a mut Tree,
		layout: Layout<'a>,
		renderer: &Renderer,
		viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
		self.child.as_widget_mut().overlay(
			&mut tree.children[0],
			layout,
			renderer,
			viewport,
			translation,
		)
	}

	fn operate(
		&mut self,
		tree: &mut Tree,
		layout: Layout<'_>,
		renderer: &Renderer,
		operation: &mut dyn Operation,
	) {
		operation.container(None, layout.bounds());
		operation.traverse(&mut |operation| {
			self.child
				.as_widget_mut()
				.operate(&mut tree.children[0], layout, renderer, operation);
		});
	}
}

impl<'a, Message: 'a> From<DragHandle<'a, Message>> for Element<'a, Message, Theme, Renderer> {
	fn from(value: DragHandle<'a, Message>) -> Self {
		Self::new(value)
	}
}
