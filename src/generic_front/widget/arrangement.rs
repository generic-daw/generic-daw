use crate::{
    generic_back::{Arrangement, Position},
    generic_front::{TimelineMessage, TimelinePosition, TimelineScale},
};
use iced::{
    advanced::{
        graphics::geometry::Renderer as _,
        layout::{Layout, Limits, Node},
        renderer::{Quad, Style},
        widget::{tree, Tree, Widget},
        Clipboard, Renderer as _, Shell,
    },
    event::Status,
    keyboard::{self, Modifiers},
    mouse::{self, Cursor, ScrollDelta},
    widget::canvas::{Cache, Frame, Path, Stroke, Text},
    Event, Length, Pixels, Point, Rectangle, Renderer, Size, Theme,
};
use std::sync::{atomic::Ordering::SeqCst, Arc};

#[derive(Eq, PartialEq, Default)]
enum Action {
    #[default]
    None,
    DraggingPlayhead,
}

#[derive(Default)]
pub struct State {
    /// information about the position of the timeline viewport
    pub position: TimelinePosition,
    /// information about the scale of the timeline viewport
    pub scale: TimelineScale,
    /// caches the geometry of the grid
    grid_cache: Cache,
    /// the current modifiers
    modifiers: Modifiers,
    /// the current action
    action: Action,
}

impl Widget<TimelineMessage, Theme, Renderer> for Arc<Arrangement> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&self, _tree: &mut Tree, _renderer: &Renderer, limits: &Limits) -> Node {
        Node::new(Size::new(limits.max().width, limits.max().height))
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, TimelineMessage>,
        _viewport: &Rectangle,
    ) -> Status {
        let state = tree.state.downcast_mut::<State>();

        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.modifiers = modifiers;
            return Status::Ignored;
        }

        if !cursor.is_over(layout.bounds()) {
            state.action = Action::None;
            return Status::Ignored;
        }

        let bounds = layout.bounds();

        match (
            state.modifiers.command(),
            state.modifiers.shift(),
            state.modifiers.alt(),
        ) {
            (false, false, false) => {
                if let Event::Mouse(event) = event {
                    match event {
                        mouse::Event::WheelScrolled { delta } => {
                            let (x, y) = match delta {
                                ScrollDelta::Pixels { x, y } => (x * 2.0, y * 4.0),
                                ScrollDelta::Lines { x, y } => (x * 100.0, y * 200.0),
                            };

                            let x = x
                                .mul_add(-state.scale.x.exp2(), state.position.x)
                                .clamp(0.0, self.len().in_interleaved_samples(&self.meter) as f32);
                            let y = (y / state.scale.y).mul_add(-0.5, state.position.y).clamp(
                                0.0,
                                self.tracks.read().unwrap().len().saturating_sub(1) as f32,
                            );

                            state.position.x = x;
                            state.position.y = y;

                            state.grid_cache.clear();
                            return Status::Captured;
                        }
                        mouse::Event::ButtonPressed(mouse::Button::Left) => {
                            let position = cursor.position_in(bounds);
                            if let Some(position) = position {
                                if position.y < 16.0 {
                                    let time =
                                        position.x.mul_add(state.scale.x.exp2(), state.position.x);
                                    self.meter.global_time.store(time as u32, SeqCst);
                                    state.action = Action::DraggingPlayhead;
                                    return Status::Captured;
                                }
                            }
                        }
                        mouse::Event::CursorMoved { .. } => match state.action {
                            Action::DraggingPlayhead => {
                                let position = cursor.position_in(bounds).unwrap();
                                let time =
                                    position.x.mul_add(state.scale.x.exp2(), state.position.x);
                                self.meter.global_time.store(time as u32, SeqCst);
                                return Status::Captured;
                            }
                            Action::None => {}
                        },
                        mouse::Event::ButtonReleased(mouse::Button::Left) => {
                            if state.action != Action::None {
                                state.action = Action::None;
                                return Status::Captured;
                            }
                        }
                        _ => {}
                    }
                }
            }
            (true, false, false) => {
                if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
                    let x = match delta {
                        ScrollDelta::Pixels { x: _, y } => -y * 0.01,
                        ScrollDelta::Lines { x: _, y } => -y * 0.5,
                    };

                    let x = (x + state.scale.x).clamp(3.0, 12.999_999);

                    state.scale.x = x;

                    state.grid_cache.clear();
                    return Status::Captured;
                }
            }
            (false, true, false) => {
                if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
                    let y = match delta {
                        ScrollDelta::Pixels { x: _, y } => y * 0.1,
                        ScrollDelta::Lines { x: _, y } => y * 10.0,
                    };

                    let y = (y + state.scale.y).clamp(36.0, 200.0);

                    state.scale.y = y;

                    return Status::Captured;
                }
            }
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
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();

        renderer.with_layer(bounds, |renderer| {
            renderer.draw_geometry(state.grid_cache.draw(renderer, bounds.size(), |frame| {
                frame.with_clip(bounds, |frame| {
                    self.grid(frame, theme, state);
                });
            }));
        });

        {
            let mut bounds = bounds;
            bounds.y += 16.0;
            bounds.height -= 16.0;

            renderer.with_layer(bounds, |renderer| {
                self.tracks
                    .read()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .for_each(|(i, track)| {
                        let track_bounds = Rectangle::new(
                            Point::new(
                                bounds.x,
                                ((i as f32) - state.position.y).mul_add(state.scale.y, bounds.y),
                            ),
                            Size::new(bounds.width, state.scale.y),
                        );
                        if track_bounds.intersects(&bounds) {
                            track.draw(renderer, theme, track_bounds, bounds, state);
                        }
                    });
            });
        }

        renderer.with_layer(bounds, |renderer| {
            self.playhead(renderer, bounds, theme, state);
        });
    }
}

