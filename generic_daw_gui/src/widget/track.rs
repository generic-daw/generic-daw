use crate::widget::clip::Clip;
use generic_daw_utils::{NoDebug, Vec2};
use iced::{
	Event, Fill, Length, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Clipboard, Layout, Shell, Widget,
		layout::{Limits, Node},
		overlay,
		renderer::Style,
		widget::{Operation, Tree},
	},
	mouse::{Cursor, Interaction},
};
use std::borrow::{Borrow, BorrowMut};

#[derive(Debug)]
pub struct Track<'a, Message> {
	scale: &'a Vec2,
	children: NoDebug<Box<[Clip<'a, Message>]>>,
}

impl<'a, Message> Widget<Message, Theme, Renderer> for Track<'a, Message>
where
	Message: Clone + 'a,
{
	fn diff(&self, tree: &mut Tree) {
		tree.diff_children(&self.children);
	}

	fn size(&self) -> Size<Length> {
		Size::new(Fill, Length::Fixed(self.scale.y))
	}

	fn children(&self) -> Vec<Tree> {
		self.children.iter().map(Tree::new).collect()
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		Node::with_children(
			Size::new(limits.max().width, self.scale.y),
			self.children
				.iter_mut()
				.zip(&mut tree.children)
				.map(|(widget, tree)| widget.layout(tree, renderer, &limits.height(self.scale.y)))
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
		self.children
			.iter()
			.zip(&tree.children)
			.zip(layout.children())
			.map(|((child, tree), clip_layout)| {
				child.mouse_interaction(tree, clip_layout, cursor, viewport, renderer)
			})
			.rfind(|&i| i != Interaction::default())
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
		self.children
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.for_each(|((child, state), layout)| {
				child.update(
					state, event, layout, cursor, renderer, clipboard, shell, viewport,
				);
			});
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
				.for_each(|((child, state), layout)| {
					child.operate(state, layout, renderer, operation);
				});
		});
	}
}

impl<'a, Message> Track<'a, Message> {
	pub fn new(scale: &'a Vec2, children: impl IntoIterator<Item = Clip<'a, Message>>) -> Self
	where
		Message: 'a,
	{
		Self {
			scale,
			children: children.into_iter().collect::<Box<_>>().into(),
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
			.children
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
