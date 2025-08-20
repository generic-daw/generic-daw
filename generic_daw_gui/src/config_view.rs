use crate::{
	components::{number_input, pick_list_custom_handle, space, styled_scrollable_with_direction},
	config::{Config, Device},
	icons::{mic, plus, rotate_ccw, save, volume_2, x},
	stylefns::{bordered_box_with_radius, button_with_radius, pick_list_with_radius},
	theme::Theme,
	widget::LINE_HEIGHT,
};
use iced::{
	Center, Element, Font, Shrink, Task, border,
	widget::{
		button, column, container, horizontal_rule, horizontal_space, pick_list, row,
		scrollable::{Direction, Scrollbar},
		slider, text, toggler, value,
	},
};
use rfd::AsyncFileDialog;
use std::{num::NonZero, path::Path, sync::Arc};

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
	ChangedAutosaveInterval(NonZero<u64>),
	ChangedAutosaveIntervalText(String),
	ToggledOpenLastProject,
	ChangedTheme(Theme),
	ChangedScaleFactor(f64),
	WriteConfig,
	ResetConfig,
}

pub struct ConfigView {
	prev_config: Config,
	config: Config,
	tab: Tab,
}

impl ConfigView {
	pub fn new(prev_config: Config) -> Self {
		Self {
			config: prev_config.clone(),
			prev_config,
			tab: Tab::Output,
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
			Message::AddSamplePath(path) => self.config.sample_paths.push(path),
			Message::RemoveSamplePath(idx) => _ = self.config.sample_paths.remove(idx),
			Message::AddClapPathFileDialog => {
				return Task::future(AsyncFileDialog::new().pick_folder())
					.and_then(Task::done)
					.map(|p| p.path().into())
					.map(Message::AddClapPath);
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
			Message::ChangedScaleFactor(scale_factor) => self.config.scale_factor = scale_factor,
			Message::WriteConfig => {
				self.config.write();
				self.prev_config = self.config.clone();
			}
			Message::ResetConfig => self.config = self.prev_config.clone(),
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
						.style(button_with_radius(button::primary, 5))
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
									value(path.display()).font(Font::MONOSPACE),
									horizontal_space(),
									button(x())
										.style(button_with_radius(button::danger, 5))
										.padding(0)
										.on_press(Message::RemoveSamplePath(idx))
								]
								.into()
							})
					)
					.padding(5)
					.spacing(5)
				)
				.style(bordered_box_with_radius(5)),
				horizontal_rule(1),
				row![
					"CLAP Plugin Paths",
					horizontal_space(),
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
									horizontal_space(),
									button(x())
										.style(button_with_radius(button::danger, 5))
										.padding(0)
										.on_press(Message::RemoveClapPath(idx))
								]
								.into()
							})
					)
					.padding(5)
					.spacing(5)
				)
				.style(bordered_box_with_radius(5)),
				horizontal_rule(1),
				row![
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
							)
					],
					horizontal_space(),
					match self.tab {
						Tab::Input => "Input",
						Tab::Output => "Output",
					}
				]
				.align_y(Center),
				self.with_device(input_devices, output_devices, |device, devices| {
					column![
						row![
							"Name: ",
							horizontal_space(),
							pick_list_custom_handle(devices, device.name.as_ref(), |name| {
								Message::ChangedName(Some(name))
							})
							.placeholder("Default")
							.width(222)
							.style(pick_list_with_radius(
								pick_list::default,
								border::top_left(5)
							)),
							button(rotate_ccw())
								.style(button_with_radius(button::primary, border::top_right(5)))
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
							.style(pick_list_with_radius(pick_list::default, 0)),
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
							.style(pick_list_with_radius(
								pick_list::default,
								border::bottom_left(5)
							)),
							button(rotate_ccw())
								.style(button_with_radius(button::primary, border::bottom_right(5)))
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
						self.config.autosave.interval.get() as usize,
						600,
						3,
						|x| Message::ChangedAutosaveInterval(
							NonZero::new(x as u64).unwrap_or(NonZero::<u64>::MIN)
						),
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
						Theme::VARIANTS,
						Some(self.config.theme),
						Message::ChangedTheme
					)
					.width(222)
					.style(pick_list_with_radius(pick_list::default, border::left(5))),
					button(rotate_ccw())
						.style(button_with_radius(button::primary, border::right(5)))
						.padding(5)
						.on_press_maybe(
							(self.config.theme != Theme::CatppuccinFrappe)
								.then_some(Message::ChangedTheme(Theme::CatppuccinFrappe))
						)
				]
				.align_y(Center),
				row![
					text("Scale factor: "),
					text(format!("{:.1}", self.config.scale_factor)).font(Font::MONOSPACE),
					horizontal_space(),
					slider(
						0.5..=2.0,
						self.config.scale_factor,
						Message::ChangedScaleFactor
					)
					.step(0.1)
					.width(212),
					space().width(5),
					button(rotate_ccw())
						.style(button_with_radius(button::primary, 5))
						.padding(5)
						.on_press_maybe(
							(self.config.scale_factor != 1.0)
								.then_some(Message::ChangedScaleFactor(1.0))
						)
				]
				.align_y(Center),
				(self.config != self.prev_config).then_some(horizontal_rule(1)),
				(self.config != self.prev_config).then_some(
					row![
						container("Changes will only take effect after a project reload!")
							.padding([5, 10])
							.style(|t| container::warning(t).border(border::rounded(f32::INFINITY))),
						horizontal_space(),
						button(save())
							.style(button_with_radius(button::primary, border::left(5)))
							.padding(5)
							.on_press(Message::WriteConfig),
						button(rotate_ccw())
							.style(button_with_radius(button::primary, border::right(5)))
							.padding(5)
							.on_press(Message::ResetConfig)
					]
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

	fn with_device<'a, T>(
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

	fn with_device_mut<T>(&mut self, f: impl FnOnce(&mut Device) -> T) -> T {
		match self.tab {
			Tab::Input => f(&mut self.config.input_device),
			Tab::Output => f(&mut self.config.output_device),
		}
	}
}
