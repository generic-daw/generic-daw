use crate::track::Track;
use iced::widget::{column, container};
use iced::{Element, Length, Sandbox}; // Add Sandbox here // Add this import

pub struct TrackPanel {
    tracks: Vec<Track>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Track(usize, <Track as Sandbox>::Message),
}

impl Sandbox for TrackPanel {
    type Message = Message;

    fn new() -> Self {
        Self {
            tracks: vec![Track::new("Track 1".to_string())],
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Track(index, track_msg) => {
                if let Some(track) = self.tracks.get_mut(index) {
                    Sandbox::update(track, track_msg);
                }
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let tracks: Element<_> = self
            .tracks
            .iter()
            .enumerate()
            .fold(column![].spacing(10), |column, (i, track)| {
                column.push(Sandbox::view(track).map(move |msg| Message::Track(i, msg)))
            })
            .into();

        container(tracks)
            .width(Length::FillPortion(2))
            .height(Length::Fill)
            .style(iced::theme::Container::Box)
            .into()
    }

    fn title(&self) -> String {
        todo!()
    }
}
