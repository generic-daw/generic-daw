use iced_widget::{
	Renderer, Theme,
	core::{
		Element, Event, Layout, Length, Point, Rectangle, Renderer as _, Shell, Size, Vector,
		Widget, keyboard,
		layout::{Limits, Node},
		mouse::{self, Cursor, Interaction},
		overlay,
		renderer::Style,
		widget::{Operation, Tree, tree},
	},
};

struct State {
	position: Option<Point>,
}

pub struct ContextMenu<'a, Message> {
	content: Element<'a, Message, Theme, Renderer>,
	context_menu: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message> ContextMenu<'a, Message> {
	pub fn new(
		content: impl Into<Element<'a, Message, Theme, Renderer>>,
		context_menu: impl Into<Element<'a, Message, Theme, Renderer>>,
	) -> Self {
		Self {
			content: content.into(),
			context_menu: context_menu.into(),
		}
	}
}

impl<Message> Widget<Message, Theme, Renderer> for ContextMenu<'_, Message> {
	fn size(&self) -> Size<Length> {
		self.content.as_widget().size()
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		self.content
			.as_widget_mut()
			.layout(&mut tree.children[0], renderer, limits)
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
		self.content.as_widget().draw(
			&tree.children[0],
			renderer,
			theme,
			style,
			layout,
			cursor,
			viewport,
		);
	}

	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State { position: None })
	}

	fn diff(&mut self, tree: &mut Tree) {
		tree.diff_children(&mut [&mut self.content, &mut self.context_menu]);
	}

	fn operate(
		&mut self,
		tree: &mut Tree,
		layout: Layout<'_>,
		renderer: &Renderer,
		operation: &mut dyn Operation,
	) {
		self.content
			.as_widget_mut()
			.operate(&mut tree.children[0], layout, renderer, operation);
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
		self.content.as_widget_mut().update(
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

		if let Event::Mouse(mouse::Event::ButtonPressed {
			button: mouse::Button::Right,
			..
		}) = event && let Some(position) = cursor.position()
			&& layout.bounds().contains(position)
		{
			tree.state.downcast_mut::<State>().position = Some(position);
			shell.capture_event();
			shell.request_redraw();
		}
	}

	fn mouse_interaction(
		&self,
		tree: &Tree,
		layout: Layout<'_>,
		cursor: Cursor,
		viewport: &Rectangle,
		renderer: &Renderer,
	) -> Interaction {
		self.content.as_widget().mouse_interaction(
			&tree.children[0],
			layout,
			cursor,
			viewport,
			renderer,
		)
	}

	fn overlay<'a>(
		&'a mut self,
		tree: &'a mut Tree,
		layout: Layout<'a>,
		renderer: &Renderer,
		viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
		let state = tree.state.downcast_mut::<State>();

		let [first, second] = &mut *tree.children else {
			unreachable!();
		};

		let children = [
			self.content
				.as_widget_mut()
				.overlay(first, layout, renderer, viewport, translation),
			state.position.map(|position| {
				overlay::Element::new(Box::new(Overlay {
					context_menu: &mut self.context_menu,
					tree: second,
					state,
					position,
				}))
			}),
		]
		.into_iter()
		.flatten()
		.collect::<Vec<_>>();

		(!children.is_empty()).then(|| overlay::Group::with_children(children).overlay())
	}
}

impl<'a, Message: 'a> From<ContextMenu<'a, Message>> for Element<'a, Message, Theme, Renderer> {
	fn from(value: ContextMenu<'a, Message>) -> Self {
		Self::new(value)
	}
}

struct Overlay<'a, 'b, Message> {
	context_menu: &'b mut Element<'a, Message, Theme, Renderer>,
	tree: &'b mut Tree,
	state: &'b mut State,
	position: Point,
}

impl<Message> overlay::Overlay<Message, Theme, Renderer> for Overlay<'_, '_, Message> {
	fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
		self.context_menu
			.as_widget_mut()
			.layout(self.tree, renderer, &Limits::new(Size::ZERO, bounds))
			.translate(Vector::new(self.position.x, self.position.y))
	}

	fn draw(
		&self,
		renderer: &mut Renderer,
		theme: &Theme,
		style: &Style,
		layout: Layout<'_>,
		cursor: Cursor,
	) {
		renderer.with_layer(layout.bounds(), |renderer| {
			self.context_menu.as_widget().draw(
				self.tree,
				renderer,
				theme,
				style,
				layout,
				cursor,
				&layout.bounds(),
			);
		});
	}

	fn operate(&mut self, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
		self.context_menu
			.as_widget_mut()
			.operate(self.tree, layout, renderer, operation);
	}

	fn update(
		&mut self,
		event: &Event,
		layout: Layout<'_>,
		cursor: Cursor,
		renderer: &Renderer,
		shell: &mut Shell<'_, Message>,
	) {
		self.context_menu.as_widget_mut().update(
			self.tree,
			event,
			layout,
			cursor,
			renderer,
			shell,
			&layout.bounds(),
		);

		if shell.is_event_captured() {
			return;
		}

		match event {
			Event::Mouse(mouse::Event::ButtonPressed { .. }) => {
				if cursor.is_over(layout.bounds()) {
					shell.capture_event();
				} else {
					self.state.position = None;
					shell.request_redraw();
				}
			}
			Event::Keyboard(keyboard::Event::KeyPressed {
				key: keyboard::Key::Named(keyboard::key::Named::Escape),
				..
			}) => {
				self.state.position = None;
				shell.request_redraw();
			}
			_ => {}
		}
	}

	fn mouse_interaction(
		&self,
		layout: Layout<'_>,
		cursor: Cursor,
		renderer: &Renderer,
	) -> Interaction {
		self.context_menu.as_widget().mouse_interaction(
			self.tree,
			layout,
			cursor,
			&layout.bounds(),
			renderer,
		)
	}

	fn overlay<'a>(
		&'a mut self,
		layout: Layout<'a>,
		renderer: &Renderer,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
		self.context_menu.as_widget_mut().overlay(
			self.tree,
			layout,
			renderer,
			&layout.bounds(),
			Vector::new(self.position.x, self.position.y),
		)
	}
}
