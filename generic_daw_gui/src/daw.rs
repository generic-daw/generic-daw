use crate::{
	arrangement_view::{ArrangementView, Message as ArrangementMessage, Tab},
	components::{number_input, pick_list_custom_handle, space},
	config::Config,
	config_view::{ConfigView, Message as ConfigViewMessage},
	file_tree::{FileTree, Message as FileTreeMessage},
	icons::{chart_no_axes_gantt, pause, play, sliders_vertical, square},
	state::State,
	stylefns::{button_with_radius, pick_list_with_radius},
	widget::LINE_HEIGHT,
};
use generic_daw_core::{
	AudioGraph, MusicalTime,
	clap_host::{PluginBundle, PluginDescriptor, get_installed_plugins},
};
use generic_daw_utils::NoClone;
use generic_daw_widget::dot::Dot;
use iced::{
	Alignment, Color, Element, Event, Fill, Subscription, Task, Theme, border,
	event::{self, Status},
	keyboard,
	mouse::Interaction,
	time::every,
	widget::{
		button, center, column, container, horizontal_space, mouse_area, opaque, pick_list,
		progress_bar, row, stack,
	},
	window::{self, Id},
};
use iced_split::{Strategy, vertical_split};
use log::trace;
use rfd::AsyncFileDialog;
use smol::unblock;
use std::{collections::BTreeMap, path::Path, sync::Arc, time::Duration};

#[derive(Clone, Debug)]
pub enum Message {
	Arrangement(ArrangementMessage),
	FileTree(FileTreeMessage),
	ConfigView(ConfigViewMessage),

	NewFile,
	OpenFileDialog,
	OpenLastFile,
	SaveFile,
	AutosaveFile,
	SaveAsFileDialog,
	ExportFileDialog,

	OpenFile(Arc<Path>),
	SaveAsFile(Arc<Path>),
	ExportFile(Arc<Path>),
	ExportProgress(f32),
	FinishExport(NoClone<Box<AudioGraph>>),

	OpenConfigView,
	CloseConfigView,

	Stop,
	TogglePlayback,
	ToggleMetronome,
	ChangedBpm(u16),
	ChangedBpmText(String),
	ChangedNumerator(u8),
	ChangedNumeratorText(String),
	ChangedTab(Tab),

	SplitAt(f32),
}

const _: () = assert!(size_of::<Message>() <= 128);

pub struct Daw {
	config: Config,
	state: State,
	plugin_bundles: BTreeMap<PluginDescriptor, PluginBundle>,

	arrangement_view: ArrangementView,
	file_tree: FileTree,
	config_view: Option<ConfigView>,
	split_at: f32,

	export_progress: Option<f32>,
}

impl Daw {
	pub fn create() -> (Self, Task<Message>) {
		let mut open = window::open(window::Settings {
			exit_on_close_request: false,
			maximized: true,
			..window::Settings::default()
		})
		.1
		.discard();

		let config = Config::read();
		let state = State::read();

		if config.open_last_project {
			open = open.chain(Task::done(Message::OpenLastFile));
		}

		let plugin_bundles = get_installed_plugins(&config.clap_paths);
		let file_tree = FileTree::new(&config.sample_paths);

		let (arrangement_view, futs) = ArrangementView::new(&config, &plugin_bundles);
		open = open.chain(futs.map(Message::Arrangement));

		(
			Self {
				config,
				state,
				plugin_bundles,

				arrangement_view,
				file_tree,
				config_view: None,
				split_at: 300.0,

				export_progress: None,
			},
			open,
		)
	}

