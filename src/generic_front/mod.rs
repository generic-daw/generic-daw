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
    widget::{button, column, row, slider},
    window::frames,
    Application, Command, Element, Sandbox,
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
    Timeline(<Timeline as Application>::Message),
    TimelineMessage(TimelineMessage),
    LoadSample(String),
    TogglePlay,
    Stop,
    Clear,
    FileSelected(Option<String>),
    ArrangementUpdated,
}

impl Application for Daw {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = iced::Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let meter = Meter::new(120.0, 4, 4, 0);

        let arrangement = Arc::new(RwLock::new(Arrangement::new(meter)));
        let (stream_sender, global_time) = build_output_stream(arrangement.clone());

        (
            Self {
                track_panel: TrackPanel::new(arrangement.clone()),
                timeline: Timeline::new(arrangement.clone(), global_time),
                arrangement,
                stream_sender,
                playing: false,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("GenericDAW")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::TrackPanel(msg) => {
                Sandbox::update(&mut self.track_panel, msg);
            }
            Message::Timeline(msg) => {
                _ = Application::update(&mut self.timeline, msg);
            }
            Message::LoadSample(_) => {
                if let Some(path) = FileDialog::new().pick_file() {
                    let path_str = path.display().to_string();
                    _ = self.update(Message::FileSelected(Some(path_str)));
                }
            }
            Message::FileSelected(Some(path)) => {
                let clip = Arc::new(AudioClip::new(
                    read_audio_file(
                        &PathBuf::from(path),
                        &self.arrangement.read().unwrap().meter,
                    )
                    .unwrap(),
                    &self.arrangement.read().unwrap().meter,
                ));
                let track = Arc::new(RwLock::new(Track::new()));
                track.write().unwrap().clips.push(clip);
                self.arrangement.write().unwrap().tracks.push(track);
                _ = self.update(Message::ArrangementUpdated);
            }
            Message::ArrangementUpdated => {
                Sandbox::update(
                    &mut self.track_panel,
                    track_panel::Message::ArrangementUpdated,
                );
                _ = Application::update(
                    &mut self.timeline,
                    timeline::TimelineMessage::ArrangementUpdated,
                );
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
            Message::Clear => {
                self.arrangement.write().unwrap().tracks.clear();
                _ = self.update(Message::ArrangementUpdated);
            }
            Message::TimelineMessage(timeline_msg) => {
                Sandbox::update(&mut self.timeline, timeline_msg);
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let controls = row![
            button("Load Sample").on_press(Message::LoadSample(String::new())),
            button(if self.playing { "Pause" } else { "Play" }).on_press(Message::TogglePlay),
            button("Stop").on_press(Message::Stop),
            button("Clear").on_press(Message::Clear),
            slider(
                1.0..=1000.0,
                self.timeline.timeline_x_scale as f32,
                |scale| { Message::Timeline(TimelineMessage::XScaleChanged(scale as usize)) }
            )
            .width(200),
            slider(
                1.0..=100.0,
                self.timeline.timeline_y_scale as f32,
                |scale| { Message::Timeline(TimelineMessage::YScaleChanged(scale as usize)) }
            )
            .width(200)
        ];

        let content = column![
            controls,
            row![
                Sandbox::view(&self.track_panel).map(Message::TrackPanel),
                Sandbox::view(&self.timeline).map(Message::Timeline)
            ]
        ]
        .padding(20)
        .spacing(20);

        content.into()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        frames().map(|_| Message::TimelineMessage(TimelineMessage::ArrangementUpdated))
    }
}
