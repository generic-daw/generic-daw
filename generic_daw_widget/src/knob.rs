use iced_widget::{
	Renderer,
	canvas::{Cache, Frame, Path, path::Arc},
	core::{
		Element, Event, Layout, Length, Point, Radians, Rectangle, Renderer as _, Shell, Size,
		Theme, Vector, Widget, border,
		layout::{Limits, Node},
		mouse::{self, Cursor, Interaction, ScrollDelta},
		overlay,
		renderer::{Quad, Style},
		widget::{Text, Tree, tree},
	},
	graphics::geometry::Renderer as _,
	text,
};
use std::{cell::RefCell, f32::consts::PI, fmt::Debug, ops::RangeInclusive};
use utils::NoDebug;

struct State {
	dragging: Option<f32>,
	hovering: bool,
	scroll: f32,
	cache: Cache,
	last_info: KnobInfo,
	last_theme: RefCell<Option<Theme>>,
}

#[derive(Clone, Debug, PartialEq)]
struct KnobInfo {
	range: RangeInclusive<f32>,
	value: f32,
	center: f32,
	default: f32,
	enabled: bool,
	radius: f32,
	stepped: bool,
}

#[derive(Debug)]
pub struct Knob<'a, Message> {
	info: KnobInfo,
	f: NoDebug<Box<dyn Fn(f32) -> Message + 'a>>,
	tooltip: Option<NoDebug<Text<'a, Theme, Renderer>>>,
}

impl<'a, Message> Knob<'a, Message> {
	#[must_use]
	pub fn new(range: RangeInclusive<f32>, value: f32, f: impl Fn(f32) -> Message + 'a) -> Self {
		Self {
			info: KnobInfo {
				range: range.clone(),
				value,
				center: *range.start(),
				default: *range.end(),
				enabled: true,
				radius: 20.0,
				stepped: false,
			},
			f: NoDebug(Box::from(f)),
			tooltip: None,
		}
	}

	#[must_use]
	pub fn center(mut self, center: f32) -> Self {
		self.info.center = center;
		self
	}

	#[must_use]
	pub fn default(mut self, default: f32) -> Self {
		self.info.default = default;
		self
	}

	#[must_use]
	pub fn radius(mut self, radius: f32) -> Self {
		self.info.radius = radius;
		self
	}

	#[must_use]
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.info.enabled = enabled;
		self
	}

	#[must_use]
	pub fn stepped(mut self, stepped: bool) -> Self {
		self.info.stepped = stepped;
		self
	}

	#[must_use]
	pub fn tooltip(self, tooltip: impl text::IntoFragment<'a>) -> Self {
		self.maybe_tooltip(Some(tooltip))
	}

	#[must_use]
	pub fn maybe_tooltip(mut self, tooltip: Option<impl text::IntoFragment<'a>>) -> Self {
		self.tooltip = tooltip.map(|tooltip| text(tooltip).line_height(1.0).into());
		self
	}

	fn border_radius(&self) -> f32 {
		(self.info.radius * 0.1).min(3.0)
	}

	fn fill_canvas(&self, state: &State, frame: &mut Frame, theme: &Theme) {
		let swatch = if self.info.enabled {
			theme.palette().primary
		} else {
			theme.palette().secondary
		};

		let color = if state.dragging.is_some() {
			swatch.strong.color
		} else if state.hovering {
			swatch.weak.color
		} else {
			swatch.base.color
		};

		let text = theme.palette().background.strong.text;

		let border_radius = self.border_radius();
		let dot_radius = border_radius * 1.5;
		let center = frame.center() + Vector::new(0.0, border_radius);

		let value_to_rad = |value: f32| {
			Radians(
				if self.info.range.start() == self.info.range.end() {
					0.0
				} else {
					(value - self.info.range.start())
						/ (self.info.range.end() - self.info.range.start())
						* (3.0 / 2.0 * PI)
				} - (5.0 / 4.0 * PI),
			)
		};

		let center_angle = value_to_rad(self.info.center);
		let value_angle = value_to_rad(self.info.value);

		let dot = |angle: Radians, offset: f32, radius: f32| {
			Path::circle(
				center + Vector::new(angle.0.cos(), angle.0.sin()) * offset,
				radius,
			)
		};

		frame.fill(
			&Path::new(|b| {
				b.arc(Arc {
					center,
					radius: self.info.radius,
					start_angle: center_angle,
					end_angle: value_angle,
				});
				b.line_to(center);
				b.close();
			}),
			text,
		);

		frame.fill(
			&Path::circle(center, self.info.radius - border_radius - border_radius),
			color,
		);

		frame.fill(
			&dot(
				center_angle,
				self.info.radius - border_radius,
				border_radius,
			),
			text,
		);

		frame.fill(
			&dot(value_angle, self.info.radius - border_radius, border_radius),
			text,
		);

		if self.info.stepped {
			let num_steps = *self.info.range.end() - *self.info.range.start() + 1.0;
			let max_steps =
				((3.0 / 2.0 * PI) / (dot_radius / self.info.radius).asin()).floor() + 1.0;

			if num_steps <= max_steps {
				for step in *self.info.range.start() as i32..=*self.info.range.end() as i32 {
					frame.fill(
						&dot(
							value_to_rad(step as f32),
							self.info.radius.midpoint(dot_radius),
							dot_radius / 2.0,
						),
						swatch.base.color.mix(swatch.base.text, 0.5),
					);
				}
			}
		}

		frame.fill(&dot(value_angle, self.info.radius / 2.0, dot_radius), text);
	}
}

