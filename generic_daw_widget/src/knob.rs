use iced_widget::{
	Renderer,
	canvas::{Cache, Frame, Path, path::Arc},
	core::{
		Clipboard, Element, Event, Layout, Length, Point, Radians, Rectangle, Renderer as _, Shell,
		Size, Theme, Vector, Widget, border,
		layout::{Limits, Node},
		mouse::{self, Cursor, Interaction, ScrollDelta},
		overlay,
		renderer::{Quad, Style},
		text,
		theme::palette::mix,
		widget::{Text, Tree, tree},
	},
	graphics::geometry::Renderer as _,
};
use std::{cell::RefCell, fmt::Debug, ops::RangeInclusive};
use utils::NoDebug;

#[derive(Default)]
struct State {
	dragging: Option<(f32, f32)>,
	hovering: bool,
	scroll: f32,
	cache: Cache,
	last_value: f32,
	last_enabled: bool,
	last_theme: RefCell<Option<Theme>>,
}

#[derive(Debug)]
pub struct Knob<'a, Message> {
	range: RangeInclusive<f32>,
	value: f32,
	center: f32,
	default: f32,
	enabled: bool,
	f: NoDebug<Box<dyn Fn(f32) -> Message + 'a>>,
	radius: f32,
	stepped: bool,
	tooltip: Option<NoDebug<Text<'a, Theme, Renderer>>>,
}

impl<'a, Message> Knob<'a, Message> {
	#[must_use]
	pub fn new(range: RangeInclusive<f32>, value: f32, f: impl Fn(f32) -> Message + 'a) -> Self {
		Self {
			value: value.clamp(*range.start(), *range.end()),
			center: *range.start(),
			default: *range.end(),
			range,
			enabled: true,
			f: NoDebug(Box::from(f)),
			radius: 20.0,
			stepped: false,
			tooltip: None,
		}
	}

	#[must_use]
	pub fn center(mut self, center: f32) -> Self {
		self.center = center;
		self
	}

	#[must_use]
	pub fn default(mut self, default: f32) -> Self {
		self.default = default;
		self
	}

	#[must_use]
	pub fn radius(mut self, radius: f32) -> Self {
		self.radius = radius;
		self
	}

	#[must_use]
	pub fn enabled(mut self, enabled: bool) -> Self {
		self.enabled = enabled;
		self
	}

	#[must_use]
	pub fn stepped(mut self, stepped: bool) -> Self {
		self.stepped = stepped;
		self
	}

	#[must_use]
	pub fn tooltip(mut self, tooltip: impl text::IntoFragment<'a>) -> Self {
		self.tooltip = Some(Text::new(tooltip).line_height(1.0).into());
		self
	}

	#[must_use]
	pub fn maybe_tooltip(self, tooltip: Option<impl text::IntoFragment<'a>>) -> Self {
		if let Some(tooltip) = tooltip {
			self.tooltip(tooltip)
		} else {
			self
		}
	}

	fn border_radius(&self) -> f32 {
		(self.radius * 0.1).min(3.0)
	}

