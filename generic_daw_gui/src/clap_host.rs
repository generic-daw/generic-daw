use crate::{config::Config, stylefns::scrollable_style, widget::LINE_HEIGHT};
use fragile::Fragile;
#[cfg(unix)]
use generic_daw_core::clap_host::FdFlags;
use generic_daw_core::{
	Event, PluginId,
	clap_host::{MainThreadMessage, ParamInfoFlags, Plugin, Size, TimerId},
};
use generic_daw_widget::knob::Knob;
#[cfg(unix)]
use iced::task::Handle;
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
use smol::{Async, future::or};
use smol::{Timer, unblock};
#[cfg(unix)]
use std::os::fd::{BorrowedFd, RawFd};
use std::{
	collections::{HashMap, HashSet},
	ops::Deref as _,
	sync::mpsc::Receiver,
	time::Duration,
};
use utils::NoClone;

#[derive(Clone, Debug)]
pub enum Message {
	MainThread(PluginId, MainThreadMessage),
	SendEvent(PluginId, Event),
	TickTimer(Duration),
	#[cfg(unix)]
	OnFd(PluginId, RawFd, FdFlags),
	GuiOpen(PluginId),
	GuiOpened(PluginId, NoClone<Box<Fragile<Plugin<Event>>>>),
	WindowResized(window::Id, Size),
	WindowCloseRequested(window::Id),
	WindowClosed(window::Id),
}

#[derive(Debug)]
pub struct ClapHost {
	plugins: HashMap<PluginId, Plugin<Event>>,
	timers_of_duration: HashMap<Duration, HashMap<PluginId, HashSet<TimerId>>>,
	window_of_plugin: HashMap<PluginId, window::Id>,
	plugin_of_window: HashMap<window::Id, PluginId>,
	#[cfg(unix)]
	fds_of_plugin: HashMap<PluginId, HashMap<RawFd, (FdFlags, Handle)>>,
	main_window_id: window::Id,
}

impl ClapHost {
	pub fn new(main_window_id: window::Id) -> Self {
		Self {
			plugins: HashMap::new(),
			timers_of_duration: HashMap::new(),
			window_of_plugin: HashMap::new(),
			plugin_of_window: HashMap::new(),
			#[cfg(unix)]
			fds_of_plugin: HashMap::new(),
			main_window_id,
		}
	}

