use crate::{
    clap_host_view::{ClapHostView, Message as ClapHostMessage},
    components::{
        round_danger_button, styled_container, styled_scrollable_with_direction, styled_svg,
    },
    stylefns::{radio_secondary, slider_secondary},
    widget::{
        Arrangement as ArrangementWidget, ArrangementPosition, ArrangementScale,
        AudioClip as AudioClipWidget, Knob, LINE_HEIGHT, PeakMeter, Track as TrackWidget,
    },
};
use fragile::Fragile;
use generic_daw_core::{
    AudioClip, AudioTrack, InterleavedAudio, Meter, MidiTrack, Position,
    audio_graph::{AudioGraph, AudioGraphNodeImpl as _, NodeId},
    clap_host::{
        self, MainThreadMessage, PluginDescriptor, PluginId, clack_host::bundle::PluginBundle,
    },
};
use generic_daw_utils::{HoleyVec, NoDebug};
use iced::{
    Alignment, Element, Function as _, Length, Subscription, Task, Theme, border,
    futures::TryFutureExt as _,
    mouse::Interaction,
    widget::{
        button, column, mouse_area, radio, row,
        scrollable::{Direction, Scrollbar},
        slider, svg, text, vertical_slider, vertical_space,
    },
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
    ConnectSucceeded((NodeId, NodeId)),
    RequestConnect((NodeId, NodeId)),
    Disconnect((NodeId, NodeId)),
    Select(NodeId),
    NodeVolumeChanged(NodeId, f32),
    NodePanChanged(NodeId, f32),
    ToggleTrackEnabled(NodeId),
    ToggleNodeEnabled(NodeId),
    ToggleTrackSolo(usize),
    LoadSample(Box<Path>),
    LoadedSample(Option<Arc<InterleavedAudio>>),
    LoadedPlugin(PluginDescriptor, NoDebug<PluginBundle>),
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

#[derive(Clone, Copy, Debug)]
pub enum Tab {
    Arrangement,
    Mixer,
}

pub struct ArrangementView {
    clap_host: ClapHostView,
    plugin_ids: HoleyVec<PluginId>,

    arrangement: ArrangementWrapper,
    meter: Arc<Meter>,

    tab: Tab,
    loading: usize,

    position: ArrangementPosition,
    scale: ArrangementScale,
    soloed_track: Option<usize>,
    grabbed_clip: Option<[usize; 2]>,

    selected_channel: Option<NodeId>,
}

impl ArrangementView {
    pub fn create(main_window_id: Id) -> (Self, Arc<Meter>) {
        let (arrangement, meter) = ArrangementWrapper::new();

        (
            Self {
                clap_host: ClapHostView::new(main_window_id),
                plugin_ids: HoleyVec::default(),

                arrangement,
                meter: meter.clone(),

                tab: Tab::Arrangement,
                loading: 0,

                position: ArrangementPosition::default(),
                scale: ArrangementScale::default(),
                soloed_track: None,
                grabbed_clip: None,

                selected_channel: None,
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
            Message::AudioGraph(message) => {
                let (audio_graph, path) =
                    Mutex::into_inner(Arc::into_inner(message).unwrap()).unwrap();
                self.arrangement.export(audio_graph, &path);
            }
            Message::RequestConnect((from, to)) => {
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
            Message::Select(id) => {
                self.selected_channel = Some(id);
            }
            Message::NodeVolumeChanged(id, volume) => {
                self.arrangement.node(id).volume.store(volume, Release);
            }
            Message::NodePanChanged(id, pan) => {
                self.arrangement.node(id).pan.store(pan, Release);
            }
            Message::ToggleTrackEnabled(id) => {
                self.soloed_track = None;
                return self.update(Message::ToggleNodeEnabled(id));
            }
            Message::ToggleNodeEnabled(id) => {
                self.arrangement.node(id).enabled.fetch_not(AcqRel);
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
                    return Task::future(self.arrangement.push(track))
                        .and_then(Task::done)
                        .map(Message::ConnectSucceeded);
                }
            }
            Message::LoadedPlugin(name, NoDebug(plugin)) => {
                let (gui, gui_receiver, audio_processor) = clap_host::init(
                    &plugin,
                    &name,
                    f64::from(self.meter.sample_rate),
                    self.meter.buffer_size,
                );

                let plugin_id = audio_processor.id();
                let track = MidiTrack::new(self.meter.clone(), audio_processor);
                self.plugin_ids.insert(*track.id(), plugin_id);

                return Task::batch([
                    Task::future(self.arrangement.push(track))
                        .and_then(Task::done)
                        .map(Message::ConnectSucceeded),
                    self.clap_host
                        .update(ClapHostMessage::Opened(Arc::new(Mutex::new((
                            Fragile::new(gui),
                            gui_receiver,
                        )))))
                        .map(Message::ClapHost),
                ]);
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

    pub fn view(&self) -> Element<'_, Message> {
        let element = match self.tab {
            Tab::Arrangement => self.arrangement(),
            Tab::Mixer => self.mixer(),
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
            self.position,
            self.scale,
            column(
                self.arrangement
                    .tracks()
                    .iter()
                    .enumerate()
                    .map(|(idx, track)| {
                        let id = track.id();
                        let node = track.node().clone();
                        let enabled = node.enabled.load(Acquire);

                        let mut buttons = column![
                            mouse_area(
                                radio("", enabled, Some(true), |_| {
                                    Message::ToggleTrackEnabled(id)
                                })
                                .style(if enabled {
                                    radio::default
                                } else {
                                    radio_secondary
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
                                    .style(if enabled {
                                        button::primary
                                    } else {
                                        button::secondary
                                    })
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
                                            Message::NodeVolumeChanged.with(id)
                                        ))
                                        .on_double_click(Message::NodeVolumeChanged(id, 1.0)),
                                        mouse_area(Knob::new(
                                            -1.0..=1.0,
                                            0.0,
                                            track.node().pan.load(Acquire),
                                            enabled,
                                            Message::NodePanChanged.with(id)
                                        ))
                                        .on_double_click(Message::NodePanChanged(id, 0.0)),
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
        .into()
    }

    #[expect(clippy::too_many_lines)]
    fn mixer(&self) -> Element<'_, Message> {
        let connections = self
            .selected_channel
            .as_ref()
            .map(|c| &self.arrangement.channel(*c).1);

        styled_scrollable_with_direction(
            row(self.arrangement.channels().map(|(i, (node, _))| {
                let node = node.clone();
                let id = node.id();
                let enabled = node.enabled.load(Acquire);
                let volume = node.volume.load(Acquire);
                let pan = node.pan.load(Acquire);

                let channel = button(
                    column![
                        row![
                            text(i).style(|t: &Theme| text::Style {
                                color: Some(t.extended_palette().background.weak.text)
                            }),
                            radio("", enabled, Some(true), |_| {
                                Message::ToggleNodeEnabled(id)
                            })
                            .style(if enabled {
                                radio::default
                            } else {
                                radio_secondary
                            })
                            .spacing(0.0)
                        ]
                        .spacing(5.0),
                        mouse_area(Knob::new(
                            -1.0..=1.0,
                            0.0,
                            pan,
                            enabled,
                            Message::NodePanChanged.with(id)
                        ))
                        .on_double_click(Message::NodePanChanged(id, 0.0)),
                        row![
                            PeakMeter::new(move || node.get_l_r(), enabled),
                            vertical_slider(0.0..=1.0, volume, Message::NodeVolumeChanged.with(id))
                                .step(0.001)
                                .style(if enabled {
                                    slider::default
                                } else {
                                    slider_secondary
                                })
                        ]
                        .spacing(5.0),
                        connections.map_or_else(
                            || button("").style(|_, _| button::Style::default()),
                            |connections| {
                                if Some(id) == self.selected_channel {
                                    button("").style(|_, _| button::Style::default())
                                } else if connections.contains(i) {
                                    button("^")
                                        .style(if enabled {
                                            button::primary
                                        } else {
                                            button::secondary
                                        })
                                        .on_press(Message::Disconnect((
                                            id,
                                            self.selected_channel.unwrap(),
                                        )))
                                } else {
                                    button("v")
                                        .style(if enabled {
                                            button::primary
                                        } else {
                                            button::secondary
                                        })
                                        .on_press(Message::RequestConnect((
                                            id,
                                            self.selected_channel.unwrap(),
                                        )))
                                }
                            },
                        ),
                    ]
                    .spacing(5.0)
                    .align_x(Alignment::Center),
                )
                .padding(5.0)
                .on_press(Message::Select(id))
                .style(move |t, _| button::Style {
                    background: Some(
                        if Some(id) == self.selected_channel {
                            t.extended_palette().background.weak.color
                        } else {
                            t.extended_palette().background.weakest.color
                        }
                        .into(),
                    ),
                    border: border::width(1.0).color(t.extended_palette().background.strong.color),
                    ..button::Style::default()
                });

                channel.into()
            }))
            .spacing(5.0),
            Direction::Horizontal(Scrollbar::default()),
        )
        .width(Length::Fill)
        .into()
    }

    pub fn title(&self, window: Id) -> Option<String> {
        self.clap_host.title(window)
    }

    pub fn subscription() -> Subscription<Message> {
        ClapHostView::subscription().map(Message::ClapHost)
    }

    pub fn change_tab(&mut self, tab: Tab) {
        self.tab = tab;
    }
}
