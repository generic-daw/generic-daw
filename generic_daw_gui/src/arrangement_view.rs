use crate::{
    clap_host::{ClapHost, Message as ClapHostMessage},
    components::{styled_button, styled_pick_list, styled_scrollable_with_direction, styled_svg},
    daw::PLUGINS,
    icons::{CANCEL, CHEVRON_RIGHT, HANDLE, REOPEN},
    stylefns::{button_with_enabled, radio_with_enabled, slider_with_enabled, svg_with_enabled},
    widget::{
        Arrangement as ArrangementWidget, AudioClip as AudioClipWidget, Knob, LINE_HEIGHT,
        MidiClip as MidiClipWidget, PeakMeter, PianoRoll, Strategy, TEXT_HEIGHT,
        Track as TrackWidget, VSplit,
    },
};
use arrangement::NodeType;
use dragking::{DragEvent, DropPosition};
use fragile::Fragile;
use generic_daw_core::{
    AudioClip, AudioTrack, InterleavedAudio, Meter, MidiClip, MidiKey, MidiNote, MidiTrack,
    MixerNode, Position,
    audio_graph::{AudioGraph, AudioGraphNodeImpl as _, NodeId},
    clap_host::{self, MainThreadMessage, PluginDescriptor, PluginId, PluginType},
};
use generic_daw_utils::{EnumDispatcher, HoleyVec, ShiftMoveExt as _, Vec2};
use iced::{
    Alignment, Element, Function as _, Length, Radians, Subscription, Task, Theme,
    border::{self, Radius},
    futures::TryFutureExt as _,
    mouse::Interaction,
    widget::{
        button, column, container, horizontal_rule, mouse_area, radio, responsive, row,
        scrollable::{Direction, Scrollbar},
        svg, text,
        text::Wrapping,
        vertical_rule, vertical_slider, vertical_space,
    },
};
use std::{
    cell::Cell,
    f32::{self, consts::FRAC_PI_2},
    iter::once,
    ops::Deref as _,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::Ordering::{AcqRel, Acquire, Release},
    },
};

mod arrangement;
mod track;
mod track_clip;

pub use arrangement::Arrangement as ArrangementWrapper;
pub use track::Track as TrackWrapper;
pub use track_clip::TrackClip as TrackClipWrapper;

#[derive(Clone, Debug)]
pub enum Message {
    ClapHost(ClapHostMessage),

    ConnectRequest((NodeId, NodeId)),
    ConnectSucceeded((NodeId, NodeId)),
    Disconnect((NodeId, NodeId)),
    ExportRequest(Box<Path>),
    Export(Arc<Mutex<(AudioGraph, Box<Path>)>>),

    ChannelAdd,
    ChannelRemove(NodeId),
    ChannelSelect(NodeId),
    ChannelVolumeChanged(NodeId, f32),
    ChannelPanChanged(NodeId, f32),
    ChannelToggleEnabled(NodeId),

    AudioEffectLoad(PluginDescriptor),
    AudioEffectRemove(usize),
    AudioEffectMixChanged(usize, f32),
    AudioEffectToggleEnabled(usize),
    AudioEffectsReordered(DragEvent),

    SampleLoad(Box<Path>, Position),
    SampleLoaded(Option<Arc<InterleavedAudio>>, Position),

    InstrumentLoad(PluginDescriptor),

    TrackRemove(NodeId),
    TrackToggleEnabled(NodeId),
    TrackToggleSolo(NodeId),

    ClipGrab(usize, usize),
    ClipDrop,
    ClipClone(usize, usize),
    ClipMove(usize, Position),
    ClipTrimStart(Position),
    ClipTrimEnd(Position),
    ClipDelete(usize, usize),

    AddMidiClip(NodeId, Position),
    OpenMidiClip(Arc<MidiClip>),

    NoteGrab(usize),
    NoteDrop,
    NoteAdd(MidiKey, Position),
    NoteClone(usize),
    NoteMove(MidiKey, Position),
    NoteTrimStart(Position),
    NoteTrimEnd(Position),
    NoteDelete(usize),

