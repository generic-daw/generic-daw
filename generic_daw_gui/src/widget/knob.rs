use super::{LINE_HEIGHT, SWM};
use generic_daw_utils::NoDebug;
use iced::{
    Element, Event, Length, Point, Radians, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        graphics::geometry::Renderer as _,
        layout::{Limits, Node},
        renderer::Style,
        widget::{Tree, tree},
    },
    mouse::{self, Cursor, Interaction, ScrollDelta},
    widget::canvas::{Cache, Frame, Path, path::Arc},
    window,
};
use std::{
    f32::consts::{FRAC_PI_2, FRAC_PI_4},
    fmt::Debug,
    ops::RangeInclusive,
};

#[derive(Default)]
struct State {
    dragging: Option<(f32, f32)>,
    hovering: bool,
    last_value: f32,
    last_enabled: bool,
    cache: Cache,
}

#[derive(Debug)]
pub struct Knob<Message, F>
where
    F: Fn(f32) -> Message,
{
    range: RangeInclusive<f32>,
    value: f32,
    center: f32,
    enabled: bool,
    on_change: NoDebug<F>,
    radius: f32,
}

impl<Message, F> Widget<Message, Theme, Renderer> for Knob<Message, F>
where
    F: Fn(f32) -> Message,
{
    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(2.0 * self.radius),
            Length::Fixed(2.0 * self.radius),
        )
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        Node::new(Size::new(2.0 * self.radius, 2.0 * self.radius))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        match event {
            Event::Window(window::Event::RedrawRequested(..)) => {
                if self.enabled != state.last_enabled {
                    state.last_enabled = self.enabled;
                    state.cache.clear();
                }

                if self.value != state.last_value {
                    state.last_value = self.value;
                    state.cache.clear();
                }
            }
            Event::Mouse(event) => match event {
                mouse::Event::ButtonPressed {
                    button: mouse::Button::Left,
                    ..
                } if state.dragging.is_none() => {
                    if let Some(pos) = cursor.position() {
                        if pos.distance(bounds.center()) < self.radius {
                            state.dragging = Some((self.value, pos.y));
                            shell.capture_event();
                        }
                    }
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) if state.dragging.is_some() => {
                    if !state.hovering {
                        state.cache.clear();
                        shell.request_redraw();
                    }
                    state.dragging = None;
                    shell.capture_event();
                }
                mouse::Event::CursorMoved {
                    position: Point { y, .. },
                    ..
                } => {
                    if state.hovering
                        != cursor
                            .position()
                            .is_some_and(|pos| pos.distance(bounds.center()) < self.radius)
                    {
                        state.hovering ^= true;
                        state.cache.clear();
                        shell.request_redraw();
                    }

                    if let Some((value, last)) = state.dragging {
                        let diff = (last - y) * (self.range.end() - self.range.start()) * 0.005;

                        shell.publish((self.on_change)(
                            (value + diff).clamp(*self.range.start(), *self.range.end()),
                        ));
                        shell.capture_event();
                    }
                }
                mouse::Event::WheelScrolled { delta, .. }
                    if state.dragging.is_none()
                        && cursor
                            .position()
                            .is_some_and(|pos| pos.distance(bounds.center()) < self.radius) =>
                {
                    let diff = match delta {
                        ScrollDelta::Lines { y, .. } => *y,
                        ScrollDelta::Pixels { y, .. } => y / SWM,
                    } * (self.range.end() - self.range.start())
                        * 0.05;

                    shell.publish((self.on_change)(
                        (self.value + diff).clamp(*self.range.start(), *self.range.end()),
                    ));
                    shell.capture_event();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        if !bounds.intersects(viewport) {
            return;
        }

        let state = tree.state.downcast_ref::<State>();

        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
            renderer.draw_geometry(state.cache.draw(renderer, bounds.size(), |frame| {
                self.fill_canvas(state, frame, theme);
            }));
        });
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

impl<Message, F> Knob<Message, F>
where
    F: Fn(f32) -> Message,
{
    pub fn new(
        range: RangeInclusive<f32>,
        zero: f32,
        current: f32,
        enabled: bool,
        on_change: F,
    ) -> Self {
        Self {
            range,
            center: zero,
            value: current,
            enabled,
            on_change: on_change.into(),
            radius: LINE_HEIGHT,
        }
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    fn fill_canvas(&self, state: &State, frame: &mut Frame, theme: &Theme) {
        let center = frame.center();

        let circle = |angle: Radians, a_m: f32, r_m: f32| {
            Path::circle(
                Point::new(
                    (a_m).mul_add(angle.0.cos(), center.x),
                    (a_m).mul_add(angle.0.sin(), center.y),
                ),
                r_m,
            )
        };

        let base_angle = Radians(-FRAC_PI_4 * 5.0);

        let start_angle = base_angle
            + Radians(
                FRAC_PI_2 * 3.0 * (self.center - self.range.start())
                    / (self.range.end() - self.range.start()),
            );

        let end_angle = base_angle
            + Radians(
                FRAC_PI_2 * 3.0 * (self.value - self.range.start())
                    / (self.range.end() - self.range.start()),
            );

        let arc = Path::new(|builder| {
            builder.arc(Arc {
                center,
                radius: self.radius,
                start_angle,
                end_angle,
            });
            builder.line_to(center);
            builder.close();
        });

        let main_color = if !self.enabled || state.hovering || state.dragging.is_some() {
            theme.extended_palette().secondary.weak.color
        } else {
            theme.extended_palette().primary.weak.color
        };
        let contrast_color = theme.extended_palette().background.strong.text;

        frame.fill(&arc, contrast_color);

        frame.fill(&Path::circle(center, self.radius - 4.0), main_color);

        frame.fill(&circle(start_angle, self.radius - 2.0, 2.0), contrast_color);

        frame.fill(&circle(end_angle, self.radius - 2.0, 2.0), contrast_color);

        frame.fill(
            &circle(end_angle, self.radius / 2.0 - 2.0, 3.0),
            contrast_color,
        );
    }
}

impl<'a, Message, F> From<Knob<Message, F>> for Element<'a, Message>
where
    Message: 'a,
    F: Fn(f32) -> Message + 'a,
{
    fn from(value: Knob<Message, F>) -> Self {
        Self::new(value)
    }
}
