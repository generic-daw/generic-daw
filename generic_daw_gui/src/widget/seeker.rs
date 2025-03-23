use super::{LINE_HEIGHT, get_time};
use generic_daw_core::{Meter, Position};
use generic_daw_utils::{NoDebug, Vec2};
use iced::{
    Background, Color, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Text, Widget,
        layout::{Limits, Node},
        renderer::{Quad, Style},
        text::Renderer as _,
        widget::{Tree, tree},
    },
    alignment::Vertical,
    border,
    mouse::{self, Cursor, Interaction},
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
    position: Vec2,
    scale: Vec2,
    left: NoDebug<Element<'a, Message>>,
    right: NoDebug<Element<'a, Message>>,
    seek_to: fn(Position) -> Message,
}

impl<Message> Widget<Message, Theme, Renderer> for Seeker<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&*self.left, &*self.right]);
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&*self.left), Tree::new(&*self.right)]
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let left = self.left.as_widget().layout(
            &mut tree.children[0],
            renderer,
            &Limits::new(limits.min(), Size::new(limits.max().width, f32::INFINITY)),
        );
        let left_width = left.size().width;

        let right = self.right.as_widget().layout(
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

        [&mut self.left, &mut self.right]
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

        let Some(cursor) = cursor.position() else {
            return;
        };

        let state = tree.state.downcast_mut::<State>();

        let seeker_bounds = Self::seeker_bounds(layout);

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::CursorMoved { modifiers, .. } => {
                    if let Some(last_time) = state.seeking {
                        let time = get_time(
                            cursor.x - seeker_bounds.x,
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
                    } else {
                        let hovering = seeker_bounds.contains(cursor);

                        if hovering != state.hovering {
                            state.hovering = hovering;
                            shell.request_redraw();
                        }
                    }
                }
                mouse::Event::ButtonPressed {
                    button: mouse::Button::Left,
                    modifiers,
                } if seeker_bounds.contains(cursor) => {
                    let time = get_time(
                        cursor.x - seeker_bounds.x,
                        *modifiers,
                        self.meter,
                        self.position,
                        self.scale,
                    );
                    state.seeking = Some(time);
                    shell.publish((self.seek_to)(time));
                    shell.capture_event();
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) => {
                    state.seeking = None;
                    shell.request_redraw();
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

        [&self.left, &self.right]
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
        let right_panel_bounds = Self::right_panel_bounds(layout);
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

        let x = (self.meter.sample.load(Acquire) as f32 - self.position.x) / sample_size;

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
            let x = (beat.in_interleaved_samples_f(bpm, self.meter.sample_rate) - self.position.x)
                / sample_size;

            let bar = Text {
                content: itoa::Buffer::new().format(bar + 1).to_owned(),
                bounds: Size::new(f32::INFINITY, 0.0),
                size: renderer.default_size(),
                line_height: LineHeight::default(),
                font: renderer.default_font(),
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

        let mut beat =
            Position::from_interleaved_samples_f(self.position.x, bpm, self.meter.sample_rate)
                .ceil();

        let end_beat = beat
            + Position::from_interleaved_samples_f(
                seeker_bounds.width * sample_size,
                bpm,
                self.meter.sample_rate,
            )
            .floor();

        while beat <= end_beat {
            let bar = beat.beat() / numerator as u32;

            if self.scale.x >= 11.0 {
                if beat.beat() % numerator as u32 == 0 && bar % 4 == 0 {
                    draw_text(beat, bar);
                }
            } else if beat.beat() % numerator as u32 == 0 {
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

            [&self.left, &self.right]
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
}

impl<'a, Message> Seeker<'a, Message> {
    pub fn new(
        meter: &'a Meter,
        position: Vec2,
        scale: Vec2,
        left: impl Into<Element<'a, Message>>,
        right: impl Into<Element<'a, Message>>,
        seek_to: fn(Position) -> Message,
    ) -> Self {
        Self {
            meter,
            position,
            scale,
            left: left.into().into(),
            right: right.into().into(),
            seek_to,
        }
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
}

impl<'a, Message> From<Seeker<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(value: Seeker<'a, Message>) -> Self {
        Self::new(value)
    }
}
