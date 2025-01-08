use crate::{
    clap_host::{get_installed_plugins, open_gui, ClapPluginWrapper},
    generic_back::{
        build_output_stream, resample, Arrangement as ArrangementInner, AudioClip, AudioTrack,
        Denominator, InterleavedAudio, Numerator,
    },
};
use clack_host::process::PluginAudioConfiguration;
use clap_host::{ClapHost, Message as ClapHostMessage};
use cpal::Stream;
use home::home_dir;
use iced::{
    border::Radius,
    event::{self, Status},
    keyboard,
    widget::{
        button, column, container, horizontal_space, pick_list, row, scrollable, toggler, Text,
    },
    window::{self, Settings},
    Alignment::Center,
    Element, Event, Subscription, Task, Theme,
};
use iced_aw::number_input;
use iced_file_tree::file_tree;
use iced_fonts::{bootstrap, BOOTSTRAP_FONT};
use include_data::include_f32s;
use rfd::{AsyncFileDialog, FileHandle};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering::SeqCst, Arc},
};
use strum::VariantArray as _;
use timeline_position::TimelinePosition;
use timeline_scale::TimelineScale;
use widget::{Arrangement, VSplit};

mod clap_host;
mod timeline_position;
mod timeline_scale;
mod widget;

static ON_BAR_CLICK: &[f32] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: &[f32] = include_f32s!("../../assets/off_bar_click.pcm");

pub struct Daw {
    arrangement: Arc<ArrangementInner>,
    clap_host: ClapHost,
    theme: Theme,
    _stream: Stream,
}

#[derive(Clone, Debug, Default)]
pub enum Message {
    ClapHost(ClapHostMessage),
    #[default]
    Ping,
    #[expect(dead_code)]
    Test,
    ThemeChanged(Theme),
    LoadSample(PathBuf),
    LoadSamplesButton,
    LoadSamples(Vec<FileHandle>),
    TogglePlay,
    Stop,
    New,
    ExportButton,
    Export(FileHandle),
    BpmChanged(u16),
    NumeratorChanged(Numerator),
    DenominatorChanged(Denominator),
    ToggleMetronome,
}

impl Default for Daw {
    fn default() -> Self {
        let arrangement = ArrangementInner::create();
        let stream = build_output_stream(arrangement.clone());

        *arrangement.on_bar_click.write().unwrap() = resample(
            44100,
            arrangement.meter.sample_rate.load(SeqCst),
            ON_BAR_CLICK.into(),
        )
        .unwrap()
        .into();
        *arrangement.off_bar_click.write().unwrap() = resample(
            44100,
            arrangement.meter.sample_rate.load(SeqCst),
            OFF_BAR_CLICK.into(),
        )
        .unwrap()
        .into();

        Self {
            arrangement,
            clap_host: ClapHost::default(),
            theme: Theme::Dark,
            _stream: stream,
        }
    }
}

impl Daw {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ping => {}
            Message::Test => {
                let (id, fut) = window::open(Settings {
                    exit_on_close_request: false,
                    ..Settings::default()
                });
                let sample_rate = self.arrangement.meter.sample_rate.load(SeqCst).into();
                let embed = window::run_with_handle(id, move |handle| {
                    (
                        id,
                        ClapPluginWrapper::new(open_gui(
                            &get_installed_plugins()[0],
                            PluginAudioConfiguration {
                                max_frames_count: 256,
                                min_frames_count: 256,
                                sample_rate,
                            },
                            handle.as_raw(),
                        )),
                    )
                });
                return Task::batch([
                    fut.discard(),
                    embed.map(ClapHostMessage::Opened).map(Message::ClapHost),
                ]);
            }
            Message::ClapHost(message) => {
                return self.clap_host.update(message).map(Message::ClapHost);
            }
            Message::ThemeChanged(theme) => self.theme = theme,
            Message::LoadSample(path) => {
                let (tx, rx) = async_channel::bounded(1);

                let arrangement = self.arrangement.clone();
                std::thread::spawn(move || {
                    let audio_file = InterleavedAudio::create(path, &arrangement.meter);
                    if let Ok(audio_file) = audio_file {
                        let track = AudioTrack::create(arrangement.meter.clone());
                        track.try_push(&AudioClip::create(audio_file, arrangement.meter.clone()));
                        arrangement.tracks.write().unwrap().push(track);
                    }
                    tx.send_blocking(()).unwrap();
                });

                return Task::perform(async move { rx.recv().await }, |_| Message::Ping);
            }
            Message::LoadSamplesButton => {
                return Task::perform(AsyncFileDialog::new().pick_files(), |paths| {
                    paths.map_or(Message::Ping, Message::LoadSamples)
                });
            }
            Message::LoadSamples(paths) => {
                return Task::batch(
                    paths
                        .iter()
                        .map(FileHandle::path)
                        .map(PathBuf::from)
                        .map(|path| self.update(Message::LoadSample(path))),
                )
            }
            Message::TogglePlay => {
                self.arrangement.meter.playing.fetch_not(SeqCst);
            }
            Message::Stop => {
                self.arrangement
                    .live_sample_playback
                    .write()
                    .unwrap()
                    .clear();
                self.arrangement.meter.playing.store(false, SeqCst);
                self.arrangement.meter.global_time.store(0, SeqCst);
            }
            Message::New => *self = Self::default(),
            Message::ExportButton => {
                return Task::perform(
                    AsyncFileDialog::new()
                        .add_filter("Wave File", &["wav"])
                        .save_file(),
                    |path| path.map_or(Message::Ping, Message::Export),
                );
            }
            Message::Export(path) => self.arrangement.export(path.path()),
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
                row![
                    container(Arrangement::new(self.arrangement.clone(), Message::Ping)).style(
                        |_| {
                            container::Style {
                                border: iced::Border {
                                    color: Theme::default().extended_palette().secondary.weak.color,
                                    width: 1.0,
                                    radius: Radius::new(0.0),
                                },
                                ..container::Style::default()
                            }
                        }
                    )
                ]
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
