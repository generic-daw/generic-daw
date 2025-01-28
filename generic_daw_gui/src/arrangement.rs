use crate::widget::{
    Arrangement as ArrangementWidget, ArrangementPosition, ArrangementScale, Knob, LINE_HEIGHT,
};
use generic_daw_core::{
    Arrangement as ArrangementInner, AudioClip, AudioTrack, InterleavedAudio, Position, Stream,
    StreamTrait as _,
};
use iced::{
    widget::{container, container::Style, row},
    Element, Length, Task,
};
use rfd::FileHandle;
use std::{
    ops::Deref as _,
    sync::{atomic::Ordering::SeqCst, Arc},
};

#[derive(Clone, Debug)]
pub enum Message {
    TrackVolumeChanged(usize, f32),
    TrackPanChanged(usize, f32),
    LoadedSample(Arc<InterleavedAudio>),
    SeekTo(usize),
    SelectClip(usize, usize),
    UnselectClip(),
    CloneClip(usize, usize),
    MoveClipTo(usize, Position),
    TrimClipStart(Position),
    TrimClipEnd(Position),
    DeleteClip(usize, usize),
    PositionScaleDelta(ArrangementPosition, ArrangementScale),
    Export(FileHandle),
}

pub struct Arrangement {
    inner: Arc<ArrangementInner>,
    position: ArrangementPosition,
    scale: ArrangementScale,
    grabbed_clip: Option<[usize; 2]>,
    stream: Stream,
}

impl Arrangement {
    pub fn new(inner: Arc<ArrangementInner>, stream: Stream) -> Self {
        Self {
            inner,
            position: ArrangementPosition::default(),
            scale: ArrangementScale::default(),
            grabbed_clip: None,
            stream,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
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
            Message::SeekTo(pos) => {
                self.inner.meter.sample.store(pos, SeqCst);
            }
            Message::SelectClip(track, clip) => {
                self.grabbed_clip = Some([track, clip]);
            }
            Message::UnselectClip() => self.grabbed_clip = None,
            Message::CloneClip(track, mut clip) => {
                let ok = self.inner.tracks()[track].try_push(&Arc::new(
                    self.inner.tracks()[track].clips()[clip].deref().clone(),
                ));
                debug_assert!(ok);
                clip = self.inner.tracks()[track].clips().len() - 1;
                self.grabbed_clip.replace([track, clip]);
            }
            Message::MoveClipTo(new_track, pos) => {
                let [track, clip] = self.grabbed_clip.as_mut().unwrap();
                if *track != new_track
                    && self.inner.tracks()[new_track]
                        .try_push(&self.inner.tracks()[*track].clips()[*clip])
                {
                    self.inner.tracks()[*track].remove_index(*clip);
                    *track = new_track;
                    *clip = self.inner.tracks()[*track].clips().len() - 1;
                }
                self.inner.tracks()[*track].clips()[*clip].move_to(pos);
            }
            Message::TrimClipStart(pos) => {
                let [track, clip] = self.grabbed_clip.unwrap();
                self.inner.tracks()[track].clips()[clip].trim_start_to(pos);
            }
            Message::TrimClipEnd(pos) => {
                let [track, clip] = self.grabbed_clip.unwrap();
                self.inner.tracks()[track].clips()[clip].trim_end_to(pos);
            }
            Message::DeleteClip(track, clip) => {
                self.inner.tracks()[track].remove_index(clip);
            }
            Message::PositionScaleDelta(pos, scale) => {
                self.position += pos;
                self.position.x = self.position.x.clamp(
                    0.0,
                    self.inner.len().in_interleaved_samples_f(&self.inner.meter),
                );
                self.position.y = self
                    .position
                    .y
                    .clamp(0.0, (self.inner.tracks().len().saturating_sub(1)) as f32);

                self.scale += scale;
                self.scale.x = self.scale.x.clamp(3.0, 12.999_999);
                self.scale.y = self.scale.y.clamp(2.0 * LINE_HEIGHT, 10.0 * LINE_HEIGHT);
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
        ArrangementWidget::new(
            &self.inner,
            self.position,
            self.scale,
            |idx| {
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
            },
            Message::SeekTo,
            Message::SelectClip,
            Message::UnselectClip,
            Message::CloneClip,
            Message::MoveClipTo,
            Message::TrimClipStart,
            Message::TrimClipEnd,
            Message::DeleteClip,
            Message::PositionScaleDelta,
        )
        .into()
    }
}
