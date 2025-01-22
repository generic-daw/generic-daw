use super::LINE_HEIGHT;
use iced::{
    advanced::{
        graphics::geometry::Renderer as _,
        layout::{Limits, Node},
        mouse::{click::Kind, Click},
        renderer::Style,
        widget::{tree, Tree},
        Clipboard, Layout, Renderer as _, Shell, Widget,
    },
    event::Status,
    mouse::{self, Cursor, Interaction},
    widget::canvas::{path::Arc, Cache, Frame, Path},
    Element, Event, Length, Point, Radians, Rectangle, Renderer, Size, Theme, Vector,
};
use std::{
    f32::consts::{FRAC_PI_2, FRAC_PI_4},
    fmt::{Debug, Formatter},
    ops::RangeInclusive,
};

const DIAMETER: f32 = LINE_HEIGHT * 2.0;
const RADIUS: f32 = LINE_HEIGHT;

struct State {
    dragging: Option<f32>,
    current: f32,
    hovering: bool,
    last_click: Option<Click>,
    cache: Cache,
}

impl State {
    pub fn new(current: f32) -> Self {
        Self {
            dragging: None,
            current,
            hovering: false,
            last_click: None,
            cache: Cache::new(),
        }
    }
}

pub struct Knob<Message> {
    range: RangeInclusive<f32>,
    zero: f32,
    default: f32,
    f: Option<Box<dyn Fn(f32) -> Message>>,
}

impl<Message> Debug for Knob<Message> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Knob")
            .field("range", &self.range)
            .field("zero", &self.zero)
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
                    if let Some(pos) = cursor.position() {
                        state.dragging = Some(pos.y);

                        let new_click = Click::new(pos, mouse::Button::Left, state.last_click);
                        if matches!(new_click.kind(), Kind::Double) {
                            state.current = self.default;
                            state.cache.clear();
                        }
                        state.last_click = Some(new_click);

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

                        if let Some(f) = &self.f {
                            shell.publish(f(state.current));
                        }

                        state.cache.clear();
                        state.current =
                            (state.current + diff).clamp(*self.range.start(), *self.range.end());
                        state.dragging = Some(y);

                        return Status::Captured;
                    } else if cursor
                        .position_in(layout.bounds())
                        .is_some_and(|pos| pos.distance(Point::new(RADIUS, RADIUS)) < RADIUS)
                    {
                        if !state.hovering {
                            state.cache.clear();
                            state.hovering = true;
                        }
                    } else if state.hovering {
                        state.cache.clear();
                        state.hovering = false;
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

        renderer.with_translation(
            Vector::new(bounds.position().x, bounds.position().y),
            |renderer| {
                renderer.draw_geometry(state.cache.draw(renderer, bounds.size(), |frame| {
                    self.fill_canvas(state, frame, theme);
                }));
            },
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> Interaction {
        let state = tree.state.downcast_ref::<State>();

        if state.dragging.is_some() {
            Interaction::Grabbing
        } else if state.hovering {
            Interaction::Grab
        } else {
            Interaction::default()
        }
    }
}

impl<Message> Knob<Message> {
    pub fn new(range: RangeInclusive<f32>, zero: f32, default: f32) -> Self {
        Self {
            range,
            zero,
            default,
            f: None,
        }
    }

    pub fn on_move(mut self, f: impl Fn(f32) -> Message + 'static) -> Self {
        self.f = Some(Box::new(f));
        self
    }

    fn fill_canvas(&self, state: &State, frame: &mut Frame, theme: &Theme) {
        let center = frame.center();

        let circle_at_angle = |angle: Radians, a_m: f32, r_m: f32| {
            Path::circle(
                Point::new(
                    (RADIUS * a_m).mul_add(angle.0.cos(), center.x),
                    (RADIUS * a_m).mul_add(angle.0.sin(), center.y),
                ),
                RADIUS * r_m,
            )
        };

        let inner_circle = Path::circle(center, RADIUS * 0.8);

        let base_angle = Radians(-FRAC_PI_4 * 5.0);

        let start_angle = base_angle
            + Radians(
                FRAC_PI_2 * 3.0 * (self.zero - self.range.start())
                    / (self.range.end() - self.range.start()),
            );

        let end_angle = base_angle
            + Radians(
                FRAC_PI_2 * 3.0 * (state.current - self.range.start())
                    / (self.range.end() - self.range.start()),
            );

        let segment = Path::new(|builder| {
            builder.arc(Arc {
                center,
                radius: RADIUS,
                start_angle,
                end_angle,
            });

            builder.line_to(center);

            builder.close();
        });

        frame.fill(&segment, theme.extended_palette().primary.weak.text);

        let color = if state.hovering || state.dragging.is_some() {
            theme.extended_palette().secondary.strong.color
        } else {
            theme.extended_palette().primary.base.color
        };

        frame.fill(&inner_circle, color);

        frame.fill(
            &circle_at_angle(start_angle, 0.9, 0.1),
            theme.extended_palette().primary.weak.text,
        );

        frame.fill(
            &circle_at_angle(end_angle, 0.9, 0.1),
            theme.extended_palette().primary.weak.text,
        );

        frame.fill(
            &circle_at_angle(end_angle, 0.4, 0.15),
            theme.extended_palette().primary.weak.text,
        );
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
