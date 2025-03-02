use iced::{
    Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        renderer::Style,
        widget::{Tree, tree},
    },
    event::Status,
    mouse::{self, Cursor, Interaction},
    widget::Rule,
};
use std::fmt::{Debug, Formatter};

const DRAG_SIZE: f32 = 10.0;

#[derive(Default)]
struct State {
    dragging: Option<f32>,
}

pub struct VSplit<'a, Message> {
    children: [Element<'a, Message>; 3],
    split_at: f32,
    resize: fn(f32) -> Message,
}

impl<Message> Debug for VSplit<'_, Message> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VSplit")
            .field("split_at", &self.split_at)
            .finish_non_exhaustive()
    }
}

impl<'a, Message> VSplit<'a, Message>
where
    Message: 'a,
{
    pub fn new(
        left: impl Into<Element<'a, Message>>,
        right: impl Into<Element<'a, Message>>,
        split_at: f32,
        resize: fn(f32) -> Message,
    ) -> Self {
        Self {
            children: [left.into(), Rule::vertical(DRAG_SIZE).into(), right.into()],
            split_at,
            resize,
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for VSplit<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
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

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.children);
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let max_limits = limits.max();

        let left_width = max_limits.width.mul_add(self.split_at, -(DRAG_SIZE * 0.5));
        let left_limits = Limits::new(
            Size::new(0.0, 0.0),
            Size::new(left_width, max_limits.height),
        );

        let right_width = max_limits.width - left_width - DRAG_SIZE;
        let right_limits = Limits::new(
            Size::new(0.0, 0.0),
            Size::new(right_width, max_limits.height),
        );

        let children = vec![
            self.children[0]
                .as_widget()
                .layout(&mut tree.children[0], renderer, &left_limits),
            self.children[1]
                .as_widget()
                .layout(&mut tree.children[1], renderer, limits)
                .translate(Vector::new(left_width, 0.0)),
            self.children[2]
                .as_widget()
                .layout(&mut tree.children[2], renderer, &right_limits)
                .translate(Vector::new(left_width + DRAG_SIZE, 0.0)),
        ];

        Node::with_children(max_limits, children)
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> Status {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if let Some(position) =
                        cursor.position_in(layout.children().nth(1).unwrap().bounds())
                    {
                        state.dragging = Some(DRAG_SIZE.mul_add(-0.5, position.x));
                        return Status::Captured;
                    }
                }
                mouse::Event::CursorMoved { position } => {
                    if let Some(last) = state.dragging {
                        let split_at = (DRAG_SIZE.mul_add(-0.5, position.x - bounds.x - last)
                            / (bounds.width - DRAG_SIZE))
                            .clamp(0.0, 1.0);
                        shell.publish((self.resize)(split_at));
                        return Status::Captured;
                    }
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) if state.dragging.is_some() => {
                    state.dragging = None;
                    return Status::Captured;
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
        if state.dragging.is_some() || cursor.is_over(layout.children().nth(1).unwrap().bounds()) {
            Interaction::ResizingHorizontally
        } else {
            self.children
                .iter()
                .zip(&tree.children)
                .zip(layout.children())
                .map(|((child, tree), layout)| {
                    child
                        .as_widget()
                        .mouse_interaction(tree, layout, cursor, viewport, renderer)
                })
                .max()
                .unwrap_or_default()
        }
    }
}

impl<'a, Message> From<VSplit<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(vsplit: VSplit<'a, Message>) -> Self {
        Self::new(vsplit)
    }
}
