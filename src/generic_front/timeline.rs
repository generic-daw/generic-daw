use crate::generic_back::arrangement::Arrangement;
use iced::{
    widget::{canvas, Canvas},
    Element, Length, Sandbox,
};
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

        self.arrangement
            .read()
            .unwrap()
            .tracks
            .iter()
            .enumerate()
            .for_each(|(i, track)| {
                let y_offset = i * (self.timeline_y_scale * 2) + self.timeline_y_scale;
                let width = frame.width() as usize;

                track.read().unwrap().clips.iter().for_each(|clip| {
                    clip.draw(
                        &mut frame,
                        self.timeline_x_scale,
                        self.timeline_y_scale,
                        width,
                        y_offset,
                        &self.arrangement.read().unwrap().meter,
                    );
                });
            });

        vec![frame.into_geometry()]
    }
}
