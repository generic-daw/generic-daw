use crate::generic_back::Arrangement;
use iced::{border::Radius, mouse::ScrollDelta, widget::container, Element, Theme};
use std::sync::{atomic::Ordering::SeqCst, Arc};

#[derive(Debug, Clone)]
pub enum TimelineMessage {
    Tick,
    XScaleChanged(f32),
    YScaleChanged(f32),
    Scrolled(ScrollDelta),
    MovePlayToStart,
}

pub struct Timeline {
    pub arrangement: Arc<Arrangement>,
}

impl Timeline {
    pub const fn new(arrangement: Arc<Arrangement>) -> Self {
        Self { arrangement }
    }

    pub fn update(&mut self, message: &TimelineMessage) {
        match message {
            TimelineMessage::XScaleChanged(x_scale) => {
                self.arrangement.scale.x.store(*x_scale, SeqCst);
                self.update(&TimelineMessage::Tick);
            }
            TimelineMessage::YScaleChanged(y_scale) => {
                self.arrangement.scale.y.store(*y_scale, SeqCst);
                self.update(&TimelineMessage::Tick);
            }
            TimelineMessage::Tick => {}
            TimelineMessage::Scrolled(delta) => match *delta {
                ScrollDelta::Pixels { x, y } => {
                    let x = x
                        .mul_add(
                            -self.arrangement.scale.x.load(SeqCst).exp2(),
                            self.arrangement.position.x.load(SeqCst),
                        )
                        .clamp(
                            0.0,
                            self.arrangement
                                .len()
                                .in_interleaved_samples(&self.arrangement.meter)
                                as f32,
                        );
                    self.arrangement.position.x.store(x, SeqCst);

                    let y = (y / self.arrangement.scale.y.load(SeqCst))
                        .mul_add(-0.5, self.arrangement.position.y.load(SeqCst))
                        .clamp(
                            0.0,
                            self.arrangement
                                .tracks
                                .read()
                                .unwrap()
                                .len()
                                .saturating_sub(1) as f32,
                        );
                    self.arrangement.position.y.store(y, SeqCst);
                }
                ScrollDelta::Lines { x, y } => {
                    self.update(&TimelineMessage::Scrolled(ScrollDelta::Pixels {
                        x: x * 50.0,
                        y: y * 50.0,
                    }));
                }
            },
            TimelineMessage::MovePlayToStart => {
                self.arrangement.position.x.store(
                    self.arrangement.meter.global_time.load(SeqCst) as f32,
                    SeqCst,
                );
                self.update(&TimelineMessage::Tick);
            }
        }
    }

    pub fn view(&self) -> Element<'_, TimelineMessage> {
        container(Element::new(self.arrangement.clone()))
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
}
