use crate::{
	action::Action,
	components::{PICK_LIST_HANDLE, number_input},
	config::Config,
	daw,
	icons::{grip_vertical, plus, rotate_ccw, save, x},
	stylefns::{
		button_with_radius, container_with_radius, menu_style, pick_list_with_radius,
		scrollable_style, sweeten_column_style, sweeten_column_with_radius, weak_bordered_box,
		weakest_bordered_box,
	},
	theme::Theme,
	widget::{LINE_HEIGHT, TEXT_HEIGHT},
};
use generic_daw_core::{
	DEFAULT_HOST, DeviceDescription, DeviceId, HostId, clap_host::DEFAULT_CLAP_PATHS, get_hosts,
	get_input_devices, get_input_ports, get_output_devices, get_output_ports,
};
use iced::{
	Center, Element, Fill, Font, Task, border, keyboard,
	mouse::Interaction,
	padding,
	widget::{
		button, checkbox, column, container, iced, mouse_area, opaque, pick_list, row, rule,
		scrollable, slider, space, text, value,
	},
	window,
};
use rfd::AsyncFileDialog;
use std::{collections::HashMap, num::NonZero, path::Path, sync::Arc};
use sweeten::widget::drag::DragEvent;
use utils::{ShiftMoveExt as _, natural_cmp};

const SAMPLE_RATES: [NonZero<u32>; 6] = [
	NonZero::new(44_100).unwrap(),
	NonZero::new(48_000).unwrap(),
	NonZero::new(88_200).unwrap(),
	NonZero::new(96_000).unwrap(),
	NonZero::new(176_400).unwrap(),
	NonZero::new(192_000).unwrap(),
];

const BUFFER_SIZES: [NonZero<u32>; 7] = [
	NonZero::new(32).unwrap(),
	NonZero::new(64).unwrap(),
	NonZero::new(128).unwrap(),
	NonZero::new(256).unwrap(),
	NonZero::new(512).unwrap(),
	NonZero::new(1024).unwrap(),
	NonZero::new(2048).unwrap(),
];

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
	ChangedHost(Option<HostId>),
	ChangedAudioInput(Option<DeviceId>),
	ChangedAudioOutput(Option<DeviceId>),
	ChangedSampleRate(Option<NonZero<u32>>),
	ChangedBufferSize(Option<NonZero<u32>>),
	ChangedMidiInput(Option<Arc<str>>),
	ChangedMidiOutput(Option<Arc<str>>),
	ToggledAutosave,
	ChangedAutosaveInterval(Option<u16>),
	ToggledOpenLastProject,
	ChangedTheme(Theme),
	ChangedScaleFactor(f32),
	WriteConfig,
	ResetConfigToLast,
}

#[derive(Debug)]
pub struct ConfigView {
	config: Config,
	hosts: Box<[HostId]>,
	input_devices: HashMap<HostId, Vec<DeviceId>>,
	input_device_info: HashMap<DeviceId, DeviceDescription>,
	output_devices: HashMap<HostId, Vec<DeviceId>>,
	output_device_info: HashMap<DeviceId, DeviceDescription>,
	input_ports: Box<[Arc<str>]>,
	input_port_info: HashMap<Arc<str>, Arc<str>>,
	output_ports: Box<[Arc<str>]>,
	output_port_info: HashMap<Arc<str>, Arc<str>>,
	main_window_id: window::Id,
}

