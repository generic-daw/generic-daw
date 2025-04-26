use super::get_time;
use generic_daw_core::{Meter, Position};
use generic_daw_utils::{NoDebug, Vec2};
use iced::{
    Element, Event, Fill, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse::{Click, click::Kind},
        overlay,
        renderer::Style,
        widget::{Operation, Tree, tree},
    },
    mouse::{self, Cursor, Interaction},
};

#[derive(Default)]
struct State {
    last_click: Option<Click>,
}

#[derive(Debug)]
pub struct Track<'a, Message> {
    meter: &'a Meter,
    position: &'a Vec2,
    scale: &'a Vec2,
    children: NoDebug<Box<[Element<'a, Message>]>>,
    on_double_click: NoDebug<Box<dyn Fn(Position) -> Message>>,
}

impl<Message> Widget<Message, Theme, Renderer> for Track<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.children);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Fill, Length::Fixed(self.scale.y))
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
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> Interaction {
        let Some(bounds) = viewport.intersection(&layout.bounds()) else {
            return Interaction::default();
        };

        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((child, tree), clip_layout)| {
                child
                    .as_widget()
                    .mouse_interaction(tree, clip_layout, cursor, &bounds, renderer)
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
        let Some(bounds) = viewport.intersection(&layout.bounds()) else {
            return;
        };

        self.children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .for_each(|((child, state), layout)| {
                child.as_widget_mut().update(
                    state, event, layout, cursor, renderer, clipboard, shell, &bounds,
                );
            });

        if shell.is_event_captured() {
            return;
        }

        let Some(cursor) = cursor.position_in(layout.bounds()) else {
            return;
        };

        if let Event::Mouse(mouse::Event::ButtonPressed {
            button: mouse::Button::Left,
            modifiers,
        }) = event
        {
            let state = tree.state.downcast_mut::<State>();

            let new_click = Click::new(cursor, mouse::Button::Left, state.last_click);
            state.last_click = Some(new_click);

            if new_click.kind() == Kind::Double {
                let time = get_time(cursor.x, *modifiers, self.meter, self.position, self.scale);

                shell.publish((self.on_double_click)(time));
                shell.capture_event();
            }
        }
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'_>,
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
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds(), &mut |operation| {
            self.children
                .iter()
                .zip(&mut tree.children)
                .zip(layout.children())
                .for_each(|((child, state), layout)| {
                    child
                        .as_widget()
                        .operate(state, layout, renderer, operation);
                });
        });
    }
}

impl<'a, Message> Track<'a, Message>
where
    Message: 'a,
{
    pub fn new(
        meter: &'a Meter,
        position: &'a Vec2,
        scale: &'a Vec2,
        children: impl IntoIterator<Item = Element<'a, Message>>,
        on_double_click: impl Fn(Position) -> Message + 'static,
    ) -> Self {
        Self {
            meter,
            position,
            scale,
            children: children.into_iter().collect::<Box<_>>().into(),
            on_double_click: NoDebug(Box::new(on_double_click)),
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
