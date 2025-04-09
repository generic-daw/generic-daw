use crate::{
    clap_host::{ClapHost, Message as ClapHostMessage},
    components::{
        char_button, empty_widget, styled_combo_box, styled_scrollable_with_direction, styled_svg,
    },
    icons::{ADD, CHEVRON_RIGHT, HANDLE},
    stylefns::{button_with_base, slider_with_enabled, svg_with_enabled},
    widget::{
        AnimatedDot, Arrangement as ArrangementWidget, AudioClip as AudioClipWidget, Knob,
        LINE_HEIGHT, MidiClip as MidiClipWidget, PeakMeter, Piano, PianoRoll,
        Recording as RecordingWidget, Seeker, TEXT_HEIGHT, Track as TrackWidget, VSplit,
        arrangement::Action as ArrangementAction, piano_roll::Action as PianoRollAction,
        vsplit::Strategy,
    },
};
use arc_swap::ArcSwap;
use arrangement::NodeType;
use dragking::DragEvent;
use fragile::Fragile;
use generic_daw_core::{
    AudioClip, Clip, InterleavedAudio, Meter, MidiClip, MidiKey, MidiNote, MixerNode, Position,
    Recording, Track,
    audio_graph::{NodeId, NodeImpl as _},
    clap_host::{
        self, MainThreadMessage, PluginBundle, PluginDescriptor, PluginId, get_installed_plugins,
    },
};
use generic_daw_project::{proto, reader::Reader, writer::Writer};
use generic_daw_utils::{EnumDispatcher, HoleyVec, ShiftMoveExt as _, Vec2};
use iced::{
    Alignment, Element, Function as _, Length, Radians, Subscription, Task, Theme, border,
    mouse::Interaction,
    padding,
    widget::{
        button, column, combo_box, container, horizontal_rule, mouse_area, responsive, row,
        scrollable::{Direction, Scrollbar},
        svg, text,
        text::Wrapping,
        vertical_rule, vertical_slider, vertical_space,
    },
};
use log::info;
use smol::unblock;
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet, HashMap},
    f32::{self, consts::FRAC_PI_2},
    fs::File,
    hash::{DefaultHasher, Hash as _, Hasher as _},
    io::{Read as _, Write as _},
    iter::once,
    ops::Deref as _,
    path::Path,
    sync::{
        Arc, Mutex, Weak,
        atomic::Ordering::{AcqRel, Acquire, Release},
        mpsc,
    },
    time::Instant,
};

mod arrangement;

pub use arrangement::Arrangement as ArrangementWrapper;

#[derive(Clone, Debug)]
enum LoadStatus {
    Loading(usize),
    Loaded(Weak<InterleavedAudio>),
}

#[derive(Clone, Debug)]
pub enum Message {
    ClapHost(ClapHostMessage),

    ConnectRequest((NodeId, NodeId)),
    ConnectSucceeded((NodeId, NodeId)),
    Disconnect((NodeId, NodeId)),
    Export(Box<Path>),

    ChannelAdd,
    ChannelRemove(NodeId),
    ChannelSelect(NodeId),
    ChannelVolumeChanged(NodeId, f32),
    ChannelPanChanged(NodeId, f32),
    ChannelToggleEnabled(NodeId),

    PluginLoad(PluginDescriptor),
    PluginRemove(usize),
    PluginMixChanged(usize, f32),
    PluginToggleEnabled(usize),
    PluginsReordered(DragEvent),

    SampleLoadFromFile(Arc<Path>),
    SampleLoadedFromFile(Arc<Path>, Option<Arc<InterleavedAudio>>),

    AddMidiClip(NodeId, Position),
    OpenMidiClip(Arc<MidiClip>),

    TrackAdd,
    TrackRemove(NodeId),
    TrackToggleEnabled(NodeId),
    TrackToggleSolo(NodeId),

    SeekTo(Position),

    ToggleRecord(NodeId),
    RecordingSplit(NodeId),
    RecordingChunk(Box<[f32]>),
    StopRecord,

    ArrangementAction(ArrangementAction),
    ArrangementPositionScaleDelta(Vec2, Vec2),

    PianoRollAction(PianoRollAction),
    PianoRollPositionScaleDelta(Vec2, Vec2),

    SplitAt(f32),
}

#[derive(Clone, Debug)]
pub enum Tab {
    Arrangement {
        grabbed_clip: Option<[usize; 2]>,
    },
    Mixer,
    PianoRoll {
        clip: Arc<MidiClip>,
        grabbed_note: Option<usize>,
    },
}

pub struct ArrangementView {
    pub clap_host: ClapHost,
    plugin_bundles: BTreeMap<PluginDescriptor, PluginBundle>,
    plugin_descriptors: combo_box::State<PluginDescriptor>,

    arrangement: ArrangementWrapper,
    meter: Arc<Meter>,
    loading: BTreeSet<Arc<Path>>,
    audios: BTreeMap<Arc<Path>, LoadStatus>,
    midis: Vec<Weak<ArcSwap<Vec<MidiNote>>>>,
    plugins_by_channel: HoleyVec<Vec<(PluginId, PluginDescriptor)>>,

    tab: Tab,

    recording: Option<(Recording, NodeId)>,

    arrangement_position: Vec2,
    arrangement_scale: Vec2,
    soloed_track: Option<NodeId>,

    piano_roll_position: Cell<Vec2>,
    piano_roll_scale: Vec2,
    last_note_len: Position,
    selected_channel: Option<NodeId>,

    split_at: f32,
}

impl ArrangementView {
    pub fn create() -> (Self, Arc<Meter>) {
        let (arrangement, meter) = ArrangementWrapper::create();

        let plugin_bundles = get_installed_plugins();
        let plugin_descriptors = combo_box::State::new(plugin_bundles.keys().cloned().collect());

        (
            Self {
                clap_host: ClapHost::default(),
                plugin_bundles,
                plugin_descriptors,

                arrangement,
                meter: meter.clone(),
                loading: BTreeSet::new(),
                audios: BTreeMap::new(),
                midis: Vec::new(),
                plugins_by_channel: HoleyVec::default(),

                tab: Tab::Arrangement { grabbed_clip: None },

                recording: None,

                arrangement_position: Vec2::default(),
                arrangement_scale: Vec2::new(9.0, 120.0),
                soloed_track: None,

                piano_roll_position: Cell::new(Vec2::new(0.0, 40.0)),
                piano_roll_scale: Vec2::new(9.0, LINE_HEIGHT),
                last_note_len: Position::BEAT,
                selected_channel: None,

                split_at: 300.0,
            },
            meter,
        )
    }

