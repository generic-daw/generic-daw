use super::drawable_clip::{Position, Scale};
use crate::generic_back::arrangement::Arrangement;
use iced::{
    mouse::ScrollDelta,
    widget::{
        canvas::{self, Cache},
        Canvas,
    },
    Element, Length,
};
use std::sync::{atomic::Ordering::SeqCst, Arc, RwLock};

#[derive(Debug, Clone)]
pub enum Message {
    ArrangementUpdated,
    XScaleChanged(usize),
    YScaleChanged(usize),
    Tick,
    Scrolled(ScrollDelta),
}

pub struct Timeline {
    pub arrangement: Arc<RwLock<Arrangement>>,
    tracks_cache: Cache,
    pub scale: Scale,
    pub scroll_delta: ScrollDelta,
}

impl Timeline {
    pub fn new(arrangement: Arc<RwLock<Arrangement>>) -> Self {
        Self {
            arrangement,
            tracks_cache: Cache::new(),
            scale: Scale { x: 100, y: 50 },
            scroll_delta: ScrollDelta::Pixels { x: 0.0, y: 0.0 },
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
            Message::Tick => {}
            Message::Scrolled(delta) => {
                if let ScrollDelta::Pixels { x, y } = *delta {
                    if let ScrollDelta::Pixels {
                        x: self_x,
                        y: self_y,
                    } = self.scroll_delta
                    {
                        let arrangement = self.arrangement.read().unwrap();

                        let x = x.mul_add(self.scale.x as f32, self_x).clamp(
                            -(arrangement.len().in_interleaved_samples(&arrangement.meter) as f32),
                            0.0,
                        );
                        let y = (self_y - y).clamp(
                            0.0,
                            (arrangement.tracks.len().saturating_sub(1)) as f32
                                * 2.0
                                * self.scale.y as f32,
                        );
                        drop(arrangement);

                        self.scroll_delta = ScrollDelta::Pixels { x, y };
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
            let x = if let ScrollDelta::Pixels { x, y: _ } = self.scroll_delta {
                -x as usize
            } else {
                0
            };
            self.arrangement
                .read()
                .unwrap()
                .tracks
                .iter()
                .enumerate()
                .for_each(|(i, track)| {
                    let y = isize::try_from(i * (self.scale.y * 2) + self.scale.y).unwrap()
                        - if let ScrollDelta::Pixels { x: _, y } = self.scroll_delta {
                            y as isize
                        } else {
                            0
                        };

                    track.read().unwrap().clips.iter().for_each(|clip| {
                        clip.draw(
                            frame,
                            self.scale,
                            Position { x, y },
                            &self.arrangement.read().unwrap().meter,
                            theme,
                        );
                    });
                });
        });

        let mut frame = iced::widget::canvas::Frame::new(renderer, bounds.size());
        let path = iced::widget::canvas::Path::new(|path| {
            let x = if let ScrollDelta::Pixels { x, y: _ } = self.scroll_delta {
                x / self.scale.x as f32
            } else {
                0.0
            };
            let x = x + self
                .arrangement
                .read()
                .unwrap()
                .meter
                .global_time
                .load(SeqCst) as f32
                / self.scale.x as f32;
            path.line_to(iced::Point::new(x, 0.0));
            path.line_to(iced::Point::new(x, bounds.height));
        });
        frame.stroke(
            &path,
            iced::widget::canvas::Stroke::default()
                .with_color(theme.palette().primary)
                .with_width(2.0),
        );

        vec![playlist_clips, frame.into_geometry()]
    }
}
