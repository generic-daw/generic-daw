use crate::{
	arrangement_view::{
		self, AUTOSAVE_DIR, Arrangement, ArrangementView, Feedback, PROJECT_DIR, Tab, format_now,
	},
	clap_host::{self, ClapHost},
	components::{PICK_LIST_HANDLE, number_input},
	config::Config,
	config_view::{self, ConfigView},
	file_tree::{self, FileTree},
	icons::{
		arrow_big_right, chart_no_axes_gantt, cpu, keyboard_music, metronome, pause, play, plus,
		sliders_vertical, square,
	},
	state::{DEFAULT_SPLIT_POSITION, State},
	stylefns::{
		bordered_box_with_radius, button_with_radius, menu_style, pick_list_with_radius,
		progress_bar_with_radius, split_style,
	},
	widget::ALPHA_2_3,
};
use generic_daw_core::{
	Event, MusicalTime, PluginId,
	clap_host::{
		DEFAULT_CLAP_PATHS, HostInfo, MainThreadMessage, Plugin, PluginDescriptor, RenderMode,
	},
};
use iced::{
	Center, Color, Element, Fill, Font, Shrink, Subscription, Task, Theme, border, keyboard,
	mouse::Interaction,
	padding,
	time::every,
	widget::{
		bottom_center, button, center, column, combo_box, container, mouse_area, opaque, pick_list,
		progress_bar, row, scrollable, space, stack, text,
	},
	window,
};
use iced_split::{Strategy, vertical_split};
use log::{trace, warn};
use rfd::AsyncFileDialog;
use scan::Id as Scan;
use smol::unblock;
use std::{
	fmt::{Display, Formatter},
	num::NonZero,
	path::Path,
	sync::{Arc, LazyLock, mpsc::Receiver},
	time::Duration,
};
use utils::{NoClone, NoDebug, natural_cmp, unique_id, variants};

unique_id!(scan);
unique_id!(project);

pub use project::Id as Project;

pub static HOST: LazyLock<HostInfo> = LazyLock::new(|| {
	HostInfo::new_from_cstring(
		c"Generic DAW".to_owned(),
		c"Generic DAW".to_owned(),
		c"https://github.com/generic-daw/generic-daw".to_owned(),
		c"0.0.0".to_owned(),
	)
});

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

pub enum Instruction {
	Message(Message),
	PluginLoad(PluginId, Plugin<Event>, Receiver<MainThreadMessage>),
	PluginSetState(PluginId, NoDebug<Box<[u8]>>),
	PluginShow(PluginId),
}

#[derive(Clone, Debug)]
pub enum Message {
	Arrangement(Project, arrangement_view::Message),
	ClapHost(clap_host::Message),
	FileTree(file_tree::Message),
	ConfigView(config_view::Message),

	CloseRequested(window::Id),
	ProjectLoaded(Project, NoClone<Box<Arrangement>>),

	PluginScanned(Scan, PluginDescriptor),
	PluginScanFinished(Scan),

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
	SetStatus(Arc<str>),
	ClearStatus,

	FileOpen(Arc<Path>),
	CantLoadSample(Arc<str>, NoClone<oneshot::Sender<Feedback<Arc<Path>>>>),
	FoundSampleResponse(usize, Feedback<Arc<Path>>),
	FileOpened(Option<Arc<Path>>),

	ExportFile(Arc<Path>),
	ExportedFile,

	OpenConfigView,
	CloseConfigView,
	MergeConfig(Box<Config>, bool),

	FileHovered,
	FileDropped(Arc<Path>),
	FileHoveredLeft,

	TogglePlayback,
	Stop,
	ToggleShowSeconds,
	ToggleMetronome,
	ToggleAutoscroll,
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
	clap_host: ClapHost,
	file_tree: FileTree,
	config_view: Option<ConfigView>,

	plugins: combo_box::State<PluginDescriptor>,

	progress: Option<f32>,
	status: Option<Arc<str>>,
	missing_samples: Vec<(Arc<str>, oneshot::Sender<Feedback<Arc<Path>>>)>,

	main_window_id: window::Id,
	project: Project,
	scan: Option<Scan>,
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

		let project = Project::unique();
		let (arrangement_view, batches) = ArrangementView::create(&config, &state);
		let clap_host = ClapHost::new(main_window_id);
		let file_tree = FileTree::new(&config.sample_paths);

