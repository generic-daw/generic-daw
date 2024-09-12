use crate::generic_back::{arrangement::Arrangement, position::Position};
use iced::{
    advanced::{layout::Node, Layout},
    border::Radius,
    mouse::{Cursor, ScrollDelta},
    widget::{
        canvas::{self, Cache, Frame, Geometry, Path, Stroke},
        container, Canvas,
    },
    Element, Length, Point, Rectangle, Renderer, Size, Theme,
};
use std::sync::{
    atomic::Ordering::SeqCst,
    mpsc::{Receiver, Sender},
    Arc,
};

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    XScaleChanged(f32),
    YScaleChanged(f32),
    Scrolled(ScrollDelta),
    MovePlayToStart,
    ArrangementUpdated,
}

pub struct Timeline {
    pub arrangement: Arc<Arrangement>,
    pub samples_sender: Sender<Message>,
    samples_receiver: Receiver<Message>,
    arrangement_cache: Cache,
}

impl Timeline {
    pub fn new(arrangement: Arc<Arrangement>) -> Self {
        let (samples_sender, samples_receiver) = std::sync::mpsc::channel();
        Self {
            arrangement,
            samples_sender,
            samples_receiver,
            arrangement_cache: Cache::new(),
        }
    }

    pub fn update(&mut self, message: &Message) {
        match message {
            Message::XScaleChanged(x_scale) => {
                self.arrangement.scale.write().unwrap().x = *x_scale;
                self.update(&Message::ArrangementUpdated);
            }
            Message::YScaleChanged(y_scale) => {
                self.arrangement.scale.write().unwrap().y = *y_scale;
                self.update(&Message::ArrangementUpdated);
            }
            Message::Tick => {
                if let Ok(msg) = self.samples_receiver.try_recv() {
                    self.update(&msg);
                }
            }
            Message::ArrangementUpdated => {
                self.arrangement_cache.clear();
            }
            Message::Scrolled(delta) => match *delta {
                ScrollDelta::Pixels { x, y } => {
                    let prev_pos = self.arrangement.position.read().unwrap().clone();
                    let x = x
                        .mul_add(
                            -self.arrangement.scale.read().unwrap().x.exp2(),
                            self.arrangement.position.read().unwrap().x,
                        )
                        .clamp(
                            0.0,
                            self.arrangement
                                .len()
                                .in_interleaved_samples(&self.arrangement.meter)
                                as f32,
                        );
                    self.arrangement.position.write().unwrap().x = x;

                    let y = (self.arrangement.position.read().unwrap().y
                        - y / self.arrangement.scale.read().unwrap().y / 2.0)
                        .clamp(
                            0.0,
                            self.arrangement
                                .tracks
                                .read()
                                .unwrap()
                                .len()
                                .saturating_sub(1) as f32,
                        );
                    self.arrangement.position.write().unwrap().y = y;

                    if *self.arrangement.position.read().unwrap() != prev_pos {
                        self.update(&Message::ArrangementUpdated);
                    }
                }
                ScrollDelta::Lines { x, y } => {
                    self.update(&Message::Scrolled(ScrollDelta::Pixels {
                        x: x * 50.0,
                        y: y * 50.0,
                    }));
                }
            },
            Message::MovePlayToStart => {
                self.arrangement.position.write().unwrap().x =
                    self.arrangement.meter.global_time.load(SeqCst) as f32;
                self.update(&Message::ArrangementUpdated);
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        container(Element::from(
            Canvas::new(self).width(Length::Fill).height(Length::Fill),
        ))
        .style(|_| container::Style {
            border: iced::Border {
                color: Theme::default().extended_palette().secondary.weak.color,
                width: 1.0,
                radius: Radius::new(0.0),
            },
            ..container::Style::default()
        })
        .into()
    }

    fn grid(&self, renderer: &Renderer, theme: &Theme, layout: Layout) -> Geometry {
        let bounds = layout.bounds();

        let mut frame = Frame::new(renderer, bounds.size());

        let mut beat = Position::from_interleaved_samples(
            self.arrangement.position.read().unwrap().x as u32,
            &self.arrangement.meter,
        );
        let mut end_beat = beat
            + Position::from_interleaved_samples(
                (bounds.width * self.arrangement.scale.read().unwrap().x.exp2()) as u32,
                &self.arrangement.meter,
            );
        if beat.sub_quarter_note != 0 {
            beat.sub_quarter_note = 0;
            beat.quarter_note += 1;
        }
        end_beat.sub_quarter_note = 0;

        // grid lines
        while beat <= end_beat {
            let color = if self.arrangement.scale.read().unwrap().x > 11.0 {
                if beat.quarter_note % self.arrangement.meter.numerator.load(SeqCst) == 0 {
                    let bar = beat.quarter_note / self.arrangement.meter.numerator.load(SeqCst);
                    if bar % 4 == 0 {
                        theme.extended_palette().secondary.strong.color
                    } else {
                        theme.extended_palette().secondary.weak.color
                    }
                } else {
                    beat.quarter_note += 1;
                    continue;
                }
            } else if beat.quarter_note % self.arrangement.meter.numerator.load(SeqCst) == 0 {
                theme.extended_palette().secondary.strong.color
            } else {
                theme.extended_palette().secondary.weak.color
            };

            let path = Path::new(|path| {
                let x = (beat.in_interleaved_samples(&self.arrangement.meter) as f32
                    - self.arrangement.position.read().unwrap().x)
                    / self.arrangement.scale.read().unwrap().x.exp2();
                path.line_to(Point::new(x, 0.0));
                path.line_to(Point::new(x, bounds.height));
            });

            frame.with_clip(bounds, |frame| {
                frame.stroke(&path, Stroke::default().with_color(color));
            });
            beat.quarter_note += 1;
        }

        frame.into_geometry()
    }

    fn playhead(&self, renderer: &Renderer, theme: &Theme, layout: Layout) -> Geometry {
        let bounds = layout.bounds();

        let mut frame = Frame::new(renderer, bounds.size());
        let path = Path::new(|path| {
            let x = -(self.arrangement.position.read().unwrap().x)
                / self.arrangement.scale.read().unwrap().x.exp2()
                + self.arrangement.meter.global_time.load(SeqCst) as f32
                    / self.arrangement.scale.read().unwrap().x.exp2();
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
        frame.into_geometry()
    }
}

impl canvas::Program<Message> for Timeline {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        let node = Node::new(Size::new(bounds.width, bounds.height));
        let layout = Layout::new(&node);

        let grid = self.grid(renderer, theme, layout);

        let arrangement = self
            .arrangement_cache
            .draw(renderer, bounds.size(), |frame| {
                self.arrangement.draw(frame, theme, layout);
            });

        let playhead = self.playhead(renderer, theme, layout);

        vec![grid, arrangement, playhead]
    }
}
