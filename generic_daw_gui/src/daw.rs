use crate::{
	arrangement_view::{self, Arrangement, ArrangementView, Feedback},
	clap_host::{self, ClapHost},
	components::{menu_entry, number_input},
	config::Config,
	config_view::{self, ConfigView},
	file_tree::{self, FileKind, FileTree},
	icons::{
		arrow_big_right, chart_no_axes_gantt, cpu, gavel, keyboard_music, magnet, menu, metronome,
		panel_bottom_dashed, pause, play, plus, sliders_vertical, square,
	},
	state::{DEFAULT_BOTTOM_PANE_POSITON, DEFAULT_SPLIT_POSITION, State},
	stylefns::{
		button_with_radius, container_with_radius, progress_bar_with_radius, selectable_box,
		split_style, weak_bordered_box, weaker_bordered_box, weakest_bordered_box,
	},
	widget::ALPHA_2_3,
};
use generic_daw_core::{
	AudioThread, BpmTapper, NodeId, PluginId, PullSlot, build_streams,
	clap_host::{
		ClapId, DEFAULT_CLAP_PATHS, MainThreadMessage, Plugin, PluginDescriptor, RenderMode,
		StateContextType,
	},
};
use generic_daw_project::proto;
use generic_daw_widget::{context_menu::ContextMenu, menu::Menu, select_area::SelectArea};
use iced::{
	Center, Color, Element, Fill, Font, Shrink, Subscription, Task, Theme, border, keyboard,
	mouse::Interaction,
	padding,
	time::every,
	widget::{
		bottom_center, button, center, checkbox, column, combo_box, container, mouse_area, opaque,
		progress_bar, right, row, rule, scrollable, slider, space, stack, text,
	},
	window,
};
use iced_split::{Strategy, horizontal_split, vertical_split};
use log::{trace, warn};
use rfd::AsyncFileDialog;
use scan::Id as Scan;
use smol::unblock;
use std::{
	convert::Infallible,
	ffi::CStr,
	num::NonZero,
	path::Path,
	sync::{Arc, LazyLock, mpsc::Receiver},
	time::Duration,
};
use utils::{NoClone, NoDebug, natural_cmp, unique_id};

unique_id!(scan);
unique_id!(project);

pub use project::Id as Project;

pub static CONFIG_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let config_dir = dirs::config_dir().unwrap().join("Generic DAW").into();
	_ = std::fs::create_dir(&config_dir);
	config_dir
});

pub static DATA_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let data_dir = dirs::data_dir().unwrap().join("Generic DAW").into();
	_ = std::fs::create_dir(&data_dir);
	data_dir
});

pub static STATE_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let state_dir = dirs::state_dir()
		.or_else(dirs::data_dir)
		.unwrap()
		.join("Generic DAW")
		.into();
	_ = std::fs::create_dir(&state_dir);
	state_dir
});

pub static CRASHES_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let crashes_dir = DATA_DIR.join("crashes").into();
	_ = std::fs::create_dir(&crashes_dir);
	crashes_dir
});

pub static PROJECTS_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let projects_dir = DATA_DIR.join("projects").into();
	_ = std::fs::create_dir(&projects_dir);
	projects_dir
});

pub static AUTOSAVED_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let autosaved_dir = PROJECTS_DIR.join("autosaved").into();
	_ = std::fs::create_dir(&autosaved_dir);
	autosaved_dir
});

pub static FREEZES_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let freezes_dir = DATA_DIR.join("freezes").into();
	_ = std::fs::create_dir(&freezes_dir);
	freezes_dir
});

pub static RECORDINGS_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let recordings_dir = DATA_DIR.join("recordings").into();
	_ = std::fs::create_dir(&recordings_dir);
	recordings_dir
});

pub fn format_now() -> jiff::fmt::strtime::Display<'static> {
	jiff::Zoned::now().strftime("%F %H-%M-%S")
}

pub enum Instruction {
	Message(Message),
	Freeze(NodeId),
	PluginAdd(PluginId, Plugin, Receiver<MainThreadMessage>),
	PluginCopyState(PluginId, PluginId),
	PluginActivate(PluginId, Option<Box<clap_host::AudioThread>>),
	PluginParamChanged(PluginId, ClapId, f32),
}

#[derive(Clone, Debug)]
pub enum Message {
	Arrangement(Project, arrangement_view::Message),
	ClapHost(clap_host::Message),
	FileTree(file_tree::Message),
	ConfigView(config_view::Message),

	CloseRequested(window::Id),
	ProjectLoaded(
		Project,
		NoClone<NoDebug<Box<Arrangement>>>,
		NoClone<NoDebug<Box<AudioThread>>>,
		Option<proto::ViewState>,
	),

	ScanProgress(Scan, f32),
	ScanStatus(Scan, Option<Arc<str>>),
	PluginScanned(Scan, PluginDescriptor),
	ScanFinished(Scan),

	NewFile,
	OpenLastFile,
	SaveFile,
	SaveAsFileDialog,
	SaveAsFile(Arc<Path>),
	AutosaveFile,
	ToggleFullscreen,

	Progress(f32),
	Status(Option<Arc<str>>),

