use crate::generic_back::arrangement::Arrangement;
use iced::{
    widget::{canvas, Canvas},
    Element, Length, Sandbox,
};
use itertools::Itertools;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub enum TimelineMessage {
    ArrangementUpdated,
    XScaleChanged(usize),
    YScaleChanged(usize),
}

pub struct Timeline {
    arrangement: Arc<RwLock<Arrangement>>,
    pub timeline_x_scale: usize,
    pub timeline_y_scale: usize,
}

impl Timeline {
    pub fn new(arrangement: Arc<RwLock<Arrangement>>) -> Self {
        Self {
            arrangement,
            timeline_x_scale: 100,
            timeline_y_scale: 50,
        }
    }
}

impl Sandbox for Timeline {
    type Message = TimelineMessage;

    fn new() -> Self {
        unimplemented!()
    }

    fn title(&self) -> String {
        String::from("Timeline")
    }

    fn update(&mut self, message: TimelineMessage) {
        match message {
            TimelineMessage::ArrangementUpdated => {}
            TimelineMessage::XScaleChanged(x_scale) => {
                self.timeline_x_scale = x_scale;
            }
            TimelineMessage::YScaleChanged(y_scale) => {
                self.timeline_y_scale = y_scale;
            }
        }
    }

    fn view(&self) -> Element<TimelineMessage> {
        Element::from(Canvas::new(self).width(Length::Fill).height(Length::Fill))
    }
}

impl canvas::Program<TimelineMessage> for Timeline {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        let mut frame = iced::widget::canvas::Frame::new(renderer, bounds.size());
        let meter = self.arrangement.read().unwrap().meter.clone();

        self.arrangement
            .read()
            .unwrap()
            .tracks
            .iter()
            .enumerate()
            .for_each(|(i, track)| {
                let track = track.read().unwrap();
                let y_offset = i * (self.timeline_y_scale * 2) + self.timeline_y_scale;
                let width = frame.width() as usize;

                track.clips.iter().for_each(|clip| {
                    (clip.get_global_start().in_interleaved_samples(&meter)
                        ..clip.get_global_end().in_interleaved_samples(&meter))
                        .chunks(self.timeline_x_scale)
                        .into_iter()
                        .enumerate()
                        .filter(|(x, _)| *x <= width)
                        .for_each(|(x, samples_group)| {
                            let path = iced::widget::canvas::Path::new(|path| {
                                let (a, b) = samples_group
                                    .map(|global_time| {
                                        track.get_at_global_time(global_time, &meter)
                                    })
                                    .minmax()
                                    .into_option()
                                    .unwrap();

                                path.line_to(iced::Point::new(
                                    x as f32,
                                    a.mul_add(self.timeline_y_scale as f32, y_offset as f32),
                                ));
                                path.line_to(iced::Point::new(
                                    x as f32,
                                    b.mul_add(self.timeline_y_scale as f32, y_offset as f32),
                                ));
                            });
                            frame.stroke(&path, iced::widget::canvas::Stroke::default());
                        });
                });
            });

        vec![frame.into_geometry()]
    }
}