impl ConfigView {
	pub fn new(main_window_id: window::Id, loaded_config: &Config) -> Self {
		let input_device_info = get_input_devices();
		let output_device_info = get_output_devices();
		let input_port_info = get_input_ports();
		let output_port_info = get_output_ports();

		let mut hosts = get_hosts().into_boxed_slice();
		hosts.sort_unstable_by(|l, r| natural_cmp(l.name().as_bytes(), r.name().as_bytes()));

		let mut input_devices =
			input_device_info
				.keys()
				.fold(HashMap::<_, Vec<_>>::new(), |mut acc, id| {
					acc.entry(id.host()).or_default().push(id.clone());
					acc
				});

		for input_devices in input_devices.values_mut() {
			input_devices.sort_unstable_by(|l, r| {
				natural_cmp(
					input_device_info[l].name().as_bytes(),
					input_device_info[r].name().as_bytes(),
				)
			});
		}

		let mut output_devices =
			output_device_info
				.keys()
				.fold(HashMap::<_, Vec<_>>::new(), |mut acc, id| {
					acc.entry(id.host()).or_default().push(id.clone());
					acc
				});

		for output_devices in output_devices.values_mut() {
			output_devices.sort_unstable_by(|l, r| {
				natural_cmp(
					output_device_info[l].name().as_bytes(),
					output_device_info[r].name().as_bytes(),
				)
			});
		}

		let mut input_ports = input_port_info.keys().cloned().collect::<Box<_>>();
		input_ports.sort_unstable_by(|l, r| natural_cmp(l.as_bytes(), r.as_bytes()));

		let mut output_ports = output_port_info.keys().cloned().collect::<Box<_>>();
		output_ports.sort_unstable_by(|l, r| natural_cmp(l.as_bytes(), r.as_bytes()));

		Self {
			config: loaded_config.clone(),
			hosts,
			input_devices,
			input_device_info,
			output_devices,
			output_device_info,
			input_ports,
			input_port_info,
			output_ports,
			output_port_info,
			main_window_id,
		}
	}

	pub fn update(&mut self, message: Message, loaded_config: &Config) -> Action<Config, Message> {
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
			Message::ChangedHost(host) => self.config.audio.devices.set_host(host),
			Message::ChangedAudioInput(input) => self.config.audio.devices.set_input(input),
			Message::ChangedAudioOutput(output) => self.config.audio.devices.set_output(output),
			Message::ChangedSampleRate(sample_rate) => self.config.audio.sample_rate = sample_rate,
			Message::ChangedBufferSize(buffer_size) => self.config.audio.buffer_size = buffer_size,
			Message::ChangedMidiInput(input) => self.config.midi.input = input,
			Message::ChangedMidiOutput(output) => self.config.midi.output = output,
			Message::ToggledAutosave => self.config.autosave.enabled ^= true,
			Message::ChangedAutosaveInterval(interval) => {
				if let Some(interval) = interval {
					self.config.autosave.interval = NonZero::new(interval.clamp(1, 999)).unwrap();
				}
			}
			Message::ToggledOpenLastProject => self.config.open_last_project ^= true,
			Message::ChangedTheme(theme) => self.config.theme = theme,
			Message::ChangedScaleFactor(scale_factor) => {
				self.config.scale_factor = scale_factor;
			}
			Message::WriteConfig => return Action::instruction(self.config.clone()),
			Message::ResetConfigToLast => self.config = loaded_config.clone(),
		}

