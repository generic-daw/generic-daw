use iced_widget::{
	Renderer,
	core::{
		Element, Event, Layout, Length, Rectangle, Shell, Size, Theme, Vector, Widget,
		layout::{Limits, Node},
		mouse::{self, Cursor, Interaction},
		overlay,
		renderer::Style,
		widget::{Operation, Tree, tree},
	},
};

pub struct SelectArea<'a, Message> {
	content: Element<'a, Message, Theme, Renderer>,
	on_select: Option<Message>,
}

impl<'a, Message> SelectArea<'a, Message> {
	#[must_use]
	pub fn new(child: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
		Self {
			content: child.into(),
			on_select: None,
		}
	}

	#[must_use]
	pub fn on_select(self, message: Message) -> Self {
		self.on_select_maybe(Some(message))
	}

	#[must_use]
	pub fn on_select_maybe(mut self, message: Option<Message>) -> Self {
		self.on_select = message;
		self
	}
}

impl<Message: Clone> Widget<Message, Theme, Renderer> for SelectArea<'_, Message> {
	fn size(&self) -> Size<Length> {
		self.content.as_widget().size()
	}

	fn tag(&self) -> tree::Tag {
		self.content.as_widget().tag()
	}

	fn state(&self) -> tree::State {
		self.content.as_widget().state()
	}

	fn diff(&mut self, tree: &mut Tree) {
		self.content.as_widget_mut().diff(tree);
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		self.content.as_widget_mut().layout(tree, renderer, limits)
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
		if let Event::Mouse(mouse::Event::ButtonPressed { .. }) = event
			&& cursor.is_over(layout.bounds())
			&& let Some(message) = self.on_select.clone()
		{
			shell.publish(message);
		}

		self.content
			.as_widget_mut()
			.update(tree, event, layout, cursor, renderer, shell, viewport);
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
		self.content
			.as_widget()
			.draw(tree, renderer, theme, style, layout, cursor, viewport);
	}

	fn mouse_interaction(
		&self,
		tree: &Tree,
		layout: Layout<'_>,
		cursor: Cursor,
		viewport: &Rectangle,
		renderer: &Renderer,
	) -> Interaction {
		self.content
			.as_widget()
			.mouse_interaction(tree, layout, cursor, viewport, renderer)
	}

	fn overlay<'a>(
		&'a mut self,
		tree: &'a mut Tree,
		layout: Layout<'a>,
		renderer: &Renderer,
		viewport: &Rectangle,
		translation: Vector,
	) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
		self.content
			.as_widget_mut()
			.overlay(tree, layout, renderer, viewport, translation)
	}

	fn operate(
		&mut self,
		tree: &mut Tree,
		layout: Layout<'_>,
		renderer: &Renderer,
		operation: &mut dyn Operation,
	) {
		operation.traverse(&mut |operation| {
			self.content
				.as_widget_mut()
				.operate(tree, layout, renderer, operation);
		});
	}
}

impl<'a, Message: Clone + 'a> From<SelectArea<'a, Message>>
	for Element<'a, Message, Theme, Renderer>
{
	fn from(value: SelectArea<'a, Message>) -> Self {
		Self::new(value)
	}
}
