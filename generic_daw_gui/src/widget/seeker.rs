use crate::widget::{ALPHA_1_3, LINE_HEIGHT, maybe_snap, px_to_time, time_to_px};
use generic_daw_core::{MusicalTime, Position, Transport};
use iced::{
	Color, Element, Event, Fill, Font, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Layout, Renderer as _, Shell, Text, Widget,
		layout::{Limits, Node},
		mouse::{self, Cursor, Interaction, ScrollDelta},
		overlay,
		renderer::{Quad, Style},
		text::Renderer as _,
		widget::{Operation, Tree, tree},
	},
	alignment::Vertical,
	border, keyboard, padding,
	widget::text::{Alignment, Ellipsis, LineHeight, Shaping, Wrapping},
	window,
};
use std::time::Instant;
use utils::NoDebug;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Status {
	Seeking(MusicalTime),
	DraggingLoop(MusicalTime),
	Panning(Point),
	#[default]
	None,
}

#[derive(Default)]
struct State {
	status: Status,
	last_height: f32,
	autoscroll_start: Option<Instant>,
	last_autoscroll: Option<Instant>,
}

#[derive(Debug)]
pub struct Seeker<'a, Message> {
	transport: &'a Transport,
	position: Vector,
	scale: Vector,
	offset: f32,
	children: NoDebug<[Element<'a, Message>; 2]>,
	seek_to: fn(MusicalTime) -> Message,
	set_loop_marker: fn(Option<Position>) -> Message,
	pan: fn(Vector, f32, f32) -> Message,
	zoom: fn(Vector, Point, f32, f32) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for Seeker<'_, Message> {
	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State::default())
	}

	fn size(&self) -> Size<Length> {
		Size::new(Fill, Fill)
	}

	fn diff(&self, tree: &mut Tree) {
		tree.diff_children(&*self.children);
	}

	fn children(&self) -> Vec<Tree> {
		self.children.iter().map(Tree::new).collect()
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		let left = self.children[0]
			.as_widget_mut()
			.layout(&mut tree.children[0], renderer, &Limits::NONE)
			.translate(Vector::new(0.0, LINE_HEIGHT - self.position.y));

		let right = self.children[1]
			.as_widget_mut()
			.layout(&mut tree.children[1], renderer, &Limits::NONE)
			.translate(Vector::new(0.0, LINE_HEIGHT - self.position.y))
			.translate(Vector::new(left.size().width, 0.0));

		Node::with_children(limits.max(), vec![left, right])
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		cursor: Cursor,
		renderer: &Renderer,
		shell: &mut Shell<'_, Message>,
		_viewport: &Rectangle,
	) {
		self.children
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.zip(Self::viewports(layout))
			.for_each(|(((child, tree), layout), viewport)| {
				child.as_widget_mut().update(
					tree,
					event,
					layout,
					if cursor.is_over(viewport) {
						cursor
					} else {
						cursor.levitate()
					},
					renderer,
					shell,
					&viewport,
				);
			});

		let state = tree.state.downcast_mut::<State>();
		let right_viewport = Self::right_viewport(layout);
		let height = right_viewport.height;

		let right_child = layout.child(1).bounds();
		let visible = right_child.y + right_child.height - right_viewport.y;

		if let Event::Window(window::Event::RedrawRequested(..)) = event
			&& state.last_height != height
		{
			state.last_height = height;
			shell.publish((self.pan)(Vector::ZERO, height, visible));
			return;
		}

		if shell.is_event_captured() {
			return;
		}

		let cursor = 'block: {
			let viewport = right_viewport.expand(padding::top(LINE_HEIGHT));

			if let Some(cursor) = cursor.position_in(viewport) {
				break 'block cursor;
			}

			if state.status == Status::None {
				return;
			}

			let Some(cursor) = cursor.position_from(viewport.position()) else {
				state.status = Status::None;
				shell.request_redraw();
				return;
			};

			if matches!(state.status, Status::Panning(..)) {
				break 'block cursor;
			}

			let clamped = Point::new(
				cursor.x.clamp(0.0, viewport.width),
				cursor.y.clamp(0.0, viewport.height),
			);

			if cursor == clamped {
				state.autoscroll_start = None;
				state.last_autoscroll = None;
				break 'block clamped;
			}

			shell.request_redraw();

			let &Event::Window(window::Event::RedrawRequested(now)) = event else {
				break 'block clamped;
			};

			if state.last_autoscroll == Some(now) {
			} else if let Some(autoscroll_start) = state.autoscroll_start {
				let autoscroll_amt = (now - autoscroll_start).as_secs_f32().sqrt();

				let delta = Vector::new(
					if cursor.x == clamped.x {
						0.0
					} else {
						20.0 * autoscroll_amt.copysign(cursor.x - clamped.x)
					},
					0.0,
				);

				shell.publish((self.pan)(delta, height, visible));

				state.last_autoscroll = Some(now);
			} else {
				state.autoscroll_start = Some(now);
			}

			clamped
		};

		match event {
			Event::Mouse(mouse::Event::CursorMoved { modifiers, .. })
			| Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
				let time = maybe_snap(
					px_to_time(
						cursor.x + self.offset,
						self.position,
						self.scale,
						self.transport,
					),
					*modifiers,
					|time| time.snap_round(self.scale.x, self.transport),
				);

				match state.status {
					Status::Seeking(last_time) => {
						if last_time != time {
							state.status = Status::Seeking(time);
							shell.publish((self.seek_to)(time));
							shell.capture_event();
						}
					}
					Status::DraggingLoop(last_time) => {
						let loop_marker = (last_time != time).then(|| {
							let start = last_time.min(time);
							let end = last_time.max(time);
							Position::new(start, end)
						});

						if self.transport.loop_marker != loop_marker {
							shell.publish((self.set_loop_marker)(loop_marker));
							shell.capture_event();
						}
					}
					Status::Panning(last_pos) => {
						let delta = last_pos - cursor;

						if delta != Vector::ZERO {
							state.status = Status::Panning(cursor);
							shell.publish((self.pan)(delta, height, visible));
							shell.capture_event();
						}
					}
					Status::None => {}
				}
			}
			Event::Mouse(mouse::Event::ButtonPressed {
				button: mouse::Button::Left,
				modifiers,
			}) if cursor.y < LINE_HEIGHT => {
				let time = maybe_snap(
					px_to_time(
						cursor.x + self.offset,
						self.position,
						self.scale,
						self.transport,
					),
					*modifiers,
					|time| time.snap_round(self.scale.x, self.transport),
				);
				state.status = if modifiers.command() {
					if let Some(loop_marker) = self.transport.loop_marker {
						if time == loop_marker.start() {
							Status::DraggingLoop(loop_marker.end())
						} else if time == loop_marker.end() {
							Status::DraggingLoop(loop_marker.start())
						} else {
							shell.publish((self.set_loop_marker)(None));
							Status::DraggingLoop(time)
						}
					} else {
						shell.publish((self.set_loop_marker)(None));
						Status::DraggingLoop(time)
					}
				} else {
					shell.publish((self.seek_to)(time));
					Status::Seeking(time)
				};
				shell.capture_event();
			}
			Event::Mouse(mouse::Event::ButtonPressed {
				button: mouse::Button::Middle,
				modifiers,
			}) if cursor.y >= LINE_HEIGHT => {
				state.status = Status::Panning(cursor);
			}
			Event::Mouse(mouse::Event::ButtonReleased { .. }) if state.status != Status::None => {
				state.status = Status::None;
				shell.capture_event();
			}
			Event::Mouse(mouse::Event::WheelScrolled { delta, modifiers }) => {
				let (x, mut y) = match *delta {
					ScrollDelta::Pixels { x, y } => (-x, -y),
					ScrollDelta::Lines { x, y } => (-x * 60.0, -y * 60.0),
				};

				match (modifiers.command(), modifiers.shift(), modifiers.alt()) {
					(false, false, false) if x != 0.0 || y != 0.0 => {
						shell.publish((self.pan)(Vector::new(x, y), height, visible));
						shell.capture_event();
					}
					(true, false, false) if y != 0.0 => {
						y /= 128.0;
						shell.publish((self.zoom)(
							Vector::new(y, 0.0),
							cursor - Vector::new(0.0, LINE_HEIGHT),
							height,
							visible,
						));
						shell.capture_event();
					}
					(false, true, false) if x != 0.0 || y != 0.0 => {
						shell.publish((self.pan)(Vector::new(y, x), height, visible));
						shell.capture_event();
					}
					(false, false, true) if y != 0.0 => {
						y /= -8.0;
						shell.publish((self.zoom)(
							Vector::new(0.0, y),
							cursor - Vector::new(0.0, LINE_HEIGHT),
							height,
							visible,
						));
						shell.capture_event();
					}
					_ => {}
				}
			}
			_ => {}
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
		_viewport: &Rectangle,
	) {
		self.grid(renderer, Self::right_viewport(layout), theme);

		renderer.with_layer(
			layout.bounds().shrink(padding::top(LINE_HEIGHT)),
			|renderer| {
				self.children
					.iter()
					.zip(&tree.children)
					.zip(layout.children())
					.zip(Self::viewports(layout))
					.for_each(|(((child, tree), layout), viewport)| {
						child.as_widget().draw(
							tree,
							renderer,
							theme,
							style,
							layout,
							if cursor.is_over(viewport) {
								cursor
							} else {
								cursor.levitate()
							},
							&viewport,
						);
					});
			},
		);

		renderer.with_layer(Rectangle::INFINITE, |renderer| {
			self.header(
				renderer,
				Self::right_viewport(layout).expand(padding::top(LINE_HEIGHT)),
				theme,
			);
		});
	}

	fn mouse_interaction(
		&self,
		tree: &Tree,
		layout: Layout<'_>,
		cursor: Cursor,
		_viewport: &Rectangle,
		renderer: &Renderer,
	) -> Interaction {
		match tree.state.downcast_ref::<State>().status {
			Status::Seeking(..) | Status::DraggingLoop(..) => Interaction::ResizingHorizontally,
			Status::Panning(..) => Interaction::Move,
			Status::None => {
				if cursor
					.position_in(Self::right_viewport(layout).expand(padding::top(LINE_HEIGHT)))
					.is_none_or(|cursor| cursor.y >= LINE_HEIGHT)
				{
					self.children
						.iter()
						.zip(&tree.children)
						.zip(layout.children())
						.zip(Self::viewports(layout))
						.map(|(((child, tree), layout), viewport)| {
							child.as_widget().mouse_interaction(
								tree,
								layout,
								if cursor.is_over(viewport) {
									cursor
								} else {
									cursor.levitate()
								},
								&viewport,
								renderer,
							)
						})
						.max()
						.unwrap_or_default()
				} else {
					Interaction::ResizingHorizontally
				}
			}
		}
	}

	fn overlay<'a>(
		&'a mut self,
		tree: &'a mut Tree,
		layout: Layout<'a>,
		renderer: &Renderer,
		_viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
		let children = self
			.children
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.zip(Self::viewports(layout))
			.filter_map(|(((child, tree), layout), viewport)| {
				child
					.as_widget_mut()
					.overlay(tree, layout, renderer, &viewport, translation)
			})
			.collect::<Vec<_>>();

		(!children.is_empty()).then(|| overlay::Group::with_children(children).overlay())
	}

	fn operate(
		&mut self,
		tree: &mut Tree,
		layout: Layout<'_>,
		renderer: &Renderer,
		operation: &mut dyn Operation,
	) {
		operation.container(None, layout.bounds());
		operation.traverse(&mut |operation| {
			self.children
				.iter_mut()
				.zip(&mut tree.children)
				.zip(layout.children())
				.for_each(|((child, tree), layout)| {
					child
						.as_widget_mut()
						.operate(tree, layout, renderer, operation);
				});
		});
	}
}

