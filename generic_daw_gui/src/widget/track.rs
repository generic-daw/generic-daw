use crate::widget::clip::Clip;
use iced::{
	Event, Fill, Length, Rectangle, Renderer, Shrink, Size, Theme, Vector,
	advanced::{
		Layout, Shell, Widget,
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
	pub(super) clips: Box<[Clip<'a, Message>]>,
}

impl<Message> Widget<Message, Theme, Renderer> for Track<'_, Message> {
	fn diff(&self, tree: &mut Tree) {
		tree.diff_children(&self.clips);
	}

	fn children(&self) -> Vec<Tree> {
		self.clips.iter().map(Tree::new).collect()
	}

	fn size(&self) -> Size<Length> {
		Size::new(Fill, Shrink)
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
		cursor: Cursor,
		renderer: &Renderer,
		shell: &mut Shell<'_, Message>,
		viewport: &Rectangle,
	) {
		self.clips
			.iter_mut()
			.zip(&mut tree.children)
			.zip(layout.children())
			.rev()
			.for_each(|((child, tree), layout)| {
				child.update(tree, event, layout, cursor, renderer, shell, viewport);
			});
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

	fn overlay<'a>(
		&'a mut self,
		tree: &'a mut Tree,
		layout: Layout<'a>,
		renderer: &Renderer,
		viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
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

	pub(super) fn alloc_layers(
		active: &mut Vec<(usize, f32)>,
		layout: Layout<'_>,
		viewport: &Rectangle,
	) -> Vec<Vec<usize>> {
		active.clear();

		let mut result = Vec::<Vec<_>>::new();

		for (i, layout) in layout.children().enumerate() {
			let Some(bounds) = layout.bounds().intersection(viewport) else {
				continue;
			};

			active.retain(|&(_, e)| e > bounds.x);
			let layer = active.iter().map(|&(l, _)| l).max().map_or(0, |l| l + 1);
			active.push((layer, bounds.x + bounds.width));

			if layer == result.len() {
				result.push(Vec::new());
			}

			result[layer].push(i);
		}

		result
	}
}

impl<'a, Message: 'a> Borrow<dyn Widget<Message, Theme, Renderer> + 'a> for Track<'a, Message> {
	fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
		self
	}
}

impl<'a, Message: 'a> Borrow<dyn Widget<Message, Theme, Renderer> + 'a> for &Track<'a, Message> {
	fn borrow(&self) -> &(dyn Widget<Message, Theme, Renderer> + 'a) {
		*self
	}
}