		let open = if config.open_last_project {
			Task::done(Message::OpenLastFile)
		} else {
			Task::none()
		};

		let scan = Scan::unique();
		let plugins = get_installed_plugins(&config);

		(
			Self {
				config,
				state,
				current_project: None,

				arrangement_view,
				clap_host,
				file_tree,
				config_view: None,

				plugins: combo_box::State::default(),

				progress: None,
				status: None,
				missing_samples: Vec::new(),

				main_window_id,
				project,
				scan: Some(scan),
				files_hovered: false,
			},
			Task::batch([
				window,
				batches.map(move |message| Message::Arrangement(project, message)),
				plugins
					.map(move |descriptor| Message::PluginScanned(scan, descriptor))
					.chain(Task::done(Message::PluginScanFinished(scan)))
					.chain(open),
			]),
		)
	}

	pub fn update(&mut self, message: Message) -> Task<Message> {
		trace!("{message:?}");

		match message {
			Message::Arrangement(project, message) => {
				if project == self.project {
					return self
						.arrangement_view
						.update(message, &self.config, &mut self.state)
						.handle(
							move |message| Message::Arrangement(project, message),
							|instruction| self.handle_instruction(instruction),
						);
				}
			}
			Message::ClapHost(message) => {
				return self.clap_host.update(message).map(Message::ClapHost);
			}
			Message::FileTree(message) => return self.handle_file_tree_message(message),
			Message::ConfigView(message) => {
				if let Some(config_view) = self.config_view.as_mut() {
					return config_view
						.update(message)
						.handle(Message::ConfigView, |config| {
							self.update(Message::MergeConfig(config.into(), true))
						});
				}
			}
			Message::CloseRequested(window) => {
				if window == self.main_window_id {
					return iced::exit();
				}
			}
			Message::ProjectLoaded(project, NoClone(arrangement)) => {
				self.arrangement_view = ArrangementView::new(*arrangement, &self.state);
				self.project = project;
			}
			Message::PluginScanned(scan, descriptor) => {
				if self.scan == Some(scan)
					&& let Err(i) = self.plugins.options().binary_search_by(|d| {
						natural_cmp(d.name.as_bytes(), descriptor.name.as_bytes())
					}) {
					self.plugins.insert(i, descriptor);
				}
			}
			Message::PluginScanFinished(scan) => {
				if self.scan == Some(scan) {
					self.scan = None;
				}
			}
			Message::NewFile => return Arrangement::empty(),
			Message::OpenLastFile => {
				if let Some(last_project) = self.state.last_project.clone() {
					return self.update(Message::FileOpen(last_project));
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
				match self
					.arrangement_view
					.arrangement
					.save(&path, &mut self.clap_host)
				{
					Ok(()) => return self.update(Message::FileOpened(Some(path))),
					Err(err) => warn!("{err}"),
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

				if let Err(err) = self
					.arrangement_view
					.arrangement
					.save(&path, &mut self.clap_host)
				{
					warn!("{err}");
				}
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
				.map(Message::FileOpen);
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
			Message::SetStatus(scanning) => self.status = Some(scanning),
			Message::ClearStatus => self.status = None,
			Message::FileOpen(path) => {
				if self.progress.is_none() {
					self.progress = Some(0.0);
					return Arrangement::start_load(path, self.plugins.clone().into_options());
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
			Message::FileOpened(path) => {
				if let Some(path) = path {
					self.current_project = Some(path.clone());
					self.state.last_project = Some(path);
					self.state.write();
				}
				self.progress = None;
				self.status = None;
				self.missing_samples.clear();
			}
			Message::ExportFile(path) => {
				if self.progress.is_none() {
					self.progress = Some(0.0);
					self.clap_host.set_render_mode(RenderMode::Offline);
					return self.arrangement_view.arrangement.export(path);
				}
			}
			Message::ExportedFile => {
				self.clap_host.set_render_mode(RenderMode::Realtime);
				self.progress = None;
			}
			Message::OpenConfigView => {
				self.config_view = Some(ConfigView::new(self.main_window_id));
			}
			Message::CloseConfigView => self.config_view = None,
			Message::MergeConfig(config, live) => {
				let fut = if self.config.clap_paths == config.clap_paths {
					Task::none()
				} else {
					let scan = Scan::unique();
					self.scan = Some(scan);
					self.plugins = combo_box::State::default();
					get_installed_plugins(&config)
						.map(move |descriptor| Message::PluginScanned(scan, descriptor))
						.chain(Task::done(Message::PluginScanFinished(scan)))
				};

				if self.config.sample_paths != config.sample_paths {
					self.file_tree.diff(&config.sample_paths);
				}

				if live {
					self.config.merge_with(*config);
				} else {
					self.config = *config;
				}

				return fut;
			}
			Message::FileHovered => self.files_hovered = true,
			Message::FileDropped(path) => {
				self.files_hovered = false;
				if self.state.file_tree_split_at != 0.0
					&& path.metadata().is_ok_and(|metadata| metadata.is_dir())
				{
					self.config.sample_paths.push(path);
					self.file_tree.diff(&self.config.sample_paths);
					self.config.write();
				}
			}
			Message::FileHoveredLeft => self.files_hovered = false,
			Message::TogglePlayback => {
				self.arrangement_view.arrangement.toggle_playback();
				self.arrangement_view.end_recording();
			}
			Message::Stop => {
				self.arrangement_view.arrangement.stop();
				self.arrangement_view.end_recording();
			}
			Message::ToggleShowSeconds => {
				self.state.show_seconds ^= true;
				self.state.write();
			}
			Message::ToggleMetronome => {
				self.arrangement_view.arrangement.toggle_metronome();
				self.state.metronome ^= true;
				self.state.write();
			}
			Message::ToggleAutoscroll => {
				self.state.autoscroll ^= true;
				self.state.write();
			}
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
				self.state.file_tree_split_at = if split_at >= 20.0 {
					split_at.clamp(200.0, 1000.0)
				} else {
					0.0
				};
			}
			Message::OnDragEnd => self.state.write(),
			Message::OnDoubleClick => {
				return Task::batch([
					self.update(Message::OnDrag(DEFAULT_SPLIT_POSITION)),
					self.update(Message::OnDragEnd),
				]);
			}
		}

		Task::none()
	}

	fn handle_instruction(&mut self, instruction: Instruction) -> Task<Message> {
		match instruction {
			Instruction::Message(message) => self.update(message),
			Instruction::PluginLoad(id, plugin, receiver) => self
				.clap_host
				.load(id, plugin, receiver)
				.map(Message::ClapHost),
			Instruction::PluginSetState(id, state) => {
				self.clap_host.set_state(id, &state);
				Task::none()
			}
			Instruction::PluginShow(id) => self
				.clap_host
				.update(clap_host::Message::GuiOpen(id))
				.map(Message::ClapHost),
		}
	}

	fn handle_file_tree_message(&mut self, message: file_tree::Message) -> Task<Message> {
		match message {
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
		if let Some(gui) = self.clap_host.view(window) {
			return gui.map(Message::ClapHost);
		}

		debug_assert_eq!(window, self.main_window_id);

		let transport = self.arrangement_view.arrangement.transport();
		let now = MusicalTime::from_samples(transport.sample, transport);

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
						button(if transport.playing { pause() } else { play() })
							.style(button_with_radius(button::primary, border::left(5)))
							.padding(padding::horizontal(7).vertical(5))
							.on_press(Message::TogglePlayback),
						button(square())
							.style(button_with_radius(button::primary, border::right(5)))
							.padding(padding::horizontal(7).vertical(5))
							.on_press(Message::Stop),
					],
					number_input(
						transport.numerator.get().into(),
						4,
						2,
						|numerator| Message::ChangedNumerator(numerator as u8),
						Message::ChangedNumeratorText
					),
					number_input(
						transport.bpm.get().into(),
						140,
						3,
						|bpm| Message::ChangedBpm(bpm as u16),
						Message::ChangedBpmText
					),
					row![
						mouse_area(
							container(
								if self.state.show_seconds {
									let duration = now.to_duration(transport);
									text!(
										"{:02}:{:02}:{:02}",
										duration.as_secs() / 60,
										duration.as_secs() % 60,
										(duration.as_secs_f32().fract() * 100.0) as u8
									)
								} else {
									text!(
										"{:03}:{:0digits$}",
										now.bar(transport) + 1,
										now.beat_in_bar(transport) + 1,
										digits = transport.numerator.ilog10() as usize + 1,
									)
								}
								.font(Font::MONOSPACE)
							)
							.padding(padding::horizontal(7).vertical(5))
							.style(|t| bordered_box_with_radius(border::left(5))(t)
								.background(t.palette().background.weakest.color))
						)
						.on_press(Message::ToggleShowSeconds)
						.interaction(Interaction::Pointer),
						button(metronome())
							.style(button_with_radius(
								if self.state.metronome {
									button::primary
								} else {
									button::secondary
								},
								border::right(5)
							))
							.padding(padding::all(5).left(4))
							.on_press(Message::ToggleMetronome),
					],
					button(arrow_big_right())
						.style(button_with_radius(
							if self.state.autoscroll {
								button::primary
							} else {
								button::secondary
							},
							5
						))
						.padding(5)
						.on_press(Message::ToggleAutoscroll),
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
								(self.arrangement_view.tab() != Tab::Playlist).then_some(
									Message::Arrangement(
										self.project,
										arrangement_view::Message::ChangedTab(Tab::Playlist)
									)
								)
							),
						button(sliders_vertical())
							.style(button_with_radius(button::primary, 0))
							.padding(padding::horizontal(7).vertical(5))
							.on_press_maybe((self.arrangement_view.tab() != Tab::Mixer).then_some(
								Message::Arrangement(
									self.project,
									arrangement_view::Message::ChangedTab(Tab::Mixer)
								)
							)),
						button(keyboard_music())
							.style(button_with_radius(
								if self.arrangement_view.midi_clip().is_some() {
									button::primary
								} else {
									button::secondary
								},
								border::right(5)
							))
							.padding(padding::horizontal(7).vertical(5))
							.on_press_maybe(
								(self.arrangement_view.midi_clip().is_some()
									&& self.arrangement_view.tab() != Tab::PianoRoll)
									.then_some(Message::Arrangement(
										self.project,
										arrangement_view::Message::ChangedTab(Tab::PianoRoll)
									))
							),
					],
				]
				.spacing(10)
				.align_y(Center),
				vertical_split(
					stack![
						self.file_tree.view().map(Message::FileTree),
						self.files_hovered.then(|| center(plus().size(40.0))
							.style(|_| container::background(Color::BLACK.scale_alpha(ALPHA_2_3))))
					],
					self.arrangement_view
						.view(&self.state, &self.plugins)
						.map(|message| Message::Arrangement(self.project, message)),
					self.state.file_tree_split_at,
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
				.loading()
				.then(|| mouse_area(space().width(Fill).height(Fill))
					.interaction(Interaction::Progress)),
			self.config_view.as_ref().map(|config_view| opaque(
				mouse_area(
					center(opaque(
						config_view.view(&self.config).map(Message::ConfigView)
					))
					.style(|_| container::background(Color::BLACK.scale_alpha(ALPHA_2_3))),
				)
				.on_press(Message::CloseConfigView),
			)),
			self.progress.map(|progress| mouse_area(
				container(
					column![
						bottom_center(self.status.as_deref().map(|scanning| {
							container(
								row![
									"scanning",
									container(
										text(scanning)
											.font(Font::MONOSPACE)
											.wrapping(text::Wrapping::None)
											.ellipsis(text::Ellipsis::Middle)
									)
									.padding(padding::horizontal(10).vertical(5))
									.style(|t| bordered_box_with_radius(5)(t)
										.background(t.palette().background.weakest.color)),
								]
								.spacing(10)
								.width(Shrink)
								.align_y(Center),
							)
							.padding(10)
							.style(bordered_box_with_radius(5))
						})),
						column![
							progress_bar(0.0..=1.0, progress).style(progress_bar_with_radius(
								if self.missing_samples.is_empty() {
									progress_bar::primary
								} else {
									progress_bar::danger
								},
								5
							)),
							(!self.missing_samples.is_empty()).then(|| scrollable(
								column(
									self.missing_samples
										.iter()
										.map(|(name, _)| &**name)
										.enumerate()
										.map(|(i, name)| {
											container(
												row![
													"can't find sample",
													container(
														text(name)
															.font(Font::MONOSPACE)
															.wrapping(text::Wrapping::None)
															.ellipsis(text::Ellipsis::Middle)
													)
													.padding(padding::horizontal(10).vertical(5))
													.style(|t| {
														bordered_box_with_radius(5)(t).background(
															t.palette().background.weakest.color,
														)
													}),
													space::horizontal(),
													row![
														button("Pick")
															.on_press(
																Message::PickSampleFileDialog(i)
															)
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
												.width(Shrink)
												.align_y(Center)
												.spacing(10),
											)
											.padding(10)
											.style(bordered_box_with_radius(5))
											.into()
										}),
								)
								.align_x(Center)
								.spacing(10)
							)
							.spacing(10))
						]
						.align_x(Center)
						.spacing(20),
						space::vertical(),
					]
					.align_x(Center)
					.spacing(20)
				)
				.padding(50)
				.style(|_| container::background(Color::BLACK.scale_alpha(ALPHA_2_3))),
			)
			.interaction(Interaction::Progress))
		]
		.into()
	}

	pub fn title(&self, window: window::Id) -> String {
		self.clap_host
			.title(window)
			.unwrap_or_else(|| "Generic DAW".to_owned())
	}

	pub fn theme(&self, _window: window::Id) -> Theme {
		self.config.theme.into()
	}

	pub fn scale_factor(&self, window: window::Id) -> f32 {
		self.clap_host
			.scale_factor(window)
			.unwrap_or(self.config.scale_factor)
	}

	pub fn subscription(&self) -> Subscription<Message> {
		let autosave = if self.config.autosave.enabled {
			every(Duration::from_secs(
				self.config.autosave.interval.get().into(),
			))
			.map(|_| Message::AutosaveFile)
		} else {
			Subscription::none()
		};

		let keybinds = if self.progress.is_some() {
			Subscription::none()
		} else if self.config_view.is_some() {
			keyboard::listen().filter_map(|event| match event {
				keyboard::Event::KeyPressed {
					key,
					physical_key,
					modifiers,
					repeat,
					..
				} => ConfigView::keybinds(&key, modifiers, repeat)
					.or_else(|| Self::keybinds(&key, physical_key, modifiers, repeat)),
				_ => None,
			})
		} else {
			keyboard::listen()
				.with(self.project)
				.filter_map(|(project, event)| match event {
					keyboard::Event::KeyPressed {
						key,
						physical_key,
						modifiers,
						repeat,
						..
					} => ArrangementView::keybinds(&key, physical_key, modifiers, repeat)
						.map(|message| Message::Arrangement(project, message))
						.or_else(|| Self::keybinds(&key, physical_key, modifiers, repeat)),
					_ => None,
				})
		};

		Subscription::batch([
			ArrangementView::subscription()
				.with(self.project)
				.map(|(project, message)| Message::Arrangement(project, message)),
			self.clap_host.subscription().map(Message::ClapHost),
			autosave,
			keybinds,
			window::events().filter_map(|(window, event)| match event {
				window::Event::CloseRequested => Some(Message::CloseRequested(window)),
				window::Event::FileHovered(..) => Some(Message::FileHovered),
				window::Event::FileDropped(file) => Some(Message::FileDropped(file.into())),
				window::Event::FilesHoveredLeft => Some(Message::FileHoveredLeft),
				_ => None,
			}),
		])
	}

	fn keybinds(
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
				keyboard::Key::Named(keyboard::key::Named::Space) => Some(Message::TogglePlayback),
				keyboard::Key::Named(keyboard::key::Named::F11) => Some(Message::ToggleFullscreen),
				_ => None,
			},
			(true, false, false, false) => match key.to_latin(physical_key)? {
				'e' => Some(Message::ExportFileDialog),
				'm' => Some(Message::ToggleMetronome),
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

fn get_installed_plugins(config: &Config) -> Task<PluginDescriptor> {
	let (sender, receiver) = smol::channel::unbounded();
	let clap_paths = config.clap_paths.clone();

	Task::batch([
		Task::future(unblock(move || {
			generic_daw_core::clap_host::get_installed_plugins(
				DEFAULT_CLAP_PATHS.iter().chain(&clap_paths),
				|descriptor| _ = sender.try_send(descriptor),
			);
		}))
		.discard(),
		Task::stream(receiver),
	])
}