impl<Message> Widget<Message, Theme, Renderer> for Knob<'_, Message> {
	fn size(&self) -> Size<Length> {
		Size::new(
			Length::Fixed(2.0 * self.info.radius),
			Length::Fixed(2.0 * (self.info.radius - self.border_radius())),
		)
	}

	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn diff(&self, tree: &mut Tree) {
		let state = tree.state.downcast_mut::<State>();

		if self.info != state.last_info {
			state.last_info = self.info.clone();
			state.cache.clear();
		}

		if let Some(tooltip) = self.tooltip.as_deref() {
			tree.diff_children(&[tooltip as &dyn Widget<Message, Theme, Renderer>]);
		} else {
			tree.children.clear();
		}
	}

	fn children(&self) -> Vec<Tree> {
		self.tooltip
			.as_deref()
			.map(|p| vec![Tree::new(p as &dyn Widget<Message, Theme, Renderer>)])
			.unwrap_or_default()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State {
			dragging: None,
			hovering: false,
			scroll: 0.0,
			cache: Cache::new(),
			last_info: self.info.clone(),
			last_theme: RefCell::default(),
		})
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
		Node::new(Size::new(
			2.0 * self.info.radius,
			2.0 * (self.info.radius - self.border_radius()),
		))
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		cursor: Cursor,
		_renderer: &Renderer,
		shell: &mut Shell<'_, Message>,
		_viewport: &Rectangle,
	) {
		if shell.is_event_captured() {
			return;
		}

		if let Event::Mouse(event) = event {
			let state = tree.state.downcast_mut::<State>();

			match event {
				mouse::Event::ButtonPressed {
					button: mouse::Button::Left,
					modifiers,
					..
				} if state.dragging.is_none() && state.hovering => {
					let pos = cursor.position().unwrap();
					state.dragging = Some(pos.y);

					if modifiers.control() || modifiers.command() {
						shell.publish((self.f)(self.info.default));
					}

					shell.capture_event();
				}
				mouse::Event::ButtonReleased {
					button: mouse::Button::Left,
					..
				} if state.dragging.is_some() => {
					if !state.hovering {
						state.cache.clear();
						shell.request_redraw();
					}

					state.dragging = None;
					shell.capture_event();
				}
				mouse::Event::CursorMoved {
					position: Point { y, .. },
					..
				} => {
					if let Some(last_y) = state.dragging {
						let diff = (last_y - y)
							* (self.info.range.end() - self.info.range.start())
							* 0.005;

						let mut new_value = (self.info.value + diff)
							.clamp(*self.info.range.start(), *self.info.range.end());
						if self.info.stepped {
							new_value = new_value.round();
						}
						if new_value != self.info.value
							|| (diff < 0.0 && new_value == *self.info.range.start())
							|| (diff > 0.0 && new_value == *self.info.range.end())
						{
							shell.publish((self.f)(new_value));
							state.dragging = Some(*y);
						}

						shell.capture_event();
					}

					if (cursor.is_over(layout.bounds())
						&& cursor.position().unwrap().distance(
							layout.bounds().center() + Vector::new(0.0, self.border_radius()),
						) <= self.info.radius)
						!= state.hovering
					{
						state.hovering ^= true;
						state.cache.clear();
						shell.request_redraw();
					}
				}
				mouse::Event::WheelScrolled { delta, modifiers }
					if state.dragging.is_none() && state.hovering =>
				{
					let mut diff = match delta {
						ScrollDelta::Lines { y, .. } => *y,
						ScrollDelta::Pixels { y, .. } => y / 60.0,
					} * (self.info.range.end() - self.info.range.start())
						* if modifiers.command() { 10.0 } else { 1.0 }
						/ 101.0 + state.scroll;

					if self.info.stepped {
						state.scroll = diff.fract();
						diff = diff.floor();
					}

					let new_value = (self.info.value + diff)
						.clamp(*self.info.range.start(), *self.info.range.end());
					if new_value != self.info.value {
						shell.publish((self.f)(new_value));
						shell.capture_event();
					} else if diff != 0.0 {
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

		if state.last_theme.borrow().as_ref() != Some(theme) {
			*state.last_theme.borrow_mut() = Some(theme.clone());
			state.cache.clear();
		}

		renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
			renderer.draw_geometry(state.cache.draw(renderer, bounds.size(), |frame| {
				self.fill_canvas(state, frame, theme);
			}));
		});
	}

	fn mouse_interaction(
		&self,
		tree: &Tree,
		_layout: Layout<'_>,
		_cursor: Cursor,
		_viewport: &Rectangle,
		_renderer: &Renderer,
	) -> Interaction {
		let state = tree.state.downcast_ref::<State>();

		if state.dragging.is_some() {
			Interaction::Grabbing
		} else if state.hovering {
			Interaction::Grab
		} else {
			Interaction::default()
		}
	}

	fn overlay<'a>(
		&'a mut self,
		tree: &'a mut Tree,
		layout: Layout<'a>,
		_renderer: &Renderer,
		viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
		let state = tree.state.downcast_ref::<State>();

		if state.hovering || state.dragging.is_some() {
			self.tooltip.as_deref_mut().map(|tooltip| {
				overlay::Element::new(Box::new(Overlay {
					tooltip,
					tree: tree.children.iter_mut().next().unwrap(),
					bounds: layout.bounds() + translation,
					viewport: *viewport,
				}))
			})
		} else {
			None
		}
	}
}

