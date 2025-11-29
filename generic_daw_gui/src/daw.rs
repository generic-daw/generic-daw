use crate::{
	arrangement_view::{self, Arrangement, ArrangementView, Feedback, Tab},
	components::{PICK_LIST_HANDLE, number_input},
	config::Config,
	config_view::{self, ConfigView},
	file_tree::{self, FileTree},
	icons::{chart_no_axes_gantt, pause, play, sliders_vertical, square},
	state::State,
	stylefns::{
		bordered_box_with_radius, button_with_radius, menu_style, pick_list_with_radius,
		progress_bar_with_radius, split_style,
	},
};
use generic_daw_core::{
	Export, MusicalTime,
	clap_host::{PluginBundle, PluginDescriptor, get_installed_plugins},
};
use generic_daw_utils::{NoClone, NoDebug};
use generic_daw_widget::dot::Dot;
use iced::{
	Center, Color, Element, Font, Function as _,
	Length::Fill,
	Shrink, Subscription, Task, Theme, border, keyboard,
	mouse::Interaction,
	padding,
	time::every,
	widget::{
		button, center, column, container, mouse_area, opaque, pick_list, progress_bar, row, space,
		stack, text,
	},
	window,
};
use iced_split::{Strategy, vertical_split};
use log::trace;
use rfd::AsyncFileDialog;
use std::{collections::HashMap, num::NonZero, path::Path, sync::Arc, time::Duration};

pub const DEFAULT_SPLIT_POSITION: f32 = 300.0;

#[derive(Clone, Debug)]
pub enum Message {
	Arrangement(arrangement_view::Message),
	FileTree(file_tree::Message),
	ConfigView(config_view::Message),

	NewFile,
	OpenLastFile,
	SaveFile,
	SaveAsFile(Arc<Path>),
	AutosaveFile,

	OpenFileDialog,
	SaveAsFileDialog,
	ExportFileDialog,
	PickSampleFileDialog(usize),

	Progress(f32),

	OpenFile(Arc<Path>),
	CantLoadSample(Arc<str>, NoClone<oneshot::Sender<Feedback<Arc<Path>>>>),
	FoundSampleResponse(usize, Feedback<Arc<Path>>),
	OpenedFile(Option<Arc<Path>>),

	ExportFile(Arc<Path>),
	ExportedFile(NoClone<Box<Export>>),

	OpenConfigView,
	CloseConfigView,
	MergeConfig(Box<Config>, bool),

	ToggleMetronome,
	ChangedBpm(u16),
	ChangedBpmText(String),
	ChangedNumerator(u8),
	ChangedNumeratorText(String),

	OnDrag(f32),
	OnDragEnd,
	OnDoubleClick,
}

const _: () = assert!(size_of::<Message>() == 56);

#[derive(Debug)]
pub struct Daw {
	config: Config,
	state: State,
	plugin_bundles: Arc<HashMap<PluginDescriptor, NoDebug<PluginBundle>>>,

	window_id: window::Id,
	current_project: Option<Arc<Path>>,

	arrangement_view: ArrangementView,
	file_tree: FileTree,
	config_view: Option<ConfigView>,
	split_at: f32,

	progress: Option<f32>,
	missing_samples: Vec<(Arc<str>, oneshot::Sender<Feedback<Arc<Path>>>)>,
}

