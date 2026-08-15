use crate::widget::clip::Clip;
use iced::{
	Event, Fill, Length, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Layout, Shell, Widget,
		layout::{Limits, Node},
		mouse::{Cursor, Interaction},
		overlay,
		renderer::Style,
		widget::{Operation, Tree},
	},
};
use std::borrow::{Borrow, BorrowMut};

#[derive(Debug)]
pub struct Track<'a, Message> {
	pub(super) clips: Box<[Clip<'a, Message>]>,
}

impl<'a, Message: 'a> Widget<Message, Theme, Renderer> for Track<'a, Message> {
	fn diff(&mut self, tree: &mut Tree) {
		tree.diff_children(&mut self.clips);
	}

	fn size(&self) -> Size<Length> {
		Size::new(Fill, Fill)
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		Node::with_children(
			limits.max(),
			self.clips
				.iter_mut()
				.zip(&mut tree.children)
				.map(|(child, tree)| child.layout(tree, renderer, limits))
				.collect(),
		)
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
		for (i, ((child, tree), layout)) in self
			.clips
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.enumerate()
			.rev()
		{
			child.update(tree, event, layout, cursor, renderer, shell, viewport);

			if i != 0
				&& !cursor.is_levitating()
				&& child.mouse_interaction(tree, layout, cursor, viewport, renderer)
					!= Interaction::default()
			{
				cursor = cursor.levitate();
			}
		}
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
			.map(|((child, tree), clip_layout)| {
				child.mouse_interaction(tree, clip_layout, cursor, viewport, renderer)
			})
			.rfind(|&i| i != Interaction::default())
			.unwrap_or_default()
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
	pub fn new(children: impl IntoIterator<Item = Clip<'a, Message>>) -> Self {
		Self {
			clips: children.into_iter().collect(),
		}
	}
}

impl<'a, Message: 'a> Borrow<dyn Widget<Message, Theme, Renderer> + 'a> for Track<'a, Message> {
	fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
		self
	}
}

impl<'a, Message: 'a> BorrowMut<dyn Widget<Message, Theme, Renderer> + 'a> for Track<'a, Message> {
	fn borrow_mut(&mut self) -> &mut (dyn Widget<Message, Theme, Renderer> + 'a) {
		self
	}
}
