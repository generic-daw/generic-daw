mod audio_engine;
mod timeline;
mod track;
mod track_panel;

use crate::generic_back::{arrangement::Arrangement, track_clip::audio_clip::AudioClip};
use audio_engine::AudioEngine;
use iced::{
    widget::{button, column, row},
    Element, Sandbox,
};
use rfd::FileDialog;
use std::sync::{Arc, Mutex};
use timeline::Timeline;
use track_panel::TrackPanel;

pub struct Daw {
    #[allow(dead_code)]
    arrangement: Arc<Mutex<Arrangement>>,
    track_panel: TrackPanel,
    timeline: Timeline,
    audio_engine: AudioEngine,
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
        Self {
            track_panel: TrackPanel::new(arrangement.clone()),
            timeline: Timeline::new(arrangement.clone()),
            audio_engine: AudioEngine::new(),
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
                    self.audio_engine
                        .load_sample(&path)
                        .expect("Failed to load sample"),
                ));
                let index = self.audio_engine.add_track();
                self.audio_engine.add_audio_clip(index, clip);
                self.update(Message::ArrangementUpdated);
            }
            Message::ArrangementUpdated => {
                self.track_panel
                    .update(track_panel::Message::ArrangementUpdated);
                self.timeline.update(timeline::Message::ArrangementUpdated);
            }
            Message::FileSelected(None) => {}
            Message::Play => self.audio_engine.play(),
            Message::Stop => self.audio_engine.stop(),
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
