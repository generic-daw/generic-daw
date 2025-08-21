use iced::{
	Animation, Color, Element, Event, Length, Rectangle, Renderer, Size, Theme,
	advanced::{
		Clipboard, Layout, Renderer as _, Shell, Widget,
		layout::{Limits, Node},
		renderer::{Quad, Style},
		widget::{Tree, tree},
	},
	border,
	mouse::Cursor,
	window,
};
use std::time::Instant;

struct State {
	animation: Animation<bool>,
	now: Instant,
}

impl State {
	fn new(enabled: bool) -> Self {
		Self {
			animation: Animation::new(enabled),
			now: Instant::now(),
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct AnimatedDot {
	enabled: bool,
	radius: f32,
}

impl<Message> Widget<Message, Theme, Renderer> for AnimatedDot {
	fn size(&self) -> Size<Length> {
		Size::new(
			Length::Fixed(2.0 * self.radius),
			Length::Fixed(2.0 * self.radius),
		)
	}

	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State::new(self.enabled))
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
		Node::new(Size::new(2.0 * self.radius, 2.0 * self.radius))
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		_layout: Layout<'_>,
		_cursor: Cursor,
		_renderer: &Renderer,
		_clipboard: &mut dyn Clipboard,
		shell: &mut Shell<'_, Message>,
		_viewport: &Rectangle,
	) {
		if let &Event::Window(window::Event::RedrawRequested(now)) = event {
			let state = tree.state.downcast_mut::<State>();
			state.now = now;

			if self.enabled != state.animation.value() {
				state.animation.go_mut(self.enabled, now);
			}

			if state.animation.is_animating(now) {
				shell.request_redraw();
			}
		}
	}

	fn draw(
		&self,
		tree: &Tree,
		renderer: &mut Renderer,
		_theme: &Theme,
		style: &Style,
		layout: Layout<'_>,
		_cursor: Cursor,
		_viewport: &Rectangle,
	) {
		let mut bounds = layout.bounds();
		let border = border::rounded(f32::INFINITY).color(style.text_color);

		let outline = Quad {
			bounds,
			border: border.width(2),
			..Quad::default()
		};

		renderer.fill_quad(outline, Color::TRANSPARENT);

		let state = tree.state.downcast_ref::<State>();

		let factor = state.animation.interpolate(0.0, 1.0, state.now);
		if factor == 0.0 {
			return;
		}

		let offset = self.radius * (1.0 - factor);

		bounds.x += offset;
		bounds.y += offset;
		bounds.width *= factor;
		bounds.height *= factor;

		let inner = Quad {
			bounds,
			border,
			..Quad::default()
		};

		renderer.fill_quad(inner, style.text_color);
	}
}

impl AnimatedDot {
	pub fn new(enabled: bool) -> Self {
		Self {
			enabled,
			radius: 8.0,
		}
	}

	pub fn radius(mut self, radius: f32) -> Self {
		self.radius = radius;
		self
	}
}

impl<Message> From<AnimatedDot> for Element<'_, Message> {
	fn from(value: AnimatedDot) -> Self {
		Element::new(value)
	}
}
