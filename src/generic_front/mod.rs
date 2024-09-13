pub mod timeline;
pub mod timeline_state;
pub mod track_panel;
pub mod widget;

use crate::generic_back::{
    arrangement::Arrangement,
    build_output_stream,
    meter::Meter,
    track::{audio_track::AudioTrack, TrackType},
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
use timeline_state::{TimelinePosition, TimelineScale};
use track_panel::{Message as TrackPanelMessage, TrackPanel};

pub struct Daw {
    arrangement: Arc<Arrangement>,
    track_panel: TrackPanel,
    timeline: Timeline,
}

#[derive(Debug, Clone)]
pub enum Message {
    TrackPanelMessage(TrackPanelMessage),
    TimelineMessage(TimelineMessage),
    LoadSample,
    TogglePlay,
    Stop,
    New,
    Export,
    FileSelected(Option<String>),
    BpmChanged(u32),
    NumeratorChanged(u32),
    DenominatorChanged(u32),
}

impl Default for Daw {
    fn default() -> Self {
        Self::new(())
    }
}

impl Daw {
    fn new(_flags: ()) -> Self {
        let position = RwLock::new(TimelinePosition { x: 0.0, y: 0.0 });
        let scale = RwLock::new(TimelineScale { x: 8.0, y: 100.0 });
        let arrangement = Arc::new(Arrangement::new(Meter::new(), scale, position));
        build_output_stream(arrangement.clone());

        Self {
            arrangement: arrangement.clone(),
            track_panel: TrackPanel::new(arrangement.clone()),
            timeline: Timeline::new(arrangement),
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::TrackPanelMessage(msg) => {
                self.track_panel.update(&msg);
            }
            Message::TimelineMessage(msg) => {
                self.timeline.update(&msg);
            }
            Message::LoadSample => {
                if let Some(paths) = FileDialog::new().pick_files() {
                    for path in paths {
                        let path_str = path.display().to_string();
                        self.update(Message::FileSelected(Some(path_str)));
                    }
                }
            }
            Message::FileSelected(Some(path)) => {
                let arrangement = self.arrangement.clone();
                let sender = self.timeline.samples_sender.clone();
                std::thread::spawn(move || {
                    let audio_file =
                        InterleavedAudio::new(&PathBuf::from(path), &arrangement.meter, sender);
                    if let Ok(audio_file) = audio_file {
                        let clip = AudioClip::new(audio_file, arrangement.clone());
                        let mut track = AudioTrack::new(arrangement.clone());
                        track.clips.push(clip);

                        arrangement
                            .tracks
                            .write()
                            .unwrap()
                            .push(TrackType::Audio(RwLock::new(track)));
                    }
                });
            }
            Message::FileSelected(None) => {}
            Message::TogglePlay => {
                if !self.arrangement.meter.playing.fetch_xor(true, SeqCst)
                    && ((self.arrangement.meter.global_time.load(SeqCst) as f32)
                        < self.arrangement.position.read().unwrap().x)
                {
                    self.timeline.update(&TimelineMessage::MovePlayToStart);
                }
            }
            Message::Stop => {
                self.arrangement.meter.playing.store(false, SeqCst);
                self.arrangement.meter.global_time.store(0, SeqCst);
            }
            Message::New => {
                self.arrangement.tracks.write().unwrap().clear();
            }
            Message::Export => {
                if let Some(path) = FileDialog::new()
                    .add_filter("Wave File", &["wav"])
                    .save_file()
                {
                    self.arrangement.export(&path);
                }
            }
            Message::BpmChanged(bpm) => {
                self.arrangement.meter.bpm.store(bpm, SeqCst);
            }
            Message::NumeratorChanged(numerator) => {
                self.arrangement.meter.numerator.store(numerator, SeqCst);
            }
            Message::DenominatorChanged(denominator) => {
                let c = u32::from(
                    (1 << self.arrangement.meter.denominator.load(SeqCst)) < denominator
                        && !(self.arrangement.meter.denominator.load(SeqCst) == 0
                            && denominator == 2),
                ) + 31;
                self.arrangement
                    .meter
                    .denominator
                    .store(c - denominator.leading_zeros(), SeqCst);
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let controls = row![
            button("Load Sample").on_press(Message::LoadSample),
            button(if self.arrangement.meter.playing.load(SeqCst) {
                "Pause"
            } else {
                "Play"
            })
            .on_press(Message::TogglePlay),
            button("Stop").on_press(Message::Stop),
            button("Export").on_press(Message::Export),
            button("New").on_press(Message::New),
            slider(
                0.0..=12.99999,
                self.arrangement.scale.read().unwrap().x,
                |scale| { Message::TimelineMessage(TimelineMessage::XScaleChanged(scale)) }
            )
            .step(0.1),
            slider(
                20.0..=200.0,
                self.arrangement.scale.read().unwrap().y,
                |scale| { Message::TimelineMessage(TimelineMessage::YScaleChanged(scale)) }
            ),
            number_input(
                self.arrangement.meter.numerator.load(SeqCst),
                1..=255,
                Message::NumeratorChanged
            )
            .ignore_buttons(true),
            number_input(
                1 << self.arrangement.meter.denominator.load(SeqCst),
                1..=128,
                Message::DenominatorChanged
            )
            .ignore_buttons(true),
            number_input(
                self.arrangement.meter.bpm.load(SeqCst),
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
