use super::ArrangementScale;
use iced::{
    Element, Event, Length, Rectangle, Renderer, Size, Theme,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        renderer::Style,
        widget::Tree,
    },
    mouse::{Cursor, Interaction},
};
use std::fmt::{Debug, Formatter};

pub struct Track<'a, Message> {
    /// list of the track panel and all the clip widgets
    children: Box<[Element<'a, Message>]>,
    /// the scale of the arrangement viewport
    scale: ArrangementScale,
}

impl<Message> Debug for Track<'_, Message> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Track")
            .field("scale", &self.scale)
            .finish_non_exhaustive()
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Track<'_, Message> {
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.children);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fixed(self.scale.y),
        }
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        Node::with_children(
            Size::new(limits.max().width, self.scale.y),
            self.children
                .iter()
                .zip(&mut tree.children)
                .map(|(widget, tree)| widget.as_widget().layout(tree, renderer, limits))
                .collect(),
        )
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        _viewport: &Rectangle,
        renderer: &Renderer,
    ) -> Interaction {
        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .filter_map(|((child, tree), clip_layout)| {
                Some(child.as_widget().mouse_interaction(
                    tree,
                    clip_layout,
                    cursor,
                    &clip_layout.bounds().intersection(&layout.bounds())?,
                    renderer,
                ))
            })
            .max()
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
        let Some(bounds) = viewport.intersection(&layout.bounds()) else {
            return;
        };

        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .for_each(|((child, tree), layout)| {
                renderer.with_layer(bounds, |renderer| {
                    child
                        .as_widget()
                        .draw(tree, renderer, theme, style, layout, cursor, &bounds);
                });
            });
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
        if shell.is_event_captured() {
            return;
        }

        self.children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .for_each(|((child, state), layout)| {
                let Some(viewport) = layout.bounds().intersection(viewport) else {
                    return;
                };

                child.as_widget_mut().update(
                    state, event, layout, cursor, renderer, clipboard, shell, &viewport,
                );
            });
    }
}

impl<'a, Message> Track<'a, Message>
where
    Message: 'a,
{
    pub fn new(
        children: impl IntoIterator<Item = Element<'a, Message>>,
        scale: ArrangementScale,
    ) -> Self {
        Self {
            children: children.into_iter().collect(),
            scale,
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
