use crate::widget::{
	ALPHA_1_3, Delta, beats_snap_step, key_to_px, maybe_snap, note::Note, px_to_key, px_to_time,
	time_to_px,
};
use generic_daw_core::{MidiKey, Transport, time::BeatTime};
use iced::{
	Element, Event, Fill, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Layout, Renderer as _, Shell, Widget,
		layout::{Limits, Node},
		mouse::{self, Cursor, Interaction},
		overlay,
		renderer::{Quad, Style},
		widget::{Operation, Tree},
	},
	border, keyboard,
};
use std::{cell::RefCell, collections::HashSet};

#[derive(Clone, Copy, Debug)]
pub enum Action {
	Pan(Vector, f32, f32),
	Zoom(Vector, Point, f32, f32),
	Add(MidiKey, BeatTime),
	Clone,
	Drag(Delta<MidiKey>, Delta<BeatTime>),
	TrimStart(Delta<BeatTime>),
	TrimEnd(Delta<BeatTime>),
	SplitAt(BeatTime),
	DragSplit(BeatTime),
	DragVelocity(f32),
	Delete,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Status {
	Selecting(MidiKey, MidiKey, BeatTime, BeatTime),
	Dragging(MidiKey, BeatTime),
	TrimmingStart(BeatTime),
	TrimmingEnd(BeatTime),
	DraggingSplit(BeatTime),
	DraggingVelocity(usize, f32),
	Deleting,
	#[default]
	None,
}

#[derive(Debug, Default)]
pub struct State {
	pub status: Status,
	pub primary: HashSet<usize>,
	pub secondary: HashSet<usize>,
	pub position: Vector,
	pub scale: Vector,
}

impl State {
	pub fn new(position: Vector, scale: Vector) -> Self {
		Self {
			position,
			scale,
			..Self::default()
		}
	}

	pub fn finish(&mut self) {
		self.status = Status::None;
		self.primary.extend(self.secondary.drain());
	}

	pub fn clear(&mut self) {
		self.status = Status::None;
		self.primary.clear();
		self.secondary.clear();
	}
}

#[derive(Debug)]
pub struct PianoRoll<'a, Message> {
	state: &'a RefCell<State>,
	transport: &'a Transport,
	notes: Box<[Note<'a, Message>]>,
	action: fn(Action) -> Message,
}

impl<'a, Message: 'a> Widget<Message, Theme, Renderer> for PianoRoll<'a, Message> {
	fn diff(&mut self, tree: &mut Tree) {
		tree.diff_children(&mut self.notes);
	}

