use crate::{
    arrangement_view::{ArrangementView, Message as ArrangementMessage},
    clap_host_view::{ClapHostView, Message as ClapHostMessage, Opened},
    widget::VSplit,
};
use generic_daw_core::{
    clap_host::{clack_host::process::PluginAudioConfiguration, get_installed_plugins, open_gui},
    Denominator, InterleavedAudio, Meter, Numerator,
};
use home::home_dir;
use iced::{
    event::{self, Status},
    keyboard,
    widget::{button, column, horizontal_space, pick_list, row, scrollable, toggler, Text},
    window::{self, Settings},
    Alignment::Center,
    Element, Event, Subscription, Task, Theme,
};
use iced_aw::number_input;
use iced_file_tree::file_tree;
use iced_fonts::{bootstrap, BOOTSTRAP_FONT};
use rfd::{AsyncFileDialog, FileHandle};
use std::{
    path::PathBuf,
    sync::{
        atomic::Ordering::{AcqRel, Acquire, Release},
        Arc, Mutex,
    },
};
use strum::VariantArray as _;

#[derive(Clone, Debug)]
pub enum Message {
    Animate,
    ThemeChanged(Theme),
    ClapHost(ClapHostMessage),
    Arrangement(ArrangementMessage),
    #[expect(dead_code)]
    Test,
    LoadSamplesButton,
    LoadSamples(Vec<FileHandle>),
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
                let (id, fut) = window::open(Settings {
                    exit_on_close_request: false,
                    ..Settings::default()
                });
                let sample_rate = f64::from(self.meter.sample_rate.load(Acquire));
                let embed = window::run_with_handle(id, move |handle| {
                    let (gui, host_audio_processor, plugin_audio_processor) = open_gui(
                        &get_installed_plugins()[0],
                        PluginAudioConfiguration {
                            sample_rate,
                            max_frames_count: 256,
                            min_frames_count: 256,
                        },
                        handle.as_raw(),
                    );
                    Arc::new(Mutex::new(Opened {
                        id,
                        gui,
                        host_audio_processor,
                        plugin_audio_processor,
                    }))
                });
                return Task::batch([
                    fut.discard(),
                    embed.map(ClapHostMessage::Opened).map(Message::ClapHost),
                ]);
            }
            Message::LoadSamplesButton => {
                return Task::future(AsyncFileDialog::new().pick_files())
                    .and_then(Task::done)
                    .map(Message::LoadSamples);
            }
            Message::LoadSamples(paths) => {
                return Task::batch(
                    paths
                        .iter()
                        .map(FileHandle::path)
                        .map(PathBuf::from)
                        .map(|path| self.update(Message::LoadSample(path))),
                );
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
        let controls = row![
            row![
                button("Load Samples").on_press(Message::LoadSamplesButton),
                button("Export").on_press(Message::ExportButton),
                button("New").on_press(Message::New),
            ],
            row![
                button(
                    Text::new(bootstrap::icon_to_string(
                        if self.meter.playing.load(Acquire) {
                            bootstrap::Bootstrap::PauseFill
                        } else {
                            bootstrap::Bootstrap::PlayFill
                        }
                    ))
                    .font(BOOTSTRAP_FONT)
                )
                .on_press(Message::TogglePlay),
                button(
                    Text::new(bootstrap::icon_to_string(bootstrap::Bootstrap::StopFill))
                        .font(BOOTSTRAP_FONT)
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
