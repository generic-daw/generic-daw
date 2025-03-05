use fragile::Fragile;
use generic_daw_core::clap_host::{GuiExt, MainThreadMessage, PluginId, Receiver};
use generic_daw_utils::HoleyVec;
use iced::{
    Function as _, Size, Subscription, Task,
    futures::SinkExt as _,
    stream::channel,
    window::{self, Id, close_requests, resize_events},
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub enum Message {
    MainThread(PluginId, MainThreadMessage),
    Opened(Arc<Mutex<(Fragile<GuiExt>, Receiver<MainThreadMessage>)>>),
    Close(PluginId),
    Shown(Id, Arc<Fragile<GuiExt>>),
    CloseRequested(Id),
    Resized((Id, Size)),
}

pub struct ClapHostView {
    main_window_id: Id,
    plugins: HoleyVec<GuiExt>,
    windows: HoleyVec<Id>,
}

impl ClapHostView {
    pub fn new(main_window_id: Id) -> Self {
        Self {
            main_window_id,
            plugins: HoleyVec::default(),
            windows: HoleyVec::default(),
        }
    }

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
                    Task::done(Message::MainThread(id, MainThreadMessage::GuiRequestShow))
                };

                let stream = Task::stream(channel(0, async move |mut sender| {
                    while let Ok(msg) = gui_receiver.recv().await {
                        sender.send(Message::MainThread(id, msg)).await.unwrap();
                    }
                }));

                return open.chain(stream);
            }
            Message::Close(id) => {
                if self.windows.contains(*id) {
                    return self.update(Message::MainThread(id, MainThreadMessage::GuiClosed));
                }
            }
            Message::Shown(window_id, arc) => {
                let gui = Arc::into_inner(arc).unwrap().into_inner();
                let id = gui.plugin_id();

                self.plugins.insert(*id, gui);
                self.windows.insert(*id, window_id);
            }
            Message::Resized((window_id, size)) => {
                if let Some(id) = self.windows.position(&window_id) {
                    let new_size = self
                        .plugins
                        .get_mut(id)
                        .unwrap()
                        .resize(size.width as u32, size.height as u32)
                        .map(|x| x as f32)
                        .into();

                    if size != new_size {
                        return window::resize(window_id, new_size);
                    }
                }
            }
            Message::CloseRequested(window_id) => {
                if window_id == self.main_window_id {
                    return iced::exit();
                }

                let id = self.windows.position(&window_id).unwrap();
                self.plugins.get_mut(id).unwrap().destroy();
                let window_id = self.windows.remove(id).unwrap();
                return window::close(window_id);
            }
        }

        Task::none()
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::batch([
            resize_events().map(Message::Resized),
            close_requests().map(Message::CloseRequested),
        ])
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
                if self.windows.contains(*id) {
                    return Task::none();
                }

                let mut gui = self.plugins.remove(*id).unwrap();
                let resizable = gui.can_resize();

                let (window_id, spawn) = window::open(window::Settings {
                    exit_on_close_request: false,
                    resizable,
                    size: Size::new(1.0, 1.0),
                    ..window::Settings::default()
                });

                gui.destroy();
                let mut gui = Fragile::new(gui);

                let embed = window::run_with_handle(window_id, move |handle| {
                    gui.get_mut().open_embedded(handle.as_raw());
                    Message::Shown(window_id, Arc::new(gui))
                });

                return spawn
                    .discard()
                    .chain(embed)
                    .chain(Task::done(Message::MainThread(
                        id,
                        MainThreadMessage::TickTimers,
                    )));
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
                if self.windows.contains(*id) {
                    if let Some((timers, timer_ext)) = self.plugins[*id].timers() {
                        let mut instance = self.plugins.get_mut(*id).unwrap().plugin_handle();

                        if let Some(sleep) =
                            timers.borrow_mut().tick_timers(&timer_ext, &mut instance)
                        {
                            return Task::future(tokio::time::sleep(sleep))
                                .map(|()| MainThreadMessage::TickTimers)
                                .map(Message::MainThread.with(id));
                        }
                    }
                }
            }
        }

        Task::none()
    }
}
