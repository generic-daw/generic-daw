use crate::widget::{LINE_HEIGHT, get_time, maybe_snap_time};
use generic_daw_core::{MusicalTime, NotePosition, RtState};
use generic_daw_utils::NoDebug;
use iced::{
	Color, Element, Event, Fill, Font, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Clipboard, Layout, Renderer as _, Shell, Text, Widget,
		layout::{Limits, Node},
		mouse::{self, Cursor, Interaction, ScrollDelta},
		overlay,
		renderer::{Quad, Style},
		text::Renderer as _,
		widget::{Operation, Tree, tree},
	},
	alignment::Vertical,
	border, keyboard, padding,
	widget::text::{Alignment, LineHeight, Shaping, Wrapping},
	window,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Status {
	Seeking(MusicalTime),
	DraggingLoop(MusicalTime),
	Hovering,
	#[default]
	None,
}

#[derive(Default)]
struct State {
	status: Status,
	last_size: Size,
}

#[derive(Debug)]
pub struct Seeker<'a, Message> {
	rtstate: &'a RtState,
	position: &'a Vector,
	scale: &'a Vector,
	offset: f32,
	children: NoDebug<[Element<'a, Message>; 2]>,
	seek_to: fn(MusicalTime) -> Message,
	set_loop_marker: fn(Option<NotePosition>) -> Message,
	pan: fn(Vector, Size) -> Message,
	zoom: fn(Vector, Point, Size) -> Message,
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
		let left = self.children[0].as_widget_mut().layout(
			&mut tree.children[0],
			renderer,
			&Limits::new(limits.min(), Size::new(limits.max().width, f32::INFINITY)),
		);
		let left_width = left.size().width;

		let right = self.children[1].as_widget_mut().layout(
			&mut tree.children[1],
			renderer,
			&Limits::new(
				limits.min(),
				Size::new(limits.max().width - left_width, f32::INFINITY),
			),
		);

		Node::with_children(
			limits.max(),
			vec![
				left.translate(Vector::new(0.0, LINE_HEIGHT - self.position.y)),
				right.translate(Vector::new(left_width, LINE_HEIGHT - self.position.y)),
			],
		)
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		cursor: Cursor,
		renderer: &Renderer,
		clipboard: &mut dyn Clipboard,
		shell: &mut Shell<'_, Message>,
		_viewport: &Rectangle,
	) {
		let bounds = layout.bounds().shrink(padding::top(LINE_HEIGHT));

		self.children
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.for_each(|((child, tree), layout)| {
				let Some(viewport) = layout.bounds().intersection(&bounds) else {
					return;
				};

				child.as_widget_mut().update(
					tree, event, layout, cursor, renderer, clipboard, shell, &viewport,
				);
			});

		let state = tree.state.downcast_mut::<State>();
		let right_half = Self::right_half(layout).shrink(padding::top(LINE_HEIGHT));

		if let Event::Window(window::Event::RedrawRequested(..)) = event
			&& state.last_size != right_half.size()
		{
			state.last_size = right_half.size();
			shell.publish((self.pan)(Vector::ZERO, state.last_size));
			return;
		}

		if shell.is_event_captured() {
			return;
		}

		let Some(mut cursor) = cursor.position_in(layout.bounds()) else {
			state.status = Status::None;
			return;
		};
		cursor = cursor - Vector::new(right_half.x - layout.position().x, LINE_HEIGHT);
		cursor.x = cursor.x.max(0.0);

		match event {
			Event::Mouse(mouse::Event::CursorMoved { modifiers, .. })
			| Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
				let time = maybe_snap_time(
					get_time(cursor.x, *self.position, *self.scale, self.rtstate),
					*modifiers,
					|time| time.snap_round(self.scale.x, self.rtstate),
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
							NotePosition::new(start, end)
						});

						if self.rtstate.loop_marker != loop_marker {
							shell.publish((self.set_loop_marker)(loop_marker));
							shell.capture_event();
						}
					}
					_ => {
						state.status = if cursor.y < 0.0 {
							Status::Hovering
						} else {
							Status::None
						};
					}
				}
			}
			Event::Mouse(mouse::Event::ButtonPressed {
				button: mouse::Button::Left,
				modifiers,
			}) if state.status == Status::Hovering => {
				let time = maybe_snap_time(
					get_time(cursor.x, *self.position, *self.scale, self.rtstate),
					*modifiers,
					|time| time.snap_round(self.scale.x, self.rtstate),
				);
				state.status = if modifiers.command() {
					if let Some(loop_marker) = self.rtstate.loop_marker {
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
			Event::Mouse(mouse::Event::ButtonReleased {
				button: mouse::Button::Left,
				..
			}) if matches!(state.status, Status::Seeking(..) | Status::DraggingLoop(..)) => {
				state.status = if cursor.y < 0.0 {
					Status::Hovering
				} else {
					Status::None
				};
				shell.capture_event();
			}
			Event::Mouse(mouse::Event::WheelScrolled { delta, modifiers }) => {
				let (x, mut y) = match *delta {
					ScrollDelta::Pixels { x, y } => (-x, -y),
					ScrollDelta::Lines { x, y } => (-x * 60.0, -y * 60.0),
				};

				match (modifiers.command(), modifiers.shift(), modifiers.alt()) {
					(false, false, false) if x != 0.0 || y != 0.0 => {
						shell.publish((self.pan)(Vector::new(x, y), right_half.size()));
						shell.capture_event();
					}
					(true, false, false) if y != 0.0 => {
						y /= 128.0;
						shell.publish((self.zoom)(Vector::new(y, 0.0), cursor, right_half.size()));
						shell.capture_event();
					}
					(false, true, false) if x != 0.0 || y != 0.0 => {
						shell.publish((self.pan)(Vector::new(y, x), right_half.size()));
						shell.capture_event();
					}
					(false, false, true) if y != 0.0 => {
						y /= -8.0;
						shell.publish((self.zoom)(Vector::new(0.0, y), cursor, right_half.size()));
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
		let bounds = layout.bounds().shrink(padding::top(LINE_HEIGHT));
		let right_half = Self::right_half(layout);
		let right_child_bounds = right_half.shrink(padding::top(LINE_HEIGHT));

		renderer.with_layer(right_child_bounds, |renderer| {
			self.grid(renderer, right_child_bounds, theme);
		});

		renderer.with_layer(bounds, |renderer| {
			self.children
				.iter()
				.zip(&tree.children)
				.zip(layout.children())
				.for_each(|((child, tree), layout)| {
					child
						.as_widget()
						.draw(tree, renderer, theme, style, layout, cursor, &bounds);
				});
		});

		renderer.with_layer(right_half, |renderer| {
			self.seeker(renderer, Self::seeker_bounds(layout), theme);
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
		let state = tree.state.downcast_ref::<State>();
		if state.status == Status::None {
			let bounds = layout.bounds().shrink(padding::top(LINE_HEIGHT));

			self.children
				.iter()
				.zip(&tree.children)
				.zip(layout.children())
				.filter_map(|((child, tree), layout)| {
					let viewport = layout.bounds().intersection(&bounds)?;

					Some(
						child
							.as_widget()
							.mouse_interaction(tree, layout, cursor, &viewport, renderer),
					)
				})
				.max()
				.unwrap_or_default()
		} else {
			Interaction::ResizingHorizontally
		}
	}

	fn overlay<'a>(
		&'a mut self,
		tree: &'a mut Tree,
		layout: Layout<'a>,
		renderer: &Renderer,
		viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
		overlay::from_children(
			&mut *self.children,
			tree,
			layout,
			renderer,
			viewport,
			translation,
		)
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
				.for_each(|((child, state), layout)| {
					child
						.as_widget_mut()
						.operate(state, layout, renderer, operation);
				});
		});
	}
}

impl<'a, Message> Seeker<'a, Message> {
	pub fn new(
		rtstate: &'a RtState,
		position: &'a Vector,
		scale: &'a Vector,
		left: impl Into<Element<'a, Message>>,
		right: impl Into<Element<'a, Message>>,
		seek_to: fn(MusicalTime) -> Message,
		set_loop_marker: fn(Option<NotePosition>) -> Message,
		pan: fn(Vector, Size) -> Message,
		zoom: fn(Vector, Point, Size) -> Message,
	) -> Self {
		Self {
			rtstate,
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

	fn seeker_bounds(layout: Layout<'_>) -> Rectangle {
		let mut bounds = Self::right_half(layout);
		bounds.height = LINE_HEIGHT;
		bounds
	}

	fn right_half(layout: Layout<'_>) -> Rectangle {
		let bounds = layout.bounds();
		let right_child_bounds = layout.child(1).bounds();

		Rectangle::new(
			Point::new(right_child_bounds.x, bounds.y),
			Size::new(right_child_bounds.width, bounds.height),
		)
	}

	fn grid(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
		let samples_per_px = self.scale.x.exp2();

		let mut beat = MusicalTime::from_samples_f(self.position.x * samples_per_px, self.rtstate);
		let end_beat =
			beat + MusicalTime::from_samples_f(bounds.width * samples_per_px, self.rtstate);
		beat = beat.snap_floor(self.scale.x + 1.0, self.rtstate);

		let background_step = MusicalTime::new(4 * u64::from(self.rtstate.numerator.get()), 0);
		let mut background_beat =
			MusicalTime::new(beat.beat() - (beat.beat() % background_step.beat()), 0);
		let background_width = background_step.to_samples_f(self.rtstate) / samples_per_px;

		while background_beat < end_beat {
			if background_beat.bar(self.rtstate).is_multiple_of(8) {
				let x =
					background_beat.to_samples_f(self.rtstate) / samples_per_px - self.position.x;

				renderer.fill_quad(
					Quad {
						bounds: Rectangle::new(
							bounds.position() + Vector::new(x, 0.0),
							Size::new(background_width, bounds.height),
						),
						..Quad::default()
					},
					theme.extended_palette().background.weakest.color,
				);
			}

			background_beat += background_step;
		}

		let snap_step = MusicalTime::snap_step(self.scale.x + 1.0, self.rtstate);

		while beat <= end_beat {
			let color = if snap_step >= MusicalTime::BEAT {
				if beat
					.beat()
					.is_multiple_of(snap_step.beat() * u64::from(self.rtstate.numerator.get()))
				{
					theme.extended_palette().background.strong.color
				} else {
					theme.extended_palette().background.weak.color
				}
			} else if beat.tick() == 0 {
				theme.extended_palette().background.strong.color
			} else {
				theme.extended_palette().background.weak.color
			};

			let x = beat.to_samples_f(self.rtstate) / samples_per_px - self.position.x;

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(
						bounds.position() + Vector::new(x, 0.0),
						Size::new(1.0, bounds.height),
					),
					..Quad::default()
				},
				color,
			);

			beat += snap_step;
		}

		renderer.fill_quad(
			Quad {
				bounds,
				border: border::width(1).color(theme.extended_palette().background.strong.color),
				..Quad::default()
			},
			Color::TRANSPARENT,
		);
	}

	fn seeker(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
		renderer.fill_quad(
			Quad {
				bounds,
				..Quad::default()
			},
			if self.rtstate.loop_marker.is_some() {
				theme.extended_palette().secondary.base.color
			} else {
				theme.extended_palette().primary.base.color
			},
		);

		let samples_per_px = self.scale.x.exp2();

		let offset_pos = |time: f32| {
			bounds.position()
				+ Vector::new(time / samples_per_px - self.position.x - self.offset, 0.0)
		};
		let offset_time = |time: MusicalTime| offset_pos(time.to_samples_f(self.rtstate));

		if let Some(loop_marker) = self.rtstate.loop_marker {
			let start = offset_time(loop_marker.start());
			let end = offset_time(loop_marker.end());

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(start - Vector::new(1.5, 0.0), Size::new(1.5, 10000.0)),
					..Quad::default()
				},
				theme.extended_palette().secondary.base.color,
			);

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(
						start - Vector::new(10000.0, 0.0),
						Size::new(10000.0, 10000.0),
					),
					..Quad::default()
				},
				theme
					.extended_palette()
					.secondary
					.base
					.color
					.scale_alpha(0.2),
			);

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(end, Size::new(1.5, 10000.0)),
					..Quad::default()
				},
				theme.extended_palette().secondary.base.color,
			);

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(end, Size::new(10000.0, 10000.0)),
					..Quad::default()
				},
				theme
					.extended_palette()
					.secondary
					.base
					.color
					.scale_alpha(0.2),
			);

			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(start, Size::new(end.x - start.x, bounds.height)),
					..Quad::default()
				},
				theme.extended_palette().primary.base.color,
			);
		}

		renderer.fill_quad(
			Quad {
				bounds: Rectangle::new(
					offset_pos(self.rtstate.sample as f32),
					Size::new(1.5, 10000.0),
				),
				..Quad::default()
			},
			theme.extended_palette().primary.base.color,
		);

		let mut draw_text = |beat: MusicalTime, bar: u64| {
			let bar = Text {
				content: (bar + 1).to_string(),
				bounds: Size::new(f32::INFINITY, 0.0),
				size: renderer.default_size(),
				line_height: LineHeight::default(),
				font: Font::MONOSPACE,
				align_x: Alignment::Left,
				align_y: Vertical::Top,
				shaping: Shaping::Basic,
				wrapping: Wrapping::None,
			};

			renderer.fill_text(
				bar,
				offset_time(beat) + Vector::new(3.0, 0.0),
				theme.extended_palette().primary.base.text,
				bounds,
			);
		};

		let mut beat = MusicalTime::from_samples_f(self.position.x * samples_per_px, self.rtstate);
		let mut end_beat =
			beat + MusicalTime::from_samples_f(bounds.width * samples_per_px, self.rtstate);
		beat = beat.snap_floor(self.scale.x + 2.0, self.rtstate).floor();
		end_beat = end_beat.snap_floor(self.scale.x + 2.0, self.rtstate);

		let bar_inc = MusicalTime::snap_step(self.scale.x + 2.0, self.rtstate)
			.bar(self.rtstate)
			.max(1);

		while beat <= end_beat {
			let bar = beat.bar(self.rtstate);

			if beat
				.beat()
				.is_multiple_of(self.rtstate.numerator.get().into())
				&& bar.is_multiple_of(bar_inc)
			{
				draw_text(beat, bar);
			}

			beat += MusicalTime::BEAT;
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
