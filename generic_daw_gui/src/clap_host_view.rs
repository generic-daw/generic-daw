use async_channel::Receiver;
use fragile::Fragile;
use generic_daw_core::clap_host::{GuiExt, MainThreadMessage, PluginId};
use generic_daw_utils::HoleyVec;
use iced::{
    Function as _, Size, Subscription, Task,
    window::{self, Id, close_requests, resize_events},
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub enum Message {
    MainThread(PluginId, MainThreadMessage),
    Opened(Arc<Mutex<(Fragile<GuiExt>, Receiver<MainThreadMessage>)>>),
    Shown(Id, Arc<Fragile<GuiExt>>),
    CloseRequested(Id),
    Resized((Id, Size)),
}

#[derive(Default)]
pub struct ClapHostView {
    plugins: HoleyVec<GuiExt>,
    windows: HoleyVec<Id>,
}

impl ClapHostView {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MainThread(id, msg) => return self.main_thread_message(id, msg),
            Message::Opened(arc) => {
                let (gui, gui_receiver) = Mutex::into_inner(Arc::into_inner(arc).unwrap()).unwrap();
                let mut gui = gui.into_inner();
                let id = gui.plugin_id();

                let open = if gui.is_floating() {
                    gui.open_floating();
                    self.plugins.insert(*id, gui);
                    Task::none()
                } else {
                    self.plugins.insert(*id, gui);
                    self.update(Message::MainThread(id, MainThreadMessage::GuiRequestShow))
                };

                let stream = Task::stream(gui_receiver).map(Message::MainThread.with(id));

                return open.chain(stream);
            }
            Message::Shown(window_id, arc) => {
                let mut gui = Arc::into_inner(arc).unwrap().into_inner();
                let id = gui.plugin_id();

                let resize = gui.get_size().map_or_else(Task::none, |size| {
                    window::resize(window_id, size.map(|x| x as f32).into())
                });
                let resizable = window::set_resizable(window_id, gui.can_resize());

                self.plugins.insert(*id, gui);
                self.windows.insert(*id, window_id);

                return resize
                    .chain(resizable)
                    .chain(self.update(Message::MainThread(id, MainThreadMessage::TickTimers)));
            }
            Message::Resized((window_id, size)) => {
                if let Some(id) = self.windows.position(&window_id) {
                    if let Some(new_size) = self
                        .plugins
                        .get_mut(id)
                        .unwrap()
                        .resize(size.width as u32, size.height as u32)
                    {
                        let new_size = new_size.map(|x| x as f32).into();
                        if size != new_size {
                            return window::resize(window_id, new_size);
                        }
                    }
                }
            }
            Message::CloseRequested(window_id) => {
                let Some(id) = self.windows.position(&window_id) else {
                    return iced::exit();
                };

                self.plugins.get_mut(id).unwrap().destroy();
                let window_id = self.windows.remove(id).unwrap();
                return window::close(window_id);
            }
        }

        Task::none()
    }

    fn main_thread_message(&mut self, id: PluginId, msg: MainThreadMessage) -> Task<Message> {
        match msg {
            MainThreadMessage::RequestCallback => self
                .plugins
                .get_mut(*id)
                .unwrap()
                .call_on_main_thread_callback(),
            MainThreadMessage::GuiRequestHide => {
                if let Some(&id) = self.windows.get(*id) {
                    return self.update(Message::CloseRequested(id));
                }
            }
            MainThreadMessage::GuiRequestShow => {
                if self.windows.contains_key(*id) {
                    return Task::none();
                }

                let mut gui = Fragile::new(self.plugins.remove(*id).unwrap());

                let (window_id, spawn) = window::open(window::Settings {
                    size: Size::new(1.0, 1.0),
                    resizable: false,
                    exit_on_close_request: false,
                    ..window::Settings::default()
                });

                let embed = window::run_with_handle(window_id, move |handle| {
                    gui.get_mut().open_embedded(handle.as_raw());
                    Message::Shown(window_id, Arc::new(gui))
                });

                return spawn.discard().chain(embed);
            }
            MainThreadMessage::GuiClosed => {
                self.plugins.remove(*id).unwrap();
                let window_id = self.windows.remove(*id).unwrap();
                return window::close(window_id);
            }
            MainThreadMessage::GuiRequestResize(new_size) => {
                if let Some(&window_id) = self.windows.get(*id) {
                    return window::resize(
                        window_id,
                        Size {
                            width: new_size.width as f32,
                            height: new_size.height as f32,
                        },
                    );
                }
            }
            MainThreadMessage::TickTimers => {
                if self.windows.contains_key(*id) {
                    if let Some(sleep) = self.plugins.get_mut(*id).unwrap().tick_timers() {
                        return Task::future(tokio::time::sleep(sleep))
                            .map(|()| MainThreadMessage::TickTimers)
                            .map(Message::MainThread.with(id));
                    }
                }
            }
        }

        Task::none()
    }

    pub fn title(&self, window: Id) -> Option<String> {
        self.windows
            .position(&window)
            .map(|id| self.plugins[id].name().to_owned())
    }

    pub fn is_plugin_window(&self, window: Id) -> bool {
        self.windows.contains_value(&window)
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::batch([
            resize_events().map(Message::Resized),
            close_requests().map(Message::CloseRequested),
        ])
    }
}
