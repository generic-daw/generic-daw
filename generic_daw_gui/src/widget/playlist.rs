use crate::{
	arrangement_view::{AudioClipRef, MidiClipRef},
	widget::{Delta, clip, get_time, get_unsnapped_time, track::Track},
};
use generic_daw_core::{MusicalTime, RtState};
use generic_daw_utils::Vec2;
use iced::{
	Alignment, Element, Event, Fill, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Clipboard, Renderer as _, Shell,
		layout::{self, Layout, Limits, Node},
		mouse::{self, Cursor, Interaction},
		overlay,
		renderer::{Quad, Style},
		widget::{Operation, Tree, Widget},
	},
	border, keyboard,
};
use std::{cell::RefCell, collections::HashSet};

#[derive(Clone, Copy, Debug)]
pub enum Action {
	Open,
	Clone,
	Drag(isize, Delta<MusicalTime>),
	TrimStart(Delta<MusicalTime>),
	TrimEnd(Delta<MusicalTime>),
	Delete,
	Add(usize, MusicalTime),
	SplitAt(MusicalTime),
	DragSplit(MusicalTime),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) enum Status {
	Selecting(usize, usize, MusicalTime, MusicalTime),
	Dragging(usize, MusicalTime),
	TrimmingStart(MusicalTime),
	TrimmingEnd(MusicalTime),
	Deleting,
	DraggingSplit(MusicalTime),
	#[default]
	None,
}

#[derive(Debug, Default)]
pub struct Selection {
	pub(super) status: Status,
	pub primary: HashSet<(usize, usize)>,
	pub secondary: HashSet<(usize, usize)>,
}

#[derive(Debug)]
pub struct Playlist<'a, Message> {
	selection: &'a RefCell<Selection>,
	rtstate: &'a RtState,
	position: &'a Vec2,
	scale: &'a Vec2,
	tracks: Box<[Track<'a, Message>]>,
	f: fn(Action) -> Message,
}

