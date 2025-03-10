use crate::{
    arrangement_view::{ArrangementView, Message as ArrangementMessage, Tab},
    components::{styled_button, styled_pick_list, styled_scrollable_with_direction, styled_svg},
    file_tree::FileTree,
    widget::{BpmInput, LINE_HEIGHT, VSplit},
};
use generic_daw_core::{
    Denominator, Meter, Numerator, VARIANTS as _,
    clap_host::{self, PluginDescriptor, PluginType, clack_host::bundle::PluginBundle},
};
use iced::{
    Alignment::Center,
    Element, Event, Subscription, Task, Theme,
    event::{self, Status},
    keyboard,
    widget::{
        column, horizontal_space, row,
        scrollable::{Direction, Scrollbar},
        svg, toggler, vertical_space,
    },
    window::{self, Id},
};
use rfd::{AsyncFileDialog, FileHandle};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Arc, LazyLock,
        atomic::Ordering::{AcqRel, Acquire, Release},
    },
};

static PLAY: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--play-arrow-rounded.svg"
    ))
});
static PAUSE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--pause-rounded.svg"
    ))
});
static STOP: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--stop-rounded.svg"
    ))
});

pub static PLUGINS: LazyLock<BTreeMap<PluginDescriptor, PluginBundle>> =
    LazyLock::new(clap_host::get_installed_plugins);

#[derive(Clone, Debug)]
pub enum Message {
    ThemeChanged(Theme),
    Arrangement(ArrangementMessage),
    FileTree(Box<Path>),
    SamplesFileDialog,
    ExportFileDialog,
    TogglePlay,
    Stop,
    BpmChanged(u16),
    NumeratorChanged(Numerator),
    DenominatorChanged(Denominator),
    ToggleMetronome,
    Tab(Tab),
    SplitAt(f32),
}

pub struct Daw {
    main_window_id: Id,
    arrangement: ArrangementView,
    file_tree: FileTree,
    split_at: f32,
    meter: Arc<Meter>,
    theme: Theme,
}

impl Daw {
    pub fn new() -> (Self, Task<Message>) {
        let (main_window_id, open) = window::open(window::Settings {
            exit_on_close_request: false,
            ..window::Settings::default()
        });

        let (arrangement, meter) = ArrangementView::create(main_window_id);

        (
            Self {
                main_window_id,
                arrangement,
                file_tree: FileTree::new(
                    #[expect(deprecated, reason = "rust#132515")]
                    &std::env::home_dir().unwrap(),
                ),
                split_at: 0.25,
                meter,
                theme: Theme::CatppuccinFrappe,
            },
            open.discard(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChanged(theme) => self.theme = theme,
            Message::Arrangement(message) => {
                return self.arrangement.update(message).map(Message::Arrangement);
            }
            Message::FileTree(path) => {
                self.file_tree.update(&path);
            }
            Message::SamplesFileDialog => {
                return Task::future(AsyncFileDialog::new().pick_files()).and_then(|paths| {
                    Task::batch(
                        paths
                            .iter()
                            .map(FileHandle::path)
                            .map(Box::from)
                            .map(ArrangementMessage::LoadSample)
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
                .map(|p| Box::from(p.path()))
                .map(ArrangementMessage::Export)
                .map(Message::Arrangement);
            }
            Message::TogglePlay => {
                self.meter.playing.fetch_not(AcqRel);
            }
            Message::Stop => {
                self.meter.playing.store(false, Release);
                self.meter.sample.store(0, Release);
                self.arrangement.stop();
            }
            Message::BpmChanged(bpm) => self.meter.bpm.store(bpm, Release),
            Message::NumeratorChanged(new_numerator) => {
                self.meter.numerator.store(new_numerator, Release);
            }
            Message::DenominatorChanged(new_denominator) => {
                self.meter.denominator.store(new_denominator, Release);
            }
            Message::ToggleMetronome => {
                self.meter.metronome.fetch_not(AcqRel);
            }
            Message::Tab(tab) => self.arrangement.change_tab(tab),
            Message::SplitAt(split_at) => self.split_at = split_at.min(0.5),
        }

        Task::none()
    }

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        if window != self.main_window_id {
            return vertical_space().into();
        }

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
                        Message::NumeratorChanged
                    )
                    .width(50),
                    styled_pick_list(
                        Denominator::VARIANTS,
                        Some(self.meter.denominator.load(Acquire)),
                        Message::DenominatorChanged
                    )
                    .width(50),
                ],
                BpmInput::new(self.meter.bpm.load(Acquire), 30..=600, Message::BpmChanged),
                toggler(self.meter.metronome.load(Acquire))
                    .label("Metronome")
                    .on_toggle(|_| Message::ToggleMetronome),
                row![
                    styled_button("Arrangement").on_press(Message::Tab(Tab::Arrangement)),
                    styled_button("Mixer").on_press(Message::Tab(Tab::Mixer))
                ],
                horizontal_space(),
                styled_pick_list(Theme::ALL, Some(&self.theme), Message::ThemeChanged),
                styled_pick_list(
                    PLUGINS
                        .keys()
                        .filter(|d| d.ty == PluginType::Instrument)
                        .collect::<Box<[_]>>(),
                    None::<&PluginDescriptor>,
                    |p| Message::Arrangement(ArrangementMessage::LoadInstrumentPlugin(
                        p.to_owned()
                    ))
                )
                .placeholder("Load Plugin")
            ]
            .spacing(20)
            .align_y(Center),
            VSplit::new(
                styled_scrollable_with_direction(
                    self.file_tree.view().0,
                    Direction::Vertical(Scrollbar::default()),
                ),
                self.arrangement.view().map(Message::Arrangement),
                self.split_at,
                Message::SplitAt
            )
        ]
        .padding(20)
        .spacing(20)
        .into()
    }

    pub fn theme(&self, _window: Id) -> Theme {
        self.theme.clone()
    }

    pub fn title(&self, window: Id) -> String {
        if window == self.main_window_id {
            String::from("Generic DAW")
        } else {
            self.arrangement
                .title(window)
                .unwrap_or_else(|| String::from("Generic DAW"))
        }
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::batch([
            ArrangementView::subscription().map(Message::Arrangement),
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
