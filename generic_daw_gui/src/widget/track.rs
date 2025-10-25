use generic_daw_utils::{NoDebug, Vec2};
use iced::{
	Element, Event, Fill, Length, Rectangle, Renderer, Size, Theme, Vector,
	advanced::{
		Clipboard, Layout, Renderer as _, Shell, Widget,
		layout::{Limits, Node},
		overlay,
		renderer::Style,
		widget::{Operation, Tree},
	},
	mouse::{Cursor, Interaction},
};

#[derive(Debug)]
pub struct Track<'a, Message> {
	scale: &'a Vec2,
	children: NoDebug<Box<[Element<'a, Message>]>>,
}

impl<Message> Widget<Message, Theme, Renderer> for Track<'_, Message> {
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
				.map(|(widget, tree)| {
					widget
						.as_widget_mut()
						.layout(tree, renderer, &limits.height(self.scale.y))
				})
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
				child
					.as_widget()
					.mouse_interaction(tree, clip_layout, cursor, viewport, renderer)
			})
			.rfind(|&i| i != Interaction::default())
			.unwrap_or_default()
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
		let mut rects = Vec::new();

		renderer.start_layer(Rectangle::INFINITE);

		self.children
			.iter()
			.zip(&tree.children)
			.zip(layout.children())
			.for_each(|((child, tree), layout)| {
				let Some(bounds) = layout.bounds().intersection(viewport) else {
					return;
				};

				if rects.iter().any(|b| bounds.intersects(b)) {
					rects.clear();
					renderer.end_layer();
					renderer.start_layer(Rectangle::INFINITE);
				}

				rects.push(bounds);

				child
					.as_widget()
					.draw(tree, renderer, theme, style, layout, cursor, viewport);
			});

		renderer.end_layer();
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
				child.as_widget_mut().update(
					state, event, layout, cursor, renderer, clipboard, shell, viewport,
				);
			});
	}

	fn overlay<'a>(
		&'a mut self,
		tree: &'a mut Tree,
		layout: Layout<'a>,
		renderer: &Renderer,
		viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
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
					child
						.as_widget_mut()
						.operate(state, layout, renderer, operation);
				});
		});
	}
}

impl<'a, Message> Track<'a, Message>
where
	Message: 'a,
{
	pub fn new(scale: &'a Vec2, children: impl IntoIterator<Item = Element<'a, Message>>) -> Self {
		Self {
			scale,
			children: children.into_iter().collect::<Box<_>>().into(),
		}
	}
}

impl<'a, Message> From<Track<'a, Message>> for Element<'a, Message>
where
	Message: 'a,
{
	fn from(value: Track<'a, Message>) -> Self {
		Element::new(value)
	}
}
