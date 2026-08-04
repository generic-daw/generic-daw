use crate::{LazyElement, menu_overlay::MenuOverlay};
use iced_widget::{
	Renderer, Theme,
	core::{
		Element, Event, Layout, Length, Point, Rectangle, Shell, Size, Vector, Widget,
		layout::{Limits, Node},
		mouse::{self, Cursor, Interaction},
		overlay,
		renderer::Style,
		widget::{Operation, Tree, tree},
	},
};

struct State {
	position: Option<Point>,
}

pub struct ContextMenu<'a, Message> {
	content: Element<'a, Message, Theme, Renderer>,
	context_menu: LazyElement<'a, Message, Theme, Renderer>,
}

impl<'a, Message> ContextMenu<'a, Message> {
	pub fn new(
		content: impl Into<Element<'a, Message, Theme, Renderer>>,
		context_menu: impl Fn() -> Element<'a, Message, Theme, Renderer> + 'a,
	) -> Self {
		Self {
			content: content.into(),
			context_menu: LazyElement::new(Box::new(context_menu)),
		}
	}
}

impl<Message> Widget<Message, Theme, Renderer> for ContextMenu<'_, Message> {
	fn size(&self) -> Size<Length> {
		self.content.as_widget().size()
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		self.content
			.as_widget_mut()
			.layout(&mut tree.children[0], renderer, limits)
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
		self.content.as_widget().draw(
			&tree.children[0],
			renderer,
			theme,
			style,
			layout,
			cursor,
			viewport,
		);
	}

	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State { position: None })
	}

	fn diff(&mut self, tree: &mut Tree) {
		if tree.state.downcast_ref::<State>().position.is_some() {
			tree.diff_children(&mut [&mut self.content, &mut self.context_menu]);
		} else {
			tree.diff_children(&mut [&mut self.content]);
		}
	}

	fn operate(
		&mut self,
		tree: &mut Tree,
		layout: Layout<'_>,
		renderer: &Renderer,
		operation: &mut dyn Operation,
	) {
		self.content
			.as_widget_mut()
			.operate(&mut tree.children[0], layout, renderer, operation);
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
		self.content.as_widget_mut().update(
			&mut tree.children[0],
			event,
			layout,
			cursor,
			renderer,
			shell,
			viewport,
		);

		if shell.is_event_captured() {
			return;
		}

		if let Event::Mouse(mouse::Event::ButtonPressed {
			button: mouse::Button::Right,
			..
		}) = event && let Some(position) = cursor.position()
			&& layout.bounds().contains(position)
		{
			tree.state.downcast_mut::<State>().position = Some(position);
			shell.capture_event();
			shell.request_redraw();

			if tree.children.len() == 1 {
				tree.children.push(Tree::new(&*self.context_menu));
			}
			self.context_menu
				.as_widget_mut()
				.diff(&mut tree.children[1]);
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
		self.content.as_widget().mouse_interaction(
			&tree.children[0],
			layout,
			cursor,
			viewport,
			renderer,
		)
	}

	fn overlay<'a>(
		&'a mut self,
		tree: &'a mut Tree,
		layout: Layout<'a>,
		renderer: &Renderer,
		viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
		let state = tree.state.downcast_mut::<State>();

		let Some(position) = state.position else {
			return self.content.as_widget_mut().overlay(
				&mut tree.children[0],
				layout,
				renderer,
				viewport,
				translation,
			);
		};

		let [first, second] = &mut *tree.children else {
			unreachable!();
		};

		let context_menu = overlay::Element::new(Box::new(MenuOverlay {
			content: &mut self.context_menu,
			tree: second,
			state: &mut state.position,
			bounds: Rectangle::new(position + translation, Size::ZERO),
		}));

		let Some(content_overlay) =
			self.content
				.as_widget_mut()
				.overlay(first, layout, renderer, viewport, translation)
		else {
			return Some(context_menu);
		};

		Some(overlay::Group::with_children(vec![content_overlay, context_menu]).overlay())
	}
}

impl<'a, Message: 'a> From<ContextMenu<'a, Message>> for Element<'a, Message, Theme, Renderer> {
	fn from(value: ContextMenu<'a, Message>) -> Self {
		Self::new(value)
	}
}
