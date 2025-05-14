use crate::{
    arrangement_view::{ArrangementView, Message as ArrangementMessage, Tab},
    components::{empty_widget, modal, number_input, styled_button, styled_pick_list},
    config::Config,
    config_view::{ConfigView, Message as ConfigViewMessage},
    file_tree::{FileTree, Message as FileTreeMessage},
    icons::{chart_no_axes_gantt, pause, play, sliders_vertical, square},
    state::State,
    stylefns::button_with_base,
    widget::{AnimatedDot, LINE_HEIGHT, VSplit, vsplit},
};
use generic_daw_core::{
    Meter, Position,
    clap_host::{PluginBundle, PluginDescriptor, get_installed_plugins},
    get_input_devices, get_output_devices,
};
use iced::{
    Alignment, Element, Event, Fill, Subscription, Task, Theme,
    event::{self, Status},
    keyboard,
    mouse::Interaction,
    time::every,
    widget::{button, column, container, horizontal_space, mouse_area, row, stack},
    window::{self, Id, frames},
};
use log::trace;
use rfd::AsyncFileDialog;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Arc,
        atomic::Ordering::{AcqRel, Acquire, Release},
    },
    time::Duration,
};

#[derive(Clone, Debug)]
pub enum Message {
    Redraw,

    Arrangement(ArrangementMessage),
    FileTree(FileTreeMessage),
    ConfigView(ConfigViewMessage),

    NewFile,
    OpenFileDialog,
    OpenLastFile,
    SaveFile,
    SaveAsFileDialog,
    ExportFileDialog,

    OpenFile(Arc<Path>),
    SaveAsFile(Arc<Path>),

    OpenConfigView,
    CloseConfigView,

    Stop,
    TogglePlay,
    ToggleMetronome,
    ChangedBpm(u16),
    ChangedBpmText(String),
    ChangedNumerator(u8),
    ChangedNumeratorText(String),
    ChangedTab(Tab),

    SplitAt(f32),
}

pub struct Daw {
    config: Config,
    plugin_bundles: BTreeMap<PluginDescriptor, PluginBundle>,
    input_devices: Vec<String>,
    output_devices: Vec<String>,

    arrangement: ArrangementView,
    file_tree: FileTree,
    config_view: Option<ConfigView>,
    state: State,
    split_at: f32,
    meter: Arc<Meter>,
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

        let config = Config::read().unwrap_or_default();
        trace!("loaded config {config:?}");

        let mut state = State::read().unwrap_or_default();
        if !config.open_last_project {
            state.last_project = None;
        }
        trace!("loaded state {state:?}");

        let plugin_bundles = get_installed_plugins(&config.clap_paths);
        let file_tree = FileTree::new(&config.sample_paths);

        let mut input_devices = get_input_devices();
        input_devices.sort_unstable();

        let mut output_devices = get_output_devices();
        output_devices.sort_unstable();

        let (mut arrangement, mut meter) = ArrangementView::create(&config, &plugin_bundles);

        if let Some((new_meter, futs)) = state
            .last_project
            .as_deref()
            .and_then(|path| arrangement.load(path, &config, &plugin_bundles))
        {
            open = open.chain(futs.map(Message::Arrangement));
            meter = new_meter;
        }

