use iced::widget::{container, text};
use iced::{Element, Length, Sandbox};

pub struct Timeline {}

#[derive(Debug, Clone)]
pub enum Message {}

impl Sandbox for Timeline {
    type Message = Message;

    fn new() -> Self {
        Self {}
    }

    fn update(&mut self, _message: Message) {
        // Update logic for timeline
    }

    fn view(&self) -> Element<Message> {
        container(text("Timeline"))
            .width(Length::FillPortion(3))
            .height(Length::Fill)
            .center_x()
            .center_y()
            .style(iced::theme::Container::Box)
            .into()
    }

    fn title(&self) -> String {
        todo!()
        // self.name.clone()
    }
}
