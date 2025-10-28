use crate::widget::{get_time, track::Track};
use generic_daw_core::{MusicalTime, RtState};
use generic_daw_utils::Vec2;
use iced::{
	Alignment, Element, Event, Fill, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Clipboard, Renderer as _, Shell,
		layout::{self, Layout, Limits, Node},
		overlay,
		renderer::Style,
		widget::{Operation, Tree, Widget, tree},
	},
	mouse::{self, Cursor, Interaction},
	window,
};

#[derive(Clone, Copy, Debug)]
pub enum Action {
	Grab(usize, usize),
	Add(usize, MusicalTime),
	Clone(usize, usize),
	Drag(usize, MusicalTime),
	SplitAt(usize, usize, MusicalTime),
	DragSplit(MusicalTime),
	TrimStart(MusicalTime),
	TrimEnd(MusicalTime),
	Delete(usize, usize),
}

#[derive(Clone, Copy, PartialEq)]
enum State {
	None,
	DraggingClip(f32, usize, MusicalTime),
	DraggingSplit(MusicalTime),
	ClipTrimmingStart(f32, MusicalTime),
	ClipTrimmingEnd(f32, MusicalTime),
	DeletingClips,
}

#[derive(Debug)]
pub struct Arrangement<'a, Message> {
	rtstate: &'a RtState,
	position: &'a Vec2,
	scale: &'a Vec2,
	children: Box<[Track<'a, Message>]>,
	deleted: bool,
	f: fn(Action) -> Message,
}

