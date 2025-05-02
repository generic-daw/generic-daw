use super::{LINE_HEIGHT, SWM};
use generic_daw_utils::NoDebug;
use iced::{
    Element, Event, Length, Point, Radians, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        graphics::geometry::Renderer as _,
        layout::{Limits, Node},
        mouse::{Click, click::Kind},
        overlay,
        renderer::{Quad, Style},
        text,
        widget::{Tree, tree},
    },
    border,
    mouse::{self, Cursor, Interaction, ScrollDelta},
    widget::{
        Text,
        canvas::{Cache, Frame, Path, path::Arc},
    },
    window,
};
use std::{
    cell::RefCell,
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
    last_theme: RefCell<Option<Theme>>,
    cache: Cache,
    last_click: Option<Click>,
}

#[derive(Debug)]
pub struct Knob<'a, Message> {
    range: RangeInclusive<f32>,
    value: f32,
    center: f32,
    reset: f32,
    enabled: bool,
    f: NoDebug<Box<dyn Fn(f32) -> Message>>,
    radius: f32,
    tooltip: Option<NoDebug<Element<'a, Message>>>,
}

impl<Message> Widget<Message, Theme, Renderer> for Knob<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(2.0 * self.radius),
            Length::Fixed(2.0 * self.radius),
        )
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn diff(&self, tree: &mut Tree) {
        if let Some(tooltip) = self.tooltip.as_ref() {
            tree.diff_children(&[&**tooltip]);
        } else {
            tree.children.clear();
        }
    }

    fn children(&self) -> Vec<Tree> {
        self.tooltip
            .as_ref()
            .map(|p| vec![Tree::new(&**p)])
            .unwrap_or_default()
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
        let state = tree.state.downcast_mut::<State>();

        if let Event::Window(window::Event::RedrawRequested(..)) = event {
            if self.enabled != state.last_enabled {
                state.last_enabled = self.enabled;
                state.cache.clear();
            }

            if self.value != state.last_value {
                state.last_value = self.value;
                state.cache.clear();
            }

            return;
        }

        if shell.is_event_captured() {
            return;
        }

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonPressed {
                    button: mouse::Button::Left,
                    ..
                } if state.dragging.is_none() && state.hovering => {
                    let pos = cursor.position().unwrap();
                    state.dragging = Some((self.value, pos.y));

                    let new_click = Click::new(pos, mouse::Button::Left, state.last_click);
                    state.last_click = Some(new_click);

                    if new_click.kind() == Kind::Double {
                        shell.publish((self.f)(self.reset));
                    }

                    shell.capture_event();
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
                    if let Some((value, pos)) = state.dragging {
                        let diff = (pos - y) * (self.range.end() - self.range.start()) * 0.005;

                        shell.publish((self.f)(
                            (value + diff).clamp(*self.range.start(), *self.range.end()),
                        ));
                        shell.capture_event();
                    }

                    if cursor.is_over(layout.bounds()) != state.hovering {
                        state.hovering ^= true;
                        state.cache.clear();
                        shell.request_redraw();
                    }
                }
                mouse::Event::WheelScrolled { delta, .. }
                    if state.dragging.is_none() && state.hovering =>
                {
                    let diff = match delta {
                        ScrollDelta::Lines { y, .. } => *y,
                        ScrollDelta::Pixels { y, .. } => y / SWM,
                    } * (self.range.end() - self.range.start())
                        * 0.05;

                    shell.publish((self.f)(
                        (self.value + diff).clamp(*self.range.start(), *self.range.end()),
                    ));
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

        if state
            .last_theme
            .borrow()
            .as_ref()
            .is_none_or(|last_theme| *last_theme != *theme)
        {
            *state.last_theme.borrow_mut() = Some(theme.clone());
            state.cache.clear();
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

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        let state = tree.state.downcast_ref::<State>();

        if state.hovering || state.dragging.is_some() {
            self.tooltip.as_ref().map(|tooltip| {
                overlay::Element::new(Box::new(Overlay {
                    tooltip,
                    tree: tree.children.iter_mut().next().unwrap(),
                    bounds: layout.bounds() + translation,
                }))
            })
        } else {
            None
        }
    }
}

impl<'a, Message> Knob<'a, Message> {
    pub fn new(
        range: RangeInclusive<f32>,
        value: f32,
        center: f32,
        reset: f32,
        enabled: bool,
        f: impl Fn(f32) -> Message + 'static,
    ) -> Self {
        Self {
            range,
            value,
            center,
            reset,
            enabled,
            f: NoDebug(Box::from(f)),
            radius: LINE_HEIGHT,
            tooltip: None,
        }
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn tooltip(mut self, tooltip: impl text::IntoFragment<'a>) -> Self {
        self.tooltip = Some(NoDebug(Text::new(tooltip).line_height(1.0).into()));
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

impl<'a, Message> From<Knob<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(value: Knob<'a, Message>) -> Self {
        Self::new(value)
    }
}

struct Overlay<'a, 'b, Message> {
    tooltip: &'b Element<'a, Message>,
    tree: &'b mut Tree,
    bounds: Rectangle,
}

impl<Message> overlay::Overlay<Message, Theme, Renderer> for Overlay<'_, '_, Message> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        let padding = 3.0;

        let layout = self.tooltip.as_widget().layout(
            self.tree,
            renderer,
            &Limits::new(Size::ZERO, bounds).shrink(Size::new(padding, padding)),
        );
        let bounds = layout.bounds();

        Node::with_children(
            bounds.expand(padding).size(),
            vec![layout.translate(Vector::new(padding, padding))],
        )
        .translate(Vector::new(
            self.bounds.x + (self.bounds.width - bounds.width) / 2.0 - padding,
            self.bounds.y + self.bounds.height,
        ))
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &Style,
        layout: Layout<'_>,
        cursor: Cursor,
    ) {
        renderer.fill_quad(
            Quad {
                bounds: layout.bounds(),
                border: border::width(1.0)
                    .rounded(2.0)
                    .color(theme.extended_palette().background.strong.color),
                ..Quad::default()
            },
            theme.extended_palette().background.weak.color,
        );

        self.tooltip.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor,
            &Rectangle::with_size(Size::INFINITY),
        );
    }
}