    SeekTo(Position),

    ArrangementPositionScaleDelta(Vec2, Vec2),
    PianoRollPositionScaleDelta(Vec2, Vec2),

    SplitAt(f32),
}

#[derive(Clone, Debug)]
pub enum Tab {
    Arrangement,
    Mixer,
    PianoRoll(Arc<MidiClip>),
}

pub struct ArrangementView {
    pub clap_host: ClapHost,

    instrument_by_track: HoleyVec<PluginId>,
    audio_effects_by_channel: HoleyVec<Vec<(PluginId, Box<str>)>>,

    arrangement: ArrangementWrapper,
    meter: Arc<Meter>,

    tab: Tab,
    loading: usize,

    arrangement_position: Vec2,
    arrangement_scale: Vec2,
    grabbed_clip: Option<[usize; 2]>,
    soloed_track: Option<NodeId>,

    piano_roll_position: Cell<Vec2>,
    piano_roll_scale: Vec2,
    grabbed_note: Option<usize>,

    selected_channel: Option<NodeId>,
    split_at: f32,
}

impl ArrangementView {
    pub fn create() -> (Self, Arc<Meter>) {
        let (arrangement, meter) = ArrangementWrapper::create();

        (
            Self {
                clap_host: ClapHost::default(),
                instrument_by_track: HoleyVec::default(),
                audio_effects_by_channel: HoleyVec::default(),

                arrangement,
                meter: meter.clone(),

                tab: Tab::Arrangement,
                loading: 0,

                arrangement_position: Vec2::default(),
                arrangement_scale: Vec2::new(9.0, 120.0),
                grabbed_clip: None,
                soloed_track: None,

                piano_roll_position: Cell::new(Vec2::new(0.0, 40.0)),
                piano_roll_scale: Vec2::new(9.0, LINE_HEIGHT),
                grabbed_note: None,

                selected_channel: None,
                split_at: 300.0,
            },
            meter,
        )
    }

    pub fn stop(&mut self) {
        self.arrangement.stop();
    }

