use crate::{
	action::Action,
	components::{PICK_LIST_HANDLE, number_input},
	config::{Config, Device},
	daw,
	icons::{grip_vertical, mic, plus, rotate_ccw, save, volume_2, x},
	stylefns::{
		button_with_radius, container_with_radius, menu_style, pick_list_with_radius,
		scrollable_style, sweeten_column_style, sweeten_column_with_radius, weak_bordered_box,
		weakest_bordered_box,
	},
	theme::Theme,
	widget::{LINE_HEIGHT, TEXT_HEIGHT},
};
use generic_daw_core::{
	DeviceDescription, DeviceId, HostId, clap_host::DEFAULT_CLAP_PATHS, default_host, get_devices,
	get_hosts,
};
use iced::{
	Center, Element, Fill, Font, Task, border, keyboard,
	mouse::Interaction,
	padding,
	widget::{
		button, center_x, checkbox, column, container, iced, mouse_area, opaque, pick_list, row,
		rule, scrollable, slider, space, text, value,
	},
	window,
};
use rfd::AsyncFileDialog;
use std::{collections::HashMap, num::NonZero, path::Path, sync::Arc};
use sweeten::widget::drag::DragEvent;
use utils::{ShiftMoveExt as _, natural_cmp};

const SAMPLE_RATES: [NonZero<u32>; 7] = [
	NonZero::new(44_100).unwrap(),
	NonZero::new(48_000).unwrap(),
	NonZero::new(64_000).unwrap(),
	NonZero::new(88_200).unwrap(),
	NonZero::new(96_000).unwrap(),
	NonZero::new(176_400).unwrap(),
	NonZero::new(192_000).unwrap(),
];

const BUFFER_SIZES: [NonZero<u32>; 10] = [
	NonZero::new(16).unwrap(),
	NonZero::new(32).unwrap(),
	NonZero::new(64).unwrap(),
	NonZero::new(128).unwrap(),
	NonZero::new(256).unwrap(),
	NonZero::new(512).unwrap(),
	NonZero::new(1024).unwrap(),
	NonZero::new(2048).unwrap(),
	NonZero::new(4096).unwrap(),
	NonZero::new(8192).unwrap(),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
	Input,
	Output,
}

#[derive(Clone, Debug)]
pub enum Message {
	AddSamplePathFileDialog,
	AddSamplePath(Arc<Path>),
	RemoveSamplePath(usize),
	MoveSamplePath(DragEvent),
	AddClapPathFileDialog,
	AddClapPath(Arc<Path>),
	RemoveClapPath(usize),
	MoveClapPath(DragEvent),
	ChangedTab(Tab),
	ChangedHost(Option<HostId>),
	ChangedDeviceId(Option<DeviceId>),
	ChangedSampleRate(NonZero<u32>),
	ChangedBufferSize(Option<NonZero<u32>>),
	ToggledAutosave,
	ChangedAutosaveInterval(u16),
	ChangedAutosaveIntervalText(String),
	ToggledOpenLastProject,
	ChangedTheme(Theme),
	ChangedScaleFactor(f32),
	WriteConfig,
	ResetConfigToPrev,
}

#[derive(Debug)]
pub struct ConfigView {
	config: Config,
	prev_config: Config,
	tab: Tab,
	hosts: Vec<HostId>,
	default_host: HostId,
	devices: HashMap<HostId, Devices>,
	device_info: HashMap<DeviceId, DeviceDescription>,
	main_window_id: window::Id,
}

#[derive(Debug, Default)]
struct Devices {
	input: Vec<DeviceId>,
	output: Vec<DeviceId>,
}

impl ConfigView {
	pub fn new(main_window_id: window::Id) -> Self {
		let device_info = get_devices();

		let mut devices =
			device_info
				.iter()
				.fold(HashMap::<_, Devices>::new(), |mut acc, (id, device)| {
					if device.supports_input() {
						acc.entry(id.0).or_default().input.push(id.clone());
					}

					if device.supports_output() {
						acc.entry(id.0).or_default().output.push(id.clone());
					}

					acc
				});

		for device in devices.values_mut() {
			device.input.sort_unstable_by(|l, r| {
				natural_cmp(
					device_info[l].name().as_bytes(),
					device_info[r].name().as_bytes(),
				)
			});

			device.output.sort_unstable_by(|l, r| {
				natural_cmp(
					device_info[l].name().as_bytes(),
					device_info[r].name().as_bytes(),
				)
			});
		}

		let config = Config::read();

		Self {
			config: config.clone(),
			prev_config: config,
			tab: Tab::Output,
			hosts: get_hosts(),
			default_host: default_host(),
			devices,
			device_info,
			main_window_id,
		}
	}

