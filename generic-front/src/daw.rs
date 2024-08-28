use crate::audio_engine::AudioEngine;
use crate::timeline::Timeline;
use crate::track_panel::TrackPanel;
use generic_back::track_clip::audio_clip::AudioClip;
use iced::widget::{button, column, row};
use iced::{Element, Sandbox};
use rfd::FileDialog; // Import the file dialog
use std::sync::Arc;

pub struct Daw {
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
    FileSelected(Option<String>), // Add a new variant for file selection
}

impl Sandbox for Daw {
    type Message = Message;

    fn new() -> Self {
        Self {
            track_panel: TrackPanel::new(),
            timeline: Timeline::new(),
            audio_engine: AudioEngine::new(),
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
                // Open file dialog and get the selected file path
                if let Some(path) = FileDialog::new().pick_file() {
                    let path_str = path.display().to_string();
                    self.update(Message::FileSelected(Some(path_str)));
                }
            }
            Message::FileSelected(Some(path)) => {
                let sample = self.audio_engine.load_sample(&path);
                if let Ok(sample) = sample {
                    let clip = Arc::new(AudioClip::new(sample));
                    let index = self.audio_engine.add_track();
                    self.audio_engine.add_audio_clip(index, clip);
                } else {
                    eprintln!("{}: {path}", sample.err().unwrap());
                }
            }
            Message::FileSelected(None) => {
                // Handle case where no file was selected, if necessary
            }
            Message::Play => self.audio_engine.play(),
            Message::Stop => self.audio_engine.stop(),
        }
    }

    fn view(&self) -> Element<Message> {
        let controls = row![
            button("Load Sample").on_press(Message::LoadSample(String::new())),
            button("Play").on_press(Message::Play),
            button("Stop").on_press(Message::Stop),
        ];

        let content = column![
            controls,
            row![
                self.track_panel.view().map(Message::TrackPanel),
                self.timeline.view().map(Message::Timeline)
            ]
        ]
        .padding(20) // Apply padding to the entire column
        .spacing(20); // Apply spacing between elements in the column

        content.into()
    }
}
