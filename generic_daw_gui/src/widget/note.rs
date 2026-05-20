use crate::widget::{
	ALPHA_1_3, beats_snap_step, key_to_px, maybe_snap,
	piano_roll::{self, Action, Status},
	px_to_time, time_to_px,
};
use generic_daw_core::{MidiNote, Transport};
use iced::{
	Event, Length, Rectangle, Renderer, Shrink, Size, Theme, Vector,
	advanced::{
		Layout, Renderer as _, Shell, Text, Widget,
		layout::{Limits, Node},
		mouse::{self, Cursor, Interaction},
		renderer::{Quad, Style},
		text::{Alignment, Ellipsis, LineHeight, Renderer as _, Shaping, Wrapping},
		widget::Tree,
	},
	alignment::Vertical,
	border,
};
use std::{borrow::Borrow, cell::RefCell};

#[derive(Debug)]
#[expect(clippy::struct_field_names)]
pub struct Note<'a, Message> {
	idx: usize,
	pub(super) note: &'a MidiNote,
	piano_roll: &'a RefCell<piano_roll::State>,
	transport: &'a Transport,
	f: fn(Action) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for Note<'_, Message> {
	fn size(&self) -> Size<Length> {
		Size::new(Shrink, Shrink)
	}

	fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
		let piano_roll = self.piano_roll.borrow();

		let (start, end) = (self.note.position.start(), self.note.position.end());

		let start = time_to_px(start, piano_roll.position, piano_roll.scale, self.transport);
		let end = time_to_px(end, piano_roll.position, piano_roll.scale, self.transport);

		Node::new(Size::new(end - start, piano_roll.scale.y)).translate(Vector::new(
			start,
			key_to_px(self.note.key, piano_roll.position, piano_roll.scale) + piano_roll.position.y,
		))
	}

	fn update(
		&mut self,
		_tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		cursor: Cursor,
		_renderer: &Renderer,
		shell: &mut Shell<'_, Message>,
		viewport: &Rectangle,
	) {
		if shell.is_event_captured() {
			return;
		}

		let Some(cursor) = cursor.position_in(*viewport) else {
			return;
		};

		let note_bounds = layout.bounds() - Vector::new(viewport.x, viewport.y);
		if !note_bounds.contains(cursor) {
			return;
		}

		let piano_roll = &mut *self.piano_roll.borrow_mut();
		match event {
			Event::Mouse(mouse::Event::ButtonPressed { button, modifiers })
				if piano_roll.status == Status::None =>
			{
				let mut clear = piano_roll.primary.insert(self.idx);

				match button {
					mouse::Button::Left => {
						let time = px_to_time(
							cursor.x,
							piano_roll.position,
							piano_roll.scale,
							self.transport,
						);

						piano_roll.status = match (modifiers.command(), modifiers.shift()) {
							(false, false) => {
								let start_offset = cursor.x - note_bounds.x;
								let end_offset = note_bounds.width - start_offset;
								let border = 10f32.min(note_bounds.width / 3.0);
								match (start_offset < border, end_offset < border) {
									(false, false) => {
										let bounds =
											layout.bounds().intersection(viewport).unwrap()
												- Vector::new(viewport.x, viewport.y);
										let vel_pixel = bounds.x
											+ border + self.note.velocity
											* (bounds.width - 2.0 * border - 1.0);
										if (vel_pixel - cursor.x).abs() < border / 2.0 {
											Status::DraggingVelocity(self.idx, self.note.velocity)
										} else {
											Status::Dragging(self.note.key, time)
										}
									}
									(true, false) => Status::TrimmingStart(time),
									(false, true) => Status::TrimmingEnd(time),
									(true, true) => unreachable!(),
								}
							}
							(true, false) => {
								clear = false;
								let time = maybe_snap(time, *modifiers, |time| {
									time.round(beats_snap_step(piano_roll.scale, self.transport))
								});
								Status::Selecting(self.note.key, self.note.key, time, time)
							}
							(false, true) => {
								shell.publish((self.f)(Action::Clone));
								Status::Dragging(self.note.key, time)
							}
							(true, true) => {
								let time = maybe_snap(time, *modifiers, |time| {
									time.round(beats_snap_step(piano_roll.scale, self.transport))
								});
								shell.publish((self.f)(Action::SplitAt(time)));
								Status::DraggingSplit(time)
							}
						};

						shell.capture_event();
						shell.request_redraw();
					}
					mouse::Button::Right if piano_roll.status != Status::Deleting => {
						clear = true;
						piano_roll.status = Status::Deleting;
						shell.publish((self.f)(Action::Delete));
						shell.capture_event();
					}
					_ => {}
				}

				if clear {
					piano_roll.primary.clear();
					piano_roll.primary.insert(self.idx);
				}
			}
			Event::Mouse(mouse::Event::CursorMoved { .. })
				if piano_roll.status == Status::Deleting =>
			{
				piano_roll.primary.insert(self.idx);
			}
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
		let Some(bounds) = layout.bounds().intersection(viewport) else {
			return;
		};

		let piano_roll = self.piano_roll.borrow();

		let color =
			if piano_roll.primary.contains(&self.idx) || piano_roll.secondary.contains(&self.idx) {
				theme.palette().danger.weak.color
			} else {
				theme.palette().primary.weak.color
			};

		renderer.fill_quad(
			Quad {
				bounds,
				border: border::width(1).color(color),
				..Quad::default()
			},
			color.scale_alpha(self.note.velocity * ALPHA_1_3 + ALPHA_1_3),
		);

		let border = 10f32.min(bounds.width / 3.0);

		renderer.fill_quad(
			Quad {
				bounds: Rectangle::new(
					bounds.position()
						+ Vector::new(
							self.note.velocity * (bounds.width - 2.0 * border - 1.0) + border,
							0.0,
						),
					Size::new(1.0, bounds.height),
				),
				..Quad::default()
			},
			theme.palette().background.strong.text,
		);

		if bounds.width > 3.0 {
			let note_name = Text {
				content: self.note.key.to_string(),
				bounds: Size::new(f32::INFINITY, 0.0),
				size: renderer.default_size(),
				line_height: LineHeight::default(),
				font: renderer.default_font(),
				align_x: Alignment::Left,
				align_y: Vertical::Center,
				shaping: Shaping::Basic,
				wrapping: Wrapping::None,
				ellipsis: Ellipsis::None,
				hint_factor: renderer.scale_factor(),
			};

			renderer.fill_text(
				note_name,
				bounds.position()
					+ Vector::new(
						3.0,
						if bounds.y == viewport.y {
							bounds.height - piano_roll.scale.y / 2.0
						} else {
							piano_roll.scale.y / 2.0
						},
					),
				theme.palette().background.strong.text,
				bounds,
			);
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
		if !cursor.is_over(*viewport) {
			return Interaction::default();
		}

		let Some(cursor) = cursor.position_in(layout.bounds()) else {
			return Interaction::default();
		};

		let border = 10f32.min(layout.bounds().width / 3.0);
		match (cursor.x < border, layout.bounds().width - cursor.x < border) {
			(false, false) => {
				let bounds = layout.bounds().intersection(viewport).unwrap()
					- Vector::new(layout.position().x, layout.position().y);
				let vel_pixel =
					bounds.x + border + self.note.velocity * (bounds.width - 2.0 * border - 1.0);
				if (vel_pixel - cursor.x).abs() < border / 2.0 {
					Interaction::Pointer
				} else {
					Interaction::Grab
				}
			}
			(true, false) | (false, true) => Interaction::ResizingHorizontally,
			(true, true) => unreachable!(),
		}
	}
}

impl<'a, Message> Note<'a, Message> {
	pub fn new(
		idx: usize,
		note: &'a MidiNote,
		piano_roll: &'a RefCell<piano_roll::State>,
		transport: &'a Transport,
		f: fn(Action) -> Message,
	) -> Self {
		Self {
			idx,
			note,
			piano_roll,
			transport,
			f,
		}
	}
}

impl<'a, Message: 'a> Borrow<dyn Widget<Message, Theme, Renderer> + 'a> for Note<'a, Message> {
	fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
		self
	}
}

impl<'a, Message: 'a> Borrow<dyn Widget<Message, Theme, Renderer> + 'a> for &Note<'a, Message> {
	fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
		*self
	}
}
