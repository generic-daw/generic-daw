use crate::generic_back::arrangement::Arrangement;
use iced::{
    widget::{column, container, text},
    Element, Length, Sandbox,
};
use std::sync::{Arc, Mutex};

pub struct Timeline {
    arrangement: Arc<Mutex<Arrangement>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    ArrangementUpdated,
}

impl Timeline {
    pub fn new(arrangement: Arc<Mutex<Arrangement>>) -> Self {
        Self { arrangement }
    }
}

impl Sandbox for Timeline {
    type Message = Message;

    fn new() -> Self {
        panic!("Timeline should be created with an arrangement")
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
        let clips = self
            .arrangement
            .lock()
            .unwrap()
            .tracks()
            .iter()
            .enumerate()
            .fold(column![].spacing(10), |col, (track_index, track)| {
                let track_name = format!("Track {}", track_index + 1);
                let track_clips = track.lock().unwrap().clips().iter().enumerate().fold(
                    column![].spacing(5),
                    |col, (clip_index, clip)| {
                        let clip_info = format!(
                            "Clip {}: Starts at {}",
                            clip_index + 1,
                            clip.get_global_end()
                        );
                        col.push(text(clip_info))
                    },
                );
                col.push(text(track_name)).push(track_clips)
            });

        container(clips)
            .width(Length::FillPortion(3))
            .height(Length::Fill)
            .center_x()
            .center_y()
            .style(iced::theme::Container::Box)
            .into()
    }

    fn title(&self) -> String {
        "Timeline".to_string()
    }
}
