use crate::generic_back::Arrangement;
use iced::{
    widget::{column, container, row, slider, text},
    Alignment::Center,
    Element, Length,
};
use std::sync::Arc;

pub struct TrackPanel {
    arrangement: Arc<Arrangement>,
}

#[derive(Clone, Copy, Debug)]
pub enum TrackPanelMessage {
    TrackVolumeChanged(usize, f32),
    TrackPanChanged(usize, f32),
}

impl TrackPanel {
    pub fn new(arrangement: Arc<Arrangement>) -> Self {
        Self { arrangement }
    }

    pub fn update(&self, message: &TrackPanelMessage) {
        match message {
            TrackPanelMessage::TrackVolumeChanged(track_index, volume) => {
                if let Some(track) = self.arrangement.tracks.read().unwrap().get(*track_index) {
                    track.set_volume(*volume);
                }
            }
            TrackPanelMessage::TrackPanChanged(track_index, pan) => {
                if let Some(track) = self.arrangement.tracks.read().unwrap().get(*track_index) {
                    track.set_pan(*pan);
                }
            }
        }
    }

    pub fn view(&self) -> Element<'_, TrackPanelMessage> {
        let tracks = self
            .arrangement
            .tracks
            .read()
            .unwrap()
            .iter()
            .enumerate()
            .fold(column![], |col, (index, track)| {
                let track_name = format!("Track {}", index + 1);
                let volume = track.get_volume(); // Get current volume
                col.push(
                    row![
                        text(track_name),
                        slider(0.0..=1.0, volume, move |v| {
                            TrackPanelMessage::TrackVolumeChanged(index, v) // Handle volume change
                        })
                        .step(0.01)
                        .width(Length::Fixed(150.0)),
                        slider(-1.0..=1.0, track.get_pan(), move |v| {
                            TrackPanelMessage::TrackPanChanged(index, v) // Handle pan change
                        })
                        .step(0.01)
                        .width(Length::Fixed(150.0))
                    ]
                    .align_y(Center),
                )
            });

        container(tracks)
            .width(Length::Shrink)
            .height(Length::Fill)
            .into()
    }
}
