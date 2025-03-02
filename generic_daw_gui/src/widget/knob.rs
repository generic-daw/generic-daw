use super::{LINE_HEIGHT, arrangement::SWM};
use iced::{
    Element, Event, Length, Point, Radians, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        graphics::geometry::Renderer as _,
        layout::{Limits, Node},
        mouse::{Click, click::Kind},
        renderer::Style,
        widget::{Tree, tree},
    },
    event::Status,
    mouse::{self, Cursor, Interaction, ScrollDelta},
    widget::canvas::{Cache, Frame, Path, path::Arc},
    window,
};
use std::{
    cell::RefCell,
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
    last_enabled: bool,
    last_theme: RefCell<Option<Theme>>,
    cache: Cache,
}

impl State {
    pub fn new(current: f32, last_enabled: bool) -> Self {
        Self {
            dragging: None,
            current,
            hovering: false,
            last_click: None,
            last_enabled,
            last_theme: RefCell::default(),
            cache: Cache::new(),
        }
    }
}

pub struct Knob<Message> {
    range: RangeInclusive<f32>,
    zero: f32,
    default: f32,
    enabled: bool,
    f: Box<dyn Fn(f32) -> Message>,
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
        tree::State::new(State::new(self.default, self.enabled))
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
        let bounds = layout.bounds();

        match event {
            Event::Window(window::Event::RedrawRequested(..)) => {
                if self.enabled != state.last_enabled {
                    state.last_enabled = self.enabled;
                    state.cache.clear();
                    return Status::Ignored;
                }
            }
            Event::Mouse(event) => match event {
                mouse::Event::ButtonPressed(mouse::Button::Left)
                    if state.dragging.is_none()
                        && cursor
                            .position()
                            .is_some_and(|pos| pos.distance(bounds.center()) < RADIUS) =>
                {
                    let pos = cursor.position().unwrap();
                    state.dragging = Some(pos.y);

                    let new_click = Click::new(pos, mouse::Button::Left, state.last_click);
                    if matches!(new_click.kind(), Kind::Double) {
                        state.current = self.default;
                        state.cache.clear();

                        shell.publish((self.f)(state.current));
                    }
                    state.last_click = Some(new_click);

                    return Status::Captured;
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) if state.dragging.is_some() => {
                    state.cache.clear();
                    state.dragging = None;
                    return Status::Captured;
                }
                mouse::Event::CursorMoved {
                    position: Point { y, .. },
                } => {
                    if let Some(last) = state.dragging {
                        let diff = (last - y) * (self.range.end() - self.range.start()) * 0.005;

                        state.cache.clear();
                        state.current =
                            (state.current + diff).clamp(*self.range.start(), *self.range.end());
                        state.dragging = Some(y);
                        state.hovering = cursor
                            .position()
                            .is_some_and(|pos| pos.distance(bounds.center()) < RADIUS);

                        shell.publish((self.f)(state.current));

                        return Status::Captured;
                    } else if cursor
                        .position()
                        .is_some_and(|pos| pos.distance(bounds.center()) < RADIUS)
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
                mouse::Event::WheelScrolled { delta }
                    if state.dragging.is_none()
                        && cursor
                            .position()
                            .is_some_and(|pos| pos.distance(bounds.center()) < RADIUS) =>
                {
                    let diff = match delta {
                        ScrollDelta::Lines { y, .. } => y,
                        ScrollDelta::Pixels { y, .. } => y / SWM,
                    } * 0.05
                        * (self.range.end() - self.range.start());

                    state.cache.clear();
                    state.current =
                        (state.current + diff).clamp(*self.range.start(), *self.range.end());

                    shell.publish((self.f)(state.current));

                    return Status::Captured;
                }
                _ => {}
            },
            _ => {}
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

        if state
            .last_theme
            .borrow()
            .as_ref()
            .is_none_or(|last_theme| last_theme != theme)
        {
            state.cache.clear();
            state.last_theme.borrow_mut().replace(theme.clone());
        }

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

impl<Message> Knob<Message> {
    pub fn new(
        range: RangeInclusive<f32>,
        zero: f32,
        default: f32,
        f: impl Fn(f32) -> Message + 'static,
    ) -> Self {
        Self {
            range,
            zero,
            default,
            enabled: true,
            f: Box::new(f),
        }
    }

    pub fn set_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn fill_canvas(&self, state: &State, frame: &mut Frame, theme: &Theme) {
        let center = frame.center();

        let circle = |angle: Radians, a_m: f32, r_m: f32| {
            Path::circle(
                Point::new(
                    (RADIUS * a_m).mul_add(angle.0.cos(), center.x),
                    (RADIUS * a_m).mul_add(angle.0.sin(), center.y),
                ),
                RADIUS * r_m,
            )
        };

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

        let arc = Path::new(|builder| {
            builder.arc(Arc {
                center,
                radius: RADIUS,
                start_angle,
                end_angle,
            });
            builder.line_to(center);
            builder.close();
        });

        let color = if !self.enabled || state.hovering || state.dragging.is_some() {
            theme.extended_palette().secondary.strong.color
        } else {
            theme.extended_palette().primary.base.color
        };

        frame.fill(&arc, theme.extended_palette().secondary.base.text);

        frame.fill(&Path::circle(center, RADIUS * 0.8), color);

        frame.fill(
            &circle(start_angle, 0.9, 0.1),
            theme.extended_palette().secondary.base.text,
        );

        frame.fill(
            &circle(end_angle, 0.9, 0.1),
            theme.extended_palette().secondary.base.text,
        );

        frame.fill(
            &circle(end_angle, 0.4, 0.15),
            theme.extended_palette().secondary.base.text,
        );
    }
}

impl<'a, Message> From<Knob<Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(knob: Knob<Message>) -> Self {
        Self::new(knob)
    }
}
