use crate::generic_back::arrangement::Arrangement;
use iced::{
    widget::{column, container, text},
    Element, Length,
};
use std::sync::{Arc, RwLock};

pub struct TrackPanel {
    arrangement: Arc<RwLock<Arrangement>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    ArrangementUpdated,
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
            .fold(column![].spacing(20), |col, (index, _)| {
                let track_name = format!("Track {}", index + 1);
                col.push(text(track_name))
            })
            .padding(20);

        container(tracks)
            .width(Length::Shrink)
            .height(Length::Fill)
            .into()
    }
}