		Action::none()
	}

	pub fn view(&self, loaded_config: &Config) -> Element<'_, Message> {
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
						text("Host:").width(Fill),
						row![
							pick_list(self.config.audio.devices.get_host(), &*self.hosts, |host| {
								if self.hosts.contains(host) {
									host.name().to_owned()
								} else {
									format!("Unknown ({host})")
								}
							})
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
										.audio
										.devices
										.get_host()
										.map(|_| Message::ChangedHost(None))
								)
						]
					]
					.align_y(Center),
					column![
						row![
							text("Audio Input:").width(Fill),
							row![
								pick_list(
									self.config.audio.devices.get_input(),
									self.input_devices
										.get(
											&self
												.config
												.audio
												.devices
												.get_host()
												.unwrap_or_else(|| *DEFAULT_HOST)
										)
										.map_or([].as_slice(), |input_devices| &**input_devices),
									|id| self.input_device_info.get(id).map_or_else(
										|| format!("Unknown ({})", id.id()),
										|device| device.name().to_owned()
									)
								)
								.on_select(|id| Message::ChangedAudioInput(Some(id)))
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
										self.config
											.audio
											.devices
											.get_input()
											.map(|_| Message::ChangedAudioInput(None))
									)
							]
						]
						.align_y(Center),
						row![
							text("Audio Output:").width(Fill),
							row![
								pick_list(
									self.config.audio.devices.get_output(),
									self.output_devices
										.get(
											&self
												.config
												.audio
												.devices
												.get_host()
												.unwrap_or_else(|| *DEFAULT_HOST)
										)
										.map_or([].as_slice(), |devices| &**devices),
									|id| self.output_device_info.get(id).map_or_else(
										|| format!("Unknown ({})", id.id()),
										|device| device.name().to_owned()
									)
								)
								.on_select(|id| Message::ChangedAudioOutput(Some(id)))
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
										self.config
											.audio
											.devices
											.get_output()
											.map(|_| Message::ChangedAudioOutput(None))
									)
							]
						]
						.align_y(Center),
					],
					column![
						row![
							text("Sample Rate:").width(Fill),
							row![
								pick_list(
									self.config.audio.sample_rate,
									SAMPLE_RATES,
									|sample_rate| format!("{sample_rate} hz")
								)
								.on_select(|sample_rate| Message::ChangedSampleRate(Some(
									sample_rate
								)))
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
										self.config
											.audio
											.sample_rate
											.map(|_| Message::ChangedSampleRate(None))
									)
							]
						]
						.align_y(Center),
						row![
							text("Buffer Size:").width(Fill),
							row![
								pick_list(
									self.config.audio.buffer_size,
									BUFFER_SIZES,
									|buffer_size| format!("{buffer_size} smp")
								)
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
										self.config
											.audio
											.buffer_size
											.map(|_| Message::ChangedBufferSize(None))
									)
							]
						]
						.align_y(Center)
					],
					rule::horizontal(1),
					column![
						row![
							text("MIDI Input:").width(Fill),
							row![
								pick_list(
									self.config.midi.input.as_ref(),
									&*self.input_ports,
									|id| self.input_port_info.get(id).map_or_else(
										|| format!("Unknown ({id})"),
										|device| (**device).to_owned()
									)
								)
								.on_select(|id| Message::ChangedMidiInput(Some(id)))
								.handle(PICK_LIST_HANDLE)
								.placeholder("None")
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
										self.config
											.midi
											.input
											.as_ref()
											.map(|_| Message::ChangedMidiInput(None))
									)
							]
						]
						.align_y(Center),
						row![
							text("MIDI Output:").width(Fill),
							row![
								pick_list(
									self.config.midi.output.as_ref(),
									&*self.output_ports,
									|id| self.output_port_info.get(id).map_or_else(
										|| format!("Unknown ({id})"),
										|device| (**device).to_owned()
									)
								)
								.on_select(|id| Message::ChangedMidiOutput(Some(id)))
								.handle(PICK_LIST_HANDLE)
								.placeholder("None")
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
										self.config
											.midi
											.output
											.as_ref()
											.map(|_| Message::ChangedMidiOutput(None))
									)
							]
						]
						.align_y(Center),
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
								300,
								5
							)
							.map(|interval| Message::ChangedAutosaveInterval(
								interval.map(|interval| interval as u16)
							)),
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
						iced(TEXT_HEIGHT),
						space::horizontal(),
						button(save())
							.style(button_with_radius(button::primary, border::left(5)))
							.padding(5)
							.on_press_maybe(
								(self.config != *loaded_config).then_some(Message::WriteConfig)
							),
						button(rotate_ccw())
							.style(button_with_radius(button::primary, border::right(5)))
							.padding(5)
							.on_press_maybe(
								(self.config != *loaded_config)
									.then_some(Message::ResetConfigToLast)
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
					Some(daw::Message::ToggleConfigView)
				}
				_ => None,
			},
			_ => None,
		}
	}
}
