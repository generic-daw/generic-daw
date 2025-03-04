use crate::{
    clap_host_view::{ClapHostView, Message as ClapHostMessage},
    components::{round_danger_button, styled_container, styled_svg},
    widget::{
        Arrangement as ArrangementWidget, ArrangementPosition, ArrangementScale,
        AudioClip as AudioClipWidget, Knob, LINE_HEIGHT, PeakMeter, Track as TrackWidget,
    },
};
use fragile::Fragile;
use generic_daw_core::{
    AudioClip, AudioTrack, InterleavedAudio, Meter, MidiTrack, Position,
    audio_graph::{AudioGraph, AudioGraphNodeImpl as _},
    build_output_stream,
    clap_host::{AudioProcessor, GuiExt, MainThreadMessage, PluginId, Receiver},
};
use generic_daw_utils::HoleyVec;
use iced::{
    Alignment, Element, Function as _, Length, Task,
    futures::TryFutureExt as _,
    mouse::Interaction,
    widget::{button, column, mouse_area, radio, row, svg, vertical_space},
    window::Id,
};
use std::{
    path::Path,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::Ordering::{AcqRel, Acquire, Release},
    },
};

mod arrangement;
mod track;
mod track_clip;

pub use arrangement::Arrangement as ArrangementWrapper;
pub use track::Track as TrackWrapper;
pub use track_clip::TrackClip as TrackClipWrapper;

static X: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--cancel-rounded.svg"
    ))
});

static REOPEN: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!(
        "../../assets/material-symbols--reopen-window-rounded.svg"
    ))
});

#[derive(Clone, Debug)]
pub enum Message {
    ClapHost(ClapHostMessage),
    AudioGraph(Arc<Mutex<(AudioGraph, Box<Path>)>>),
    TrackVolumeChanged(usize, f32),
    TrackPanChanged(usize, f32),
    LoadSample(Box<Path>),
    LoadedSample(Option<Arc<InterleavedAudio>>),
    LoadedPlugin(
        Arc<Mutex<AudioProcessor>>,
        Arc<Mutex<(Fragile<GuiExt>, Receiver<MainThreadMessage>)>>,
    ),
    ToggleTrackEnabled(usize),
    ToggleTrackSolo(usize),
    RemoveTrack(usize),
    SeekTo(usize),
    SelectClip(usize, usize),
    UnselectClip(),
    CloneClip(usize, usize),
    MoveClipTo(usize, Position),
    TrimClipStart(Position),
    TrimClipEnd(Position),
    DeleteClip(usize, usize),
    PositionScaleDelta(ArrangementPosition, ArrangementScale),
    Export(Box<Path>),
}

pub struct ArrangementView {
    clap_host: ClapHostView,
    plugin_ids: HoleyVec<PluginId>,

    arrangement: ArrangementWrapper,
    meter: Arc<Meter>,

    loading: usize,

    position: ArrangementPosition,
    scale: ArrangementScale,
    soloed_track: Option<usize>,
    grabbed_clip: Option<[usize; 2]>,
}

impl ArrangementView {
    pub fn create(main_window_id: Id) -> (Arc<Meter>, Self) {
        let (stream, producer, meter) = build_output_stream(44100, 1024);

        let arrangement = ArrangementWrapper::new(producer, stream, meter.clone());

        let arrangement = Self {
            clap_host: ClapHostView::new(main_window_id),
            plugin_ids: HoleyVec::default(),

            arrangement,
            meter: meter.clone(),

            loading: 0,

            position: ArrangementPosition::default(),
            scale: ArrangementScale::default(),
            soloed_track: None,
            grabbed_clip: None,
        };

        (meter, arrangement)
    }

    pub fn stop(&mut self) {
        self.arrangement.stop();
    }