impl<'a, Message> Widget<Message, Theme, Renderer> for Arrangement<'a, Message>
where
	Message: Clone + 'a,
{
	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State::None)
	}

	fn size(&self) -> Size<Length> {
		Size::new(Fill, Fill)
	}

	fn diff(&self, tree: &mut Tree) {
		tree.diff_children(&self.children);
	}

	fn children(&self) -> Vec<Tree> {
		self.children.iter().map(Tree::new).collect()
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
			&mut self.children,
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

		self.children
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.for_each(|((child, tree), layout)| {
				child.update(
					tree, event, layout, cursor, renderer, clipboard, shell, &viewport,
				);
			});

		if let Event::Window(window::Event::RedrawRequested(..)) = event {
			self.deleted = false;
			return;
		}

		if shell.is_event_captured() {
			return;
		}

		if let Event::Mouse(event) = event {
			let state = tree.state.downcast_mut::<State>();

			let Some(cursor) = cursor.position_in(viewport) else {
				*state = State::None;
				return;
			};

			match event {
				mouse::Event::ButtonPressed { button, modifiers } if *state == State::None => {
					match button {
						mouse::Button::Left => {
							let time = get_time(
								cursor.x,
								*modifiers,
								self.rtstate,
								*self.position,
								*self.scale,
							);

							if let Some(track) = track_idx(&layout, viewport, cursor)
								&& let Some(clip) = clip_idx(&layout, viewport, cursor, track)
							{
								let clip_bounds = clip_layout(&layout, track, clip)
									.unwrap()
									.bounds() - Vector::new(viewport.x, viewport.y);

								let start_pixel = clip_bounds.x;
								let end_pixel = clip_bounds.x + clip_bounds.width;
								let offset = start_pixel - cursor.x;

								match (modifiers.command(), modifiers.shift()) {
									(true, false) => {
										shell.publish((self.f)(Action::Clone(track, clip)));
										*state = State::DraggingClip(offset, track, time);
									}
									(false, true) => {
										shell.publish((self.f)(Action::SplitAt(track, clip, time)));
										*state = State::DraggingSplit(time);
									}
									_ => {
										shell.publish((self.f)(Action::Grab(track, clip)));
										let start_offset = cursor.x - start_pixel;
										let end_offset = end_pixel - cursor.x;
										let border = 10f32.min((end_pixel - start_pixel) / 3.0);
										*state = match (start_offset < border, end_offset < border)
										{
											(true, false) => State::ClipTrimmingStart(offset, time),
											(false, true) => State::ClipTrimmingEnd(
												offset + end_pixel - start_pixel,
												time,
											),
											(false, false) => {
												State::DraggingClip(offset, track, time)
											}
											(true, true) => unreachable!(),
										};
									}
								}

								shell.capture_event();
							} else if let Some(track) = track_idx(&layout, viewport, cursor) {
								shell.publish((self.f)(Action::Add(track, time)));
								*state = State::DraggingClip(0.0, track, time);
							}
						}
						mouse::Button::Right if !self.deleted => {
							*state = State::DeletingClips;

							if let Some(track) = track_idx(&layout, viewport, cursor)
								&& let Some(clip) = clip_idx(&layout, viewport, cursor, track)
							{
								self.deleted = true;

								shell.publish((self.f)(Action::Delete(track, clip)));
								shell.capture_event();
							}
						}
						_ => {}
					}
				}
				mouse::Event::ButtonReleased { .. } if *state != State::None => {
					*state = State::None;
					shell.capture_event();
				}
				mouse::Event::CursorMoved { modifiers, .. } => match *state {
					State::DraggingClip(offset, track, time) => {
						let new_track = track_idx(&layout, viewport, cursor).unwrap_or_else(|| {
							layout.children().next().unwrap().children().len() - 1
						});
						let new_start = get_time(
							cursor.x + offset,
							*modifiers,
							self.rtstate,
							*self.position,
							*self.scale,
						);
						if new_track != track || new_start != time {
							*state = State::DraggingClip(offset, new_track, new_start);

							shell.publish((self.f)(Action::Drag(new_track, new_start)));
							shell.capture_event();
						}
					}
					State::DraggingSplit(time) => {
						let new_time = get_time(
							cursor.x,
							*modifiers,
							self.rtstate,
							*self.position,
							*self.scale,
						);
						if new_time != time {
							*state = State::DraggingSplit(new_time);

							shell.publish((self.f)(Action::DragSplit(new_time)));
							shell.capture_event();
						}
					}
					State::ClipTrimmingStart(offset, time) => {
						let new_start = get_time(
							cursor.x + offset,
							*modifiers,
							self.rtstate,
							*self.position,
							*self.scale,
						);
						if new_start != time {
							*state = State::ClipTrimmingStart(offset, new_start);

							shell.publish((self.f)(Action::TrimStart(new_start)));
							shell.capture_event();
						}
					}
					State::ClipTrimmingEnd(offset, time) => {
						let new_end = get_time(
							cursor.x + offset,
							*modifiers,
							self.rtstate,
							*self.position,
							*self.scale,
						);
						if new_end != time {
							*state = State::ClipTrimmingEnd(offset, new_end);

							shell.publish((self.f)(Action::TrimEnd(new_end)));
							shell.capture_event();
						}
					}
					State::DeletingClips => {
						if !self.deleted
							&& let Some(track) = track_idx(&layout, viewport, cursor)
							&& let Some(clip) = clip_idx(&layout, viewport, cursor, track)
						{
							self.deleted = true;

							shell.publish((self.f)(Action::Delete(track, clip)));
							shell.capture_event();
						}
					}
					State::None => {}
				},
				_ => {}
			}
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
		match tree.state.downcast_ref::<State>() {
			State::ClipTrimmingStart(..)
			| State::ClipTrimmingEnd(..)
			| State::DraggingSplit(..) => Interaction::ResizingHorizontally,
			State::DraggingClip(..) => Interaction::Grabbing,
			State::DeletingClips => Interaction::NoDrop,
			State::None => self
				.children
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
		let mut starts = vec![Some(0); self.children.len()];

		loop {
			let mut done = true;

			renderer.with_layer(Rectangle::INFINITE, |renderer| {
				self.children
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
			&mut self.children,
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
				.for_each(|((child, tree), layout)| {
					child.operate(tree, layout, renderer, operation);
				});
		});
	}
}

impl<'a, Message> Arrangement<'a, Message>
where
	Message: 'a,
{
	pub fn new(
		rtstate: &'a RtState,
		position: &'a Vec2,
		scale: &'a Vec2,
		children: impl IntoIterator<Item = Track<'a, Message>>,
		f: fn(Action) -> Message,
	) -> Self {
		Self {
			rtstate,
			children: children.into_iter().collect(),
			position,
			scale,
			deleted: false,
			f,
		}
	}
}

impl<'a, Message> From<Arrangement<'a, Message>> for Element<'a, Message>
where
	Message: Clone + 'a,
{
	fn from(value: Arrangement<'a, Message>) -> Self {
		Self::new(value)
	}
}

fn track_idx(layout: &Layout<'_>, viewport: Rectangle, cursor: Point) -> Option<usize> {
	let offset = Vector::new(viewport.position().x, viewport.position().y);
	layout
		.children()
		.position(|child| child.bounds().contains(cursor + offset))
}

fn clip_idx(
	layout: &Layout<'_>,
	viewport: Rectangle,
	cursor: Point,
	track: usize,
) -> Option<usize> {
	let offset = Vector::new(viewport.position().x, viewport.position().y);
	track_layout(layout, track)?
		.children()
		.rposition(|child| child.bounds().contains(cursor + offset))
}

fn track_layout<'a>(layout: &Layout<'a>, track: usize) -> Option<Layout<'a>> {
	layout.children().nth(track)
}

fn clip_layout<'a>(layout: &Layout<'a>, track: usize, clip: usize) -> Option<Layout<'a>> {
	track_layout(layout, track)?.children().nth(clip)
}
