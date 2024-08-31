use crate::generic_back::arrangement::Arrangement;
use iced::widget::{canvas, Canvas};
use iced::{Element, Length, Sandbox};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum TimelineMessage {
    ArrangementUpdated,
}

pub struct Timeline {
    arrangement: Arc<Mutex<Arrangement>>,
    timeline_x_scale: usize,
    timeline_y_scale: usize,
}

impl Timeline {
    pub fn new(arrangement: Arc<Mutex<Arrangement>>) -> Self {
        Self {
            arrangement,
            timeline_x_scale: 100,
            timeline_y_scale: 100,
        }
    }
}

impl Sandbox for Timeline {
    type Message = TimelineMessage;

    fn new() -> Self {
        Self::new(Arc::new(Mutex::new(Arrangement::new())))
    }

    fn title(&self) -> String {
        String::from("Timeline")
    }

    fn update(&mut self, message: TimelineMessage) {
        match message {
            TimelineMessage::ArrangementUpdated => {}
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
            .lock()
            .unwrap()
            .tracks()
            .iter()
            .enumerate()
            .for_each(|(i, track)| {
                let path = iced::widget::canvas::Path::new(|path| {
                    let track = track.lock().unwrap();

                    let y_offset = i * (self.timeline_y_scale) + self.timeline_y_scale / 2;
                    (0..track.len())
                        .step_by(self.timeline_x_scale)
                        .enumerate()
                        .for_each(|(x, global_time)| {
                            let y_pos = track
                                .get_at_global_time(global_time)
                                .mul_add(self.timeline_y_scale as f32, y_offset as f32);
                            path.line_to(iced::Point::new(x as f32, y_pos));
                        });
                });

                frame.stroke(&path, iced::widget::canvas::Stroke::default());
            });

        vec![frame.into_geometry()]
    }
}
