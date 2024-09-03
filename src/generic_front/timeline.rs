use crate::generic_back::arrangement::Arrangement;
use iced::{
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
}

pub struct Timeline {
    pub arrangement: Arc<RwLock<Arrangement>>,
    tracks_cache: Cache,
    pub timeline_x_scale: usize,
    pub timeline_y_scale: usize,
}

impl Timeline {
    pub fn new(arrangement: Arc<RwLock<Arrangement>>) -> Self {
        Self {
            arrangement,
            tracks_cache: Cache::new(),
            timeline_x_scale: 100,
            timeline_y_scale: 50,
        }
    }

    pub fn update(&mut self, message: &Message) {
        match message {
            Message::ArrangementUpdated => {
                self.tracks_cache.clear();
            }
            Message::XScaleChanged(x_scale) => {
                self.timeline_x_scale = *x_scale;
                self.tracks_cache.clear();
            }
            Message::YScaleChanged(y_scale) => {
                self.timeline_y_scale = *y_scale;
                self.tracks_cache.clear();
            }
            Message::Tick => {}
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
                    let y_offset = i * (self.timeline_y_scale * 2) + self.timeline_y_scale;

                    track.read().unwrap().clips.iter().for_each(|clip| {
                        clip.draw(
                            frame,
                            self.timeline_x_scale,
                            self.timeline_y_scale,
                            y_offset,
                            &self.arrangement.read().unwrap().meter,
                            theme,
                        );
                    });
                });
        });

        let mut frame = iced::widget::canvas::Frame::new(renderer, bounds.size());
        let path = iced::widget::canvas::Path::new(|path| {
            let x = self
                .arrangement
                .read()
                .unwrap()
                .meter
                .global_time
                .load(SeqCst) as f32
                / self.timeline_x_scale as f32;
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
