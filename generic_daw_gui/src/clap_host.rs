use crate::{config::Config, widget::LINE_HEIGHT};
use fragile::Fragile;
#[cfg(unix)]
use generic_daw_core::clap_host::{FdFlags, PosixFdMessage};
use generic_daw_core::{
	Event,
	clap_host::{MainThreadMessage, ParamInfoFlags, Plugin, PluginId, Size},
};
use generic_daw_utils::{HoleyVec, NoClone, NoDebug};
use generic_daw_widget::knob::Knob;
use iced::{
	Center, Element, Font, Function as _,
	Length::{Fill, Shrink},
	Subscription, Task, padding,
	time::every,
	widget::{column, container, row, rule, sensor, space, text},
	window,
};
#[cfg(unix)]
use iced::{
	futures::{SinkExt as _, TryFutureExt as _},
	stream,
	task::Handle,
};
use log::info;
#[cfg(unix)]
use smol::future::or;
use smol::{Timer, unblock};
#[cfg(unix)]
use std::os::fd::{BorrowedFd, RawFd};
use std::{collections::HashMap, ops::Deref as _, sync::mpsc::Receiver, time::Duration};

#[derive(Clone, Debug)]
pub enum Message {
	MainThread(PluginId, MainThreadMessage),
	SendEvent(PluginId, Event),
	TickTimer(Duration),
	Loaded(NoClone<(Box<Fragile<Plugin<Event>>>, Receiver<MainThreadMessage>)>),
	SetState(PluginId, NoDebug<Box<[u8]>>),
	GuiEmbedded(NoClone<Box<Fragile<Plugin<Event>>>>),
	WindowResized(window::Id, Size),
	WindowCloseRequested(window::Id),
	WindowClosed(window::Id),
	SetPluginSize(PluginId, Size),
}

#[derive(Debug, Default)]
pub struct ClapHost {
	plugins: HoleyVec<Plugin<Event>>,
	timers: HashMap<Duration, HashMap<PluginId, u32>>,
	windows: HoleyVec<window::Id>,
	#[cfg(unix)]
	fds: HoleyVec<HashMap<RawFd, Handle>>,
}

impl ClapHost {
	pub fn update(&mut self, message: Message, config: &Config) -> Task<Message> {
		match message {
			Message::MainThread(id, msg) => {
				return self.handle_main_thread_message(id, msg, config);
			}
			Message::SendEvent(id, event) => self.plugins.get_mut(*id).unwrap().send_event(event),
			Message::TickTimer(duration) => {
				for (&plugin, &timer_id) in &self.timers[&duration] {
					if let Some(plugin) = self.plugins.get_mut(*plugin) {
						plugin.tick_timer(timer_id);
					}
				}
			}
			Message::Loaded(NoClone((plugin, plugin_receiver))) => {
				let mut plugin = plugin.into_inner();
				plugin.activate();
				let id = plugin.plugin_id();
				self.plugins.insert(*id, plugin);
				let (sender, receiver) = smol::channel::unbounded();
				return Task::batch([
					Task::future(unblock(move || {
						while let Ok(msg) = plugin_receiver.recv() {
							if sender.try_send(msg).is_err() {
								break;
							}
						}
					}))
					.discard(),
					Task::stream(receiver).map(Message::MainThread.with(id)),
				]);
			}
			Message::SetState(plugin, state) => {
				self.plugins.get_mut(*plugin).unwrap().set_state(&state);
			}
			Message::GuiEmbedded(NoClone(plugin)) => {
				let mut plugin = plugin.into_inner();
				let id = plugin.plugin_id();
				plugin.show();
				self.plugins.insert(*id, plugin);
			}
			Message::WindowResized(window, size) => {
				if let Some(id) = self.windows.key_of(&window)
					&& let Some(plugin) = self.plugins.get_mut(id)
					&& let Some(new_size) = plugin.resize(size)
					&& let Some(scale_factor) = plugin.get_scale()
					&& let new_size = new_size.to_logical(scale_factor)
					&& size.to_physical(scale_factor) != new_size
				{
					return window::resize(window, new_size.into());
				}
			}
			Message::WindowCloseRequested(window) => {
				return if let Some(plugin) = self.windows.key_of(&window) {
					self.plugins.get_mut(plugin).unwrap().destroy();
					window::close(window)
				} else {
					iced::exit()
				};
			}
			Message::WindowClosed(window) => {
				let id = self.windows.key_of(&window).unwrap();
				self.windows.remove(id).unwrap();
			}
			Message::SetPluginSize(id, size) => {
				if let Some(&window) = self.windows.get(*id) {
					return window::resize(window, size.to_logical(config.app_scale_factor).into());
				}
			}
		}

		Task::none()
	}