impl Arrangement {
    fn grid(&self, frame: &mut Frame, theme: &Theme, state: &State) {
        let bounds = frame.size();
        let numerator = self.meter.numerator.load(SeqCst);

        let mut beat = Position::from_interleaved_samples(state.position.x as u32, &self.meter);
        if beat.sub_quarter_note != 0 {
            beat.sub_quarter_note = 0;
            beat.quarter_note += 1;
        }

        let mut end_beat = beat
            + Position::from_interleaved_samples(
                (bounds.width * state.scale.x.exp2()) as u32,
                &self.meter,
            );
        end_beat.sub_quarter_note = 0;

        while beat <= end_beat {
            let bar = beat.quarter_note / u16::from(numerator);
            let color = if state.scale.x.exp2() > 11f32.exp2() {
                if beat.quarter_note % u16::from(numerator) == 0 {
                    if bar % 4 == 0 {
                        theme.extended_palette().secondary.strong.color
                    } else {
                        theme.extended_palette().secondary.weak.color
                    }
                } else {
                    beat.quarter_note += 1;
                    continue;
                }
            } else if beat.quarter_note % u16::from(numerator) == 0 {
                theme.extended_palette().secondary.strong.color
            } else {
                theme.extended_palette().secondary.weak.color
            };

            let x = (beat.in_interleaved_samples(&self.meter) as f32 - state.position.x)
                / state.scale.x.exp2();

            let path = Path::new(|path| {
                path.line_to(Point::new(x, 16.0));
                path.line_to(Point::new(x, bounds.height));
            });

            frame.stroke(&path, Stroke::default().with_color(color));

            if state.scale.x.exp2() > 11f32.exp2() {
                if bar % 4 == 0 {
                    let bar = Text {
                        content: format!("{}", bar + 1),
                        position: Point::new(x + 2.0, 2.0),
                        color: theme.extended_palette().secondary.base.text,
                        size: Pixels(12.0),
                        ..Default::default()
                    };
                    frame.fill_text(bar);
                }
            } else if beat.quarter_note % u16::from(numerator) == 0 {
                let bar = Text {
                    content: format!("{}", bar + 1),
                    position: Point::new(x + 2.0, 2.0),
                    color: theme.extended_palette().secondary.base.text,
                    size: Pixels(12.0),
                    ..Default::default()
                };
                frame.fill_text(bar);
            }

            beat.quarter_note += 1;
        }
    }

    fn playhead(&self, renderer: &mut Renderer, bounds: Rectangle, theme: &Theme, state: &State) {
        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(bounds.position(), Size::new(bounds.width, 16.0)),
                ..Quad::default()
            },
            theme.extended_palette().primary.base.color,
        );

        let mut frame = Frame::new(renderer, bounds.size());

        let path = Path::new(|path| {
            let x = (self.meter.global_time.load(SeqCst) as f32 - state.position.x)
                / state.scale.x.exp2();
            path.line_to(Point::new(x, 0.0));
            path.line_to(Point::new(x, bounds.height));
        });

        frame.with_clip(bounds, |frame| {
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(theme.extended_palette().primary.base.color)
                    .with_width(2.0),
            );
        });

        renderer.draw_geometry(frame.into_geometry());
    }
}
