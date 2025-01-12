use crate::clap_host::{ClapPlugin, ClapPluginWrapper};
use iced::{
    window::{self, close_events, close_requests, resize_events, Id},
    Size, Subscription, Task,
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Message {
    Opened((Id, ClapPluginWrapper)),
    CloseRequested(Id),
    Closed,
    Resized((Id, Size)),
}

#[derive(Default)]
pub struct ClapHost {
    windows: HashMap<Id, ClapPlugin>,
    closed: Option<Id>,
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
            Message::CloseRequested(id) => {
                self.windows.remove(&id).unwrap().destroy();
                self.closed.replace(id);
                return window::close::<Id>(id).discard();
            }
            Message::Closed => {
                if self.closed.take().is_none() {
                    self.windows
                        .drain()
                        .for_each(|(_, plugin)| plugin.destroy());
                    return iced::exit();
                }
            }
        }

        Task::none()
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::batch([
            resize_events().map(Message::Resized),
            close_requests().map(Message::CloseRequested),
            close_events().map(|_| Message::Closed),
        ])
    }
}
