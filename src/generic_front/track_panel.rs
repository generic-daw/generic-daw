use crate::generic_back::arrangement::Arrangement;
use iced::{
    widget::{column, container, row, slider, text},
    Element, Length,
};
use std::sync::Arc;

pub struct TrackPanel {
    arrangement: Arc<Arrangement>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TrackVolumeChanged(usize, f32),
}

impl TrackPanel {
    pub const fn new(arrangement: Arc<Arrangement>) -> Self {
        Self { arrangement }
    }

    #[expect(clippy::needless_pass_by_ref_mut)]
    pub fn update(&mut self, message: &Message) {
        match message {
            Message::TrackVolumeChanged(track_index, volume) => {
                if let Some(track) = self.arrangement.tracks.read().unwrap().get(*track_index) {
                    track.set_volume(*volume);
                }
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let tracks = self
            .arrangement
            .tracks
            .read()
            .unwrap()
            .iter()
            .enumerate()
            .fold(column![].spacing(20), |col, (index, track)| {
                let track_name = format!("Track {}", index + 1);
                let volume = track.get_volume(); // Get current volume
                col.push(row![
                    text(track_name),
                    slider(0.0..=1.0, volume, move |v| {
                        Message::TrackVolumeChanged(index, v) // Handle volume change
                    })
                    .step(0.01)
                    .width(Length::Fixed(150.0))
                ])
            })
            .padding(20);

        container(tracks)
            .width(Length::Shrink)
            .height(Length::Fill)
            .into()
    }
}