    #[expect(clippy::too_many_lines)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ClapHost(msg) => return self.clap_host.update(msg).map(Message::ClapHost),
            Message::ConnectRequest((from, to)) => {
                return Task::future(self.arrangement.request_connect(from, to))
                    .and_then(Task::done)
                    .map(Message::ConnectSucceeded);
            }
            Message::ConnectSucceeded((from, to)) => {
                self.arrangement.connect_succeeded(from, to);
            }
            Message::Disconnect((from, to)) => {
                self.arrangement.disconnect(from, to);
            }
            Message::ExportRequest(path) => {
                return Task::future(self.arrangement.request_export().map_ok(|ok| (ok, path)))
                    .and_then(Task::done)
                    .map(Mutex::new)
                    .map(Arc::new)
                    .map(Message::Export);
            }
            Message::Export(message) => {
                let (audio_graph, path) =
                    Mutex::into_inner(Arc::into_inner(message).unwrap()).unwrap();
                self.arrangement.export(audio_graph, &path);
            }
            Message::ChannelAdd => {
                return Task::future(self.arrangement.add_channel())
                    .and_then(Task::done)
                    .map(Message::ConnectSucceeded);
            }
            Message::ChannelRemove(id) => {
                self.arrangement.remove_channel(id);

                if self.selected_channel == Some(id) {
                    self.selected_channel = None;
                }

                if let Some(effects) = self.audio_effects_by_channel.remove(id.get()) {
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
                    self.audio_effects_by_channel
                        .entry(id.get())
                        .get_or_insert_default();
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
            Message::AudioEffectLoad(name) => {
                let Some(selected) = self.selected_channel else {
                    return Task::none();
                };
                let node = self.arrangement.node(selected).0.clone();

                let (gui, receiver, audio_processor) = clap_host::init(
                    &PLUGINS[&name],
                    name,
                    f64::from(self.meter.sample_rate),
                    self.meter.buffer_size,
                );

                let id = audio_processor.id();
                node.add_effect(audio_processor);

                self.audio_effects_by_channel
                    .get_mut(selected.get())
                    .unwrap()
                    .push((id, gui.name().into()));

                return self
                    .clap_host
                    .update(ClapHostMessage::Opened(Arc::new(Mutex::new((
                        Fragile::new(gui),
                        receiver,
                    )))))
                    .map(Message::ClapHost);
            }
            Message::AudioEffectMixChanged(i, mix) => {
                let selected = self.selected_channel.unwrap();
                self.arrangement.node(selected).0.set_effect_mix(i, mix);
            }
            Message::AudioEffectToggleEnabled(i) => {
                let selected = self.selected_channel.unwrap();
                self.arrangement.node(selected).0.toggle_effect_enabled(i);
            }
            Message::AudioEffectsReordered(event) => {
                if let DragEvent::Dropped {
                    index,
                    mut target_index,
                    drop_position,
                } = event
                {
                    if drop_position == DropPosition::After {
                        target_index -= 1;
                    }

                    if index != target_index {
                        let selected = self.selected_channel.unwrap();

                        self.arrangement
                            .node(selected)
                            .0
                            .shift_move(index, target_index);
                        self.audio_effects_by_channel
                            .get_mut(selected.get())
                            .unwrap()
                            .shift_move(index, target_index);
                    }
                }
            }
            Message::AudioEffectRemove(i) => {
                let selected = self.selected_channel.unwrap();
                self.arrangement.node(selected).0.remove_effect(i);
                let id = self
                    .audio_effects_by_channel
                    .get_mut(selected.get())
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
            Message::SampleLoad(path, position) => {
                self.loading += 1;
                let meter = self.meter.clone();
                return Task::future(tokio::task::spawn_blocking(move || {
                    InterleavedAudio::create(&path, meter.sample_rate)
                }))
                .and_then(Task::done)
                .map(Result::ok)
                .map(move |audio_file| Message::SampleLoaded(audio_file, position));
            }
            Message::SampleLoaded(audio_file, start) => {
                self.loading -= 1;

                if let Some(audio_file) = audio_file {
                    let clip = AudioClip::create(audio_file, self.meter.clone());
                    clip.position.move_to(start);
                    let end = clip.position.get_global_end();

                    let (track, fut) = self
                        .arrangement
                        .tracks()
                        .iter()
                        .filter(|track| {
                            track.clips().all(|clip| {
                                clip.get_global_start() >= end || clip.get_global_end() <= start
                            })
                        })
                        .position(|track| matches!(track, TrackWrapper::AudioTrack(..)))
                        .map_or_else(
                            || {
                                let track = AudioTrack::new(self.meter.clone());
                                (
                                    self.arrangement.tracks().len(),
                                    Task::future(self.arrangement.add_track(track))
                                        .and_then(Task::done)
                                        .map(Message::ConnectSucceeded),
                                )
                            },
                            |x| (x, Task::none()),
                        );

                    self.arrangement.add_clip(track, clip);

                    return fut;
                }
            }
            Message::InstrumentLoad(name) => {
                let (gui, receiver, audio_processor) = clap_host::init(
                    &PLUGINS[&name],
                    name,
                    f64::from(self.meter.sample_rate),
                    self.meter.buffer_size,
                );

                let plugin_id = audio_processor.id();
                let track = MidiTrack::new(self.meter.clone(), audio_processor);
                self.instrument_by_track.insert(track.id().get(), plugin_id);

                return Task::batch([
                    Task::future(self.arrangement.add_track(track))
                        .and_then(Task::done)
                        .map(Message::ConnectSucceeded),
                    self.clap_host
                        .update(ClapHostMessage::Opened(Arc::new(Mutex::new((
                            Fragile::new(gui),
                            receiver,
                        )))))
                        .map(Message::ClapHost),
                ]);
            }
            Message::TrackRemove(id) => {
                self.arrangement.remove_track(id);
                let fut = self.update(Message::ChannelRemove(id));

                return if let Some(id) = self.instrument_by_track.remove(id.get()) {
                    self.update(Message::ClapHost(ClapHostMessage::MainThread(
                        id,
                        MainThreadMessage::GuiClosed,
                    )))
                    .chain(fut)
                } else {
                    fut
                };
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
                        .for_each(|track| track.node().enabled.store(true, Release));
                } else {
                    self.arrangement
                        .tracks()
                        .iter()
                        .for_each(|track| track.node().enabled.store(false, Release));
                    self.arrangement.node(id).0.enabled.store(true, Release);
                    self.soloed_track = Some(id);
                }
            }
            Message::ClipGrab(track, clip) => self.grabbed_clip = Some([track, clip]),
            Message::ClipDrop => self.grabbed_clip = None,
            Message::ClipClone(track, mut clip) => {
                self.arrangement.clone_clip(track, clip);
                clip = self.arrangement.tracks()[track].clips().len() - 1;
                self.grabbed_clip.replace([track, clip]);
            }
            Message::ClipMove(new_track, pos) => {
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
            Message::ClipTrimStart(pos) => {
                let [track, clip] = self.grabbed_clip.unwrap();
                self.arrangement.tracks()[track]
                    .get_clip(clip)
                    .trim_start_to(pos);
            }
            Message::ClipTrimEnd(pos) => {
                let [track, clip] = self.grabbed_clip.unwrap();
                self.arrangement.tracks()[track]
                    .get_clip(clip)
                    .trim_end_to(pos);
            }
            Message::ClipDelete(track, clip) => {
                self.arrangement.delete_clip(track, clip);
            }
            Message::OpenMidiClip(clip) => self.tab = Tab::PianoRoll(clip),
            Message::AddMidiClip(track, pos) => {
                let clip = MidiClip::create(Arc::default(), self.meter.clone());
                clip.position
                    .trim_end_to(Position::BEAT * self.meter.numerator.load(Acquire) as u32);
                clip.position.move_to(pos);
                let track = self
                    .arrangement
                    .tracks()
                    .iter()
                    .position(|t| t.id() == track)
                    .unwrap();
                self.arrangement.add_clip(track, clip);
            }
            Message::NoteGrab(note) => self.grabbed_note = Some(note),
            Message::NoteDrop => self.grabbed_note = None,
            Message::NoteAdd(key, pos) => {
                let Tab::PianoRoll(selected_clip) = &self.tab else {
                    return Task::none();
                };

                let mut notes = selected_clip.pattern.load().deref().deref().clone();
                notes.push(MidiNote {
                    channel: 0,
                    key,
                    velocity: 1.0,
                    start: pos,
                    end: pos + Position::BEAT,
                });
                self.grabbed_note = Some(notes.len() - 1);
                selected_clip.pattern.store(Arc::new(notes));
            }
            Message::NoteClone(note) => {
                let Tab::PianoRoll(selected_clip) = &self.tab else {
                    return Task::none();
                };

                let mut notes = selected_clip.pattern.load().deref().deref().clone();
                notes.push(notes[note]);
                self.grabbed_note = Some(notes.len() - 1);
                selected_clip.pattern.store(Arc::new(notes));
            }
            Message::NoteMove(key, pos) => {
                let Tab::PianoRoll(selected_clip) = &self.tab else {
                    return Task::none();
                };

                let mut notes = selected_clip.pattern.load().deref().deref().clone();
                notes[self.grabbed_note.unwrap()].move_to(pos);
                notes[self.grabbed_note.unwrap()].key = key;
                selected_clip.pattern.store(Arc::new(notes));
            }
            Message::NoteTrimStart(pos) => {
                let Tab::PianoRoll(selected_clip) = &self.tab else {
                    return Task::none();
                };

                let mut notes = selected_clip.pattern.load().deref().deref().clone();
                notes[self.grabbed_note.unwrap()].trim_start_to(pos);
                selected_clip.pattern.store(Arc::new(notes));
            }
            Message::NoteTrimEnd(pos) => {
                let Tab::PianoRoll(selected_clip) = &self.tab else {
                    return Task::none();
                };

                let mut notes = selected_clip.pattern.load().deref().deref().clone();
                notes[self.grabbed_note.unwrap()].trim_end_to(pos);
                selected_clip.pattern.store(Arc::new(notes));
            }
            Message::NoteDelete(note) => {
                let Tab::PianoRoll(selected_clip) = &self.tab else {
                    return Task::none();
                };

                let mut notes = selected_clip.pattern.load().deref().deref().clone();
                notes.remove(note);
                selected_clip.pattern.store(Arc::new(notes));
            }
            Message::SeekTo(pos) => {
                self.meter.sample.store(
                    pos.in_interleaved_samples(
                        self.meter.bpm.load(Acquire),
                        self.meter.sample_rate,
                    ),
                    Release,
                );
            }
            Message::ArrangementPositionScaleDelta(pos, scale) => {
                let sd = scale != Vec2::ZERO;
                let mut pd = pos != Vec2::ZERO;

                if sd {
                    let old_scale = self.arrangement_scale;
                    self.arrangement_scale += scale;
                    self.arrangement_scale.x = self.arrangement_scale.x.clamp(3.0, 12.999_999);
                    self.arrangement_scale.y = self
                        .arrangement_scale
                        .y
                        .clamp(2.0 * LINE_HEIGHT, 10.0 * LINE_HEIGHT);
                    pd &= old_scale != self.arrangement_scale;
                }

                if pd {
                    self.arrangement_position += pos;
                    self.arrangement_position.x = self.arrangement_position.x.clamp(
                        0.0,
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
                    );
                    self.arrangement_position.y = self.arrangement_position.y.clamp(
                        0.0,
                        self.arrangement.tracks().len().saturating_sub(1) as f32,
                    );
                }
            }
            Message::PianoRollPositionScaleDelta(pos, scale) => {
                let Tab::PianoRoll(selected_clip) = &self.tab else {
                    return Task::none();
                };

                let sd = scale != Vec2::ZERO;
                let mut pd = pos != Vec2::ZERO;

                if sd {
                    let old_scale = self.piano_roll_scale;
                    self.piano_roll_scale += scale;
                    self.piano_roll_scale.x = self.piano_roll_scale.x.clamp(3.0, 12.999_999);
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
                        selected_clip
                            .pattern
                            .load()
                            .iter()
                            .map(|note| note.end)
                            .max()
                            .unwrap_or_default()
                            .in_interleaved_samples_f(
                                self.meter.bpm.load(Acquire),
                                self.meter.sample_rate,
                            ),
                    );
                    piano_roll_position.y = piano_roll_position.y.clamp(0.0, 127.0);

                    self.piano_roll_position.set(piano_roll_position);
                }
            }
            Message::SplitAt(split_at) => self.split_at = split_at.clamp(100.0, 500.0),
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let element = match &self.tab {
            Tab::Arrangement => self.arrangement(),
            Tab::Mixer => self.mixer(),
            Tab::PianoRoll(selected_clip) => self.piano_roll(selected_clip),
        };

        if self.loading > 0 {
            mouse_area(element)
                .interaction(Interaction::Progress)
                .into()
        } else {
            element
        }
    }

    #[expect(clippy::too_many_lines)]
    fn arrangement(&self) -> Element<'_, Message> {
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
                        let node = track.node().clone();
                        let enabled = node.enabled.load(Acquire);

                        let mut buttons = column![
                            mouse_area(
                                radio("", enabled, Some(true), |_| {
                                    Message::TrackToggleEnabled(id)
                                })
                                .text_line_height(1.0)
                                .style(move |t, s| radio_with_enabled(t, s, enabled))
                                .spacing(0.0)
                            )
                            .on_right_press(Message::TrackToggleSolo(id)),
                            button(styled_svg(CANCEL.clone()).height(TEXT_HEIGHT))
                                .style(|t, s| {
                                    let mut style = button::danger(t, s);
                                    style.border.radius = Radius::new(f32::INFINITY);
                                    style
                                })
                                .padding(0.0)
                                .on_press(Message::TrackRemove(id)),
                        ]
                        .spacing(5.0);

                        if let Some(&id) = self.instrument_by_track.get(id.get()) {
                            buttons = buttons.extend([
                                vertical_space().into(),
                                button(
                                    svg(REOPEN.clone())
                                        .style(move |t, s| svg_with_enabled(t, s, enabled))
                                        .width(Length::Shrink)
                                        .height(TEXT_HEIGHT),
                                )
                                .style(move |t, s| button_with_enabled(t, s, enabled))
                                .padding(0.0)
                                .on_press(Message::ClapHost(ClapHostMessage::MainThread(
                                    id,
                                    MainThreadMessage::GuiRequestShow,
                                )))
                                .into(),
                            ]);
                        }

                        row![
                            container(
                                row![
                                    PeakMeter::new(move || node.get_l_r(), enabled),
                                    column![
                                        mouse_area(Knob::new(
                                            0.0..=1.0,
                                            0.0,
                                            track.node().volume.load(Acquire),
                                            enabled,
                                            Message::ChannelVolumeChanged.with(id)
                                        ))
                                        .on_double_click(Message::ChannelVolumeChanged(id, 1.0)),
                                        mouse_area(Knob::new(
                                            -1.0..=1.0,
                                            0.0,
                                            track.node().pan.load(Acquire),
                                            enabled,
                                            Message::ChannelPanChanged.with(id)
                                        ))
                                        .on_double_click(Message::ChannelPanChanged(id, 0.0)),
                                        vertical_space(),
                                    ]
                                    .spacing(5.0),
                                    buttons,
                                ]
                                .spacing(5.0),
                            )
                            .style(|t| container::transparent(t)
                                .background(t.extended_palette().background.weak.color)
                                .border(
                                    border::width(1.0)
                                        .color(t.extended_palette().background.strong.color)
                                ))
                            .padding(5.0)
                            .height(Length::Fixed(self.arrangement_scale.y)),
                            TrackWidget::new(
                                &self.meter,
                                track.clips().map(|clip| match clip {
                                    TrackClipWrapper::AudioClip(clip) => {
                                        AudioClipWidget::new(
                                            clip,
                                            self.arrangement_position,
                                            self.arrangement_scale,
                                            enabled,
                                        )
                                        .into()
                                    }
                                    TrackClipWrapper::MidiClip(clip) => MidiClipWidget::new(
                                        clip.clone(),
                                        self.arrangement_position,
                                        self.arrangement_scale,
                                        enabled,
                                        Message::OpenMidiClip(clip)
                                    )
                                    .into(),
                                }),
                                self.arrangement_position,
                                self.arrangement_scale,
                                matches!(track, TrackWrapper::MidiTrack(..))
                                    .then_some(Message::AddMidiClip.with(id))
                            )
                        ]
                    })
                    .map(Element::new),
            ),
            Message::SeekTo,
            Message::ClipGrab,
            Message::ClipDrop,
            Message::ClipClone,
            Message::ClipMove,
            Message::ClipTrimStart,
            Message::ClipTrimEnd,
            Message::ClipDelete,
            Message::ArrangementPositionScaleDelta,
        )
        .into()
    }

    #[expect(clippy::too_many_lines)]
    fn mixer(&self) -> Element<'_, Message> {
        fn channel<'a>(
            selected_channel: Option<NodeId>,
            name: String,
            node: Arc<MixerNode>,
            toggle: impl Fn(bool, NodeId) -> Element<'a, Message>,
            remove: impl Fn(bool, NodeId) -> Element<'a, Message>,
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
                            mouse_area(Knob::new(
                                -1.0..=1.0,
                                0.0,
                                node.pan.load(Acquire),
                                enabled,
                                Message::ChannelPanChanged.with(id)
                            )),
                            PeakMeter::new(move || node.get_l_r(), enabled)
                        ]
                        .spacing(5.0)
                        .align_x(Alignment::Center),
                        column![
                            toggle(enabled, id),
                            remove(enabled, id),
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
                || {
                    button("")
                        .height(24.0)
                        .style(|_, _| button::Style::default())
                },
                |(_, connections, ty)| {
                    let selected_channel = self.selected_channel.unwrap();

                    if *ty == NodeType::Master || id == selected_channel {
                        button("")
                            .height(24.0)
                            .style(|_, _| button::Style::default())
                    } else {
                        let connected = connections.contains(id.get());

                        button(
                            svg(CHEVRON_RIGHT.clone())
                                .style(move |t, s| svg_with_enabled(t, s, enabled))
                                .width(Length::Shrink)
                                .height(Length::Shrink)
                                .rotation(Radians(-FRAC_PI_2)),
                        )
                        .style(move |t, s| button_with_enabled(t, s, enabled && connected))
                        .padding(0.0)
                        .on_press(if connected {
                            Message::Disconnect((id, selected_channel))
                        } else {
                            Message::ConnectRequest((id, selected_channel))
                        })
                    }
                },
            )
        };

        let mixer_panel = styled_scrollable_with_direction(
            row(once(channel(
                self.selected_channel,
                "M".to_owned(),
                self.arrangement.master().0.clone(),
                |enabled, id| {
                    radio("", enabled, Some(true), |_| {
                        Message::ChannelToggleEnabled(id)
                    })
                    .text_line_height(1.0)
                    .style(move |t, s| radio_with_enabled(t, s, enabled))
                    .spacing(0.0)
                    .into()
                },
                |_, _| vertical_space().height(TEXT_HEIGHT).into(),
                |enabled, id| connect(enabled, id).into(),
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
                            track.node().clone(),
                            |enabled, id| {
                                mouse_area(
                                    radio("", enabled, Some(true), |_| {
                                        Message::TrackToggleEnabled(id)
                                    })
                                    .text_line_height(1.0)
                                    .style(move |t, s| radio_with_enabled(t, s, enabled))
                                    .spacing(0.0),
                                )
                                .on_right_press(Message::TrackToggleSolo(id))
                                .into()
                            },
                            |_, id| {
                                button(styled_svg(CANCEL.clone()).height(TEXT_HEIGHT))
                                    .style(|t, s| {
                                        let mut style = button::danger(t, s);
                                        style.border.radius = Radius::new(f32::INFINITY);
                                        style
                                    })
                                    .padding(0.0)
                                    .on_press(Message::TrackRemove(id))
                                    .into()
                            },
                            |_, _| button("").style(|_, _| button::Style::default()).into(),
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
                            node.clone(),
                            |enabled, id| {
                                radio("", enabled, Some(true), |_| {
                                    Message::ChannelToggleEnabled(id)
                                })
                                .text_line_height(1.0)
                                .style(move |t, s| radio_with_enabled(t, s, enabled))
                                .spacing(0.0)
                                .into()
                            },
                            |_, id| {
                                button(styled_svg(CANCEL.clone()).height(TEXT_HEIGHT))
                                    .style(|t, s| {
                                        let mut style = button::danger(t, s);
                                        style.border.radius = Radius::new(f32::INFINITY);
                                        style
                                    })
                                    .padding(0.0)
                                    .on_press(Message::ChannelRemove(id))
                                    .into()
                            },
                            |enabled, id| connect(enabled, id).into(),
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
                styled_button(row!["+"].height(Length::Fill).align_y(Alignment::Center))
                    .on_press(Message::ChannelAdd)
                    .into(),
            )))
            .spacing(5.0),
            Direction::Horizontal(Scrollbar::default()),
        )
        .width(Length::Fill);

        let plugin_picker = styled_pick_list(
            PLUGINS
                .keys()
                .filter(|d| d.ty == PluginType::AudioEffect)
                .collect::<Box<[_]>>(),
            None::<&PluginDescriptor>,
            |p| Message::AudioEffectLoad(p.to_owned()),
        )
        .width(Length::Fill)
        .placeholder("Add Effect");

        if let Some(selected) = self.selected_channel {
            VSplit::new(
                mixer_panel,
                if self.audio_effects_by_channel[selected.get()].is_empty() {
                    Element::new(plugin_picker)
                } else {
                    let node = self.arrangement.node(selected).0.clone();

                    column![
                        plugin_picker,
                        horizontal_rule(11.0),
                        styled_scrollable_with_direction(
                            dragking::column({
                                self.audio_effects_by_channel[selected.get()]
                                    .iter()
                                    .enumerate()
                                    .map(|(i, (plugin_id, name))| {
                                        let enabled = node.get_effect_enabled(i);

                                        row![
                                            mouse_area(
                                                Knob::new(
                                                    0.0..=1.0,
                                                    0.0,
                                                    node.get_effect_mix(i),
                                                    enabled,
                                                    move |mix| {
                                                        Message::AudioEffectMixChanged(i, mix)
                                                    }
                                                )
                                                .radius(TEXT_HEIGHT)
                                            )
                                            .on_double_click(Message::AudioEffectMixChanged(
                                                i, 1.0
                                            )),
                                            button(
                                                container(text(&**name).wrapping(Wrapping::None))
                                                    .clip(true)
                                            )
                                            .style(move |t, s| button_with_enabled(t, s, enabled))
                                            .width(Length::Fill)
                                            .on_press(
                                                Message::ClapHost(ClapHostMessage::MainThread(
                                                    *plugin_id,
                                                    MainThreadMessage::GuiRequestShow,
                                                ),)
                                            ),
                                            column![
                                                radio("", enabled, Some(true), |_| {
                                                    Message::AudioEffectToggleEnabled(i)
                                                })
                                                .text_line_height(1.0)
                                                .style(move |t, s| radio_with_enabled(
                                                    t, s, enabled
                                                ))
                                                .spacing(0.0),
                                                button(
                                                    styled_svg(CANCEL.clone()).height(TEXT_HEIGHT)
                                                )
                                                .style(|t, s| {
                                                    let mut style = button::danger(t, s);
                                                    style.border.radius =
                                                        Radius::new(f32::INFINITY);
                                                    style
                                                })
                                                .padding(0.0)
                                                .on_press(Message::AudioEffectRemove(i)),
                                            ],
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
                                                .style(|t: &Theme| container::Style {
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
                                                })
                                            )
                                            .interaction(Interaction::Grab),
                                        ]
                                        .align_y(Alignment::Center)
                                        .spacing(5.0)
                                        .into()
                                    })
                            })
                            .spacing(5.0)
                            .on_drag(Message::AudioEffectsReordered),
                            Direction::Vertical(Scrollbar::default())
                        )
                        .height(Length::Fill)
                    ]
                    .into()
                },
            )
            .strategy(Strategy::Right)
            .split_at(self.split_at)
            .on_resize(Message::SplitAt)
            .into()
        } else {
            mixer_panel.into()
        }
    }

    fn piano_roll<'a>(&'a self, selected_clip: &'a Arc<MidiClip>) -> Element<'a, Message> {
        responsive(move |size| {
            let mut piano_roll_position = self.piano_roll_position.get();
            let height = size.height / self.piano_roll_scale.y;
            piano_roll_position.y = piano_roll_position.y.min(127.0 - height);
            self.piano_roll_position.set(piano_roll_position);

            PianoRoll {
                notes: selected_clip.pattern.load().deref().clone(),
                meter: &self.meter,
                position: piano_roll_position,
                scale: self.piano_roll_scale,
                deleted: false,
                select_note: Message::NoteGrab,
                unselect_note: Message::NoteDrop,
                add_note: Message::NoteAdd,
                clone_note: Message::NoteClone,
                move_note_to: Message::NoteMove,
                trim_note_start: Message::NoteTrimStart,
                trim_note_end: Message::NoteTrimEnd,
                delete_note: Message::NoteDelete,
                position_scale_delta: Message::PianoRollPositionScaleDelta,
            }
            .into()
        })
        .into()
    }

    pub fn subscription() -> Subscription<Message> {
        ClapHost::subscription().map(Message::ClapHost)
    }

    pub fn change_tab(&mut self, tab: Tab) {
        self.tab = tab;
    }
}