	pub fn update(&mut self, message: Message) -> Action<Config, Message> {
		match message {
			Message::AddSamplePathFileDialog => {
				return window::run(self.main_window_id, |window| {
					AsyncFileDialog::new().set_parent(window).pick_folder()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().into())
				.map(Message::AddSamplePath)
				.into();
			}
			Message::AddSamplePath(path) => self.config.sample_paths.push(path),
			Message::RemoveSamplePath(index) => _ = self.config.sample_paths.remove(index),
			Message::MoveSamplePath(event) => {
				if let DragEvent::Dropped {
					index,
					target_index,
				} = event && index != target_index
				{
					self.config.sample_paths.shift_move(index, target_index);
				}
			}
			Message::AddClapPathFileDialog => {
				return window::run(self.main_window_id, |window| {
					AsyncFileDialog::new().set_parent(window).pick_folder()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().into())
				.map(Message::AddClapPath)
				.into();
			}
			Message::AddClapPath(path) => self.config.clap_paths.push(path),
			Message::RemoveClapPath(index) => _ = self.config.clap_paths.remove(index),
			Message::MoveClapPath(event) => {
				if let DragEvent::Dropped {
					index,
					target_index,
				} = event && index != target_index
				{
					self.config.clap_paths.shift_move(index, target_index);
				}
			}
			Message::ChangedTab(tab) => self.tab = tab,
			Message::ChangedHost(host) => self.config.devices.host = host,
			Message::ChangedDeviceId(id) => self.with_device_mut(|device| {
				device.id = id;
			}),
			Message::ChangedSampleRate(sample_rate) => self.with_device_mut(|device| {
				device.sample_rate = sample_rate;
			}),
			Message::ChangedBufferSize(buffer_size) => self.with_device_mut(|device| {
				device.buffer_size = buffer_size;
			}),
			Message::ToggledAutosave => self.config.autosave.enabled ^= true,
			Message::ChangedAutosaveInterval(interval) => {
				self.config.autosave.interval = NonZero::new(interval.clamp(1, 999)).unwrap();
			}
			Message::ChangedAutosaveIntervalText(text) => {
				if let Ok(interval) = text.parse() {
					return self.update(Message::ChangedAutosaveInterval(interval));
				}
			}
			Message::ToggledOpenLastProject => self.config.open_last_project ^= true,
			Message::ChangedTheme(theme) => self.config.theme = theme,
			Message::ChangedScaleFactor(scale_factor) => {
				self.config.scale_factor = scale_factor;
			}
			Message::WriteConfig => {
				self.config.write();
				self.prev_config = self.config.clone();
				return Action::instruction(self.config.clone());
			}
			Message::ResetConfigToPrev => self.config = self.prev_config.clone(),
		}

		Action::none()
	}

	pub fn view(&self, live_config: &Config) -> Element<'_, Message> {
		let (device, devices) = match self.tab {
			Tab::Input => (
				&self.config.devices.input,
				&*self.devices[&self.config.devices.host.unwrap_or(self.default_host)].input,
			),
			Tab::Output => (
				&self.config.devices.output,
				&*self.devices[&self.config.devices.host.unwrap_or(self.default_host)].output,
			),
		};

		container(
			scrollable(
				column![
					text("Settings")
						.size(LINE_HEIGHT)
						.line_height(1.0)
						.font(Font::MONOSPACE),
					container(rule::horizontal(1)).padding(padding::vertical(5)),
					row![
						"Sample Paths",
						space::horizontal(),
						button(plus())
							.style(button_with_radius(button::primary, 5))
							.padding(0)
							.on_press(Message::AddSamplePathFileDialog),
						space().width(5)
					]
					.align_y(Center),
					container(
						sweeten::column(
							self.config
								.sample_paths
								.iter()
								.enumerate()
								.map(|(index, path)| {
									row![
										value(path.display())
											.font(Font::MONOSPACE)
											.wrapping(text::Wrapping::None)
											.ellipsis(text::Ellipsis::Middle)
											.width(Fill),
										button(x())
											.style(button_with_radius(button::danger, 5))
											.padding(0)
											.on_press(Message::RemoveSamplePath(index))
									]
									.spacing(5)
									.align_y(Center)
								})
								.map(|widget| row![
									mouse_area(grip_vertical()).interaction(Interaction::Grab),
									opaque(widget)
								]
								.align_y(Center)
								.into())
						)
						.padding(padding::all(5).left(2))
						.spacing(5)
						.on_drag(Message::MoveSamplePath)
						.style(sweeten_column_with_radius(sweeten_column_style, 5))
					)
					.style(container_with_radius(weak_bordered_box, 5)),
					rule::horizontal(1),
					row![
						"CLAP Plugin Paths",
						space::horizontal(),
						button(plus())
							.style(button_with_radius(button::primary, 5))
							.padding(0)
							.on_press(Message::AddClapPathFileDialog),
						space().width(5)
					],
					container(
						column![
							column(DEFAULT_CLAP_PATHS.iter().map(|path| {
								row![
									mouse_area(grip_vertical()).interaction(Interaction::NoDrop),
									value(path.display())
										.font(Font::MONOSPACE)
										.width(Fill)
										.wrapping(text::Wrapping::None)
										.ellipsis(text::Ellipsis::Middle),
									button(x())
										.style(button_with_radius(button::danger, 5))
										.padding(0)
								]
								.spacing(5)
								.align_y(Center)
								.into()
							}))
							.spacing(5),
							(!self.config.clap_paths.is_empty()).then(|| sweeten::column(
								self.config
									.clap_paths
									.iter()
									.enumerate()
									.map(|(index, path)| {
										row![
											value(path.display())
												.font(Font::MONOSPACE)
												.width(Fill)
												.wrapping(text::Wrapping::None)
												.ellipsis(text::Ellipsis::Middle),
											button(x())
												.style(button_with_radius(button::danger, 5))
												.padding(0)
												.on_press(Message::RemoveClapPath(index))
										]
										.spacing(5)
										.align_y(Center)
									})
									.map(|widget| row![
										mouse_area(grip_vertical()).interaction(Interaction::Grab),
										opaque(widget)
									]
									.align_y(Center)
									.into())
							)
							.spacing(5)
							.on_drag(Message::MoveClapPath)
							.style(sweeten_column_with_radius(sweeten_column_style, 5)))
						]
						.padding(padding::all(5).left(2))
						.spacing(5)
					)
					.style(container_with_radius(weak_bordered_box, 5)),
					rule::horizontal(1),
					row![
						button(mic())
							.style(button_with_radius(button::primary, border::left(5)))
							.padding(5)
							.on_press_maybe(
								(self.tab != Tab::Input).then_some(Message::ChangedTab(Tab::Input))
							),
						button(volume_2())
							.style(button_with_radius(button::primary, border::right(5)))
							.padding(5)
							.on_press_maybe(
								(self.tab != Tab::Output)
									.then_some(Message::ChangedTab(Tab::Output))
							),
						center_x(device.buffer_size.map(|buffer_size| {
							text!(
								"{buffer_size} smp @ {} hz = {:.1} ms",
								device.sample_rate,
								buffer_size.get() as f32 / device.sample_rate.get() as f32 * 1000.0
							)
							.font(Font::MONOSPACE)
							.size(13)
						})),
						match self.tab {
							Tab::Input => "Input",
							Tab::Output => "Output",
						}
					]
					.align_y(Center),
					row![
						text("Host:").width(Fill),
						row![
							pick_list(self.config.devices.host.as_ref(), &*self.hosts, |host| host
								.name()
								.to_owned())
							.on_select(|host| Message::ChangedHost(Some(host)))
							.handle(PICK_LIST_HANDLE)
							.placeholder("Default")
							.width(Fill)
							.style(pick_list_with_radius(border::left(5)))
							.menu_style(menu_style),
							button(rotate_ccw())
								.style(button_with_radius(button::primary, border::right(5)))
								.padding(5)
								.on_press_maybe(
									self.config
										.devices
										.host
										.as_ref()
										.map(|_| Message::ChangedHost(None))
								)
						]
					]
					.align_y(Center),
					column![
						row![
							text("Name:").width(Fill),
							row![
								pick_list(
									device
										.id
										.as_ref()
										.filter(|id| self.device_info.contains_key(id)),
									devices,
									|id| self.device_info[id].to_string()
								)
								.on_select(|id| Message::ChangedDeviceId(Some(id)))
								.handle(PICK_LIST_HANDLE)
								.placeholder("Default")
								.width(Fill)
								.style(pick_list_with_radius(border::top_left(5)))
								.menu_style(menu_style),
								button(rotate_ccw())
									.style(button_with_radius(
										button::primary,
										border::top_right(5)
									))
									.padding(5)
									.on_press_maybe(
										device.id.as_ref().map(|_| Message::ChangedDeviceId(None))
									)
							]
						]
						.align_y(Center),
						row![
							text("Sample Rate:").width(Fill),
							row![
								pick_list(
									Some(device.sample_rate),
									SAMPLE_RATES,
									|sample_rate| format!("{sample_rate} hz")
								)
								.on_select(Message::ChangedSampleRate)
								.handle(PICK_LIST_HANDLE)
								.placeholder("Default")
								.width(Fill)
								.style(pick_list_with_radius(0))
								.menu_style(menu_style),
								button(rotate_ccw())
									.style(button_with_radius(button::primary, 0))
									.padding(5)
									.on_press_maybe((device.sample_rate.get() != 44100).then_some(
										Message::ChangedSampleRate(NonZero::new(44100).unwrap())
									))
							]
						]
						.align_y(Center),
						row![
							text("Buffer Size:").width(Fill),
							row![
								pick_list(device.buffer_size, BUFFER_SIZES, |buffer_size| format!(
									"{buffer_size} smp"
								))
								.on_select(|buffer_size| {
									Message::ChangedBufferSize(Some(buffer_size))
								})
								.handle(PICK_LIST_HANDLE)
								.placeholder("Default")
								.width(Fill)
								.style(pick_list_with_radius(border::bottom_left(5)))
								.menu_style(menu_style),
								button(rotate_ccw())
									.style(button_with_radius(
										button::primary,
										border::bottom_right(5)
									))
									.padding(5)
									.on_press_maybe(
										device
											.buffer_size
											.map(|_| Message::ChangedBufferSize(None))
									)
							]
						]
						.align_y(Center)
					],
					rule::horizontal(1),
					row![
						row![
							checkbox(self.config.autosave.enabled)
								.label("Autosave every ")
								.on_toggle(|_| Message::ToggledAutosave),
							number_input(
								1..=999,
								self.config.autosave.interval.get().into(),
								600,
								|interval| Message::ChangedAutosaveInterval(interval as u16),
								Message::ChangedAutosaveIntervalText,
								5
							),
							" s"
						]
						.width(Fill)
						.align_y(Center),
						container(
							checkbox(self.config.open_last_project)
								.label("Open last project on startup")
								.on_toggle(|_| Message::ToggledOpenLastProject)
						)
						.width(Fill)
					]
					.align_y(Center),
					rule::horizontal(1),
					row![
						"Scale factor:",
						text!("{:.1}", self.config.scale_factor).font(Font::MONOSPACE),
						slider(
							-1.0..=1.0,
							self.config.scale_factor.log2(),
							|scale_factor| Message::ChangedScaleFactor(
								(scale_factor.exp2() * 10.0).round() / 10.0
							)
						)
						.step(f32::EPSILON),
						button(rotate_ccw())
							.style(button_with_radius(button::primary, 5))
							.padding(5)
							.on_press_maybe(
								(self.config.scale_factor != 1.0)
									.then_some(Message::ChangedScaleFactor(1.0))
							),
					]
					.align_y(Center)
					.spacing(10),
					row![
						text("Theme:").width(Fill),
						row![
							pick_list(Some(self.config.theme), Theme::VARIANTS, |&t| {
								iced::Theme::from(t).to_string()
							})
							.on_select(Message::ChangedTheme)
							.handle(PICK_LIST_HANDLE)
							.width(Fill)
							.style(pick_list_with_radius(border::left(5)))
							.menu_style(menu_style),
							button(rotate_ccw())
								.style(button_with_radius(button::primary, border::right(5)))
								.padding(5)
								.on_press_maybe(
									(self.config.theme != Theme::CatppuccinFrappe)
										.then_some(Message::ChangedTheme(Theme::CatppuccinFrappe))
								)
						]
					]
					.align_y(Center),
					rule::horizontal(1),
					row![
						if self.config.is_mergeable(live_config) {
							iced(TEXT_HEIGHT)
						} else {
							container("Some changes may only take effect after a reload!")
								.padding(padding::horizontal(10).vertical(5))
								.style(container_with_radius(container::warning, f32::INFINITY))
								.into()
						},
						space::horizontal(),
						button(save())
							.style(button_with_radius(button::primary, border::left(5)))
							.padding(5)
							.on_press_maybe(
								(self.config != self.prev_config).then_some(Message::WriteConfig)
							),
						button(rotate_ccw())
							.style(button_with_radius(button::primary, border::right(5)))
							.padding(5)
							.on_press_maybe(
								(self.config != self.prev_config)
									.then_some(Message::ResetConfigToPrev)
							)
					]
					.align_y(Center)
				]
				.spacing(10)
				.padding(10)
				.width(540),
			)
			.spacing(5)
			.style(scrollable_style),
		)
		.style(container_with_radius(weakest_bordered_box, 5))
		.into()
	}

	pub fn keybinds(
		key: &keyboard::Key,
		modifiers: keyboard::Modifiers,
		repeat: bool,
	) -> Option<daw::Message> {
		match (
			modifiers.command(),
			modifiers.shift(),
			modifiers.alt(),
			repeat,
		) {
			(false, false, false, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::Escape) => {
					Some(daw::Message::CloseConfigView)
				}
				_ => None,
			},
			_ => None,
		}
	}

	fn with_device_mut<T>(&mut self, f: impl FnOnce(&mut Device) -> T) -> T {
		match self.tab {
			Tab::Input => f(&mut self.config.devices.input),
			Tab::Output => f(&mut self.config.devices.output),
		}
	}
}
