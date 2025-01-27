use crate::widget::{
    Arrangement as ArrangementWidget, ArrangementPosition, ArrangementScale, Knob,
};
use generic_daw_core::{
    Arrangement as ArrangementInner, AudioClip, AudioTrack, InterleavedAudio, Stream,
    StreamTrait as _,
};
use iced::{
    widget::{container, container::Style, row},
    Element, Length, Task,
};
use rfd::FileHandle;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum Message {
    TrackVolumeChanged(usize, f32),
    TrackPanChanged(usize, f32),
    LoadedSample(Arc<InterleavedAudio>),
    Export(FileHandle),
}

pub struct Arrangement {
    inner: Arc<ArrangementInner>,
    position: ArrangementPosition,
    scale: ArrangementScale,
    stream: Stream,
}

impl Arrangement {
    pub fn new(inner: Arc<ArrangementInner>, stream: Stream) -> Self {
        Self {
            inner,
            position: ArrangementPosition::default(),
            scale: ArrangementScale::default(),
            stream,
        }
    }

    pub fn update(&self, message: Message) -> Task<Message> {
        match message {
            Message::TrackVolumeChanged(idx, volume) => {
                self.inner.tracks()[idx].set_volume(volume);
            }
            Message::TrackPanChanged(idx, pan) => {
                self.inner.tracks()[idx].set_pan(pan);
            }
            Message::LoadedSample(audio_file) => {
                let (node, track) = AudioTrack::create(self.inner.meter.clone());

                let mut ok = true;
                ok &= self.inner.audio_graph.add(&node);
                ok &= self
                    .inner
                    .audio_graph
                    .connect(&self.inner.audio_graph.root(), &node);
                ok &= track.try_push(&AudioClip::create(audio_file, self.inner.meter.clone()));
                debug_assert!(ok);

                self.inner.tracks.write().unwrap().push(track);
            }
            Message::Export(path) => {
                self.stream.pause().unwrap();
                self.inner.export(path.path());
                self.stream.play().unwrap();
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        ArrangementWidget::new(&self.inner, &self.position, &self.scale, |idx| {
            container(
                row![
                    Knob::new(0.0..=1.0, 0.0, 1.0, move |f| Message::TrackVolumeChanged(
                        idx, f
                    )),
                    Knob::new(-1.0..=1.0, 0.0, 0.0, move |f| Message::TrackPanChanged(
                        idx, f
                    ))
                ]
                .spacing(5.0),
            )
            .padding(5.0)
            .height(Length::Fill)
            .style(|theme| Style {
                background: Some(
                    theme
                        .extended_palette()
                        .secondary
                        .weak
                        .color
                        .scale_alpha(0.25)
                        .into(),
                ),
                ..Style::default()
            })
            .into()
        })
        .into()
    }
}
