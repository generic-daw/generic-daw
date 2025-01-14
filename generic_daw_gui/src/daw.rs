use crate::{
    clap_host::{ClapHost, Message as ClapHostMessage, OpenedMessage},
    widget::{Arrangement, VSplit},
};
use generic_daw_core::{
    build_output_stream,
    clap_host::{clack_host::process::PluginAudioConfiguration, get_installed_plugins, open_gui},
    Arrangement as ArrangementInner, AudioClip, AudioTrack, Denominator, InterleavedAudio,
    Numerator, Stream,
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
    sync::{atomic::Ordering::SeqCst, Arc, Mutex},
};
use strum::VariantArray as _;

pub struct Daw {
    arrangement: Arc<ArrangementInner>,
    clap_host: ClapHost,
    theme: Theme,
    _stream: Stream,
}

#[derive(Clone, Debug, Default)]
pub enum Message {
    #[default]
    Ping,
    ThemeChanged(Theme),
    ClapHost(ClapHostMessage),
    #[expect(dead_code)]
    Test,
    LoadSamplesButton,
    LoadSamples(Vec<FileHandle>),
    LoadSample(PathBuf),
    LoadedSample(Arc<InterleavedAudio>),
    ExportButton,
    Export(FileHandle),
    TogglePlay,
    Stop,
    New,
    BpmChanged(u16),
    NumeratorChanged(Numerator),
    DenominatorChanged(Denominator),
    ToggleMetronome,
}

impl Default for Daw {
    fn default() -> Self {
        let arrangement = ArrangementInner::create();
        let stream = build_output_stream(arrangement.clone());

        Self {
            arrangement,
            clap_host: ClapHost::default(),
            theme: Theme::Dark,
            _stream: stream,
        }
    }
}

impl Daw {
    #[expect(clippy::too_many_lines)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ping => {}
            Message::ThemeChanged(theme) => self.theme = theme,
            Message::ClapHost(message) => {
                return self.clap_host.update(message).map(Message::ClapHost);
            }
            Message::Test => {
                let (id, fut) = window::open(Settings {
                    exit_on_close_request: false,
                    ..Settings::default()
                });
                let sample_rate = f64::from(self.arrangement.meter.sample_rate.load(SeqCst));
                let embed = window::run_with_handle(id, move |handle| {
                    let (plugin, host_audio_processor, plugin_audio_processor) = open_gui(
                        &get_installed_plugins()[0],
                        PluginAudioConfiguration {
                            sample_rate,
                            max_frames_count: 256,
                            min_frames_count: 256,
                        },
                        handle.as_raw(),
                    );
                    Arc::new(Mutex::new(OpenedMessage {
                        id,
                        plugin,
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
                let (tx, rx) = async_channel::bounded(1);

                let arrangement = self.arrangement.clone();
                std::thread::spawn(move || {
                    let audio_file = InterleavedAudio::create(path, &arrangement.meter);
                    tx.send_blocking(audio_file).unwrap();
                });

                return Task::future(async move { rx.recv().await })
                    .and_then(Task::done)
                    .and_then(Task::done)
                    .map(Message::LoadedSample);
            }
            Message::LoadedSample(audio_file) => {
                let track = AudioTrack::create(self.arrangement.meter.clone());
                debug_assert!(track.try_push(&AudioClip::create(
                    audio_file,
                    self.arrangement.meter.clone(),
                )));
                self.arrangement.tracks.write().unwrap().push(track);
            }
            Message::ExportButton => {
                return Task::future(
                    AsyncFileDialog::new()
                        .add_filter("Wave File", &["wav"])
                        .save_file(),
                )
                .and_then(Task::done)
                .map(Message::Export);
            }
            Message::Export(path) => self.arrangement.export(path.path()),
            Message::TogglePlay => {
                self.arrangement.meter.playing.fetch_not(SeqCst);
            }
            Message::Stop => {
                self.arrangement.meter.playing.store(false, SeqCst);
                self.arrangement.meter.sample.store(0, SeqCst);
                self.arrangement
                    .live_sample_playback
                    .write()
                    .unwrap()
                    .clear();
            }
            Message::New => *self = Self::default(),
            Message::BpmChanged(bpm) => self.arrangement.meter.bpm.store(bpm, SeqCst),
            Message::NumeratorChanged(new_numerator) => self
                .arrangement
                .meter
                .numerator
                .store(new_numerator, SeqCst),
            Message::DenominatorChanged(new_denominator) => self
                .arrangement
                .meter
                .denominator
                .store(new_denominator, SeqCst),
            Message::ToggleMetronome => {
                self.arrangement.metronome.fetch_not(SeqCst);
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
                        if self.arrangement.meter.playing.load(SeqCst) {
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
                    Some(self.arrangement.meter.numerator.load(SeqCst)),
                    Message::NumeratorChanged
                )
                .width(50),
                pick_list(
                    Denominator::VARIANTS,
                    Some(self.arrangement.meter.denominator.load(SeqCst)),
                    Message::DenominatorChanged
                )
                .width(50),
            ],
            number_input(
                self.arrangement.meter.bpm.load(SeqCst),
                30..=600,
                Message::BpmChanged
            )
            .width(50),
            toggler(self.arrangement.metronome.load(SeqCst))
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
                scrollable(
                    file_tree(home_dir().unwrap())
                        .unwrap()
                        .on_double_click(Message::LoadSample)
                ),
                Arrangement::new(self.arrangement.clone())
            )
            .split(0.25)
        ]
        .padding(20)
        .spacing(20);

        content.into()
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::batch([
            ClapHost::subscription().map(Message::ClapHost),
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
