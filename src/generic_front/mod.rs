mod timeline;
mod track_panel;

use crate::generic_back::{
    arrangement::Arrangement,
    cpal_get_default_device, get_output_stream,
    track::Track,
    track_clip::audio_clip::{read_audio_file, AudioClip},
    StreamMessage,
};
use cpal::{traits::StreamTrait, Stream, StreamConfig};
use iced::{
    widget::{button, column, row},
    Element, Sandbox,
};
use rfd::FileDialog;
use std::{
    path::PathBuf,
    sync::{mpsc::Sender, Arc, Mutex},
};
use timeline::Timeline;
use track_panel::TrackPanel;

pub struct Daw {
    #[allow(dead_code)]
    arrangement: Arc<Mutex<Arrangement>>,
    track_panel: TrackPanel,
    timeline: Timeline,
    stream: Stream,
    stream_config: StreamConfig,
    stream_sender: Sender<StreamMessage>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TrackPanel(<TrackPanel as Sandbox>::Message),
    Timeline(<Timeline as Sandbox>::Message),
    LoadSample(#[allow(dead_code)] String),
    Play,
    Stop,
    FileSelected(Option<String>),
    ArrangementUpdated,
}

impl Sandbox for Daw {
    type Message = Message;

    fn new() -> Self {
        let arrangement = Arc::new(Mutex::new(Arrangement::new()));

        let (stream_device, stream_config) = cpal_get_default_device();
        let (stream_sender, stream_receiver) = std::sync::mpsc::channel();
        let stream = get_output_stream(
            &stream_device,
            &stream_config,
            arrangement.clone(),
            stream_receiver,
        );
        stream.play().unwrap();

        Self {
            track_panel: TrackPanel::new(arrangement.clone()),
            timeline: Timeline::new(arrangement.clone()),
            stream,
            stream_config,
            stream_sender,
            arrangement,
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
                let clip = Arc::new(AudioClip::new(
                    read_audio_file(&PathBuf::from(path), &self.stream_config)
                        .expect("Failed to load sample"),
                ));
                let mut track = Track::new();
                track.push(clip);
                self.arrangement.lock().unwrap().push(track);
                self.update(Message::ArrangementUpdated);
            }
            Message::ArrangementUpdated => {
                self.track_panel
                    .update(track_panel::Message::ArrangementUpdated);
                self.timeline.update(timeline::Message::ArrangementUpdated);
            }
            Message::FileSelected(None) => {}
            Message::Play => {
                _ = self.stream_sender.send(StreamMessage::Play);
            }
            Message::Stop => {
                _ = self.stream_sender.send(StreamMessage::Stop);
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let controls = row![
            button("Load Sample").on_press(Message::LoadSample(String::new())),
            button("Play").on_press(Message::Play),
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
