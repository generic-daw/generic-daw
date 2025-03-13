use iced::{
    Element, Length, Rectangle, Renderer, Size, Theme,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        renderer::Style,
        widget::Tree,
    },
    mouse::Cursor,
};

#[derive(Clone, Copy, Debug)]
pub struct Redrawer(pub bool);

impl<Message> Widget<Message, Theme, Renderer> for Redrawer {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        Node::new(Size::new(0.0, 0.0))
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        _event: &iced::Event,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if self.0 {
            shell.request_redraw();
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
    }
}

impl<Message> From<Redrawer> for Element<'_, Message> {
    fn from(value: Redrawer) -> Self {
        Element::new(value)
    }
}
