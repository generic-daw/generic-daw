use iced::{
	Animation, Color, Element, Event, Fill, Length, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Clipboard, Layout, Renderer as _, Shell, Widget,
		layout::{Limits, Node},
		renderer::{Quad, Style},
		widget::{Tree, tree},
	},
	animation::Easing,
	gradient::Linear,
	mouse::Cursor,
	window,
};
use std::{
	convert::identity,
	time::{Duration, Instant},
};

#[derive(Debug)]
struct State {
	line: Animation<f32>,
	line_mix: Animation<f32>,
	bar: Animation<f32>,
	bar_mix: Animation<f32>,
	now: Instant,
}

impl Default for State {
	fn default() -> Self {
		Self {
			line: Animation::new(0.0),
			line_mix: Animation::new(0.0),
			bar: Animation::new(0.0),
			bar_mix: Animation::new(0.0),
			now: Instant::now(),
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct PeakMeter {
	peak: f32,
	enabled: bool,
	width: f32,
}

impl<Message> Widget<Message, Theme, Renderer> for PeakMeter {
	fn size(&self) -> Size<Length> {
		Size::new(Length::Fixed(self.width), Fill)
	}

	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State::default())
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
		Node::new(Size::new(self.width, limits.max().height))
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		_cursor: Cursor,
		_renderer: &Renderer,
		_clipboard: &mut dyn Clipboard,
		shell: &mut Shell<'_, Message>,
		_viewport: &Rectangle,
	) {
		if let &Event::Window(window::Event::RedrawRequested(now)) = event {
			let state = tree.state.downcast_mut::<State>();
			state.now = now;

			let peak = std::mem::take(&mut self.peak);

			if peak > state.bar.interpolate_with(identity, now) {
				state.bar = Animation::new(peak)
					.duration(Duration::from_secs_f32(
						peak.exp2() * layout.bounds().height.sqrt() / 10.0,
					))
					.easing(Easing::EaseOutExpo)
					.go(0.0, now);
			}

			if peak > state.line.interpolate_with(identity, now) {
				state.line = Animation::new(peak)
					.duration(Duration::from_secs_f32(
						peak * layout.bounds().height.sqrt() / 3.0,
					))
					.delay(Duration::from_secs_f32(peak.exp2()))
					.go(0.0, now);
			}

			if self.enabled {
				if state.bar.interpolate_with(identity, now) > 1.0 {
					state.bar_mix = Animation::new(1.0).very_quick().go(0.0, now);
				}
				if state.line.interpolate_with(identity, now) > 1.0 {
					state.line_mix = Animation::new(1.0).very_quick().go(0.0, now);
				}
			} else {
				state.bar_mix = Animation::new(0.0);
				state.line_mix = Animation::new(0.0);
			}

			if state.bar.is_animating(now)
				|| state.bar_mix.is_animating(now)
				|| state.line.is_animating(now)
				|| state.line_mix.is_animating(now)
			{
				shell.request_redraw();
			}
		}
	}

	fn draw(
		&self,
		tree: &Tree,
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

		let state = tree.state.downcast_ref::<State>();

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

		let bar_color = mix(
			base,
			theme.extended_palette().danger.weak.color,
			state.bar_mix.interpolate_with(identity, state.now),
		);
		let bar_pos = bounds.height * state.bar.interpolate_with(identity, state.now).min(1.0);
		let bar = Quad {
			bounds: Rectangle::new(
				bounds.position() + Vector::new(0.0, bounds.height - bar_pos),
				Size::new(bounds.width, bar_pos),
			),
			..Quad::default()
		};
		renderer.fill_quad(bar, bar_color);

		let line_color = mix(
			base,
			theme.extended_palette().danger.weak.color,
			state.line_mix.interpolate_with(identity, state.now),
		);
		let line_pos = bounds.height * state.line.interpolate_with(identity, state.now).min(1.0);
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

impl PeakMeter {
	pub fn new(peak: f32, enabled: bool) -> Self {
		Self {
			peak,
			enabled,
			width: 14.0,
		}
	}

	pub fn width(mut self, width: f32) -> Self {
		self.width = width;
		self
	}
}

impl<Message> From<PeakMeter> for Element<'_, Message> {
	fn from(value: PeakMeter) -> Self {
		Self::new(value)
	}
}

fn mix(a: Color, b: Color, factor: f32) -> Color {
	let b_amount = factor.clamp(0.0, 1.0);
	let a_amount = 1.0 - b_amount;

	let a_linear = a.into_linear().map(|c| c * a_amount);
	let b_linear = b.into_linear().map(|c| c * b_amount);

	Color::from_linear_rgba(
		a_linear[0] + b_linear[0],
		a_linear[1] + b_linear[1],
		a_linear[2] + b_linear[2],
		a_linear[3] + b_linear[3],
	)
}
