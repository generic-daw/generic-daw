use crate::{
    arrangement_view::{ArrangementView, Message as ArrangementMessage},
    clap_host_view::{ClapHostView, Message as ClapHostMessage, Opened},
    widget::VSplit,
};
use fragile::Fragile;
use generic_daw_core::{
    clap_host::{
        clack_host::process::PluginAudioConfiguration, get_installed_plugins, init_gui,
        open_embedded, open_floating,
    },
    Denominator, InterleavedAudio, Meter, Numerator, VARIANTS as _,
};
use home::home_dir;
use iced::{
    event::{self, Status},
    keyboard,
    widget::{button, column, horizontal_space, pick_list, row, scrollable, svg, toggler},
    window::{self, Id, Settings},
    Alignment::Center,
    Element, Event, Length, Subscription, Task, Theme,
};
use iced_aw::number_input;
use iced_file_tree::file_tree;
use rfd::{AsyncFileDialog, FileHandle};
use std::{
    path::PathBuf,
    sync::{
        atomic::Ordering::{AcqRel, Acquire, Release},
        Arc, Mutex,
    },
};

#[derive(Clone, Debug)]
pub enum Message {
    Animate,
    ThemeChanged(Theme),
    ClapHost(ClapHostMessage),
    Arrangement(ArrangementMessage),
    #[expect(dead_code)]
    Test,
    LoadSamplesButton,
    LoadSample(PathBuf),
    ExportButton,
    TogglePlay,
    Stop,
    New,
    BpmChanged(u16),
    NumeratorChanged(Numerator),
    DenominatorChanged(Denominator),
    ToggleMetronome,
}

pub struct Daw {
    arrangement: ArrangementView,
    clap_host: ClapHostView,
    meter: Arc<Meter>,
    theme: Theme,
}

impl Daw {
    pub fn create() -> (Self, Task<Message>) {
        let (meter, arrangement, task) = ArrangementView::create();

        let daw = Self {
            arrangement,
            clap_host: ClapHostView::default(),
            meter,
            theme: Theme::Dark,
        };

        (daw, task.map(Message::Arrangement))
    }

