use crate::widget::{clip::Clip, get_time, maybe_snap_time, playlist::Action};
use generic_daw_core::Transport;
use generic_daw_utils::NoDebug;
use iced::{
	Event, Fill, Length, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Clipboard, Layout, Shell, Widget,
		layout::{Limits, Node},
		mouse::{self, Click, Cursor, Interaction, click::Kind},
		overlay,
		renderer::Style,
		widget::{Operation, Tree, tree},
	},
};
use std::borrow::{Borrow, BorrowMut};

#[derive(Default)]
struct State {
	last_click: Option<Click>,
}

#[derive(Debug)]
pub struct Track<'a, Message> {
	idx: usize,
	transport: &'a Transport,
	position: &'a Vector,
	scale: &'a Vector,
	pub(super) clips: NoDebug<Box<[Clip<'a, Message>]>>,
	f: fn(Action) -> Message,
}

impl<'a, Message> Widget<Message, Theme, Renderer> for Track<'a, Message>
where
	Message: Clone + 'a,
{
	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State::default())
	}

	fn diff(&self, tree: &mut Tree) {
		tree.diff_children(&self.clips);
	}

	fn children(&self) -> Vec<Tree> {
		self.clips.iter().map(Tree::new).collect()
	}

	fn size(&self) -> Size<Length> {
		Size::new(Fill, Length::Fixed(self.scale.y))
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		Node::with_children(
			Size::new(limits.max().width, self.scale.y),
			self.clips
				.iter_mut()
				.zip(&mut tree.children)
				.map(|(child, tree)| child.layout(tree, renderer, &limits.height(self.scale.y)))
				.map(|node| node.translate(Vector::new(-self.position.x, 0.0)))
				.collect(),
		)
	}

	fn mouse_interaction(
		&self,
		tree: &Tree,
		layout: Layout<'_>,
		cursor: Cursor,
		viewport: &Rectangle,
		renderer: &Renderer,
	) -> Interaction {
		self.clips
			.iter()
			.zip(&tree.children)
			.zip(layout.children())
			.rev()
			.map(|((child, tree), clip_layout)| {
				child.mouse_interaction(tree, clip_layout, cursor, viewport, renderer)
			})
			.find(|&i| i != Interaction::default())
			.unwrap_or_default()
	}

	fn draw(
		&self,
		_tree: &Tree,
		_renderer: &mut Renderer,
		_theme: &Theme,
		_style: &Style,
		_layout: Layout<'_>,
		_cursor: Cursor,
		_viewport: &Rectangle,
	) {
		panic!()
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
		self.clips
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.rev()
			.for_each(|((child, state), layout)| {
				child.update(
					state, event, layout, cursor, renderer, clipboard, shell, viewport,
				);
			});

		if shell.is_event_captured() {
			return;
		}

		let track_bounds = layout.bounds() - Vector::new(viewport.x, viewport.y);

		if let Event::Mouse(mouse::Event::ButtonPressed {
			button: mouse::Button::Left,
			modifiers,
		}) = event && let Some(cursor) = cursor.position_in(*viewport)
			&& track_bounds.contains(cursor)
		{
			let state = tree.state.downcast_mut::<State>();

			let new_click = Click::new(cursor, mouse::Button::Left, state.last_click);
			state.last_click = Some(new_click);

			if new_click.kind() == Kind::Double {
				let time = maybe_snap_time(
					get_time(cursor.x, *self.position, *self.scale, self.transport),
					*modifiers,
					|time| time.snap_round(self.scale.x, self.transport),
				);
				shell.publish((self.f)(Action::Add(self.idx, time)));
				shell.capture_event();
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
			&mut self.clips,
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
			self.clips
				.iter_mut()
				.zip(&mut tree.children)
				.zip(layout.children())
				.for_each(|((child, state), layout)| {
					child.operate(state, layout, renderer, operation);
				});
		});
	}
}

impl<'a, Message> Track<'a, Message> {
	pub fn new(
		idx: usize,
		transport: &'a Transport,
		position: &'a Vector,
		scale: &'a Vector,
		children: impl IntoIterator<Item = Clip<'a, Message>>,
		f: fn(Action) -> Message,
	) -> Self
	where
		Message: 'a,
	{
		Self {
			idx,
			transport,
			position,
			scale,
			clips: children.into_iter().collect::<Box<_>>().into(),
			f,
		}
	}

	pub(super) fn fill_layer(
		&self,
		start: usize,
		rects: &mut Vec<Rectangle>,
		tree: &Tree,
		renderer: &mut Renderer,
		theme: &Theme,
		style: &Style,
		layout: Layout<'_>,
		cursor: Cursor,
		viewport: &Rectangle,
	) -> Option<usize>
	where
		Message: Clone,
	{
		rects.clear();

		for (i, ((child, tree), layout)) in self
			.clips
			.iter()
			.zip(&tree.children)
			.zip(layout.children())
			.enumerate()
			.skip(start)
		{
			let Some(bounds) = layout.bounds().intersection(viewport) else {
				continue;
			};

			if rects.iter().any(|b| bounds.intersects(b)) {
				return Some(i);
			}

			rects.push(bounds);

			child.draw(tree, renderer, theme, style, layout, cursor, viewport);
		}

		None
	}
}

impl<'a, Message> Borrow<dyn Widget<Message, Theme, Renderer> + 'a> for Track<'a, Message>
where
	Message: Clone + 'a,
{
	fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
		self
	}
}

impl<'a, Message> Borrow<dyn Widget<Message, Theme, Renderer> + 'a> for &Track<'a, Message>
where
	Message: Clone + 'a,
{
	fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
		*self
	}
}

impl<'a, Message> BorrowMut<dyn Widget<Message, Theme, Renderer> + 'a> for Track<'a, Message>
where
	Message: Clone + 'a,
{
	fn borrow_mut(&mut self) -> &mut (dyn Widget<Message, Theme, Renderer> + 'a) {
		self
	}
}
