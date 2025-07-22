use crate::{
	components::{number_input, pick_list_custom_handle, space, styled_scrollable_with_direction},
	config::{Config, Device},
	icons::{mic, plus, rotate_ccw, save, volume_2, x},
	stylefns::{button_with_radius, pick_list_with_radius},
	theme,
	widget::LINE_HEIGHT,
};
use iced::{
	Center, Element, Font, Shrink, Task, Theme, border,
	widget::{
		button, column, container, horizontal_rule, horizontal_space, pick_list, row,
		scrollable::{Direction, Scrollbar},
		text, toggler,
	},
};
use rfd::AsyncFileDialog;
use std::{path::Path, sync::Arc};

static COMMON_SAMPLE_RATES: &[u32] = &[44_100, 48_000, 88_200, 96_000, 176_400, 192_000];
static COMMON_BUFFER_SIZES: &[u32] = &[16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192];

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
	ChangedName(Option<String>),
	ChangedSampleRate(Option<u32>),
	ChangedBufferSize(Option<u32>),
	ToggledAutosave,
	ChangedAutosaveInterval(u64),
	ChangedAutosaveIntervalText(String),
	ToggledOpenLastProject,
	ChangedTheme(Theme),
	WriteConfig,
	ResetConfig,
}

pub struct ConfigView {
	config: Config,
	tab: Tab,
	dirty: bool,
}

impl ConfigView {
	pub fn new(config: Config) -> Self {
		Self {
			config,
			tab: Tab::Output,
			dirty: false,
		}
	}

	pub fn update(&mut self, message: Message) -> Task<Message> {
		match message {
			Message::AddSamplePathFileDialog => {
				return Task::future(AsyncFileDialog::new().pick_folder())
					.and_then(Task::done)
					.map(|p| p.path().into())
					.map(Message::AddSamplePath);
			}
			Message::AddSamplePath(path) => {
				self.config.sample_paths.push(path);
				self.dirty = true;
			}
			Message::RemoveSamplePath(idx) => {
				self.config.sample_paths.remove(idx);
				self.dirty = true;
			}
			Message::AddClapPathFileDialog => {
				return Task::future(AsyncFileDialog::new().pick_folder())
					.and_then(Task::done)
					.map(|p| p.path().into())
					.map(Message::AddClapPath);
			}
			Message::AddClapPath(path) => {
				self.config.clap_paths.push(path);
				self.dirty = true;
			}
			Message::RemoveClapPath(idx) => {
				self.config.clap_paths.remove(idx);
				self.dirty = true;
			}
			Message::ChangedTab(tab) => self.tab = tab,
			Message::ChangedName(name) => {
				self.delegate_device_update(|device| {
					device.name = name;
				});
				self.dirty = true;
			}
			Message::ChangedSampleRate(sample_rate) => {
				self.delegate_device_update(|device| {
					device.sample_rate = sample_rate;
				});
				self.dirty = true;
			}
			Message::ChangedBufferSize(buffer_size) => {
				self.delegate_device_update(|device| {
					device.buffer_size = buffer_size;
				});
				self.dirty = true;
			}
			Message::ToggledAutosave => {
				self.config.autosave.enabled ^= true;
				self.dirty = true;
			}
			Message::ChangedAutosaveInterval(interval) => {
				self.config.autosave.interval = interval;
				self.dirty = true;
			}
			Message::ChangedAutosaveIntervalText(text) => {
				if let Ok(interval) = text.parse() {
					return self.update(Message::ChangedAutosaveInterval(interval));
				}
			}
			Message::ToggledOpenLastProject => {
				self.config.open_last_project ^= true;
				self.dirty = true;
			}
			Message::ChangedTheme(theme) => {
				self.config.theme = theme.try_into().unwrap();
				self.dirty = true;
			}
			Message::WriteConfig => {
				self.config.write();
				self.dirty = false;
			}
			Message::ResetConfig => {
				self.config = Config::read();
				self.dirty = false;
			}
		}

		Task::none()
	}

