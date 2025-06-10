use crate::{
    arrangement_view::{ArrangementView, Message as ArrangementMessage, Tab},
    components::{number_input, space, styled_button, styled_pick_list},
    config::Config,
    config_view::{ConfigView, Message as ConfigViewMessage},
    file_tree::{FileTree, Message as FileTreeMessage},
    icons::{chart_no_axes_gantt, pause, play, sliders_vertical, square},
    state::State,
    stylefns::button_with_base,
    widget::{AnimatedDot, LINE_HEIGHT},
};
use generic_daw_core::{
    MusicalTime,
    clap_host::{PluginBundle, PluginDescriptor, get_installed_plugins},
    get_input_devices, get_output_devices,
};
use iced::{
    Alignment, Color, Element, Event, Fill, Subscription, Task, Theme,
    event::{self, Status},
    keyboard,
    mouse::Interaction,
    time::every,
    widget::{button, center, column, container, horizontal_space, mouse_area, opaque, row, stack},
    window::{self, Id},
};
use iced_split::{Split, Strategy};
use log::trace;
use rfd::AsyncFileDialog;
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
    SaveAsFileDialog,
    ExportFileDialog,

    OpenFile(Arc<Path>),
    SaveAsFile(Arc<Path>),

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

pub struct Daw {
    config: Config,
    plugin_bundles: BTreeMap<PluginDescriptor, PluginBundle>,
    input_devices: Vec<String>,
    output_devices: Vec<String>,

    arrangement_view: ArrangementView,
    file_tree: FileTree,
    config_view: Option<ConfigView>,
    state: State,
    split_at: f32,
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
        trace!("loaded config {config:?}");

        let state = State::read();
        trace!("loaded state {state:?}");

        let plugin_bundles = get_installed_plugins(&config.clap_paths);
        let file_tree = FileTree::new(&config.sample_paths);

        let mut input_devices = get_input_devices();
        input_devices.sort_unstable();

        let mut output_devices = get_output_devices();
        output_devices.sort_unstable();

        let (mut arrangement, futs) = ArrangementView::new(&config, &plugin_bundles);
        open = open.chain(futs.map(Message::Arrangement));

        if let Some(futs) = state
            .last_project
            .as_deref()
            .filter(|_| config.open_last_project)
            .and_then(|path| arrangement.load(path, &config, &plugin_bundles))
        {
            open = open.chain(futs.map(Message::Arrangement));
        }

        (
            Self {
                config,
                plugin_bundles,
                input_devices,
                output_devices,

                arrangement_view: arrangement,
                file_tree,
                config_view: None,
                state,
                split_at: 300.0,
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
                if let Some(config_view) = self.config_view.as_mut() {
                    return config_view.update(message).map(Message::ConfigView);
                }
            }
            Message::NewFile => {
                self.reload_config();
                self.state.last_project = None;
                return self
                    .arrangement_view
                    .unload(&self.config, &self.plugin_bundles)
                    .map(Message::Arrangement);
            }
            Message::ChangedTab(tab) => self.arrangement_view.tab = tab,
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
                if let Some(last_file) = self.state.last_project.clone() {
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
                .map(|p| p.path().with_extension("wav").into())
                .map(ArrangementMessage::Export)
                .map(Message::Arrangement);
            }
            Message::OpenFile(path) => {
                self.reload_config();
                if self.state.last_project.as_deref() != Some(&path) {
                    self.state.last_project = Some(path.clone());
                    self.state.write();
                }
                return self
                    .arrangement_view
                    .load(&path, &self.config, &self.plugin_bundles)
                    .unwrap()
                    .map(Message::Arrangement);
            }
            Message::SaveAsFile(path) => {
                self.arrangement_view.save(&path);
                if self.state.last_project.as_deref() != Some(&path) {
                    self.state.last_project = Some(path);
                    self.state.write();
                }
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
        if self.arrangement_view.clap_host.is_plugin_window(window) {
            return space().into();
        }

        let fill = MusicalTime::from_samples(
            self.arrangement_view.arrangement.rtstate().sample,
            self.arrangement_view.arrangement.rtstate(),
        )
        .beat()
            % 2
            == 0;

        let mut base = stack![
            column![
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
                            container(if self.arrangement_view.arrangement.rtstate().playing {
                                pause()
                            } else {
                                play()
                            })
                            .width(LINE_HEIGHT)
                            .align_x(Alignment::Center)
                        )
                        .on_press(Message::TogglePlayback),
                        styled_button(
                            container(square())
                                .width(LINE_HEIGHT)
                                .align_x(Alignment::Center)
                        )
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
                    button(row![AnimatedDot::new(fill), AnimatedDot::new(!fill)].spacing(5.0))
                        .padding(8.0)
                        .style(move |t, s| button_with_base(
                            t,
                            s,
                            if self.arrangement_view.arrangement.rtstate().metronome {
                                button::primary
                            } else {
                                button::secondary
                            }
                        ))
                        .on_press(Message::ToggleMetronome),
                    horizontal_space(),
                    row![
                        styled_button(chart_no_axes_gantt()).on_press_maybe(
                            (!matches!(self.arrangement_view.tab, Tab::Arrangement { .. }))
                                .then_some(Message::ChangedTab(Tab::Arrangement {
                                    grabbed_clip: None
                                }))
                        ),
                        styled_button(sliders_vertical()).on_press_maybe(
                            (!matches!(self.arrangement_view.tab, Tab::Mixer))
                                .then_some(Message::ChangedTab(Tab::Mixer))
                        )
                    ],
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                Split::new(
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
                    center(opaque(
                        config_view
                            .view(&self.input_devices, &self.output_devices)
                            .map(Message::ConfigView),
                    ))
                    .style(|_| container::background(Color::BLACK.scale_alpha(0.8))),
                )
                .on_press(Message::CloseConfigView),
            ));
        }

        base.into()
    }

    pub fn theme(&self, _window: Id) -> Theme {
        self.config.theme.into()
    }

    pub fn title(&self, window: Id) -> String {
        self.arrangement_view
            .clap_host
            .title(window)
            .unwrap_or_else(|| String::from("Generic DAW"))
    }

    pub fn subscription(&self) -> Subscription<Message> {
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
