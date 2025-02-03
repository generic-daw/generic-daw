use crate::widget::{
    Arrangement as ArrangementWidget, ArrangementPosition, ArrangementScale, Knob, PeakMeter,
};
use generic_daw_core::{
    audio_graph::AudioGraphNode, build_output_stream, rtrb::Producer,
    Arrangement as ArrangementInner, AudioClip, AudioCtxMessage, InterleavedAudio, Meter, Position,
    Stream, StreamTrait as _, Track,
};
use iced::{
    widget::{column, container, container::Style, mouse_area, radio, row},
    Border, Element, Task,
};
use rfd::FileHandle;
use std::{
    ops::Deref as _,
    sync::{
        atomic::Ordering::{AcqRel, Release},
        Arc,
    },
};

#[derive(Clone, Debug)]
pub enum Message {
    Animate(),
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
    arrangement: Arc<ArrangementInner>,
    producer: Producer<AudioCtxMessage>,
    stream: Stream,

    position: ArrangementPosition,
    scale: ArrangementScale,
    soloed_track: Option<usize>,
    grabbed_clip: Option<[usize; 2]>,
}

impl Arrangement {
    pub fn create() -> (Arc<Meter>, Self) {
        let (stream, producer, arrangement) = build_output_stream();

        (
            arrangement.meter.clone(),
            Self {
                arrangement,
                producer,
                stream,
                position: ArrangementPosition::default(),
                scale: ArrangementScale::default(),
                soloed_track: None,
                grabbed_clip: None,
            },
        )
    }

    #[expect(clippy::too_many_lines)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Animate() => {}
            Message::TrackVolumeChanged(track, volume) => {
                self.arrangement.tracks.read().unwrap()[track]
                    .node
                    .volume
                    .store(volume, Release);
            }
            Message::TrackPanChanged(track, pan) => {
                self.arrangement.tracks.read().unwrap()[track]
                    .node
                    .pan
                    .store(pan, Release);
            }
            Message::LoadedSample(audio_file) => {
                let track = Track::audio(self.arrangement.meter.clone());

                track.clips.write().unwrap().push(AudioClip::create(
                    audio_file,
                    self.arrangement.meter.clone(),
                ));
                self.arrangement.tracks.write().unwrap().push(track.clone());

                let node: AudioGraphNode = track.into();
                self.producer
                    .push(AudioCtxMessage::Add(node.clone()))
                    .unwrap();
                self.producer
                    .push(AudioCtxMessage::Connect(
                        self.arrangement.clone().into(),
                        node,
                    ))
                    .unwrap();
            }
            Message::ToggleTrackEnabled(track) => {
                self.arrangement.tracks.read().unwrap()[track]
                    .node
                    .enabled
                    .fetch_not(AcqRel);
                self.soloed_track = None;
            }
            Message::ToggleTrackSolo(track) => {
                if self.soloed_track.is_some_and(|s| s == track) {
                    self.soloed_track = None;
                    self.arrangement
                        .tracks
                        .read()
                        .unwrap()
                        .iter()
                        .for_each(|track| track.node.enabled.store(true, Release));
                } else {
                    self.arrangement
                        .tracks
                        .read()
                        .unwrap()
                        .iter()
                        .for_each(|track| track.node.enabled.store(false, Release));
                    self.arrangement.tracks.read().unwrap()[track]
                        .node
                        .enabled
                        .store(true, Release);
                    self.soloed_track = Some(track);
                }
            }
            Message::SeekTo(pos) => {
                self.arrangement.meter.sample.store(pos, Release);
            }
            Message::SelectClip(track, clip) => {
                self.grabbed_clip = Some([track, clip]);
            }
            Message::UnselectClip() => self.grabbed_clip = None,
            Message::CloneClip(track, mut clip_idx) => {
                let clip = self.arrangement.tracks.read().unwrap()[track]
                    .clips
                    .read()
                    .unwrap()[clip_idx]
                    .deref()
                    .clone();
                self.arrangement.tracks.read().unwrap()[track]
                    .clips
                    .write()
                    .unwrap()
                    .push(Arc::new(clip));
                clip_idx = self.arrangement.tracks.read().unwrap()[track]
                    .clips
                    .read()
                    .unwrap()
                    .len()
                    - 1;
                self.grabbed_clip.replace([track, clip_idx]);
            }
            Message::MoveClipTo(new_track, pos) => {
                let [track, clip] = self.grabbed_clip.as_mut().unwrap();
                if *track != new_track
                    && self.arrangement.tracks.read().unwrap()[new_track].try_push(
                        &self.arrangement.tracks.read().unwrap()[*track]
                            .clips
                            .read()
                            .unwrap()[*clip],
                    )
                {
                    self.arrangement.tracks.read().unwrap()[*track]
                        .clips
                        .write()
                        .unwrap()
                        .remove(*clip);
                    *track = new_track;
                    *clip = self.arrangement.tracks.read().unwrap()[*track]
                        .clips
                        .read()
                        .unwrap()
                        .len()
                        - 1;
                }
                self.arrangement.tracks.read().unwrap()[*track]
                    .clips
                    .read()
                    .unwrap()[*clip]
                    .move_to(pos);
            }
            Message::TrimClipStart(pos) => {
                let [track, clip] = self.grabbed_clip.unwrap();
                self.arrangement.tracks.read().unwrap()[track]
                    .clips
                    .read()
                    .unwrap()[clip]
                    .trim_start_to(pos);
            }
            Message::TrimClipEnd(pos) => {
                let [track, clip] = self.grabbed_clip.unwrap();
                self.arrangement.tracks.read().unwrap()[track]
                    .clips
                    .read()
                    .unwrap()[clip]
                    .trim_end_to(pos);
            }
            Message::DeleteClip(track, clip) => {
                self.arrangement.tracks.read().unwrap()[track]
                    .clips
                    .write()
                    .unwrap()
                    .remove(clip);
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
                        self.arrangement
                            .len()
                            .in_interleaved_samples_f(&self.arrangement.meter),
                        (self
                            .arrangement
                            .tracks
                            .read()
                            .unwrap()
                            .len()
                            .saturating_sub(1)) as f32,
                    );
                }
            }
            Message::Export(path) => {
                self.stream.pause().unwrap();
                self.arrangement.export(path.path());
                self.stream.play().unwrap();
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        ArrangementWidget::new(
            &self.arrangement,
            self.position,
            self.scale,
            |track, enabled| {
                let left = self.arrangement.tracks.read().unwrap()[track]
                    .node
                    .max_l
                    .swap(0.0, AcqRel);
                let right = self.arrangement.tracks.read().unwrap()[track]
                    .node
                    .max_r
                    .swap(0.0, AcqRel);

                container(
                    row![
                        PeakMeter::new(left, right, enabled, Message::Animate),
                        column![
                            Knob::new(0.0..=1.0, 0.0, 1.0, move |f| {
                                Message::TrackVolumeChanged(track, f)
                            })
                            .set_enabled(enabled),
                            Knob::new(-1.0..=1.0, 0.0, 0.0, move |f| Message::TrackPanChanged(
                                track, f
                            ))
                            .set_enabled(enabled),
                        ]
                        .spacing(5.0),
                        mouse_area(
                            radio("", enabled, Some(true), |_| {
                                Message::ToggleTrackEnabled(track)
                            })
                            .spacing(0.0)
                        )
                        .on_right_press(Message::ToggleTrackSolo(track)),
                    ]
                    .spacing(5.0),
                )
                .padding(5.0)
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
                    border: Border::default()
                        .width(1.0)
                        .color(theme.extended_palette().secondary.weak.color),
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
