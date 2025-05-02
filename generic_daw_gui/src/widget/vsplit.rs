use generic_daw_utils::NoDebug;
use iced::{
    Element, Event, Fill, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        overlay,
        renderer::Style,
        widget::{Operation, Tree, tree},
    },
    mouse::{self, Cursor, Interaction},
    widget::{mouse_area, rule, vertical_rule},
};

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
}

#[derive(Debug)]
pub struct VSplit<'a, Message> {
    children: NoDebug<[Element<'a, Message>; 3]>,
    split_at: f32,
    strategy: Strategy,
    rule_width: f32,
    f: fn(f32) -> Message,
}

impl<'a, Message> VSplit<'a, Message>
where
    Message: Clone + 'a,
{
    pub fn new(
        left: impl Into<Element<'a, Message>>,
        right: impl Into<Element<'a, Message>>,
        on_resize: fn(f32) -> Message,
    ) -> Self {
        Self {
            children: [
                left.into(),
                mouse_area(vertical_rule(11.0))
                    .interaction(Interaction::ResizingColumn)
                    .into(),
                right.into(),
            ]
            .into(),
            split_at: 0.5,
            strategy: Strategy::default(),
            rule_width: 11.0,
            f: on_resize,
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

    pub fn rule_width(mut self, rule_width: f32) -> Self {
        self.rule_width = rule_width;
        self.children[1] = mouse_area(vertical_rule(self.rule_width))
            .interaction(Interaction::ResizingColumn)
            .into();
        self
    }

    pub fn style(mut self, style: impl Fn(&Theme) -> rule::Style + 'a) -> Self {
        self.children[1] = mouse_area(vertical_rule(self.rule_width).style(style))
            .interaction(Interaction::ResizingColumn)
            .into();
        self
    }
}

impl<Message> Widget<Message, Theme, Renderer> for VSplit<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn size(&self) -> Size<Length> {
        Size::new(Fill, Fill)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&*self.children);
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
                } if state.dragging => {
                    let relative_pos =
                        (x - bounds.x - self.rule_width / 2.0).clamp(0.0, bounds.width);
                    let split_at = match self.strategy {
                        Strategy::Relative => relative_pos / bounds.width,
                        Strategy::Left => relative_pos,
                        Strategy::Right => bounds.width - relative_pos - self.rule_width,
                    };
                    shell.publish((self.f)(split_at));
                    shell.capture_event();
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
        if tree.state.downcast_ref::<State>().dragging {
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

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut *self.children,
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

impl<'a, Message> From<VSplit<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(value: VSplit<'a, Message>) -> Self {
        Self::new(value)
    }
}
