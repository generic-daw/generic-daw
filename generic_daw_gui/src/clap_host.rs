use crate::{components::space, config::Config, widget::LINE_HEIGHT};
use fragile::Fragile;
use generic_daw_core::{
	Event,
	clap_host::{MainThreadMessage, ParamInfoFlags, ParamRescanFlags, Plugin, PluginId, Size},
};
use generic_daw_utils::{HoleyVec, NoClone, NoDebug};
use generic_daw_widget::knob::Knob;
use iced::{
	Alignment::Center,
	Element, Font, Function as _,
	Length::{Fill, Shrink},
	Subscription, Task,
	time::every,
	widget::{column, container, horizontal_rule, row, sensor, text, text::Wrapping},
	window::{self, Id, Level, close_events, close_requests, resize_events},
};
use log::info;
use smol::{Timer, unblock};
use std::{ops::Deref as _, sync::mpsc::Receiver, time::Duration};

#[derive(Clone, Debug)]
pub enum Message {
	MainThread(PluginId, MainThreadMessage),
	SendEvent(PluginId, Event),
	TickTimer(usize, u32),
	Loaded(NoClone<(Box<Fragile<Plugin<Event>>>, Receiver<MainThreadMessage>)>),
	GuiShown(NoClone<Box<Fragile<Plugin<Event>>>>),
	GuiSetState(PluginId, NoDebug<Box<[u8]>>),
	GuiRequestResize(Id, Size),
	GuiRequestHide(Id),
	GuiHidden(Id),
	SetPluginSize(PluginId, Size),
}

#[derive(Default)]
pub struct ClapHost {
	plugins: HoleyVec<Plugin<Event>>,
	timers: HoleyVec<HoleyVec<Duration>>,
	windows: HoleyVec<Id>,
}