	fn handle_main_thread_message(
		&mut self,
		id: PluginId,
		message: MainThreadMessage,
		config: &Config,
	) -> Task<Message> {
		macro_rules! plugin {
			($expr:expr) => {{
				let Some(plugin) = self.plugins.get_mut(*id) else {
					let msg = Message::MainThread(id, $expr);
					info!("retrying {msg:?}");
					return Task::perform(Timer::after(Duration::from_millis(100)), |_| msg);
				};
				plugin
			}};
		}

		match message {
			MainThreadMessage::RequestCallback => {
				plugin!(MainThreadMessage::RequestCallback).call_on_main_thread_callback();
			}
			MainThreadMessage::Restart(processor) => {
				let plugin = plugin!(MainThreadMessage::Restart(processor));
				plugin.deactivate(processor);
				plugin.activate();
			}
			MainThreadMessage::Destroy(processor) => {
				plugin!(MainThreadMessage::Destroy(processor));

				self.plugins.remove(*id).unwrap().deactivate(processor);
				self.timers.values_mut().for_each(|set| _ = set.remove(&id));

				#[cfg(unix)]
				self.fds.remove(*id);

				return self.update(
					Message::MainThread(id, MainThreadMessage::GuiClosed),
					config,
				);
			}
			MainThreadMessage::GuiRequestShow => {
				let plugin = plugin!(MainThreadMessage::GuiRequestShow);

				if self.windows.contains_key(*id) {
				} else if !plugin.has_gui() {
					let (window, spawn) = window::open(window::Settings {
						size: (400.0, 600.0).into(),
						resizable: false,
						exit_on_close_request: false,
						level: window::Level::AlwaysOnTop,
						..window::Settings::default()
					});
					self.windows.insert(*id, window);

					return spawn.discard();
				} else if plugin.is_floating() {
					plugin.show();
				} else {
					let scale_factor = plugin.set_scale(
						config
							.plugin_scale_factor
							.unwrap_or(config.app_scale_factor),
					);

					let (window, spawn) = window::open(window::Settings {
						size: plugin.get_size().map_or_else(
							|| (400.0, 600.0).into(),
							|size| size.to_logical(scale_factor).into(),
						),
						resizable: plugin.can_resize(),
						exit_on_close_request: false,
						level: window::Level::AlwaysOnTop,
						..window::Settings::default()
					});
					self.windows.insert(*id, window);

					let mut plugin = Fragile::new(self.plugins.remove(*id).unwrap());
					let embed = window::run(window, move |window| {
						// SAFETY:
						// The plugin gui is destroyed before the window is closed (see
						// [`Message::WindowCloseRequested`]).
						unsafe {
							plugin
								.get_mut()
								.set_parent(window.window_handle().unwrap().as_raw());
						}
						Message::GuiEmbedded(NoClone(Box::new(plugin)))
					});

					return spawn.discard().chain(embed);
				}
			}
			MainThreadMessage::GuiRequestResize(size) => {
				if let Some(&window) = self.windows.get(*id) {
					return self.update(Message::WindowResized(window, size), config);
				}
			}
			MainThreadMessage::GuiRequestHide => {
				if let Some(&window) = self.windows.get(*id) {
					return self.update(Message::WindowCloseRequested(window), config);
				}

				plugin!(MainThreadMessage::GuiRequestHide).hide();
			}
			MainThreadMessage::GuiClosed => {
				if let Some(&window) = self.windows.get(*id) {
					return window::close(window);
				}
			}
			MainThreadMessage::RegisterTimer(timer_id, duration) => {
				self.timers
					.entry(duration)
					.or_default()
					.insert(id, timer_id);
			}
			MainThreadMessage::UnregisterTimer(timer_id) => {
				if let Some(set) = self
					.timers
					.values_mut()
					.find(|set| set.get(&id) == Some(&timer_id))
				{
					set.remove(&id);
				}
			}
			MainThreadMessage::RescanParams(flags) => {
				plugin!(MainThreadMessage::RescanParams(flags)).rescan_params(flags);
			}
			MainThreadMessage::RescanParam(param_id, flags) => {
				plugin!(MainThreadMessage::RescanParam(param_id, flags))
					.rescan_param(param_id, flags);
			}
			#[cfg(unix)]
			MainThreadMessage::PosixFd(fd, msg) => {
				return self
					.handle_posix_fd_message(id, fd, msg)
					.map(Message::MainThread.with(id));
			}
		}

		Task::none()
	}

