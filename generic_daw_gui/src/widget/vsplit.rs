use iced::{
    Element, Event, Length, Pixels, Point, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        overlay,
        renderer::Style,
        widget::{Operation, Tree, tree},
    },
    mouse::{self, Cursor, Interaction},
    widget::{Rule, rule},
};
use std::fmt::{Debug, Formatter};

#[derive(Clone, Copy, Debug, Default)]
pub enum Strategy {
    #[default]
    Relative,
    Left,
    Right,
}

#[derive(Default)]
struct State {
    dragging: bool,
    hovering: bool,
}

pub struct VSplit<'a, Message> {
    children: [Element<'a, Message>; 3],
    split_at: f32,
    strategy: Strategy,
    rule_width: f32,
    on_resize: Option<fn(f32) -> Message>,
}

impl<Message> Debug for VSplit<'_, Message> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VSplit")
            .field("split_at", &self.split_at)
            .field("strategy", &self.strategy)
            .field("rule_width", &self.rule_width)
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
    ) -> Self {
        Self {
            children: [left.into(), Rule::vertical(11.0).into(), right.into()],
            split_at: 0.5,
            strategy: Strategy::default(),
            rule_width: 11.0,
            on_resize: None,
        }
    }

    pub fn split_at(mut self, split_at: f32) -> Self {
        self.split_at = split_at;
        self
    }

    pub fn strategy(mut self, strategy: Strategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn rule_width(mut self, rule_width: impl Into<Pixels>) -> Self {
        self.rule_width = rule_width.into().0;
        self.children[1] = Rule::vertical(self.rule_width).into();
        self
    }

    pub fn on_resize(mut self, on_resize: fn(f32) -> Message) -> Self {
        self.on_resize = Some(on_resize);
        self
    }

    pub fn style(mut self, style: impl Fn(&Theme) -> rule::Style + 'a) -> Self {
        self.children[1] = Rule::vertical(self.rule_width).style(style).into();
        self
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

        let left_width = match self.strategy {
            Strategy::Relative => max_limits
                .width
                .mul_add(self.split_at, -self.rule_width / 2.0)
                .floor(),
            Strategy::Left => self.split_at,
            Strategy::Right => max_limits.width - self.split_at - self.rule_width,
        }
        .min(max_limits.width)
        .max(0.0);

        let left_limits = Limits::new(
            Size::new(0.0, 0.0),
            Size::new(left_width, max_limits.height),
        );

        let right_width = max_limits.width - left_width - self.rule_width;
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
                .translate(Vector::new(left_width + self.rule_width, 0.0)),
        ];

        Node::with_children(max_limits, children)
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
            .for_each(|((child, tree), layout)| {
                child.as_widget_mut().update(
                    tree, event, layout, cursor, renderer, clipboard, shell, viewport,
                );
            });

        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonPressed {
                    button: mouse::Button::Left,
                    ..
                } if cursor.is_over(layout.children().nth(1).unwrap().bounds()) => {
                    state.dragging = true;
                    shell.capture_event();
                }
                mouse::Event::CursorMoved {
                    position: Point { x, .. },
                    ..
                } => {
                    if state.dragging {
                        if let Some(on_resize) = self.on_resize {
                            let relative_pos =
                                (x - bounds.x - self.rule_width / 2.0).clamp(0.0, bounds.width);
                            let split_at = match self.strategy {
                                Strategy::Relative => relative_pos / bounds.width,
                                Strategy::Left => relative_pos,
                                Strategy::Right => bounds.width - relative_pos - self.rule_width,
                            };
                            shell.publish((on_resize)(split_at));
                            shell.capture_event();
                        }
                    } else if state.hovering
                        != cursor.is_over(layout.children().nth(1).unwrap().bounds())
                    {
                        state.hovering ^= true;
                        shell.request_redraw();
                    }
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) if state.dragging => {
                    state.dragging = false;
                    shell.capture_event();
                }
                _ => {}
            }
        }
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
        if state.dragging || state.hovering {
            Interaction::ResizingColumn
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

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(&mut self.children, tree, layout, renderer, translation)
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

impl<'a, Message> From<VSplit<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(value: VSplit<'a, Message>) -> Self {
        Self::new(value)
    }
}