	fn fill_canvas(&self, state: &State, frame: &mut Frame, theme: &Theme) {
		let border_radius = self.border_radius();
		let dot_radius = border_radius * 1.5;

		let center = frame.center() + Vector::new(0.0, border_radius);

		let circle = |angle: Radians, offset: f32, radius: f32| {
			Path::circle(
				center + Vector::new(angle.0.cos(), angle.0.sin()) * offset,
				radius,
			)
		};

		let angle_of = |value: f32| {
			Radians(f32::to_radians(
				270.0 * (value - self.range.start()) / (self.range.end() - self.range.start())
					- 135.0 - 90.0,
			))
		};

		let center_angle = angle_of(self.center);
		let value_angle = angle_of(self.value);

		let arc = Path::new(|b| {
			b.arc(Arc {
				center,
				radius: self.radius,
				start_angle: center_angle,
				end_angle: value_angle,
			});
			b.line_to(center);
			b.close();
		});

		let color = if self.enabled {
			if state.hovering || state.dragging.is_some() {
				theme.extended_palette().primary.base.color
			} else {
				theme.extended_palette().primary.weak.color
			}
		} else if state.hovering || state.dragging.is_some() {
			theme.extended_palette().secondary.base.color
		} else {
			theme.extended_palette().secondary.weak.color
		};

		let text = theme.extended_palette().background.strong.text;

		frame.fill(&arc, text);

		frame.fill(
			&Path::circle(center, self.radius - border_radius - border_radius),
			color,
		);

		frame.fill(
			&circle(center_angle, self.radius - border_radius, border_radius),
			text,
		);

		frame.fill(
			&circle(value_angle, self.radius - border_radius, border_radius),
			text,
		);

		if self.stepped {
			let mixed_color = mix(color, text, 0.25);

			for step in *self.range.start() as i32..=*self.range.end() as i32 {
				frame.fill(
					&circle(
						angle_of(step as f32),
						self.radius.midpoint(dot_radius),
						dot_radius / 2.0,
					),
					mixed_color,
				);
			}
		}

		frame.fill(&circle(value_angle, self.radius / 2.0, dot_radius), text);
	}
}

impl<Message> Widget<Message, Theme, Renderer> for Knob<'_, Message> {
	fn size(&self) -> Size<Length> {
		Size::new(
			Length::Fixed(2.0 * self.radius),
			Length::Fixed(2.0 * (self.radius - self.border_radius())),
		)
	}

	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn diff(&self, tree: &mut Tree) {
		let state = tree.state.downcast_mut::<State>();

		if self.enabled != state.last_enabled {
			state.last_enabled = self.enabled;
			state.cache.clear();
		}

		if self.value != state.last_value {
			state.last_value = self.value;
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
		tree::State::new(State::default())
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
		Node::new(Size::new(
			2.0 * self.radius,
			2.0 * (self.radius - self.border_radius()),
		))
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		cursor: Cursor,
		_renderer: &Renderer,
		_clipboard: &mut dyn Clipboard,
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
					state.dragging = Some((self.value, pos.y));

					if modifiers.control() || modifiers.command() {
						shell.publish((self.f)(self.default));
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
					if let Some((value, pos)) = state.dragging {
						let diff = (pos - y) * (self.range.end() - self.range.start()) * 0.005;
						let mut new_value =
							(value + diff).clamp(*self.range.start(), *self.range.end());
						if self.stepped {
							new_value = new_value.round();
						}
						if new_value != self.value {
							shell.publish((self.f)(new_value));
						}
						shell.capture_event();
					}

					if (cursor.is_over(layout.bounds())
						&& cursor.position().unwrap().distance(
							layout.bounds().center() + Vector::new(0.0, self.border_radius()),
						) <= self.radius) != state.hovering
					{
						state.hovering ^= true;
						state.cache.clear();
						shell.request_redraw();
					}
				}
				mouse::Event::WheelScrolled { delta, .. }
					if state.dragging.is_none() && state.hovering =>
				{
					if !self.stepped {
						state.scroll = 0.0;
					}

					let mut diff = match delta {
						ScrollDelta::Lines { y, .. } => *y,
						ScrollDelta::Pixels { y, .. } => y / 60.0,
					} + state.scroll;

					if self.stepped {
						state.scroll = diff.fract();
						diff = diff.floor();
					}

					let new_value =
						(self.value + diff).clamp(*self.range.start(), *self.range.end());
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

impl<'a, Message> From<Knob<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
	Message: 'a,
{
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

		let layout = Widget::<Message, Theme, Renderer>::layout(
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
					.color(theme.extended_palette().background.strong.color),
				..Quad::default()
			},
			theme.extended_palette().background.weak.color,
		);

		Widget::<Message, Theme, Renderer>::draw(
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