        (
            Self {
                config,
                plugin_bundles,
                input_devices,
                output_devices,

                arrangement,
                file_tree,
                config_view: None,
                state,
                split_at: 300.0,
                meter,
            },
            open,
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        trace!("{message:?}");

        match message {
            Message::Redraw => {}
            Message::Arrangement(message) => {
                return self
                    .arrangement
                    .update(message, &self.config, &self.plugin_bundles)
                    .map(Message::Arrangement);
            }
            Message::FileTree(action) => return self.handle_file_tree_action(action),
            Message::ConfigView(message) => {
                if let Some(config_view) = self.config_view.as_mut() {
                    return config_view.update(message).map(Message::ConfigView);
                }
            }
            Message::NewFile => {
                self.reload_config();

                let (meter, futs) = self.arrangement.unload(&self.config, &self.plugin_bundles);
                self.meter = meter;
                self.state.last_project = None;

                return futs.map(Message::Arrangement);
            }
            Message::ChangedTab(tab) => self.arrangement.tab = tab,
            Message::OpenFileDialog => {
                return Task::future(
                    AsyncFileDialog::new()
                        .add_filter("Generic Daw project file", &["gdp"])
                        .pick_file(),
                )
                .and_then(Task::done)
                .map(|p| p.path().into())
                .map(Message::OpenFile);
            }
            Message::OpenLastFile => {
                if let Some(last_file) = State::read().unwrap_or_default().last_project {
                    return self.update(Message::OpenFile(last_file));
                }
            }
            Message::SaveFile => {
                return self.update(
                    self.state
                        .last_project
                        .clone()
                        .map_or(Message::SaveAsFileDialog, Message::SaveAsFile),
                );
            }
            Message::SaveAsFileDialog => {
                return Task::future(
                    AsyncFileDialog::new()
                        .add_filter("Generic Daw project file", &["gdp"])
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
                .map(|p| p.path().into())
                .map(ArrangementMessage::Export)
                .map(Message::Arrangement);
            }
            Message::OpenFile(path) => {
                self.reload_config();

                let (meter, futs) = self
                    .arrangement
                    .load(&path, &self.config, &self.plugin_bundles)
                    .unwrap();

                self.meter = meter;

                if self.state.last_project.as_deref() != Some(&path) {
                    self.state.last_project = Some(path);
                    self.state.write();
                }

                return futs.map(Message::Arrangement);
            }
            Message::SaveAsFile(path) => {
                self.arrangement.save(&path);

                if self.state.last_project.as_deref() != Some(&path) {
                    self.state.last_project = Some(path);
                    self.state.write();
                }
            }
            Message::OpenConfigView => self.config_view = Some(ConfigView::default()),
            Message::CloseConfigView => self.config_view = None,
            Message::Stop => {
                self.meter.playing.store(false, Release);
                self.meter.sample.store(0, Release);
                self.arrangement.stop();
                return self
                    .arrangement
                    .update(
                        ArrangementMessage::StopRecord,
                        &self.config,
                        &self.plugin_bundles,
                    )
                    .map(Message::Arrangement);
            }
            Message::TogglePlay => {
                if self.meter.playing.fetch_not(AcqRel) {
                    return self
                        .arrangement
                        .update(
                            ArrangementMessage::StopRecord,
                            &self.config,
                            &self.plugin_bundles,
                        )
                        .map(Message::Arrangement);
                }
            }
            Message::ToggleMetronome => {
                self.meter.metronome.fetch_not(AcqRel);
            }
            Message::ChangedBpm(bpm) => self.meter.bpm.store(bpm.clamp(10, 999), Release),
            Message::ChangedBpmText(bpm) => {
                if let Ok(bpm) = bpm.parse() {
                    return self.update(Message::ChangedBpm(bpm));
                }
            }
            Message::ChangedNumerator(numerator) => {
                self.meter.numerator.store(numerator.clamp(1, 99), Release);
            }
            Message::ChangedNumeratorText(numerator) => {
                if let Ok(numerator) = numerator.parse() {
                    return self.update(Message::ChangedNumerator(numerator));
                }
            }
            Message::SplitAt(split_at) => {
                self.split_at = if split_at >= 20.0 {
                    split_at.clamp(200.0, 400.0)
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
                .arrangement
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
        let config = Config::read().unwrap_or_default();

        if self.config.clap_paths != config.clap_paths {
            self.plugin_bundles = get_installed_plugins(&config.clap_paths);
        }

        if self.config.sample_paths != config.sample_paths {
            self.file_tree.diff(&config.sample_paths);
        }

        self.config = config;
    }

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        if self.arrangement.clap_host.is_plugin_window(window) {
            return empty_widget().into();
        }

        let bpm = self.meter.bpm.load(Acquire);
        let numerator = self.meter.numerator.load(Acquire);
        let fill =
            Position::from_samples(self.meter.sample.load(Acquire), bpm, self.meter.sample_rate)
                .beat()
                % 2
                == 0;

        let mut base = column![
            row![
                styled_pick_list(
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
                ),
                row![
                    styled_button(
                        container(if self.meter.playing.load(Acquire) {
                            pause()
                        } else {
                            play()
                        })
                        .width(LINE_HEIGHT)
                        .align_x(Alignment::Center)
                    )
                    .on_press(Message::TogglePlay),
                    styled_button(
                        container(square())
                            .width(LINE_HEIGHT)
                            .align_x(Alignment::Center)
                    )
                    .on_press(Message::Stop),
                ],
                number_input(
                    numerator as usize,
                    4,
                    2,
                    |x| Message::ChangedNumerator(x as u8),
                    Message::ChangedNumeratorText
                ),
                number_input(
                    bpm as usize,
                    140,
                    3,
                    |x| Message::ChangedBpm(x as u16),
                    Message::ChangedBpmText
                ),
                button(row![AnimatedDot::new(fill), AnimatedDot::new(!fill)].spacing(5.0))
                    .padding(8.0)
                    .style(move |t, s| button_with_base(
                        t,
                        s,
                        if self.meter.metronome.load(Acquire) {
                            button::primary
                        } else {
                            button::secondary
                        }
                    ))
                    .on_press(Message::ToggleMetronome),
                horizontal_space(),
                row![
                    styled_button(chart_no_axes_gantt()).on_press_maybe(
                        (!matches!(self.arrangement.tab, Tab::Arrangement { .. })).then_some(
                            Message::ChangedTab(Tab::Arrangement { grabbed_clip: None })
                        )
                    ),
                    styled_button(sliders_vertical()).on_press_maybe(
                        (!matches!(self.arrangement.tab, Tab::Mixer))
                            .then_some(Message::ChangedTab(Tab::Mixer))
                    )
                ],
            ]
            .spacing(20)
            .align_y(Alignment::Center),
            VSplit::new(
                self.file_tree.view().map(Message::FileTree),
                self.arrangement.view().map(Message::Arrangement),
                self.split_at,
                Message::SplitAt
            )
            .strategy(vsplit::Strategy::Left)
        ]
        .padding(20)
        .spacing(20)
        .into();

        if self.arrangement.loading() {
            base = stack![
                base,
                mouse_area(empty_widget().width(Fill).height(Fill))
                    .interaction(Interaction::Progress)
            ]
            .into();
        }

        if let Some(config_view) = &self.config_view {
            base = modal(
                base,
                config_view
                    .view(&self.input_devices, &self.output_devices)
                    .map(Message::ConfigView),
                Message::CloseConfigView,
            )
            .into();
        }

        base
    }

    pub fn theme(&self, _window: Id) -> Theme {
        self.config.theme.into()
    }

    pub fn title(&self, window: Id) -> String {
        self.arrangement
            .clap_host
            .title(window)
            .unwrap_or_else(|| String::from("Generic DAW"))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let redraw = if self.meter.playing.load(Acquire) {
            frames().map(|_| Message::Redraw)
        } else {
            Subscription::none()
        };

        let autosave = if self.config.autosave.enabled && self.state.last_project.is_some() {
            every(Duration::from_secs(self.config.autosave.interval)).map(|_| Message::SaveFile)
        } else {
            Subscription::none()
        };

        let keybinds = if self.config_view.is_none() {
            keybinds()
        } else {
            Subscription::none()
        };

        Subscription::batch([
            self.arrangement.subscription().map(Message::Arrangement),
            redraw,
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
                            Some(Message::TogglePlay)
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
