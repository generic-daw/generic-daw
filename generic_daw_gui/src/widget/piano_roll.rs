use super::get_time;
use generic_daw_core::{MidiKey, MidiNote, MusicalTime, RtState};
use generic_daw_utils::Vec2;
use iced::{
	Element, Event, Fill, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Clipboard, Layout, Renderer as _, Shell, Text, Widget,
		layout::{Limits, Node},
		renderer::{Quad, Style},
		text::Renderer as _,
		widget::{Tree, tree},
	},
	alignment::Vertical,
	border,
	mouse::{self, Cursor, Interaction},
	widget::text::{Alignment, LineHeight, Shaping, Wrapping},
	window,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub enum Action {
	Grab(usize),
	Drop,
	Add(MidiKey, MusicalTime),
	Clone(usize),
	Drag(MidiKey, MusicalTime),
	TrimStart(MusicalTime),
	TrimEnd(MusicalTime),
	Delete(usize),
}

#[derive(Clone, Copy, PartialEq)]
enum State {
	None,
	DraggingNote(f32, MidiKey, MusicalTime),
	NoteTrimmingStart(f32, MusicalTime),
	NoteTrimmingEnd(f32, MusicalTime),
	DeletingNotes,
}

impl State {
	fn unselect(&mut self) -> bool {
		let unselect = matches!(
			self,
			Self::DraggingNote(..) | Self::NoteTrimmingStart(..) | Self::NoteTrimmingEnd(..)
		);

		*self = Self::None;

		unselect
	}
}

#[derive(Debug)]
pub struct PianoRoll<'a, Message> {
	notes: Arc<Vec<MidiNote>>,
	rtstate: &'a RtState,
	position: &'a Vec2,
	scale: &'a Vec2,
	deleted: bool,

	f: fn(Action) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for PianoRoll<'_, Message>
