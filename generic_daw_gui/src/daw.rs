use crate::{
	action::Action,
	arrangement_view::{
		self, AUTOSAVE_DIR, Arrangement, ArrangementView, Feedback, PROJECT_DIR, Tab, format_now,
	},
	components::{PICK_LIST_HANDLE, number_input},
	config::Config,
	config_view::{self, ConfigView},
	file_tree::{self, FileTree},
	icons::{chart_no_axes_gantt, cpu, pause, play, plus, sliders_vertical, square},
	state::{DEFAULT_SPLIT_POSITION, State},
	stylefns::{
		bordered_box_with_radius, button_with_radius, menu_style, pick_list_with_radius,
		progress_bar_with_radius, split_style,
	},
	widget::OPACITY_67,
};
use generic_daw_core::{Export, MusicalTime};
use generic_daw_widget::dot::Dot;
use iced::{
	Center, Color, Element, Fill, Font, Shrink, Subscription, Task, Theme, border, keyboard,
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
use std::{
	fmt::{Display, Formatter},
	num::NonZero,
	path::Path,
	sync::Arc,
	time::Duration,
};
use utils::{NoClone, variants};

variants! {
#[derive(Clone, Copy, Eq, PartialEq)]
enum FileMenu {
	New,
	Open,
	OpenLast,
	Save,
	SaveAs,
	Export,
	Settings,
}
}

impl From<FileMenu> for Message {
	fn from(value: FileMenu) -> Self {
		match value {
			FileMenu::New => Self::NewFile,
			FileMenu::Open => Self::OpenFileDialog,
			FileMenu::OpenLast => Self::OpenLastFile,
			FileMenu::Save => Self::SaveFile,
			FileMenu::SaveAs => Self::SaveAsFileDialog,
			FileMenu::Export => Self::ExportFileDialog,
			FileMenu::Settings => Self::OpenConfigView,
		}
	}
}

impl Display for FileMenu {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::New => "New",
			Self::Open => "Open",
			Self::OpenLast => "Open Last",
			Self::Save => "Save",
			Self::SaveAs => "Save As",
			Self::Export => "Export",
			Self::Settings => "Settings",
		})
	}
}

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
	ToggleFullscreen,

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

	FileHovered,
	FileDropped(Arc<Path>),
	FileLeft,

	ToggleShowSeconds,
	ToggleMetronome,
	ChangedBpm(u16),
	ChangedBpmText(String),
	ChangedNumerator(u8),
	ChangedNumeratorText(String),

	OnDrag(f32),
	OnDragEnd,
	OnDoubleClick,
}

const _: () = assert!(size_of::<Message>() == 72);

#[derive(Debug)]
pub struct Daw {
	config: Config,
	state: State,
	current_project: Option<Arc<Path>>,

	arrangement_view: ArrangementView,
	file_tree: FileTree,
	config_view: Option<ConfigView>,
	split_at: f32,
	show_seconds: bool,

	progress: Option<f32>,
	missing_samples: Vec<(Arc<str>, oneshot::Sender<Feedback<Arc<Path>>>)>,

	main_window_id: window::Id,
	files_hovered: bool,
}

impl Daw {
	pub fn create() -> (Self, Task<Message>) {
		let (main_window_id, window) = window::open(window::Settings {
			exit_on_close_request: false,
			maximized: true,
			..window::Settings::default()
		});
		let window = window.discard();

		let config = Config::read();
		let state = State::read();

		let file_tree = FileTree::new(&config.sample_paths);

		let (mut arrangement_view, batches) = ArrangementView::new(&config, &state, main_window_id);
		let scan = arrangement_view.get_installed_plugins(&config);

		let open = if config.open_last_project {
			Task::done(Message::OpenLastFile)
		} else {
			Task::none()
		};

		let split_at = state.file_tree_split_at;
		let show_seconds = state.show_seconds;

		(
			Self {
				config,
				state,
				current_project: None,

				arrangement_view,
				file_tree,
				config_view: None,
				split_at,
				show_seconds,

				progress: None,
				missing_samples: Vec::new(),

				main_window_id,
				files_hovered: false,
			},
			Task::batch([
				window,
				batches.map(Message::Arrangement),
				scan.map(Message::Arrangement).chain(open),
			]),
		)
	}