	OpenFileDialog,
	OpenFile(Arc<Path>),
	CantFindPlugin(Arc<CStr>, NoClone<oneshot::Sender<Feedback<Infallible>>>),
	CantFindSample(Arc<str>, NoClone<oneshot::Sender<Feedback<Arc<Path>>>>),
	FindPlugin(usize, Feedback<Infallible>),
	FindSampleFileDialog(usize),
	FindSampleFile(usize, Feedback<Arc<Path>>),
	OpenedFile(Option<Arc<Path>>),

	RenderFileDialog,
	RenderFile(Arc<Path>),
	RenderedFile,

	ToggleConfigView,
	LoadConfig(Box<Config>),

	RescanDevices,
	RescanPlugins,

	FileHovered,
	FileDropped(Arc<Path>),
	FileHoveredLeft,

	TogglePlayback,
	Stop,
	ToggleShowSeconds,
	ToggleMetronome,
	ToggleAutoscroll,
	TappedBpm,
	ChangedBpm(Option<u16>),
	ChangedNumerator(Option<u8>),
	ChangedGridSize(f32),
	NarrowedGrid,
	WidenedGrid,
	ToggleGridTriplets,
	ToggleGrid,

	CycleForwards,
	CycleBackwards,
	TopPane(Tab),
	BottomPane(Tab),
	BottomSelected(bool),
	MovePaneUp,
	MovePaneDown,

	OnFileTreeDrag(f32),
	OnBottomPaneDrag(f32),
	OnBottomPaneDragEnd,
	OnFileTreeDragEnd,
	OnFileTreeDoubleClick,
	OnBottomPaneDoubleClick,
}

const _: () = assert!(size_of::<Message>() == 72);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Tab {
	Playlist,
	Mixer,
	PianoRoll,
}

impl Tab {
	fn tab<'a>(self, top_pane: Self, bottom_pane: Option<Self>) -> Element<'a, Message> {
		ContextMenu::new(
			button(match self {
				Self::Playlist => chart_no_axes_gantt(),
				Self::Mixer => sliders_vertical(),
				Self::PianoRoll => keyboard_music(),
			})
			.style(button_with_radius(
				if bottom_pane == Some(self) {
					button::secondary
				} else {
					button::primary
				},
				match self {
					Self::Playlist => border::left(5),
					Self::Mixer => border::radius(0),
					Self::PianoRoll => border::right(5),
				},
			))
			.padding(padding::horizontal(7).vertical(5))
			.on_press_maybe(
				(top_pane != self && bottom_pane != Some(self)).then_some(Message::TopPane(self)),
			),
			move || {
				container(
					menu_entry(panel_bottom_dashed(), "Detach", "").on_press_maybe(
						(bottom_pane != Some(self)).then_some(Message::BottomPane(self)),
					),
				)
				.width(160)
				.style(container_with_radius(weaker_bordered_box, 5))
				.into()
			},
		)
		.into()
	}

