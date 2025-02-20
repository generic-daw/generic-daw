use fragile::Fragile;
use generic_daw_core::clap_host::{ClapPluginGui, MainThreadMessage, PluginId};
use generic_daw_utils::HoleyVec;
use iced::{
    futures::SinkExt as _,
    stream::channel,
    window::{self, close_requests, resize_events, Id, Settings},
    Size, Subscription, Task,
};
use std::sync::{Arc, Mutex};

mod opened;

pub use opened::Opened;

#[derive(Clone, Debug)]
pub enum Message {
    Opened(Id, Arc<Mutex<Opened>>),
    Shown(Id, Arc<Fragile<ClapPluginGui>>),
    CloseRequested(Id),
    Resized((Id, Size)),
    MainThread((PluginId, MainThreadMessage)),
}

#[derive(Default)]
pub struct ClapHostView {
    plugins: HoleyVec<ClapPluginGui>,
    windows: HoleyVec<Id>,
}

impl ClapHostView {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Opened(window_id, arc) => {
                let Opened { gui, hap: _, pap } =
                    Mutex::into_inner(Arc::into_inner(arc).unwrap()).unwrap();

                let gui = gui.into_inner();
                let id = gui.plugin_id();

                self.plugins.insert(id, gui);
                self.windows.insert(id, window_id);

                #[expect(tail_expr_drop_order)]
                return Task::stream(channel(16, move |mut sender| async move {
                    while let Ok(msg) = pap.receiver.recv().await {
                        sender.send(Message::MainThread((id, msg))).await.unwrap();
                    }
                }));
            }
            Message::Shown(window_id, arc) => {
                let gui = Arc::into_inner(arc).unwrap().into_inner();
                let id = gui.plugin_id();

                self.plugins.insert(id, gui);
                self.windows.insert(id, window_id);
            }
            Message::Resized((window_id, size)) => {
                if let Some(id) = self.windows.position(&window_id) {
                    self.plugins
                        .get_mut(id)
                        .unwrap()
                        .resize(size.width as u32, size.height as u32);
                }
            }
            Message::CloseRequested(window_id) => {
                let id = self.windows.position(&window_id).unwrap();
                self.windows.remove(id).unwrap();

                if let Some(gui) = self.plugins.get_mut(id) {
                    gui.destroy();
                }

                return window::close::<()>(window_id).discard();
            }
            Message::MainThread((id, msg)) => match msg {
                MainThreadMessage::RequestCallback => self
                    .plugins
                    .get_mut(id)
                    .unwrap()
                    .call_on_main_thread_callback(),
                MainThreadMessage::GuiRequestHide => {
                    let window_id = self.windows.remove(id).unwrap();
                    return window::close(window_id);
                }
                MainThreadMessage::GuiRequestShow => {
                    let gui = self.plugins.remove(id).unwrap();
                    let mut gui = Fragile::new(gui);

                    let (window_id, spawn) = window::open(Settings {
                        exit_on_close_request: false,
                        ..Settings::default()
                    });

                    let embed = window::run_with_handle(window_id, move |handle| {
                        gui.get_mut().destroy();
                        gui.get_mut().open_embedded(handle.as_raw());
                        Message::Shown(window_id, Arc::new(gui))
                    });

                    return spawn.discard().chain(embed);
                }
                MainThreadMessage::GuiClosed => {
                    self.plugins.remove(id).unwrap().destroy();

                    return self
                        .update(Message::MainThread((id, MainThreadMessage::GuiRequestHide)));
                }
                MainThreadMessage::GuiRequestResize(new_size) => {
                    let window_id = self.windows[id];

                    return window::resize(
                        window_id,
                        Size {
                            width: new_size.width as f32,
                            height: new_size.height as f32,
                        },
                    );
                }
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