    pub fn stop(&self) {
        self.arrangement.stop();
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ClapHost(msg) => return self.clap_host.update(msg).map(Message::ClapHost),
            Message::ConnectRequest((from, to)) => {
                return Task::perform(self.arrangement.request_connect(from, to), |con| {
                    Message::ConnectSucceeded(con.unwrap())
                });
            }
            Message::ConnectSucceeded((from, to)) => self.arrangement.connect_succeeded(from, to),
            Message::Disconnect((from, to)) => self.arrangement.disconnect(from, to),
            Message::Export(path) => {
                self.clap_host.set_realtime(false);
                self.arrangement.export(&path);
                self.clap_host.set_realtime(true);
            }
            Message::ChannelAdd => {
                return Task::perform(self.arrangement.add_channel(), |con| {
                    Message::ConnectSucceeded(con.unwrap())
                });
            }
            Message::ChannelRemove(id) => {
                self.arrangement.remove_channel(id);

                if self.selected_channel == Some(id) {
                    self.selected_channel = None;
                }

                if let Some(effects) = self.plugins_by_channel.remove(*id) {
                    return Task::batch(effects.into_iter().map(|(id, _)| {
                        self.clap_host
                            .update(ClapHostMessage::MainThread(
                                id,
                                MainThreadMessage::GuiClosed,
                            ))
                            .map(Message::ClapHost)
                    }));
                }
            }
            Message::ChannelSelect(id) => {
                self.selected_channel = if self.selected_channel == Some(id) {
                    None
                } else {
                    self.plugins_by_channel.entry(*id).get_or_insert_default();
                    Some(id)
                };
            }
            Message::ChannelVolumeChanged(id, volume) => {
                self.arrangement.node(id).0.volume.store(volume, Release);
            }
            Message::ChannelPanChanged(id, pan) => {
                self.arrangement.node(id).0.pan.store(pan, Release);
            }
            Message::ChannelToggleEnabled(id) => {
                self.arrangement.node(id).0.enabled.fetch_not(AcqRel);
            }
            Message::PluginLoad(descriptor) => {
                let Some(selected) = self.selected_channel else {
                    panic!()
                };

                let (gui, receiver, audio_processor) = clap_host::init(
                    &self.plugin_bundles[&descriptor],
                    descriptor.clone(),
                    f64::from(self.meter.sample_rate),
                    self.meter.buffer_size,
                );

                let id = audio_processor.id();
                self.arrangement
                    .node(selected)
                    .0
                    .add_plugin(audio_processor);
                self.plugins_by_channel
                    .get_mut(*selected)
                    .unwrap()
                    .push((id, descriptor));

                return self
                    .clap_host
                    .update(ClapHostMessage::Opened(Arc::new(Mutex::new((
                        Fragile::new(gui),
                        receiver,
                    )))))
                    .map(Message::ClapHost);
            }
            Message::PluginMixChanged(i, mix) => {
                let selected = self.selected_channel.unwrap();
                self.arrangement.node(selected).0.set_plugin_mix(i, mix);
            }
            Message::PluginToggleEnabled(i) => {
                let selected = self.selected_channel.unwrap();
                self.arrangement.node(selected).0.toggle_plugin_enabled(i);
            }
            Message::PluginsReordered(event) => {
                if let DragEvent::Dropped {
                    index,
                    target_index,
                } = event
                {
                    if index != target_index {
                        let selected = self.selected_channel.unwrap();

                        self.arrangement
                            .node(selected)
                            .0
                            .shift_move(index, target_index);
                        self.plugins_by_channel
                            .get_mut(*selected)
                            .unwrap()
                            .shift_move(index, target_index);
                    }
                }
            }
            Message::PluginRemove(i) => {
                let selected = self.selected_channel.unwrap();
                self.arrangement.node(selected).0.remove_plugin(i);
                let id = self
                    .plugins_by_channel
                    .get_mut(*selected)
                    .unwrap()
                    .remove(i)
                    .0;
                return self
                    .clap_host
                    .update(ClapHostMessage::MainThread(
                        id,
                        MainThreadMessage::GuiClosed,
                    ))
                    .map(Message::ClapHost);
            }
            Message::SampleLoadFromFile(path) => {
                if let Some(entry) = self.audios.get_mut(&path) {
                    match entry {
                        LoadStatus::Loading(count) => {
                            *count += 1;

                            return Task::none();
                        }
                        LoadStatus::Loaded(audio) => {
                            if let Some(audio) = audio.upgrade() {
                                return self
                                    .update(Message::SampleLoadedFromFile(path, Some(audio)));
                            }
                        }
                    }
                }

                self.audios.insert(path.clone(), LoadStatus::Loading(1));
                self.loading.insert(path.clone());
                let sample_rate = self.meter.sample_rate;

                return Task::perform(
                    {
                        let path = path.clone();
                        unblock(move || InterleavedAudio::create(path, sample_rate))
                    },
                    Message::SampleLoadedFromFile.with(path),
                );
            }
            Message::SampleLoadedFromFile(path, audio) => {
                self.loading.remove(&path);

                if let Some(audio) = audio {
                    let count = match self.audios[&audio.path] {
                        LoadStatus::Loading(count) => {
                            self.audios.insert(
                                audio.path.clone(),
                                LoadStatus::Loaded(Arc::downgrade(&audio)),
                            );

                            count
                        }
                        LoadStatus::Loaded(..) => 1,
                    };

                    let clip = AudioClip::create(audio, self.meter.clone());
                    let end = clip.position.get_global_end();

                    let mut futs = Vec::new();
                    let mut track = 0;

                    for _ in 0..count {
                        while self.arrangement.tracks().get(track).is_some_and(|track| {
                            track
                                .clips
                                .iter()
                                .any(|clip| clip.position().get_global_start() < end)
                        }) {
                            track += 1;
                        }

                        if track == self.arrangement.tracks().len() {
                            futs.push(Task::perform(self.arrangement.add_track(), |con| {
                                Message::ConnectSucceeded(con.unwrap())
                            }));
                        }

                        self.arrangement.add_clip(track, clip.clone());
                    }

                    return Task::batch(futs);
                }
            }
            Message::OpenMidiClip(clip) => {
                self.tab = Tab::PianoRoll {
                    clip,
                    grabbed_note: None,
                }
            }
            Message::AddMidiClip(track, pos) => {
                let pattern = Arc::default();
                self.midis.push(Arc::downgrade(&pattern));
                let clip = MidiClip::create(pattern, self.meter.clone());
                clip.position
                    .trim_end_to(Position::BEAT * self.meter.numerator.load(Acquire) as u32);
                clip.position.move_to(pos);
                let track = self.arrangement.track_of(track).unwrap();
                self.arrangement.add_clip(track, clip);
            }
            Message::TrackAdd => {
                return Task::perform(self.arrangement.add_track(), |con| {
                    Message::ConnectSucceeded(con.unwrap())
                });
            }
            Message::TrackRemove(id) => {
                let track = self.arrangement.track_of(id).unwrap();
                self.arrangement.remove_track(track);

                if self.recording.as_ref().is_some_and(|&(_, i)| i == id) {
                    self.recording = None;
                }

                return self.update(Message::ChannelRemove(id));
            }
            Message::TrackToggleEnabled(id) => {
                self.soloed_track = None;
                return self.update(Message::ChannelToggleEnabled(id));
            }
            Message::TrackToggleSolo(id) => {
                if self.soloed_track == Some(id) {
                    self.soloed_track = None;
                    self.arrangement
                        .tracks()
                        .iter()
                        .for_each(|track| track.node.enabled.store(true, Release));
                } else {
                    self.arrangement
                        .tracks()
                        .iter()
                        .for_each(|track| track.node.enabled.store(false, Release));
                    self.arrangement.node(id).0.enabled.store(true, Release);
                    self.soloed_track = Some(id);
                }
            }
            Message::SeekTo(pos) => {
                self.meter.sample.store(
                    pos.in_samples(self.meter.bpm.load(Acquire), self.meter.sample_rate),
                    Release,
                );
            }
            Message::ToggleRecord(id) => {
                if let Some((_, i)) = &self.recording {
                    return if *i == id {
                        self.update(Message::StopRecord)
                    } else {
                        self.update(Message::RecordingSplit(id))
                    };
                }

                let (recording, receiver) =
                    Recording::create(Self::make_recording_path(), &self.meter);
                self.recording = Some((recording, id));

                self.meter.playing.store(true, Release);

                return Task::stream(receiver).map(Message::RecordingChunk);
            }
            Message::RecordingSplit(id) => {
                if let Some((mut recording, track)) = self.recording.take() {
                    let mut pos = Position::from_samples(
                        self.meter.sample.load(Acquire),
                        self.meter.bpm.load(Acquire),
                        self.meter.sample_rate,
                    );

                    (pos, recording.position) = (recording.position, pos);
                    let audio = recording.split_off(Self::make_recording_path());
                    let track = self.arrangement.track_of(track).unwrap();

                    let clip = AudioClip::create(audio, self.meter.clone());
                    clip.position.move_to(pos);
                    self.arrangement.add_clip(track, clip);

                    self.recording = Some((recording, id));
                }
            }
            Message::RecordingChunk(samples) => {
                if let Some((recording, _)) = self.recording.as_mut() {
                    recording.write(&samples);
                }
            }
            Message::StopRecord => {
                if let Some((recording, track)) = self.recording.take() {
                    self.meter.playing.store(false, Release);

                    let pos = recording.position;
                    let audio = recording.try_into().unwrap();
                    let track = self.arrangement.track_of(track).unwrap();

                    let clip = AudioClip::create(audio, self.meter.clone());
                    clip.position.move_to(pos);
                    self.arrangement.add_clip(track, clip);
                }
            }
            Message::ArrangementAction(action) => self.handle_arrangement_action(action),
            Message::ArrangementPositionScaleDelta(pos, scale) => {
                let sd = scale != Vec2::ZERO;
                let mut pd = pos != Vec2::ZERO;

                if sd {
                    let old_scale = self.arrangement_scale;
                    self.arrangement_scale += scale;
                    self.arrangement_scale.x =
                        self.arrangement_scale.x.clamp(3.0, 13f32.next_down());
                    self.arrangement_scale.y = self.arrangement_scale.y.clamp(77.0, 200.0);
                    pd &= old_scale != self.arrangement_scale;
                }

                if pd {
                    self.arrangement_position += pos;
                    self.arrangement_position.x = self.arrangement_position.x.clamp(
                        0.0,
                        self.arrangement
                            .tracks()
                            .iter()
                            .map(Track::len)
                            .max()
                            .map(|m| {
                                m.in_samples(self.meter.bpm.load(Acquire), self.meter.sample_rate)
                            })
                            .max(
                                self.recording
                                    .as_ref()
                                    .map(|_| self.meter.sample.load(Acquire)),
                            )
                            .unwrap_or_default() as f32,
                    );
                    self.arrangement_position.y = self.arrangement_position.y.clamp(
                        0.0,
                        self.arrangement.tracks().len().saturating_sub(1) as f32,
                    );
                }
            }
            Message::PianoRollAction(action) => self.handle_piano_roll_action(action),
            Message::PianoRollPositionScaleDelta(pos, scale) => {
                let Tab::PianoRoll { clip, .. } = &self.tab else {
                    panic!()
                };

                let sd = scale != Vec2::ZERO;
                let mut pd = pos != Vec2::ZERO;

                if sd {
                    let old_scale = self.piano_roll_scale;
                    self.piano_roll_scale += scale;
                    self.piano_roll_scale.x = self.piano_roll_scale.x.clamp(3.0, 13f32.next_down());
                    self.piano_roll_scale.y = self
                        .piano_roll_scale
                        .y
                        .clamp(LINE_HEIGHT, 2.0 * LINE_HEIGHT);
                    pd &= old_scale != self.piano_roll_scale;
                }

                if pd {
                    let mut piano_roll_position = self.piano_roll_position.get();

                    piano_roll_position += pos;
                    piano_roll_position.x = piano_roll_position.x.clamp(
                        0.0,
                        clip.pattern
                            .load()
                            .iter()
                            .map(|note| note.end)
                            .max()
                            .unwrap_or_default()
                            .in_samples_f(self.meter.bpm.load(Acquire), self.meter.sample_rate),
                    );
                    piano_roll_position.y = piano_roll_position.y.max(0.0);

                    self.piano_roll_position.set(piano_roll_position);
                }
            }
            Message::SplitAt(split_at) => self.split_at = split_at.clamp(100.0, 500.0),
        }

