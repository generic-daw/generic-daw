use crate::widget::{
    Arrangement as ArrangementWidget, ArrangementPosition, ArrangementScale, Knob,
};
use generic_daw_core::{
    Arrangement as ArrangementInner, AudioClip, AudioTrack, InterleavedAudio, Position, Stream,
    StreamTrait as _,
};
use iced::{
    widget::{
        column, container, container::Style as ContainerStyle, horizontal_space, mouse_area, radio,
        row, vertical_space,
    },
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
    ToggleTrackEnabled(usize),
    ToggleTrackSolo(usize),
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
    soloed_track: Option<usize>,
    grabbed_clip: Option<[usize; 2]>,
    stream: Stream,
}

impl Arrangement {
    pub fn new(inner: Arc<ArrangementInner>, stream: Stream) -> Self {
        Self {
            inner,
            position: ArrangementPosition::default(),
            scale: ArrangementScale::default(),
            soloed_track: None,
            grabbed_clip: None,
            stream,
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TrackVolumeChanged(track, volume) => {
                self.inner.tracks()[track].set_volume(volume);
            }
            Message::TrackPanChanged(track, pan) => {
                self.inner.tracks()[track].set_pan(pan);
            }
            Message::LoadedSample(audio_file) => {
                let track = AudioTrack::create(self.inner.meter.clone());

                let mut ok = true;
                ok &= track.try_push(&AudioClip::create(audio_file, self.inner.meter.clone()));
                self.inner.tracks.write().unwrap().push(track.clone());

                let node = track.into();
                ok &= self.inner.audio_graph.add(&node);
                ok &= self
                    .inner
                    .audio_graph
                    .connect(&self.inner.audio_graph.root(), &node);

                debug_assert!(ok);
            }
            Message::ToggleTrackEnabled(track) => {
                self.inner.tracks()[track].toggle_enabled();
                self.soloed_track = None;
            }
            Message::ToggleTrackSolo(track) => {
                if self.soloed_track.is_some_and(|s| s == track) {
                    self.soloed_track = None;
                    self.inner
                        .tracks()
                        .iter()
                        .for_each(|track| track.set_enabled(true));
                } else {
                    self.inner
                        .tracks()
                        .iter()
                        .for_each(|track| track.set_enabled(false));
                    self.inner.tracks()[track].set_enabled(true);
                    self.soloed_track = Some(track);
                }
            }
            Message::SeekTo(pos) => {
                self.inner.meter.sample.store(pos, SeqCst);
            }
            Message::SelectClip(track, clip) => {
                self.grabbed_clip = Some([track, clip]);
            }
            Message::UnselectClip() => self.grabbed_clip = None,
            Message::CloneClip(track, mut clip) => {
                let inner = self.inner.tracks()[track].clips()[clip].deref().clone();
                let ok = self.inner.tracks()[track].try_push(&Arc::new(inner));
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
                let sd = scale != ArrangementScale::ZERO;
                let mut pd = pos != ArrangementPosition::ZERO;

                if sd {
                    let old_scale = self.scale;
                    self.scale += scale;
                    self.scale = self.scale.clamp();
                    pd &= old_scale != self.scale;
                }

                if pd {
                    self.position += pos;
                    self.position = self.position.clamp(
                        self.inner.len().in_interleaved_samples_f(&self.inner.meter),
                        (self.inner.tracks().len().saturating_sub(1)) as f32,
                    );
                }
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
            |idx, enabled| {
                container(
                    column![
                        row![
                            Knob::new(0.0..=1.0, 0.0, 1.0, move |f| Message::TrackVolumeChanged(
                                idx, f
                            ))
                            .set_enabled(enabled),
                            Knob::new(-1.0..=1.0, 0.0, 0.0, move |f| Message::TrackPanChanged(
                                idx, f
                            ))
                            .set_enabled(enabled)
                        ]
                        .spacing(5.0),
                        vertical_space(),
                        row![
                            horizontal_space(),
                            mouse_area(radio("", enabled, Some(true), |_| {
                                Message::ToggleTrackEnabled(idx)
                            }))
                            .on_right_press(Message::ToggleTrackSolo(idx)),
                        ]
                    ]
                    .width(Length::Shrink)
                    .spacing(5.0),
                )
                .padding(5.0)
                .height(Length::Fill)
                .style(|theme| ContainerStyle {
                    background: Some(
                        theme
                            .extended_palette()
                            .secondary
                            .weak
                            .color
                            .scale_alpha(0.25)
                            .into(),
                    ),
                    ..ContainerStyle::default()
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
