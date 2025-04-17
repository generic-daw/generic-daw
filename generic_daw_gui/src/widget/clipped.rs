use generic_daw_utils::NoDebug;
use iced::{
    Element, Fill, Length, Rectangle, Renderer, Size, Theme,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        renderer::Style,
        widget::{Tree, tree},
    },
    mouse::{Cursor, Interaction},
    overlay,
};

#[derive(Debug)]
pub struct Clipped<'a, Message>(NoDebug<Element<'a, Message>>);

impl<'a, Message> Clipped<'a, Message> {
    pub fn new(inner: impl Into<Element<'a, Message>>) -> Self {
        let inner = inner.into();

        debug_assert!(inner.as_widget().size().width != Fill);
        debug_assert!(inner.as_widget().size().height != Fill);

        Self(inner.into())
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Clipped<'_, Message> {
    fn size(&self) -> Size<Length> {
        self.0.as_widget().size()
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, _limits: &Limits) -> Node {
        self.0.as_widget().layout(tree, renderer, &Limits::NONE)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.0.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
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
        if let Some(clipped_viewport) = layout.bounds().intersection(viewport) {
            self.0.as_widget().draw(
                tree,
                renderer,
                theme,
                style,
                layout,
                cursor,
                &clipped_viewport,
            );
        }
    }

    fn diff(&self, tree: &mut Tree) {
        self.0.as_widget().diff(tree);
    }

    fn children(&self) -> Vec<Tree> {
        self.0.as_widget().children()
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> Interaction {
        self.0
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.0
            .as_widget()
            .operate(tree, layout, renderer, operation);
    }

    fn tag(&self) -> tree::Tag {
        self.0.as_widget().tag()
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        translation: iced::Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.0
            .as_widget_mut()
            .overlay(tree, layout, renderer, translation)
    }

    fn state(&self) -> tree::State {
        self.0.as_widget().state()
    }
}

impl<'a, Message> From<Clipped<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(value: Clipped<'a, Message>) -> Self {
        Element::new(value)
    }
}
