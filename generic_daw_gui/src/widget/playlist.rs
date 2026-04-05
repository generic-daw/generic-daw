use crate::{
	arrangement_view::{AudioClipRef, MidiClipRef},
	file_tree::FileKind,
	widget::{ALPHA_1_3, Delta, clip, maybe_snap, px_to_time, snap_step, time_to_px, track::Track},
};
use generic_daw_core::{MusicalTime, Transport};
use iced::{
	Color, Element, Event, Fill, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Renderer as _, Shell,
		layout::{Layout, Limits, Node},
		mouse::{self, Cursor, Interaction},
		overlay,
		renderer::{Quad, Style},
		widget::{Operation, Tree, Widget},
	},
	border,
	gradient::Linear,
	keyboard, window,
};
use std::{
	cell::RefCell,
	collections::HashSet,
	f32::consts::{FRAC_PI_2, PI},
	path::Path,
	sync::Arc,
	time::Instant,
};

#[derive(Clone, Debug)]
pub enum Action {
	Pan(Vector, f32),
	Zoom(Vector, Point, f32),
	Add(Option<(Arc<Path>, FileKind)>, Option<usize>, MusicalTime),
	Open(usize, usize),
	Clone,
	Drag(Delta<usize>, Delta<MusicalTime>),
	TrimStart(Delta<MusicalTime>),
	TrimEnd(Delta<MusicalTime>),
	SplitAt(MusicalTime),
	DragSplit(MusicalTime),
	Delete,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Status {
	Hovering(Arc<Path>, FileKind, Option<(Option<usize>, MusicalTime)>),
	Selecting(usize, usize, MusicalTime, MusicalTime),
	Dragging(usize, MusicalTime),
	TrimmingStart(MusicalTime),
	TrimmingEnd(MusicalTime),
	DraggingSplit(MusicalTime),
	Deleting,
	#[default]
	None,
}

#[derive(Debug, Default)]
pub struct State {
	pub status: Status,
	pub primary: HashSet<(usize, usize)>,
	pub secondary: HashSet<(usize, usize)>,
	pub position: Vector,
	pub scale: Vector,
	autoscroll_start: Option<Instant>,
	last_autoscroll: Option<Instant>,
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
pub struct Playlist<'a, Message> {
	state: &'a RefCell<State>,
	transport: &'a Transport,
	tracks: Box<[Track<'a, Message>]>,
	action: fn(Action) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for Playlist<'_, Message>
where
	Message: Clone,
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
		let mut height = 0.0;

		let children = self
			.tracks
			.iter_mut()
			.zip(&mut tree.children)
			.map(|(child, tree)| {
				let node = child
					.layout(tree, renderer, &limits.height(self.state.borrow().scale.y))
					.translate(Vector::new(0.0, height));
				height += node.bounds().height;
				node
			})
			.collect();

		Node::with_children(Size::new(limits.max().width, height), children)
	}

	fn update(
		&mut self,
		tree: &mut Tree,
		event: &Event,
		layout: Layout<'_>,
		mut cursor: Cursor,
		renderer: &Renderer,
		shell: &mut Shell<'_, Message>,
		viewport: &Rectangle,
	) {
		self.tracks
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.for_each(|((child, tree), layout)| {
				child.update(tree, event, layout, cursor, renderer, shell, viewport);
			});

		if shell.is_event_captured() {
			return;
		}

		let state = &mut *self.state.borrow_mut();

		cursor = cursor.land();

		let cursor = 'block: {
			if let Some(cursor) = cursor.position_in(*viewport) {
				break 'block cursor;
			}

			if state.status == Status::None {
				return;
			}

			let Some(cursor) = cursor.position_from(viewport.position()) else {
				state.finish();
				shell.request_redraw();
				return;
			};

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
				let visible = layout.position().y + layout.bounds().height - viewport.y;

				let autoscroll_amt = (now - autoscroll_start).as_secs_f32().sqrt();

				let delta = Vector::new(
					if cursor.x == clamped.x {
						0.0
					} else {
						20.0 * autoscroll_amt.copysign(cursor.x - clamped.x)
					},
					if cursor.y == clamped.y {
						0.0
					} else {
						10.0 * autoscroll_amt.copysign(cursor.y - clamped.y)
					},
				);

				shell.publish((self.action)(Action::Pan(delta, visible)));

				state.last_autoscroll = Some(now);
			} else {
				state.autoscroll_start = Some(now);
			}

