use crate::widget::clip::Clip;
use iced::{
	Event, Fill, Length, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Clipboard, Layout, Shell, Widget,
		layout::{Limits, Node},
		mouse::{Cursor, Interaction},
		overlay,
		renderer::Style,
		widget::{Operation, Tree},
	},
};
use std::borrow::Borrow;

#[derive(Debug)]
pub struct Track<'a, Message> {
	scale: &'a Vector,
	pub(super) clips: Box<[Clip<'a, Message>]>,
}

impl<'a, Message> Widget<Message, Theme, Renderer> for Track<'a, Message>
where
	Message: Clone + 'a,
{
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
			limits.height(self.scale.y).max(),
			self.clips
				.iter_mut()
				.zip(&mut tree.children)
				.map(|(child, tree)| child.layout(tree, renderer, &limits.height(self.scale.y)))
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
		panic!();
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
			.for_each(|((child, tree), layout)| {
				child.update(
					tree, event, layout, cursor, renderer, clipboard, shell, viewport,
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
		let children = self
			.clips
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
			self.clips
				.iter_mut()
				.zip(&mut tree.children)
				.zip(layout.children())
				.for_each(|((child, tree), layout)| {
					child.operate(tree, layout, renderer, operation);
				});
		});
	}
}

impl<'a, Message> Track<'a, Message> {
	pub fn new(scale: &'a Vector, children: impl IntoIterator<Item = Clip<'a, Message>>) -> Self
	where
		Message: 'a,
	{
		Self {
			scale,
			clips: children.into_iter().collect(),
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
