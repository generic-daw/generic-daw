use generic_daw_core::clap_host::{
    ClapPluginGui, ClapPluginGuiWrapper, HostAudioProcessor, PluginAudioProcessor,
};
use iced::{
    window::{self, close_events, close_requests, resize_events, Id},
    Size, Subscription, Task,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
pub enum Message {
    Opened(Arc<Mutex<OpenedMessage>>),
    CloseRequested(Id),
    Closed,
    Resized((Id, Size)),
}

#[derive(Debug)]
pub struct OpenedMessage {
    pub id: Id,
    pub plugin: ClapPluginGuiWrapper,
    #[expect(dead_code)]
    pub host_audio_processor: HostAudioProcessor,
    #[expect(dead_code)]
    pub plugin_audio_processor: PluginAudioProcessor,
}

#[derive(Default)]
pub struct ClapHost {
    windows: HashMap<Id, ClapPluginGui>,
    closed: Option<Id>,
}

impl ClapHost {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Opened(arc) => {
                let OpenedMessage {
                    id,
                    plugin,
                    host_audio_processor: _,
                    plugin_audio_processor: _,
                } = Mutex::into_inner(Arc::into_inner(arc).unwrap()).unwrap();
                self.windows.insert(id, plugin.into_inner());
            }
            Message::Resized((id, size)) => {
                if let Some(plugin) = self.windows.get_mut(&id) {
                    plugin.resize(size.width, size.height);
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
