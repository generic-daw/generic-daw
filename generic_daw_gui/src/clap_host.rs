use crate::{config::Config, stylefns::scrollable_style, widget::LINE_HEIGHT};
use fragile::Fragile;
#[cfg(unix)]
use generic_daw_core::clap_host::{FdFlags, PosixFdMessage};
use generic_daw_core::{
	Event, PluginId,
	clap_host::{MainThreadMessage, ParamInfoFlags, Plugin, Size, TimerId},
};
use generic_daw_widget::knob::Knob;
use iced::{
	Center, Element, Font, Function as _,
	Length::Fill,
	Subscription, Task, padding,
	time::every,
	widget::{column, container, row, rule, scrollable, space, text},
	window,
};
use log::info;
#[cfg(unix)]
use smol::{
	Async,
	future::or,
	stream::{StreamExt as _, unfold},
};
use smol::{Timer, unblock};
use std::{
	collections::{HashMap, hash_map::Entry},
	ops::Deref as _,
	sync::mpsc::Receiver,
	time::Duration,
};
#[cfg(unix)]
use std::{
	iter::repeat,
	os::fd::{BorrowedFd, RawFd},
};
use utils::NoClone;

#[derive(Clone, Debug)]
pub enum Message {
	MainThread(PluginId, MainThreadMessage),
	SendEvent(PluginId, Event),
	TickTimer(Duration),
	GuiEmbedded(PluginId, NoClone<Box<Fragile<Plugin<Event>>>>),
	WindowResized(window::Id, Size),
	WindowCloseRequested(window::Id),
	WindowClosed(window::Id),
}

#[derive(Debug, Default)]
pub struct ClapHost {
	plugins: HashMap<PluginId, Plugin<Event>>,
	timers_of_plugin: HashMap<Duration, HashMap<PluginId, TimerId>>,
	window_of_plugin: HashMap<PluginId, window::Id>,
	plugin_of_window: HashMap<window::Id, PluginId>,
	#[cfg(unix)]
	fds_of_plugin: HashMap<PluginId, HashMap<RawFd, FdFlags>>,
}