        Task::none()
    }

    fn make_recording_path() -> Arc<Path> {
        let mut file_name = "recording-".to_owned();

        let mut hasher = DefaultHasher::new();
        Instant::now().hash(&mut hasher);
        file_name.push_str(itoa::Buffer::new().format(hasher.finish()));

        file_name.push_str(".wav");

        let data_dir = dirs::data_dir().unwrap().join("Generic Daw");
        _ = std::fs::create_dir(&data_dir);

        data_dir.join(file_name).into()
    }

    fn handle_arrangement_action(&mut self, action: ArrangementAction) {
        let Tab::Arrangement { grabbed_clip } = &mut self.tab else {
            panic!()
        };

        match action {
            ArrangementAction::Grab(track, clip) => *grabbed_clip = Some([track, clip]),
            ArrangementAction::Drop => *grabbed_clip = None,
            ArrangementAction::Clone(track, mut clip) => {
                self.arrangement.clone_clip(track, clip);
                clip = self.arrangement.tracks()[track].clips.len() - 1;
                *grabbed_clip = Some([track, clip]);
            }
            ArrangementAction::Drag(new_track, pos) => {
                let [track, clip] = grabbed_clip.as_mut().unwrap();

                if *track != new_track {
                    self.arrangement.clip_switch_track(*track, *clip, new_track);
                    *track = new_track;
                    *clip = self.arrangement.tracks()[*track].clips.len() - 1;
                }

                self.arrangement.tracks()[*track].clips[*clip]
                    .position()
                    .move_to(pos);
            }
            ArrangementAction::TrimStart(pos) => {
                let [track, clip] = grabbed_clip.unwrap();
                self.arrangement.tracks()[track].clips[clip]
                    .position()
                    .trim_start_to(pos);
            }
            ArrangementAction::TrimEnd(pos) => {
                let [track, clip] = grabbed_clip.unwrap();
                self.arrangement.tracks()[track].clips[clip]
                    .position()
                    .trim_end_to(pos);
            }
            ArrangementAction::Delete(track, clip) => self.arrangement.delete_clip(track, clip),
        }
    }

    fn handle_piano_roll_action(&mut self, action: PianoRollAction) {
        let Tab::PianoRoll { clip, grabbed_note } = &mut self.tab else {
            panic!()
        };

        match action {
            PianoRollAction::Grab(note) => *grabbed_note = Some(note),
            PianoRollAction::Drop => {
                let note = clip.pattern.load()[grabbed_note.unwrap()];
                self.last_note_len = note.end - note.start;
                *grabbed_note = None;
            }
            PianoRollAction::Add(key, pos) => {
                let mut notes = clip.pattern.load().deref().deref().clone();
                notes.push(MidiNote {
                    channel: 0,
                    key,
                    velocity: 1.0,
                    start: pos,
                    end: pos + self.last_note_len,
                });
                *grabbed_note = Some(notes.len() - 1);
                clip.pattern.store(Arc::new(notes));
            }
            PianoRollAction::Clone(note) => {
                let mut notes = clip.pattern.load().deref().deref().clone();
                notes.push(notes[note]);
                *grabbed_note = Some(notes.len() - 1);
                clip.pattern.store(Arc::new(notes));
            }
            PianoRollAction::Drag(key, pos) => {
                let mut notes = clip.pattern.load().deref().deref().clone();
                notes[grabbed_note.unwrap()].move_to(pos);
                notes[grabbed_note.unwrap()].key = key;
                clip.pattern.store(Arc::new(notes));
            }
            PianoRollAction::TrimStart(pos) => {
                let mut notes = clip.pattern.load().deref().deref().clone();
                notes[grabbed_note.unwrap()].trim_start_to(pos);
                clip.pattern.store(Arc::new(notes));
            }
            PianoRollAction::TrimEnd(pos) => {
                let mut notes = clip.pattern.load().deref().deref().clone();
                notes[grabbed_note.unwrap()].trim_end_to(pos);
                clip.pattern.store(Arc::new(notes));
            }
            PianoRollAction::Delete(note) => {
                let mut notes = clip.pattern.load().deref().deref().clone();
                notes.remove(note);
                clip.pattern.store(Arc::new(notes));
            }
        }
    }

    pub fn save(&mut self, path: &Path, sample_dirs: &[Box<Path>]) {
        let mut writer = Writer::new(
            u32::from(self.meter.bpm.load(Acquire)),
            self.meter.numerator.load(Acquire) as u32,
        );

        let mut audios = HashMap::new();
        for entry in &self.audios {
            let path = match entry.1 {
                LoadStatus::Loaded(audio) => {
                    let Some(audio) = audio.upgrade() else {
                        continue;
                    };
                    audio.path.clone()
                }
                LoadStatus::Loading(..) => continue,
            };
            audios.insert(
                path.clone(),
                writer.push_audio(
                    sample_dirs
                        .iter()
                        .find_map(|dir| path.strip_prefix(dir).ok())
                        .unwrap_or(&path),
                ),
            );
        }

        let mut midis = HashMap::new();
        for entry in &self.midis {
            let Some(pattern) = entry.upgrade() else {
                continue;
            };

            midis.insert(
                Arc::as_ptr(&pattern).addr(),
                writer.push_midi(
                    pattern
                        .load()
                        .iter()
                        .map(|note| proto::project::midi::Note {
                            key: u32::from(note.key.0),
                            velocity: note.velocity,
                            start: note.start.into(),
                            end: note.end.into(),
                        }),
                ),
            );
        }

        let mut tracks = HashMap::new();
        for track in self.arrangement.tracks() {
            tracks.insert(
                track.id(),
                writer.push_track(
                    track.clips.iter().map(|clip| match clip {
                        Clip::Audio(audio) => proto::project::track::AudioClip {
                            audio: Some(audios[&audio.audio.path]),
                            position: Some(proto::project::track::ClipPosition {
                                global_start: audio.position.get_global_start().into(),
                                global_end: audio.position.get_global_end().into(),
                                clip_start: audio.position.get_clip_start().into(),
                            }),
                        }
                        .into(),
                        Clip::Midi(midi) => proto::project::track::MidiClip {
                            midi: Some(midis[&Arc::as_ptr(&midi.pattern).addr()]),
                            position: Some(proto::project::track::ClipPosition {
                                global_start: midi.position.get_global_start().into(),
                                global_end: midi.position.get_global_end().into(),
                                clip_start: midi.position.get_clip_start().into(),
                            }),
                        }
                        .into(),
                    }),
                    self.plugins_by_channel
                        .get(*track.id())
                        .into_iter()
                        .flatten()
                        .map(|(id, descriptor)| proto::project::channel::Plugin {
                            id: descriptor.id.to_bytes_with_nul().to_owned(),
                            state: self.clap_host.get_state(*id),
                        }),
                    track.node.volume.load(Acquire),
                    track.node.pan.load(Acquire),
                ),
            );
        }

        let mut channels = HashMap::new();
        for channel in once(&*self.arrangement.master().0).chain(self.arrangement.channels()) {
            channels.insert(
                channel.id(),
                writer.push_channel(
                    self.plugins_by_channel
                        .get(*channel.id())
                        .into_iter()
                        .flatten()
                        .map(|(id, descriptor)| proto::project::channel::Plugin {
                            id: descriptor.id.to_bytes_with_nul().to_owned(),
                            state: self.clap_host.get_state(*id),
                        }),
                    channel.volume.load(Acquire),
                    channel.pan.load(Acquire),
                ),
            );
        }

        for track in self.arrangement.tracks() {
            for connection in &self.arrangement.node(track.id()).1 {
                writer.connect_track_to_channel(tracks[&track.id()], channels[&connection]);
            }
        }

        for channel in self.arrangement.channels() {
            for connection in &self.arrangement.node(channel.id()).1 {
                writer.connect_channel_to_channel(channels[&channel.id()], channels[&connection]);
            }
        }

        File::create(path)
            .unwrap()
            .write_all(&writer.finalize())
            .unwrap();
    }

    pub fn load(
        path: &Path,
        sample_dirs: &[Box<Path>],
    ) -> Option<(Self, Arc<Meter>, Task<Message>)> {
        info!("loading project {path:?}");

        let mut gdp = Vec::new();
        File::open(path).ok()?.read_to_end(&mut gdp).ok()?;
        let reader = Reader::new(&gdp)?;

        let (mut arrangement, meter) = ArrangementWrapper::create();

        let mut audios = HashMap::new();
        let mut midis = HashMap::new();

        let (sender, receiver) = mpsc::channel();
        std::thread::scope(|s| {
            for (idx, path) in reader.iter_audios() {
                let sender = sender.clone();
                let sample_rate = meter.sample_rate;
                s.spawn(move || {
                    let path = path.path();

                    let audio = sample_dirs
                        .iter()
                        .map(|dir| dir.join(&path).into())
                        .chain(once(path.as_ref().into()))
                        .find_map(|path| InterleavedAudio::create(path, sample_rate));

                    sender.send((idx, audio)).unwrap();
                });
            }

            for (idx, notes) in reader.iter_midis() {
                let pattern = notes
                    .notes
                    .iter()
                    .map(|note| MidiNote {
                        channel: 0,
                        key: MidiKey(note.key as u8),
                        velocity: note.velocity,
                        start: note.start.into(),
                        end: note.end.into(),
                    })
                    .collect();
                midis.insert(idx, Arc::new(ArcSwap::new(Arc::new(pattern))));
            }
        });

        while let Ok((idx, audio)) = receiver.try_recv() {
            audios.insert(idx, audio?);
        }

        let plugin_bundles = get_installed_plugins();
        let plugin_descriptors = plugin_bundles.keys().cloned().collect::<Vec<_>>();
        let mut plugins_by_channel: HoleyVec<Vec<(PluginId, PluginDescriptor)>> =
            HoleyVec::default();
        let mut futs = Vec::new();

        let mut load_channel = |node: &MixerNode, channel: &proto::project::Channel| {
            node.volume.store(channel.volume, Release);
            node.pan.store(channel.pan, Release);

            for plugin in &channel.plugins {
                let id = plugin.id();
                let descriptor = plugin_descriptors.iter().find(|d| &*d.id == id)?;

                let (mut gui, receiver, audio_processor) = clap_host::init(
                    plugin_bundles.get(descriptor)?,
                    descriptor.clone(),
                    f64::from(meter.sample_rate),
                    meter.buffer_size,
                );

                if let Some(state) = &plugin.state {
                    gui.set_state(state);
                }

                plugins_by_channel
                    .entry(*node.id())
                    .get_or_insert_default()
                    .push((audio_processor.id(), descriptor.clone()));

                node.add_plugin(audio_processor);

                futs.push(Task::done(Message::ClapHost(ClapHostMessage::Opened(
                    Arc::new(Mutex::new((Fragile::new(gui), receiver))),
                ))));
            }

            Some(())
        };

        let mut tracks = HashMap::new();
        for (idx, clips, channel) in reader.iter_tracks() {
            let mut track = Track::new(meter.clone());
            load_channel(&track.node, channel?)?;

            for clip in clips {
                let clip = match clip.clip? {
                    proto::project::track::clip::Clip::Audio(audio) => {
                        let clip =
                            AudioClip::create(audios.get(&audio.audio?)?.clone(), meter.clone());
                        clip.position.move_to(audio.position?.global_start.into());
                        clip.position.trim_end_to(audio.position?.global_end.into());
                        clip.position
                            .trim_start_to(audio.position?.clip_start.into());
                        Clip::Audio(clip)
                    }
                    proto::project::track::clip::Clip::Midi(midi) => {
                        let clip = MidiClip::create(midis.get(&midi.midi?)?.clone(), meter.clone());
                        clip.position.move_to(midi.position?.global_start.into());
                        clip.position.trim_end_to(midi.position?.global_end.into());
                        clip.position
                            .trim_start_to(midi.position?.clip_start.into());
                        Clip::Midi(clip)
                    }
                };

                track.clips.push(clip);
            }

            tracks.insert(idx, track.id());
            arrangement.push_track(track);
        }

        let mut channels = HashMap::new();
        let mut iter_channels = reader.iter_channels();

        let node = &arrangement.master().0;
        let (idx, channel) = iter_channels.next()?;
        load_channel(node, channel)?;
        channels.insert(idx, node.id());

        for (idx, channel) in iter_channels {
            let node = Arc::new(MixerNode::default());

            load_channel(&node, channel)?;

            channels.insert(idx, node.id());
            arrangement.push_channel(node);
        }

        for (from, to) in reader.iter_connections_track_channel() {
            futs.push(Task::perform(
                arrangement.request_connect(*channels.get(&to)?, *tracks.get(&from)?),
                |con| Message::ConnectSucceeded(con.unwrap()),
            ));
        }

        for (from, to) in reader.iter_connections_channel_channel() {
            futs.push(Task::perform(
                arrangement.request_connect(*channels.get(&from)?, *channels.get(&to)?),
                |con| Message::ConnectSucceeded(con.unwrap()),
            ));
        }

        info!("loaded project {path:?}");

        Some((
            Self {
                clap_host: ClapHost::default(),
                plugin_bundles,
                plugin_descriptors: combo_box::State::new(plugin_descriptors),

                arrangement,
                meter: meter.clone(),
                loading: BTreeSet::new(),
                audios: audios
                    .values()
                    .map(|audio| {
                        (
                            audio.path.clone(),
                            LoadStatus::Loaded(Arc::downgrade(audio)),
                        )
                    })
                    .collect(),
                midis: midis.values().map(Arc::downgrade).collect(),
                plugins_by_channel,

                tab: Tab::Arrangement { grabbed_clip: None },

                recording: None,

                arrangement_position: Vec2::default(),
                arrangement_scale: Vec2::new(9.0, 120.0),
                soloed_track: None,

                piano_roll_position: Cell::new(Vec2::new(0.0, 40.0)),
                piano_roll_scale: Vec2::new(9.0, LINE_HEIGHT),
                last_note_len: Position::BEAT,
                selected_channel: None,

                split_at: 300.0,
            },
            meter,
            Task::batch(futs),
        ))
    }

    pub fn view(&self) -> Element<'_, Message> {
        let element = match &self.tab {
            Tab::Arrangement { .. } => self.arrangement(),
            Tab::Mixer => self.mixer(),
            Tab::PianoRoll { clip, .. } => self.piano_roll(clip),
        };

        if self.loading.is_empty() {
            element
        } else {
            mouse_area(element)
                .interaction(Interaction::Progress)
                .into()
        }
    }

    fn arrangement(&self) -> Element<'_, Message> {
        Seeker::new(
            &self.meter,
            self.arrangement_position,
            self.arrangement_scale,
            column(
                self.arrangement
                    .tracks()
                    .iter()
                    .map(|track| {
                        let id = track.id();
                        let node = track.node.clone();
                        let enabled = node.enabled.load(Acquire);

                        container(
                            row![
                                PeakMeter::new(node.get_l_r(), enabled),
                                column![
                                    Knob::new(
                                        0.0..=1.0,
                                        track.node.volume.load(Acquire),
                                        0.0,
                                        1.0,
                                        enabled,
                                        Message::ChannelVolumeChanged.with(id)
                                    ),
                                    Knob::new(
                                        -1.0..=1.0,
                                        track.node.pan.load(Acquire),
                                        0.0,
                                        0.0,
                                        enabled,
                                        Message::ChannelPanChanged.with(id)
                                    ),
                                ]
                                .spacing(5.0)
                                .wrap(),
                                column![
                                    char_button('M')
                                        .on_press(Message::TrackToggleEnabled(id))
                                        .style(move |t, s| {
                                            button_with_base(
                                                t,
                                                s,
                                                if enabled {
                                                    button::primary
                                                } else {
                                                    button::secondary
                                                },
                                            )
                                        }),
                                    char_button('S')
                                        .on_press(Message::TrackToggleSolo(id))
                                        .style(move |t, s| {
                                            button_with_base(
                                                t,
                                                s,
                                                if self.soloed_track == Some(id) {
                                                    button::warning
                                                } else if enabled {
                                                    button::primary
                                                } else {
                                                    button::secondary
                                                },
                                            )
                                        }),
                                    char_button('X').on_press(Message::TrackRemove(id)).style(
                                        move |t, s| {
                                            button_with_base(
                                                t,
                                                s,
                                                if enabled {
                                                    button::danger
                                                } else {
                                                    button::secondary
                                                },
                                            )
                                        }
                                    ),
                                    column![
                                        vertical_space(),
                                        button(
                                            AnimatedDot::new(
                                                self.recording
                                                    .as_ref()
                                                    .is_some_and(|&(_, i)| i == id)
                                            )
                                            .radius(5.0)
                                        )
                                        .padding(1.5)
                                        .on_press(Message::ToggleRecord(id))
                                        .style(
                                            move |t, s| {
                                                button_with_base(
                                                    t,
                                                    s,
                                                    if self
                                                        .recording
                                                        .as_ref()
                                                        .is_some_and(|&(_, i)| i == id)
                                                    {
                                                        button::danger
                                                    } else if enabled {
                                                        button::primary
                                                    } else {
                                                        button::secondary
                                                    },
                                                )
                                            }
                                        )
                                    ]
                                ]
                                .spacing(5.0)
                            ]
                            .spacing(5.0),
                        )
                        .style(|t| {
                            container::transparent(t)
                                .background(t.extended_palette().background.weak.color)
                                .border(
                                    border::width(1.0)
                                        .color(t.extended_palette().background.strong.color),
                                )
                        })
                        .padding(5.0)
                        .height(self.arrangement_scale.y)
                    })
                    .map(Element::new)
                    .chain(once(
                        container(
                            button(styled_svg(ADD.clone()))
                                .style(|t, s| {
                                    let mut style = button_with_base(t, s, button::primary);
                                    style.border.radius = f32::INFINITY.into();
                                    style
                                })
                                .padding(5.0)
                                .on_press(Message::TrackAdd),
                        )
                        .padding(padding::right(5.0).top(5.0))
                        .into(),
                    )),
            )
            .align_x(Alignment::Center),
            ArrangementWidget::new(
                &self.meter,
                self.arrangement_position,
                self.arrangement_scale,
                column(
                    self.arrangement
                        .tracks()
                        .iter()
                        .map(|track| {
                            let id = track.id();
                            let enabled = track.node.enabled.load(Acquire);

                            let clips_iter = track.clips.iter().map(|clip| match clip {
                                Clip::Audio(clip) => AudioClipWidget::new(
                                    clip,
                                    self.arrangement_position,
                                    self.arrangement_scale,
                                    enabled,
                                )
                                .into(),
                                Clip::Midi(clip) => MidiClipWidget::new(
                                    clip,
                                    self.arrangement_position,
                                    self.arrangement_scale,
                                    enabled,
                                    Message::OpenMidiClip(clip.clone()),
                                )
                                .into(),
                            });

                            let clips_iter =
                                if self.recording.as_ref().is_some_and(|&(_, i)| i == id) {
                                    EnumDispatcher::A(
                                        clips_iter.chain(once(
                                            RecordingWidget::new(
                                                &self.recording.as_ref().unwrap().0,
                                                &self.meter,
                                                self.arrangement_position,
                                                self.arrangement_scale,
                                            )
                                            .into(),
                                        )),
                                    )
                                } else {
                                    EnumDispatcher::B(clips_iter)
                                };

                            TrackWidget::new(
                                &self.meter,
                                clips_iter,
                                self.arrangement_position,
                                self.arrangement_scale,
                                Message::AddMidiClip.with(id),
                            )
                        })
                        .map(Element::new),
                ),
                Message::ArrangementAction,
            ),
            Message::SeekTo,
            Message::ArrangementPositionScaleDelta,
        )
        .into()
    }

    fn mixer(&self) -> Element<'_, Message> {
        fn channel<'a>(
            selected_channel: Option<NodeId>,
            name: String,
            node: &MixerNode,
            buttons: impl Fn(bool, NodeId) -> Element<'a, Message>,
            connect: impl Fn(bool, NodeId) -> Element<'a, Message>,
        ) -> Element<'a, Message> {
            let id = node.id();
            let enabled = node.enabled.load(Acquire);
            let volume = node.volume.load(Acquire);

            button(
                column![
                    row![
                        column![
                            text(name),
                            Knob::new(
                                -1.0..=1.0,
                                node.pan.load(Acquire),
                                0.0,
                                0.0,
                                enabled,
                                Message::ChannelPanChanged.with(id)
                            ),
                            PeakMeter::new(node.get_l_r(), enabled)
                        ]
                        .spacing(5.0)
                        .align_x(Alignment::Center),
                        column![
                            buttons(enabled, id),
                            vertical_slider(
                                0.0..=1.0,
                                volume,
                                Message::ChannelVolumeChanged.with(id)
                            )
                            .step(f32::EPSILON)
                            .style(move |t, s| slider_with_enabled(t, s, enabled))
                        ]
                        .spacing(5.0)
                        .align_x(Alignment::Center)
                    ]
                    .spacing(5.0),
                    connect(enabled, id)
                ]
                .spacing(5.0)
                .align_x(Alignment::Center),
            )
            .padding(5.0)
            .on_press(Message::ChannelSelect(id))
            .style(move |t, _| {
                let pair = if Some(id) == selected_channel {
                    t.extended_palette().background.weak
                } else {
                    t.extended_palette().background.weakest
                };

                button::Style {
                    background: Some(pair.color.into()),
                    text_color: pair.text,
                    border: border::width(1.0).color(t.extended_palette().background.strong.color),
                    ..button::Style::default()
                }
            })
            .into()
        }

        let selected_channel = self
            .selected_channel
            .as_ref()
            .map(|c| self.arrangement.node(*c));

        let connect = |enabled: bool, id: NodeId| {
            selected_channel.map_or_else(
                || Element::new(empty_widget().width(TEXT_HEIGHT).height(TEXT_HEIGHT)),
                |(_, connections, ty)| {
                    let selected_channel = self.selected_channel.unwrap();

                    if *ty == NodeType::Master || id == selected_channel {
                        empty_widget().width(TEXT_HEIGHT).height(TEXT_HEIGHT).into()
                    } else {
                        let connected = connections.contains(*id);

                        button(
                            svg(CHEVRON_RIGHT.clone())
                                .style(move |t, s| svg_with_enabled(t, s, enabled))
                                .width(Length::Shrink)
                                .height(Length::Shrink)
                                .rotation(Radians(-FRAC_PI_2)),
                        )
                        .style(move |t, s| {
                            button_with_base(
                                t,
                                s,
                                if enabled && connected {
                                    button::primary
                                } else {
                                    button::secondary
                                },
                            )
                        })
                        .padding(0.0)
                        .on_press(if connected {
                            Message::Disconnect((id, selected_channel))
                        } else {
                            Message::ConnectRequest((id, selected_channel))
                        })
                        .into()
                    }
                },
            )
        };

        let mixer_panel = styled_scrollable_with_direction(
            row(once(channel(
                self.selected_channel,
                "M".to_owned(),
                &self.arrangement.master().0,
                |enabled, id| {
                    column![
                        char_button('M')
                            .on_press(Message::ChannelToggleEnabled(id))
                            .style(move |t, s| {
                                button_with_base(
                                    t,
                                    s,
                                    if enabled {
                                        button::primary
                                    } else {
                                        button::secondary
                                    },
                                )
                            }),
                        empty_widget().width(13.0).height(13.0),
                        empty_widget().width(13.0).height(13.0)
                    ]
                    .spacing(5.0)
                    .into()
                },
                connect,
            ))
            .chain(once(vertical_rule(1).into()))
            .chain({
                let mut iter = self
                    .arrangement
                    .tracks()
                    .iter()
                    .enumerate()
                    .map(|(i, track)| {
                        let mut name = "T ".to_owned();
                        name.push_str(itoa::Buffer::new().format(i + 1));

                        channel(
                            self.selected_channel,
                            name,
                            &track.node,
                            |enabled, id| {
                                column![
                                    char_button('M')
                                        .on_press(Message::TrackToggleEnabled(id))
                                        .style(move |t, s| {
                                            button_with_base(
                                                t,
                                                s,
                                                if enabled {
                                                    button::primary
                                                } else {
                                                    button::secondary
                                                },
                                            )
                                        }),
                                    char_button('S')
                                        .on_press(Message::TrackToggleSolo(id))
                                        .style(move |t, s| {
                                            button_with_base(
                                                t,
                                                s,
                                                if self.soloed_track == Some(id) {
                                                    button::warning
                                                } else if enabled {
                                                    button::primary
                                                } else {
                                                    button::secondary
                                                },
                                            )
                                        }),
                                    char_button('X').on_press(Message::TrackRemove(id)).style(
                                        move |t, s| {
                                            button_with_base(
                                                t,
                                                s,
                                                if enabled {
                                                    button::danger
                                                } else {
                                                    button::secondary
                                                },
                                            )
                                        }
                                    )
                                ]
                                .spacing(5.0)
                                .into()
                            },
                            |_, _| empty_widget().width(TEXT_HEIGHT).height(LINE_HEIGHT).into(),
                        )
                    })
                    .peekable();

                if iter.peek().is_some() {
                    EnumDispatcher::A(iter.chain(once(vertical_rule(1).into())))
                } else {
                    EnumDispatcher::B(iter)
                }
            })
            .chain({
                let mut iter = self
                    .arrangement
                    .channels()
                    .enumerate()
                    .map(|(i, node)| {
                        let mut name = "C ".to_owned();
                        name.push_str(itoa::Buffer::new().format(i + 1));

                        channel(
                            self.selected_channel,
                            name,
                            node,
                            |enabled, id| {
                                column![
                                    char_button('M')
                                        .on_press(Message::ChannelToggleEnabled(id))
                                        .style(move |t, s| {
                                            button_with_base(
                                                t,
                                                s,
                                                if enabled {
                                                    button::primary
                                                } else {
                                                    button::secondary
                                                },
                                            )
                                        }),
                                    empty_widget().width(13.0).height(13.0),
                                    char_button('X').on_press(Message::ChannelRemove(id)).style(
                                        move |t, s| {
                                            button_with_base(
                                                t,
                                                s,
                                                if enabled {
                                                    button::danger
                                                } else {
                                                    button::secondary
                                                },
                                            )
                                        }
                                    )
                                ]
                                .spacing(5.0)
                                .into()
                            },
                            connect,
                        )
                    })
                    .peekable();

                if iter.peek().is_some() {
                    EnumDispatcher::A(iter.chain(once(vertical_rule(1).into())))
                } else {
                    EnumDispatcher::B(iter)
                }
            })
            .chain(once(
                button(styled_svg(ADD.clone()))
                    .style(|t, s| {
                        let mut style = button_with_base(t, s, button::primary);
                        style.border.radius = f32::INFINITY.into();
                        style
                    })
                    .padding(5.0)
                    .on_press(Message::ChannelAdd)
                    .into(),
            )))
            .align_y(Alignment::Center)
            .spacing(5.0),
            Direction::Horizontal(Scrollbar::default()),
        )
        .width(Length::Fill);

        let plugin_picker = styled_combo_box(
            &self.plugin_descriptors,
            "Add Plugin",
            None,
            Message::PluginLoad,
        )
        .width(Length::Fill);

        if let Some(selected) = self.selected_channel {
            let node = &self.arrangement.node(selected).0;
            VSplit::new(
                mixer_panel,
                column![
                    plugin_picker,
                    horizontal_rule(11.0),
                    styled_scrollable_with_direction(
                        dragking::column({
                            self.plugins_by_channel[*selected].iter().enumerate().map(
                                |(i, (plugin_id, descriptor))| {
                                    let enabled = node.get_plugin_enabled(i);

                                    row![
                                        Knob::new(
                                            0.0..=1.0,
                                            node.get_plugin_mix(i),
                                            0.0,
                                            1.0,
                                            enabled,
                                            Message::PluginMixChanged.with(i)
                                        )
                                        .radius(TEXT_HEIGHT),
                                        button(
                                            container(
                                                text(&*descriptor.name).wrapping(Wrapping::None)
                                            )
                                            .clip(true)
                                        )
                                        .style(move |t, s| button_with_base(
                                            t,
                                            s,
                                            if enabled {
                                                button::primary
                                            } else {
                                                button::secondary
                                            }
                                        ))
                                        .width(Length::Fill)
                                        .on_press(
                                            Message::ClapHost(ClapHostMessage::MainThread(
                                                *plugin_id,
                                                MainThreadMessage::GuiRequestShow,
                                            ))
                                        ),
                                        column![
                                            char_button('M',)
                                                .on_press(Message::PluginToggleEnabled(i))
                                                .style(move |t, s| {
                                                    button_with_base(
                                                        t,
                                                        s,
                                                        if enabled {
                                                            button::primary
                                                        } else {
                                                            button::secondary
                                                        },
                                                    )
                                                }),
                                            char_button('X')
                                                .on_press(Message::PluginRemove(i))
                                                .style(move |t, s| {
                                                    button_with_base(
                                                        t,
                                                        s,
                                                        if enabled {
                                                            button::danger
                                                        } else {
                                                            button::secondary
                                                        },
                                                    )
                                                }),
                                        ]
                                        .spacing(5.0),
                                        mouse_area(
                                            container(
                                                svg(HANDLE.clone())
                                                    .rotation(Radians(FRAC_PI_2))
                                                    .width(Length::Shrink)
                                                    .height(LINE_HEIGHT + 10.0)
                                                    .style(|t: &Theme, _| svg::Style {
                                                        color: Some(
                                                            t.extended_palette()
                                                                .background
                                                                .weak
                                                                .text
                                                        )
                                                    })
                                            )
                                            .style(
                                                |t: &Theme| container::Style {
                                                    background: Some(
                                                        t.extended_palette()
                                                            .background
                                                            .weak
                                                            .color
                                                            .into()
                                                    ),
                                                    border: border::width(1.0).color(
                                                        t.extended_palette()
                                                            .background
                                                            .strong
                                                            .color
                                                    ),
                                                    ..container::Style::default()
                                                }
                                            )
                                        )
                                        .interaction(Interaction::Grab),
                                    ]
                                    .spacing(5.0)
                                    .into()
                                },
                            )
                        })
                        .spacing(5.0)
                        .on_drag(Message::PluginsReordered),
                        Direction::Vertical(Scrollbar::default())
                    )
                    .height(Length::Fill)
                ],
                Message::SplitAt,
            )
            .strategy(Strategy::Right)
            .split_at(self.split_at)
            .into()
        } else {
            mixer_panel.into()
        }
    }

    fn piano_roll<'a>(&'a self, clip: &'a MidiClip) -> Element<'a, Message> {
        responsive(move |size| {
            let mut piano_roll_position = self.piano_roll_position.get();
            let height = (size.height - LINE_HEIGHT) / self.piano_roll_scale.y;
            piano_roll_position.y = piano_roll_position.y.min(128.0 - height);
            self.piano_roll_position.set(piano_roll_position);

            let bpm = self.meter.bpm.load(Acquire);

            Seeker::new(
                &self.meter,
                piano_roll_position,
                self.piano_roll_scale,
                Piano::new(piano_roll_position, self.piano_roll_scale),
                PianoRoll::new(
                    clip.pattern.load().deref().clone(),
                    &self.meter,
                    piano_roll_position,
                    self.piano_roll_scale,
                    Message::PianoRollAction,
                ),
                Message::SeekTo,
                Message::PianoRollPositionScaleDelta,
            )
            .with_offset(
                clip.position
                    .get_global_start()
                    .in_samples_f(bpm, self.meter.sample_rate)
                    - clip
                        .position
                        .get_clip_start()
                        .in_samples_f(bpm, self.meter.sample_rate),
            )
            .into()
        })
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        self.clap_host.subscription().map(Message::ClapHost)
    }

    pub fn change_tab(&mut self, tab: Tab) {
        self.tab = tab;
    }
}