	pub fn update(&mut self, message: Message) -> Task<Message> {
		trace!("{message:?}");

		match message {
			Message::Arrangement(message) => {
				return self
					.arrangement_view
					.update(message, &self.config, &self.plugin_bundles)
					.map(Message::Arrangement);
			}
			Message::FileTree(action) => return self.handle_file_tree_action(action),
			Message::ConfigView(message) => {
				return self
					.config_view
					.as_mut()
					.unwrap()
					.update(message)
					.map(Message::ConfigView);
			}
			Message::NewFile => {
				self.reload_config();
				return self
					.arrangement_view
					.unload(&self.config, &self.plugin_bundles)
					.map(Message::Arrangement);
			}
			Message::ChangedTab(tab) => self.arrangement_view.tab = tab,
			Message::OpenFileDialog => {
				return Task::future(
					AsyncFileDialog::new()
						.add_filter("Generic DAW project file", &["gdp"])
						.pick_file(),
				)
				.and_then(Task::done)
				.map(|p| p.path().into())
				.map(Message::OpenFile);
			}
			Message::OpenLastFile => {
				if let Some(last_project) = self.state.last_project.clone() {
					return self.update(Message::OpenFile(last_project));
				}
			}
			Message::SaveFile => {
				return self.update(
					self.state
						.current_project
						.clone()
						.map_or(Message::SaveAsFileDialog, Message::SaveAsFile),
				);
			}
			Message::AutosaveFile => {
				if let Some(current_project) = self.state.current_project.clone() {
					return self.update(Message::SaveAsFile(current_project));
				}
			}
			Message::SaveAsFileDialog => {
				return Task::future(
					AsyncFileDialog::new()
						.add_filter("Generic DAW project file", &["gdp"])
						.save_file(),
				)
				.and_then(Task::done)
				.map(|p| p.path().with_extension("gdp").into())
				.map(Message::SaveAsFile);
			}
			Message::ExportFileDialog => {
				return Task::future(
					AsyncFileDialog::new()
						.add_filter("Wave File", &["wav"])
						.save_file(),
				)
				.and_then(Task::done)
				.map(|p| p.path().with_extension("wav").into())
				.map(Message::ExportFile);
			}
			Message::OpenFile(path) => {
				self.reload_config();
				if let Some(futs) =
					self.arrangement_view
						.load(&path, &self.config, &self.plugin_bundles)
				{
					self.state.current_project = Some(path.clone());
					if self.state.last_project.as_deref() != Some(&path) {
						self.state.last_project = Some(path);
						self.state.write();
					}

					return futs.map(Message::Arrangement);
				}
			}
			Message::SaveAsFile(path) => {
				self.arrangement_view.save(&path);
				self.state.current_project = Some(path.clone());
				if self.state.last_project.as_deref() != Some(&path) {
					self.state.last_project = Some(path);
					self.state.write();
				}
			}
			Message::ExportFile(path) => {
				self.export_progress = Some(0.0);
				self.arrangement_view.clap_host.set_realtime(false);
				let (sender, receiver) = smol::channel::unbounded();
				let (audio_graph, export_fn) =
					self.arrangement_view.arrangement.start_export(path, sender);
				return Task::batch([
					Task::future(unblock(export_fn)).discard(),
					Task::stream(receiver)
						.map(Message::ExportProgress)
						.chain(Task::perform(audio_graph, |audio_graph| {
							Message::FinishExport(NoClone(Box::new(audio_graph.unwrap())))
						})),
				]);
			}
			Message::ExportProgress(progress) => self.export_progress = Some(progress),
			Message::FinishExport(NoClone(audio_graph)) => {
				self.arrangement_view
					.arrangement
					.finish_export(*audio_graph);
				self.arrangement_view.clap_host.set_realtime(true);
				self.export_progress = None;
			}
			Message::OpenConfigView => {
				self.config_view = Some(ConfigView::new(self.config.clone()));
			}
			Message::CloseConfigView => self.config_view = None,
			Message::Stop => {
				self.arrangement_view.arrangement.stop();
				return self
					.arrangement_view
					.update(
						ArrangementMessage::StopRecord,
						&self.config,
						&self.plugin_bundles,
					)
					.map(Message::Arrangement);
			}
			Message::TogglePlayback => {
				self.arrangement_view.arrangement.toggle_playback();
				return self
					.arrangement_view
					.update(
						ArrangementMessage::StopRecord,
						&self.config,
						&self.plugin_bundles,
					)
					.map(Message::Arrangement);
			}
			Message::ToggleMetronome => self.arrangement_view.arrangement.toggle_metronome(),
			Message::ChangedBpm(bpm) => self
				.arrangement_view
				.arrangement
				.set_bpm(bpm.clamp(10, 999)),
			Message::ChangedBpmText(bpm) => {
				if let Ok(bpm) = bpm.parse() {
					return self.update(Message::ChangedBpm(bpm));
				}
			}
			Message::ChangedNumerator(numerator) => {
				self.arrangement_view
					.arrangement
					.set_numerator(numerator.clamp(1, 99));
			}
			Message::ChangedNumeratorText(numerator) => {
				if let Ok(numerator) = numerator.parse() {
					return self.update(Message::ChangedNumerator(numerator));
				}
			}
			Message::SplitAt(split_at) => {
				self.split_at = if split_at >= 20.0 {
					split_at.clamp(200.0, 1000.0)
				} else {
					0.0
				};
			}
		}

		Task::none()
	}

