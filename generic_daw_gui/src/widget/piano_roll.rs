use crate::widget::{
	ALPHA_1_3, Delta, key_to_px, maybe_snap, note::Note, px_to_key, px_to_time, time_to_px,
};
use generic_daw_core::{MidiKey, MusicalTime, Transport};
use iced::{
	Element, Event, Fill, Length, Rectangle, Renderer, Size, Theme, Vector,
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
	Add(MidiKey, MusicalTime),
	Clone,
	Drag(Delta<MidiKey>, Delta<MusicalTime>),
	TrimStart(Delta<MusicalTime>),
	TrimEnd(Delta<MusicalTime>),
	SplitAt(MusicalTime),
	DragSplit(MusicalTime),
	DragVelocity(f32),
	Delete,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Status {
	Selecting(MidiKey, MidiKey, MusicalTime, MusicalTime),
	Dragging(MidiKey, MusicalTime),
	TrimmingStart(MusicalTime),
	TrimmingEnd(MusicalTime),
	DraggingSplit(MusicalTime),
	DraggingVelocity(usize, f32),
	Deleting,
	#[default]
	None,
}

#[derive(Debug, Default)]
pub struct Selection {
	pub status: Status,
	pub primary: HashSet<usize>,
	pub secondary: HashSet<usize>,
}

impl Selection {
	pub fn clear(&mut self) {
		self.status = Status::None;
		self.primary.clear();
		self.secondary.clear();
	}
}

#[derive(Debug)]
pub struct PianoRoll<'a, Message> {
	selection: &'a RefCell<Selection>,
	transport: &'a Transport,
	position: &'a Vector,
	scale: &'a Vector,
	notes: Box<[Note<'a, Message>]>,
	f: fn(Action) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for PianoRoll<'_, Message> {
	fn diff(&self, tree: &mut Tree) {
		tree.diff_children(&self.notes);
	}

	fn children(&self) -> Vec<Tree> {
		self.notes.iter().map(Tree::new).collect()
	}

	fn size(&self) -> Size<Length> {
		Size::new(Fill, Length::Fixed(128.0 * self.scale.y))
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		Node::with_children(
			limits.height(128.0 * self.scale.y).max(),
			self.notes
				.iter_mut()
				.zip(&mut tree.children)
				.map(|(child, tree)| child.layout(tree, renderer, limits))
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

		let selection = &mut *self.selection.borrow_mut();

		let Some(cursor) = cursor.position_in(*viewport) else {
			selection.status = Status::None;
			return;
		};

		let new_time = px_to_time(cursor.x, *self.position, *self.scale, self.transport);

		match event {
			Event::Mouse(mouse::Event::ButtonPressed { button, modifiers })
				if selection.status == Status::None =>
			{
				match button {
					mouse::Button::Left => {
						let time = maybe_snap(new_time, *modifiers, |time| {
							time.snap_round(self.scale.x, self.transport)
						});
						let key = px_to_key(cursor.y, *self.position, *self.scale);

						if modifiers.command() {
							selection.status = Status::Selecting(key, key, time, time);
							shell.capture_event();
							shell.request_redraw();
						} else {
							selection.primary.clear();
							selection.status = Status::Dragging(key, time);
							shell.publish((self.f)(Action::Add(key, time)));
						}
					}
					mouse::Button::Right => {
						selection.primary.clear();
						selection.status = Status::Deleting;
					}
					_ => {}
				}
			}
			Event::Mouse(mouse::Event::ButtonReleased { .. })
				if selection.status != Status::None =>
			{
				selection.status = Status::None;
				selection.primary.extend(&selection.secondary);
				selection.secondary.clear();
				shell.capture_event();
				shell.request_redraw();
			}
			Event::Mouse(mouse::Event::CursorMoved { modifiers, .. })
			| Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => match selection.status {
				Status::Selecting(start_key, last_end_key, start_pos, last_end_pos) => {
					let end_key = px_to_key(cursor.y, *self.position, *self.scale);

					let end_pos = maybe_snap(new_time, *modifiers, |time| {
						time.snap_round(self.scale.x, self.transport)
					});

					if end_key == last_end_key && end_pos == last_end_pos {
						return;
					}

					selection.status = Status::Selecting(start_key, end_key, start_pos, end_pos);

					let (start_key, end_key) = (start_key.min(end_key), start_key.max(end_key));
					let (start_pos, end_pos) = (start_pos.min(end_pos), start_pos.max(end_pos));

					self.notes.iter().enumerate().for_each(|(idx, note)| {
						if (start_key..=end_key).contains(&note.note.key)
							&& (start_pos.max(note.note.position.start())
								< end_pos.min(note.note.position.end()))
						{
							selection.secondary.insert(idx);
						} else {
							selection.secondary.remove(&idx);
						}
					});

					shell.request_redraw();
				}
				Status::Dragging(key, time) => {
					let new_key = px_to_key(cursor.y, *self.position, *self.scale);

					let abs_diff = maybe_snap(new_time.abs_diff(time), *modifiers, |abs_diff| {
						abs_diff.snap_round(self.scale.x, self.transport)
					});

					if new_key != key || abs_diff != MusicalTime::ZERO {
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

						selection.status = Status::Dragging(new_key, time + time_delta);
						shell.publish((self.f)(Action::Drag(key_delta, time_delta)));
						shell.capture_event();
					}
				}
				Status::TrimmingStart(time) => {
					let abs_diff = maybe_snap(new_time.abs_diff(time), *modifiers, |abs_diff| {
						abs_diff.snap_round(self.scale.x, self.transport)
					});

					if abs_diff != MusicalTime::ZERO {
						let delta = if new_time > time {
							Delta::Positive
						} else {
							Delta::Negative
						}(abs_diff);

						selection.status = Status::TrimmingStart(time + delta);
						shell.publish((self.f)(Action::TrimStart(delta)));
						shell.capture_event();
					}
				}
				Status::TrimmingEnd(time) => {
					let abs_diff = maybe_snap(new_time.abs_diff(time), *modifiers, |abs_diff| {
						abs_diff.snap_round(self.scale.x, self.transport)
					});

					if abs_diff != MusicalTime::ZERO {
						let delta = if new_time > time {
							Delta::Positive
						} else {
							Delta::Negative
						}(abs_diff);

						selection.status = Status::TrimmingEnd(time + delta);
						shell.publish((self.f)(Action::TrimEnd(delta)));
						shell.capture_event();
					}
				}
				Status::DraggingSplit(time) => {
					let new_time = maybe_snap(new_time, *modifiers, |time| {
						time.snap_round(self.scale.x, self.transport)
					});

					if new_time != time {
						selection.status = Status::DraggingSplit(new_time);
						shell.publish((self.f)(Action::DragSplit(new_time)));
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
							selection.status = Status::DraggingVelocity(note, new_val);
							shell.publish((self.f)(Action::DragVelocity(new_val)));
							shell.capture_event();
						}
					}
				}
				Status::Deleting => {
					if !selection.primary.is_empty() {
						shell.publish((self.f)(Action::Delete));
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
		for key in (0..127).map(MidiKey) {
			let Some(bounds) = Rectangle::new(
				viewport.position() + Vector::new(0.0, key_to_px(key, *self.position, *self.scale)),
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

		if let Status::Selecting(start_key, end_key, start_pos, end_pos) =
			self.selection.borrow().status
			&& start_pos != end_pos
		{
			let (start_key, end_key) = (start_key.max(end_key), start_key.min(end_key));
			let (start_pos, end_pos) = (start_pos.min(end_pos), start_pos.max(end_pos));

			let y = key_to_px(start_key, *self.position, *self.scale);
			let height = key_to_px(end_key, *self.position, *self.scale) + self.scale.y - y;
			let y = y + viewport.y;

			let x = time_to_px(start_pos, *self.position, *self.scale, self.transport);
			let width = time_to_px(end_pos, *self.position, *self.scale, self.transport) - x;
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
		match self.selection.borrow().status {
			Status::Selecting(..) => Interaction::Idle,
			Status::Dragging(..) => Interaction::Grabbing,
			Status::TrimmingStart(..)
			| Status::TrimmingEnd(..)
			| Status::DraggingSplit(..)
			| Status::DraggingVelocity(..) => Interaction::ResizingHorizontally,
			Status::Deleting => Interaction::NoDrop,
			Status::None => self
				.notes
				.iter()
				.zip(&tree.children)
				.zip(layout.children())
				.map(|((child, tree), layout)| {
					child.mouse_interaction(tree, layout, cursor, viewport, renderer)
				})
				.find(|&i| i != Interaction::default())
				.unwrap_or_default(),
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
		selection: &'a RefCell<Selection>,
		transport: &'a Transport,
		position: &'a Vector,
		scale: &'a Vector,
		notes: impl IntoIterator<Item = Note<'a, Message>>,
		f: fn(Action) -> Message,
	) -> Self {
		Self {
			selection,
			notes: notes.into_iter().collect(),
			transport,
			position,
			scale,
			f,
		}
	}
}

impl<'a, Message> From<PianoRoll<'a, Message>> for Element<'a, Message>
where
	Message: 'a,
{
	fn from(value: PianoRoll<'a, Message>) -> Self {
		Self::new(value)
	}
}