			clamped
		};

		let new_time = px_to_time(cursor.x, state.position, state.scale, self.transport);

		match event {
			Event::Mouse(mouse::Event::ButtonPressed { button, modifiers }) => match button {
				mouse::Button::Left => {
					let time = maybe_snap(new_time, *modifiers, |time| {
						time.round(snap_step(state.scale.x, self.transport))
					});
					let track = track_idx(&layout, *viewport, cursor);

					if modifiers.command() {
						let Some(track) = track.or_else(|| layout.children().len().checked_sub(1))
						else {
							return;
						};
						state.status = Status::Selecting(track, track, time, time);
						shell.capture_event();
						shell.request_redraw();
					} else if let Some(track) = track {
						state.primary.clear();
						state.status = Status::Dragging(track, time);
						shell.publish((self.action)(Action::Add(None, Some(track), time)));
					} else {
						state.primary.clear();
						shell.capture_event();
						shell.request_redraw();
					}
				}
				mouse::Button::Right => {
					state.primary.clear();
					state.status = Status::Deleting;
				}
				_ => {}
			},
			Event::Mouse(mouse::Event::ButtonReleased { .. }) if state.status != Status::None => {
				if let Status::Hovering(path, kind, Some((track, time))) = state.status.clone() {
					shell.publish((self.action)(Action::Add(Some((path, kind)), track, time)));
				}

				state.finish();
				shell.capture_event();
				shell.request_redraw();
			}
			Event::Mouse(mouse::Event::CursorMoved { modifiers, .. })
			| Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => match state.status.clone() {
				Status::Hovering(path, kind, time) => {
					let track = track_idx(&layout, *viewport, cursor);

					let new_time = maybe_snap(new_time, *modifiers, |time| {
						time.floor(snap_step(state.scale.x, self.transport))
					});

					let new_time = Some((track, new_time));

					if time != new_time {
						state.status = Status::Hovering(path, kind, new_time);
						shell.capture_event();
						shell.request_redraw();
					}
				}
				Status::Selecting(start_track, last_end_track, start_pos, last_end_pos) => {
					let Some(end_track) = track_idx(&layout, *viewport, cursor)
						.or_else(|| layout.children().len().checked_sub(1))
					else {
						return;
					};

					let end_pos = maybe_snap(new_time, *modifiers, |time| {
						time.round(snap_step(state.scale.x, self.transport))
					});

					if end_track == last_end_track && end_pos == last_end_pos {
						return;
					}

					state.status = Status::Selecting(start_track, end_track, start_pos, end_pos);

					let (start_track, end_track) =
						(start_track.min(end_track), start_track.max(end_track));
					let (start_pos, end_pos) = (start_pos.min(end_pos), start_pos.max(end_pos));

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
								clip::Inner::AudioClip(AudioClipRef { clip, .. }) => clip.position,
								clip::Inner::MidiClip(MidiClipRef { clip, .. }) => clip.position,
								clip::Inner::Recording(..) => return,
							};

							if (start_track..=end_track).contains(&idx.0)
								&& (start_pos.max(clip_pos.start()) < end_pos.min(clip_pos.end()))
							{
								state.secondary.insert(idx);
							} else {
								state.secondary.remove(&idx);
							}
						});

