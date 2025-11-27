use crate::widget::{Delta, get_time, key_y, maybe_snap_time};
use bit_set::BitSet;
use generic_daw_core::{MidiKey, MidiNote, MusicalTime, RtState};
use iced::{
	Element, Event, Fill, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Clipboard, Layout, Renderer as _, Shell, Text, Widget,
		layout::{Limits, Node},
		mouse::{self, Cursor, Interaction},
		renderer::{Quad, Style},
		text::Renderer as _,
		widget::Tree,
	},
	alignment::Vertical,
	border, keyboard,
	widget::text::{Alignment, LineHeight, Shaping, Wrapping},
};
use std::cell::RefCell;

#[derive(Clone, Copy, Debug)]
pub enum Action {
	Clone,
	Drag(Delta<MidiKey>, Delta<MusicalTime>),
	TrimStart(Delta<MusicalTime>),
	TrimEnd(Delta<MusicalTime>),
	Delete,
	Add(MidiKey, MusicalTime),
	SplitAt(MusicalTime),
	DragSplit(MusicalTime),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Status {
	Selecting(MidiKey, MidiKey, MusicalTime, MusicalTime),
	Dragging(MidiKey, MusicalTime),
	TrimmingStart(MusicalTime),
	TrimmingEnd(MusicalTime),
	Deleting,
	DraggingSplit(MusicalTime),
	#[default]
	None,
}

#[derive(Debug, Default)]
pub struct Selection {
	pub status: Status,
	pub primary: BitSet,
	pub secondary: BitSet,
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
	notes: &'a [MidiNote],
	rtstate: &'a RtState,
	position: &'a Vector,
	scale: &'a Vector,
	f: fn(Action) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for PianoRoll<'_, Message> {
	fn size(&self) -> Size<Length> {
		Size::new(Fill, Length::Fixed(128.0 * self.scale.y))
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
		Node::new(Size::new(limits.max().width, 128.0 * self.scale.y))
	}