    #[expect(clippy::too_many_lines)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ClapHost(msg) => return self.clap_host.update(msg).map(Message::ClapHost),
            Message::AudioGraph(message) => {
                let (audio_graph, path) =
                    Mutex::into_inner(Arc::into_inner(message).unwrap()).unwrap();
                self.arrangement.export(audio_graph, &path);
            }
            Message::TrackVolumeChanged(track, volume) => {
                self.arrangement.tracks()[track]
                    .node()
                    .volume
                    .store(volume, Release);
            }
            Message::TrackPanChanged(track, pan) => {
                self.arrangement.tracks()[track]
                    .node()
                    .pan
                    .store(pan, Release);
            }
            Message::LoadSample(path) => {
                self.loading += 1;
                let meter = self.meter.clone();
                return Task::future(tokio::task::spawn_blocking(move || {
                    InterleavedAudio::create(&path, meter.sample_rate)
                }))
                .and_then(Task::done)
                .map(Result::ok)
                .map(Message::LoadedSample);
            }
            Message::LoadedSample(audio_file) => {
                self.loading -= 1;
                if let Some(audio_file) = audio_file {
                    let mut track = AudioTrack::new(self.meter.clone());
                    track
                        .clips
                        .push(AudioClip::create(audio_file, self.meter.clone()));
                    self.arrangement.push(track);
                }
            }
            Message::LoadedPlugin(arc, clap_host) => {
                let plugin_id = clap_host.lock().unwrap().0.get().plugin_id();
                let audio_processor = Mutex::into_inner(Arc::into_inner(arc).unwrap()).unwrap();
                let track = MidiTrack::new(self.meter.clone(), audio_processor);
                self.plugin_ids.insert(*track.id(), plugin_id);
                self.arrangement.push(track);

                return self
                    .clap_host
                    .update(ClapHostMessage::Opened(clap_host))
                    .map(Message::ClapHost);
            }
            Message::ToggleTrackEnabled(track) => {
                self.arrangement.tracks()[track]
                    .node()
                    .enabled
                    .fetch_not(AcqRel);
                self.soloed_track = None;
            }
            Message::ToggleTrackSolo(track) => {
                if self.soloed_track == Some(track) {
                    self.soloed_track = None;
                    self.arrangement
                        .tracks()
                        .iter()
                        .for_each(|track| track.node().enabled.store(true, Release));
                } else {
                    self.arrangement
                        .tracks()
                        .iter()
                        .for_each(|track| track.node().enabled.store(false, Release));
                    self.arrangement.tracks()[track]
                        .node()
                        .enabled
                        .store(true, Release);
                    self.soloed_track = Some(track);
                }
            }
            Message::RemoveTrack(track) => {
                let id = self.arrangement.remove(track);
                if let Some(id) = self.plugin_ids.remove(*id) {
                    return self
                        .clap_host
                        .update(ClapHostMessage::Close(id))
                        .map(Message::ClapHost);
                }
            }
            Message::SeekTo(pos) => {
                self.meter.sample.store(pos, Release);
            }
            Message::SelectClip(track, clip) => {
                self.grabbed_clip = Some([track, clip]);
            }
            Message::UnselectClip() => self.grabbed_clip = None,
            Message::CloneClip(track, mut clip) => {
                self.arrangement.clone_clip(track, clip);
                clip = self.arrangement.tracks()[track].clips().len() - 1;
                self.grabbed_clip.replace([track, clip]);
            }
            Message::MoveClipTo(new_track, pos) => {
                let [track, clip] = self.grabbed_clip.as_mut().unwrap();

                if *track != new_track
                    && self.arrangement.clip_switch_track(*track, *clip, new_track)
                {
                    *track = new_track;
                    *clip = self.arrangement.tracks()[*track].clips().len() - 1;
                }

                self.arrangement.tracks()[*track]
                    .get_clip(*clip)
                    .move_to(pos);
            }
            Message::TrimClipStart(pos) => {
                let [track, clip] = self.grabbed_clip.unwrap();
                self.arrangement.tracks()[track]
                    .get_clip(clip)
                    .trim_start_to(pos);
            }
            Message::TrimClipEnd(pos) => {
                let [track, clip] = self.grabbed_clip.unwrap();
                self.arrangement.tracks()[track]
                    .get_clip(clip)
                    .trim_end_to(pos);
            }
            Message::DeleteClip(track, clip) => {
                self.arrangement.delete_clip(track, clip);
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
                            .tracks()
                            .iter()
                            .map(TrackWrapper::len)
                            .max()
                            .unwrap_or_default()
                            .in_interleaved_samples_f(
                                self.meter.bpm.load(Acquire),
                                self.meter.sample_rate,
                            ),
                        self.arrangement.tracks().len().saturating_sub(1) as f32,
                    );
                }
            }
            Message::Export(path) => {
                return Task::future(self.arrangement.request_export().map_ok(|ok| (ok, path)))
                    .and_then(Task::done)
                    .map(Mutex::new)
                    .map(Arc::new)
                    .map(Message::AudioGraph);
            }
        }

        Task::none()
    }

    #[expect(clippy::too_many_lines)]
    pub fn view(&self) -> Element<'_, Message> {
        let arrangement = ArrangementWidget::new(
            &self.arrangement.meter,
            self.position,
            self.scale,
            column(
                self.arrangement
                    .tracks()
                    .iter()
                    .enumerate()
                    .map(|(idx, track)| {
                        let node = track.node().clone();
                        let enabled = node.enabled.load(Acquire);

                        let mut buttons = column![
                            mouse_area(
                                radio("", enabled, Some(true), |_| {
                                    Message::ToggleTrackEnabled(idx)
                                })
                                .spacing(0.0)
                            )
                            .on_right_press(Message::ToggleTrackSolo(idx)),
                            vertical_space(),
                            round_danger_button(styled_svg(X.clone()).height(LINE_HEIGHT))
                                .padding(0.0)
                                .on_press(Message::RemoveTrack(idx)),
                        ]
                        .spacing(5.0)
                        .align_x(Alignment::Center);

                        if let Some(&id) = self.plugin_ids.get(*track.id()) {
                            buttons = buttons.push(
                                button(styled_svg(REOPEN.clone()).height(LINE_HEIGHT))
                                    .padding(0.0)
                                    .on_press(Message::ClapHost(ClapHostMessage::MainThread(
                                        id,
                                        MainThreadMessage::GuiRequestShow,
                                    ))),
                            );
                        }

                        row![
                            styled_container(
                                row![
                                    PeakMeter::new(move || node.get_l_r(), enabled),
                                    column![
                                        mouse_area(Knob::new(
                                            0.0..=1.0,
                                            0.0,
                                            track.node().volume.load(Acquire),
                                            enabled,
                                            Message::TrackVolumeChanged.with(idx)
                                        ))
                                        .on_double_click(Message::TrackVolumeChanged(idx, 1.0)),
                                        mouse_area(Knob::new(
                                            -1.0..=1.0,
                                            0.0,
                                            track.node().pan.load(Acquire),
                                            enabled,
                                            Message::TrackPanChanged.with(idx)
                                        ))
                                        .on_double_click(Message::TrackPanChanged(idx, 0.0)),
                                        vertical_space(),
                                    ]
                                    .spacing(5.0),
                                    buttons,
                                ]
                                .spacing(5.0),
                            )
                            .padding(5.0)
                            .height(Length::Fixed(self.scale.y)),
                            TrackWidget::new(
                                track.clips().map(|clip| match clip {
                                    TrackClipWrapper::AudioClip(clip) => {
                                        AudioClipWidget::new(
                                            clip,
                                            self.position,
                                            self.scale,
                                            enabled,
                                        )
                                        .into()
                                    }
                                    TrackClipWrapper::MidiClip(_) => unimplemented!(),
                                }),
                                self.scale,
                            )
                        ]
                    })
                    .map(Element::new),
            ),
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
        .into();

        if self.loading > 0 {
            mouse_area(arrangement)
                .interaction(Interaction::Progress)
                .into()
        } else {
            arrangement
        }
    }

    pub fn title(&self, window: Id) -> Option<String> {
        self.clap_host.title(window)
    }
}
