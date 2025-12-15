use iced_widget::{
	Renderer,
	core::{
		Animation, Clipboard, Color, Element, Event, Layout, Length, Rectangle, Renderer as _,
		Shell, Size, Theme, Vector, Widget,
		animation::Easing,
		layout::{Limits, Node},
		mouse::Cursor,
		renderer::{Quad, Style},
		time::{Duration, Instant},
		widget::Tree,
		window,
	},
	theme::palette::mix,
};
use std::{cell::Cell, convert::identity};

const CBRT_HALF: f32 = 0.793_700_5;
const CBRT_QUARTER: f32 = 0.629_960_54;

#[derive(Debug)]
pub struct State {
	line: Animation<f32>,
	bar: Animation<f32>,
	last_update: Instant,
	now: Cell<Instant>,
	delay: Instant,
}

impl Default for State {
	fn default() -> Self {
		let now = Instant::now();
		Self {
			line: Animation::new(0.0),
			bar: Animation::new(0.0),
			last_update: now,
			now: Cell::new(now),
			delay: now,
		}
	}
}

impl State {
	pub fn update(&mut self, peak: f32, now: Instant) {
		let peak = (peak / 2.0).cbrt();

		let min_duration = now - self.last_update;
		self.last_update = now;

		let old_bar = self.bar.interpolate_with(identity, now);
		self.bar = if peak >= old_bar {
			Animation::new(peak)
		} else {
			Animation::new(old_bar)
				.easing(Easing::Linear)
				.duration(Duration::from_secs_f32(1.5 * (old_bar - peak)).max(min_duration))
				.go(peak, now)
		};

		let old_line = self.line.interpolate_with(identity, now);
		self.line = if peak >= old_line {
			self.delay = now + Duration::from_secs(1);
			Animation::new(peak)
		} else {
			Animation::new(old_line)
				.easing(Easing::Linear)
				.duration(Duration::from_secs_f32(3.0 * (old_line - peak)).max(min_duration))
				.delay(self.delay.saturating_duration_since(now))
				.go(peak, now)
		};
	}
}

#[derive(Debug)]
pub struct PeakMeter<'a> {
	state: &'a State,
	width: f32,
}

impl<'a> PeakMeter<'a> {
	#[must_use]
	pub fn new(state: &'a State) -> Self {
		Self { state, width: 13.0 }
	}

	#[must_use]
	pub fn width(mut self, width: f32) -> Self {
		self.width = width;
		self
	}
}

impl<Message> Widget<Message, Theme, Renderer> for PeakMeter<'_> {
	fn size(&self) -> Size<Length> {
		Size::new(Length::Fixed(self.width), Length::Fill)
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
		Node::new(limits.width(self.width).max())
	}

	fn update(
		&mut self,
		_tree: &mut Tree,
		event: &Event,
		_layout: Layout<'_>,
		_cursor: Cursor,
		_renderer: &Renderer,
		_clipboard: &mut dyn Clipboard,
		shell: &mut Shell<'_, Message>,
		_viewport: &Rectangle,
	) {
		if let &Event::Window(window::Event::RedrawRequested(now)) = event {
			self.state.now.set(now);

			if self.state.bar.is_animating(now) || self.state.line.is_animating(now) {
				shell.request_redraw();
			}
		}
	}

	fn draw(
		&self,
		_tree: &Tree,
		renderer: &mut Renderer,
		theme: &Theme,
		_style: &Style,
		layout: Layout<'_>,
		_cursor: Cursor,
		viewport: &Rectangle,
	) {
		let bounds = layout.bounds();

		if !bounds.intersects(viewport) {
			return;
		}

		let success = theme.palette().success;
		let warning = theme.palette().warning;
		let danger = theme.palette().danger;
		let background = theme.palette().background;

		let muted = |color: Color| mix(color, background, 2.0 / 3.0);

		let bar = self
			.state
			.bar
			.interpolate_with(identity, self.state.now.get());

		if bar < 1.0 {
			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(
						bounds.position(),
						Size::new(bounds.width, bounds.height * (1.0 - CBRT_HALF)),
					),
					..Quad::default()
				},
				muted(danger),
			);
		}

		if bar < CBRT_HALF {
			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(
						bounds.position() + Vector::new(0.0, bounds.height * (1.0 - CBRT_HALF)),
						Size::new(bounds.width, bounds.height * (CBRT_HALF - CBRT_QUARTER)),
					),
					..Quad::default()
				},
				muted(warning),
			);
		}

		if bar < CBRT_QUARTER {
			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(
						bounds.position() + Vector::new(0.0, bounds.height * (1.0 - CBRT_QUARTER)),
						Size::new(bounds.width, bounds.height * CBRT_QUARTER),
					),
					..Quad::default()
				},
				muted(success),
			);
		}

		if bar > 0.0 {
			let bar_pos = bounds.height * bar.min(1.0);
			let bar_quad = Quad {
				bounds: Rectangle::new(
					bounds.position() + Vector::new(0.0, bounds.height - bar_pos),
					Size::new(bounds.width, bar_pos),
				),
				..Quad::default()
			};
			renderer.fill_quad(
				bar_quad,
				if bar > CBRT_HALF {
					danger
				} else if bar > CBRT_QUARTER {
					warning
				} else {
					success
				},
			);
		}

		let line = self
			.state
			.line
			.interpolate_with(identity, self.state.now.get());

		if line > 0.0 {
			let line_pos = bounds.height * line.min(1.0);
			let line_quad = Quad {
				bounds: Rectangle::new(
					bounds.position() + Vector::new(0.0, bounds.height - line_pos),
					Size::new(bounds.width, line_pos.min(2.0)),
				),
				..Quad::default()
			};
			renderer.fill_quad(
				line_quad,
				if line > CBRT_HALF {
					danger
				} else if line > CBRT_QUARTER {
					warning
				} else {
					success
				},
			);
		}
	}
}

impl<'a, Message> From<PeakMeter<'a>> for Element<'a, Message, Theme, Renderer> {
	fn from(value: PeakMeter<'a>) -> Self {
		Self::new(value)
	}
}
