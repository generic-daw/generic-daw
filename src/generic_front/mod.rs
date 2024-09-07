pub mod drawable;
pub mod timeline;
pub mod track_panel;

use crate::generic_back::{
    arrangement::Arrangement,
    build_output_stream,
    meter::Meter,
    track::audio_track::AudioTrack,
    track_clip::audio_clip::{interleaved_audio::InterleavedAudio, AudioClip},
};
use iced::{
    event, keyboard, mouse,
    widget::{button, column, row, slider},
    window::frames,
    Element, Event, Subscription,
};
use iced_aw::number_input;
use rfd::FileDialog;
use std::{
    path::PathBuf,
    sync::{atomic::Ordering::SeqCst, Arc, RwLock},
};
use timeline::{Message as TimelineMessage, Timeline};
use track_panel::{Message as TrackPanelMessage, TrackPanel};

pub struct Daw {
    arrangement: Arc<RwLock<Arrangement>>,
    track_panel: TrackPanel,
    timeline: Timeline,
}

#[derive(Debug, Clone)]
pub enum Message {
    TrackPanelMessage(TrackPanelMessage),
    TimelineMessage(TimelineMessage),
    LoadSample(String),
    TogglePlay,
    Stop,
    New,
    Export,
    FileSelected(Option<String>),
    ArrangementUpdated,
    BpmChanged(u16),
    NumeratorChanged(u8),
    DenominatorChanged(u8),
}

impl Default for Daw {
    fn default() -> Self {
        Self::new(())
    }
}

impl Daw {
    fn new(_flags: ()) -> Self {
        let meter = Meter::new(140, 4, 4);
        let arrangement = Arc::new(RwLock::new(Arrangement::new(meter)));
        build_output_stream(arrangement.clone());

        Self {
            arrangement: arrangement.clone(),
            track_panel: TrackPanel::new(arrangement.clone()),
            timeline: Timeline::new(arrangement),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn update(&mut self, message: Message) {
        match message {
            Message::TrackPanelMessage(msg) => {
                self.track_panel.update(&msg);
            }
            Message::TimelineMessage(msg) => {
                self.timeline.update(&msg);
            }
            Message::LoadSample(_) => {
                if let Some(paths) = FileDialog::new().pick_files() {
                    for path in paths {
                        let path_str = path.display().to_string();
                        self.update(Message::FileSelected(Some(path_str)));
                    }
                }
            }
            Message::FileSelected(Some(path)) => {
                let audio_file = InterleavedAudio::new(
                    &PathBuf::from(path),
                    &self.arrangement.read().unwrap().meter,
                    self.timeline.samples_sender.clone(),
                );
                if let Ok(audio_file) = audio_file {
                    let clip = AudioClip::new(audio_file, &self.arrangement.read().unwrap().meter);
                    let track = Arc::new(AudioTrack::new());
                    track.clips.write().unwrap().push(clip);
                    self.arrangement.write().unwrap().tracks.push(track);
                    self.update(Message::ArrangementUpdated);
                }
            }
            Message::FileSelected(None) => {}
            Message::ArrangementUpdated => {
                self.track_panel
                    .update(&TrackPanelMessage::ArrangementUpdated);
                self.timeline.update(&TimelineMessage::ArrangementUpdated);
            }
            Message::TogglePlay => {
                let arrangement = self.arrangement.read().unwrap();
                arrangement.meter.playing.fetch_xor(true, SeqCst);
                if arrangement.meter.playing.load(SeqCst) {
                    self.timeline.update(&TimelineMessage::MovePlayToStart);
                }
            }
            Message::Stop => {
                self.arrangement
                    .read()
                    .unwrap()
                    .meter
                    .playing
                    .store(false, SeqCst);
                self.arrangement
                    .read()
                    .unwrap()
                    .meter
                    .global_time
                    .store(0, SeqCst);
            }
            Message::New => {
                self.arrangement.write().unwrap().tracks.clear();
                self.update(Message::ArrangementUpdated);
            }
            Message::Export => {
                if let Some(path) = FileDialog::new()
                    .add_filter("Wave File", &["wav"])
                    .save_file()
                {
                    self.arrangement
                        .read()
                        .unwrap()
                        .export(&path, &self.arrangement.read().unwrap().meter);
                }
            }
            Message::BpmChanged(bpm) => {
                self.arrangement.write().unwrap().meter.bpm = bpm;
            }
            Message::NumeratorChanged(numerator) => {
                self.arrangement.write().unwrap().meter.numerator = numerator;
            }
            Message::DenominatorChanged(denominator) => {
                let c = u8::from(
                    (1 << self.arrangement.read().unwrap().meter.denominator) < denominator
                        && !(self.arrangement.read().unwrap().meter.denominator == 0
                            && denominator == 2),
                ) + 7;
                self.arrangement.write().unwrap().meter.denominator =
                    c - u8::try_from(denominator.leading_zeros()).unwrap();
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let controls = row![
            button("Load Sample").on_press(Message::LoadSample(String::new())),
            button(
                if self.arrangement.read().unwrap().meter.playing.load(SeqCst) {
                    "Pause"
                } else {
                    "Play"
                }
            )
            .on_press(Message::TogglePlay),
            button("Stop").on_press(Message::Stop),
            button("Export").on_press(Message::Export),
            button("New").on_press(Message::New),
            slider(0.0..=12.99999, self.timeline.scale.x, |scale| {
                Message::TimelineMessage(TimelineMessage::XScaleChanged(scale))
            })
            .step(0.1),
            slider(20.0..=200.0, self.timeline.scale.y, |scale| {
                Message::TimelineMessage(TimelineMessage::YScaleChanged(scale))
            }),
            number_input(
                self.arrangement.read().unwrap().meter.numerator,
                1..=255,
                Message::NumeratorChanged
            )
            .ignore_buttons(true),
            number_input(
                1 << self.arrangement.read().unwrap().meter.denominator,
                1..=128,
                Message::DenominatorChanged
            )
            .ignore_buttons(true),
            number_input(
                self.arrangement.read().unwrap().meter.bpm,
                1..=65535,
                Message::BpmChanged
            )
            .ignore_buttons(true)
        ];

        let content = column![
            controls,
            row![
                self.track_panel.view().map(Message::TrackPanelMessage),
                self.timeline.view().map(Message::TimelineMessage)
            ]
        ]
        .padding(20)
        .spacing(20);

        content.into()
    }

    pub fn subscription(_state: &Self) -> Subscription<Message> {
        Subscription::batch([
            frames().map(|_| Message::TimelineMessage(TimelineMessage::Tick)),
            event::listen_with(|e, _, _| match e {
                Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                    Some(Message::TimelineMessage(TimelineMessage::Scrolled(delta)))
                }
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
                                "e" => Some(Message::Export),
                                _ => None,
                            },
                            _ => None,
                        },
                        _ => None,
                    }
                }
                _ => None,
            }),
        ])
    }
}
