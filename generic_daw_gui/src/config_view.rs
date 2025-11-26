use crate::{
	action::Action,
	components::{PICK_LIST_HANDLE, number_input},
	config::{Config, Device},
	icons::{link, mic, plus, rotate_ccw, save, unlink, volume_2, x},
	stylefns::{
		bordered_box_with_radius, button_with_radius, menu_style, pick_list_with_radius,
		scrollable_style,
	},
	theme::Theme,
	widget::{LINE_HEIGHT, TEXT_HEIGHT},
};
use generic_daw_core::{get_input_devices, get_output_devices};
use iced::{
	Center, Element, Font,
	Length::Fill,
	Task, border, padding,
	widget::{
		button, checkbox, column, container, iced, pick_list, row, rule, scrollable, slider, space,
		text, value,
	},
};
use rfd::AsyncFileDialog;
use std::{num::NonZero, path::Path, sync::Arc};

static COMMON_SAMPLE_RATES: &[u32] = &[44_100, 48_000, 64_000, 88_200, 96_000, 176_400, 192_000];
static COMMON_BUFFER_SIZES: &[u32] = &[64, 128, 256, 512, 1024, 2048, 4096, 8192];

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
	AddClapPathFileDialog,
	AddClapPath(Arc<Path>),
	RemoveClapPath(usize),
	ChangedTab(Tab),
	ChangedName(Option<Arc<str>>),
	ChangedSampleRate(NonZero<u32>),
	ChangedBufferSize(Option<NonZero<u32>>),
	ToggledAutosave,
	ChangedAutosaveInterval(NonZero<u64>),
	ChangedAutosaveIntervalText(String),
	ToggledOpenLastProject,
	ChangedTheme(Theme),
	ChangedAppScaleFactor(f32),
	ChangedPluginScaleFactor(Option<f32>),
	WriteConfig,
	ResetConfigToPrev,
	ResetConfigToDefault,
}

#[derive(Debug)]
pub struct ConfigView {
	config: Config,
	prev_config: Config,
	tab: Tab,
	input_devices: Vec<Arc<str>>,
	output_devices: Vec<Arc<str>>,
}

impl Default for ConfigView {
	fn default() -> Self {
		let mut input_devices = get_input_devices();
		input_devices.sort_unstable();

		let mut output_devices = get_output_devices();
		output_devices.sort_unstable();

		let config = Config::read();

		Self {
			config: config.clone(),
			prev_config: config,
			tab: Tab::Output,
			input_devices,
			output_devices,
		}
	}
}

impl ConfigView {
	pub fn update(&mut self, message: Message) -> Action<Config, Message> {
		match message {
			Message::AddSamplePathFileDialog => {
				return Task::future(AsyncFileDialog::new().pick_folder())
					.and_then(Task::done)
					.map(|p| p.path().into())
					.map(Message::AddSamplePath)
					.into();
			}
			Message::AddSamplePath(path) => self.config.sample_paths.push(path),
			Message::RemoveSamplePath(idx) => _ = self.config.sample_paths.remove(idx),
			Message::AddClapPathFileDialog => {
				return Task::future(AsyncFileDialog::new().pick_folder())
					.and_then(Task::done)
					.map(|p| p.path().into())
					.map(Message::AddClapPath)
					.into();
			}
			Message::AddClapPath(path) => self.config.clap_paths.push(path),
			Message::RemoveClapPath(idx) => _ = self.config.clap_paths.remove(idx),
			Message::ChangedTab(tab) => self.tab = tab,
			Message::ChangedName(name) => self.with_device_mut(|device| {
				device.name = name;
			}),
			Message::ChangedSampleRate(sample_rate) => self.with_device_mut(|device| {
				device.sample_rate = sample_rate;
			}),
			Message::ChangedBufferSize(buffer_size) => self.with_device_mut(|device| {
				device.buffer_size = buffer_size;
			}),
			Message::ToggledAutosave => self.config.autosave.enabled ^= true,
			Message::ChangedAutosaveInterval(interval) => self.config.autosave.interval = interval,
			Message::ChangedAutosaveIntervalText(text) => {
				if let Ok(interval) = text.parse() {
					return self.update(Message::ChangedAutosaveInterval(interval));
				}
			}
			Message::ToggledOpenLastProject => self.config.open_last_project ^= true,
			Message::ChangedTheme(theme) => self.config.theme = theme,
			Message::ChangedAppScaleFactor(app_scale_factor) => {
				self.config.app_scale_factor = app_scale_factor;
			}
			Message::ChangedPluginScaleFactor(plugin_scale_factor) => {
				self.config.plugin_scale_factor = plugin_scale_factor;
			}
			Message::WriteConfig => {
				return Action::instruction(self.config.clone());
			}
			Message::ResetConfigToPrev => self.config = self.prev_config.clone(),
			Message::ResetConfigToDefault => self.config = Config::default(),
		}

		Action::none()
	}