impl Daw {
	pub fn create() -> (Self, Task<Message>) {
		let (main_window_id, open) = window::open(window::Settings {
			exit_on_close_request: false,
			maximized: true,
			..window::Settings::default()
		});
		let mut open = open.discard();

		let config = Config::read();
		let state = State::read();

		if config.open_last_project {
			open = open.chain(Task::done(Message::OpenLastFile));
		}

		let plugin_bundles = get_installed_plugins(&config.clap_paths);
		let file_tree = FileTree::new(&config.sample_paths);

		let (arrangement_view, futs) = ArrangementView::new(&config, &state, &plugin_bundles);
		open = Task::batch([open, futs.map(Message::Arrangement)]);

		let split_at = state.file_tree_split_at;

		(
			Self {
				config,
				state,
				plugin_bundles: plugin_bundles.into(),

				window_id: main_window_id,
				current_project: None,

				arrangement_view,
				file_tree,
				config_view: None,
				split_at,

				progress: None,
				missing_samples: Vec::new(),
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
					.update(message, &self.config, &mut self.state, &self.plugin_bundles)
					.map(Message::Arrangement);
			}
			Message::FileTree(action) => return self.handle_file_tree_message(action),
			Message::ConfigView(message) => {
				if let Some(config_view) = self.config_view.as_mut() {
					let action = config_view.update(message, self.window_id);
					let mut futs = vec![action.task.map(Message::ConfigView)];

					if let Some(config) = action.instruction {
						config.write();
						futs.push(self.update(Message::MergeConfig(config.into(), true)));
						futs.push(self.update(Message::OpenConfigView));
					}

					return Task::batch(futs);
				}
			}
			Message::NewFile => {
				let config = Config::read();
				let (wrapper, task) = Arrangement::create(&config);
				let fut1 = self.update(Message::MergeConfig(config.into(), false));
				let fut2 = self
					.arrangement_view
					.update(
						arrangement_view::Message::SetArrangement(NoClone(Box::new(wrapper))),
						&self.config,
						&mut self.state,
						&self.plugin_bundles,
					)
					.map(Message::Arrangement);

				return Task::batch([
					fut1,
					fut2,
					task.map(Box::new)
						.map(arrangement_view::Message::Batch)
						.map(Message::Arrangement),
				]);
			}
			Message::OpenLastFile => {
				if let Some(last_project) = self.state.last_project.clone() {
					return self.update(Message::OpenFile(last_project));
				}
			}
			Message::SaveFile => {
				return self.update(
					self.current_project
						.clone()
						.map_or(Message::SaveAsFileDialog, Message::SaveAsFile),
				);
			}
			Message::SaveAsFile(path) => {
				self.arrangement_view
					.arrangement
					.save(&path, &mut self.arrangement_view.clap_host);
				self.current_project = Some(path.clone());
				if self.state.last_project.as_deref() != Some(&path) {
					self.state.last_project = Some(path);
					self.state.write();
				}
			}
			Message::AutosaveFile => {
				if let Some(current_project) = self.current_project.clone() {
					return self.update(Message::SaveAsFile(current_project));
				}
			}
			Message::OpenFileDialog => {
				return window::run(self.window_id, |window| {
					AsyncFileDialog::new()
						.set_parent(window)
						.add_filter("Generic DAW project file", &["gdp"])
						.pick_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().into())
				.map(Message::OpenFile);
			}
			Message::SaveAsFileDialog => {
				return window::run(self.window_id, |window| {
					AsyncFileDialog::new()
						.set_parent(window)
						.add_filter("Generic DAW project file", &["gdp"])
						.save_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().with_extension("gdp").into())
				.map(Message::SaveAsFile);
			}
			Message::ExportFileDialog => {
				return window::run(self.window_id, |window| {
					AsyncFileDialog::new()
						.set_parent(window)
						.add_filter("Wave File", &["wav"])
						.save_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().with_extension("wav").into())
				.map(Message::ExportFile);
			}
			Message::PickSampleFileDialog(idx) => {
				return window::run(self.window_id, |window| {
					AsyncFileDialog::new().set_parent(window).pick_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().into())
				.map(Feedback::Use)
				.map(Message::FoundSampleResponse.with(idx));
			}
			Message::Progress(progress) => self.progress = Some(progress),
			Message::OpenFile(path) => {
				if self.progress.is_none() {
					self.progress = Some(0.0);
					return Arrangement::start_load(
						path,
						Config::read(),
						self.plugin_bundles.clone(),
					);
				}
			}
			Message::CantLoadSample(name, NoClone(sender)) => {
				if self.progress.is_some() {
					self.missing_samples.push((name, sender));
				}
			}
			Message::FoundSampleResponse(idx, response) => {
				self.missing_samples.remove(idx).1.send(response).unwrap();
			}
			Message::OpenedFile(path) => {
				if let Some(path) = path {
					self.current_project = Some(path.clone());
					if self.state.last_project.as_deref() != Some(&path) {
						self.state.last_project = Some(path);
						self.state.write();
					}
				}
				self.missing_samples.clear();
				self.progress = None;
			}
			Message::ExportFile(path) => {
				if self.progress.is_none() {
					self.progress = Some(0.0);
					self.arrangement_view.clap_host.set_realtime(false);
					return self.arrangement_view.arrangement.start_export(path);
				}
			}
			Message::ExportedFile(NoClone(audio_graph)) => {
				self.arrangement_view
					.arrangement
					.finish_export(*audio_graph);
				self.arrangement_view.clap_host.set_realtime(true);
				self.progress = None;
			}
			Message::OpenConfigView => self.config_view = Some(ConfigView::default()),
			Message::CloseConfigView => self.config_view = None,
			Message::MergeConfig(config, live) => {
				if self.config.clap_paths != config.clap_paths {
					self.plugin_bundles = get_installed_plugins(&config.clap_paths).into();
				}

				if self.config.sample_paths != config.sample_paths {
					self.file_tree.diff(&config.sample_paths);
				}

				if live {
					self.config.merge_with(*config);
				} else {
					self.config = *config;
				}
			}
			Message::ToggleMetronome => self.arrangement_view.arrangement.toggle_metronome(),
			Message::ChangedBpm(bpm) => self
				.arrangement_view
				.arrangement
				.set_bpm(NonZero::new(bpm.clamp(10, 999)).unwrap()),
			Message::ChangedBpmText(bpm) => {
				if let Ok(bpm) = bpm.parse() {
					return self.update(Message::ChangedBpm(bpm));
				}
			}
			Message::ChangedNumerator(numerator) => {
				self.arrangement_view
					.arrangement
					.set_numerator(NonZero::new(numerator.clamp(1, 99)).unwrap());
			}
			Message::ChangedNumeratorText(numerator) => {
				if let Ok(numerator) = numerator.parse() {
					return self.update(Message::ChangedNumerator(numerator));
				}
			}
			Message::OnDrag(split_at) => {
				self.split_at = if split_at >= 20.0 {
					split_at.clamp(200.0, 1000.0)
				} else {
					0.0
				};
			}
			Message::OnDragEnd => {
				if self.state.file_tree_split_at != self.split_at {
					self.state.file_tree_split_at = self.split_at;
					self.state.write();
				}
			}
			Message::OnDoubleClick => {
				return Task::batch([
					self.update(Message::OnDrag(DEFAULT_SPLIT_POSITION)),
					self.update(Message::OnDragEnd),
				]);
			}
		}

		Task::none()
	}

	fn handle_file_tree_message(&mut self, action: file_tree::Message) -> Task<Message> {
		match action {
			file_tree::Message::File(path) => {
				self.arrangement_view.playlist_selection.get_mut().file = Some((path, None));
			}
			file_tree::Message::Action(id, action) => {
				if let Some(task) = self.file_tree.update(id, &action) {
					return task.map(Message::FileTree);
				}
			}
		}

		Task::none()
	}

	pub fn view(&self, window: window::Id) -> Element<'_, Message> {
		if let Some(gui) = self.arrangement_view.clap_host.view(window) {
			return gui
				.map(arrangement_view::Message::ClapHost)
				.map(Message::Arrangement);
		}

		debug_assert_eq!(window, self.window_id);

		let now = MusicalTime::from_samples(
			self.arrangement_view.arrangement.rtstate().sample,
			self.arrangement_view.arrangement.rtstate(),
		);

		stack![
			column![
				row![
					pick_list(
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
					.handle(PICK_LIST_HANDLE)
					.style(pick_list_with_radius(5))
					.menu_style(menu_style),
					row![
						button(if self.arrangement_view.arrangement.rtstate().playing {
							pause
						} else {
							play
						}())
						.style(button_with_radius(button::primary, border::left(5)))
						.padding(padding::horizontal(7).vertical(5))
						.on_press(Message::Arrangement(
							arrangement_view::Message::TogglePlayback
						)),
						button(square())
							.style(button_with_radius(button::primary, border::right(5)))
							.padding(padding::horizontal(7).vertical(5))
							.on_press(Message::Arrangement(arrangement_view::Message::Stop)),
					],
					number_input(
						self.arrangement_view
							.arrangement
							.rtstate()
							.numerator
							.get()
							.into(),
						4,
						2,
						|x| Message::ChangedNumerator(x as u8),
						Message::ChangedNumeratorText
					),
					number_input(
						self.arrangement_view.arrangement.rtstate().bpm.get().into(),
						140,
						3,
						|x| Message::ChangedBpm(x as u16),
						Message::ChangedBpmText
					),
					row![
						container(
							text(format!(
								"{:#03}:{:#digits$}",
								now.bar(self.arrangement_view.arrangement.rtstate()) + 1,
								now.beat_in_bar(self.arrangement_view.arrangement.rtstate()) + 1,
								digits = self
									.arrangement_view
									.arrangement
									.rtstate()
									.numerator
									.ilog10() as usize + 1,
							))
							.font(Font::MONOSPACE)
						)
						.padding(padding::horizontal(7).vertical(5.6))
						.style(|t| bordered_box_with_radius(border::left(5))(t)
							.background(t.extended_palette().background.weakest.color)),
						button(
							row![
								Dot::new(now.beat().is_multiple_of(2)),
								Dot::new(!now.beat().is_multiple_of(2))
							]
							.spacing(5)
						)
						.style(button_with_radius(
							if self.arrangement_view.arrangement.rtstate().metronome {
								button::primary
							} else {
								button::secondary
							},
							border::right(5)
						))
						.padding(8)
						.on_press(Message::ToggleMetronome),
					],
					space::horizontal(),
					row![
						button(chart_no_axes_gantt())
							.style(button_with_radius(button::primary, border::left(5)))
							.padding(padding::horizontal(7).vertical(5))
							.on_press_maybe(
								(!matches!(self.arrangement_view.tab, Tab::Playlist)).then_some(
									Message::Arrangement(arrangement_view::Message::ChangedTab(
										Tab::Playlist
									))
								)
							),
						button(sliders_vertical())
							.style(button_with_radius(button::primary, border::right(5)))
							.padding(padding::horizontal(7).vertical(5))
							.on_press_maybe(
								(!matches!(self.arrangement_view.tab, Tab::Mixer)).then_some(
									Message::Arrangement(arrangement_view::Message::ChangedTab(
										Tab::Mixer
									))
								)
							)
					],
				]
				.spacing(10)
				.align_y(Center),
				vertical_split(
					self.file_tree.view().map(Message::FileTree),
					self.arrangement_view.view().map(Message::Arrangement),
					self.split_at,
					Message::OnDrag
				)
				.on_drag_end(Message::OnDragEnd)
				.on_double_click(Message::OnDoubleClick)
				.strategy(Strategy::Start)
				.focus_delay(Duration::ZERO)
				.style(split_style)
			]
			.padding(10)
			.spacing(10),
			self.arrangement_view
				.playlist_selection
				.borrow()
				.file
				.as_ref()
				.map(|_| mouse_area(space().width(Fill).height(Fill))
					.interaction(Interaction::Copy)
					.on_release(Message::Arrangement(
						arrangement_view::Message::LoadHoveredSample,
					))),
			self.arrangement_view
				.loading()
				.then(|| mouse_area(space().width(Fill).height(Fill))
					.interaction(Interaction::Progress)),
			self.config_view.as_ref().map(|config_view| opaque(
				mouse_area(
					center(opaque(
						config_view.view(&self.config).map(Message::ConfigView)
					))
					.style(|_| container::background(Color::BLACK.scale_alpha(0.8))),
				)
				.on_press(Message::CloseConfigView),
			)),
			self.progress.map(|progress| opaque(
				mouse_area(
					center(
						column![
							progress_bar(0.0..=1.0, progress).style(progress_bar_with_radius(
								if self.missing_samples.is_empty() {
									progress_bar::primary
								} else {
									progress_bar::danger
								},
								5
							)),
							(!self.missing_samples.is_empty()).then(|| container(
								column(
									self.missing_samples
										.iter()
										.map(|(name, _)| &**name)
										.enumerate()
										.map(|(i, name)| {
											row![
												"can't find sample",
												container(text(name).font(Font::MONOSPACE))
													.padding(padding::horizontal(10).vertical(5))
													.style(|t| bordered_box_with_radius(5)(t)
														.background(
															t.extended_palette()
																.background
																.weakest
																.color
														)),
												space::horizontal(),
												row![
													button("Pick")
														.on_press(Message::PickSampleFileDialog(i))
														.style(button_with_radius(
															button::success,
															border::left(5)
														)),
													button("Ignore")
														.on_press(Message::FoundSampleResponse(
															i,
															Feedback::Ignore
														))
														.style(button_with_radius(
															button::warning,
															0
														)),
													button("Cancel")
														.on_press(Message::FoundSampleResponse(
															i,
															Feedback::Cancel
														))
														.style(button_with_radius(
															button::danger,
															border::right(5)
														))
												]
											]
											.spacing(10)
											.width(Shrink)
											.align_y(Center)
											.into()
										}),
								)
								.spacing(10)
							)
							.padding(10)
							.style(bordered_box_with_radius(5)))
						]
						.align_x(Center)
						.spacing(20)
					)
					.padding(50)
					.style(|_| container::background(Color::BLACK.scale_alpha(0.8))),
				)
				.interaction(Interaction::Progress),
			))
		]
		.into()
	}

	pub fn title(&self, window: window::Id) -> String {
		self.arrangement_view
			.clap_host
			.title(window)
			.unwrap_or_else(|| "Generic DAW".to_owned())
	}

	pub fn theme(&self, _window: window::Id) -> Theme {
		self.config.theme.into()
	}

	pub fn scale_factor(&self, window: window::Id) -> f32 {
		self.arrangement_view
			.clap_host
			.scale_factor(window)
			.unwrap_or(self.config.app_scale_factor)
	}

	pub fn subscription(&self) -> Subscription<Message> {
		let autosave = if self.config.autosave.enabled {
			every(Duration::from_secs(self.config.autosave.interval.get()))
				.map(|_| Message::AutosaveFile)
		} else {
			Subscription::none()
		};

		let keybinds = if self.progress.is_some() {
			Subscription::none()
		} else if self.config_view.is_some() {
			keyboard::on_key_press(|k, m| {
				Self::config_view_keybinds(&k, m).or_else(|| Self::base_keybinds(&k, m))
			})
		} else {
			keyboard::on_key_press(|k, m| {
				Self::arrangement_view_keybinds(&k, m).or_else(|| Self::base_keybinds(&k, m))
			})
		};

		Subscription::batch([
			self.arrangement_view
				.subscription()
				.map(Message::Arrangement),
			autosave,
			keybinds,
		])
	}

	fn arrangement_view_keybinds(
		key: &keyboard::Key,
		modifiers: keyboard::Modifiers,
	) -> Option<Message> {
		match (modifiers.command(), modifiers.shift(), modifiers.alt()) {
			(false, false, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::F5) => Some(Message::Arrangement(
					arrangement_view::Message::ChangedTab(Tab::Playlist),
				)),
				keyboard::Key::Named(keyboard::key::Named::F9) => Some(Message::Arrangement(
					arrangement_view::Message::ChangedTab(Tab::Mixer),
				)),
				keyboard::Key::Named(
					keyboard::key::Named::Delete | keyboard::key::Named::Backspace,
				) => Some(Message::Arrangement(
					arrangement_view::Message::DeleteSelection,
				)),
				keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::Arrangement(
					arrangement_view::Message::ClearSelection,
				)),
				_ => None,
			},
			_ => None,
		}
	}

	fn config_view_keybinds(
		key: &keyboard::Key,
		modifiers: keyboard::Modifiers,
	) -> Option<Message> {
		match (modifiers.command(), modifiers.shift(), modifiers.alt()) {
			(false, false, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::Escape) => {
					Some(Message::CloseConfigView)
				}
				_ => None,
			},
			_ => None,
		}
	}

	fn base_keybinds(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Message> {
		match (modifiers.command(), modifiers.shift(), modifiers.alt()) {
			(false, false, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::Space) => Some(Message::Arrangement(
					arrangement_view::Message::TogglePlayback,
				)),
				_ => None,
			},
			(true, false, false) => match key.as_ref() {
				keyboard::Key::Character("e") => Some(Message::ExportFileDialog),
				keyboard::Key::Character("n") => Some(Message::NewFile),
				keyboard::Key::Character("o") => Some(Message::OpenFileDialog),
				keyboard::Key::Character("s") => Some(Message::SaveFile),
				_ => None,
			},
			(true, true, false) => match key.as_ref() {
				keyboard::Key::Character("s") => Some(Message::SaveAsFileDialog),
				_ => None,
			},
			_ => None,
		}
	}
}
