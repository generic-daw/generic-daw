use crate::widget::{
    Arrangement as ArrangementWidget, ArrangementPosition, ArrangementScale, Knob, PeakMeter,
};
use generic_daw_core::{
    audio_graph::{AudioGraph, AudioGraphNodeImpl as _, MixerNode},
    build_output_stream,
    cpal::{traits::StreamTrait as _, Stream},
    rtrb::Producer,
    AudioClip, AudioCtxMessage, InterleavedAudio, Meter, Position, Track, UiMessage,
};
use hound::WavWriter;
use iced::{
    futures::SinkExt as _,
    stream::channel,
    widget::{column, container, container::Style, mouse_area, radio, row},
    Border, Element, Task,
};
use rfd::FileHandle;
use std::{
    ops::Deref as _,
    path::Path,
    sync::{
        atomic::Ordering::{AcqRel, Acquire, Release},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Clone, Debug)]
pub enum Message {
    Animate(),
    Ui(Arc<Mutex<UiMessage<FileHandle>>>),
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
    tracks: Vec<Track>,
    meter: Arc<Meter>,
    producer: Producer<AudioCtxMessage<FileHandle>>,
    stream: Stream,

    position: ArrangementPosition,
    scale: ArrangementScale,
    soloed_track: Option<usize>,
    grabbed_clip: Option<[usize; 2]>,
}

impl Arrangement {
    pub fn create() -> (Arc<Meter>, Self, Task<Message>) {
        let (stream, producer, mut consumer, meter) = build_output_stream();

        let arrangement = Self {
            tracks: Vec::new(),
            meter: meter.clone(),
            producer,
            stream,
            position: ArrangementPosition::default(),
            scale: ArrangementScale::default(),
            soloed_track: None,
            grabbed_clip: None,
        };

        let task = Task::stream(channel(16, move |mut sender| async move {
            loop {
                if let Ok(msg) = consumer.pop() {
                    sender.send(msg).await.unwrap();
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }))
        .map(Mutex::new)
        .map(Arc::new)
        .map(Message::Ui);

        (meter, arrangement, task)
    }

    #[expect(clippy::too_many_lines)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Animate() => {}
            Message::Ui(message) => {
                let message = Mutex::into_inner(Arc::into_inner(message).unwrap()).unwrap();
                match message {
                    UiMessage::AudioGraph(path, audio_graph) => {
                        self.export(path.path(), audio_graph);
                    }
                }
            }
            Message::TrackVolumeChanged(track, volume) => {
                self.tracks[track].node.volume.store(volume, Release);
            }
            Message::TrackPanChanged(track, pan) => {
                self.tracks[track].node.pan.store(pan, Release);
            }
            Message::LoadedSample(audio_file) => {
                let mut track = Track::audio(self.meter.clone(), Arc::new(MixerNode::default()));

                track
                    .clips
                    .push(AudioClip::create(audio_file, self.meter.clone()));
                self.tracks.push(track.clone());

                let id = track.id();
                self.producer
                    .push(AudioCtxMessage::Insert(track.into()))
                    .unwrap();
                self.producer
                    .push(AudioCtxMessage::ConnectToMaster(id))
                    .unwrap();
            }
            Message::ToggleTrackEnabled(track) => {
                self.tracks[track].node.enabled.fetch_not(AcqRel);
                self.soloed_track = None;
            }
            Message::ToggleTrackSolo(track) => {
                if self.soloed_track.is_some_and(|s| s == track) {
                    self.soloed_track = None;
                    self.tracks
                        .iter()
                        .for_each(|track| track.node.enabled.store(true, Release));
                } else {
                    self.tracks
                        .iter()
                        .for_each(|track| track.node.enabled.store(false, Release));
                    self.tracks[track].node.enabled.store(true, Release);
                    self.soloed_track = Some(track);
                }
            }
            Message::SeekTo(pos) => {
                self.meter.sample.store(pos, Release);
            }
            Message::SelectClip(track, clip) => {
                self.grabbed_clip = Some([track, clip]);
            }
            Message::UnselectClip() => self.grabbed_clip = None,
            Message::CloneClip(track, mut clip_idx) => {
                let clip = self.tracks[track].clips[clip_idx].deref().clone();
                self.tracks[track].clips.push(Arc::new(clip));

                self.producer
                    .push(AudioCtxMessage::Insert(self.tracks[track].clone().into()))
                    .unwrap();

                clip_idx = self.tracks[track].clips.len() - 1;
                self.grabbed_clip.replace([track, clip_idx]);
            }
            Message::MoveClipTo(new_track, pos) => {
                let [track, clip] = self.grabbed_clip.as_mut().unwrap();
                let inner = self.tracks[*track].clips[*clip].clone();

                if *track != new_track && self.tracks[new_track].try_push(&inner) {
                    self.tracks[*track].clips.remove(*clip);

                    self.producer
                        .push(AudioCtxMessage::Insert(self.tracks[*track].clone().into()))
                        .unwrap();
                    self.producer
                        .push(AudioCtxMessage::Insert(
                            self.tracks[new_track].clone().into(),
                        ))
                        .unwrap();

                    *track = new_track;
                    *clip = self.tracks[*track].clips.len() - 1;
                }

                self.tracks[*track].clips[*clip].move_to(pos);
            }
            Message::TrimClipStart(pos) => {
                let [track, clip] = self.grabbed_clip.unwrap();
                self.tracks[track].clips[clip].trim_start_to(pos);
            }
            Message::TrimClipEnd(pos) => {
                let [track, clip] = self.grabbed_clip.unwrap();
                self.tracks[track].clips[clip].trim_end_to(pos);
            }
            Message::DeleteClip(track, clip) => {
                self.tracks[track].clips.remove(clip);
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
                        self.tracks
                            .iter()
                            .map(Track::len)
                            .max()
                            .unwrap_or_default()
                            .in_interleaved_samples_f(&self.meter),
                        (self.tracks.len().saturating_sub(1)) as f32,
                    );
                }
            }
            Message::Export(path) => {
                self.producer
                    .push(AudioCtxMessage::RequestAudioGraph(path))
                    .unwrap();
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        ArrangementWidget::new(
            &self.tracks,
            &self.meter,
            self.position,
            self.scale,
            |track, enabled| {
                let left = self.tracks[track].node.max_l.swap(0.0, AcqRel);
                let right = self.tracks[track].node.max_r.swap(0.0, AcqRel);

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

    fn export(&mut self, path: &Path, mut audio_graph: AudioGraph) {
        const CHUNK_SIZE: usize = 64;

        self.stream.pause().unwrap();

        let playing = self.meter.playing.swap(true, AcqRel);
        let metronome = self.meter.metronome.swap(false, AcqRel);

        let mut writer = WavWriter::create(
            path,
            hound::WavSpec {
                channels: 2,
                sample_rate: self.meter.sample_rate.load(Acquire),
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();

        let mut buf = [0.0; CHUNK_SIZE];

        let len = self.tracks.iter().map(Track::len).max().unwrap_or_default();
        let len = len.in_interleaved_samples(&self.meter);

        for i in (0..len).step_by(CHUNK_SIZE) {
            audio_graph.fill_buf(i, &mut buf);

            for s in buf {
                writer.write_sample(s).unwrap();
            }
        }

        writer.finalize().unwrap();

        self.meter.playing.store(playing, Release);
        self.meter.metronome.store(metronome, Release);

        self.producer
            .push(AudioCtxMessage::AudioGraph(audio_graph))
            .unwrap();

        self.stream.play().unwrap();
    }
}
