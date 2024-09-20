mod timeline;
pub(in crate::generic_front) use timeline::{Timeline, TimelineMessage};

mod timeline_position;
pub use timeline_position::TimelinePosition;

mod timeline_scale;
pub use timeline_scale::TimelineScale;

mod track_panel;
pub(in crate::generic_front) use track_panel::{TrackPanel, TrackPanelMessage};

mod widget;

use crate::generic_back::{
    build_output_stream, Arrangement, AudioClip, AudioTrack, InterleavedAudio,
};
use iced::{
    event, keyboard, mouse,
    widget::{button, column, row, slider},
    window::frames,
    Alignment::Center,
    Element, Event, Subscription,
};
use iced_aw::number_input;
use rfd::FileDialog;
use std::{
    path::PathBuf,
    sync::{atomic::Ordering::SeqCst, Arc},
};

pub struct Daw {
    arrangement: Arc<Arrangement>,
    track_panel: TrackPanel,
    timeline: Timeline,
}

#[derive(Debug, Clone)]
pub enum Message {
    TrackPanel(TrackPanelMessage),
    Timeline(TimelineMessage),
    LoadSample,
    TogglePlay,
    Stop,
    New,
    Export,
    FileSelected(String),
    BpmChanged(u16),
    NumeratorChanged(u8),
    DenominatorChanged(u8),
}

impl Default for Daw {
    fn default() -> Self {
        Self::new()
    }
}

impl Daw {
    fn new() -> Self {
        let arrangement = Arrangement::create();
        build_output_stream(arrangement.clone());

        Self {
            arrangement: arrangement.clone(),
            track_panel: TrackPanel::new(arrangement.clone()),
            timeline: Timeline::new(arrangement),
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::TrackPanel(msg) => {
                self.track_panel.update(&msg);
            }
            Message::Timeline(msg) => {
                self.timeline.update(&msg);
            }
            Message::LoadSample => {
                if let Some(paths) = FileDialog::new().pick_files() {
                    for path in paths {
                        let path_str = path.display().to_string();
                        self.update(Message::FileSelected(path_str));
                    }
                }
            }
            Message::FileSelected(path) => {
                let arrangement = self.arrangement.clone();
                std::thread::spawn(move || {
                    let audio_file = InterleavedAudio::create(&PathBuf::from(path), &arrangement);
                    if let Ok(audio_file) = audio_file {
                        let track = AudioTrack::create(arrangement.clone());
                        track.try_push_audio(AudioClip::new(audio_file, arrangement.clone()));
                        arrangement.tracks.write().unwrap().push(track);
                    }
                });
            }
            Message::TogglePlay => {
                if !self.arrangement.meter.playing.fetch_not(SeqCst)
                    && ((self.arrangement.meter.global_time.load(SeqCst) as f32)
                        < self.arrangement.position.x.load(SeqCst))
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
            Message::NumeratorChanged(new_numerator) => {
                self.arrangement
                    .meter
                    .numerator
                    .store(new_numerator, SeqCst);
            }
            Message::DenominatorChanged(new_denominator) => {
                let old_denominator = self.arrangement.meter.denominator.load(SeqCst);
                let c = u8::from(
                    (1 << old_denominator) < new_denominator
                        && !(old_denominator == 0 && new_denominator == 2),
                ) + 7;
                self.arrangement.meter.denominator.store(
                    c - u8::try_from(new_denominator.leading_zeros()).unwrap(),
                    SeqCst,
                );
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
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
                3.0..=12.999_999,
                self.arrangement.scale.x.load(SeqCst),
                |scale| { Message::Timeline(TimelineMessage::XScaleChanged(scale)) }
            )
            .step(0.1),
            slider(
                20.0..=200.0,
                self.arrangement.scale.y.load(SeqCst),
                |scale| { Message::Timeline(TimelineMessage::YScaleChanged(scale)) }
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
        ]
        .align_y(Center);

        let content = column![
            controls,
            row![
                self.track_panel.view().map(Message::TrackPanel),
                self.timeline.view().map(Message::Timeline)
            ]
            .spacing(20)
        ]
        .padding(20)
        .spacing(20);

        content.into()
    }

    pub fn subscription(_state: &Self) -> Subscription<Message> {
        Subscription::batch([
            frames().map(|_| Message::Timeline(TimelineMessage::Tick)),
            event::listen_with(|e, _, _| match e {
                Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                    Some(Message::Timeline(TimelineMessage::Scrolled(delta)))
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