impl ClapHost {
	pub fn update(&mut self, message: Message, config: &Config) -> Task<Message> {
		match message {
			Message::MainThread(id, msg) => return self.main_thread_message(id, msg, config),
			Message::SendEvent(id, event) => {
				self.plugins.get_mut(*id).unwrap().send_event(event);
				if let Event::ParamValue { param_id, .. } = event {
					return self.update(
						Message::MainThread(
							id,
							MainThreadMessage::RescanParam(param_id, ParamRescanFlags::VALUES),
						),
						config,
					);
				}
			}
			Message::TickTimer(id, timer_id) => {
				self.plugins.get_mut(id).unwrap().tick_timer(timer_id);
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
			Message::GuiShown(NoClone(plugin)) => {
				let mut plugin = plugin.into_inner();
				let id = plugin.plugin_id();

				plugin.show();
				self.plugins.insert(*id, plugin);
			}
			Message::GuiSetState(plugin, state) => {
				self.plugins.get_mut(*plugin).unwrap().set_state(&state);
			}
			Message::GuiRequestResize(window, size) => {
				if let Some(id) = self.windows.key_of(&window)
					&& let Some(plugin) = self.plugins.get_mut(id)
					&& let Some(new_size) = plugin.resize(size)
					&& size.to_physical(config.scale_factor)
						!= new_size.to_physical(config.scale_factor)
				{
					return window::resize(window, new_size.to_logical(config.scale_factor).into());
				}
			}
			Message::GuiRequestHide(window) => {
				let Some(plugin) = self.windows.key_of(&window) else {
					return iced::exit();
				};

				self.plugins.get_mut(plugin).unwrap().destroy();
				return window::close(window);
			}
			Message::GuiHidden(window) => {
				let id = self.windows.key_of(&window).unwrap();
				self.windows.remove(id).unwrap();
			}
			Message::SetPluginSize(id, size) => {
				if let Some(&window) = self.windows.get(*id) {
					return window::resize(window, size.to_logical(config.scale_factor).into());
				}
			}
		}

		Task::none()
	}

	fn main_thread_message(
		&mut self,
		id: PluginId,
		msg: MainThreadMessage,
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

		match msg {
			MainThreadMessage::RequestCallback => {
				plugin!(MainThreadMessage::RequestCallback).call_on_main_thread_callback();
			}
			MainThreadMessage::LatencyChanged => {
				plugin!(MainThreadMessage::LatencyChanged).latency_changed();
			}
			MainThreadMessage::Restart(processor) => {
				let plugin = plugin!(MainThreadMessage::Restart(processor));
				plugin.deactivate(processor);
				plugin.activate();
			}
			MainThreadMessage::Destroy(processor) => {
				plugin!(MainThreadMessage::Destroy(processor));

				self.plugins.remove(*id).unwrap().deactivate(processor);
				return self.update(
					Message::MainThread(id, MainThreadMessage::GuiClosed),
					config,
				);
			}
			MainThreadMessage::GuiRequestShow => {
				plugin!(MainThreadMessage::GuiRequestShow);

				if self.windows.contains_key(*id) {
					return Task::none();
				}

				let mut plugin = self.plugins.remove(*id).unwrap();
				plugin.create();

				return if !plugin.has_gui() {
					let (window, spawn) = window::open(window::Settings {
						size: (400.0, 640.0).into(),
						resizable: false,
						exit_on_close_request: false,
						level: Level::AlwaysOnTop,
						..window::Settings::default()
					});
					self.windows.insert(*id, window);

					spawn.discard().chain(self.update(
						Message::GuiShown(NoClone(Box::new(Fragile::new(plugin)))),
						config,
					))
				} else if plugin.is_floating() {
					self.update(
						Message::GuiShown(NoClone(Box::new(Fragile::new(plugin)))),
						config,
					)
				} else {
					plugin.set_scale(config.scale_factor);

					let (window, spawn) = window::open(window::Settings {
						size: plugin.get_size().map_or((480.0, 640.0).into(), |size| {
							size.to_logical(config.scale_factor).into()
						}),
						resizable: plugin.can_resize(),
						exit_on_close_request: false,
						level: Level::AlwaysOnTop,
						..window::Settings::default()
					});
					self.windows.insert(*id, window);

					let mut plugin = Fragile::new(plugin);
					let embed = window::run_with_handle(window, move |handle| {
						// SAFETY:
						// The plugin gui is destroyed before the window is closed (see
						// [`Message::GuiRequestHide`]).
						unsafe {
							plugin.get_mut().set_parent(handle.as_raw());
						}
						plugin
					});

					spawn
						.discard()
						.chain(embed)
						.map(move |plugin| Message::GuiShown(NoClone(Box::new(plugin))))
				};
			}
			MainThreadMessage::GuiRequestResize(size) => {
				if let Some(&window) = self.windows.get(*id) {
					return self.update(Message::GuiRequestResize(window, size), config);
				}
			}
			MainThreadMessage::GuiRequestHide => {
				if let Some(&window) = self.windows.get(*id) {
					return self.update(Message::GuiRequestHide(window), config);
				}
			}
			MainThreadMessage::GuiClosed => {
				self.timers.remove(*id);
				if let Some(&window) = self.windows.get(*id) {
					return window::close(window);
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
			MainThreadMessage::RescanParams(flags) => {
				plugin!(MainThreadMessage::RescanParams(flags)).rescan_params(flags);
			}
			MainThreadMessage::RescanParam(param_id, flags) => {
				plugin!(MainThreadMessage::RescanParam(param_id, flags))
					.rescan_param(param_id, flags);
			}
		}

		Task::none()
	}

	pub fn plugin_gui(&self, window: Id) -> Option<Element<'_, Message>> {
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
					container(horizontal_rule(1)).padding([5, 0]),
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
								.reset(param.reset)
								.radius(25.0)
								.enabled(!param.flags.contains(ParamInfoFlags::IS_READONLY))
								.stepped(param.flags.contains(ParamInfoFlags::IS_STEPPED))
								.maybe_tooltip(param.value_text.as_deref())
							)
							.padding([0, 10]),
							text(&*param.name)
								.wrapping(Wrapping::WordOrGlyph)
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

	pub fn title(&self, window: Id) -> Option<String> {
		self.windows
			.key_of(&window)
			.and_then(|id| self.plugins.get(id))
			.map(|plugin| plugin.descriptor().name.deref().to_owned())
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
				.filter(|id| self.plugins.contains_key(*id))
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
					resize_events().map(|(id, size)| {
						Message::GuiRequestResize(
							id,
							Size::Logical {
								width: size.width,
								height: size.height,
							},
						)
					}),
					close_requests().map(Message::GuiRequestHide),
					close_events().map(Message::GuiHidden),
				]),
		)
	}

	pub fn clear(&mut self) {
		self.timers.clear();
		self.windows.clear();
	}
}