	fn handle_file_tree_action(&mut self, action: FileTreeMessage) -> Task<Message> {
		match action {
			FileTreeMessage::File(path) => self
				.arrangement_view
				.update(
					ArrangementMessage::SampleLoadFromFile(path),
					&self.config,
					&self.plugin_bundles,
				)
				.map(Message::Arrangement),
			FileTreeMessage::Action(id, action) => {
				self.file_tree.update(id, &action).map(Message::FileTree)
			}
		}
	}

	fn reload_config(&mut self) {
		let config = Config::read();

		if self.config.clap_paths != config.clap_paths {
			self.plugin_bundles = get_installed_plugins(&config.clap_paths);
		}

		if self.config.sample_paths != config.sample_paths {
			self.file_tree.diff(&config.sample_paths);
		}

		self.config = config;
	}

	pub fn view(&self, window: Id) -> Element<'_, Message> {
		if let Some(gui) = self.arrangement_view.clap_host.plugin_gui(window) {
			return gui
				.map(ArrangementMessage::ClapHost)
				.map(Message::Arrangement);
		}

		let fill = MusicalTime::from_samples(
			self.arrangement_view.arrangement.rtstate().sample,
			self.arrangement_view.arrangement.rtstate(),
		)
		.beat()
		.is_multiple_of(2);

		let mut base = stack![
			column![
				row![
					pick_list_custom_handle(
						[
							"New",
							"Open",
							"Open Last",
							"Save",
							"Save As",
							"Export",
							"Settings"
						],
						Some("File"),
						|s| {
							match s {
								"New" => Message::NewFile,
								"Open" => Message::OpenFileDialog,
								"Open Last" => Message::OpenLastFile,
								"Save" => Message::SaveFile,
								"Save As" => Message::SaveAsFileDialog,
								"Export" => Message::ExportFileDialog,
								"Settings" => Message::OpenConfigView,
								_ => unreachable!(),
							}
						}
					)
					.style(pick_list_with_radius(pick_list::default, 5)),
					row![
						button(
							container(if self.arrangement_view.arrangement.rtstate().playing {
								pause()
							} else {
								play()
							})
							.width(LINE_HEIGHT)
							.align_x(Alignment::Center)
						)
						.style(button_with_radius(button::primary, border::left(5)))
						.padding([5, 7])
						.on_press(Message::TogglePlayback),
						button(
							container(square())
								.width(LINE_HEIGHT)
								.align_x(Alignment::Center)
						)
						.style(button_with_radius(button::primary, border::right(5)))
						.padding([5, 7])
						.on_press(Message::Stop),
					],
					number_input(
						self.arrangement_view.arrangement.rtstate().numerator as usize,
						4,
						2,
						|x| Message::ChangedNumerator(x as u8),
						Message::ChangedNumeratorText
					),
					number_input(
						self.arrangement_view.arrangement.rtstate().bpm as usize,
						140,
						3,
						|x| Message::ChangedBpm(x as u16),
						Message::ChangedBpmText
					),
					button(row![Dot::new(fill), Dot::new(!fill)].spacing(5))
						.style(button_with_radius(
							if self.arrangement_view.arrangement.rtstate().metronome {
								button::primary
							} else {
								button::secondary
							},
							5
						))
						.padding(8)
						.on_press(Message::ToggleMetronome),
					horizontal_space(),
					row![
						button(chart_no_axes_gantt())
							.style(button_with_radius(button::primary, border::left(5)))
							.padding([5, 7])
							.on_press_maybe(
								(!matches!(self.arrangement_view.tab, Tab::Arrangement { .. }))
									.then_some(Message::ChangedTab(Tab::Arrangement {
										grabbed_clip: None
									}))
							),
						button(sliders_vertical())
							.style(button_with_radius(button::primary, border::right(5)))
							.padding([5, 7])
							.on_press_maybe(
								(!matches!(self.arrangement_view.tab, Tab::Mixer))
									.then_some(Message::ChangedTab(Tab::Mixer))
							)
					],
				]
				.spacing(10)
				.align_y(Alignment::Center),
				vertical_split(
					self.file_tree.view().map(Message::FileTree),
					self.arrangement_view.view().map(Message::Arrangement),
					self.split_at,
					Message::SplitAt
				)
				.strategy(Strategy::Start)
			]
			.padding(10)
			.spacing(10)
		];