	fn update(
		&mut self,
		_tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		cursor: Cursor,
		_renderer: &Renderer,
		_clipboard: &mut dyn Clipboard,
		shell: &mut Shell<'_, Message>,
		viewport: &Rectangle,
	) {
		let Some(viewport) = layout.bounds().intersection(viewport) else {
			return;
		};

		for note in (0..self.notes.len()).rev() {
			self.update_note(note, event, cursor, shell, &viewport);
		}

		if shell.is_event_captured() {
			return;
		}

		let selection = &mut *self.selection.borrow_mut();
		let Some(cursor) = cursor.position_in(viewport) else {
			selection.status = Status::None;
			return;
		};

		match event {
			Event::Mouse(mouse::Event::ButtonPressed { button, modifiers })
				if selection.status == Status::None =>
			{
				match button {
					mouse::Button::Left => {
						let time = maybe_snap_time(
							get_time(cursor.x, *self.position, *self.scale, self.rtstate),
							*modifiers,
							|time| time.snap_round(self.scale.x, self.rtstate),
						);
						let key = self.get_key(cursor);

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
					let end_key = self.get_key(cursor);

					let end_pos = maybe_snap_time(
						get_time(cursor.x, *self.position, *self.scale, self.rtstate),
						*modifiers,
						|time| time.snap_round(self.scale.x, self.rtstate),
					);

					if end_key == last_end_key && end_pos == last_end_pos {
						return;
					}

					selection.status = Status::Selecting(start_key, end_key, start_pos, end_pos);

					let (start_key, end_key) = (start_key.min(end_key), start_key.max(end_key));
					let (start_pos, end_pos) = (start_pos.min(end_pos), start_pos.max(end_pos));

					self.notes.iter().enumerate().for_each(|(idx, note)| {
						if (start_key..=end_key).contains(&note.key)
							&& (start_pos.max(note.position.start())
								< end_pos.min(note.position.end()))
						{
							selection.secondary.insert(idx);
						} else {
							selection.secondary.remove(idx);
						}
					});

					shell.request_redraw();
				}
				Status::Dragging(key, time) => {
					let new_key = self.get_key(cursor);

					let new_time = get_time(cursor.x, *self.position, *self.scale, self.rtstate);

					let abs_diff =
						maybe_snap_time(new_time.abs_diff(time), *modifiers, |abs_diff| {
							abs_diff.snap_round(self.scale.x, self.rtstate)
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
				Status::DraggingSplit(time) => {
					let new_time = maybe_snap_time(
						get_time(cursor.x, *self.position, *self.scale, self.rtstate),
						*modifiers,
						|time| time.snap_round(self.scale.x, self.rtstate),
					);

					if new_time != time {
						selection.status = Status::DraggingSplit(new_time);
						shell.publish((self.f)(Action::DragSplit(new_time)));
						shell.capture_event();
					}
				}
				Status::TrimmingStart(time) => {
					let new_time = get_time(cursor.x, *self.position, *self.scale, self.rtstate);

					let abs_diff =
						maybe_snap_time(new_time.abs_diff(time), *modifiers, |abs_diff| {
							abs_diff.snap_round(self.scale.x, self.rtstate)
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
					let new_time = get_time(cursor.x, *self.position, *self.scale, self.rtstate);

					let abs_diff =
						maybe_snap_time(new_time.abs_diff(time), *modifiers, |abs_diff| {
							abs_diff.snap_round(self.scale.x, self.rtstate)
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
		_tree: &Tree,
		renderer: &mut Renderer,
		theme: &Theme,
		_style: &Style,
		layout: Layout<'_>,
		_cursor: Cursor,
		viewport: &Rectangle,
	) {
		let Some(viewport) = layout.bounds().intersection(viewport) else {
			return;
		};

		for key in (0..127).map(MidiKey) {
			renderer.fill_quad(
				Quad {
					bounds: Rectangle::new(
						viewport.position()
							+ Vector::new(0.0, key_y(key, *self.position, *self.scale)),
						Size::new(viewport.width, 1.0),
					),
					..Quad::default()
				},
				theme.extended_palette().background.strong.color,
			);
		}

		let mut rects = Vec::new();

		renderer.start_layer(Rectangle::INFINITE);

		for note in 0..self.notes.len() {
			let note_bounds =
				self.note_bounds(&self.notes[note]) + Vector::new(viewport.x, viewport.y);
			let Some(bounds) = note_bounds.intersection(&viewport) else {
				continue;
			};

			if rects.iter().any(|b| bounds.intersects(b)) {
				rects.clear();
				renderer.end_layer();
				renderer.start_layer(Rectangle::INFINITE);
			}

			rects.push(bounds);

			self.draw_note(note, renderer, theme, bounds);
		}

		renderer.end_layer();

		if let Status::Selecting(start_key, end_key, start_pos, end_pos) =
			self.selection.borrow().status
			&& start_pos != end_pos
		{
			let (start_key, end_key) = (start_key.max(end_key), start_key.min(end_key));
			let (start_pos, end_pos) = (start_pos.min(end_pos), start_pos.max(end_pos));
			renderer.with_layer(viewport, |renderer| {
				renderer.with_translation(Vector::new(viewport.x, viewport.y), |renderer| {
					let samples_per_px = self.scale.x.exp2();

					let y = key_y(start_key, *self.position, *self.scale);
					let height = key_y(end_key, *self.position, *self.scale) + self.scale.y - y;

					let x = start_pos.to_samples_f(self.rtstate) / samples_per_px;
					let width = end_pos.to_samples_f(self.rtstate) / samples_per_px - x;
					let x = x - self.position.x;

					renderer.fill_quad(
						Quad {
							bounds: Rectangle {
								x,
								y,
								width,
								height,
							},
							border: border::width(1)
								.color(theme.extended_palette().danger.weak.color),
							..Quad::default()
						},
						theme.extended_palette().danger.weak.color.scale_alpha(0.2),
					);
				});
			});
		}
	}

	fn mouse_interaction(
		&self,
		_tree: &Tree,
		layout: Layout<'_>,
		cursor: Cursor,
		viewport: &Rectangle,
		_renderer: &Renderer,
	) -> Interaction {
		match self.selection.borrow().status {
			Status::Selecting(..) => Interaction::Idle,
			Status::Dragging(..) => Interaction::Grabbing,
			Status::TrimmingStart(..) | Status::TrimmingEnd(..) | Status::DraggingSplit(..) => {
				Interaction::ResizingHorizontally
			}
			Status::Deleting => Interaction::NoDrop,
			Status::None => layout
				.bounds()
				.intersection(viewport)
				.and_then(|viewport| cursor.position_in(viewport))
				.and_then(|cursor| {
					self.notes
						.iter()
						.map(|note| self.note_bounds(note))
						.rfind(|note_bounds| note_bounds.contains(cursor))
						.map(|note_bounds| {
							let x = cursor.x - note_bounds.x;
							let border = 10f32.min(note_bounds.width / 3.0);
							match (x < border, note_bounds.width - x < border) {
								(false, false) => Interaction::Grab,
								(true, false) | (false, true) => Interaction::ResizingHorizontally,
								(true, true) => unreachable!(),
							}
						})
				})
				.unwrap_or_default(),
		}
	}
}

impl<'a, Message> PianoRoll<'a, Message> {
	pub fn new(
		selection: &'a RefCell<Selection>,
		notes: &'a [MidiNote],
		rtstate: &'a RtState,
		position: &'a Vector,
		scale: &'a Vector,
		f: fn(Action) -> Message,
	) -> Self {
		Self {
			selection,
			notes,
			rtstate,
			position,
			scale,
			f,
		}
	}

	fn get_key(&self, cursor: Point) -> MidiKey {
		let new_key = 127.0 - (cursor.y + self.position.y) / self.scale.y;
		MidiKey(new_key.ceil() as u8)
	}

	fn note_bounds(&self, note: &MidiNote) -> Rectangle {
		let samples_per_px = self.scale.x.exp2();

		let x = note.position.start().to_samples_f(self.rtstate) / samples_per_px;
		let width = note.position.end().to_samples_f(self.rtstate) / samples_per_px - x;
		let x = x - self.position.x;

		Rectangle::new(
			Point::new(x, key_y(note.key, *self.position, *self.scale)),
			Size::new(width, self.scale.y),
		)
	}

	fn update_note(
		&self,
		note: usize,
		event: &Event,
		cursor: Cursor,
		shell: &mut Shell<'_, Message>,
		viewport: &Rectangle,
	) {
		let Some(cursor) = cursor.position_in(*viewport) else {
			return;
		};

		let selection = &mut *self.selection.borrow_mut();
		let note_bounds = self.note_bounds(&self.notes[note]);

		if let Event::Mouse(event) = event
			&& note_bounds.contains(cursor)
		{
			match event {
				mouse::Event::ButtonPressed { button, modifiers }
					if selection.status == Status::None =>
				{
					let mut clear = selection.primary.insert(note);

					match button {
						mouse::Button::Left => {
							let key = self.notes[note].key;
							let time =
								get_time(cursor.x, *self.position, *self.scale, self.rtstate);

							selection.status = match (modifiers.command(), modifiers.shift()) {
								(false, false) => {
									let start_pixel = note_bounds.x;
									let end_pixel = note_bounds.x + note_bounds.width;
									let start_offset = cursor.x - start_pixel;
									let end_offset = end_pixel - cursor.x;
									let border = 10f32.min((end_pixel - start_pixel) / 3.0);
									match (start_offset < border, end_offset < border) {
										(true, false) => Status::TrimmingStart(time),
										(false, true) => Status::TrimmingEnd(time),
										(false, false) => Status::Dragging(key, time),
										(true, true) => unreachable!(),
									}
								}
								(true, false) => {
									clear = false;
									let time = maybe_snap_time(
										get_time(
											cursor.x,
											*self.position,
											*self.scale,
											self.rtstate,
										),
										*modifiers,
										|time| time.snap_round(self.scale.x, self.rtstate),
									);
									Status::Selecting(key, key, time, time)
								}
								(false, true) => {
									shell.publish((self.f)(Action::Clone));
									Status::Dragging(key, time)
								}
								(true, true) => {
									let time = maybe_snap_time(
										get_time(
											cursor.x,
											*self.position,
											*self.scale,
											self.rtstate,
										),
										*modifiers,
										|time| time.snap_round(self.scale.x, self.rtstate),
									);
									shell.publish((self.f)(Action::SplitAt(time)));
									Status::DraggingSplit(time)
								}
							};

							shell.capture_event();
							shell.request_redraw();
						}
						mouse::Button::Right if selection.status != Status::Deleting => {
							selection.status = Status::Deleting;
							shell.publish((self.f)(Action::Delete));
							shell.capture_event();
						}
						_ => {}
					}

					if clear {
						selection.primary.clear();
						selection.primary.insert(note);
					}
				}
				mouse::Event::CursorMoved { .. } if selection.status == Status::Deleting => {
					selection.primary.insert(note);
				}
				_ => {}
			}
		}
	}

	fn draw_note(&self, note: usize, renderer: &mut Renderer, theme: &Theme, bounds: Rectangle) {
		let selection = self.selection.borrow();

		let color = if selection.primary.contains(note) || selection.secondary.contains(note) {
			theme.extended_palette().danger.weak.color
		} else {
			theme.extended_palette().primary.weak.color
		};

		renderer.fill_quad(
			Quad {
				bounds,
				border: border::width(1).color(theme.extended_palette().background.strong.color),
				..Quad::default()
			},
			color,
		);

		let note_name = Text {
			content: self.notes[note].key.to_string(),
			bounds: Size::new(f32::INFINITY, 0.0),
			size: renderer.default_size(),
			line_height: LineHeight::default(),
			font: renderer.default_font(),
			align_x: Alignment::Left,
			align_y: Vertical::Top,
			shaping: Shaping::Basic,
			wrapping: Wrapping::None,
		};

		renderer.fill_text(
			note_name,
			bounds.position() + Vector::new(3.0, 0.0),
			theme.extended_palette().primary.weak.text,
			bounds,
		);
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