impl<'a, Message: 'a> From<Knob<'a, Message>> for Element<'a, Message, Theme, Renderer> {
	fn from(value: Knob<'a, Message>) -> Self {
		Self::new(value)
	}
}

struct Overlay<'a, 'b> {
	tooltip: &'b mut Text<'a, Theme, Renderer>,
	tree: &'b mut Tree,
	bounds: Rectangle,
	viewport: Rectangle,
}

impl<Message> overlay::Overlay<Message, Theme, Renderer> for Overlay<'_, '_> {
	fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
		let padding = 3.0;

		let layout = Widget::<Message, _, _>::layout(
			self.tooltip,
			self.tree,
			renderer,
			&Limits::new(Size::ZERO, bounds).shrink(Size::new(padding, padding)),
		);
		let bounds = layout.bounds();

		let mut layout = Node::with_children(
			bounds.expand(padding).size(),
			vec![layout.translate(Vector::new(padding, padding))],
		)
		.move_to(self.bounds.position())
		.translate(Vector::new(
			(self.bounds.width - bounds.width) / 2.0 - padding,
			self.bounds.height,
		));

		let clamp = self.viewport.x - layout.bounds().x;
		if clamp + padding > 0.0 {
			layout.translate_mut(Vector::new(clamp + padding, 0.0));
		} else {
			let clamp = clamp + self.viewport.width - layout.bounds().width;
			if clamp - padding < 0.0 {
				layout.translate_mut(Vector::new(clamp - padding, 0.0));
			}
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
		renderer.fill_quad(
			Quad {
				bounds: layout.bounds(),
				border: border::width(1)
					.rounded(2)
					.color(theme.palette().background.strong.color),
				..Quad::default()
			},
			theme.palette().background.weak.color,
		);

		Widget::<Message, _, _>::draw(
			self.tooltip,
			self.tree,
			renderer,
			theme,
			style,
			layout.child(0),
			cursor,
			&Rectangle::INFINITE,
		);
	}
}