where
	Message: Clone,
{
	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State::None)
	}

	fn size(&self) -> Size<Length> {
		Size::new(Fill, Length::Fixed(128.0 * self.scale.y))
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
		Node::new(Size::new(limits.max().width, 128.0 * self.scale.y))
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
		viewport: &Rectangle,
	) {
		if let Event::Window(window::Event::RedrawRequested(..)) = event {
			self.deleted = false;
			return;
		}

		if shell.is_event_captured() {
			return;
		}

		let Some(bounds) = layout.bounds().intersection(viewport) else {
			return;
		};

		let state = tree.state.downcast_mut::<State>();

		let Some(cursor) = cursor.position_in(bounds) else {
			if state.unselect() {
				shell.publish((self.f)(Action::Drop));
			}

			return;
		};

		if let Event::Mouse(event) = event {
			match event {
				mouse::Event::ButtonPressed { button, modifiers } => match button {
					mouse::Button::Left => {
						let time = get_time(
							cursor.x,
							*modifiers,
							self.rtstate,
							*self.position,
							*self.scale,
						);

						if let Some(i) = self.get_note(cursor) {
							let note = &self.notes[i];
							let note_bounds = self.note_bounds(note);

							let start_pixel = note_bounds.x;
							let end_pixel = note_bounds.x + note_bounds.width;
							let offset = start_pixel - cursor.x;

							*state = match (
								cursor.x - start_pixel < 10.0,
								end_pixel - cursor.x < 10.0,
							) {
								(true, true) if cursor.x - start_pixel < end_pixel - cursor.x => {
									State::NoteTrimmingStart(offset, time)
								}
								(true, false) => State::NoteTrimmingStart(offset, time),
								(_, true) => {
									State::NoteTrimmingEnd(offset + end_pixel - start_pixel, time)
								}
								(false, false) => State::DraggingNote(offset, note.key, time),
							};

							shell.publish((self.f)(if modifiers.control() {
								Action::Clone(i)
							} else {
								Action::Grab(i)
							}));
						} else {
							let key = self.get_key(cursor);

							*state = State::DraggingNote(0.0, key, time);

							shell.publish((self.f)(Action::Add(key, time)));
						}

						shell.capture_event();
					}
					mouse::Button::Right if !self.deleted => {
						*state = State::DeletingNotes;

						if let Some(note) = self.get_note(cursor) {
							self.deleted = true;

							shell.publish((self.f)(Action::Delete(note)));
							shell.capture_event();
						}
					}
					_ => {}
				},
				mouse::Event::ButtonReleased(..) if *state != State::None => {
					if state.unselect() {
						shell.publish((self.f)(Action::Drop));
					}

					shell.capture_event();
				}
				mouse::Event::CursorMoved { modifiers, .. } => match *state {
					State::DraggingNote(offset, key, time) => {
						let new_key = self.get_key(cursor);

						let new_start = get_time(
							cursor.x + offset,
							*modifiers,
							self.rtstate,
							*self.position,
							*self.scale,
						);

						if new_key != key || new_start != time {
							*state = State::DraggingNote(offset, new_key, new_start);

							shell.publish((self.f)(Action::Drag(new_key, new_start)));
							shell.capture_event();
						}
					}
					State::NoteTrimmingStart(offset, time) => {
						let new_start = get_time(
							cursor.x + offset,
							*modifiers,
							self.rtstate,
							*self.position,
							*self.scale,
						);
						if new_start != time {
							*state = State::NoteTrimmingStart(offset, new_start);

							shell.publish((self.f)(Action::TrimStart(new_start)));
							shell.capture_event();
						}
					}
					State::NoteTrimmingEnd(offset, time) => {
						let new_end = get_time(
							cursor.x + offset,
							*modifiers,
							self.rtstate,
							*self.position,
							*self.scale,
						);
						if new_end != time {
							*state = State::NoteTrimmingEnd(offset, new_end);

							shell.publish((self.f)(Action::TrimEnd(new_end)));
							shell.capture_event();
						}
					}
					State::DeletingNotes => {
						if !self.deleted
							&& let Some(note) = self.get_note(cursor)
						{
							self.deleted = true;

							shell.publish((self.f)(Action::Delete(note)));
							shell.capture_event();
						}
					}
					State::None => {}
				},
				_ => {}
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
		let Some(bounds) = layout.bounds().intersection(viewport) else {
			return;
		};

		for note in self.notes.iter() {
			let viewport = self.note_bounds(note) + Vector::new(bounds.x, bounds.y);
			let Some(note_bounds) = viewport.intersection(&bounds) else {
				continue;
			};

			renderer.with_layer(note_bounds, |renderer| {
				Self::draw_note(note, renderer, theme, viewport.position(), note_bounds);
			});
		}
	}

	fn mouse_interaction(
		&self,
		tree: &Tree,
		layout: Layout<'_>,
		cursor: Cursor,
		viewport: &Rectangle,
		_renderer: &Renderer,
	) -> Interaction {
		match tree.state.downcast_ref::<State>() {
			State::NoteTrimmingStart(..) | State::NoteTrimmingEnd(..) => {
				Interaction::ResizingHorizontally
			}
			State::DraggingNote(..) => Interaction::Grabbing,
			State::DeletingNotes => Interaction::NoDrop,
			State::None => layout
				.bounds()
				.intersection(viewport)
				.and_then(|bounds| cursor.position_in(bounds))
				.and_then(|cursor| {
					self.notes
						.iter()
						.map(|note| self.note_bounds(note))
						.rfind(|note_bounds| note_bounds.contains(cursor))
						.map(|note_bounds| {
							let x = cursor.x - note_bounds.x;

							if x < 10.0 || note_bounds.width - x < 10.0 {
								Interaction::ResizingHorizontally
							} else {
								Interaction::Grab
							}
						})
				})
				.unwrap_or_default(),
		}
	}
}

impl<'a, Message> PianoRoll<'a, Message> {
	pub fn new(
		notes: Arc<Vec<MidiNote>>,
		rtstate: &'a RtState,
		position: &'a Vec2,
		scale: &'a Vec2,
		f: fn(Action) -> Message,
	) -> Self {
		Self {
			notes,
			rtstate,
			position,
			scale,
			deleted: false,
			f,
		}
	}

	fn get_key(&self, cursor: Point) -> MidiKey {
		let new_key = 128.0 - cursor.y / self.scale.y - self.position.y;
		MidiKey(new_key as u8)
	}

	fn get_note(&self, cursor: Point) -> Option<usize> {
		self.notes
			.iter()
			.rposition(|note| self.note_bounds(note).contains(cursor))
	}

	fn note_bounds(&self, note: &MidiNote) -> Rectangle {
		let sample_size = self.scale.x.exp2();

		let start = (note.start.to_samples_f(self.rtstate) - self.position.x) / sample_size;
		let end = (note.end.to_samples_f(self.rtstate) - self.position.x) / sample_size;

		Rectangle::new(
			Point::new(
				start,
				(127.0 - f32::from(note.key.0) - self.position.y) * self.scale.y,
			),
			Size::new(end - start, self.scale.y),
		)
	}

	fn draw_note(
		note: &MidiNote,
		renderer: &mut Renderer,
		theme: &Theme,
		point: Point,
		bounds: Rectangle,
	) {
		renderer.fill_quad(
			Quad {
				bounds,
				border: border::width(1).color(theme.extended_palette().background.strong.color),
				..Quad::default()
			},
			theme.extended_palette().primary.weak.color,
		);

		let note_name = Text {
			content: note.key.to_string(),
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
			point + Vector::new(3.0, 0.0),
			theme.extended_palette().primary.weak.text,
			bounds,
		);
	}
}

impl<'a, Message> From<PianoRoll<'a, Message>> for Element<'a, Message>
where
	Message: Clone + 'a,
{
	fn from(value: PianoRoll<'a, Message>) -> Self {
		Self::new(value)
	}
}