    #[expect(clippy::too_many_lines)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Animate => {}
            Message::ThemeChanged(theme) => self.theme = theme,
            Message::ClapHost(message) => {
                return self.clap_host.update(message).map(Message::ClapHost);
            }
            Message::Arrangement(message) => {
                return self.arrangement.update(message).map(Message::Arrangement);
            }
            Message::Test => {
                let sample_rate = f64::from(self.meter.sample_rate);
                let config = PluginAudioConfiguration {
                    sample_rate,
                    max_frames_count: 256,
                    min_frames_count: 256,
                };
                let (gui, hap, pap, i) = init_gui(&get_installed_plugins()[0], config);

                return if gui.needs_floating().unwrap() {
                    let gui = open_floating(gui, i);
                    let id = Id::unique();

                    self.clap_host
                        .update(ClapHostMessage::Opened(Arc::new(Mutex::new(Opened {
                            id,
                            gui: Fragile::new(gui),
                            hap,
                            pap,
                        }))))
                        .map(Message::ClapHost)
                } else {
                    let i = Fragile::new(i);

                    let (id, spawn) = window::open(Settings {
                        exit_on_close_request: false,
                        ..Settings::default()
                    });

                    let embed = window::run_with_handle(id, move |handle| {
                        let gui = open_embedded(gui, i.into_inner(), handle.as_raw());

                        Arc::new(Mutex::new(Opened {
                            id,
                            gui: Fragile::new(gui),
                            hap,
                            pap,
                        }))
                    });

                    spawn
                        .discard()
                        .chain(embed)
                        .map(ClapHostMessage::Opened)
                        .map(Message::ClapHost)
                };
            }
            Message::LoadSamplesButton => {
                return Task::future(AsyncFileDialog::new().pick_files()).and_then(|paths| {
                    Task::batch(
                        paths
                            .iter()
                            .map(FileHandle::path)
                            .map(PathBuf::from)
                            .map(Message::LoadSample)
                            .map(Task::done),
                    )
                });
            }
            Message::LoadSample(path) => {
                let meter = self.meter.clone();
                return Task::future(tokio::task::spawn_blocking(move || {
                    InterleavedAudio::create(path, &meter)
                }))
                .and_then(Task::done)
                .and_then(Task::done)
                .map(ArrangementMessage::LoadedSample)
                .map(Message::Arrangement);
            }
            Message::ExportButton => {
                return Task::future(
                    AsyncFileDialog::new()
                        .add_filter("Wave File", &["wav"])
                        .save_file(),
                )
                .and_then(Task::done)
                .map(ArrangementMessage::Export)
                .map(Message::Arrangement);
            }
            Message::TogglePlay => {
                self.meter.playing.fetch_not(AcqRel);
            }
            Message::Stop => {
                self.meter.playing.store(false, Release);
                self.meter.sample.store(0, Release);
            }
            Message::New => {
                let (s, task) = Self::create();
                *self = s;
                return task;
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
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let stop_handle = svg::Handle::from_path("assets/material-symbols--stop-rounded.svg");
        let play_pause_handle = svg::Handle::from_path(if self.meter.playing.load(Acquire) {
            "assets/material-symbols--pause-rounded.svg"
        } else {
            "assets/material-symbols--play-arrow-rounded.svg"
        });

        let controls = row![
            row![
                button("Load Samples").on_press(Message::LoadSamplesButton),
                button("Export").on_press(Message::ExportButton),
                button("New").on_press(Message::New),
            ],
            row![
                button(
                    svg(play_pause_handle)
                        .style(|theme: &Theme, _| svg::Style {
                            color: Some(theme.extended_palette().secondary.base.text)
                        })
                        .width(Length::Shrink)
                        .height(Length::Fixed(21.0))
                )
                .on_press(Message::TogglePlay),
                button(
                    svg(stop_handle)
                        .style(|theme: &Theme, _| svg::Style {
                            color: Some(theme.extended_palette().secondary.base.text)
                        })
                        .width(Length::Shrink)
                        .height(Length::Fixed(21.0))
                )
                .on_press(Message::Stop),
            ],
            row![
                pick_list(
                    Numerator::VARIANTS,
                    Some(self.meter.numerator.load(Acquire)),
                    Message::NumeratorChanged
                )
                .width(50),
                pick_list(
                    Denominator::VARIANTS,
                    Some(self.meter.denominator.load(Acquire)),
                    Message::DenominatorChanged
                )
                .width(50),
            ],
            number_input(&self.meter.bpm.load(Acquire), 30..=600, Message::BpmChanged).width(50),
            toggler(self.meter.metronome.load(Acquire))
                .label("Metronome")
                .on_toggle(|_| Message::ToggleMetronome),
            horizontal_space(),
            pick_list(Theme::ALL, Some(&self.theme), Message::ThemeChanged),
        ]
        .spacing(20)
        .align_y(Center);

        let content = column![
            controls,
            VSplit::new(
                scrollable(file_tree(home_dir().unwrap()).on_double_click(Message::LoadSample),),
                self.arrangement.view().map(Message::Arrangement)
            )
            .split(0.25)
        ]
        .padding(20)
        .spacing(20);

        content.into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let animate = if self.meter.playing.load(Acquire) {
            window::frames().map(|_| Message::Animate)
        } else {
            Subscription::none()
        };

        Subscription::batch([
            animate,
            ClapHostView::subscription().map(Message::ClapHost),
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
                                keyboard::Key::Character(c) => match c.to_string().as_str() {
                                    "n" => Some(Message::New),
                                    "e" => Some(Message::ExportButton),
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

    pub fn theme(&self) -> Theme {
        self.theme.clone()
    }
}