		if self.arrangement_view.loading() {
			base = base.push(
				mouse_area(space().width(Fill).height(Fill)).interaction(Interaction::Progress),
			);
		}

		if let Some(config_view) = &self.config_view {
			base = base.push(opaque(
				mouse_area(
					center(opaque(config_view.view().map(Message::ConfigView)))
						.style(|_| container::background(Color::BLACK.scale_alpha(0.8))),
				)
				.on_press(Message::CloseConfigView),
			));
		}

		if let Some(progress) = self.export_progress {
			base = base.push(opaque(
				center(progress_bar(0.0..=1.0, progress))
					.padding(50)
					.style(|_| container::background(Color::BLACK.scale_alpha(0.8))),
			));
		}

		base.into()
	}

	pub fn title(&self, window: Id) -> String {
		self.arrangement_view
			.clap_host
			.title(window)
			.unwrap_or_else(|| "Generic DAW".to_owned())
	}

	pub fn theme(&self, _window: Id) -> Theme {
		self.config.theme.into()
	}

	pub fn scale_factor(&self, _window: Id) -> f32 {
		self.config.scale_factor
	}

	pub fn subscription(&self) -> Subscription<Message> {
		let autosave = if self.config.autosave.enabled {
			every(Duration::from_secs(self.config.autosave.interval.get()))
				.map(|_| Message::AutosaveFile)
		} else {
			Subscription::none()
		};

		let keybinds = if self.config_view.is_none() && self.export_progress.is_none() {
			keybinds()
		} else {
			Subscription::none()
		};

		Subscription::batch([
			self.arrangement_view
				.subscription()
				.map(Message::Arrangement),
			autosave,
			keybinds,
		])
	}
}

fn keybinds() -> Subscription<Message> {
	event::listen_with(|e, s, _| match s {
		Status::Ignored => match e {
			Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
				match (modifiers.command(), modifiers.shift(), modifiers.alt()) {
					(false, false, false) => match key {
						keyboard::Key::Named(keyboard::key::Named::Space) => {
							Some(Message::TogglePlayback)
						}
						_ => None,
					},
					(true, false, false) => match key {
						keyboard::Key::Character(c) => match c.as_str() {
							"e" => Some(Message::ExportFileDialog),
							"n" => Some(Message::NewFile),
							"o" => Some(Message::OpenFileDialog),
							"s" => Some(Message::SaveFile),
							_ => None,
						},
						_ => None,
					},
					(true, true, false) => match key {
						keyboard::Key::Character(c) => match c.as_str() {
							"s" => Some(Message::SaveAsFileDialog),
							_ => None,
						},
						_ => None,
					},
					_ => None,
				}
			}
			_ => None,
		},
		Status::Captured => None,
	})
}
