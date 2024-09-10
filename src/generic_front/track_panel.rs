use crate::generic_back::arrangement::Arrangement;
use iced::{
    widget::{column, container, row, slider, text},
    Element, Length,
};
use std::sync::{Arc, RwLock};

pub struct TrackPanel {
    arrangement: Arc<RwLock<Arrangement>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    ArrangementUpdated,
    TrackVolumeChanged(usize, f32),
}

impl TrackPanel {
    pub const fn new(arrangement: Arc<RwLock<Arrangement>>) -> Self {
        Self { arrangement }
    }

    #[expect(clippy::unused_self, clippy::needless_pass_by_ref_mut)]
    pub fn update(&mut self, message: &Message) {
        match message {
            Message::ArrangementUpdated => {
                // Handle arrangement updates, if necessary
                // For example, you could trigger a re-render or refresh here
            }
            Message::TrackVolumeChanged(track_index, volume) => {
                if let Some(track) = self
                    .arrangement
                    .write()
                    .unwrap()
                    .tracks
                    .get_mut(*track_index)
                {
                    track.write().unwrap().set_volume(*volume);
                }
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let tracks = self
            .arrangement
            .read()
            .unwrap()
            .tracks
            .iter()
            .enumerate()
            .fold(column![].spacing(20), |col, (index, track)| {
                let track_name = format!("Track {}", index + 1);
                let volume = track.read().unwrap().get_volume(); // Get current volume
                col.push(row![
                    text(track_name),
                    slider(0.0..=1.0, volume, move |v| {
                        Message::TrackVolumeChanged(index, v) // Handle volume change
                    })
                ])
            })
            .padding(20);

        container(tracks)
            .width(Length::Shrink)
            .height(Length::Fill)
            .into()
    }
}
