use super::{Vec2, get_time};
use generic_daw_core::{MusicalTime, RtState};
use generic_daw_utils::NoDebug;
use iced::{
	Element, Event, Fill, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Clipboard, Shell,
		layout::{Layout, Limits, Node},
		renderer::Style,
		widget::{Operation, Tree, Widget, tree},
	},
	mouse::{self, Cursor, Interaction},
	overlay, window,
};

#[derive(Clone, Copy, Debug)]
pub enum Action {
	Grab(usize, usize),
	Drop,
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

impl State {
	fn drop(&mut self) -> bool {
		let unselect = matches!(
			self,
			Self::DraggingClip(..)
				| Self::DraggingSplit(..)
				| Self::ClipTrimmingStart(..)
				| Self::ClipTrimmingEnd(..)
		);

		*self = Self::None;

		unselect
	}
}

#[derive(Debug)]
pub struct Arrangement<'a, Message> {
	rtstate: &'a RtState,
	position: &'a Vec2,
	scale: &'a Vec2,
	children: NoDebug<Element<'a, Message>>,
	deleted: bool,
	f: fn(Action) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for Arrangement<'_, Message>
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
		Size::new(Fill, Fill)
	}

	fn diff(&self, tree: &mut Tree) {
		tree.diff_children(&[&*self.children]);
	}

	fn children(&self) -> Vec<Tree> {
		vec![Tree::new(&*self.children)]
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		Node::with_children(
			limits.max(),
			vec![
				self.children
					.as_widget_mut()
					.layout(&mut tree.children[0], renderer, limits),
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
		viewport: &Rectangle,
	) {
		let Some(viewport) = layout.bounds().intersection(viewport) else {
			return;
		};

		self.children.as_widget_mut().update(
			&mut tree.children[0],
			event,
			layout.children().next().unwrap(),
			cursor,
			renderer,
			clipboard,
			shell,
			&viewport,
		);

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
				if state.drop() {
					shell.publish((self.f)(Action::Drop));
				}

				return;
			};

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

						if let Some(track) = track_idx(&layout, viewport, cursor)
							&& let Some(clip) = clip_idx(&layout, viewport, cursor, track)
						{
							let clip_bounds = clip_layout(&layout, track, clip).unwrap().bounds()
								- Vector::new(viewport.x, viewport.y);

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
									*state = match (
										cursor.x - start_pixel < 10.0,
										end_pixel - cursor.x < 10.0,
									) {
										(true, true)
											if cursor.x - start_pixel < end_pixel - cursor.x =>
										{
											State::ClipTrimmingStart(offset, time)
										}
										(true, false) => State::ClipTrimmingStart(offset, time),
										(_, true) => State::ClipTrimmingEnd(
											offset + end_pixel - start_pixel,
											time,
										),
										(false, false) => State::DraggingClip(offset, track, time),
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
				},
				mouse::Event::ButtonReleased { .. } if *state != State::None => {
					if state.drop() {
						shell.publish((self.f)(Action::Drop));
					}

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
			State::None => self.children.as_widget().mouse_interaction(
				&tree.children[0],
				layout.children().next().unwrap(),
				cursor,
				viewport,
				renderer,
			),
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

		self.children.as_widget().draw(
			&tree.children[0],
			renderer,
			theme,
			style,
			layout.children().next().unwrap(),
			cursor,
			&viewport,
		);
	}

	fn overlay<'a>(
		&'a mut self,
		tree: &'a mut Tree,
		layout: Layout<'a>,
		renderer: &Renderer,
		viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
		self.children.as_widget_mut().overlay(
			&mut tree.children[0],
			layout.children().next().unwrap(),
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
			self.children.as_widget_mut().operate(
				&mut tree.children[0],
				layout.children().next().unwrap(),
				renderer,
				operation,
			);
		});
	}
}

impl<'a, Message> Arrangement<'a, Message>
where
	Message: Clone + 'a,
{
	pub fn new(
		rtstate: &'a RtState,
		position: &'a Vec2,
		scale: &'a Vec2,
		children: impl Into<Element<'a, Message>>,
		f: fn(Action) -> Message,
	) -> Self {
		Self {
			rtstate,
			children: children.into().into(),
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
		.next()?
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
	layout.children().next()?.children().nth(track)
}

fn clip_layout<'a>(layout: &Layout<'a>, track: usize, clip: usize) -> Option<Layout<'a>> {
	track_layout(layout, track)?.children().nth(clip)
}
