use crate::{LazyElement, menu_overlay::MenuOverlay};
use iced_widget::{
	Renderer, button,
	button::Catalog as _,
	core::{
		Background, Color, Element, Event, Layout, Length, Padding, Pixels, Point, Rectangle,
		Renderer as _, Shell, Size, Theme, Vector, Widget, layout,
		layout::{Limits, Node},
		mouse,
		mouse::{Cursor, Interaction},
		overlay, renderer,
		renderer::Style,
		widget::{
			Operation,
			tree::{self, Tree},
		},
		window,
	},
};

pub struct Menu<'a, Message> {
	content: Element<'a, Message, Theme, Renderer>,
	#[expect(clippy::struct_field_names)]
	menu: LazyElement<'a, Message, Theme, Renderer>,
	width: Length,
	height: Length,
	padding: Padding,
	spacing: f32,
	class: <Theme as button::Catalog>::Class<'a>,
	status: Option<button::Status>,
}

impl<'a, Message> Menu<'a, Message> {
	pub fn new(
		content: impl Into<Element<'a, Message, Theme, Renderer>>,
		menu: impl Fn() -> Element<'a, Message, Theme, Renderer> + 'a,
	) -> Self {
		Menu {
			content: content.into(),
			menu: LazyElement::new(Box::new(menu)),
			width: Length::Fit,
			height: Length::Fit,
			padding: button::DEFAULT_PADDING,
			spacing: 0.0,
			class: <Theme as button::Catalog>::default(),
			status: None,
		}
	}

	#[must_use]
	pub fn width(mut self, width: impl Into<Length>) -> Self {
		self.width = width.into();
		self
	}

	#[must_use]
	pub fn height(mut self, height: impl Into<Length>) -> Self {
		self.height = height.into();
		self
	}

	#[must_use]
	pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
		self.padding = padding.into();
		self
	}

	#[must_use]
	pub fn spacing(mut self, spacing: impl Into<Pixels>) -> Self {
		self.spacing = spacing.into().0;
		self
	}

	#[must_use]
	pub fn style(mut self, style: impl Fn(&Theme, button::Status) -> button::Style + 'a) -> Self {
		self.class = Box::new(style) as _;
		self
	}
}

#[derive(Clone, Copy, Debug, Default)]
struct State {
	is_pressed: bool,
	position: Option<Point>,
}

impl<'a, Message: Clone + 'a> Widget<Message, Theme, Renderer> for Menu<'a, Message> {
	fn tag(&self) -> tree::Tag {
		tree::Tag::of::<State>()
	}

	fn state(&self) -> tree::State {
		tree::State::new(State::default())
	}

	fn diff(&mut self, tree: &mut Tree) {
		if tree.state.downcast_ref::<State>().position.is_some() {
			tree.diff_children(&mut [&mut self.content, &mut self.menu]);
		} else {
			tree.diff_children(&mut [&mut self.content]);
		}

		let size = self.content.as_widget().size();
		self.width = self.width.stack(size.width);
		self.height = self.height.stack(size.height);
	}

	fn size(&self) -> Size<Length> {
		Size {
			width: self.width,
			height: self.height,
		}
	}

	fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
		layout::padded(limits, self.width, self.height, self.padding, |limits| {
			self.content
				.as_widget_mut()
				.layout(&mut tree.children[0], renderer, limits)
		})
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
			self.content.as_widget_mut().operate(
				&mut tree.children[0],
				layout.children().next().unwrap(),
				renderer,
				operation,
			);
		});
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
			layout.children().next().unwrap(),
			cursor,
			renderer,
			shell,
			viewport,
		);

		if shell.is_event_captured() {
			return;
		}

		let state = tree.state.downcast_mut::<State>();

		match event {
			Event::Mouse(mouse::Event::ButtonPressed {
				button: mouse::Button::Left,
				..
			}) if cursor.is_over(layout.bounds()) => {
				state.is_pressed = true;
				shell.capture_event();
			}
			Event::Mouse(mouse::Event::ButtonReleased {
				button: mouse::Button::Left,
				..
			}) if state.is_pressed => {
				state.is_pressed = false;
				shell.capture_event();

				if cursor.is_over(layout.bounds()) {
					state.position = Some(layout.position());
					shell.request_redraw();

					if tree.children.len() == 1 {
						tree.children.push(Tree::new(&*self.menu));
					}
					self.menu.as_widget_mut().diff(&mut tree.children[1]);
				}
			}
			_ => {}
		}

		let current_status = if state.position.is_some() {
			button::Status::Pressed
		} else if !cursor.is_over(layout.bounds()) {
			button::Status::Active
		} else if state.is_pressed {
			button::Status::Pressed
		} else {
			button::Status::Hovered
		};

		if let Event::Window(window::Event::RedrawRequested(..)) = event {
			self.status = Some(current_status);
		} else if self.status.is_some_and(|status| status != current_status) {
			shell.request_redraw();
		}
	}

	fn draw(
		&self,
		tree: &Tree,
		renderer: &mut Renderer,
		theme: &Theme,
		_style: &Style,
		layout: Layout<'_>,
		cursor: Cursor,
		viewport: &Rectangle,
	) {
		let bounds = layout.bounds();
		let content_layout = layout.children().next().unwrap();
		let style = theme.style(&self.class, self.status.unwrap_or(button::Status::Disabled));

		if style.background.is_some() || style.border.width > 0.0 || style.shadow.color.a > 0.0 {
			renderer.fill_quad(
				renderer::Quad {
					bounds,
					border: style.border,
					shadow: style.shadow,
					snap: style.snap,
				},
				style
					.background
					.unwrap_or(Background::Color(Color::TRANSPARENT)),
			);
		}

		self.content.as_widget().draw(
			&tree.children[0],
			renderer,
			theme,
			&Style {
				text_color: style.text_color,
			},
			content_layout,
			cursor,
			viewport,
		);
	}

	fn mouse_interaction(
		&self,
		_tree: &Tree,
		layout: Layout<'_>,
		cursor: Cursor,
		_viewport: &Rectangle,
		_renderer: &Renderer,
	) -> Interaction {
		if cursor.is_over(layout.bounds()) {
			Interaction::Pointer
		} else {
			Interaction::default()
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
		let state = tree.state.downcast_mut::<State>();

		let Some(_) = state.position else {
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

		let menu = overlay::Element::new(Box::new(MenuOverlay {
			content: &mut self.menu,
			tree: second,
			state: &mut state.position,
			bounds: Rectangle::new(
				layout.position() + Vector::new(0.0, layout.bounds().height),
				Size::new(
					layout.bounds().width + self.spacing,
					-layout.bounds().height - self.spacing,
				),
			),
		}));

		let Some(content_overlay) =
			self.content
				.as_widget_mut()
				.overlay(first, layout, renderer, viewport, translation)
		else {
			return Some(menu);
		};

		Some(overlay::Group::with_children(vec![content_overlay, menu]).overlay())
	}
}

impl<'a, Message: Clone + 'a> From<Menu<'a, Message>> for Element<'a, Message, Theme, Renderer> {
	fn from(value: Menu<'a, Message>) -> Self {
		Self::new(value)
	}
}
