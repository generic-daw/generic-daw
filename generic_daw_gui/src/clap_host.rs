use fragile::Fragile;
use generic_daw_core::clap_host::{GuiExt, MainThreadMessage, PluginId, events::Match};
use generic_daw_utils::HoleyVec;
use iced::{
	Function as _, Size, Subscription, Task,
	time::every,
	window::{self, Id, close_requests, resize_events},
};
use smol::channel::Receiver;
use std::{ops::Deref as _, sync::Arc, time::Duration};

#[derive(Clone, Debug)]
pub enum Message {
	MainThread(PluginId, MainThreadMessage),
	TickTimer(usize, u32),
	Opened(Arc<Fragile<GuiExt>>, Receiver<MainThreadMessage>),
	GuiRequestShow(Option<Id>, Arc<Fragile<GuiExt>>),
	GuiRequestResize((Id, Size)),
	GuiRequestHide(Id),
}

#[derive(Default)]
pub struct ClapHost {
	plugins: HoleyVec<GuiExt>,
	timers: HoleyVec<HoleyVec<Duration>>,
	windows: HoleyVec<Id>,
}

impl ClapHost {
	pub fn update(&mut self, message: Message) -> Task<Message> {
		match message {
			Message::MainThread(id, msg) => return self.main_thread_message(id, msg),
			Message::TickTimer(id, timer_id) => {
				self.plugins.get_mut(id).unwrap().tick_timer(timer_id);
			}
			Message::Opened(gui, receiver) => {
				let gui = Arc::into_inner(gui).unwrap().into_inner();
				let id = gui.plugin_id();
				self.plugins.insert(*id, gui);

				let open = self.update(Message::MainThread(id, MainThreadMessage::GuiRequestShow));
				let stream = Task::stream(receiver).map(Message::MainThread.with(id));

				return open.chain(stream);
			}
			Message::GuiRequestShow(window_id, arc) => {
				let mut gui = Arc::into_inner(arc).unwrap().into_inner();
				let id = gui.plugin_id();

				gui.show();
				self.plugins.insert(*id, gui);

				if let Some(window_id) = window_id {
					self.windows.insert(*id, window_id);
				}
			}
			Message::GuiRequestResize((window_id, size)) => {
				if let Some([w, h]) = self.windows.key_of(&window_id).and_then(|id| {
					self.plugins
						.get_mut(id)
						.unwrap()
						.resize(size.width as u32, size.height as u32)
				}) {
					let new_size = Size::new(w as f32, h as f32);
					if size != new_size {
						return window::resize(window_id, new_size);
					}
				}
			}
			Message::GuiRequestHide(window_id) => {
				let Some(id) = self.windows.key_of(&window_id) else {
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
			MainThreadMessage::GuiRequestShow => {
				if self.windows.contains_key(*id) {
					return Task::none();
				}

				let mut gui = self.plugins.remove(*id).unwrap();

				gui.create();
				return if gui.is_floating() {
					self.update(Message::GuiRequestShow(None, Arc::new(Fragile::new(gui))))
				} else {
					let (window_id, spawn) = window::open(window::Settings {
						size: gui.get_size().map_or(Size::new(1280.0, 720.0), |[w, h]| {
							Size::new(w as f32, h as f32)
						}),
						resizable: gui.can_resize(),
						exit_on_close_request: false,
						..window::Settings::default()
					});

					let mut gui = Fragile::new(gui);
					let embed = window::run_with_handle(window_id, move |handle| {
						// SAFETY:
						// The plugin gui is destroyed before the window is closed (see
						// [`Message::GuiRequestHide`]).
						unsafe {
							gui.get_mut().set_parent(handle.as_raw());
						}
						gui
					});

					spawn
						.discard()
						.chain(embed)
						.map(move |gui| Message::GuiRequestShow(Some(window_id), Arc::new(gui)))
				};
			}
			MainThreadMessage::GuiRequestResize(new_size) => {
				if let Some(&window_id) = self.windows.get(*id) {
					return window::resize(
						window_id,
						Size::new(new_size.width as f32, new_size.height as f32),
					);
				}
			}
			MainThreadMessage::GuiRequestHide => {
				if let Some(&id) = self.windows.get(*id) {
					return self.update(Message::GuiRequestHide(id));
				}
			}
			MainThreadMessage::GuiClosed => {
				self.plugins.remove(*id).unwrap();
				self.timers.remove(*id);

				if let Some(window_id) = self.windows.remove(*id) {
					return window::close(window_id);
				}
			}
			MainThreadMessage::RegisterTimer(timer_id, duration) => {
				self.timers
					.entry(*id)
					.get_or_insert_default()
					.insert(timer_id as usize, duration);
			}
			MainThreadMessage::UnregisterTimer(timer_id) => {
				self.timers.get_mut(*id).unwrap().remove(timer_id as usize);
			}
			MainThreadMessage::LatencyChanged => {
				self.plugins.get_mut(*id).unwrap().latency_changed();
			}
			MainThreadMessage::RescanValues => {
				self.plugins.get_mut(*id).unwrap().rescan_values(Match::All);
			}
		}

		Task::none()
	}

	pub fn title(&self, window: Id) -> Option<String> {
		self.windows
			.key_of(&window)
			.map(|id| self.plugins[id].descriptor().name.deref().to_owned())
	}

	pub fn is_plugin_window(&self, window: Id) -> bool {
		self.windows.contains_value(&window)
	}

	pub fn set_realtime(&mut self, realtime: bool) {
		for plugin in self.plugins.values_mut() {
			plugin.set_realtime(realtime);
		}
	}

	pub fn get_state(&mut self, id: PluginId) -> Option<Vec<u8>> {
		self.plugins.get_mut(*id).unwrap().get_state()
	}

	pub fn subscription(&self) -> Subscription<Message> {
		Subscription::batch(
			self.windows
				.keys()
				.flat_map(|id| {
					self.timers
						.get(id)
						.into_iter()
						.flat_map(HoleyVec::iter)
						.map(move |(k, &v)| {
							every(v)
								.with(k)
								.with(id)
								.map(|(id, (k, _))| Message::TickTimer(id, k as u32))
						})
				})
				.chain([
					resize_events().map(Message::GuiRequestResize),
					close_requests().map(Message::GuiRequestHide),
				]),
		)
	}
}
