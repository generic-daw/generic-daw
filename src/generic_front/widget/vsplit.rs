use iced::{
    advanced::{
        layout::{Limits, Node},
        renderer::Style,
        widget::{tree, Tree},
        Clipboard, Layout, Shell, Widget,
    },
    event::Status,
    mouse::{Cursor, Interaction},
    widget::{rule::Catalog, Rule},
    Element, Length, Rectangle, Size, Vector,
};

static DRAG_SIZE: f32 = 10.0;

struct State {
    split_at: f32,
    dragging: bool,
    offset: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            split_at: 0.5,
            dragging: false,
            offset: 0.0,
        }
    }
}

#[expect(missing_debug_implementations)]
pub struct VSplit<'a, Message, Theme, Renderer> {
    children: [Element<'a, Message, Theme, Renderer>; 3],
}

impl<'a, Message, Theme, Renderer> VSplit<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    pub fn new(
        left: Element<'a, Message, Theme, Renderer>,
        right: Element<'a, Message, Theme, Renderer>,
    ) -> Self {
        Self {
            children: [left, Rule::vertical(DRAG_SIZE).into(), right],
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VSplit<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(|e| Tree::new(e)).collect()
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let state = tree.state.downcast_ref::<State>();
        let max_limits = limits.max();

        let left_limits = Size::new(
            DRAG_SIZE.mul_add(-0.5, max_limits.width) * state.split_at,
            max_limits.height,
        );
        let left_limits = Limits::new(left_limits, left_limits);

        let right_limits = Size::new(
            DRAG_SIZE.mul_add(-0.5, max_limits.width) * (1.0 - state.split_at),
            max_limits.height,
        );
        let right_limits = Limits::new(right_limits, right_limits);

        let mut moved = 0.0;
        let children = self
            .children
            .iter()
            .zip(&mut tree.children)
            .zip([&left_limits, limits, &right_limits])
            .map(|((child, tree), limits)| {
                let layout = child
                    .as_widget()
                    .layout(tree, renderer, limits)
                    .translate(Vector::new(moved, 0.0));
                moved += layout.bounds().width;

                layout
            })
            .collect();

        Node::with_children(max_limits, children)
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: iced::Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> Status {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        if let iced::Event::Mouse(event) = event {
            match event {
                iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left) => {
                    if let Some(position) =
                        cursor.position_in(layout.children().nth(1).unwrap().bounds())
                    {
                        state.offset = DRAG_SIZE.mul_add(-0.5, position.x);
                        state.dragging = true;
                        return Status::Captured;
                    }
                }
                iced::mouse::Event::CursorMoved { .. } => {
                    if state.dragging {
                        if let Some(position) = cursor.position() {
                            state.split_at = DRAG_SIZE
                                .mul_add(-0.5, position.x - bounds.position().x - state.offset)
                                / (bounds.width - DRAG_SIZE);
                        } else {
                            state.dragging = false;
                        }
                        return Status::Captured;
                    }
                }
                iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left) => {
                    if state.dragging {
                        state.dragging = false;
                        return Status::Captured;
                    }
                }
                _ => {}
            }
        }

        self.children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .map(|((child, tree), layout)| {
                child.as_widget_mut().on_event(
                    tree,
                    event.clone(),
                    layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                )
            })
            .fold(Status::Ignored, Status::merge)
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
        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .filter(|(_, layout)| layout.bounds().intersects(viewport))
            .for_each(|((child, tree), layout)| {
                child
                    .as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, viewport);
            });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> Interaction {
        let state = tree.state.downcast_ref::<State>();
        if state.dragging
            || cursor
                .position_in(layout.children().nth(1).unwrap().bounds())
                .is_some()
        {
            Interaction::ResizingHorizontally
        } else {
            self.children
                .iter()
                .zip(&tree.children)
                .zip(layout.children())
                .find(|(_, layout)| cursor.position_in(layout.bounds()).is_some())
                .map_or_else(Interaction::default, |((child, tree), layout)| {
                    child
                        .as_widget()
                        .mouse_interaction(tree, layout, cursor, viewport, renderer)
                })
        }
    }
}

impl<'a, Message, Theme, Renderer> From<VSplit<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(vsplit: VSplit<'a, Message, Theme, Renderer>) -> Self {
        Self::new(vsplit)
    }
}
