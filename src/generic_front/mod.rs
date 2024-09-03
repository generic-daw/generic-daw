pub mod drawable_clip;
pub mod timeline;
pub mod track_panel;

use crate::generic_back::{
    arrangement::Arrangement,
    build_output_stream,
    position::Meter,
    track::Track,
    track_clip::audio_clip::{read_audio_file, AudioClip},
    StreamMessage,
};
use iced::{
    event, mouse,
    widget::{button, column, row, slider},
    window::frames,
    Element, Event, Subscription,
};
use rfd::FileDialog;
use std::{
    path::PathBuf,
    sync::{atomic::Ordering::SeqCst, mpsc::Sender, Arc, RwLock},
};
use timeline::{Message as TimelineMessage, Timeline};
use track_panel::{Message as TrackPanelMessage, TrackPanel};

pub struct Daw {
    track_panel: TrackPanel,
    timeline: Timeline,
    stream_sender: Sender<StreamMessage>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TrackPanelMessage(TrackPanelMessage),
    TimelineMessage(TimelineMessage),
    LoadSample(String),
    TogglePlay,
    Stop,
    Clear,
    FileSelected(Option<String>),
    ArrangementUpdated,
}

impl Default for Daw {
    fn default() -> Self {
        Self::new(())
    }
}

impl Daw {
    fn new(_flags: ()) -> Self {
        let meter = Meter::new(120.0, 4, 4);
        let arrangement = Arc::new(RwLock::new(Arrangement::new(meter)));
        let stream_sender = build_output_stream(arrangement.clone());

        Self {
            track_panel: TrackPanel::new(arrangement.clone()),
            timeline: Timeline::new(arrangement),
            stream_sender,
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
            Message::LoadSample(_) => {
                if let Some(path) = FileDialog::new().pick_file() {
                    let path_str = path.display().to_string();
                    self.update(Message::FileSelected(Some(path_str)));
                }
            }
            Message::FileSelected(Some(path)) => {
                let clip = Arc::new(AudioClip::new(
                    read_audio_file(
                        &PathBuf::from(path),
                        &self.timeline.arrangement.read().unwrap().meter,
                        self.timeline.samples_sender.clone(),
                    )
                    .unwrap(),
                    &self.timeline.arrangement.read().unwrap().meter,
                ));
                let track = RwLock::new(Track::new());
                track.write().unwrap().clips.push(clip);
                self.timeline
                    .arrangement
                    .write()
                    .unwrap()
                    .tracks
                    .push(track);
                self.update(Message::ArrangementUpdated);
            }
            Message::ArrangementUpdated => {
                self.track_panel
                    .update(&TrackPanelMessage::ArrangementUpdated);
                self.timeline.update(&TimelineMessage::ArrangementUpdated);
            }
            Message::FileSelected(None) => {}
            Message::TogglePlay => {
                self.stream_sender.send(StreamMessage::TogglePlay).unwrap();
            }
            Message::Stop => {
                self.stream_sender.send(StreamMessage::Stop).unwrap();
            }
            Message::Clear => {
                self.timeline.arrangement.write().unwrap().tracks.clear();
                self.update(Message::ArrangementUpdated);
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let controls = row![
            button("Load Sample").on_press(Message::LoadSample(String::new())),
            button(
                if self
                    .timeline
                    .arrangement
                    .read()
                    .unwrap()
                    .meter
                    .playing
                    .load(SeqCst)
                {
                    "Pause"
                } else {
                    "Play"
                }
            )
            .on_press(Message::TogglePlay),
            button("Stop").on_press(Message::Stop),
            button("Clear").on_press(Message::Clear),
            slider(1.0..=13.99999, self.timeline.scale.x, |scale| {
                Message::TimelineMessage(TimelineMessage::XScaleChanged(scale))
            })
            .step(0.1)
            .width(200),
            slider(1.0..=100.0, self.timeline.scale.y, |scale| {
                Message::TimelineMessage(TimelineMessage::YScaleChanged(scale))
            })
            .width(200)
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
            event::listen().map(|e| {
                if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = e {
                    Message::TimelineMessage(TimelineMessage::Scrolled(delta))
                } else {
                    Message::TimelineMessage(TimelineMessage::Tick)
                }
            }),
        ])
    }
}