	fn radius(self) -> border::Radius {
		match self {
			Self::Playlist | Self::PianoRoll => border::left(5),
			Self::Mixer => border::top(5),
		}
	}
}

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
	bpm_tapper: BpmTapper,

	progress: Option<f32>,
	status: Option<Arc<str>>,
	missing_plugins: Vec<(Arc<CStr>, oneshot::Sender<Feedback<Infallible>>)>,
	missing_samples: Vec<(Arc<str>, oneshot::Sender<Feedback<Arc<Path>>>)>,

	scan: Option<Scan>,
	scan_progress: Option<f32>,
	scan_status: Option<Arc<str>>,

	top_pane: Tab,
	bottom_pane: Option<Tab>,
	bottom_selected: bool,

	main_window_id: window::Id,
	project: Project,
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

		let (sender, receiver) = oneshot::channel();
		let (streams, input_channels, output_channels, sample_rate, frames) = build_streams(
			&config.audio.devices.as_core(),
			config.midi.input.as_deref(),
			config.midi.output.as_deref(),
			config.audio.sample_rate,
			config.audio.buffer_size,
			PullSlot::Empty(receiver),
		);

		let project = Project::unique();
		let (mut arrangement, processor, batches) =
			Arrangement::create(input_channels, output_channels, sample_rate, frames);
		let pool = processor.create_pool();
		sender.send((PullSlot::Full(processor), pool)).unwrap();
		arrangement.replace_streams(Some(streams));

		let arrangement_view = ArrangementView::new(arrangement, &state, None);
		let clap_host = ClapHost::new(main_window_id);
		let file_tree = FileTree::new(&config.sample_paths);
		let bottom_pane = state.bottom_pane_split_at != 0.0;

		let open = if config.open_last_project {
			Task::done(Message::OpenLastFile)
		} else {
			Task::none()
		};

		let mut this = Self {
			config,
			state,
			current_project: None,

			arrangement_view,
			clap_host,
			file_tree,
			config_view: None,

			plugins: combo_box::State::default(),
			bpm_tapper: BpmTapper::default(),

			progress: None,
			status: None,
			missing_plugins: Vec::new(),
			missing_samples: Vec::new(),

			scan: None,
			scan_progress: None,
			scan_status: None,

			top_pane: Tab::Playlist,
			bottom_pane: bottom_pane.then_some(Tab::Mixer),
			bottom_selected: false,

			main_window_id,
			project,
			files_hovered: false,
		};

		let scan = this.update(Message::RescanPlugins);

		(
			this,
			Task::batch([
				window,
				batches
					.map(NoClone)
					.map(arrangement_view::Message::Batch)
					.map(move |message| Message::Arrangement(project, message)),
				scan.chain(open),
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
				return self
					.clap_host
					.update(message, self.arrangement_view.arrangement.transport())
					.handle(Message::ClapHost, |instruction| {
						self.handle_instruction(instruction)
					});
			}
			Message::FileTree(message) => return self.handle_file_tree_message(message),
			Message::ConfigView(message) => {
				if let Some(config_view) = self.config_view.as_mut() {
					return config_view
						.update(message, &self.config)
						.handle(Message::ConfigView, |config| {
							self.update(Message::LoadConfig(config.into()))
						});
				}
			}
			Message::CloseRequested(window) => {
				if window == self.main_window_id {
					return iced::exit();
				}
			}
			Message::ProjectLoaded(
				project,
				NoClone(NoDebug(mut arrangement)),
				NoClone(NoDebug(processor)),
				view,
			) => {
				self.top_pane = Tab::Playlist;
				self.bottom_pane = self.bottom_pane.map(|_| Tab::Mixer);
				self.project = project;

				arrangement
					.replace_streams(self.arrangement_view.arrangement.replace_streams(None));
				let mut arrangement = std::mem::replace(
					&mut self.arrangement_view,
					ArrangementView::new(*arrangement, &self.state, view),
				)
				.arrangement;

				let p_receiver = arrangement.request_processor(PullSlot::Full(*processor));

				return Task::future(unblock(|| {
					while !arrangement.drain_queue() {
						std::thread::yield_now();
					}
					drop(arrangement);
					drop(p_receiver.recv().unwrap());
				}))
				.discard();
			}
			Message::ScanProgress(scan, progress) => {
				if self.scan == Some(scan) {
					self.scan_progress = Some(progress);
				}
			}
			Message::ScanStatus(scan, status) => {
				if self.scan == Some(scan) {
					self.scan_status = status;
				}
			}
			Message::PluginScanned(scan, descriptor) => {
				if self.scan == Some(scan)
					&& let Err(i) = self.plugins.options().binary_search_by(|d| {
						natural_cmp(d.name.as_bytes(), descriptor.name.as_bytes())
					}) {
					self.plugins.insert(i, descriptor);
				}
			}
			Message::ScanFinished(scan) => {
				if self.scan == Some(scan) {
					self.scan = None;
					self.scan_progress = None;
					self.scan_status = None;
				}
			}
			Message::NewFile => {
				return Arrangement::empty(
					self.arrangement_view.arrangement.transport().input_channels,
					self.arrangement_view
						.arrangement
						.transport()
						.output_channels,
					self.arrangement_view.arrangement.transport().sample_rate,
					self.arrangement_view.arrangement.transport().frames,
				);
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
			Message::SaveAsFileDialog => {
				let current_project = self.current_project.clone();
				return window::run(self.main_window_id, |window| {
					let mut dialog = AsyncFileDialog::new();
					if let Some(current_project) = current_project
						&& let Some(current_project) = current_project.file_name()
						&& let Some(current_project) = current_project.to_str()
					{
						dialog = dialog.set_file_name(current_project);
					}
					dialog
						.set_parent(window)
						.add_filter("Generic DAW project file", &["gdp"])
						.set_directory(&*PROJECTS_DIR)
						.save_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().with_extension("gdp").into())
				.map(Message::SaveAsFile);
			}
			Message::SaveAsFile(path) => {
				match std::fs::write(&path, self.arrangement_view.save(&mut self.clap_host)) {
					Ok(()) => return self.update(Message::OpenedFile(Some(path))),
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

				let path = AUTOSAVED_DIR.join(format!("{} {}.gdp", name, format_now()));

				if let Err(err) =
					std::fs::write(&path, self.arrangement_view.save(&mut self.clap_host))
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
			Message::ChangedGridSize(size) => {
				self.state.grid.size = size.clamp(0.5, 4.5);
				self.state.write();
			}
			Message::NarrowedGrid => {
				return self.update(Message::ChangedGridSize(self.state.grid.size - 1.0));
			}
			Message::WidenedGrid => {
				return self.update(Message::ChangedGridSize(self.state.grid.size + 1.0));
			}
			Message::ToggleGridTriplets => {
				self.state.grid.triplets ^= true;
				self.state.write();
			}
			Message::ToggleGrid => {
				self.state.grid.enabled ^= true;
				self.state.write();
			}
			Message::Progress(progress) => self.progress = Some(progress),
			Message::Status(status) => self.status = status,
			Message::OpenFileDialog => {
				return window::run(self.main_window_id, |window| {
					AsyncFileDialog::new()
						.set_parent(window)
						.add_filter("Generic DAW project file", &["gdp"])
						.set_directory(&*PROJECTS_DIR)
						.pick_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().into())
				.map(Message::OpenFile);
			}
			Message::OpenFile(path) => {
				if self.progress.is_none() {
					self.progress = Some(0.0);
					return Arrangement::start_load(
						path,
						self.arrangement_view.arrangement.transport().input_channels,
						self.arrangement_view
							.arrangement
							.transport()
							.output_channels,
						self.arrangement_view.arrangement.transport().sample_rate,
						self.arrangement_view.arrangement.transport().frames,
						self.config.clone(),
						self.plugins.clone().into_options(),
					);
				}
			}
			Message::CantFindPlugin(name, NoClone(sender)) => {
				if self.progress.is_some() {
					self.missing_plugins.push((name, sender));
				}
			}
			Message::CantFindSample(name, NoClone(sender)) => {
				if self.progress.is_some() {
					self.missing_samples.push((name, sender));
				}
			}
			Message::FindPlugin(index, response) => {
				self.missing_plugins.remove(index).1.send(response).unwrap();
			}
			Message::FindSampleFileDialog(index) => {
				return window::run(self.main_window_id, |window| {
					AsyncFileDialog::new().set_parent(window).pick_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().into())
				.map(move |response| Message::FindSampleFile(index, Feedback::Use(response)));
			}
			Message::FindSampleFile(index, response) => {
				self.missing_samples.remove(index).1.send(response).unwrap();
			}
			Message::OpenedFile(path) => {
				if let Some(path) = path {
					self.current_project = Some(path.clone());
					self.state.last_project = Some(path);
					self.state.write();
				}
				self.progress = None;
				self.status = None;
				self.missing_plugins.clear();
				self.missing_samples.clear();
			}
			Message::RenderFileDialog => {
				let current_project = self.current_project.clone();
				return window::run(self.main_window_id, |window| {
					let mut dialog = AsyncFileDialog::new();
					if let Some(current_project) = current_project
						&& let Some(current_project) = current_project.file_prefix()
						&& let Some(current_project) = current_project.to_str()
					{
						dialog = dialog.set_file_name(format!("{current_project}.wav"));
					}
					dialog
						.set_parent(window)
						.add_filter("Wave file", &["wav"])
						.set_directory(&*PROJECTS_DIR)
						.save_file()
				})
				.then(Task::future)
				.and_then(Task::done)
				.map(|p| p.path().with_extension("wav").into())
				.map(Message::RenderFile);
			}
			Message::RenderFile(path) => {
				if self.progress.is_none() {
					self.progress = Some(0.0);
					self.clap_host.set_render_mode(RenderMode::Offline);
					return self.arrangement_view.arrangement.render(&path);
				}
			}
			Message::RenderedFile => {
				self.clap_host.set_render_mode(RenderMode::Realtime);
				self.progress = None;
			}
			Message::ToggleConfigView => {
				self.config_view = if self.config_view.is_some() {
					None
				} else {
					Some(ConfigView::new(self.main_window_id, &self.config))
				};
			}
			Message::LoadConfig(config) => {
				let mut fut = if self.config.clap_paths == config.clap_paths {
					Task::none()
				} else {
					self.update(Message::RescanPlugins)
				};

				if self.config.sample_paths != config.sample_paths {
					self.file_tree.diff(&config.sample_paths);
				}

				if self.config.audio != config.audio || self.config.midi != config.midi {
					fut = Task::batch([fut, self.update(Message::RescanDevices)]);
				}

				self.config = *config;
				self.config.write();

				return fut;
			}
			Message::RescanDevices => {
				let project = self.project;
				return self
					.arrangement_view
					.change_config()
					.map(move |message| Message::Arrangement(project, message));
			}
			Message::RescanPlugins => {
				let scan = Scan::unique();
				self.plugins = combo_box::State::default();
				self.scan = Some(scan);
				self.scan_progress = Some(0.0);
				self.scan_status = None;

				let (sender, receiver) = smol::channel::unbounded();
				let clap_paths = self.config.clap_paths.clone();

				return Task::batch([
					Task::future(unblock(move || {
						let plugin_paths = clap_host::find_plugin_paths(
							DEFAULT_CLAP_PATHS.iter().chain(&clap_paths),
						)
						.collect::<Box<_>>();

						let len = plugin_paths.len();
						for (i, path) in plugin_paths.into_iter().enumerate() {
							sender
								.try_send(Message::ScanStatus(
									scan,
									path.file_name()
										.map(|name| name.display().to_string().into()),
								))
								.unwrap();

							if let Some(descriptors) = Plugin::descriptors(&path) {
								for descriptor in descriptors {
									sender
										.try_send(Message::PluginScanned(scan, descriptor))
										.unwrap();
								}
							}

							sender
								.try_send(Message::ScanProgress(scan, (i + 1) as f32 / len as f32))
								.unwrap();
						}

						sender.try_send(Message::ScanFinished(scan)).unwrap();
					}))
					.discard(),
					Task::stream(receiver),
				]);
			}
			Message::FileHovered => self.files_hovered = true,
			Message::FileDropped(path) => {
				self.files_hovered = false;
				if self.state.file_tree_split_at != 0.0
					&& path.metadata().is_ok_and(|metadata| metadata.is_dir())
				{
					self.config.sample_paths.push(path);
					self.config.write();
					self.file_tree.diff(&self.config.sample_paths);
				}
			}
			Message::FileHoveredLeft => self.files_hovered = false,
			Message::TogglePlayback => {
				self.arrangement_view.arrangement.toggle_playback();
			}
			Message::Stop => {
				let before = self.arrangement_view.arrangement.transport().position;
				self.arrangement_view.arrangement.stop();
				let after = self.arrangement_view.arrangement.transport().position;
				self.arrangement_view
					.autoscroll(before, after, &self.config, &mut self.state);
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
			Message::TappedBpm => {
				self.bpm_tapper.tap();
				return self.update(Message::ChangedBpm(
					self.bpm_tapper.get_bpm().map(NonZero::get),
				));
			}
			Message::ChangedBpm(bpm) => {
				if let Some(bpm) = bpm {
					self.arrangement_view
						.arrangement
						.set_bpm(NonZero::new(bpm.clamp(10, 999)).unwrap());
				}
			}
			Message::ChangedNumerator(numerator) => {
				if let Some(numerator) = numerator {
					self.arrangement_view
						.arrangement
						.set_numerator(NonZero::new(numerator.clamp(1, 99)).unwrap());
				}
			}
			Message::CycleForwards => {
				let (message, this, other): (fn(_) -> _, _, _) = if let Some(bottom_pane) =
					self.bottom_pane
					&& self.bottom_selected
				{
					(Message::BottomPane, bottom_pane, Some(self.top_pane))
				} else {
					(Message::TopPane, self.top_pane, self.bottom_pane)
				};

				return self.update(message(match this {
					Tab::Playlist if other != Some(Tab::Mixer) => Tab::Mixer,
					Tab::Playlist | Tab::Mixer if other != Some(Tab::PianoRoll) => Tab::PianoRoll,
					Tab::Mixer | Tab::PianoRoll if other != Some(Tab::Playlist) => Tab::Playlist,
					Tab::PianoRoll if other != Some(Tab::Mixer) => Tab::Mixer,
					_ => this,
				}));
			}
			Message::CycleBackwards => {
				let (message, this, other): (fn(_) -> _, _, _) = if let Some(bottom_pane) =
					self.bottom_pane
					&& self.bottom_selected
				{
					(Message::BottomPane, bottom_pane, Some(self.top_pane))
				} else {
					(Message::TopPane, self.top_pane, self.bottom_pane)
				};

				return self.update(message(match this {
					Tab::PianoRoll if other != Some(Tab::Mixer) => Tab::Mixer,
					Tab::Mixer | Tab::PianoRoll if other != Some(Tab::Playlist) => Tab::Playlist,
					Tab::Playlist | Tab::Mixer if other != Some(Tab::PianoRoll) => Tab::PianoRoll,
					Tab::Playlist if other != Some(Tab::Mixer) => Tab::Mixer,
					_ => this,
				}));
			}
			Message::TopPane(top_pane) => {
				if self.bottom_pane == Some(top_pane) {
					return self.update(Message::BottomSelected(true));
				} else if self.top_pane != top_pane {
					self.arrangement_view.finish(self.top_pane);
					self.top_pane = top_pane;
				}
			}
			Message::BottomPane(bottom_pane) => {
				if self.state.bottom_pane_split_at == 0.0 {
					self.state.bottom_pane_split_at = DEFAULT_BOTTOM_PANE_POSITON;
					self.state.write();
				}

				let fut = if self.top_pane == bottom_pane {
					if let Some(bottom_pane) = self.bottom_pane {
						self.top_pane = bottom_pane;
						self.bottom_selected ^= true;
						Task::none()
					} else {
						self.bottom_selected = true;
						self.update(Message::CycleForwards)
					}
				} else {
					self.update(Message::BottomSelected(true))
				};

				self.bottom_pane = Some(bottom_pane);

				return fut;
			}
			Message::BottomSelected(bottom_selected) => {
				if self.bottom_selected != bottom_selected
					&& let Some(bottom_pane) = self.bottom_pane
				{
					self.bottom_selected = bottom_selected;
					self.arrangement_view.unselect_all(if self.bottom_selected {
						self.top_pane
					} else {
						bottom_pane
					});
				}
			}
			Message::MovePaneUp => {
				return if self.bottom_pane.is_none() {
					Task::batch([
						self.update(Message::BottomPane(self.top_pane)),
						self.update(Message::BottomPane(self.top_pane)),
					])
				} else if self.bottom_selected {
					self.update(Message::BottomPane(self.top_pane))
				} else {
					Task::batch([
						self.update(Message::OnBottomPaneDrag(0.0)),
						self.update(Message::OnBottomPaneDragEnd),
					])
				};
			}
			Message::MovePaneDown => {
				return if self.bottom_pane.is_none() || !self.bottom_selected {
					self.update(Message::BottomPane(self.top_pane))
				} else {
					Task::batch([
						self.update(Message::BottomPane(self.top_pane)),
						self.update(Message::MovePaneUp),
					])
				};
			}
			Message::OnFileTreeDrag(split_at) => {
				self.state.file_tree_split_at = if split_at >= 20.0 {
					split_at.clamp(200.0, 1000.0)
				} else {
					0.0
				};
			}
			Message::OnBottomPaneDrag(split_at) => {
				self.state.bottom_pane_split_at = if split_at >= 30.0 {
					split_at.clamp(300.0, 1000.0)
				} else {
					0.0
				};
			}
			Message::OnBottomPaneDragEnd => {
				if self.state.bottom_pane_split_at == 0.0 {
					self.bottom_pane = None;
				}
				self.state.write();
			}
			Message::OnFileTreeDragEnd => self.state.write(),
			Message::OnFileTreeDoubleClick => {
				return Task::batch([
					self.update(Message::OnFileTreeDrag(DEFAULT_SPLIT_POSITION)),
					self.update(Message::OnFileTreeDragEnd),
				]);
			}
			Message::OnBottomPaneDoubleClick => {
				return Task::batch([
					self.update(Message::OnBottomPaneDrag(DEFAULT_BOTTOM_PANE_POSITON)),
					self.update(Message::OnFileTreeDragEnd),
				]);
			}
		}

		Task::none()
	}

	fn handle_instruction(&mut self, instruction: Instruction) -> Task<Message> {
		match instruction {
			Instruction::Message(message) => return self.update(message),
			Instruction::Freeze(node) => {
				self.progress = Some(0.0);
				self.clap_host.set_render_mode(RenderMode::Offline);
				return self.arrangement_view.arrangement.freeze(node, self.project);
			}
			Instruction::PluginAdd(id, plugin, receiver) => {
				return self
					.clap_host
					.plugin_add(id, plugin, receiver)
					.map(Message::ClapHost);
			}
			Instruction::PluginCopyState(from, to) => {
				if let Some(state) = self
					.clap_host
					.get_state(from, StateContextType::ForDuplicate)
					.map(Box::from)
				{
					return self.update(Message::ClapHost(clap_host::Message::SetState(
						to,
						NoDebug(state),
					)));
				}
			}
			Instruction::PluginActivate(id, processor) => {
				if let Some((node, index)) = self.arrangement_view.arrangement.plugin_of(id) {
					self.arrangement_view.arrangement.plugin_activate(
						node,
						index,
						processor.map(|processor| *processor),
					);
				}
			}
			Instruction::PluginParamChanged(id, param_id, value) => {
				if let Some((node, index)) = self.arrangement_view.arrangement.plugin_of(id) {
					self.arrangement_view
						.arrangement
						.plugin_param_changed(node, index, param_id, value);
				}
			}
		}

		Task::none()
	}

	fn handle_file_tree_message(&mut self, message: file_tree::Message) -> Task<Message> {
		match message {
			file_tree::Message::Action(id, action) => {
				return self
					.file_tree
					.update(id, action)
					.unwrap_or_default()
					.map(Message::FileTree);
			}
			file_tree::Message::DragFile(file, kind) => {
				if kind != FileKind::Project {
					self.arrangement_view.hover_file(file, kind);
				}
			}
			file_tree::Message::OpenFile(file, kind) => {
				if kind == FileKind::Project {
					return self.update(Message::OpenFile(file));
				}
			}
			file_tree::Message::OpenDir(dir) => {
				if let Err(err) = open::that_detached(&*dir) {
					warn!("{err}");
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
		let now_beats = transport.position.to_beat_time(transport);

		stack![
			column![
				row![
					Menu::new(menu(), || container(column![
						menu_entry(None, "New", "Ctrl+N").on_press(Message::NewFile),
						menu_entry(None, "Open", "Ctrl+O").on_press(Message::OpenFileDialog),
						menu_entry(None, "Open last", "Ctrl+Shift+O")
							.on_press(Message::OpenLastFile),
						menu_entry(None, "Save", "Ctrl+S").on_press(Message::SaveFile),
						menu_entry(None, "Save as", "Ctrl+Shift+S")
							.on_press(Message::SaveAsFileDialog),
						menu_entry(None, "Render", "Ctrl+R").on_press(Message::RenderFileDialog),
						rule::horizontal(1),
						menu_entry(None, "Reconnect devices", "").on_press(Message::RescanDevices),
						menu_entry(None, "Rescan plugins", "").on_press(Message::RescanPlugins),
						menu_entry(None, "Settings", "Ctrl+,").on_press(Message::ToggleConfigView),
					])
					.width(200)
					.style(container_with_radius(weaker_bordered_box, 5))
					.into())
					.style(button_with_radius(button::background, 5))
					.padding(padding::horizontal(7).vertical(5)),
					rule::vertical(1),
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
					number_input(1..=99, transport.numerator.get().into(), 4, 5).map(|numerator| {
						Message::ChangedNumerator(numerator.map(|numerator| numerator as u8))
					}),
					row![
						number_input(10..=999, transport.bpm.get().into(), 140, border::left(5))
							.map(|bpm| Message::ChangedBpm(bpm.map(|bpm| bpm as u16))),
						button(metronome())
							.style(button_with_radius(
								if self.state.metronome {
									button::primary
								} else {
									button::secondary
								},
								0
							))
							.padding(padding::all(5).left(4))
							.on_press(Message::ToggleMetronome),
						button(
							mouse_area(container(gavel()).padding(5)).on_press(Message::TappedBpm)
						)
						.style(button_with_radius(button::primary, border::right(5)))
						.padding(0)
						.on_press_with(|| unreachable!()),
					],
					row![
						mouse_area(
							container(
								if self.state.show_seconds {
									text!(
										"{:02}:{:02}:{:03.0}",
										transport.position.second() / 60,
										transport.position.second() % 60,
										(transport.position.to_float().fract() * 1000.0)
									)
								} else {
									text!(
										"{:03}:{:0digits$}",
										now_beats.bar(transport) + 1,
										now_beats.beat_in_bar(transport) + 1,
										digits = transport.numerator.ilog10() as usize + 1,
									)
								}
								.font(Font::MONOSPACE)
							)
							.padding(padding::horizontal(7).vertical(5))
							.style(container_with_radius(weakest_bordered_box, border::left(5)))
						)
						.on_press(Message::ToggleShowSeconds)
						.interaction(Interaction::Pointer),
						button(arrow_big_right())
							.style(button_with_radius(
								if self.state.autoscroll {
									button::primary
								} else {
									button::secondary
								},
								border::right(5)
							))
							.padding(5)
							.on_press(Message::ToggleAutoscroll),
					],
					Menu::new(magnet(), || container(
						column![
							checkbox(self.state.grid.enabled)
								.label("Grid enabled")
								.on_toggle(|_| Message::ToggleGrid)
								.style(if self.state.grid.enabled {
									checkbox::primary
								} else {
									checkbox::secondary
								}),
							checkbox(self.state.grid.triplets)
								.label("Triplet grid")
								.on_toggle(|_| Message::ToggleGridTriplets)
								.style(if self.state.grid.enabled {
									checkbox::primary
								} else {
									checkbox::secondary
								}),
							slider(0.5..=4.5, self.state.grid.size, Message::ChangedGridSize)
								.style(if self.state.grid.enabled {
									slider::primary
								} else {
									slider::secondary
								})
						]
						.width(Shrink)
						.spacing(5)
					)
					.padding(5)
					.style(container_with_radius(weaker_bordered_box, 5))
					.into())
					.padding(5)
					.style(button_with_radius(
						if self.state.grid.enabled {
							button::primary
						} else {
							button::secondary
						},
						5
					)),
					right(self.scan_progress.map(|progress| {
						column![
							self.scan_status
								.as_deref()
								.map(|status| text!("scanning {}", status)
									.size(13)
									.wrapping(text::Wrapping::None)
									.ellipsis(text::Ellipsis::End)),
							progress_bar(0.0..=1.0, progress).girth(4).style(
								progress_bar_with_radius(progress_bar::secondary, f32::INFINITY)
							)
						]
						.spacing(5)
						.width(Fill.max(200))
					})),
					row![
						cpu(),
						text!("{:.1}%", self.arrangement_view.arrangement.load() * 100.0)
							.font(Font::MONOSPACE)
					]
					.spacing(5),
					row![
						Tab::Playlist.tab(self.top_pane, self.bottom_pane),
						Tab::Mixer.tab(self.top_pane, self.bottom_pane),
						Tab::PianoRoll.tab(self.top_pane, self.bottom_pane),
					],
				]
				.height(Shrink)
				.align_y(Center)
				.spacing(10),
				vertical_split(
					stack![
						self.file_tree.view().map(Message::FileTree),
						self.files_hovered.then(|| center(plus().size(40.0))
							.style(|_| container::background(Color::BLACK.scale_alpha(ALPHA_2_3))))
					],
					self.bottom_pane.map_or_else(
						|| self
							.arrangement_view
							.view(self.top_pane, &self.state, &self.plugins)
							.map(|message| Message::Arrangement(self.project, message)),
						|bottom_pane| horizontal_split(
							SelectArea::new(
								container(
									self.arrangement_view
										.view(self.top_pane, &self.state, &self.plugins)
										.map(|message| Message::Arrangement(self.project, message))
								)
								.padding(5)
								.style(container_with_radius(
									selectable_box(container::transparent, !self.bottom_selected),
									self.top_pane.radius()
								))
							)
							.on_select_maybe(
								self.bottom_selected
									.then_some(Message::BottomSelected(false))
							),
							SelectArea::new(
								container(
									self.arrangement_view
										.view(bottom_pane, &self.state, &self.plugins)
										.map(|message| Message::Arrangement(self.project, message))
								)
								.padding(5)
								.style(container_with_radius(
									selectable_box(container::transparent, self.bottom_selected),
									bottom_pane.radius()
								))
							)
							.on_select_maybe(
								(!self.bottom_selected).then_some(Message::BottomSelected(true))
							),
							self.state.bottom_pane_split_at,
							Message::OnBottomPaneDrag,
						)
						.on_drag_end(Message::OnBottomPaneDragEnd)
						.on_double_click(Message::OnBottomPaneDoubleClick)
						.strategy(Strategy::End)
						.focus_delay(Duration::ZERO)
						.style(split_style)
						.into()
					),
					self.state.file_tree_split_at,
					Message::OnFileTreeDrag
				)
				.on_drag_end(Message::OnFileTreeDragEnd)
				.on_double_click(Message::OnFileTreeDoubleClick)
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
				.on_press(Message::ToggleConfigView),
			)),
			self.progress.map(|progress| mouse_area(
				container(
					column![
						bottom_center(self.status.as_deref().map(|status| {
							container(
								row![
									"scanning",
									container(
										text(status)
											.font(Font::MONOSPACE)
											.wrapping(text::Wrapping::None)
											.ellipsis(text::Ellipsis::Middle)
									)
									.padding(padding::horizontal(10).vertical(5))
									.style(container_with_radius(weakest_bordered_box, 5))
								]
								.align_y(Center)
								.spacing(10),
							)
							.padding(10)
							.style(container_with_radius(weak_bordered_box, 5))
						})),
						column![
							progress_bar(0.0..=1.0, progress).style(progress_bar_with_radius(
								if self.missing_plugins.is_empty()
									&& self.missing_samples.is_empty()
								{
									progress_bar::primary
								} else {
									progress_bar::danger
								},
								5
							)),
							scrollable(
								column(
									self.missing_plugins
										.iter()
										.map(|(name, _)| &**name)
										.enumerate()
										.map(|(i, name)| {
											container(
												row![
													"can't find plugin",
													container(
														text(name.to_string_lossy())
															.font(Font::MONOSPACE)
															.wrapping(text::Wrapping::None)
															.ellipsis(text::Ellipsis::Middle)
													)
													.padding(padding::horizontal(10).vertical(5))
													.style(container_with_radius(
														weakest_bordered_box,
														5
													)),
													row![
														button("Ignore")
															.on_press(Message::FindPlugin(
																i,
																Feedback::Ignore
															))
															.style(button_with_radius(
																button::warning,
																border::left(5)
															)),
														button("Cancel")
															.on_press(Message::FindPlugin(
																i,
																Feedback::Cancel
															))
															.style(button_with_radius(
																button::danger,
																border::right(5)
															))
													]
												]
												.align_y(Center)
												.spacing(10),
											)
											.padding(10)
											.style(container_with_radius(weak_bordered_box, 5))
											.into()
										})
										.chain(
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
																	.ellipsis(
																		text::Ellipsis::Middle
																	)
															)
															.padding(
																padding::horizontal(10).vertical(5)
															)
															.style(
																container_with_radius(
																	weakest_bordered_box,
																	5
																)
															),
															row![
																button("Pick")
																	.on_press(
																		Message::FindSampleFileDialog(i)
																	)
																	.style(button_with_radius(
																		button::success,
																		border::left(5)
																	)),
																button("Ignore")
																	.on_press(Message::FindSampleFile(
																		i,
																		Feedback::Ignore
																	))
																	.style(button_with_radius(
																		button::warning,
																		0
																	)),
																button("Cancel")
																	.on_press(Message::FindSampleFile(
																		i,
																		Feedback::Cancel
																	))
																	.style(button_with_radius(
																		button::danger,
																		border::right(5)
																	))
															]
														]
														.align_y(Center)
														.spacing(10),
													)
													.padding(10)
													.style(container_with_radius(
														weak_bordered_box,
														5,
													))
													.into()
												})
										),
								)
								.align_x(Center)
								.spacing(10)
							)
							.spacing(10)
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
			.or_else(|| {
				self.current_project
					.as_deref()
					.and_then(|current_project| current_project.file_prefix())
					.and_then(|current_project| current_project.to_str())
					.map(|current_project| format!("{current_project} - Generic DAW"))
			})
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
		Subscription::batch([
			self.arrangement_view
				.subscription()
				.with(self.project)
				.map(|(project, message)| Message::Arrangement(project, message)),
			self.clap_host.subscription().map(Message::ClapHost),
			if self.config.autosave.enabled {
				every(Duration::from_secs(
					self.config.autosave.interval.get().into(),
				))
				.map(|_| Message::AutosaveFile)
			} else {
				Subscription::none()
			},
			if self.progress.is_some() {
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
					.with((
						self.project,
						self.bottom_pane
							.filter(|_| self.bottom_selected)
							.unwrap_or(self.top_pane),
					))
					.filter_map(|((project, tab), event)| match event {
						keyboard::Event::KeyPressed {
							key,
							physical_key,
							modifiers,
							repeat,
							..
						} => ArrangementView::keybinds(&key, physical_key, modifiers, repeat)
							.map(|message| Message::Arrangement(project, message(tab)))
							.or_else(|| Self::keybinds(&key, physical_key, modifiers, repeat)),
						_ => None,
					})
			},
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
				keyboard::Key::Named(keyboard::key::Named::Tab) => Some(Message::CycleForwards),
				_ => None,
			},
			(true, false, false, false) => match key.to_latin(physical_key) {
				Some(',') => Some(Message::ToggleConfigView),
				Some('m') => Some(Message::ToggleMetronome),
				Some('n') => Some(Message::NewFile),
				Some('o') => Some(Message::OpenFileDialog),
				Some('r') => Some(Message::RenderFileDialog),
				Some('s') => Some(Message::SaveFile),
				Some('1') => Some(Message::NarrowedGrid),
				Some('2') => Some(Message::WidenedGrid),
				Some('3') => Some(Message::ToggleGridTriplets),
				Some('4') => Some(Message::ToggleGrid),
				_ => None,
			},
			(false, true, false, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::Space) => Some(Message::Stop),
				keyboard::Key::Named(keyboard::key::Named::Tab) => Some(Message::CycleBackwards),

				_ => None,
			},
			(true, true, false, false) => match key.to_latin(physical_key) {
				Some('o') => Some(Message::OpenLastFile),
				Some('s') => Some(Message::SaveAsFileDialog),
				_ => None,
			},
			(false, false, true, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
					Some(Message::BottomSelected(false))
				}
				keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
					Some(Message::BottomSelected(true))
				}
				_ => None,
			},
			(false, true, true, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(Message::MovePaneUp),
				keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
					Some(Message::MovePaneDown)
				}
				_ => None,
			},
			_ => None,
		}
	}
}