					shell.request_redraw();
				}
				Status::Dragging(track, time) => {
					let Some(new_track) = track_idx(&layout, *viewport, cursor)
						.or_else(|| layout.children().len().checked_sub(1))
					else {
						return;
					};

					let abs_diff = maybe_snap(new_time.abs_diff(time), *modifiers, |abs_diff| {
						abs_diff.round(snap_step(state.scale.x, self.transport))
					});

					if new_track != track || abs_diff != MusicalTime::ZERO {
						let track_delta = if new_track > track {
							Delta::Positive
						} else {
							Delta::Negative
						}(new_track.abs_diff(track));

						let time_delta = if new_time > time {
							Delta::Positive
						} else {
							Delta::Negative
						}(abs_diff);

						state.status = Status::Dragging(new_track, time + time_delta);
						shell.publish((self.action)(Action::Drag(track_delta, time_delta)));
						shell.capture_event();
					}
				}
				Status::TrimmingStart(time) => {
					let abs_diff = maybe_snap(new_time.abs_diff(time), *modifiers, |abs_diff| {
						abs_diff.round(snap_step(state.scale.x, self.transport))
					});

					if abs_diff != MusicalTime::ZERO {
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
						abs_diff.round(snap_step(state.scale.x, self.transport))
					});

					if abs_diff != MusicalTime::ZERO {
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
					let new_time = maybe_snap(new_time, *modifiers, |time| {
						time.round(snap_step(state.scale.x, self.transport))
					});

					if new_time != time {
						state.status = Status::DraggingSplit(new_time);
						shell.publish((self.action)(Action::DragSplit(new_time)));
						shell.capture_event();
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
		let state = &*self.state.borrow();

		for layout in layout.children() {
			let Some(bounds) = Rectangle::new(
				layout.position() + Vector::new(0.0, layout.bounds().height - 1.0),
				Size::new(layout.bounds().width, 1.0),
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

		if let Status::Hovering(_, _, Some((track, time))) = state.status {
			if let Some(track) = track {
				let bounds = layout.child(track).bounds();
				renderer.fill_quad(
					Quad {
						bounds: Rectangle::new(
							bounds.position()
								+ Vector::new(
									time_to_px(time, state.position, state.scale, self.transport),
									0.0,
								),
							Size::new(50.0, bounds.height),
						),
						..Quad::default()
					},
					Linear::new(FRAC_PI_2)
						.add_stop(0.0, theme.palette().background.strong.color)
						.add_stop(1.0, Color::TRANSPARENT),
				);
			} else {
				renderer.fill_quad(
					Quad {
						bounds: Rectangle::new(
							layout.children().next_back().map_or_else(
								|| layout.position(),
								|layout| {
									layout.position() + Vector::new(0.0, layout.bounds().height)
								},
							) + Vector::new(state.position.x, 0.0),
							Size::new(viewport.width, 50.0),
						),
						..Quad::default()
					},
					Linear::new(PI)
						.add_stop(0.0, theme.palette().background.strong.color)
						.add_stop(1.0, Color::TRANSPARENT),
				);
			}
		}

		let active = &mut Vec::new();

		let allocs = layout
			.children()
			.map(|layout| Track::<Message>::alloc_layers(active, layout, viewport))
			.collect::<Vec<_>>();

		for i in 0..allocs.iter().map(Vec::len).max().unwrap_or_default() {
			renderer.with_layer(Rectangle::INFINITE, |renderer| {
				self.tracks
					.iter()
					.zip(&tree.children)
					.zip(layout.children())
					.zip(&allocs)
					.filter(|(_, alloc)| i < alloc.len())
					.for_each(|(((child, tree), layout), alloc)| {
						alloc[i]
							.iter()
							.map(|&i| ((&child.clips[i], &tree.children[i]), layout.child(i)))
							.for_each(|((child, tree), layout)| {
								child.draw(tree, renderer, theme, style, layout, cursor, viewport);
							});
					});
			});
		}

		if let Status::Selecting(start_track, end_track, start_pos, end_pos) = state.status
			&& start_pos != end_pos
		{
			let (start_track, end_track) = (start_track.min(end_track), start_track.max(end_track));
			let (start_pos, end_pos) = (start_pos.min(end_pos), start_pos.max(end_pos));

			let y = layout.child(start_track).position().y;
			let height =
				layout.child(end_track).position().y + layout.child(end_track).bounds().height - y;

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
			Status::Hovering(..) => Interaction::Copy,
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
			.tracks
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
		state: &'a RefCell<State>,
		transport: &'a Transport,
		tracks: impl IntoIterator<Item = Track<'a, Message>>,
		action: fn(Action) -> Message,
	) -> Self {
		Self {
			state,
			transport,
			tracks: tracks.into_iter().collect(),
			action,
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