	pub fn update(&mut self, message: Message) -> Task<Message> {
		trace!("{message:?}");

		match message {
			Message::Arrangement(message) => {
				let Action { instruction, task } =
					self.arrangement_view.update(message, &self.config);

				if let Some(plugins_panel_split_at) = instruction {
					self.state.plugins_panel_split_at = plugins_panel_split_at;
					self.state.write();
				}

				return task.map(Message::Arrangement);
			}
			Message::FileTree(action) => return self.handle_file_tree_message(action),
			Message::ConfigView(message) => {
				if let Some(config_view) = self.config_view.as_mut() {
					let Action { instruction, task } = config_view.update(message);
					let mut futs = vec![task.map(Message::ConfigView)];

					if let Some(config) = instruction {
						config.write();
						futs.push(self.update(Message::MergeConfig(config.into(), true)));
						futs.push(self.update(Message::OpenConfigView));
					}

					return Task::batch(futs);
				}
			}
			Message::NewFile => return Arrangement::empty(),
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
				if self
					.arrangement_view
					.arrangement
					.save(&path, &mut self.arrangement_view.clap_host)
					.is_ok()
				{
					self.current_project = Some(path.clone());
					if self.state.last_project.as_deref() != Some(&path) {
						self.state.last_project = Some(path);
						self.state.write();
					}
				}
			}
			Message::AutosaveFile => {
				let name = self
					.current_project
					.as_deref()
					.and_then(|path| path.file_prefix())
					.and_then(|name| name.to_str())
					.unwrap_or("autosaved");

				let path = AUTOSAVE_DIR.join(format!("{} {}.gdp", name, format_now()));

				_ = self
					.arrangement_view
					.arrangement
					.save(&path, &mut self.arrangement_view.clap_host);
			}
			Message::ToggleFullscreen => {
				let id = self.main_window_id;
				return window::mode(id).then(move |mode| match mode {
					window::Mode::Windowed => window::set_mode(id, window::Mode::Fullscreen),
					window::Mode::Fullscreen => window::set_mode(id, window::Mode::Windowed),
					window::Mode::Hidden => Task::none(),
				});
			}
			Message::OpenFileDialog => {
				return window::run(self.main_window_id, |window| {
					AsyncFileDialog::new()
						.set_parent(window)
						.add_filter("Generic DAW project file", &["gdp"])
						.set_directory(&*PROJECT_DIR)
						.pick_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().into())
				.map(Message::OpenFile);
			}
			Message::SaveAsFileDialog => {
				return window::run(self.main_window_id, |window| {
					AsyncFileDialog::new()
						.set_parent(window)
						.add_filter("Generic DAW project file", &["gdp"])
						.set_directory(&*PROJECT_DIR)
						.save_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().with_extension("gdp").into())
				.map(Message::SaveAsFile);
			}
			Message::ExportFileDialog => {
				return window::run(self.main_window_id, |window| {
					AsyncFileDialog::new()
						.set_parent(window)
						.add_filter("Wave file", &["wav"])
						.set_directory(&*PROJECT_DIR)
						.save_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().with_extension("wav").into())
				.map(Message::ExportFile);
			}
			Message::PickSampleFileDialog(idx) => {
				return window::run(self.main_window_id, |window| {
					AsyncFileDialog::new().set_parent(window).pick_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().into())
				.map(Feedback::Use)
				.map(move |response| Message::FoundSampleResponse(idx, response));
			}
			Message::Progress(progress) => self.progress = Some(progress),
			Message::OpenFile(path) => {
				if self.progress.is_none() {
					self.progress = Some(0.0);
					return Arrangement::start_load(
						path,
						self.arrangement_view.get_discovered_plugins(),
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
			Message::OpenConfigView => {
				self.config_view = Some(ConfigView::new(self.main_window_id));
			}
			Message::CloseConfigView => self.config_view = None,
			Message::MergeConfig(config, live) => {
				let fut = (self.config.clap_paths != config.clap_paths)
					.then(|| self.arrangement_view.get_installed_plugins(&config));

				if self.config.sample_paths != config.sample_paths {
					self.file_tree.diff(&config.sample_paths);
				}

				if live {
					self.config.merge_with(*config);
				} else {
					self.config = *config;
				}

				if let Some(fut) = fut {
					return fut.map(Message::Arrangement);
				}
			}
			Message::FileHovered => self.files_hovered = true,
			Message::FileDropped(path) => {
				self.files_hovered = false;
				if self.split_at != 0.0
					&& std::fs::metadata(&path).is_ok_and(|metadata| metadata.is_dir())
				{
					self.config.sample_paths.push(path);
					self.file_tree.diff(&self.config.sample_paths);
					self.config.write();
				}
			}
			Message::FileLeft => self.files_hovered = false,
			Message::ToggleShowSeconds => {
				self.show_seconds ^= true;
				if self.state.show_seconds != self.show_seconds {
					self.state.show_seconds = self.show_seconds;
					self.state.write();
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
			file_tree::Message::File(file, kind) => {
				self.arrangement_view.hover_file(file, kind);
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

		debug_assert_eq!(window, self.main_window_id);

		let now = MusicalTime::from_samples(
			self.arrangement_view.arrangement.transport().sample,
			self.arrangement_view.arrangement.transport(),
		);

		stack![
			column![
				row![
					pick_list(None::<FileMenu>, FileMenu::VARIANTS, FileMenu::to_string)
						.on_select(Message::from)
						.handle(PICK_LIST_HANDLE)
						.placeholder("File")
						.style(pick_list_with_radius(5))
						.menu_style(menu_style),
					row![
						button(if self.arrangement_view.arrangement.transport().playing {
							pause()
						} else {
							play()
						})
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
							.transport()
							.numerator
							.get()
							.into(),
						4,
						2,
						|numerator| Message::ChangedNumerator(numerator as u8),
						Message::ChangedNumeratorText
					),
					number_input(
						self.arrangement_view
							.arrangement
							.transport()
							.bpm
							.get()
							.into(),
						140,
						3,
						|bpm| Message::ChangedBpm(bpm as u16),
						Message::ChangedBpmText
					),
					row![
						mouse_area(
							container(
								if self.show_seconds {
									let duration = now
										.to_duration(self.arrangement_view.arrangement.transport());
									text!(
										"{:02}:{:02}:{:02}",
										duration.as_secs() / 60,
										duration.as_secs() % 60,
										(duration.as_secs_f32().fract() * 100.0) as u8
									)
								} else {
									text!(
										"{:03}:{:digits$}",
										now.bar(self.arrangement_view.arrangement.transport()) + 1,
										now.beat_in_bar(
											self.arrangement_view.arrangement.transport()
										) + 1,
										digits = self
											.arrangement_view
											.arrangement
											.transport()
											.numerator
											.ilog10() as usize + 1,
									)
								}
								.font(Font::MONOSPACE)
							)
							.padding(padding::horizontal(7).vertical(5.6))
							.style(|t| bordered_box_with_radius(border::left(5))(t)
								.background(t.extended_palette().background.weakest.color))
						)
						.on_press(Message::ToggleShowSeconds)
						.interaction(Interaction::Pointer),
						button(
							row![
								Dot::new(now.beat().is_multiple_of(2)),
								Dot::new(!now.beat().is_multiple_of(2))
							]
							.spacing(5)
						)
						.style(button_with_radius(
							if self.arrangement_view.arrangement.transport().metronome {
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
						cpu(),
						text!("{:.1}%", self.arrangement_view.arrangement.load() * 100.0)
							.font(Font::MONOSPACE)
					]
					.spacing(5),
					row![
						button(chart_no_axes_gantt())
							.style(button_with_radius(button::primary, border::left(5)))
							.padding(padding::horizontal(7).vertical(5))
							.on_press_maybe(
								(!matches!(self.arrangement_view.tab(), Tab::Playlist)).then_some(
									Message::Arrangement(arrangement_view::Message::ChangedTab(
										Tab::Playlist
									))
								)
							),
						button(sliders_vertical())
							.style(button_with_radius(button::primary, border::right(5)))
							.padding(padding::horizontal(7).vertical(5))
							.on_press_maybe(
								(!matches!(self.arrangement_view.tab(), Tab::Mixer)).then_some(
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
					stack![
						self.file_tree.view().map(Message::FileTree),
						self.files_hovered.then(|| container(plus().size(40.0))
							.center_x(Fill)
							.center_y(Fill)
							.style(|_| container::background(
								Color::BLACK.scale_alpha(OPACITY_67)
							)))
					],
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
			self.arrangement_view.hovering_file().then(|| mouse_area(
				space().width(Fill).height(Fill)
			)
			.interaction(Interaction::Copy)
			.on_release(Message::Arrangement(
				arrangement_view::Message::LoadHoveredFile,
			))
			.on_exit(Message::Arrangement(
				arrangement_view::Message::LoadHoveredFile,
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
					.style(|_| container::background(Color::BLACK.scale_alpha(OPACITY_67))),
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
					.style(|_| container::background(Color::BLACK.scale_alpha(OPACITY_67))),
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
		if self.progress.is_some() {
			return Subscription::none();
		}

		let autosave = if self.config.autosave.enabled {
			every(Duration::from_secs(self.config.autosave.interval.get()))
				.map(|_| Message::AutosaveFile)
		} else {
			Subscription::none()
		};

		let keybinds = if self.config_view.is_some() {
			keyboard::listen().filter_map(|e| match e {
				keyboard::Event::KeyPressed {
					key,
					physical_key,
					modifiers,
					repeat,
					..
				} => Self::config_view_keybinds(&key, modifiers, repeat)
					.or_else(|| Self::base_keybinds(&key, physical_key, modifiers, repeat)),
				_ => None,
			})
		} else {
			keyboard::listen().filter_map(|e| match e {
				keyboard::Event::KeyPressed {
					key,
					physical_key,
					modifiers,
					repeat,
					..
				} => Self::arrangement_view_keybinds(&key, modifiers, repeat)
					.map(Message::Arrangement)
					.or_else(|| Self::base_keybinds(&key, physical_key, modifiers, repeat)),
				_ => None,
			})
		};

		Subscription::batch([
			self.arrangement_view
				.subscription()
				.map(Message::Arrangement),
			autosave,
			keybinds,
			window::events().filter_map(|(_, event)| match event {
				window::Event::FileHovered(..) => Some(Message::FileHovered),
				window::Event::FileDropped(file) => Some(Message::FileDropped(file.into())),
				window::Event::FilesHoveredLeft => Some(Message::FileLeft),
				_ => None,
			}),
		])
	}

	fn arrangement_view_keybinds(
		key: &keyboard::Key,
		modifiers: keyboard::Modifiers,
		repeat: bool,
	) -> Option<arrangement_view::Message> {
		match (
			modifiers.command(),
			modifiers.shift(),
			modifiers.alt(),
			repeat,
		) {
			(false, false, false, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::F5) => {
					Some(arrangement_view::Message::ChangedTab(Tab::Playlist))
				}
				keyboard::Key::Named(keyboard::key::Named::F9) => {
					Some(arrangement_view::Message::ChangedTab(Tab::Mixer))
				}
				keyboard::Key::Named(
					keyboard::key::Named::Delete | keyboard::key::Named::Backspace,
				) => Some(arrangement_view::Message::DeleteSelection),
				keyboard::Key::Named(keyboard::key::Named::Escape) => {
					Some(arrangement_view::Message::ClearSelection)
				}
				keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
					Some(arrangement_view::Message::ArrowLeft)
				}
				keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
					Some(arrangement_view::Message::ArrowRight)
				}
				_ => None,
			},
			(false, false, false, true) => match key.as_ref() {
				keyboard::Key::Named(
					keyboard::key::Named::Delete | keyboard::key::Named::Backspace,
				) => Some(arrangement_view::Message::DeleteSelection),
				keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
					Some(arrangement_view::Message::ArrowLeft)
				}
				keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
					Some(arrangement_view::Message::ArrowRight)
				}
				_ => None,
			},
			_ => None,
		}
	}

	fn config_view_keybinds(
		key: &keyboard::Key,
		modifiers: keyboard::Modifiers,
		repeat: bool,
	) -> Option<Message> {
		match (
			modifiers.command(),
			modifiers.shift(),
			modifiers.alt(),
			repeat,
		) {
			(false, false, false, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::Escape) => {
					Some(Message::CloseConfigView)
				}
				_ => None,
			},
			_ => None,
		}
	}

	fn base_keybinds(
		key: &keyboard::Key,
		physical_key: keyboard::key::Physical,
		modifiers: keyboard::Modifiers,
		repeat: bool,
	) -> Option<Message> {
		match (
			modifiers.command(),
			modifiers.shift(),
			modifiers.alt(),
			repeat,
		) {
			(false, false, false, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::Space) => Some(Message::Arrangement(
					arrangement_view::Message::TogglePlayback,
				)),
				keyboard::Key::Named(keyboard::key::Named::F11) => Some(Message::ToggleFullscreen),
				_ => None,
			},
			(true, false, false, false) => match key.to_latin(physical_key)? {
				'e' => Some(Message::ExportFileDialog),
				'n' => Some(Message::NewFile),
				'o' => Some(Message::OpenFileDialog),
				's' => Some(Message::SaveFile),
				_ => None,
			},
			(true, true, false, false) => match key.to_latin(physical_key)? {
				's' => Some(Message::SaveAsFileDialog),
				_ => None,
			},
			_ => None,
		}
	}
}
