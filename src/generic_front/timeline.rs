use crate::generic_back::{arrangement::Arrangement, track_clip::audio_clip::AudioClip};
use iced::widget::{canvas, Canvas};
use iced::{Element, Length, Sandbox};
use std::sync::{Arc, Mutex};

// use super::Message;

#[derive(Debug, Clone)]
pub enum TimelineMessage {
    // Add specific messages for Timeline here
    UpdateWaveforms,
    ArrangementUpdated, // ... other timeline-specific messages
}

pub struct Timeline {
    arrangement: Arc<Mutex<Arrangement>>,
    waveforms: Vec<Vec<f32>>,
}

impl Timeline {
    pub fn new(arrangement: Arc<Mutex<Arrangement>>) -> Self {
        let mut timeline = Self {
            arrangement,
            waveforms: Vec::new(),
        };
        timeline.update_waveforms();
        timeline
    }

    pub fn update_waveforms(&mut self) {
        self.waveforms.clear();
        let arrangement = self.arrangement.lock().unwrap();

        for track in arrangement.tracks() {
            let track = track.lock().unwrap();
            for clip in track.clips() {
                if let Some(audio_clip) = clip.as_any().downcast_ref::<AudioClip>() {
                    let waveform: Vec<f32> = audio_clip
                        .audio()
                        .samples()
                        .iter()
                        .step_by(100)
                        .copied()
                        .collect();
                    self.waveforms.push(waveform);
                }
            }
        }
    }
}

impl Sandbox for Timeline {
    type Message = TimelineMessage;

    fn new() -> Self {
        Timeline::new(Arc::new(Mutex::new(Arrangement::new())))
    }

    fn title(&self) -> String {
        String::from("Timeline")
    }

    fn update(&mut self, message: TimelineMessage) {
        match message {
            TimelineMessage::UpdateWaveforms => self.update_waveforms(),
            TimelineMessage::ArrangementUpdated => self.update_waveforms(),
            // ... handle other timeline-specific messages
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
        for (i, waveform) in self.waveforms.iter().enumerate() {
            let y_offset = i as f32 * 100.0;
            let path = iced::widget::canvas::Path::new(|path| {
                for (x, sample) in waveform.iter().enumerate() {
                    let x_pos = x as f32;
                    let y_pos = y_offset + (*sample * 100.0);
                    path.line_to(iced::Point::new(x_pos, y_pos));
                }
            });
            frame.stroke(&path, iced::widget::canvas::Stroke::default());
        }
        vec![frame.into_geometry()]
    }
}