	pub fn view<'a>(
		&'a self,
		input_devices: &'a [String],
		output_devices: &'a [String],
	) -> Element<'a, Message> {
		container(styled_scrollable_with_direction(
			column![
				text("Settings")
					.size(LINE_HEIGHT)
					.line_height(1.0)
					.font(Font::MONOSPACE),
				container(horizontal_rule(1)).padding([5, 0]),
				row![
					"Sample Paths",
					horizontal_space(),
					button(plus())
						.style(button_with_radius(button::primary, 0))
						.padding(0)
						.on_press(Message::AddSamplePathFileDialog),
					space().width(5)
				],
				container(
					column(
						self.config
							.sample_paths
							.iter()
							.enumerate()
							.map(|(idx, path)| {
								row![
									text(path.to_string_lossy()).font(Font::MONOSPACE),
									horizontal_space(),
									button(x())
										.style(button_with_radius(button::danger, 0))
										.padding(0)
										.on_press(Message::RemoveSamplePath(idx))
								]
								.into()
							})
					)
					.padding(5)
					.spacing(5)
				)
				.style(|t| {
					container::background(t.extended_palette().background.weak.color).border(
						border::width(1).color(t.extended_palette().background.strong.color),
					)
				}),
				horizontal_rule(1),
				row![
					"CLAP Plugin Paths",
					horizontal_space(),
					button(plus())
						.style(button_with_radius(button::primary, 0))
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
									text(path.to_string_lossy()).font(Font::MONOSPACE),
									horizontal_space(),
									button(x())
										.style(button_with_radius(button::danger, 0))
										.padding(0)
										.on_press(Message::RemoveClapPath(idx))
								]
								.into()
							})
					)
					.padding(5)
					.spacing(5)
				)
				.style(|t| {
					container::background(t.extended_palette().background.weak.color).border(
						border::width(1).color(t.extended_palette().background.strong.color),
					)
				}),
				horizontal_rule(1),
				row![
					row![
						button(mic())
							.style(button_with_radius(button::primary, 0))
							.on_press_maybe(
								(self.tab != Tab::Input).then_some(Message::ChangedTab(Tab::Input))
							),
						button(volume_2())
							.style(button_with_radius(button::primary, 0))
							.on_press_maybe(
								(self.tab != Tab::Output)
									.then_some(Message::ChangedTab(Tab::Output))
							)
					],
					horizontal_space(),
					match self.tab {
						Tab::Input => "Input",
						Tab::Output => "Output",
					}
				]
				.align_y(Center),
				self.delegate_device_view(input_devices, output_devices, |device, devices| {
					column![
						row![
							"Name: ",
							horizontal_space(),
							pick_list_custom_handle(devices, device.name.as_ref(), |name| {
								Message::ChangedName(Some(name))
							})
							.placeholder("Default")
							.width(222)
							.style(pick_list_with_radius(pick_list::default, border::left(5))),
							button(rotate_ccw())
								.style(button_with_radius(button::primary, 0))
								.padding(5)
								.on_press_maybe(
									device.name.as_deref().map(|_| Message::ChangedName(None))
								)
						]
						.align_y(Center),
						row![
							"Sample Rate: ",
							horizontal_space(),
							pick_list_custom_handle(
								COMMON_SAMPLE_RATES,
								device.sample_rate,
								|sample_rate| { Message::ChangedSampleRate(Some(sample_rate)) }
							)
							.placeholder("Default")
							.width(222)
							.style(pick_list_with_radius(pick_list::default, border::left(5))),
							button(rotate_ccw())
								.style(button_with_radius(button::primary, 0))
								.padding(5)
								.on_press_maybe(
									device.sample_rate.map(|_| Message::ChangedSampleRate(None))
								)
						]
						.align_y(Center),
						row![
							"Buffer Size: ",
							horizontal_space(),
							pick_list_custom_handle(
								COMMON_BUFFER_SIZES,
								device.buffer_size,
								|buffer_size| { Message::ChangedBufferSize(Some(buffer_size)) }
							)
							.placeholder("Default")
							.width(222)
							.style(pick_list_with_radius(pick_list::default, border::left(5))),
							button(rotate_ccw())
								.style(button_with_radius(button::primary, 0))
								.padding(5)
								.on_press_maybe(
									device.buffer_size.map(|_| Message::ChangedBufferSize(None))
								)
						]
						.align_y(Center)
					]
				}),
				horizontal_rule(1),
				row![
					toggler(self.config.autosave.enabled)
						.label("Autosave every ")
						.on_toggle(|_| Message::ToggledAutosave),
					number_input(
						self.config.autosave.interval as usize,
						600,
						3,
						|x| Message::ChangedAutosaveInterval(x as u64),
						Message::ChangedAutosaveIntervalText
					),
					" s"
				]
				.align_y(Center),
				toggler(self.config.open_last_project)
					.label("Open last project on startup")
					.on_toggle(|_| Message::ToggledOpenLastProject),
				horizontal_rule(1),
				row![
					"Theme: ",
					horizontal_space(),
					pick_list_custom_handle(
						Theme::ALL,
						Some::<Theme>(self.config.theme.into()),
						Message::ChangedTheme
					)
					.width(222)
					.style(pick_list_with_radius(pick_list::default, border::left(5))),
					button(rotate_ccw())
						.style(button_with_radius(button::primary, 0))
						.padding(5)
						.on_press_maybe(
							(self.config.theme != theme::Theme::CatppuccinFrappe)
								.then_some(Message::ChangedTheme(Theme::CatppuccinFrappe))
						)
				]
				.align_y(Center),
				self.dirty.then_some(horizontal_rule(1)),
				self.dirty.then_some(
					row![
						container("Changes will only take effect after a project reload!")
							.padding([5, 10])
							.style(|t| container::warning(t).border(border::rounded(f32::INFINITY))),
						horizontal_space(),
						button(save())
							.style(button_with_radius(button::primary, 0))
							.padding(5)
							.on_press(Message::WriteConfig),
						button(rotate_ccw())
							.style(button_with_radius(button::primary, 0))
							.padding(5)
							.on_press(Message::ResetConfig)
					]
					.spacing(5)
					.height(Shrink),
				)
			]
			.spacing(10)
			.padding(10)
			.width(530),
			Direction::Vertical(Scrollbar::default()),
		))
		.style(|t| {
			container::background(t.extended_palette().background.weakest.color)
				.border(border::width(1).color(t.extended_palette().background.strong.color))
		})
		.into()
	}

	fn delegate_device_update<T>(&mut self, f: impl FnOnce(&mut Device) -> T) -> T {
		match self.tab {
			Tab::Input => f(&mut self.config.input_device),
			Tab::Output => f(&mut self.config.output_device),
		}
	}

	fn delegate_device_view<'a, T>(
		&'a self,
		input_devices: &'a [String],
		output_devices: &'a [String],
		f: impl FnOnce(&'a Device, &'a [String]) -> T,
	) -> T {
		match self.tab {
			Tab::Input => f(&self.config.input_device, input_devices),
			Tab::Output => f(&self.config.output_device, output_devices),
		}
	}
}
