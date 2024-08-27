use iced::widget::{row, slider, text};
use iced::{Element, Length, Sandbox};

pub struct Track {
    name: String,
    volume: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum Message {
    VolumeChanged(f32),
}

impl Sandbox for Track {
    type Message = Message;

    fn new() -> Self {
        Self {
            name: "New Track".to_string(),
            volume: 0.5,
        }
    }

    fn title(&self) -> String {
        self.name.clone()
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::VolumeChanged(new_volume) => {
                self.volume = new_volume;
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let volume_knob = slider(0.0..=1.0, self.volume, Message::VolumeChanged)
            .step(0.01)
            .width(Length::Fixed(100.0));

        row![
            text(&self.name).width(Length::Fill),
            text(format!("Volume: {:.2}", self.volume)),
            volume_knob
        ]
        .spacing(20)
        .into()
    }
}

impl Track {
    pub const fn new(name: String) -> Self {
        Self { name, volume: 0.5 }
    }
}
