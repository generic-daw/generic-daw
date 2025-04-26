use super::{LINE_HEIGHT, SWM, get_time};
use generic_daw_core::{Meter, Position};
use generic_daw_utils::{NoDebug, Vec2};
use iced::{
    Background, Color, Element, Event, Fill, Font, Length, Point, Rectangle, Renderer, Size, Theme,
    Transformation, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Text, Widget,
        layout::{Limits, Node},
        overlay,
        renderer::{Quad, Style},
        text::Renderer as _,
        widget::{Operation, Tree, tree},
    },
    alignment::Vertical,
    border,
    mouse::{self, Cursor, Interaction, ScrollDelta},
    padding,
    widget::text::{Alignment, LineHeight, Shaping, Wrapping},
};
use std::sync::atomic::Ordering::Acquire;

#[derive(Default)]
struct State {
    hovering: bool,
    seeking: Option<Position>,
}

#[derive(Debug)]
pub struct Seeker<'a, Message> {
    meter: &'a Meter,
    position: &'a Vec2,
    scale: &'a Vec2,
    offset: f32,
    children: NoDebug<[Element<'a, Message>; 2]>,
    seek_to: fn(Position) -> Message,
    position_scale_delta: fn(Vec2, Vec2, Size) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for Seeker<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Fill, Fill)
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&*self.children);
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let left = self.children[0].as_widget().layout(
            &mut tree.children[0],
            renderer,
            &Limits::new(limits.min(), Size::new(limits.max().width, f32::INFINITY)),
        );
        let left_width = left.size().width;

        let right = self.children[1].as_widget().layout(
            &mut tree.children[1],
            renderer,
            &Limits::new(
                limits.min(),
                Size::new(limits.max().width - left_width, f32::INFINITY),
            ),
        );

        Node::with_children(
            limits.max(),
            vec![
                left.translate(Vector::new(
                    0.0,
                    self.position.y.mul_add(-self.scale.y, LINE_HEIGHT),
                )),
                right.translate(Vector::new(
                    left_width,
                    self.position.y.mul_add(-self.scale.y, LINE_HEIGHT),
                )),
            ],
        )
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
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds().shrink(padding::top(LINE_HEIGHT));
        let right_panel_bounds = Self::right_panel_bounds(layout);

        self.children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .for_each(|((child, tree), layout)| {
                let Some(viewport) = bounds.intersection(&layout.bounds()) else {
                    return;
                };

                child.as_widget_mut().update(
                    tree, event, layout, cursor, renderer, clipboard, shell, &viewport,
                );
            });

        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();

        let Some(mut cursor) = cursor.position_in(right_panel_bounds) else {
            if state.hovering {
                state.hovering = false;
                shell.request_redraw();
            }

            if state.seeking.is_some() {
                state.seeking = None;
                shell.request_redraw();
            }

            return;
        };
        cursor.y -= LINE_HEIGHT;

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::CursorMoved { modifiers, .. } => {
                    if let Some(last_time) = state.seeking {
                        let time = get_time(
                            cursor.x + self.offset,
                            *modifiers,
                            self.meter,
                            self.position,
                            self.scale,
                        );

                        if last_time != time {
                            state.seeking = Some(time);
                            shell.publish((self.seek_to)(time));
                            shell.capture_event();
                        }
                    } else if (cursor.y < 0.0) != state.hovering {
                        state.hovering ^= true;
                        shell.request_redraw();
                    }
                }
                mouse::Event::ButtonPressed {
                    button: mouse::Button::Left,
                    modifiers,
                } if state.hovering => {
                    let time =
                        get_time(cursor.x, *modifiers, self.meter, self.position, self.scale);
                    state.seeking = Some(time);
                    shell.publish((self.seek_to)(time));
                    shell.capture_event();
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) => {
                    state.seeking = None;
                    shell.request_redraw();
                }
                mouse::Event::WheelScrolled { delta, modifiers } => {
                    let (mut x, mut y) = match *delta {
                        ScrollDelta::Pixels { x, y } => (-x, -y),
                        ScrollDelta::Lines { x, y } => (-x * SWM, -y * SWM),
                    };

                    match (modifiers.control(), modifiers.shift(), modifiers.alt()) {
                        (false, false, false) => {
                            x *= self.scale.x.exp2();
                            y /= self.scale.y;

                            shell.publish((self.position_scale_delta)(
                                Vec2::new(x, y),
                                Vec2::ZERO,
                                layout.bounds().size(),
                            ));
                            shell.capture_event();
                        }
                        (true, false, false) => {
                            x = y / 128.0;

                            let x_pos =
                                cursor.x * (self.scale.x.exp2() - (self.scale.x + x).exp2());

                            shell.publish((self.position_scale_delta)(
                                Vec2::new(x_pos, 0.0),
                                Vec2::new(x, 0.0),
                                layout.bounds().size(),
                            ));
                            shell.capture_event();
                        }
                        (false, true, false) => {
                            y *= 4.0 * self.scale.x.exp2();

                            shell.publish((self.position_scale_delta)(
                                Vec2::new(y, 0.0),
                                Vec2::ZERO,
                                layout.bounds().size(),
                            ));
                            shell.capture_event();
                        }
                        (false, false, true) => {
                            y /= -8.0;

                            let y_pos = (cursor.y * y) / (self.scale.y * (self.scale.y + y));

                            shell.publish((self.position_scale_delta)(
                                Vec2::new(0.0, y_pos),
                                Vec2::new(0.0, y),
                                layout.bounds().size(),
                            ));
                            shell.capture_event();
                        }
                        _ => {}
                    }
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
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds().shrink(padding::top(LINE_HEIGHT));
        let right_panel_bounds = Self::right_panel_bounds(layout);
        let right_child_bounds = right_panel_bounds.shrink(padding::top(LINE_HEIGHT));

        renderer.with_layer(right_child_bounds, |renderer| {
            self.grid(renderer, right_child_bounds, theme);
        });

        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .for_each(|((child, tree), layout)| {
                let Some(viewport) = bounds.intersection(&layout.bounds()) else {
                    return;
                };

                renderer.with_layer(viewport, |renderer| {
                    child
                        .as_widget()
                        .draw(tree, renderer, theme, style, layout, cursor, &viewport);
                });
            });

        let seeker_bounds = Self::seeker_bounds(layout);
        let bpm = self.meter.bpm.load(Acquire);

        renderer.start_layer(right_panel_bounds);

        renderer.fill_quad(
            Quad {
                bounds: seeker_bounds,
                ..Quad::default()
            },
            theme.extended_palette().primary.base.color,
        );

        let sample_size = self.scale.x.exp2();

        let x =
            (self.meter.sample.load(Acquire) as f32 - self.position.x) / sample_size - self.offset;

        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(
                    seeker_bounds.position() + Vector::new(x, 0.0),
                    Size::new(1.5, right_panel_bounds.height),
                ),
                ..Quad::default()
            },
            theme.extended_palette().primary.base.color,
        );

        let mut draw_text = |beat: Position, bar: u32| {
            let x =
                (beat.in_samples_f(bpm, self.meter.sample_rate) - self.position.x) / sample_size;

            let bar = Text {
                content: (bar + 1).to_string(),
                bounds: Size::new(f32::INFINITY, 0.0),
                size: renderer.default_size(),
                line_height: LineHeight::default(),
                font: Font::MONOSPACE,
                align_x: Alignment::Left,
                align_y: Vertical::Top,
                shaping: Shaping::Basic,
                wrapping: Wrapping::None,
            };

            renderer.fill_text(
                bar,
                seeker_bounds.position() + Vector::new(x + 3.0, 0.0),
                theme.extended_palette().primary.base.text,
                seeker_bounds,
            );
        };

        let numerator = self.meter.numerator.load(Acquire);

        let mut beat = Position::from_samples_f(self.position.x, bpm, self.meter.sample_rate);
        let end_beat = beat
            + Position::from_samples_f(
                seeker_bounds.width * sample_size,
                bpm,
                self.meter.sample_rate,
            );
        beat = beat.floor();

        while beat <= end_beat {
            let bar = beat.beat() / u32::from(numerator);

            if self.scale.x >= 11.0 {
                if beat.beat() % u32::from(numerator) == 0 && bar % 4 == 0 {
                    draw_text(beat, bar);
                }
            } else if beat.beat() % u32::from(numerator) == 0 {
                draw_text(beat, bar);
            }

            beat += Position::BEAT;
        }

        renderer.fill_quad(
            Quad {
                bounds: right_panel_bounds,
                border: border::width(1.0).color(theme.extended_palette().background.strong.color),
                ..Quad::default()
            },
            Background::Color(Color::TRANSPARENT),
        );

        renderer.end_layer();
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        _viewport: &Rectangle,
        renderer: &Renderer,
    ) -> Interaction {
        let state = tree.state.downcast_ref::<State>();

        if state.hovering || state.seeking.is_some() {
            Interaction::ResizingHorizontally
        } else {
            let bounds = layout.bounds().shrink(padding::top(LINE_HEIGHT));

            self.children
                .iter()
                .zip(&tree.children)
                .zip(layout.children())
                .filter_map(|((child, tree), layout)| {
                    let viewport = bounds.intersection(&layout.bounds())?;

                    Some(
                        child
                            .as_widget()
                            .mouse_interaction(tree, layout, cursor, &viewport, renderer),
                    )
                })
                .max()
                .unwrap_or_default()
        }
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'_>,
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

impl<'a, Message> Seeker<'a, Message> {
    pub fn new(
        meter: &'a Meter,
        position: &'a Vec2,
        scale: &'a Vec2,
        left: impl Into<Element<'a, Message>>,
        right: impl Into<Element<'a, Message>>,
        seek_to: fn(Position) -> Message,
        position_scale_delta: fn(Vec2, Vec2, Size) -> Message,
    ) -> Self {
        Self {
            meter,
            position,
            scale,
            offset: 0.0,
            children: [left.into(), right.into()].into(),
            seek_to,
            position_scale_delta,
        }
    }

    pub fn with_offset(mut self, offset: f32) -> Self {
        self.offset = offset / self.scale.x.exp2();
        self
    }

    fn seeker_bounds(layout: Layout<'_>) -> Rectangle {
        let bounds = layout.bounds();
        let right_child_bounds = layout.children().next_back().unwrap().bounds();

        Rectangle::new(
            Point::new(right_child_bounds.x, bounds.y),
            Size::new(right_child_bounds.width, LINE_HEIGHT),
        )
    }

    fn right_panel_bounds(layout: Layout<'_>) -> Rectangle {
        let bounds = layout.bounds();
        let right_child_bounds = layout.children().next_back().unwrap().bounds();

        Rectangle::new(
            Point::new(right_child_bounds.x, bounds.y),
            Size::new(right_child_bounds.width, bounds.height),
        )
    }

    fn grid(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme) {
        renderer.start_transformation(Transformation::translate(bounds.x, bounds.y));

        let numerator = self.meter.numerator.load(Acquire);
        let bpm = self.meter.bpm.load(Acquire);
        let sample_size = self.scale.x.exp2();

        let mut beat = Position::from_samples_f(self.position.x, bpm, self.meter.sample_rate);
        let end_beat = beat
            + Position::from_samples_f(bounds.width * sample_size, bpm, self.meter.sample_rate);

        let mut background_beat = Position::new(beat.beat() & !0x0f, 0);
        let background_step = Position::new(4 * u32::from(numerator), 0);
        let background_width =
            background_step.in_samples_f(bpm, self.meter.sample_rate) / sample_size;

        while background_beat < end_beat {
            if (background_beat.beat() / (4 * u32::from(numerator))) % 2 == 1 {
                let x = (background_beat.in_samples_f(bpm, self.meter.sample_rate)
                    - self.position.x)
                    / sample_size;

                renderer.fill_quad(
                    Quad {
                        bounds: Rectangle::new(
                            Point::new(x, 0.0),
                            Size::new(background_width, bounds.height),
                        ),
                        ..Quad::default()
                    },
                    theme.extended_palette().background.weakest.color,
                );
            }

            background_beat += background_step;
        }

        beat = beat.ceil_to_snap_step(self.scale.x, numerator, bpm);
        let snap_step = Position::snap_step(self.scale.x, numerator, bpm);

        while beat <= end_beat {
            let color = if snap_step >= Position::BEAT {
                if beat.beat() % (snap_step.beat() * u32::from(numerator)) == 0 {
                    theme.extended_palette().background.strong.color
                } else {
                    theme.extended_palette().background.weak.color
                }
            } else if beat.step() == 0 {
                theme.extended_palette().background.strong.color
            } else {
                theme.extended_palette().background.weak.color
            };

            let x =
                (beat.in_samples_f(bpm, self.meter.sample_rate) - self.position.x) / sample_size;

            renderer.fill_quad(
                Quad {
                    bounds: Rectangle::new(Point::new(x, 0.0), Size::new(1.0, bounds.height)),
                    ..Quad::default()
                },
                color,
            );

            beat += snap_step;
        }

        let offset = self.position.y.fract() * self.scale.y;

        let rows = (bounds.height / self.scale.y) as usize + 1;

        for i in 0..=rows {
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle::new(
                        Point::new(0.0, (i as f32).mul_add(self.scale.y, -offset) - 0.5),
                        Size::new(bounds.width, 1.0),
                    ),
                    ..Quad::default()
                },
                theme.extended_palette().background.strong.color,
            );
        }

        renderer.end_transformation();
    }
}

impl<'a, Message> From<Seeker<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(value: Seeker<'a, Message>) -> Self {
        Self::new(value)
    }
}