	pub fn update(&mut self, message: Message, config: &Config) -> Task<Message> {
		match message {
			Message::MainThread(id, msg) => {
				return self.handle_main_thread_message(id, msg, config);
			}
			Message::SendEvent(id, event) => self.plugins.get_mut(&id).unwrap().send_event(event),
			Message::TickTimer(duration) => {
				for (&id, timer_ids) in &self.timers_of_duration[&duration] {
					for &timer_id in timer_ids {
						if let Some(plugin) = self.plugins.get_mut(&id) {
							plugin.tick_timer(timer_id);
						}
					}
				}
			}
			#[cfg(unix)]
			Message::OnFd(id, fd, flag) => {
				if let Some(fds) = self.fds_of_plugin.get(&id)
					&& let Some(&(flags, _)) = fds.get(&fd)
					&& flag.intersects(flags)
				{
					if let Some(plugin) = self.plugins.get_mut(&id) {
						plugin.on_fd(fd, flag);
					}

					return self.handle_main_thread_message(
						id,
						MainThreadMessage::RegisterFd(fd, flags),
						config,
					);
				}
			}
			Message::GuiOpen(id) => {
				let Some(plugin) = self.plugins.get_mut(&id) else {
					info!("retrying {message:?}");
					return Task::perform(Timer::after(Duration::from_millis(100)), |_| message);
				};

				return if plugin.is_shown() {
					Task::none()
				} else if !plugin.has_gui() {
					let (window, spawn) = window::open(window::Settings {
						size: (400.0, 600.0).into(),
						exit_on_close_request: false,
						level: window::Level::AlwaysOnTop,
						..window::Settings::default()
					});
					self.window_of_plugin.insert(id, window);
					self.plugin_of_window.insert(window, id);

					plugin.show();

					spawn.discard()
				} else if plugin.is_floating() {
					let mut plugin = Fragile::new(self.plugins.remove(&id).unwrap());
					window::run(self.main_window_id, move |window| {
						// SAFETY:
						// The plugin gui is destroyed before the window is closed (see
						// [`Message::WindowCloseRequested`]).
						unsafe { plugin.get_mut().set_transient(window) }
						Message::GuiOpened(id, Box::new(plugin).into())
					})
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
					self.window_of_plugin.insert(id, window);
					self.plugin_of_window.insert(window, id);

					let mut plugin = Fragile::new(self.plugins.remove(&id).unwrap());
					let embed = window::run(window, move |window| {
						// SAFETY:
						// The plugin gui is destroyed before the window is closed (see
						// [`Message::WindowCloseRequested`]).
						unsafe { plugin.get_mut().set_parent(window) }
						Message::GuiOpened(id, Box::new(plugin).into())
					});

					spawn.discard().chain(embed)
				};
			}
			Message::GuiOpened(id, NoClone(plugin)) => {
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
					for plugin in self.plugins.values_mut() {
						plugin.destroy();
					}
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
			() => {
				plugin!(message)
			};
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
			MainThreadMessage::RequestCallback => plugin!().call_on_main_thread_callback(),
			MainThreadMessage::Restart(processor) => {
				let plugin = plugin!(MainThreadMessage::Restart(processor));
				plugin.deactivate(processor);
				plugin.activate();
			}
			MainThreadMessage::Destroy(processor) => {
				plugin!(MainThreadMessage::Destroy(processor));

				self.plugins.remove(&id).unwrap().deactivate(processor);
				self.timers_of_duration
					.values_mut()
					.for_each(|set| _ = set.remove(&id));

				#[cfg(unix)]
				self.fds_of_plugin.remove(&id);

				return self.update(
					Message::MainThread(id, MainThreadMessage::GuiClosed),
					config,
				);
			}
			MainThreadMessage::GuiRequestResize(size) => {
				if let Some(&window) = self.window_of_plugin.get(&id)
					&& let Some(scale_factor) = plugin!().get_scale()
				{
					return window::resize(window, size.to_logical(scale_factor).into());
				}
			}
			MainThreadMessage::GuiRequestShow => plugin!().show(),
			MainThreadMessage::GuiRequestHide => plugin!().hide(),
			MainThreadMessage::GuiClosed => {
				if let Some(&window) = self.window_of_plugin.get(&id) {
					return window::close(window);
				}
			}
			MainThreadMessage::RegisterTimer(timer_id, duration) => {
				self.timers_of_duration
					.entry(duration)
					.or_default()
					.entry(id)
					.or_default()
					.insert(timer_id);
			}
			MainThreadMessage::UnregisterTimer(timer_id) => {
				if let Some(timers) = self
					.timers_of_duration
					.values_mut()
					.filter_map(|timers| timers.get_mut(&id))
					.find(|timers| timers.contains(&timer_id))
				{
					timers.remove(&timer_id);
				}
			}
			MainThreadMessage::RescanParams(flags) => plugin!().rescan_params(flags),
			MainThreadMessage::RescanParam(param_id, flags) => {
				plugin!().rescan_param(param_id, flags);
			}
			#[cfg(unix)]
			MainThreadMessage::RegisterFd(fd, flags) => {
				let (task, handle) = Task::future(async move {
					// SAFETY:
					// This fd is owned by the plugin, and is open at least until
					// [`PosixFd::Unregister`] is processed. This fd is not -1.
					let async_fd = Async::new(unsafe { BorrowedFd::borrow_raw(fd) }).unwrap();

					macro_rules! flag {
						($flag:expr, $fut:expr) => {
							async {
								if flags.contains($flag) {
									if $fut.await.is_ok() {
										$flag
									} else if flags.contains(FdFlags::ERROR) {
										FdFlags::ERROR
									} else {
										smol::future::pending().await
									}
								} else {
									smol::future::pending().await
								}
							}
						};
					}

					or(
						flag!(FdFlags::READ, async_fd.readable()),
						flag!(FdFlags::WRITE, async_fd.writable()),
					)
					.await
				})
				.abortable();

				self.fds_of_plugin
					.entry(id)
					.or_default()
					.insert(fd, (flags, handle.abort_on_drop()));

				return task.map(move |flag| Message::OnFd(id, fd, flag));
			}
			#[cfg(unix)]
			MainThreadMessage::UnregisterFd(fd) => {
				if let Some(fds) = self.fds_of_plugin.get_mut(&id) {
					fds.remove(&fd);
				}
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
			self.timers_of_duration
				.iter()
				.filter(|(_, v)| v.values().any(|v| !v.is_empty()))
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
				for msg in receiver {
					if sender.try_send(msg).is_err() {
						return;
					}
				}
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
