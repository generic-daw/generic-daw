use crate::generic_back::{
    build_output_stream, resample, Arrangement as ArrangementInner, AudioClip, AudioTrack,
    Denominator, InterleavedAudio, Numerator,
};
use cpal::Stream;
use etcetera::{choose_base_strategy, BaseStrategy as _};
use iced::{
    border::Radius,
    event::{self, Status},
    keyboard,
    widget::{button, column, container, pick_list, row, scrollable, toggler, Text},
    window::frames,
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

mod timeline_position;
pub(in crate::generic_front) use timeline_position::TimelinePosition;

mod timeline_scale;
pub(in crate::generic_front) use timeline_scale::TimelineScale;

mod track_panel;
pub(in crate::generic_front) use track_panel::{TrackPanel, TrackPanelMessage};

mod widget;
use widget::{Arrangement, VSplit};

static ON_BAR_CLICK: &[f32] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: &[f32] = include_f32s!("../../assets/off_bar_click.pcm");

pub struct Daw {
    arrangement: Arc<ArrangementInner>,
    track_panel: TrackPanel,
    _stream: Stream,
}

#[derive(Clone, Debug, Default)]
pub enum Message {
    #[default]
    Tick,
    TrackPanel(TrackPanelMessage),
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
            arrangement: arrangement.clone(),
            track_panel: TrackPanel::new(arrangement),
            _stream: stream,
        }
    }
}

impl Daw {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {}
            Message::TrackPanel(msg) => {
                self.track_panel.update(&msg);
            }
            Message::LoadSample(path) => {
                let arrangement = self.arrangement.clone();
                std::thread::spawn(move || {
                    let audio_file = InterleavedAudio::create(path, &arrangement.meter);
                    if let Ok(audio_file) = audio_file {
                        let track = AudioTrack::create(arrangement.meter.clone());
                        track.try_push(&AudioClip::create(audio_file, arrangement.meter.clone()));
                        arrangement.tracks.write().unwrap().push(track);
                    }
                });
            }
            Message::LoadSamplesButton => {
                return Task::perform(AsyncFileDialog::new().pick_files(), |paths| {
                    paths.map_or(Message::Tick, Message::LoadSamples)
                });
            }
            Message::LoadSamples(paths) => paths
                .iter()
                .map(FileHandle::path)
                .map(PathBuf::from)
                .for_each(|path| {
                    drop(self.update(Message::LoadSample(path)));
                }),
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
            Message::New => {
                *self = Self::default();
            }
            Message::ExportButton => {
                return Task::perform(
                    AsyncFileDialog::new()
                        .add_filter("Wave File", &["wav"])
                        .save_file(),
                    |path| path.map_or(Message::Tick, Message::Export),
                );
            }
            Message::Export(path) => {
                self.arrangement.export(path.path());
            }
            Message::BpmChanged(bpm) => {
                self.arrangement.meter.bpm.store(bpm, SeqCst);
            }
            Message::NumeratorChanged(new_numerator) => {
                self.arrangement
                    .meter
                    .numerator
                    .store(new_numerator, SeqCst);
            }
            Message::DenominatorChanged(new_denominator) => {
                self.arrangement
                    .meter
                    .denominator
                    .store(new_denominator, SeqCst);
            }
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
        ]
        .spacing(20)
        .align_y(Center);

        let content = column![
            controls,
            VSplit::new(
                scrollable(
                    file_tree(PathBuf::from(choose_base_strategy().unwrap().home_dir()))
                        .unwrap()
                        .on_double_click(Message::LoadSample)
                ),
                row![
                    self.track_panel.view().map(Message::TrackPanel),
                    container(Arrangement::new(self.arrangement.clone())).style(|_| {
                        container::Style {
                            border: iced::Border {
                                color: Theme::default().extended_palette().secondary.weak.color,
                                width: 1.0,
                                radius: Radius::new(0.0),
                            },
                            ..container::Style::default()
                        }
                    })
                ]
            )
            .split(0.25)
        ]
        .padding(20)
        .spacing(20);

        content.into()
    }

    pub fn subscription(_state: &Self) -> Subscription<Message> {
        Subscription::batch([
            frames().map(|_| Message::Tick),
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
}
