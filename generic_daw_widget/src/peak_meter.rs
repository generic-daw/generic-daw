use crate::mix;
use iced_widget::{
	Renderer,
	core::{
		Animation, Clipboard, Element, Event, Layout, Length, Rectangle, Renderer as _, Shell,
		Size, Theme, Vector, Widget,
		animation::Easing,
		gradient::Linear,
		layout::{Limits, Node},
		mouse::Cursor,
		renderer::{Quad, Style},
		widget::Tree,
		window,
	},
};
use std::{
	cell::Cell,
	convert::identity,
	time::{Duration, Instant},
};

#[derive(Debug)]
pub struct State {
	line: Animation<f32>,
	bar: Animation<f32>,
	now: Cell<Instant>,
	time: f32,
}

impl State {
	#[must_use]
	pub fn new(time: f32) -> Self {
		Self {
			line: Animation::new(0.0),
			bar: Animation::new(0.0),
			now: Cell::new(Instant::now()),
			time,
		}
	}

	pub fn update(&mut self, peak: f32, now: Instant) {
		if peak > self.bar.interpolate_with(identity, now) {
			self.bar = Animation::new(peak)
				.duration(Duration::from_secs_f32(peak.exp2() * self.time))
				.easing(Easing::EaseOutExpo)
				.go(0.0, now);
		}

		if peak > self.line.interpolate_with(identity, now) {
			self.line = Animation::new(peak)
				.duration(Duration::from_secs_f32(peak * self.time * 3.0))
				.delay(Duration::from_secs_f32(peak.exp2()))
				.go(0.0, now);
		}
	}
}

#[derive(Debug)]
pub struct PeakMeter<'a> {
	state: &'a State,
	enabled: bool,
	width: f32,
}

impl<'a> PeakMeter<'a> {
	#[must_use]
	pub fn new(state: &'a State, enabled: bool) -> Self {
		Self {
			state,
			enabled,
			width: 14.0,
		}
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
		Node::new(Size::new(self.width, limits.max().height))
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

		let base = if self.enabled {
			theme.extended_palette().primary.weak.color
		} else {
			theme.extended_palette().secondary.strong.color
		};

		let background_color = mix(base, theme.extended_palette().background.weak.color, 0.5);
		let background = Quad {
			bounds,
			..Quad::default()
		};
		renderer.fill_quad(background, background_color);

		let mix_amt = |val: f32| {
			let amt = 0.05 / self.state.time;
			(val + amt - 1.0).clamp(0.0, amt) * (1.0 / amt)
		};

		let bar = self
			.state
			.bar
			.interpolate_with(identity, self.state.now.get());
		let bar_color = mix(
			base,
			theme.extended_palette().danger.weak.color,
			mix_amt(bar),
		);
		let bar_pos = bounds.height * bar.min(1.0);
		let bar = Quad {
			bounds: Rectangle::new(
				bounds.position() + Vector::new(0.0, bounds.height - bar_pos),
				Size::new(bounds.width, bar_pos),
			),
			..Quad::default()
		};
		renderer.fill_quad(bar, bar_color);

		let line = self
			.state
			.line
			.interpolate_with(identity, self.state.now.get());
		let line_color = mix(
			base,
			theme.extended_palette().danger.weak.color,
			mix_amt(line),
		);
		let line_pos = bounds.height * line.min(1.0);
		let max_line_height = bounds.height.sqrt();
		let line_height = max_line_height.min(line_pos);
		let line = Quad {
			bounds: Rectangle::new(
				bounds.position() + Vector::new(0.0, bounds.height - line_pos),
				Size::new(bounds.width, line_height),
			),
			..Quad::default()
		};
		renderer.fill_quad(
			line,
			Linear::new(0.0)
				.add_stop(
					0.0,
					line_color.scale_alpha(1.0 - (line_height / max_line_height)),
				)
				.add_stop(1.0, line_color),
		);
	}
}

impl<'a, Message> From<PeakMeter<'a>> for Element<'a, Message, Theme, Renderer> {
	fn from(value: PeakMeter<'a>) -> Self {
		Self::new(value)
	}
}
