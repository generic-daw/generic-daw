use super::drawable_clip::{TimelinePosition, TimelineScale};
use crate::generic_back::{arrangement::Arrangement, position::Position};
use iced::{
    mouse::ScrollDelta,
    widget::{
        canvas::{self, Cache},
        Canvas,
    },
    Element, Length,
};
use std::sync::{
    atomic::Ordering::SeqCst,
    mpsc::{Receiver, Sender},
    Arc, RwLock,
};

#[derive(Debug, Clone)]
pub enum Message {
    ArrangementUpdated,
    XScaleChanged(f32),
    YScaleChanged(f32),
    Tick,
    Scrolled(ScrollDelta),
}

pub struct Timeline {
    pub arrangement: Arc<RwLock<Arrangement>>,
    tracks_cache: Cache,
    pub scale: TimelineScale,
    pub position: TimelinePosition,
    pub samples_sender: Sender<Message>,
    samples_receiver: Receiver<Message>,
}

impl Timeline {
    pub fn new(arrangement: Arc<RwLock<Arrangement>>) -> Self {
        let (samples_sender, samples_receiver) = std::sync::mpsc::channel();
        Self {
            arrangement,
            tracks_cache: Cache::new(),
            scale: TimelineScale { x: 8.0, y: 100.0 },
            position: TimelinePosition {
                x: Position::new(0, 0),
                y: 0.0,
            },
            samples_sender,
            samples_receiver,
        }
    }

    pub fn update(&mut self, message: &Message) {
        match message {
            Message::ArrangementUpdated => {
                self.tracks_cache.clear();
            }
            Message::XScaleChanged(x_scale) => {
                self.scale.x = *x_scale;
                self.tracks_cache.clear();
            }
            Message::YScaleChanged(y_scale) => {
                self.scale.y = *y_scale;
                self.tracks_cache.clear();
            }
            Message::Tick => {
                if let Ok(msg) = self.samples_receiver.try_recv() {
                    self.update(&msg);
                }
            }
            Message::Scrolled(delta) => {
                match *delta {
                    ScrollDelta::Pixels { x, y } => {
                        let arrangement = self.arrangement.read().unwrap();

                        if x.abs() > f32::EPSILON {
                            let x = (-x)
                                .mul_add(
                                    self.scale.x.exp2(),
                                    self.position.x.in_interleaved_samples(&arrangement.meter)
                                        as f32,
                                )
                                .clamp(
                                    0.0,
                                    arrangement.len().in_interleaved_samples(&arrangement.meter)
                                        as f32,
                                );
                            self.position.x =
                                Position::from_interleaved_samples(x as u32, &arrangement.meter);
                        }

                        if y.abs() > f32::EPSILON {
                            self.position.y = (self.position.y - y / self.scale.y / 2.0)
                                .clamp(0.0, arrangement.tracks.len().saturating_sub(1) as f32);
                        }
                    }
                    ScrollDelta::Lines { x, y } => {
                        self.update(&Message::Scrolled(ScrollDelta::Pixels {
                            x: x * 50.0,
                            y: y * 50.0,
                        }));
                    }
                }
                self.tracks_cache.clear();
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        Element::from(Canvas::new(self).width(Length::Fill).height(Length::Fill))
    }
}

impl canvas::Program<Message> for Timeline {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        let playlist_clips = self.tracks_cache.draw(renderer, bounds.size(), |frame| {
            self.arrangement
                .read()
                .unwrap()
                .tracks
                .iter()
                .enumerate()
                .for_each(|(i, track)| {
                    let y = i as f32 - self.position.y;

                    track.read().unwrap().clips.iter().for_each(|clip| {
                        clip.draw(
                            frame,
                            self.scale,
                            TimelinePosition {
                                x: self.position.x,
                                y,
                            },
                            &self.arrangement.read().unwrap().meter,
                            theme,
                        );
                    });
                });
        });

        let mut frame = iced::widget::canvas::Frame::new(renderer, bounds.size());
        let path = iced::widget::canvas::Path::new(|path| {
            let x = -(self
                .position
                .x
                .in_interleaved_samples(&self.arrangement.read().unwrap().meter)
                as f32)
                / self.scale.x.exp2()
                + self
                    .arrangement
                    .read()
                    .unwrap()
                    .meter
                    .global_time
                    .load(SeqCst) as f32
                    / self.scale.x.exp2();
            path.line_to(iced::Point::new(x, 0.0));
            path.line_to(iced::Point::new(x, bounds.height));
        });
        frame.stroke(
            &path,
            iced::widget::canvas::Stroke::default()
                .with_color(theme.extended_palette().primary.base.color)
                .with_width(2.0),
        );

        vec![playlist_clips, frame.into_geometry()]
    }
}
