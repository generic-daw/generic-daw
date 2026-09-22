use crate::{
	state::Grid,
	widget::{ALPHA_1_3, Delta, LINE_HEIGHT, px_to_time, time_to_px},
};
use generic_daw_core::{
	Transport,
	time::{BeatRange, BeatTime, SecondsTime},
};
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
	widget::text::{Alignment, Ellipsis, Shaping, Wrapping},
	window,
};
use std::time::Instant;
use utils::NoDebug;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Status {
	SeekingBeats(BeatTime),
	DraggingLoopBeats(BeatTime),
	TrimmingLoopBeats(BeatTime),
	SeekingSeconds(SecondsTime),
	DraggingLoopSeconds(SecondsTime),
	TrimmingLoopSeconds(SecondsTime),
	Panning(Point),
	#[default]
	None,
}

#[derive(Default)]
struct State {
	status: Status,
	last_height: f32,
	horizontal_autoscroll_start: Option<Instant>,
	vertical_autoscroll_start: Option<Instant>,
	last_autoscroll: Option<Instant>,
}

#[derive(Debug)]
pub struct Seeker<'a, Message> {
	transport: &'a Transport,
	grid: &'a Grid,
	position: Vector,
	scale: Vector,
	children: NoDebug<[Element<'a, Message>; 2]>,
	seek_to: fn(BeatTime) -> Message,
	set_loop_range: fn(Option<BeatRange>) -> Message,
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

	fn diff(&mut self, tree: &mut Tree) {
		tree.diff_children(&mut *self.children);
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		let left = self.children[0]
			.as_widget_mut()
			.layout(&mut tree.children[0], renderer, &Limits::NONE)
			.translate(Vector::new(0.0, LINE_HEIGHT));

		let right = self.children[1]
			.as_widget_mut()
			.layout(&mut tree.children[1], renderer, &Limits::NONE)
			.translate(Vector::new(0.0, LINE_HEIGHT))
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
		let was_event_captured = shell.is_event_captured();

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
					} + Vector::new(0.0, self.position.y),
					renderer,
					shell,
					&(viewport + Vector::new(0.0, self.position.y)),
				);
			});

		let state = tree.state.downcast_mut::<State>();
		let top_seeker = Self::top_seeker(layout);
		let bottom_seeker = Self::bottom_seeker(layout);
		let left_viewport = Self::left_viewport(layout);
		let right_viewport = Self::right_viewport(layout);
		let height = right_viewport.height;

		let visible = layout.child(1).bounds().height - self.position.y;

		if let Event::Window(window::Event::RedrawRequested(..)) = event
			&& state.last_height != height
		{
			state.last_height = height;
			shell.publish((self.pan)(Vector::ZERO, height, visible));
			return;
		}

		if let Event::Mouse(mouse::Event::ButtonReleased { .. }) = event
			&& state.status != Status::None
		{
			state.status = Status::None;
			shell.capture_event();
			return;
		}

		if was_event_captured {
			return;
		}

		let cursor = 'block: {
			if let Some(cursor) = cursor.position_in(right_viewport) {
				state.horizontal_autoscroll_start = None;
				state.vertical_autoscroll_start = None;
				state.last_autoscroll = None;
				break 'block cursor;
			}

			let Some(cursor) = cursor.position_from(right_viewport.position()) else {
				return;
			};

			match state.status {
				Status::None if !shell.is_event_captured() => break 'block cursor,
				Status::Panning(..) => break 'block cursor,
				_ => {}
			}

			let clamped = Point::new(
				cursor.x.clamp(0.0, right_viewport.width),
				cursor.y.clamp(0.0, right_viewport.height),
			);

			debug_assert_ne!(cursor, clamped);

			shell.request_redraw();

			let &Event::Window(window::Event::RedrawRequested(now)) = event else {
				break 'block clamped;
			};

			let Some(last_frame) = state.last_autoscroll.replace(now) else {
				break 'block clamped;
			};

			if last_frame == now {
				break 'block clamped;
			}

			let delta = Vector::new(
				if cursor.x == clamped.x {
					state.horizontal_autoscroll_start = None;
					0.0
				} else {
					let autoscroll_start = *state.horizontal_autoscroll_start.get_or_insert(now);
					let autoscroll_amt = (now - autoscroll_start).as_secs_f32().powf(1.5)
						- (last_frame - autoscroll_start).as_secs_f32().powf(1.5);
					1000.0 * autoscroll_amt.copysign(cursor.x - clamped.x)
				},
				if !shell.is_event_captured() || cursor.y == clamped.y {
					state.vertical_autoscroll_start = None;
					0.0
				} else {
					let autoscroll_start = *state.vertical_autoscroll_start.get_or_insert(now);
					let autoscroll_amt = (now - autoscroll_start).as_secs_f32().powf(1.5)
						- (last_frame - autoscroll_start).as_secs_f32().powf(1.5);
					1000.0 * autoscroll_amt.copysign(cursor.y - clamped.y)
				},
			);

			shell.publish((self.pan)(delta, height, visible));

			clamped
		};

		if shell.is_event_captured() {
			return;
		}

		let offset = Vector::new(right_viewport.x, right_viewport.y);

		let new_time = px_to_time(cursor.x, self.position, self.scale, self.transport);

		match event {
			Event::Mouse(mouse::Event::CursorMoved { modifiers, .. })
			| Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => match state.status {
				Status::SeekingBeats(last_time) => {
					let time = self.grid.maybe_snap(new_time, *modifiers, |time| {
						time.round(self.grid.beats_snap_step(self.scale, self.transport))
					});

					if last_time != time {
						state.status = Status::SeekingBeats(time);
						shell.publish((self.seek_to)(time));
						shell.capture_event();
					}
				}
				Status::DraggingLoopBeats(last_time) => {
					let abs_diff = self.grid.maybe_snap(
						new_time.abs_diff(last_time),
						*modifiers,
						|abs_diff| {
							abs_diff.round(self.grid.beats_snap_step(self.scale, self.transport))
						},
					);

					if abs_diff != BeatTime::ZERO {
						let time_delta = if new_time > last_time {
							Delta::Positive
						} else {
							Delta::Negative
						}(abs_diff);

						let loop_range = self.transport.loop_range.unwrap() + time_delta;

						shell.publish((self.set_loop_range)(loop_range));
						shell.capture_event();

						state.status = if loop_range.is_some() {
							Status::DraggingLoopBeats(last_time + time_delta)
						} else {
							Status::None
						};
					}
				}
				Status::TrimmingLoopBeats(last_time) => {
					let time = self.grid.maybe_snap(new_time, *modifiers, |time| {
						time.round(self.grid.beats_snap_step(self.scale, self.transport))
					});

					let loop_range = (last_time != time).then(|| {
						let start = last_time.min(time);
						let end = last_time.max(time);
						BeatRange::new(start, end)
					});

					if self.transport.loop_range != loop_range {
						shell.publish((self.set_loop_range)(loop_range));
						shell.capture_event();
					}
				}
				Status::SeekingSeconds(last_time) => {
					let time = self.grid.maybe_snap(
						new_time.to_seconds_time(self.transport),
						*modifiers,
						|time| time.round(self.grid.seconds_snap_step(self.scale)),
					);

					if last_time != time {
						state.status = Status::SeekingSeconds(time);
						shell.publish((self.seek_to)(time.to_beat_time(self.transport)));
						shell.capture_event();
					}
				}
				Status::DraggingLoopSeconds(last_time) => {
					let new_time = new_time.to_seconds_time(self.transport);

					let abs_diff = self
						.grid
						.maybe_snap(new_time.abs_diff(last_time), *modifiers, |abs_diff| {
							abs_diff.round(self.grid.seconds_snap_step(self.scale))
						})
						.to_beat_time(self.transport);

					if abs_diff != BeatTime::ZERO {
						let time_delta = if new_time > last_time {
							Delta::Positive
						} else {
							Delta::Negative
						}(abs_diff);

						let loop_range = self.transport.loop_range.unwrap() + time_delta;

						shell.publish((self.set_loop_range)(loop_range));
						shell.capture_event();

						state.status = if loop_range.is_some() {
							Status::DraggingLoopSeconds(
								last_time
									+ time_delta.map(|time_delta| {
										time_delta.to_seconds_time(self.transport)
									}),
							)
						} else {
							Status::None
						};
					}
				}
				Status::TrimmingLoopSeconds(last_time) => {
					let time = self.grid.maybe_snap(
						new_time.to_seconds_time(self.transport),
						*modifiers,
						|time| time.round(self.grid.seconds_snap_step(self.scale)),
					);

					let loop_range = (last_time != time).then(|| {
						let start = last_time.min(time);
						let end = last_time.max(time);
						BeatRange::new(
							start.to_beat_time(self.transport),
							end.to_beat_time(self.transport),
						)
					});

					if self.transport.loop_range != loop_range {
						shell.publish((self.set_loop_range)(loop_range));
						shell.capture_event();
					}
				}
				Status::Panning(last_pos) => {
					let delta = last_pos - cursor;

					if delta != Vector::ZERO {
						state.status = Status::Panning(cursor);
						match (modifiers.command(), modifiers.shift(), modifiers.alt()) {
							(false, false, false) => {
								shell.publish((self.pan)(delta, height, visible));
							}
							(true, false, false) => {
								shell.publish((self.zoom)(
									Vector::new(delta.y / -64.0, 0.0),
									cursor - Vector::new(0.0, LINE_HEIGHT),
									height,
									visible,
								));
							}
							(false, false, true) => {
								shell.publish((self.zoom)(
									Vector::new(0.0, delta.y / 2.0),
									cursor - Vector::new(0.0, LINE_HEIGHT),
									height,
									visible,
								));
							}
							_ => {}
						}
						shell.capture_event();
					}
				}
				Status::None => {}
			},
			Event::Mouse(mouse::Event::ButtonPressed {
				button: mouse::Button::Left,
				modifiers,
			}) if top_seeker.contains(cursor + offset) => {
				let snap_step = self.grid.beats_snap_step(self.scale, self.transport);
				let time = self
					.grid
					.maybe_snap(new_time, *modifiers, |time| time.round(snap_step));
				state.status = if modifiers.command() {
					if let Some(loop_range) = self.transport.loop_range {
						let (start, end) = (loop_range.start(), loop_range.end());
						if modifiers.shift() {
							Status::DraggingLoopBeats(time)
						} else if time
							== self
								.grid
								.maybe_snap(start, *modifiers, |time| time.round(snap_step))
						{
							Status::TrimmingLoopBeats(end)
						} else if time
							== self
								.grid
								.maybe_snap(end, *modifiers, |time| time.round(snap_step))
						{
							Status::TrimmingLoopBeats(start)
						} else {
							shell.publish((self.set_loop_range)(None));
							Status::TrimmingLoopBeats(time)
						}
					} else {
						Status::TrimmingLoopBeats(time)
					}
				} else {
					shell.publish((self.seek_to)(time));
					Status::SeekingBeats(time)
				};
				shell.capture_event();
			}
			Event::Mouse(mouse::Event::ButtonPressed {
				button: mouse::Button::Left,
				modifiers,
			}) if bottom_seeker.contains(cursor + offset) => {
				let snap_step = self.grid.seconds_snap_step(self.scale);
				let time = self.grid.maybe_snap(
					new_time.to_seconds_time(self.transport),
					*modifiers,
					|time| time.round(snap_step),
				);
				state.status = if modifiers.command() {
					if let Some(loop_range) = self.transport.loop_range {
						let (start, end) = (
							loop_range.start().to_seconds_time(self.transport),
							loop_range.end().to_seconds_time(self.transport),
						);
						if modifiers.shift() {
							Status::DraggingLoopSeconds(time)
						} else if time
							== self
								.grid
								.maybe_snap(start, *modifiers, |time| time.round(snap_step))
						{
							Status::TrimmingLoopSeconds(end)
						} else if time
							== self
								.grid
								.maybe_snap(end, *modifiers, |time| time.round(snap_step))
						{
							Status::TrimmingLoopSeconds(start)
						} else {
							shell.publish((self.set_loop_range)(None));
							Status::TrimmingLoopSeconds(time)
						}
					} else {
						Status::TrimmingLoopSeconds(time)
					}
				} else {
					shell.publish((self.seek_to)(time.to_beat_time(self.transport)));
					Status::SeekingSeconds(time)
				};
			}
			Event::Mouse(mouse::Event::ButtonPressed {
				button: mouse::Button::Middle,
				modifiers,
			}) if right_viewport.contains(cursor + offset) => {
				state.status = Status::Panning(cursor);
			}
			Event::Mouse(mouse::Event::WheelScrolled { delta, modifiers }) => {
				let (x, y) = match *delta {
					ScrollDelta::Pixels { x, y } => (-x, -y),
					ScrollDelta::Lines { x, y } => (-x * 60.0, -y * 60.0),
				};

				if let Some((mut x, mut y, pan)) =
					match (modifiers.command(), modifiers.shift(), modifiers.alt()) {
						(false, false, false) => Some((x, y, true)),
						(true, false, false) => Some((y / 128.0, 0.0, false)),
						(false, true, false) => Some((y, x, true)),
						(false, false, true) => Some((0.0, y / -8.0, false)),
						_ => None,
					} {
					if !(right_viewport.contains(cursor + offset)
						|| top_seeker.contains(cursor + offset)
						|| bottom_seeker.contains(cursor + offset))
					{
						x = 0.0;
					}

					if !(right_viewport.contains(cursor + offset)
						|| left_viewport.contains(cursor + offset))
					{
						y = 0.0;
					}

					if x != 0.0 || y != 0.0 {
						shell.publish(if pan {
							(self.pan)(Vector::new(x, y), height, visible)
						} else {
							(self.zoom)(Vector::new(x, y), cursor, height, visible)
						});
						shell.capture_event();
					}
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
		self.bottom_layer(renderer, Self::right_viewport(layout), theme);

		renderer.with_layer(
			layout.bounds().shrink(padding::vertical(LINE_HEIGHT)),
			|renderer| {
				self.children
					.iter()
					.zip(&tree.children)
					.zip(layout.children())
					.zip(Self::viewports(layout))
					.for_each(|(((child, tree), layout), viewport)| {
						renderer.with_translation(-Vector::new(0.0, self.position.y), |renderer| {
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
								} + Vector::new(0.0, self.position.y),
								&(viewport + Vector::new(0.0, self.position.y)),
							);
						});
					});
			},
		);

		renderer.with_layer(Rectangle::INFINITE, |renderer| {
			self.top_layer(renderer, Self::right_viewport(layout), theme);
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
			Status::SeekingBeats(..)
			| Status::DraggingLoopBeats(..)
			| Status::TrimmingLoopBeats(..)
			| Status::SeekingSeconds(..)
			| Status::DraggingLoopSeconds(..)
			| Status::TrimmingLoopSeconds(..) => Interaction::ResizingHorizontally,
			Status::Panning(..) => Interaction::Move,
			Status::None => {
				if cursor.is_over(Self::top_seeker(layout))
					|| cursor.is_over(Self::bottom_seeker(layout))
				{
					Interaction::ResizingHorizontally
				} else {
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
								} + Vector::new(0.0, self.position.y),
								&(viewport + Vector::new(0.0, self.position.y)),
								renderer,
							)
						})
						.max()
						.unwrap_or_default()
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
				child.as_widget_mut().overlay(
					tree,
					layout,
					renderer,
					&(viewport + Vector::new(0.0, self.position.y)),
					translation - Vector::new(0.0, self.position.y),
				)
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
		grid: &'a Grid,
		position: Vector,
		scale: Vector,
		left: impl Into<Element<'a, Message>>,
		right: impl Into<Element<'a, Message>>,
		seek_to: fn(BeatTime) -> Message,
		set_loop_range: fn(Option<BeatRange>) -> Message,
		pan: fn(Vector, f32, f32) -> Message,
		zoom: fn(Vector, Point, f32, f32) -> Message,
	) -> Self {
		Self {
			transport,
			grid,
			position,
			scale,
			children: [left.into(), right.into()].into(),
			seek_to,
			set_loop_range,
			pan,
			zoom,
		}
	}

	fn viewports(layout: Layout<'_>) -> [Rectangle; 2] {
		[Self::left_viewport(layout), Self::right_viewport(layout)]
	}

	fn left_viewport(layout: Layout<'_>) -> Rectangle {
		layout
			.bounds()
			.shrink(padding::right(Self::right_viewport(layout).width).vertical(LINE_HEIGHT))
	}

	fn right_viewport(layout: Layout<'_>) -> Rectangle {
		layout
			.bounds()
			.shrink(padding::left(layout.child(0).bounds().width).vertical(LINE_HEIGHT))
	}

	fn top_seeker(layout: Layout<'_>) -> Rectangle {
		Rectangle {
			height: LINE_HEIGHT,
			..layout.bounds()
		}
		.shrink(padding::left(layout.child(0).bounds().width))
	}

	fn bottom_seeker(layout: Layout<'_>) -> Rectangle {
		Self::top_seeker(layout) + Vector::new(0.0, layout.bounds().height - LINE_HEIGHT)
	}

	fn bottom_layer(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
		let offset_time = |time: BeatTime| {
			bounds.position()
				+ Vector::new(
					time_to_px(time, self.position, self.scale, self.transport),
					0.0,
				)
		};

		let snap_step = self
			.grid
			.beats_snap_step(self.scale + Vector::new(1.0, 0.0), self.transport);

		let mut beat = px_to_time(0.0, self.position, self.scale, self.transport).floor(snap_step);
		let end_beat = px_to_time(bounds.width, self.position, self.scale, self.transport);

		let background_step = BeatTime::new(u64::from(self.transport.numerator.get()), 0) * 8;
		let mut background_beat = beat.round(background_step);
		let background_width =
			time_to_px(background_step, Vector::ZERO, self.scale, self.transport);

		while background_beat < end_beat {
			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(
						offset_time(background_beat),
						Size::new(background_width / 2.0, bounds.height),
					)
					.intersection(&bounds)
					.unwrap_or_default(),
					..Quad::default()
				},
				theme.palette().background.weakest.color,
			);
			background_beat += background_step;
		}

		while beat <= end_beat {
			let color = if snap_step >= BeatTime::BEAT {
				if beat.beat_in_bar(self.transport) == 0
					&& beat.bar(self.transport).is_multiple_of(snap_step.beat())
				{
					theme.palette().background.strongest.color
				} else {
					theme.palette().background.weak.color
				}
			} else if beat.tick() == 0 {
				theme.palette().background.strongest.color
			} else {
				theme.palette().background.weak.color
			};

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(offset_time(beat), Size::new(1.5, bounds.height))
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

		let bounds = bounds.expand(padding::vertical(LINE_HEIGHT));
		let offset_time = |time: BeatTime| {
			bounds.position()
				+ Vector::new(
					time_to_px(time, self.position, self.scale, self.transport),
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
			if self.transport.loop_range.is_some() {
				theme.palette().secondary.base.color
			} else {
				theme.palette().primary.base.color
			},
		);

		let offset = Vector::new(0.0, bounds.height - LINE_HEIGHT);

		renderer.fill_quad(
			Quad {
				bounds: Rectangle::new(
					bounds.position() + offset,
					Size::new(bounds.width, LINE_HEIGHT),
				)
				.intersection(&bounds)
				.unwrap_or_default(),
				..Quad::default()
			},
			if self.transport.loop_range.is_some() {
				theme.palette().secondary.base.color
			} else {
				theme.palette().primary.base.color
			},
		);

		if let Some(loop_range) = self.transport.loop_range {
			let start = offset_time(loop_range.start());
			let end = offset_time(loop_range.end());

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
					bounds: Rectangle::new(start + offset, Size::new(end.x - start.x, LINE_HEIGHT))
						.intersection(&bounds)
						.unwrap_or_default(),
					..Quad::default()
				},
				theme.palette().primary.base.color,
			);
		}

		let mut numbering_grid = Grid::default();
		numbering_grid.size = numbering_grid.size.max(self.grid.size);

		let snap_step = numbering_grid
			.beats_snap_step(self.scale + Vector::new(3.0, 0.0), self.transport)
			.beat_ceil();

		let mut beat = px_to_time(0.0, self.position, self.scale, self.transport).floor(snap_step);

		while beat <= end_beat {
			let content = if snap_step == BeatTime::BEAT {
				format!(
					"{}:{:0digits$}",
					beat.bar(self.transport) + 1,
					beat.beat_in_bar(self.transport) + 1,
					digits = self.transport.numerator.ilog10() as usize + 1,
				)
			} else {
				format!("{}", beat.bar(self.transport) + 1)
			};

			let bar = Text {
				content,
				bounds: Size::new(f32::INFINITY, 0.0),
				size: renderer.text_size(),
				line_height: renderer.line_height(),
				font: Font::MONOSPACE,
				align_x: Alignment::Left,
				align_y: Vertical::Bottom,
				shaping: Shaping::Basic,
				wrapping: Wrapping::None,
				ellipsis: Ellipsis::None,
				hint_factor: renderer.hint_factor(),
			};

			let pos = offset_time(beat) + Vector::new(0.0, LINE_HEIGHT);

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(
						pos - Vector::new(0.0, LINE_HEIGHT / 3.0),
						Size::new(1.5, LINE_HEIGHT / 3.0),
					)
					.intersection(&bounds)
					.unwrap_or_default(),
					..Quad::default()
				},
				theme.palette().primary.base.text,
			);

			renderer.fill_text(
				bar,
				pos + Vector::new(3.0, 0.0),
				theme.palette().primary.base.text,
				bounds,
			);

			beat += snap_step;
		}

		let snap_step = numbering_grid
			.seconds_snap_step(self.scale + Vector::new(3.0, 0.0))
			.second_ceil();

		let mut second = px_to_time(0.0, self.position, self.scale, self.transport)
			.to_seconds_time(self.transport)
			.floor(snap_step)
			.second_floor();
		let end_second = end_beat.to_seconds_time(self.transport);

		while second <= end_second {
			let bar = Text {
				content: format!("{}:{:02}", second.second() / 60, second.second() % 60),
				bounds: Size::new(f32::INFINITY, 0.0),
				size: renderer.text_size(),
				line_height: renderer.line_height(),
				font: Font::MONOSPACE,
				align_x: Alignment::Left,
				align_y: Vertical::Top,
				shaping: Shaping::Basic,
				wrapping: Wrapping::None,
				ellipsis: Ellipsis::None,
				hint_factor: renderer.hint_factor(),
			};

			let pos = offset_time(second.to_beat_time(self.transport)) + offset;

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(pos, Size::new(1.5, LINE_HEIGHT / 3.0))
						.intersection(&bounds)
						.unwrap_or_default(),
					..Quad::default()
				},
				theme.palette().primary.base.text,
			);

			renderer.fill_text(
				bar,
				pos + Vector::new(3.0, 0.0),
				theme.palette().primary.base.text,
				bounds,
			);

			second += snap_step;
		}
	}

	fn top_layer(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
		let offset_time = |time: BeatTime| {
			bounds.position()
				+ Vector::new(
					time_to_px(time, self.position, self.scale, self.transport),
					0.0,
				)
		};

		renderer.fill_quad(
			Quad {
				bounds: Rectangle::new(
					offset_time(self.transport.position.to_beat_time(self.transport)),
					Size::new(1.5, bounds.height),
				)
				.intersection(&bounds)
				.unwrap_or_default(),
				..Quad::default()
			},
			theme.palette().primary.base.color,
		);

		if let Some(loop_range) = self.transport.loop_range {
			let start = offset_time(loop_range.start());
			let end = offset_time(loop_range.end());

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
	}
}

impl<'a, Message: 'a> From<Seeker<'a, Message>> for Element<'a, Message> {
	fn from(value: Seeker<'a, Message>) -> Self {
		Self::new(value)
	}
}