impl<'a, Message> Seeker<'a, Message> {
	pub fn new(
		transport: &'a Transport,
		position: Vector,
		scale: Vector,
		left: impl Into<Element<'a, Message>>,
		right: impl Into<Element<'a, Message>>,
		seek_to: fn(MusicalTime) -> Message,
		set_loop_marker: fn(Option<Position>) -> Message,
		pan: fn(Vector, f32, f32) -> Message,
		zoom: fn(Vector, Point, f32, f32) -> Message,
	) -> Self {
		Self {
			transport,
			position,
			scale,
			offset: 0.0,
			children: [left.into(), right.into()].into(),
			seek_to,
			set_loop_marker,
			pan,
			zoom,
		}
	}

	pub fn with_offset(mut self, offset: f32) -> Self {
		self.offset = offset / self.scale.x.exp2();
		self
	}

	fn viewports(layout: Layout<'_>) -> [Rectangle; 2] {
		[Self::left_viewport(layout), Self::right_viewport(layout)]
	}

	fn left_viewport(layout: Layout<'_>) -> Rectangle {
		layout
			.bounds()
			.shrink(padding::right(Self::right_viewport(layout).width).top(LINE_HEIGHT))
	}

	fn right_viewport(layout: Layout<'_>) -> Rectangle {
		layout
			.bounds()
			.shrink(padding::left(layout.child(0).bounds().width).top(LINE_HEIGHT))
	}