	fn size(&self) -> Size<Length> {
		Size::new(Fill, Length::Fixed(128.0 * self.state.borrow().scale.y))
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		Node::with_children(
			Size::new(limits.max().width, 128.0 * self.state.borrow().scale.y),
			self.notes
				.iter_mut()
				.zip(&mut tree.children)
				.map(|(child, tree)| {
					child.layout(tree, renderer, &limits.height(self.state.borrow().scale.y))
				})
				.collect(),
		)
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		cursor: Cursor,
		renderer: &Renderer,
		shell: &mut Shell<'_, Message>,
		viewport: &Rectangle,
	) {
		self.notes
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.rev()
			.for_each(|((child, tree), layout)| {
				child.update(tree, event, layout, cursor, renderer, shell, viewport);
			});

		if shell.is_event_captured() {
			return;
		}

		let state = &mut *self.state.borrow_mut();

		if let Event::Mouse(mouse::Event::ButtonReleased { .. }) = event
			&& state.status != Status::None
		{
			state.finish();
			shell.capture_event();
			shell.request_redraw();
			return;
		}

		let cursor = match cursor.position_in(*viewport) {
			Some(cursor) => cursor,
			None if state.status == Status::None => return,
			None => {
				shell.capture_event();
				match cursor.land().position_from(viewport.position()) {
					Some(cursor) => Point::new(
						cursor.x.clamp(0.0, viewport.width),
						cursor.y.clamp(0.0, viewport.height),
					),
					None => return,
				}
			}
		};

		let new_time = px_to_time(cursor.x, state.position, state.scale, self.transport);
		let snap_step = beats_snap_step(state.scale, self.transport);

		match event {
			Event::Mouse(mouse::Event::ButtonPressed {
				button: mouse::Button::Left,
				modifiers,
			}) if state.status == Status::None => {
				let key = px_to_key(cursor.y, state.position, state.scale);

				if modifiers.command() {
					let time = maybe_snap(new_time, *modifiers, |time| time.round(snap_step));

					state.status = Status::Selecting(key, key, time, time);
				} else {
					let time = maybe_snap(new_time, *modifiers, |time| time.floor(snap_step));

					state.primary.clear();
					shell.publish((self.action)(Action::Add(key, time)));

					state.status = Status::Dragging(key, new_time);
				}

				shell.capture_event();
				shell.request_redraw();
			}
			Event::Mouse(mouse::Event::ButtonPressed {
				button: mouse::Button::Right,
				..
			}) if state.status == Status::None => {
				state.clear();
				state.status = Status::Deleting;
				shell.capture_event();
				shell.request_redraw();
			}
			Event::Mouse(mouse::Event::CursorMoved { modifiers, .. })
			| Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => match state.status {
				Status::Selecting(start_key, last_end_key, start_pos, last_end_pos) => {
					let end_key = px_to_key(cursor.y, state.position, state.scale);

					let end_pos = maybe_snap(new_time, *modifiers, |time| time.round(snap_step));

					if end_key == last_end_key && end_pos == last_end_pos {
						return;
					}

					state.status = Status::Selecting(start_key, end_key, start_pos, end_pos);

					let (start_key, end_key) = (start_key.min(end_key), start_key.max(end_key));
					let (start_pos, end_pos) = (start_pos.min(end_pos), start_pos.max(end_pos));

					self.notes.iter().for_each(|note| {
						if (start_key..=end_key).contains(&note.note.key)
							&& (start_pos.max(note.note.position.start())
								< end_pos.min(note.note.position.end()))
						{
							state.secondary.insert(note.index);
						} else {
							state.secondary.remove(&note.index);
						}
					});

					shell.request_redraw();
				}
				Status::Dragging(key, time) => {
					let new_key = px_to_key(cursor.y, state.position, state.scale);

					let abs_diff = maybe_snap(new_time.abs_diff(time), *modifiers, |abs_diff| {
						abs_diff.round(snap_step)
					});

					if new_key != key || abs_diff != BeatTime::ZERO {
						let key_delta = if new_key > key {
							Delta::Positive
						} else {
							Delta::Negative
						}(MidiKey(new_key.0.abs_diff(key.0)));

						let time_delta = if new_time > time {
							Delta::Positive
						} else {
							Delta::Negative
						}(abs_diff);

						state.status = Status::Dragging(new_key, time + time_delta);
						shell.publish((self.action)(Action::Drag(key_delta, time_delta)));
						shell.capture_event();
					}
				}
				Status::TrimmingStart(time) => {
					let abs_diff = maybe_snap(new_time.abs_diff(time), *modifiers, |abs_diff| {
						abs_diff.round(snap_step)
					});

					if abs_diff != BeatTime::ZERO {
						let delta = if new_time > time {
							Delta::Positive
						} else {
							Delta::Negative
						}(abs_diff);

						state.status = Status::TrimmingStart(time + delta);
						shell.publish((self.action)(Action::TrimStart(delta)));
						shell.capture_event();
					}
				}
				Status::TrimmingEnd(time) => {
					let abs_diff = maybe_snap(new_time.abs_diff(time), *modifiers, |abs_diff| {
						abs_diff.round(snap_step)
					});

					if abs_diff != BeatTime::ZERO {
						let delta = if new_time > time {
							Delta::Positive
						} else {
							Delta::Negative
						}(abs_diff);

						state.status = Status::TrimmingEnd(time + delta);
						shell.publish((self.action)(Action::TrimEnd(delta)));
						shell.capture_event();
					}
				}
				Status::DraggingSplit(time) => {
					let new_time = maybe_snap(new_time, *modifiers, |time| time.round(snap_step));

					if new_time != time {
						state.status = Status::DraggingSplit(new_time);
						shell.publish((self.action)(Action::DragSplit(new_time)));
						shell.capture_event();
					}
				}
				Status::DraggingVelocity(note, val) => {
					if let Some(note_bounds) = layout.child(note).bounds().intersection(viewport) {
						let border = 10f32.min(note_bounds.width / 3.0);
						let new_val = (cursor.x - border - note_bounds.x + viewport.x)
							/ (note_bounds.width - 2.0 * border - 1.0);

						let new_val = maybe_snap(new_val.clamp(0.0, 1.0), *modifiers, |val| {
							(val * 127.0).round() / 127.0
						});
						if val != new_val {
							state.status = Status::DraggingVelocity(note, new_val);
							shell.publish((self.action)(Action::DragVelocity(new_val)));
							shell.capture_event();
						}
					}
				}
				Status::Deleting => {
					if !state.primary.is_empty() {
						shell.publish((self.action)(Action::Delete));
						shell.capture_event();
					}
				}
				Status::None => {}
			},
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
		viewport: &Rectangle,
	) {
		let state = self.state.borrow();

		for key in (0..127).map(MidiKey) {
			let Some(bounds) = Rectangle::new(
				viewport.position() + Vector::new(0.0, key_to_px(key, state.position, state.scale)),
				Size::new(viewport.width, 1.0),
			)
			.intersection(viewport) else {
				continue;
			};

			renderer.fill_quad(
				Quad {
					bounds,
					..Quad::default()
				},
				theme.palette().background.strong.color,
			);
		}

		self.notes
			.iter()
			.zip(&tree.children)
			.zip(layout.children())
			.for_each(|((child, tree), layout)| {
				renderer.with_layer(Rectangle::INFINITE, |renderer| {
					child.draw(tree, renderer, theme, style, layout, cursor, viewport);
				});
			});

		if let Status::Selecting(start_key, end_key, start_pos, end_pos) = state.status
			&& start_pos != end_pos
		{
			let (start_key, end_key) = (start_key.max(end_key), start_key.min(end_key));
			let (start_pos, end_pos) = (start_pos.min(end_pos), start_pos.max(end_pos));

			let y = key_to_px(start_key, state.position, state.scale);
			let height = key_to_px(end_key, state.position, state.scale) + state.scale.y - y;
			let y = y + viewport.y;

			let x = time_to_px(start_pos, state.position, state.scale, self.transport);
			let width = time_to_px(end_pos, state.position, state.scale, self.transport) - x;
			let x = x + viewport.x;

			renderer.with_layer(*viewport, |renderer| {
				renderer.fill_quad(
					Quad {
						bounds: Rectangle {
							x,
							y,
							width,
							height,
						},
						border: border::width(1).color(theme.palette().danger.weak.color),
						..Quad::default()
					},
					theme.palette().danger.weak.color.scale_alpha(ALPHA_1_3),
				);
			});
		}
	}

	fn mouse_interaction(
		&self,
		tree: &Tree,
		layout: Layout<'_>,
		cursor: Cursor,
		viewport: &Rectangle,
		renderer: &Renderer,
	) -> Interaction {
		match self.state.borrow().status {
			Status::Selecting(..) => Interaction::Idle,
			Status::Dragging(..) => Interaction::Grabbing,
			Status::TrimmingStart(..) | Status::TrimmingEnd(..) | Status::DraggingSplit(..) => {
				Interaction::ResizingHorizontally
			}
			Status::DraggingVelocity(..) => Interaction::Pointer,
			Status::Deleting => Interaction::NoDrop,
			Status::None => self
				.notes
				.iter()
				.zip(&tree.children)
				.zip(layout.children())
				.map(|((child, tree), layout)| {
					child.mouse_interaction(tree, layout, cursor, viewport, renderer)
				})
				.rfind(|&i| i != Interaction::default())
				.unwrap_or_default(),
		}
	}

	fn overlay<'b>(
		&'b mut self,
		tree: &'b mut Tree,
		layout: Layout<'b>,
		renderer: &Renderer,
		viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
		let children = self
			.notes
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.filter_map(|((child, tree), layout)| {
				child.overlay(tree, layout, renderer, viewport, translation)
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
			self.notes
				.iter_mut()
				.zip(&mut tree.children)
				.zip(layout.children())
				.for_each(|((child, tree), layout)| {
					child.operate(tree, layout, renderer, operation);
				});
		});
	}
}

impl<'a, Message> PianoRoll<'a, Message> {
	pub fn new(
		state: &'a RefCell<State>,
		transport: &'a Transport,
		notes: impl IntoIterator<Item = Note<'a, Message>>,
		action: fn(Action) -> Message,
	) -> Self {
		Self {
			state,
			notes: notes.into_iter().collect(),
			transport,
			action,
		}
	}
}

impl<'a, Message: 'a> From<PianoRoll<'a, Message>> for Element<'a, Message> {
	fn from(value: PianoRoll<'a, Message>) -> Self {
		Self::new(value)
	}
}
