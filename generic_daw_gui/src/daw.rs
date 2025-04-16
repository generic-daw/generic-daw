use crate::{
    arrangement_view::{ArrangementView, Message as ArrangementMessage, Tab},
    components::{empty_widget, styled_button, styled_pick_list, styled_text_input},
    file_tree::{FileTree, Message as FileTreeMessage},
    icons::{PAUSE, PLAY, RANGE, STOP},
    stylefns::button_with_base,
    widget::{
        AnimatedDot, DragHandle, LINE_HEIGHT, VSplit,
        vsplit::{self},
    },
};
use generic_daw_core::{Meter, Numerator, Position};
use iced::{
    Alignment::Center,
    Element, Event, Length, Subscription, Task, Theme, border,
    event::{self, Status},
    keyboard,
    widget::{button, column, container, horizontal_space, row, svg},
    window::{self, Id, frames},
};
use log::trace;
use rfd::AsyncFileDialog;
use std::{
    f32::consts::FRAC_PI_2,
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

    NewFile,
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
    ChangedBpmText(String),
    ChangedNumerator(Numerator),
    ChangedTab(Tab),

    SplitAt(f32),
}

pub struct Daw {
    arrangement: ArrangementView,
    file_tree: FileTree,
    sample_dirs: Box<[Box<Path>]>,
    open_project: Option<Arc<Path>>,
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

        let sample_dirs = vec![dirs::home_dir().unwrap().into()].into_boxed_slice();

        (
            Self {
                arrangement,
                file_tree: FileTree::from(&sample_dirs),
                sample_dirs,
                open_project: None,
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
            Message::NewFile => (self.arrangement, self.meter) = ArrangementView::create(),
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
                self.arrangement.save(&path);
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
            Message::ChangedBpm(bpm) => self.meter.bpm.store(bpm.max(10), Release),
            Message::ChangedBpmText(bpm) => {
                if let Ok(bpm) = bpm.parse() {
                    return self.update(Message::ChangedBpm(bpm));
                }
            }
            Message::ChangedNumerator(new_numerator) => {
                self.meter.numerator.store(new_numerator, Release);
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
                return self.file_tree.update(&path, action).map(Message::FileTree);
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
                styled_pick_list(
                    ["New", "Open", "Save", "Save As", "Export"],
                    Some("File"),
                    |s| {
                        match s {
                            "New" => Message::NewFile,
                            "Open" => Message::OpenFileDialog,
                            "Save" => Message::SaveFile,
                            "Save As" => Message::SaveAsFileDialog,
                            "Export" => Message::ExportFileDialog,
                            _ => unreachable!(),
                        }
                    }
                ),
                row![
                    styled_button(
                        svg(if self.meter.playing.load(Acquire) {
                            PAUSE.clone()
                        } else {
                            PLAY.clone()
                        })
                        .width(Length::Shrink)
                        .height(LINE_HEIGHT)
                    )
                    .on_press(Message::TogglePlay),
                    styled_button(svg(STOP.clone()).width(Length::Shrink).height(LINE_HEIGHT))
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
                row![
                    DragHandle::new(
                        container(
                            svg(RANGE.clone())
                                .style(|t: &Theme, _| svg::Style {
                                    color: Some(t.extended_palette().background.weak.text)
                                })
                                .width(Length::Shrink)
                                .height(LINE_HEIGHT)
                                .rotation(FRAC_PI_2)
                        )
                        .style(|t: &Theme| {
                            container::transparent(t)
                                .background(t.extended_palette().background.weak.color)
                                .border(
                                    border::width(1.0)
                                        .color(t.extended_palette().background.strongest.color),
                                )
                        })
                        .padding([5.0, 0.0]),
                        bpm,
                        140,
                        Message::ChangedBpm
                    ),
                    styled_text_input("", &bpm.to_string())
                        .width(42.0)
                        .on_input(Message::ChangedBpmText)
                ],
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
            .strategy(vsplit::Strategy::Left)
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
            }),
        ])
    }
}
