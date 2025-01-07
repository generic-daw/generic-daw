use crate::clap_host::{ClapPlugin, ClapPluginWrapper};
use iced::{
    window::{close_events, resize_events, Id},
    Size, Subscription, Task,
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Message {
    Opened((Id, ClapPluginWrapper)),
    Closed(Id),
    Resized((Id, Size)),
}

#[derive(Default)]
pub struct ClapHost {
    windows: HashMap<Id, ClapPlugin>,
}

impl ClapHost {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Opened((id, plugin)) => {
                self.windows.insert(id, plugin.into_inner());
            }
            Message::Resized((id, size)) => {
                if let Some(plugin) = self.windows.get_mut(&id) {
                    plugin.resize(size);
                }
            }
            Message::Closed(id) => {
                self.windows.remove(&id).unwrap().destroy();
            }
        }
        Task::none()
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::batch([
            resize_events().map(Message::Resized),
            close_events().map(Message::Closed),
        ])
    }
}