	#[cfg(unix)]
	fn handle_posix_fd_message(
		&mut self,
		id: PluginId,
		fd: RawFd,
		message: PosixFdMessage,
	) -> Task<MainThreadMessage> {
		macro_rules! plugin {
			($expr:expr) => {{
				let Some(plugin) = self.plugins.get_mut(*id) else {
					let msg = MainThreadMessage::PosixFd(fd, $expr);
					info!("retrying {msg:?}");
					return Task::perform(Timer::after(Duration::from_millis(100)), |_| msg);
				};
				plugin
			}};
		}

		match message {
			PosixFdMessage::OnFd(flags) => plugin!(PosixFdMessage::OnFd(flags)).on_fd(fd, flags),
			PosixFdMessage::Register(flags) => {
				let (_, handle) = Task::<()>::none().abortable();

				let handle = self
					.fds
					.entry(*id)
					.get_or_insert_default()
					.insert(fd, handle.abort_on_drop());
				debug_assert!(handle.is_none());

				return self.handle_posix_fd_message(id, fd, PosixFdMessage::Modify(flags));
			}
			PosixFdMessage::Modify(flags) => {
				// SAFETY:
				// This fd is owned by the plugin, and is open at least until
				// [`PosixFd::Unregister`] is processed. The fd is not -1.
				let async_fd = smol::Async::new(unsafe { BorrowedFd::borrow_raw(fd) }).unwrap();

				let (task, handle) = Task::stream(stream::channel(100, async move |mut sender| {
					loop {
						let msg = or(
							async_fd
								.readable()
								.map_ok_or_else(|_| FdFlags::READ, |()| FdFlags::ERROR),
							async_fd
								.writable()
								.map_ok_or_else(|_| FdFlags::WRITE, |()| FdFlags::ERROR),
						)
						.await;

						if flags.intersects(msg)
							&& sender
								.send(MainThreadMessage::PosixFd(fd, PosixFdMessage::OnFd(msg)))
								.await
								.is_err()
						{
							return;
						}
					}
				}))
				.abortable();

				let handle = self
					.fds
					.get_mut(*id)
					.unwrap()
					.insert(fd, handle.abort_on_drop());
				debug_assert!(handle.is_some());

				return task;
			}
			PosixFdMessage::Unregister => {
				let handle = self.fds.get_mut(*id).unwrap().remove(&fd);
				debug_assert!(handle.is_some());
			}
		}

		Task::none()
	}

	pub fn view(&self, window: window::Id) -> Option<Element<'_, Message>> {
		let Some(plugin) = &self.plugins.get(self.windows.key_of(&window)?) else {
			return Some(space().into());
		};

		if plugin.has_gui() {
			return Some(space().into());
		}

		Some(
			sensor(
				column![
					text(&*plugin.descriptor().name)
						.size(LINE_HEIGHT)
						.line_height(1.0)
						.font(Font::MONOSPACE),
					container(rule::horizontal(1)).padding(padding::vertical(5)),
					row(plugin.params().map(|param| {
						column![
							container(
								Knob::new(param.range.clone(), param.value, |value| {
									Message::SendEvent(
										plugin.plugin_id(),
										Event::ParamValue {
											time: 0,
											param_id: param.id,
											value,
											cookie: param.cookie,
										},
									)
								})
								.default(param.reset)
								.radius(25.0)
								.enabled(!param.flags.contains(ParamInfoFlags::IS_READONLY))
								.stepped(param.flags.contains(ParamInfoFlags::IS_STEPPED))
								.maybe_tooltip(param.value_text.as_deref())
							)
							.padding(padding::horizontal(10)),
							text(&*param.name)
								.wrapping(text::Wrapping::WordOrGlyph)
								.align_x(Center)
								.width(Fill)
						]
						.spacing(5)
						.width(Shrink)
						.into()
					}))
					.spacing(10)
					.wrap()
					.vertical_spacing(10)
				]
				.width(Shrink)
				.padding(10),
			)
			.on_show(|size| {
				Message::SetPluginSize(
					plugin.plugin_id(),
					Size::Logical {
						width: size.width,
						height: size.height,
					},
				)
			})
			.into(),
		)
	}

	pub fn title(&self, window: window::Id) -> Option<String> {
		self.windows
			.key_of(&window)
			.and_then(|id| self.plugins.get(id))
			.map(|plugin| plugin.descriptor().name.deref().to_owned())
	}

	pub fn scale_factor(&self, window: window::Id) -> Option<f32> {
		self.windows
			.key_of(&window)
			.and_then(|id| self.plugins.get(id))
			.and_then(Plugin::get_scale)
	}

	pub fn subscription(&self) -> Subscription<Message> {
		Subscription::batch(
			self.timers
				.iter()
				.filter(|(_, v)| !v.is_empty())
				.map(|(&k, _)| every(k).with(k).map(|(k, _)| Message::TickTimer(k)))
				.chain([
					window::resize_events().map(|(id, size)| {
						Message::WindowResized(
							id,
							Size::Logical {
								width: size.width,
								height: size.height,
							},
						)
					}),
					window::close_requests().map(Message::WindowCloseRequested),
					window::close_events().map(Message::WindowClosed),
				]),
		)
	}

	pub fn set_realtime(&mut self, realtime: bool) {
		for plugin in self.plugins.values_mut() {
			plugin.set_realtime(realtime);
		}
	}

	pub fn get_state(&mut self, id: PluginId) -> Option<Vec<u8>> {
		self.plugins.get_mut(*id).unwrap().get_state()
	}
}