	pub fn view(&self, live_config: &Config) -> Element<'_, Message> {
		container(
			scrollable(
				column![
					row![
						text("Settings")
							.size(LINE_HEIGHT)
							.line_height(1.0)
							.font(Font::MONOSPACE),
						space::horizontal(),
						button(rotate_ccw())
							.style(button_with_radius(button::primary, 5))
							.padding(5)
							.on_press_maybe(
								(self.config != Config::default())
									.then_some(Message::ResetConfigToDefault)
							)
					],
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
						column(
							self.config
								.sample_paths
								.iter()
								.enumerate()
								.map(|(idx, path)| {
									row![
										value(path.display()).font(Font::MONOSPACE),
										space::horizontal(),
										button(x())
											.style(button_with_radius(button::danger, 5))
											.padding(0)
											.on_press(Message::RemoveSamplePath(idx))
									]
									.align_y(Center)
									.into()
								})
						)
						.padding(5)
						.spacing(5)
					)
					.style(bordered_box_with_radius(5)),
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
						column(
							self.config
								.clap_paths
								.iter()
								.enumerate()
								.map(|(idx, path)| {
									row![
										value(path.display()).font(Font::MONOSPACE),
										space::horizontal(),
										button(x())
											.style(button_with_radius(button::danger, 5))
											.padding(0)
											.on_press(Message::RemoveClapPath(idx))
									]
									.align_y(Center)
									.into()
								})
						)
						.padding(5)
						.spacing(5)
					)
					.style(bordered_box_with_radius(5)),
					rule::horizontal(1),
					row![
						row![
							button(mic())
								.style(button_with_radius(button::primary, border::left(5)))
								.padding(5)
								.on_press_maybe(
									(self.tab != Tab::Input)
										.then_some(Message::ChangedTab(Tab::Input))
								),
							button(volume_2())
								.style(button_with_radius(button::primary, border::right(5)))
								.padding(5)
								.on_press_maybe(
									(self.tab != Tab::Output)
										.then_some(Message::ChangedTab(Tab::Output))
								)
						],
						space::horizontal(),
						self.with_device(|device, _| device.buffer_size.map(|buffer_size| text(
							format!(
								"{buffer_size} smp @ {} hz = {:.1} ms",
								device.sample_rate,
								buffer_size.get() as f32 / device.sample_rate.get() as f32 * 1000.0
							)
						)
						.font(Font::MONOSPACE)
						.size(12))),
						space::horizontal(),
						match self.tab {
							Tab::Input => "Input",
							Tab::Output => "Output",
						}
					]
					.align_y(Center),
					self.with_device(|device, devices| {
						column![
							row![
								"Name: ",
								space::horizontal(),
								pick_list(devices, device.name.as_ref(), |name| {
									Message::ChangedName(Some(name))
								})
								.handle(PICK_LIST_HANDLE)
								.placeholder("Default")
								.width(222)
								.style(pick_list_with_radius(border::top_left(5)))
								.menu_style(menu_style),
								button(rotate_ccw())
									.style(button_with_radius(
										button::primary,
										border::top_right(5)
									))
									.padding(5)
									.on_press_maybe(
										device.name.as_deref().map(|_| Message::ChangedName(None))
									)
							]
							.align_y(Center),
							row![
								"Sample Rate: ",
								space::horizontal(),
								pick_list(
									COMMON_SAMPLE_RATES,
									Some(device.sample_rate.get()),
									|sample_rate| {
										Message::ChangedSampleRate(
											NonZero::new(sample_rate).unwrap(),
										)
									}
								)
								.handle(PICK_LIST_HANDLE)
								.placeholder("Default")
								.width(222)
								.style(pick_list_with_radius(0))
								.menu_style(menu_style),
								button(rotate_ccw())
									.style(button_with_radius(button::primary, 0))
									.padding(5)
									.on_press_maybe((device.sample_rate.get() != 44100).then_some(
										Message::ChangedSampleRate(NonZero::new(44100).unwrap(),)
									))
							]
							.align_y(Center),
							row![
								"Buffer Size: ",
								space::horizontal(),
								pick_list(
									COMMON_BUFFER_SIZES,
									device.buffer_size.map(NonZero::get),
									|buffer_size| {
										Message::ChangedBufferSize(NonZero::new(buffer_size))
									}
								)
								.handle(PICK_LIST_HANDLE)
								.placeholder("Default")
								.width(222)
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
							.align_y(Center)
						]
					}),
					rule::horizontal(1),
					row![
						row![
							checkbox(self.config.autosave.enabled)
								.label("Autosave every ")
								.on_toggle(|_| Message::ToggledAutosave),
							number_input(
								self.config.autosave.interval.get() as usize,
								600,
								3,
								|x| Message::ChangedAutosaveInterval(
									NonZero::new(x as u64).or(NonZero::new(1)).unwrap()
								),
								Message::ChangedAutosaveIntervalText
							),
							" s"
						]
						.align_y(Center)
						.width(Fill),
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
						column![
							row![
								"App scale factor:  ",
								text(format!("{:.1}", self.config.app_scale_factor))
									.font(Font::MONOSPACE),
								space::horizontal(),
								button(rotate_ccw().size(LINE_HEIGHT - 3.0))
									.style(button_with_radius(button::primary, 5))
									.padding(3)
									.on_press_maybe(
										(self.config.app_scale_factor != 1.0)
											.then_some(Message::ChangedAppScaleFactor(1.0))
									),
								space().width(5)
							]
							.align_y(Center),
							slider(
								0.5..=2.0,
								self.config.app_scale_factor,
								Message::ChangedAppScaleFactor
							)
							.step(0.1),
						]
						.spacing(5),
						container(
							button(
								self.config
									.plugin_scale_factor
									.map_or_else(link, |_| unlink())
									.size(LINE_HEIGHT - 3.0)
							)
							.padding(0)
							.style(button::text)
							.on_press(self.config.plugin_scale_factor.map_or(
								Message::ChangedPluginScaleFactor(Some(
									self.config.app_scale_factor
								)),
								|_| Message::ChangedPluginScaleFactor(None)
							))
						)
						.align_bottom(Fill)
						.width(LINE_HEIGHT - 2.0),
						column![
							row![
								"Plugin scale factor:  ",
								text(format!(
									"{:.1}",
									self.config
										.plugin_scale_factor
										.unwrap_or(self.config.app_scale_factor)
								))
								.font(Font::MONOSPACE),
								space::horizontal(),
								button(rotate_ccw().size(LINE_HEIGHT - 3.0))
									.style(button_with_radius(button::primary, 5))
									.padding(3)
									.on_press_maybe(
										self.config
											.plugin_scale_factor
											.map(|_| Message::ChangedPluginScaleFactor(None))
									),
								space().width(5)
							]
							.align_y(Center),
							slider(
								0.5..=2.0,
								self.config
									.plugin_scale_factor
									.unwrap_or(self.config.app_scale_factor),
								|scale| self
									.config
									.plugin_scale_factor
									.map_or(Message::ChangedAppScaleFactor(scale), |_| {
										Message::ChangedPluginScaleFactor(Some(scale))
									})
							)
							.step(0.1)
						]
						.spacing(5)
					]
					.align_y(Center)
					.spacing(10),
					row![
						"Theme: ",
						space::horizontal(),
						pick_list(
							Theme::VARIANTS,
							Some(self.config.theme),
							Message::ChangedTheme
						)
						.handle(PICK_LIST_HANDLE)
						.width(222)
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
					.align_y(Center),
					rule::horizontal(1),
					row![
						if self.config.is_mergeable(live_config) {
							iced(TEXT_HEIGHT)
						} else {
							container("Some changes may only take effect after a reload!")
								.padding(padding::horizontal(10).vertical(5))
								.style(|t| {
									container::warning(t).border(border::rounded(f32::INFINITY))
								})
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
				.width(530),
			)
			.spacing(5)
			.style(scrollable_style),
		)
		.style(|t| {
			bordered_box_with_radius(5)(t).background(t.extended_palette().background.weakest.color)
		})
		.into()
	}

	fn with_device<'a, T>(&'a self, f: impl FnOnce(&'a Device, &'a [Arc<str>]) -> T) -> T {
		match self.tab {
			Tab::Input => f(&self.config.input_device, &self.input_devices),
			Tab::Output => f(&self.config.output_device, &self.output_devices),
		}
	}

	fn with_device_mut<T>(&mut self, f: impl FnOnce(&mut Device) -> T) -> T {
		match self.tab {
			Tab::Input => f(&mut self.config.input_device),
			Tab::Output => f(&mut self.config.output_device),
		}
	}
}
