use crate::{stylefns::scrollable_style, widget::LINE_HEIGHT};
use fragile::Fragile;
use generic_daw_core::{
	Event, PluginId,
	clap_host::{
		MainThreadMessage, ParamInfoFlags, Plugin, RenderMode, Size, StateContextType, TimerId,
	},
};
use generic_daw_widget::knob::Knob;
use iced::{
	Center, Element, Fill, Font, Subscription, Task, padding,
	time::every,
	widget::{column, container, row, rule, scrollable, space, text},
	window,
};
use log::info;
use smol::{Timer, unblock};
use std::{
	collections::{HashMap, HashSet},
	iter::repeat,
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
	GuiOpen(PluginId),
	GuiOpened(PluginId, NoClone<Box<Fragile<Plugin<Event>>>>),
	WindowResized(window::Id, iced::Size),
	WindowRescaled(window::Id, f32),
	WindowCloseRequested(window::Id),
	WindowClosed(window::Id),
}

#[derive(Debug)]
pub struct ClapHost {
	plugins: HashMap<PluginId, Plugin<Event>>,
	timers_of_duration: HashMap<Duration, HashMap<PluginId, HashSet<TimerId>>>,
	window_of_plugin: HashMap<PluginId, window::Id>,
	plugin_of_window: HashMap<window::Id, PluginId>,
	scale_factor_of_window: HashMap<window::Id, f32>,
	main_window_id: window::Id,
}

impl ClapHost {
	pub fn new(main_window_id: window::Id) -> Self {
		Self {
			plugins: HashMap::new(),
			timers_of_duration: HashMap::new(),
			window_of_plugin: HashMap::new(),
			plugin_of_window: HashMap::new(),
			scale_factor_of_window: HashMap::new(),
			main_window_id,
		}
	}

	pub fn update(&mut self, message: Message) -> Task<Message> {
		match message {
			Message::MainThread(id, msg) => {
				return self.handle_main_thread_message(id, msg);
			}
			Message::SendEvent(id, event) => self.plugins.get_mut(&id).unwrap().send_event(event),
			Message::TickTimer(duration) => {
				self.timers_of_duration
					.get(&duration)
					.into_iter()
					.flatten()
					.flat_map(|(id, timer_ids)| repeat(id).zip(timer_ids))
					.for_each(|(id, &timer_id)| {
						if let Some(plugin) = self.plugins.get_mut(id) {
							plugin.tick_timer(timer_id);
						}
					});
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
					let (window, spawn) = window::open(window::Settings {
						size: (640.0, 480.0).into(),
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

				if let Some(&window) = self.window_of_plugin.get(&id)
					&& let Some(&scale_factor) = self.scale_factor_of_window.get(&window)
				{
					return self.update(Message::WindowRescaled(window, scale_factor));
				}
			}
			Message::WindowResized(window, size) => {
				if let Some(&scale_factor) = self.scale_factor_of_window.get(&window)
					&& let Some(&id) = self.plugin_of_window.get(&window)
					&& let Some(plugin) = self.plugins.get_mut(&id)
					&& let size = size * scale_factor
					&& let size = Size::from_physical((size.width, size.height))
					&& let Some(new_size) = plugin.resize(size)
					&& let Some(plugin_scale) = plugin.get_scale()
					&& !size.approx_eq(new_size, plugin_scale)
				{
					return window::resize(window, new_size.to_logical(plugin_scale).into());
				}
			}
			Message::WindowRescaled(window, scale_factor) => {
				self.scale_factor_of_window.insert(window, scale_factor);

				if let Some(&plugin) = self.plugin_of_window.get(&window)
					&& let Some(plugin) = self.plugins.get_mut(&plugin)
				{
					let plugin_scale = plugin.set_scale(scale_factor);
					if let Some(size) = plugin.get_size() {
						return window::resize(window, size.to_logical(plugin_scale).into());
					}
				}
			}
			Message::WindowCloseRequested(window) => {
				if let Some(plugin) = self.plugin_of_window.get(&window) {
					self.plugins.get_mut(plugin).unwrap().destroy();
					return window::close(window);
				}
			}
			Message::WindowClosed(window) => {
				let id = self.plugin_of_window.remove(&window).unwrap();
				self.window_of_plugin.remove(&id).unwrap();
				self.scale_factor_of_window.remove(&window);
			}
		}

		Task::none()
	}

	fn handle_main_thread_message(
		&mut self,
		id: PluginId,
		message: MainThreadMessage,
	) -> Task<Message> {
		macro_rules! plugin {
			() => {
				plugin!(message)
			};
			($expr:expr) => {{
				let Some(plugin) = self.plugins.get_mut(&id) else {
					let message = Message::MainThread(id, $expr);
					info!("retrying {message:?}");
					return Task::perform(Timer::after(Duration::from_millis(100)), |_| message);
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
					.retain(|_, set| set.remove(&id).is_none() || !set.is_empty());

				return self.update(Message::MainThread(id, MainThreadMessage::GuiClosed));
			}
			MainThreadMessage::GuiRequestResize(size) => {
				if let Some(&window) = self.window_of_plugin.get(&id)
					&& let Some(plugin_scale) = plugin!().get_scale()
				{
					return window::resize(window, size.to_logical(plugin_scale).into());
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
		if let Some(&scale_factor) = self.scale_factor_of_window.get(&window)
			&& let Some(&plugin) = self.plugin_of_window.get(&window)
			&& let Some(plugin) = self.plugins.get(&plugin)
			&& let Some(plugin_scale) = plugin.get_scale()
		{
			Some(plugin_scale / scale_factor)
		} else {
			None
		}
	}

	pub fn subscription(&self) -> Subscription<Message> {
		Subscription::batch(
			self.timers_of_duration
				.iter()
				.filter(|(_, v)| v.values().any(|v| !v.is_empty()))
				.map(|(&k, _)| every(k).with(k).map(|(k, _)| Message::TickTimer(k)))
				.chain([window::events().filter_map(|(id, event)| match event {
					window::Event::Resized(size) => Some(Message::WindowResized(id, size)),
					window::Event::Opened { scale_factor, .. }
					| window::Event::Rescaled(scale_factor) => Some(Message::WindowRescaled(id, scale_factor)),
					window::Event::CloseRequested => Some(Message::WindowCloseRequested(id)),
					window::Event::Closed => Some(Message::WindowClosed(id)),
					_ => None,
				})]),
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
			Task::run(stream, move |msg| Message::MainThread(id, msg)),
		])
	}

	pub fn set_render_mode(&mut self, render_mode: RenderMode) {
		for plugin in self.plugins.values_mut() {
			plugin.set_render_mode(render_mode);
		}
	}

	pub fn get_state(&mut self, id: PluginId) -> Option<&[u8]> {
		self.plugins
			.get_mut(&id)
			.unwrap()
			.get_state(StateContextType::ForProject)
	}

	pub fn set_state(&mut self, id: PluginId, state: &[u8]) {
		self.plugins
			.get_mut(&id)
			.unwrap()
			.set_state(state, StateContextType::ForProject);
	}
}
