use crate::{
    arrangement_view::{ArrangementView, Message as ArrangementMessage, Tab},
    components::{
        empty_widget, styled_button, styled_pick_list, styled_scrollable_with_direction, styled_svg,
    },
    file_tree::FileTree,
    icons::{PAUSE, PLAY, STOP},
    stylefns::button_with_base,
    widget::{AnimatedDot, BpmInput, LINE_HEIGHT, Strategy, VSplit},
};
use generic_daw_core::{
    Denominator, Meter, Numerator, Position, VARIANTS as _,
    clap_host::{self, PluginBundle, PluginDescriptor, PluginType},
};
use iced::{
    Alignment::Center,
    Element, Event, Subscription, Task,
    event::{self, Status},
    keyboard,
    widget::{
        button, column, horizontal_space, row,
        scrollable::{Direction, Scrollbar},
    },
    window::{self, Id, frames},
};
use log::trace;
use rfd::{AsyncFileDialog, FileHandle};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{
        Arc, LazyLock,
        atomic::Ordering::{AcqRel, Acquire, Release},
    },
};

pub static PLUGINS: LazyLock<BTreeMap<PluginDescriptor, PluginBundle>> =
    LazyLock::new(clap_host::get_installed_plugins);

#[derive(Clone, Debug)]
pub enum Message {
    Redraw,

    Arrangement(ArrangementMessage),
    FileTree(Box<Path>),

    SamplesFileDialog,
    ExportFileDialog,

    Stop,
    TogglePlay,
    ToggleMetronome,
    ChangedBpm(u16),
    ChangedNumerator(Numerator),
    ChangedDenominator(Denominator),
    ChangedTab(Tab),

    SplitAt(f32),
}

pub struct Daw {
    arrangement: ArrangementView,
    file_tree: FileTree,
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

        _ = fs::create_dir(dirs::data_dir().unwrap().join("Generic Daw"));

        (
            Self {
                arrangement,
                file_tree: FileTree::new(&dirs::home_dir().unwrap()),
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
            Message::FileTree(path) => self.file_tree.update(&path),
            Message::ChangedTab(tab) => self.arrangement.change_tab(tab),
            Message::SamplesFileDialog => {
                return Task::future(AsyncFileDialog::new().pick_files()).and_then(|paths| {
                    Task::batch(
                        paths
                            .iter()
                            .map(FileHandle::path)
                            .map(Box::from)
                            .map(ArrangementMessage::SampleLoadFromFile)
                            .map(Message::Arrangement)
                            .map(Task::done),
                    )
                });
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
            Message::ChangedDenominator(new_denominator) => {
                self.meter.denominator.store(new_denominator, Release);
            }
            Message::SplitAt(split_at) => self.split_at = split_at.clamp(100.0, 500.0),
        }

        Task::none()
    }

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        if self.arrangement.clap_host.is_plugin_window(window) {
            return empty_widget().into();
        }

        let bpm = self.meter.bpm.load(Acquire);
        let fill = Position::from_interleaved_samples(
            self.meter.sample.load(Acquire),
            bpm,
            self.meter.sample_rate,
        )
        .beat()
            % 2
            == 0;

        column![
            row![
                row![
                    styled_button("Load Samples").on_press(Message::SamplesFileDialog),
                    styled_button("Export").on_press(Message::ExportFileDialog),
                ],
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
                    styled_pick_list(
                        Denominator::VARIANTS,
                        Some(self.meter.denominator.load(Acquire)),
                        Message::ChangedDenominator
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
                    styled_button("Arrangement").on_press(Message::ChangedTab(Tab::Arrangement)),
                    styled_button("Mixer").on_press(Message::ChangedTab(Tab::Mixer))
                ],
                horizontal_space(),
                styled_pick_list(
                    PLUGINS
                        .keys()
                        .filter(|d| d.ty == PluginType::Instrument)
                        .collect::<Box<[_]>>(),
                    None::<&PluginDescriptor>,
                    |p| Message::Arrangement(ArrangementMessage::InstrumentLoad(p.to_owned()))
                )
                .placeholder("Add Instrument")
            ]
            .spacing(20)
            .align_y(Center),
            VSplit::new(
                styled_scrollable_with_direction(
                    self.file_tree.view().0,
                    Direction::Vertical(Scrollbar::default()),
                ),
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
            ArrangementView::subscription().map(Message::Arrangement),
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
