use super::LINE_HEIGHT;
use iced::{
    advanced::{
        layout::{Limits, Node},
        renderer::Style,
        widget::{tree, Tree},
        Clipboard, Layout, Shell, Widget,
    },
    event::Status,
    mouse::{self, Cursor},
    Element, Event, Length, Point, Rectangle, Renderer, Size, Theme,
};
use std::{fmt::Debug, ops::RangeInclusive};

const DIAMETER: f32 = LINE_HEIGHT * 2.0;
const RADIUS: f32 = LINE_HEIGHT;

struct State {
    dragging: Option<f32>,
    current: f32,
}

impl State {
    pub fn new(current: f32) -> Self {
        Self {
            dragging: None,
            current,
        }
    }
}

pub struct Knob<Message> {
    range: RangeInclusive<f32>,
    default: f32,
    f: Option<Box<dyn Fn(f32) -> Message>>,
}

impl<Message> Debug for Knob<Message> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Knob")
            .field("range", &self.range)
            .field("default", &self.default)
            .finish_non_exhaustive()
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Knob<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(DIAMETER), Length::Fixed(DIAMETER))
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(self.default))
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        Node::new(Size::new(DIAMETER, DIAMETER))
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> Status {
        let state = tree.state.downcast_mut::<State>();

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonPressed(mouse::Button::Left)
                    if state.dragging.is_none()
                        && cursor.position_in(layout.bounds()).is_some_and(|pos| {
                            pos.distance(Point::new(RADIUS, RADIUS)) < RADIUS
                        }) =>
                {
                    state.dragging = cursor.position().map(|pos| pos.y);
                    if state.dragging.is_some() {
                        return Status::Captured;
                    }
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) if state.dragging.is_some() => {
                    state.dragging = None;
                    return Status::Captured;
                }
                mouse::Event::CursorMoved {
                    position: Point { y, .. },
                } => {
                    if let Some(base) = state.dragging {
                        let mut diff = y - base;
                        diff *= self.range.end() - self.range.start();
                        state.current =
                            (state.current + diff).clamp(*self.range.start(), *self.range.end());
                        if let Some(f) = &self.f {
                            shell.publish(f(state.current));
                        }
                        return Status::Captured;
                    }
                }
                _ => {}
            }
        }

        Status::Ignored
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

impl<'a, Message> From<Knob<Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
{
    fn from(knob: Knob<Message>) -> Self {
        Self::new(knob)
    }
}