	fn grid(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
		let offset_time = |time: MusicalTime| {
			bounds.position()
				+ Vector::new(
					time_to_px(time, self.position, self.scale, self.transport) - self.offset,
					0.0,
				)
		};

		let mut beat = px_to_time(self.offset, self.position, self.scale, self.transport);
		let end_beat = px_to_time(
			self.offset + bounds.width,
			self.position,
			self.scale,
			self.transport,
		);
		beat = beat.snap_floor(self.scale.x + 1.0, self.transport);

		let background_step = MusicalTime::new(8 * u64::from(self.transport.numerator.get()), 0);
		let mut background_beat = beat.round(background_step);
		let background_width =
			background_step.to_samples(self.transport) as f32 / self.scale.x.exp2() / 2.0;

		while background_beat < end_beat {
			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(
						offset_time(background_beat),
						Size::new(background_width, bounds.height),
					)
					.intersection(&bounds)
					.unwrap_or_default(),
					..Quad::default()
				},
				theme.palette().background.weakest.color,
			);
			background_beat += background_step;
		}

		let snap_step = MusicalTime::snap_step(self.scale.x + 1.0, self.transport);

		while beat <= end_beat {
			let color = if snap_step >= MusicalTime::BEAT {
				if beat.beat_in_bar(self.transport) == 0
					&& beat.bar(self.transport).is_multiple_of(snap_step.beat())
				{
					theme.palette().background.strong.color
				} else {
					theme.palette().background.weak.color
				}
			} else if beat.tick() == 0 {
				theme.palette().background.strong.color
			} else {
				theme.palette().background.weak.color
			};

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(offset_time(beat), Size::new(1.0, bounds.height))
						.intersection(&bounds)
						.unwrap_or_default(),
					..Quad::default()
				},
				color,
			);
			beat += snap_step;
		}

		renderer.fill_quad(
			Quad {
				bounds,
				border: border::width(1).color(theme.palette().background.strong.color),
				..Quad::default()
			},
			Color::TRANSPARENT,
		);
	}

	fn header(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
		let offset_time = |time: MusicalTime| {
			bounds.position()
				+ Vector::new(
					time_to_px(time, self.position, self.scale, self.transport) - self.offset,
					0.0,
				)
		};

		renderer.fill_quad(
			Quad {
				bounds: Rectangle::new(bounds.position(), Size::new(bounds.width, LINE_HEIGHT))
					.intersection(&bounds)
					.unwrap_or_default(),
				..Quad::default()
			},
			if self.transport.loop_marker.is_some() {
				theme.palette().secondary.base.color
			} else {
				theme.palette().primary.base.color
			},
		);

		if let Some(loop_marker) = self.transport.loop_marker {
			let start = offset_time(loop_marker.start());
			let end = offset_time(loop_marker.end());

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(start, Size::new(end.x - start.x, LINE_HEIGHT))
						.intersection(&bounds)
						.unwrap_or_default(),
					..Quad::default()
				},
				theme.palette().primary.base.color,
			);

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(
						start - Vector::new(1.5, 0.0),
						Size::new(1.5, bounds.height),
					)
					.intersection(&bounds)
					.unwrap_or_default(),
					..Quad::default()
				},
				theme.palette().secondary.base.color,
			);

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(end, Size::new(1.5, bounds.height))
						.intersection(&bounds)
						.unwrap_or_default(),
					..Quad::default()
				},
				theme.palette().secondary.base.color,
			);

			renderer.fill_quad(
				Quad {
					bounds: bounds
						.shrink(padding::right(0f32.max(bounds.x + bounds.width - start.x))),
					..Quad::default()
				},
				theme.palette().secondary.base.color.scale_alpha(ALPHA_1_3),
			);

			renderer.fill_quad(
				Quad {
					bounds: bounds.shrink(padding::left(0f32.max(end.x - bounds.x))),
					..Quad::default()
				},
				theme.palette().secondary.base.color.scale_alpha(ALPHA_1_3),
			);
		}

		renderer.fill_quad(
			Quad {
				bounds: Rectangle::new(
					offset_time(MusicalTime::from_samples(
						self.transport.sample,
						self.transport,
					)),
					Size::new(1.5, bounds.height),
				)
				.intersection(&bounds)
				.unwrap_or_default(),
				..Quad::default()
			},
			theme.palette().primary.base.color,
		);

		let mut beat = px_to_time(self.offset, self.position, self.scale, self.transport);
		let end_beat = px_to_time(
			self.offset + bounds.width,
			self.position,
			self.scale,
			self.transport,
		);
		beat = beat
			.snap_floor(self.scale.x + 3.0, self.transport)
			.bar_floor(self.transport);

		let snap_step =
			MusicalTime::snap_step(self.scale.x + 3.0, self.transport).bar_ceil(self.transport);

		while beat <= end_beat {
			let bar = Text {
				content: (beat.bar(self.transport) + 1).to_string(),
				bounds: Size::new(f32::INFINITY, 0.0),
				size: renderer.default_size(),
				line_height: LineHeight::default(),
				font: Font::MONOSPACE,
				align_x: Alignment::Left,
				align_y: Vertical::Top,
				shaping: Shaping::Basic,
				wrapping: Wrapping::None,
				ellipsis: Ellipsis::None,
				hint_factor: renderer.scale_factor(),
			};

			renderer.fill_text(
				bar,
				offset_time(beat) + Vector::new(3.0, 0.0),
				theme.palette().primary.base.text,
				bounds,
			);

			beat += snap_step;
		}
	}
}

impl<'a, Message> From<Seeker<'a, Message>> for Element<'a, Message>
where
	Message: 'a,
{
	fn from(value: Seeker<'a, Message>) -> Self {
		Self::new(value)
	}
}
