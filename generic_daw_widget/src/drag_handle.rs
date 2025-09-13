use generic_daw_utils::NoDebug;
use iced_widget::{
	Renderer,
	core::{
		Clipboard, Element, Event, Layout, Length, Point, Rectangle, Shell, Size, Theme, Vector,
		Widget,
		layout::{Limits, Node},
		mouse::{self, Click, Cursor, Interaction, ScrollDelta, click::Kind},
		overlay,
		renderer::Style,
		widget::{Operation, Tree, tree},
	},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Strategy {
	Horizontal,
	Vertical,
}

#[derive(Default)]
struct State {
	dragging: Option<(usize, f32)>,
	hovering: bool,
	scroll: f32,
	last_click: Option<Click>,
}

#[derive(Debug)]
pub struct DragHandle<'a, Message> {
	child: NoDebug<Element<'a, Message, Theme, Renderer>>,
	value: usize,
	reset: usize,
	strategy: Strategy,
	f: NoDebug<Box<dyn Fn(usize) -> Message + 'a>>,
}

impl<'a, Message> DragHandle<'a, Message> {
	#[must_use]
	pub fn new(
		child: impl Into<Element<'a, Message, Theme, Renderer>>,
		value: usize,
		reset: usize,
		f: impl Fn(usize) -> Message + 'a,
	) -> Self {
		Self {
			child: child.into().into(),
			value,
			reset,
			strategy: Strategy::Vertical,
			f: NoDebug(Box::from(f)),
		}
	}

	#[must_use]
	pub fn strategy(mut self, strategy: Strategy) -> Self {
		self.strategy = strategy;
		self
	}
}

impl<Message> Widget<Message, Theme, Renderer> for DragHandle<'_, Message> {
	fn size(&self) -> Size<Length> {
		self.child.as_widget().size()
	}

	fn size_hint(&self) -> Size<Length> {
		self.child.as_widget().size_hint()
	}

	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State::default())
	}

	fn children(&self) -> Vec<Tree> {
		vec![Tree::new(&*self.child)]
	}

	fn diff(&self, tree: &mut Tree) {
		tree.diff_children(&[&*self.child]);
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
		clipboard: &mut dyn Clipboard,
		shell: &mut Shell<'_, Message>,
		viewport: &Rectangle,
	) {
		self.child.as_widget_mut().update(
			&mut tree.children[0],
			event,
			layout,
			cursor,
			renderer,
			clipboard,
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
					..
				} if state.dragging.is_none() && state.hovering => {
					let pos = cursor.position().unwrap();
					state.dragging = Some((
						self.value,
						match self.strategy {
							Strategy::Horizontal => pos.x,
							Strategy::Vertical => pos.y,
						},
					));

					let new_click = Click::new(pos, mouse::Button::Left, state.last_click);
					state.last_click = Some(new_click);

					if new_click.kind() == Kind::Double {
						shell.publish((self.f)(self.reset));
					}

					shell.capture_event();
				}
				mouse::Event::ButtonReleased(mouse::Button::Left) if state.dragging.is_some() => {
					state.dragging = None;
					shell.capture_event();
				}
				mouse::Event::CursorMoved {
					position: Point { x, y },
					..
				} => {
					if let Some((value, pos)) = state.dragging {
						let diff = ((pos
							- match self.strategy {
								Strategy::Horizontal => x,
								Strategy::Vertical => y,
							}) * 0.1)
							.trunc();

						shell.publish((self.f)(value.saturating_add_signed(diff as isize)));
						shell.capture_event();
					}

					state.hovering = cursor.is_over(layout.bounds());
				}
				mouse::Event::WheelScrolled { delta, .. }
					if state.dragging.is_none() && state.hovering =>
				{
					let diff = match delta {
						ScrollDelta::Lines { y, .. } => *y,
						ScrollDelta::Pixels { y, .. } => y / 60.0,
					} + state.scroll;
					state.scroll = diff.fract();

					shell.publish((self.f)(self.value.saturating_add_signed(diff as isize)));
					shell.capture_event();
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
		if tree.state.downcast_ref::<State>().dragging.is_some() {
			match self.strategy {
				Strategy::Horizontal => Interaction::ResizingHorizontally,
				Strategy::Vertical => Interaction::ResizingVertically,
			}
		} else if cursor.is_over(layout.bounds()) {
			let interaction = self.child.as_widget().mouse_interaction(
				&tree.children[0],
				layout,
				cursor,
				viewport,
				renderer,
			);

			if interaction == Interaction::default() {
				match self.strategy {
					Strategy::Horizontal => Interaction::ResizingHorizontally,
					Strategy::Vertical => Interaction::ResizingVertically,
				}
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
		operation.container(None, layout.bounds(), &mut |operation| {
			self.child
				.as_widget_mut()
				.operate(&mut tree.children[0], layout, renderer, operation);
		});
	}
}

impl<'a, Message> From<DragHandle<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
	Message: 'a,
{
	fn from(value: DragHandle<'a, Message>) -> Self {
		Self::new(value)
	}
}
