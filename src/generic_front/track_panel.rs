use crate::generic_back::arrangement::Arrangement;
use iced::{
    widget::{column, container, text},
    Element, Length, Sandbox,
};
use std::sync::{Arc, Mutex};

pub struct TrackPanel {
    arrangement: Arc<Mutex<Arrangement>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    ArrangementUpdated,
}

impl TrackPanel {
    pub fn new(arrangement: Arc<Mutex<Arrangement>>) -> Self {
        Self { arrangement }
    }
}

impl Sandbox for TrackPanel {
    type Message = Message;

    fn new() -> Self {
        unimplemented!()
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::ArrangementUpdated => {
                // Handle arrangement updates, if necessary
                // For example, you could trigger a re-render or refresh here
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let tracks = self
            .arrangement
            .lock()
            .unwrap()
            .tracks
            .iter()
            .enumerate()
            .fold(column![].spacing(10), |col, (index, _)| {
                let track_name = format!("Track {}", index + 1);
                col.push(text(track_name))
            });

        container(tracks)
            .width(Length::FillPortion(2))
            .height(Length::Fill)
            .style(iced::theme::Container::Box)
            .into()
    }

    fn title(&self) -> String {
        "Track Panel".to_string()
    }
}
