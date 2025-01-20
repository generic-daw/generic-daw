use super::LINE_HEIGHT;
use iced::{
    advanced::{
        graphics::geometry::Renderer as _,
        layout::{Limits, Node},
        renderer::Style,
        widget::{tree, Tree},
        Clipboard, Layout, Renderer as _, Shell, Widget,
    },
    event::Status,
    mouse::{self, Cursor},
    widget::canvas::{path::Arc, Frame, Path},
    Element, Event, Length, Point, Radians, Rectangle, Renderer, Size, Theme, Vector,
};
use std::{
    f32::consts::{FRAC_PI_2, FRAC_PI_4},
    fmt::Debug,
    ops::RangeInclusive,
};

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
                    if let Some(last) = state.dragging {
                        let mut diff = last - y;
                        diff *= self.range.end() - self.range.start();
                        diff /= 200.0;
                        state.current =
                            (state.current + diff).clamp(*self.range.start(), *self.range.end());
                        if let Some(f) = &self.f {
                            shell.publish(f(state.current));
                        }
                        state.dragging = Some(y);
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
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<State>();

        let mut frame = Frame::new(renderer, bounds.size());

        let inner_circle = Path::circle(frame.center(), RADIUS * 0.7);

        let segment = Path::new(|builder| {
            let start_angle = Radians((-FRAC_PI_4).mul_add(
                5.0,
                FRAC_PI_2 * 3.0 * (self.default - self.range.start())
                    / (self.range.end() - self.range.start()),
            ));
            let end_angle = Radians((-FRAC_PI_4).mul_add(
                5.0,
                FRAC_PI_2 * 3.0 * (state.current - self.range.start())
                    / (self.range.end() - self.range.start()),
            ));

            builder.arc(Arc {
                center: frame.center(),
                radius: RADIUS,
                start_angle,
                end_angle,
            });

            builder.line_to(frame.center());

            builder.close();
        });

        frame.fill(&segment, theme.extended_palette().primary.weak.text);
        frame.fill(&inner_circle, theme.extended_palette().primary.base.color);

        renderer.with_translation(
            Vector::new(bounds.position().x, bounds.position().y),
            |renderer| {
                renderer.draw_geometry(frame.into_geometry());
            },
        );
    }
}

impl<Message> Knob<Message> {
    pub fn new(range: RangeInclusive<f32>, default: f32) -> Self {
        Self {
            range,
            default,
            f: None,
        }
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