impl ClapHost {
	pub fn update(&mut self, message: Message, config: &Config) -> Task<Message> {
		match message {
			Message::MainThread(id, msg) => {
				return self.handle_main_thread_message(id, msg, config);
			}
			Message::SendEvent(id, event) => self.plugins.get_mut(&id).unwrap().send_event(event),
			Message::TickTimer(duration) => {
				for (&plugin, &timer_id) in &self.timers_of_plugin[&duration] {
					if let Some(plugin) = self.plugins.get_mut(&plugin) {
						plugin.tick_timer(timer_id);
					}
				}
			}
			Message::GuiEmbedded(id, NoClone(plugin)) => {
				let mut plugin = plugin.into_inner();
				plugin.show();
				self.plugins.insert(id, plugin);
			}
			Message::WindowResized(window, size) => {
				if let Some(id) = self.plugin_of_window.get(&window)
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
				return if let Some(plugin) = self.plugin_of_window.get(&window) {
					self.plugins.get_mut(plugin).unwrap().destroy();
					window::close(window)
				} else {
					iced::exit()
				};
			}
			Message::WindowClosed(window) => {
				let id = self.plugin_of_window.remove(&window).unwrap();
				self.window_of_plugin.remove(&id).unwrap();
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
				let Some(plugin) = self.plugins.get_mut(&id) else {
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

				self.plugins.remove(&id).unwrap().deactivate(processor);
				self.timers_of_plugin
					.values_mut()
					.for_each(|set| _ = set.remove(&id));

				#[cfg(unix)]
				self.fds_of_plugin.remove(&id);

				return self.update(
					Message::MainThread(id, MainThreadMessage::GuiClosed),
					config,
				);
			}
			MainThreadMessage::GuiRequestShow => {
				let plugin = plugin!(MainThreadMessage::GuiRequestShow);

				if let Entry::Vacant(entry) = self.window_of_plugin.entry(id) {
					if !plugin.has_gui() {
						let (window, spawn) = window::open(window::Settings {
							size: (400.0, 600.0).into(),
							exit_on_close_request: false,
							level: window::Level::AlwaysOnTop,
							..window::Settings::default()
						});
						entry.insert(window);
						self.plugin_of_window.insert(window, id);

						return spawn.discard();
					} else if plugin.is_floating() {
						plugin.show();
					} else {
						let scale_factor = plugin.set_scale(
							config
								.plugin_scale_factor
								.unwrap_or(config.app_scale_factor),
						) / config.app_scale_factor;

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
						entry.insert(window);
						self.plugin_of_window.insert(window, id);

						let mut plugin = Fragile::new(self.plugins.remove(&id).unwrap());
						let embed = window::run(window, move |window| {
							// SAFETY:
							// The plugin gui is destroyed before the window is closed (see
							// [`Message::WindowCloseRequested`]).
							unsafe {
								plugin.get_mut().set_parent(window.window_handle().unwrap());
							}
							Message::GuiEmbedded(id, Box::new(plugin).into())
						});

						return spawn.discard().chain(embed);
					}
				}
			}
			MainThreadMessage::GuiRequestResize(size) => {
				let plugin = plugin!(MainThreadMessage::GuiRequestResize(size));

				if let Some(&window) = self.window_of_plugin.get(&id)
					&& let Some(scale_factor) = plugin.get_scale()
				{
					return window::resize(window, size.to_logical(scale_factor).into());
				}
			}
			MainThreadMessage::GuiRequestHide => {
				if let Some(&window) = self.window_of_plugin.get(&id) {
					return self.update(Message::WindowCloseRequested(window), config);
				}

				plugin!(MainThreadMessage::GuiRequestHide).hide();
			}
			MainThreadMessage::GuiClosed => {
				if let Some(&window) = self.window_of_plugin.get(&id) {
					return window::close(window);
				}
			}
			MainThreadMessage::RegisterTimer(timer_id, duration) => {
				self.timers_of_plugin
					.entry(duration)
					.or_default()
					.insert(id, timer_id);
			}
			MainThreadMessage::UnregisterTimer(timer_id) => {
				if let Some(set) = self
					.timers_of_plugin
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
				let Some(plugin) = self.plugins.get_mut(&id) else {
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
				let flags = self.fds_of_plugin.entry(id).or_default().insert(fd, flags);
				debug_assert!(flags.is_none());
			}
			PosixFdMessage::Modify(flags) => {
				let flags = self.fds_of_plugin.get_mut(&id).unwrap().insert(fd, flags);
				debug_assert!(flags.is_some());
			}
			PosixFdMessage::Unregister => {
				let flags = self.fds_of_plugin.get_mut(&id).unwrap().remove(&fd);
				debug_assert!(flags.is_some());
			}
		}

		Task::none()
	}

	pub fn view(&self, window: window::Id) -> Option<Element<'_, Message>> {
		let id = *self.plugin_of_window.get(&window)?;
		let Some(plugin) = &self.plugins.get(&id) else {
			return Some(space().into());
		};

		if plugin.has_gui() {
			return Some(space().into());
		}

		Some(
			column![
				text(&*plugin.descriptor().name)
					.size(LINE_HEIGHT)
					.line_height(1.0)
					.font(Font::MONOSPACE),
				container(rule::horizontal(1)).padding(padding::vertical(5)),
				scrollable(
					row(plugin.params().map(|param| {
						column![
							Knob::new(param.range.clone(), param.value, move |value| {
								Message::SendEvent(
									id,
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
							.maybe_tooltip(param.value_text.as_deref()),
							text(&*param.name).wrapping(text::Wrapping::WordOrGlyph)
						]
						.spacing(5)
						.width(70)
						.align_x(Center)
						.into()
					}))
					.spacing(10)
					.wrap()
					.vertical_spacing(10)
				)
				.width(Fill)
				.spacing(5)
				.style(scrollable_style)
			]
			.padding(10)
			.into(),
		)
	}

	pub fn title(&self, window: window::Id) -> Option<String> {
		self.plugin_of_window
			.get(&window)
			.and_then(|id| self.plugins.get(id))
			.map(|plugin| plugin.descriptor().name.deref().to_owned())
	}

	pub fn scale_factor(&self, window: window::Id) -> Option<f32> {
		self.plugin_of_window
			.get(&window)
			.and_then(|id| self.plugins.get(id))
			.and_then(Plugin::get_scale)
	}

	pub fn subscription(&self) -> Subscription<Message> {
		Subscription::batch(
			self.timers_of_plugin
				.iter()
				.filter(|(_, v)| !v.is_empty())
				.map(|(&k, _)| every(k).with(k).map(|(k, _)| Message::TickTimer(k)))
				.chain({
					#[cfg(unix)]
					{
						self.fds_of_plugin
							.iter()
							.flat_map(|(&k, v)| repeat(k).zip(v))
							.map(|(id, (&fd, &flags))| {
								Subscription::run_with((id, fd, flags), |&(id, fd, flags)| {
									// SAFETY:
									// This fd is owned by the plugin, and is open at least until
									// [`PosixFd::Unregister`] is processed. This fd is not -1.
									let async_fd =
										Async::new(unsafe { BorrowedFd::borrow_raw(fd) }).unwrap();

									unfold(async_fd, async |async_fd| {
										let msg = or(
											async {
												async_fd.readable().await.map_or_else(
													|_| FdFlags::ERROR,
													|()| FdFlags::READ,
												)
											},
											async {
												async_fd.writable().await.map_or_else(
													|_| FdFlags::ERROR,
													|()| FdFlags::WRITE,
												)
											},
										)
										.await;

										Some((msg, async_fd))
									})
									.filter(move |&msg| flags.intersects(msg))
									.map(PosixFdMessage::OnFd)
									.map(MainThreadMessage::PosixFd.with(fd))
									.map(Message::MainThread.with(id))
								})
							})
					}
					#[cfg(not(unix))]
					[]
				})
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

	pub fn plugin_load(
		&mut self,
		id: PluginId,
		mut plugin: Plugin<Event>,
		receiver: Receiver<MainThreadMessage>,
	) -> Task<Message> {
		plugin.activate();
		self.plugins.insert(id, plugin);
		let (sender, stream) = smol::channel::unbounded();
		Task::batch([
			Task::future(unblock(move || {
				while let Ok(msg) = receiver.recv()
					&& sender.try_send(msg).is_ok()
				{}
			}))
			.discard(),
			Task::run(stream, Message::MainThread.with(id)),
		])
	}

	pub fn set_realtime(&mut self, realtime: bool) {
		for plugin in self.plugins.values_mut() {
			plugin.set_realtime(realtime);
		}
	}

	pub fn get_state(&mut self, id: PluginId) -> Option<Vec<u8>> {
		self.plugins.get_mut(&id).unwrap().get_state()
	}

	pub fn set_state(&mut self, id: PluginId, state: &[u8]) {
		self.plugins.get_mut(&id).unwrap().set_state(state);
	}
}
