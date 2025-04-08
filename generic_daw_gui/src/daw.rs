use crate::{
    arrangement_view::{ArrangementView, Message as ArrangementMessage, Tab},
    components::{empty_widget, styled_button, styled_pick_list, styled_svg},
    file_tree::{FileTree, Message as FileTreeMessage},
    icons::{PAUSE, PLAY, STOP},
    stylefns::button_with_base,
    widget::{AnimatedDot, BpmInput, LINE_HEIGHT, VSplit, vsplit::Strategy},
};
use generic_daw_core::{Meter, Numerator, Position};
use iced::{
    Alignment::Center,
    Element, Event, Subscription, Task,
    event::{self, Status},
    keyboard,
    widget::{button, column, horizontal_space, row},
    window::{self, Id, frames},
};
use log::trace;
use rfd::AsyncFileDialog;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::Ordering::{AcqRel, Acquire, Release},
    },
};

#[derive(Clone, Debug)]
pub enum Message {
    Redraw,

    Arrangement(ArrangementMessage),
    FileTree(FileTreeMessage),

    OpenFileDialog,
    SaveFile,
    SaveAsFileDialog,
    ExportFileDialog,

    OpenFile(Arc<Path>),
    SaveAsFile(Arc<Path>),

    Stop,
    TogglePlay,
    ToggleMetronome,
    ChangedBpm(u16),
    ChangedNumerator(Numerator),
    ChangedTab(Tab),

    SplitAt(f32),
}

pub struct Daw {
    arrangement: ArrangementView,
    open_project: Option<Arc<Path>>,
    file_tree: FileTree,
    sample_dirs: Vec<Box<Path>>,
    split_at: f32,
    meter: Arc<Meter>,
}

impl Daw {
    pub fn create() -> (Self, Task<Message>) {
        let (_, open) = window::open(window::Settings {
            exit_on_close_request: false,
            maximized: true,
            ..window::Settings::default()
        });

        let (arrangement, meter) = ArrangementView::create();

        let home_dir = dirs::home_dir().unwrap().into();
        let data_dir = dirs::data_dir().unwrap().join("Generic Daw").into();

        _ = std::fs::create_dir(&data_dir);

        let sample_dirs = vec![home_dir, data_dir];

        (
            Self {
                arrangement,
                open_project: None,
                file_tree: (&sample_dirs).into(),
                sample_dirs,
                split_at: 300.0,
                meter,
            },
            open.discard(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        trace!("{message:?}");

        match message {
            Message::Redraw => {}
            Message::Arrangement(message) => {
                return self.arrangement.update(message).map(Message::Arrangement);
            }
            Message::FileTree(action) => return self.handle_file_tree_action(action),
            Message::ChangedTab(tab) => self.arrangement.change_tab(tab),
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
            Message::SaveFile => {
                return self.update(
                    self.open_project
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
                if let Some((arrangement, meter, futs)) =
                    ArrangementView::load(&path, &self.sample_dirs)
                {
                    self.arrangement = arrangement;
                    self.meter = meter;
                    self.open_project = Some(path);
                    return futs.map(Message::Arrangement);
                }
            }
            Message::SaveAsFile(path) => {
                self.arrangement.save(&path, &self.sample_dirs);
                self.open_project = Some(path);
            }
            Message::Stop => {
                self.meter.playing.store(false, Release);
                self.meter.sample.store(0, Release);
                self.arrangement.stop();
                return self
                    .arrangement
                    .update(ArrangementMessage::StopRecord)
                    .map(Message::Arrangement);
            }
            Message::TogglePlay => {
                if self.meter.playing.fetch_not(AcqRel) {
                    return self
                        .arrangement
                        .update(ArrangementMessage::StopRecord)
                        .map(Message::Arrangement);
                }
            }
            Message::ToggleMetronome => {
                self.meter.metronome.fetch_not(AcqRel);
            }
            Message::ChangedBpm(bpm) => self.meter.bpm.store(bpm, Release),
            Message::ChangedNumerator(new_numerator) => {
                self.meter.numerator.store(new_numerator, Release);
            }
            Message::SplitAt(split_at) => self.split_at = split_at.clamp(100.0, 500.0),
        }

        Task::none()
    }

    pub fn handle_file_tree_action(&mut self, action: FileTreeMessage) -> Task<Message> {
        match action {
            FileTreeMessage::None => {}
            FileTreeMessage::File(path) => {
                return self
                    .arrangement
                    .update(ArrangementMessage::SampleLoadFromFile(path))
                    .map(Message::Arrangement);
            }
            FileTreeMessage::Action(path, action) => {
                return self.file_tree.update(&path, &action).map(Message::FileTree);
            }
        }

        Task::none()
    }

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        if self.arrangement.clap_host.is_plugin_window(window) {
            return empty_widget().into();
        }

        let bpm = self.meter.bpm.load(Acquire);
        let fill =
            Position::from_samples(self.meter.sample.load(Acquire), bpm, self.meter.sample_rate)
                .beat()
                % 2
                == 0;

        column![
            row![
                styled_pick_list(["Open", "Save", "Save As", "Export"], Some("File"), |s| {
                    match s {
                        "Open" => Message::OpenFileDialog,
                        "Save" => Message::SaveFile,
                        "Save As" => Message::SaveAsFileDialog,
                        "Export" => Message::ExportFileDialog,
                        _ => unreachable!(),
                    }
                }),
                row![
                    styled_button(
                        styled_svg(if self.meter.playing.load(Acquire) {
                            PAUSE.clone()
                        } else {
                            PLAY.clone()
                        })
                        .height(LINE_HEIGHT)
                    )
                    .on_press(Message::TogglePlay),
                    styled_button(styled_svg(STOP.clone()).height(LINE_HEIGHT))
                        .on_press(Message::Stop),
                ],
                row![
                    styled_pick_list(
                        Numerator::VARIANTS,
                        Some(self.meter.numerator.load(Acquire)),
                        Message::ChangedNumerator
                    )
                    .width(50),
                ],
                BpmInput::new(30..=600, bpm, Message::ChangedBpm),
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
                row![
                    styled_button("Arrangement")
                        .on_press(Message::ChangedTab(Tab::Arrangement { grabbed_clip: None })),
                    styled_button("Mixer").on_press(Message::ChangedTab(Tab::Mixer))
                ],
                horizontal_space(),
            ]
            .spacing(20)
            .align_y(Center),
            VSplit::new(
                self.file_tree.view().map(Message::FileTree),
                self.arrangement.view().map(Message::Arrangement),
                Message::SplitAt
            )
            .strategy(Strategy::Left)
            .split_at(self.split_at)
        ]
        .padding(20)
        .spacing(20)
        .into()
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

        Subscription::batch([
            self.arrangement.subscription().map(Message::Arrangement),
            redraw,
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
                                    "s" => Some(Message::SaveFile),
                                    "o" => Some(Message::OpenFileDialog),
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
            }),
        ])
    }
}
