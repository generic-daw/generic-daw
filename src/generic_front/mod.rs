mod timeline;
mod track_panel;

use crate::generic_back::{
    arrangement::Arrangement,
    build_output_stream,
    position::Meter,
    track::Track,
    track_clip::audio_clip::{read_audio_file, AudioClip},
    StreamMessage,
};
use cpal::traits::{DeviceTrait, HostTrait};
use iced::{
    widget::{button, column, row},
    Element, Sandbox,
};
use rfd::FileDialog;
use std::{
    path::PathBuf,
    sync::{mpsc::Sender, Arc, RwLock},
};
use timeline::{Timeline, TimelineMessage};
use track_panel::TrackPanel;

pub struct Daw {
    arrangement: Arc<RwLock<Arrangement>>,
    track_panel: TrackPanel,
    timeline: Timeline,
    stream_sender: Sender<StreamMessage>,
    playing: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    TrackPanel(<TrackPanel as Sandbox>::Message),
    Timeline(<Timeline as Sandbox>::Message),
    TimelineMessage(TimelineMessage),
    LoadSample(String),
    TogglePlay,
    Stop,
    FileSelected(Option<String>),
    ArrangementUpdated,
}

impl Sandbox for Daw {
    type Message = Message;

    fn new() -> Self {
        let meter = Arc::new(RwLock::new(Meter::new(
            120.0,
            4,
            4,
            cpal::default_host()
                .default_output_device()
                .unwrap()
                .default_output_config()
                .unwrap()
                .sample_rate()
                .0,
        )));

        let arrangement = Arc::new(RwLock::new(Arrangement::new(meter.clone())));
        let stream_sender = build_output_stream(arrangement.clone(), meter);

        Self {
            track_panel: TrackPanel::new(arrangement.clone()),
            timeline: Timeline::new(arrangement.clone()),
            arrangement,
            stream_sender,
            playing: false,
        }
    }

    fn title(&self) -> String {
        String::from("GenericDAW")
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::TrackPanel(msg) => self.track_panel.update(msg),
            Message::Timeline(msg) => self.timeline.update(msg),
            Message::LoadSample(_) => {
                if let Some(path) = FileDialog::new().pick_file() {
                    let path_str = path.display().to_string();
                    self.update(Message::FileSelected(Some(path_str)));
                }
            }
            Message::FileSelected(Some(path)) => {
                let meter = self.arrangement.read().unwrap().meter.clone();
                let clip = Arc::new(AudioClip::new(
                    read_audio_file(&PathBuf::from(path), &meter).unwrap(),
                    &meter,
                ));
                let track = Arc::new(RwLock::new(Track::new()));
                track.write().unwrap().clips.push(clip);
                self.arrangement.write().unwrap().tracks.push(track);
                self.update(Message::ArrangementUpdated);
            }
            Message::ArrangementUpdated => {
                self.track_panel
                    .update(track_panel::Message::ArrangementUpdated);
                self.timeline
                    .update(timeline::TimelineMessage::ArrangementUpdated);
            }
            Message::FileSelected(None) => {}
            Message::TogglePlay => {
                self.stream_sender.send(StreamMessage::TogglePlay).unwrap();
                self.playing ^= true;
            }
            Message::Stop => {
                self.stream_sender.send(StreamMessage::Stop).unwrap();
                self.playing = false;
            }
            Message::TimelineMessage(timeline_msg) => {
                self.timeline.update(timeline_msg);
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let controls = row![
            button("Load Sample").on_press(Message::LoadSample(String::new())),
            button(if self.playing { "Pause" } else { "Play" }).on_press(Message::TogglePlay),
            button("Stop").on_press(Message::Stop)
        ];

        let content = column![
            controls,
            row![
                self.track_panel.view().map(Message::TrackPanel),
                self.timeline.view().map(Message::Timeline)
            ]
        ]
        .padding(20)
        .spacing(20);

        content.into()
    }
}
