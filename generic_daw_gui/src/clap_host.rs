use crate::{components::space, config::Config, widget::LINE_HEIGHT};
use fragile::Fragile;
use generic_daw_core::{
	Event,
	clap_host::{MainThreadMessage, Plugin, PluginId},
};
use generic_daw_utils::HoleyVec;
use generic_daw_widget::knob::Knob;
use iced::{
	Alignment::Center,
	Element, Font, Function as _,
	Length::{Fill, Shrink},
	Size, Subscription, Task,
	time::every,
	widget::{column, container, horizontal_rule, row, text, text::Wrapping},
	window::{self, Id, close_events, close_requests, resize_events},
};
use log::info;
use smol::{Timer, channel::Receiver};
use std::{ops::Deref as _, sync::Arc, time::Duration};

#[derive(Clone, Debug)]
pub enum Message {
	MainThread(PluginId, MainThreadMessage<Event>),
	SendEvent(PluginId, Event),
	TickTimer(usize, u32),
	Loaded(
		Arc<Fragile<Plugin<Event>>>,
		Receiver<MainThreadMessage<Event>>,
	),
	GuiShown(Arc<Fragile<Plugin<Event>>>),
	GuiSetState(PluginId, Box<[u8]>),
	GuiRequestResize(Id, Size),
	GuiRequestHide(Id),
	GuiHidden(Id),
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
				return self.update(
					Message::MainThread(id, MainThreadMessage::LiveEvent(event)),
					config,
				);
			}
			Message::TickTimer(id, timer_id) => {
				self.plugins.get_mut(id).unwrap().tick_timer(timer_id);
			}
			Message::Loaded(plugin, receiver) => {
				let plugin = Arc::into_inner(plugin).unwrap().into_inner();
				let id = plugin.plugin_id();
				self.plugins.insert(*id, plugin);
				return Task::stream(receiver).map(Message::MainThread.with(id));
			}
			Message::GuiShown(plugin) => {
				let mut plugin = Arc::into_inner(plugin).unwrap().into_inner();
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
					&& let Some([width, height]) = plugin.resize(size.width, size.height)
				{
					let new_size = Size::new(width, height);
					if size != new_size {
						return window::resize(window, new_size);
					}
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
		}

		Task::none()
	}

	fn main_thread_message(
		&mut self,
		id: PluginId,
		msg: MainThreadMessage<Event>,
		config: &Config,
	) -> Task<Message> {
		let Some(plugin) = self.plugins.get_mut(*id) else {
			let msg = Message::MainThread(id, msg);
			info!("retrying {msg:?}");
			return Task::perform(Timer::after(Duration::from_millis(100)), |_| msg);
		};

		match msg {
			MainThreadMessage::RequestCallback => plugin.call_on_main_thread_callback(),
			MainThreadMessage::GuiRequestShow => {
				if self.windows.contains_key(*id) {
					return Task::none();
				}

				let mut plugin = self.plugins.remove(*id).unwrap();
				plugin.create();

				return if !plugin.has_gui() {
					let (window, spawn) = window::open(window::Settings {
						size: Size::new(480.0, 640.0),
						resizable: true,
						exit_on_close_request: false,
						..window::Settings::default()
					});
					self.windows.insert(*id, window);

					spawn.discard().chain(
						self.update(Message::GuiShown(Arc::new(Fragile::new(plugin))), config),
					)
				} else if plugin.is_floating() {
					self.update(Message::GuiShown(Arc::new(Fragile::new(plugin))), config)
				} else {
					plugin.set_scale(config.scale_factor);

					let (window, spawn) = window::open(window::Settings {
						size: plugin
							.get_size()
							.map_or(Size::new(480.0, 640.0), |[width, height]| {
								Size::new(width, height)
							}),
						resizable: plugin.can_resize(),
						exit_on_close_request: false,
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
						.map(move |plugin| Message::GuiShown(Arc::new(plugin)))
				};
			}
			MainThreadMessage::GuiRequestResize([width, height]) => {
				if let Some(&window) = self.windows.get(*id) {
					return self.update(
						Message::GuiRequestResize(window, Size::new(width, height)),
						config,
					);
				}
			}
			MainThreadMessage::GuiRequestHide => {
				if let Some(&window) = self.windows.get(*id) {
					return self.update(Message::GuiRequestHide(window), config);
				}
			}
			MainThreadMessage::GuiClosed => {
				self.plugins.remove(*id).unwrap();
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
			MainThreadMessage::LatencyChanged => plugin.latency_changed(),
			MainThreadMessage::RescanValues => plugin.rescan_values(),
			MainThreadMessage::LiveEvent(msg) => return Self::live_event(plugin, msg),
		}

		Task::none()
	}

	fn live_event(plugin: &mut Plugin<Event>, msg: Event) -> Task<Message> {
		if let Event::ParamValue {
			param_id, value, ..
		} = msg
		{
			plugin.update_param(param_id, value);
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
			column![
				text(&*plugin.descriptor().name)
					.size(LINE_HEIGHT)
					.line_height(1.0)
					.font(Font::MONOSPACE),
				container(horizontal_rule(1)).padding([5, 0]),
				row(plugin.params().map(|param| {
					column![
						container(
							Knob::new(param.range.clone(), param.value, true, |value| {
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
							.maybe_tooltip(
								(!param.value_text.is_empty()).then_some(&param.value_text)
							)
							.reset(param.reset)
							.radius(25.0)
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
			.padding(10)
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
					resize_events().map(|(id, size)| Message::GuiRequestResize(id, size)),
					close_requests().map(Message::GuiRequestHide),
					close_events().map(Message::GuiHidden),
				]),
		)
	}
}
