use fragile::Fragile;
use generic_daw_core::clap_host::{GuiExt, GuiMessage, PluginId, Receiver};
use generic_daw_utils::HoleyVec;
use iced::{
    Size, Subscription, Task,
    futures::SinkExt as _,
    stream::channel,
    window::{self, Id, Settings, close_requests, resize_events},
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub enum Message {
    MainThread(PluginId, GuiMessage),
    Opened(Arc<Mutex<(Fragile<GuiExt>, Receiver<GuiMessage>)>>),
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
                    self.update(Message::MainThread(id, GuiMessage::GuiRequestShow))
                };

                let stream = Task::stream(channel(16, async move |mut sender| {
                    while let Ok(msg) = gui_receiver.recv().await {
                        sender.send(Message::MainThread(id, msg)).await.unwrap();
                    }
                }));

                return open.chain(stream);
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
                let id = self.windows.position(&window_id).unwrap();
                self.windows.remove(id).unwrap();
                self.plugins.get_mut(id).unwrap().destroy();

                return window::close::<()>(window_id).discard();
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

    fn main_thread_message(&mut self, id: PluginId, msg: GuiMessage) -> Task<Message> {
        match msg {
            GuiMessage::RequestCallback => self
                .plugins
                .get_mut(*id)
                .unwrap()
                .call_on_main_thread_callback(),
            GuiMessage::GuiRequestHide => {
                let window_id = self.windows.remove(*id).unwrap();
                return window::close(window_id);
            }
            GuiMessage::GuiRequestShow => {
                let mut gui = self.plugins.remove(*id).unwrap();
                let resizable = gui.can_resize();

                let size = gui.get_size().map_or_else(
                    || Size::new(1.0, 1.0),
                    |[width, height]| Size::new(width as f32, height as f32),
                );

                let (window_id, spawn) = window::open(Settings {
                    exit_on_close_request: false,
                    resizable,
                    size,
                    ..Settings::default()
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
                    .chain(Task::done(Message::MainThread(id, GuiMessage::TickTimers)));
            }
            GuiMessage::GuiClosed => {
                self.plugins.remove(*id).unwrap().destroy();

                return self.main_thread_message(id, GuiMessage::GuiRequestHide);
            }
            GuiMessage::GuiRequestResize(new_size) => {
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
            GuiMessage::TickTimers => {
                if self.windows.get(*id).is_some() {
                    if let Some((timers, timer_ext)) = self.plugins[*id].timers() {
                        let mut instance = self.plugins.get_mut(*id).unwrap().plugin_handle();

                        if let Some(sleep) =
                            timers.borrow_mut().tick_timers(&timer_ext, &mut instance)
                        {
                            return Task::future(tokio::time::sleep(sleep))
                                .map(|()| GuiMessage::TickTimers)
                                .map(move |msg| Message::MainThread(id, msg));
                        }
                    }
                }
            }
        }

        Task::none()
    }
}