impl<'a, Message> Widget<Message, Theme, Renderer> for Playlist<'a, Message>
where
	Message: Clone + 'a,
{
	fn diff(&self, tree: &mut Tree) {
		tree.diff_children(&self.tracks);
	}

	fn children(&self) -> Vec<Tree> {
		self.tracks.iter().map(Tree::new).collect()
	}
	fn size(&self) -> Size<Length> {
		Size::new(Fill, Fill)
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		layout::flex::resolve(
			layout::flex::Axis::Vertical,
			renderer,
			limits,
			Fill,
			Fill,
			0.into(),
			0.0,
			Alignment::Start,
			&mut self.tracks,
			&mut tree.children,
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
		viewport: &Rectangle,
	) {
		let Some(viewport) = layout.bounds().intersection(viewport) else {
			return;
		};

		self.tracks
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.for_each(|((child, tree), layout)| {
				child.update(
					tree, event, layout, cursor, renderer, clipboard, shell, &viewport,
				);
			});

		if shell.is_event_captured() {
			return;
		}

		let selection = &mut *self.selection.borrow_mut();

		match event {
			Event::Mouse(event) => {
				let Some(cursor) = cursor.position_in(viewport) else {
					selection.status = Status::None;
					return;
				};

				match event {
					mouse::Event::ButtonPressed { button, modifiers } => match button {
						mouse::Button::Left => {
							if modifiers.command() {
								let Some(track) = track_idx(&layout, viewport, cursor)
									.or_else(|| layout.children().len().checked_sub(1))
								else {
									return;
								};

								let time = get_time(
									cursor.x,
									*self.position,
									*self.scale,
									self.rtstate,
									*modifiers,
								);

								selection.status = Status::Selecting(track, track, time, time);
								shell.capture_event();
								shell.request_redraw();
							} else if !selection.primary.is_empty() {
								selection.primary.clear();
								shell.capture_event();
								shell.request_redraw();
							}
						}
						mouse::Button::Right => {
							selection.primary.clear();
							selection.status = Status::Deleting;
						}
						_ => {}
					},
					mouse::Event::ButtonReleased { .. } if selection.status != Status::None => {
						selection.status = Status::None;
						selection.primary.extend(selection.secondary.drain());
						shell.capture_event();
						shell.request_redraw();
					}
					mouse::Event::CursorMoved { modifiers, .. } => match selection.status {
						Status::Selecting(start_track, last_end_track, start_pos, last_end_pos) => {
							let Some(end_track) = track_idx(&layout, viewport, cursor)
								.or_else(|| layout.children().len().checked_sub(1))
							else {
								return;
							};

							let end_pos = get_time(
								cursor.x,
								*self.position,
								*self.scale,
								self.rtstate,
								*modifiers,
							);

							if end_track == last_end_track && end_pos == last_end_pos {
								return;
							}

							selection.status =
								Status::Selecting(start_track, end_track, start_pos, end_pos);

							let (start_track, end_track) =
								(start_track.min(end_track), start_track.max(end_track));
							let (start_pos, end_pos) =
								(start_pos.min(end_pos), start_pos.max(end_pos));

							self.tracks
								.iter()
								.enumerate()
								.flat_map(|(t_idx, track)| {
									track
										.clips
										.iter()
										.enumerate()
										.map(move |(c_idx, clip)| ((t_idx, c_idx), clip))
								})
								.for_each(|(idx, clip)| {
									let clip_pos = match clip.inner {
										clip::Inner::AudioClip(AudioClipRef { clip, .. }) => {
											clip.position
										}
										clip::Inner::MidiClip(MidiClipRef { clip, .. }) => {
											clip.position
										}
										clip::Inner::Recording(..) => return,
									};

									if (start_track..=end_track).contains(&idx.0)
										&& (start_pos.max(clip_pos.start())
											< end_pos.min(clip_pos.end()))
									{
										selection.secondary.insert(idx);
									} else {
										selection.secondary.remove(&idx);
									}
								});

							shell.request_redraw();
						}
						Status::Dragging(track, time) => {
							let Some(new_track) = track_idx(&layout, viewport, cursor)
								.or_else(|| layout.children().len().checked_sub(1))
							else {
								return;
							};

							let new_time = get_unsnapped_time(
								cursor.x,
								*self.position,
								*self.scale,
								self.rtstate,
							);

							let mut abs_diff = new_time.abs_diff(time);
							if !modifiers.alt() {
								abs_diff = abs_diff.snap_round(self.scale.x, self.rtstate);
							}

							if new_track != track || abs_diff != MusicalTime::ZERO {
								let delta = if new_time > time {
									Delta::Positive
								} else {
									Delta::Negative
								}(abs_diff);

								selection.status = Status::Dragging(new_track, time + delta);
								shell.publish((self.f)(Action::Drag(
									new_track.cast_signed() - track.cast_signed(),
									delta,
								)));
								shell.capture_event();
							}
						}
						Status::DraggingSplit(time) => {
							let new_time = get_time(
								cursor.x,
								*self.position,
								*self.scale,
								self.rtstate,
								*modifiers,
							);

							if new_time != time {
								selection.status = Status::DraggingSplit(new_time);
								shell.publish((self.f)(Action::DragSplit(new_time)));
								shell.capture_event();
							}
						}
						Status::TrimmingStart(time) => {
							let new_time = get_unsnapped_time(
								cursor.x,
								*self.position,
								*self.scale,
								self.rtstate,
							);

							let mut abs_diff = new_time.abs_diff(time);
							if !modifiers.alt() {
								abs_diff = abs_diff.snap_round(self.scale.x, self.rtstate);
							}

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
							let new_time = get_unsnapped_time(
								cursor.x,
								*self.position,
								*self.scale,
								self.rtstate,
							);

							let mut abs_diff = new_time.abs_diff(time);
							if !modifiers.alt() {
								abs_diff = abs_diff.snap_round(self.scale.x, self.rtstate);
							}

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
			Event::Keyboard(keyboard::Event::KeyPressed {
				physical_key: keyboard::key::Physical::Code(code),
				..
			}) if selection.status == Status::None && !selection.primary.is_empty() => match code {
				keyboard::key::Code::Delete | keyboard::key::Code::Backspace => {
					shell.publish((self.f)(Action::Delete));
					shell.capture_event();
				}
				keyboard::key::Code::Escape => {
					selection.primary.clear();
					shell.capture_event();
					shell.request_redraw();
				}
				_ => {}
			},
			_ => {}
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
			Status::TrimmingStart(..) | Status::TrimmingEnd(..) | Status::DraggingSplit(..) => {
				Interaction::ResizingHorizontally
			}
			Status::Deleting => Interaction::NoDrop,
			Status::None => self
				.tracks
				.iter()
				.zip(&tree.children)
				.zip(layout.children())
				.map(|((child, tree), layout)| {
					child.mouse_interaction(tree, layout, cursor, viewport, renderer)
				})
				.max()
				.unwrap_or_default(),
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
		let Some(viewport) = layout.bounds().intersection(viewport) else {
			return;
		};

		let rects = &mut vec![];
		let mut starts = vec![Some(0); self.tracks.len()];

		loop {
			let mut done = true;

			renderer.with_layer(Rectangle::INFINITE, |renderer| {
				self.tracks
					.iter()
					.zip(&tree.children)
					.zip(layout.children())
					.zip(&mut starts)
					.for_each(|(((child, tree), layout), start)| {
						let Some(st) = *start else {
							return;
						};

						done = false;

						*start = child.fill_layer(
							st, rects, tree, renderer, theme, style, layout, cursor, &viewport,
						);
					});
			});

			if done {
				break;
			}
		}

		if let Status::Selecting(start_track, end_track, start_pos, end_pos) =
			self.selection.borrow().status
			&& start_pos != end_pos
		{
			let (start_track, end_track) = (start_track.min(end_track), start_track.max(end_track));
			let (start_pos, end_pos) = (start_pos.min(end_pos), start_pos.max(end_pos));
			renderer.with_layer(viewport, |renderer| {
				renderer.with_translation(Vector::new(viewport.x, 0.0), |renderer| {
					let samples_per_px = self.scale.x.exp2();

					let y = layout.child(start_track).position().y;
					let height = layout.child(end_track).position().y
						+ layout.child(end_track).bounds().height
						- y;

					let x = start_pos.to_samples_f(self.rtstate) / samples_per_px;
					let width = end_pos.to_samples_f(self.rtstate) / samples_per_px - x;
					let x = x - self.position.x / samples_per_px;

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

	fn overlay<'b>(
		&'b mut self,
		tree: &'b mut Tree,
		layout: Layout<'b>,
		renderer: &Renderer,
		viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
		overlay::from_children(
			&mut self.tracks,
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
			self.tracks
				.iter_mut()
				.zip(&mut tree.children)
				.zip(layout.children())
				.for_each(|((child, tree), layout)| {
					child.operate(tree, layout, renderer, operation);
				});
		});
	}
}

impl<'a, Message> Playlist<'a, Message>
where
	Message: 'a,
{
	pub fn new(
		selection: &'a RefCell<Selection>,
		rtstate: &'a RtState,
		position: &'a Vec2,
		scale: &'a Vec2,
		children: impl IntoIterator<Item = Track<'a, Message>>,
		f: fn(Action) -> Message,
	) -> Self {
		Self {
			selection,
			rtstate,
			tracks: children.into_iter().collect(),
			position,
			scale,
			f,
		}
	}
}

impl<'a, Message> From<Playlist<'a, Message>> for Element<'a, Message>
where
	Message: Clone + 'a,
{
	fn from(value: Playlist<'a, Message>) -> Self {
		Self::new(value)
	}
}

fn track_idx(layout: &Layout<'_>, viewport: Rectangle, cursor: Point) -> Option<usize> {
	let offset = Vector::new(viewport.position().x, viewport.position().y);
	layout
		.children()
		.position(|child| child.bounds().contains(cursor + offset))
}
