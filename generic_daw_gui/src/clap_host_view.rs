use generic_daw_core::clap_host::ClapPluginGui;
use iced::{
    window::{self, close_events, close_requests, resize_events, Id},
    Size, Subscription, Task,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

mod opened;

pub use opened::Opened;

#[derive(Clone, Debug)]
pub enum Message {
    Opened(Arc<Mutex<Opened>>),
    CloseRequested(Id),
    Closed,
    Resized((Id, Size)),
}

#[derive(Default)]
pub struct ClapHostView {
    windows: HashMap<Id, ClapPluginGui>,
    closed: Option<Id>,
}

impl ClapHostView {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Opened(arc) => {
                let Opened {
                    id,
                    gui,
                    host_audio_processor: _,
                    plugin_audio_processor: _,
                } = Mutex::into_inner(Arc::into_inner(arc).unwrap()).unwrap();
                self.windows.insert(id, gui.into_inner());
            }
            Message::Resized((id, size)) => {
                if let Some(plugin) = self.windows.get_mut(&id) {
                    plugin.resize(size.width, size.height);
                }
            }
            Message::CloseRequested(id) => {
                self.windows.remove(&id).unwrap().destroy();
                self.closed.replace(id);
                return window::close::<()>(id).discard();
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
