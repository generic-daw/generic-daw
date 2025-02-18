use generic_daw_core::clap_host::{ClapPluginGui, MainThreadMessage};
use iced::{
    futures::SinkExt as _,
    stream::channel,
    window::{self, close_requests, resize_events, Id},
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
    Resized((Id, Size)),
    MainThread((Id, MainThreadMessage)),
}

#[derive(Default)]
pub struct ClapHostView {
    windows: HashMap<Id, ClapPluginGui>,
}

impl ClapHostView {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Opened(arc) => {
                let Opened {
                    id,
                    gui,
                    hap: _,
                    pap,
                } = Mutex::into_inner(Arc::into_inner(arc).unwrap()).unwrap();
                self.windows.insert(id, gui.into_inner());

                #[expect(tail_expr_drop_order)]
                return Task::stream(channel(16, move |mut sender| async move {
                    while let Ok(msg) = pap.receiver.recv().await {
                        sender.send(Message::MainThread((id, msg))).await.unwrap();
                    }
                }));
            }
            Message::Resized((id, size)) => {
                if let Some(plugin) = self.windows.get_mut(&id) {
                    plugin.resize(size.width as u32, size.height as u32);
                }
            }
            Message::CloseRequested(id) => {
                self.windows.remove(&id).unwrap();
                return window::close::<()>(id).discard();
            }
            Message::MainThread((id, msg)) => match msg {
                MainThreadMessage::GuiClosed => {
                    return window::close(id);
                }
                MainThreadMessage::GuiRequestResized(new_size) => {
                    return window::resize(
                        id,
                        Size {
                            width: new_size.width as f32,
                            height: new_size.height as f32,
                        },
                    );
                }
                MainThreadMessage::RunOnMainThread => self
                    .windows
                    .get_mut(&id)
                    .unwrap()
                    .call_on_main_thread_callback(),
            },
        }

        Task::none()
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::batch([
            resize_events().map(Message::Resized),
            close_requests().map(Message::CloseRequested),
        ])
    }
}
