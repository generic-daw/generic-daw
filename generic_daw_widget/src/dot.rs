use iced_widget::{
	Renderer,
	core::{
		Animation, Clipboard, Color, Element, Event, Layout, Length, Rectangle, Renderer as _,
		Shell, Size, Theme, Widget, border,
		layout::{Limits, Node},
		mouse::Cursor,
		renderer::{Quad, Style},
		time::Instant,
		widget::{Tree, tree},
		window,
	},
};

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
pub struct Dot {
	enabled: bool,
	radius: f32,
}

impl Dot {
	#[must_use]
	pub fn new(enabled: bool) -> Self {
		Self {
			enabled,
			radius: 8.0,
		}
	}

	#[must_use]
	pub fn radius(mut self, radius: f32) -> Self {
		self.radius = radius;
		self
	}
}

impl<Message> Widget<Message, Theme, Renderer> for Dot {
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
		renderer.fill_quad(
			Quad {
				bounds: layout.bounds(),
				border: border::rounded(f32::INFINITY)
					.color(style.text_color)
					.width(2),
				..Quad::default()
			},
			Color::TRANSPARENT,
		);

		let state = tree.state.downcast_ref::<State>();
		let animation = state.animation.interpolate(1.0, 0.0, state.now);
		if animation != 1.0 {
			renderer.fill_quad(
				Quad {
					bounds: layout.bounds().shrink(self.radius * animation),
					border: border::rounded(f32::INFINITY),
					..Quad::default()
				},
				style.text_color,
			);
		}
	}
}

impl<Message> From<Dot> for Element<'_, Message, Theme, Renderer> {
	fn from(value: Dot) -> Self {
		Element::new(value)
	}
}
