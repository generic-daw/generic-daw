use iced_widget::{
	Renderer,
	core::{
		Element, Event, Layout, Point, Renderer as _, Shell, Size, Theme, Vector, keyboard,
		layout::{Limits, Node},
		mouse,
		mouse::{Cursor, Interaction},
		overlay,
		renderer::Style,
		widget::{Operation, tree::Tree},
	},
};

pub struct MenuOverlay<'a, 'b, Message> {
	pub content: &'b mut Element<'a, Message, Theme, Renderer>,
	pub tree: &'b mut Tree,
	pub state: &'b mut Option<Point>,
	pub position: Point,
}

impl<Message> overlay::Overlay<Message, Theme, Renderer> for MenuOverlay<'_, '_, Message> {
	fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
		let mut layout = self
			.content
			.as_widget_mut()
			.layout(
				self.tree,
				renderer,
				&Limits::new(
					Size::ZERO,
					Size::new(self.position.x, self.position.y)
						.max(bounds - Size::new(self.position.x, self.position.y)),
				),
			)
			.move_to(self.position);

		if bounds.width < layout.bounds().x + layout.bounds().width {
			layout.translate_mut(Vector::new(-layout.bounds().width, 0.0));
		}

		if bounds.height < layout.bounds().y + layout.bounds().height {
			layout.translate_mut(Vector::new(0.0, -layout.bounds().height));
		}

		layout
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
			self.content.as_widget().draw(
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
		self.content
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
		let was_event_captured = shell.is_event_captured();

		self.content.as_widget_mut().update(
			self.tree,
			event,
			layout,
			cursor,
			renderer,
			shell,
			&layout.bounds(),
		);

		if was_event_captured {
			return;
		}

		match event {
			Event::Mouse(mouse::Event::ButtonPressed { .. }) => {
				if cursor.is_over(layout.bounds()) {
					shell.capture_event();
				} else {
					*self.state = None;
					shell.request_redraw();
				}
			}
			Event::Mouse(mouse::Event::ButtonReleased { .. })
				if shell.is_event_captured() && cursor.is_over(layout.bounds()) =>
			{
				*self.state = None;
				shell.request_redraw();
			}
			Event::Mouse(mouse::Event::WheelScrolled { .. }) => {
				shell.capture_event();
			}
			Event::Keyboard(keyboard::Event::KeyPressed {
				key: keyboard::Key::Named(keyboard::key::Named::Escape),
				..
			}) => {
				*self.state = None;
				shell.capture_event();
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
		let interaction = self.content.as_widget().mouse_interaction(
			self.tree,
			layout,
			cursor,
			&layout.bounds(),
			renderer,
		);

		if interaction == Interaction::default() && cursor.is_over(layout.bounds()) {
			Interaction::Idle
		} else {
			interaction
		}
	}

	fn overlay<'a>(
		&'a mut self,
		layout: Layout<'a>,
		renderer: &Renderer,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
		self.content.as_widget_mut().overlay(
			self.tree,
			layout,
			renderer,
			&layout.bounds(),
			Vector::ZERO,
		)
	}
}
