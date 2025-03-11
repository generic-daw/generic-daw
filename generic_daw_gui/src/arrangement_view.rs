use crate::{
    clap_host_view::{ClapHostView, Message as ClapHostMessage},
    components::{styled_button, styled_pick_list, styled_scrollable_with_direction, styled_svg},
    daw::PLUGINS,
    stylefns::{button_with_enabled, radio_with_enabled, slider_with_enabled},
    widget::{
        Arrangement as ArrangementWidget, ArrangementPosition, ArrangementScale,
        AudioClip as AudioClipWidget, Knob, LINE_HEIGHT, PeakMeter, TEXT_HEIGHT,
        Track as TrackWidget,
    },
};
use arrangement::NodeType;
use fragile::Fragile;
use generic_daw_core::{
    AudioClip, AudioTrack, InterleavedAudio, Meter, MidiTrack, MixerNode, Position,
    audio_graph::{AudioGraph, AudioGraphNodeImpl as _, NodeId},
    clap_host::{self, MainThreadMessage, PluginDescriptor, PluginId, PluginType},
};
use generic_daw_utils::{EnumDispatcher, HoleyVec};
use iced::{
    Alignment, Element, Function as _, Length, Subscription, Task,
    border::{self, Radius},
    futures::TryFutureExt as _,
    mouse::Interaction,
    widget::{
        button, column, container, horizontal_rule, horizontal_space, mouse_area, radio, row,
        scrollable::{Direction, Scrollbar},
        svg, text, vertical_rule, vertical_slider, vertical_space,
    },
    window::Id,
};
use std::{
    iter::once,
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
    RequestConnect((NodeId, NodeId)),
    ConnectSucceeded((NodeId, NodeId)),
    Disconnect((NodeId, NodeId)),
    SelectChannel(NodeId),
    AddChannel,
    RemoveChannel(NodeId),
    NodeVolumeChanged(NodeId, f32),
    NodePanChanged(NodeId, f32),
    NodeToggleEnabled(NodeId),
    TrackToggleEnabled(NodeId),
    TrackToggleSolo(usize),
    RemoveTrack(usize),
    LoadSample(Box<Path>),
    LoadedSample(Option<Arc<InterleavedAudio>>),
    LoadInstrumentPlugin(PluginDescriptor),
    LoadAudioEffectPlugin(PluginDescriptor),
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
    instrument_by_track: HoleyVec<PluginId>,
    audio_effects_by_channel: HoleyVec<Vec<(PluginId, String)>>,

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
                instrument_by_track: HoleyVec::default(),
                audio_effects_by_channel: vec![Some(vec![])].into(),

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
            Message::SelectChannel(id) => {
                self.selected_channel = Some(id);
            }
            Message::AddChannel => {
                let (id, fut) = self.arrangement.add_channel();
                self.audio_effects_by_channel.insert(*id, vec![]);
                return Task::future(fut)
                    .and_then(Task::done)
                    .map(Message::ConnectSucceeded);
            }
            Message::RemoveChannel(id) => {
                self.arrangement.remove_channel(id);
            }
            Message::NodeVolumeChanged(id, volume) => {
                self.arrangement.node(id).0.volume.store(volume, Release);
            }
            Message::NodePanChanged(id, pan) => {
                self.arrangement.node(id).0.pan.store(pan, Release);
            }
            Message::NodeToggleEnabled(id) => {
                self.arrangement.node(id).0.enabled.fetch_not(AcqRel);
            }
            Message::TrackToggleEnabled(id) => {
                self.soloed_track = None;
                return self.update(Message::NodeToggleEnabled(id));
            }
            Message::TrackToggleSolo(track) => {
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
                let id = self.arrangement.remove_track(track);
                return Task::batch({
                    let iter = self
                        .audio_effects_by_channel
                        .remove(*id)
                        .unwrap()
                        .into_iter()
                        .map(|(id, _)| id);

                    if let Some(id) = self.instrument_by_track.remove(*id) {
                        EnumDispatcher::A(once(id).chain(iter))
                    } else {
                        EnumDispatcher::B(iter)
                    }
                    .map(|id| {
                        Message::ClapHost(ClapHostMessage::MainThread(
                            id,
                            MainThreadMessage::GuiRequestHide,
                        ))
                    })
                    .map(Task::done)
                });
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
                    self.audio_effects_by_channel.insert(*track.id(), vec![]);
                    track
                        .clips
                        .push(AudioClip::create(audio_file, self.meter.clone()));
                    return Task::future(self.arrangement.add_track(track))
                        .and_then(Task::done)
                        .map(Message::ConnectSucceeded);
                }
            }
            Message::LoadInstrumentPlugin(name) => {
                let (gui, gui_receiver, audio_processor) = clap_host::init(
                    &PLUGINS[&name],
                    &name,
                    f64::from(self.meter.sample_rate),
                    self.meter.buffer_size,
                );

                let plugin_id = audio_processor.id();
                let track = MidiTrack::new(self.meter.clone(), audio_processor);
                self.instrument_by_track.insert(*track.id(), plugin_id);
                self.audio_effects_by_channel.insert(*track.id(), vec![]);

                return Task::batch([
                    Task::future(self.arrangement.add_track(track))
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
            Message::LoadAudioEffectPlugin(name) => {
                let Some(selected) = self.selected_channel else {
                    return Task::none();
                };
                let node = self.arrangement.node(selected).0.clone();

                let (gui, gui_receiver, audio_processor) = clap_host::init(
                    &PLUGINS[&name],
                    &name,
                    f64::from(self.meter.sample_rate),
                    self.meter.buffer_size,
                );

                let id = audio_processor.id();

                let mut effects = Arc::into_inner(node.effects.swap(Arc::new(vec![]))).unwrap();
                effects.push(Mutex::new(audio_processor));
                node.effects.store(Arc::new(effects));

                self.audio_effects_by_channel
                    .get_mut(*selected)
                    .unwrap()
                    .push((id, gui.name().to_owned()));

                return self
                    .clap_host
                    .update(ClapHostMessage::Opened(Arc::new(Mutex::new((
                        Fragile::new(gui),
                        gui_receiver,
                    )))))
                    .map(Message::ClapHost);
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
                                    Message::TrackToggleEnabled(id)
                                })
                                .style(move |t, s| radio_with_enabled(t, s, enabled))
                                .spacing(0.0)
                            )
                            .on_right_press(Message::TrackToggleSolo(idx)),
                            button(styled_svg(X.clone()).height(TEXT_HEIGHT))
                                .style(|t, s| {
                                    let mut style = button::danger(t, s);
                                    style.border.radius = Radius::new(f32::INFINITY);
                                    style
                                })
                                .padding(0.0)
                                .on_press(Message::RemoveTrack(idx)),
                        ]
                        .spacing(5.0)
                        .align_x(Alignment::Center);

                        if let Some(&id) = self.instrument_by_track.get(*track.id()) {
                            buttons = buttons.extend([
                                vertical_space().into(),
                                button(styled_svg(REOPEN.clone()).height(LINE_HEIGHT))
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
                            .style(|t| container::transparent(t)
                                .background(t.extended_palette().background.weak.color)
                                .border(
                                    border::width(1.0)
                                        .color(t.extended_palette().background.strong.color)
                                ))
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
                                Message::NodePanChanged.with(id)
                            )),
                            PeakMeter::new(move || node.get_l_r(), enabled)
                        ]
                        .spacing(5.0)
                        .align_x(Alignment::Center),
                        column![
                            toggle(enabled, id),
                            remove(enabled, id),
                            vertical_slider(0.0..=1.0, volume, Message::NodeVolumeChanged.with(id))
                                .step(0.001)
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
            .on_press(Message::SelectChannel(id))
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
                || button("").style(|_, _| button::Style::default()),
                |(_, connections, ty)| {
                    let selected_channel = self.selected_channel.unwrap();

                    if *ty == NodeType::Master || id == selected_channel {
                        button("").style(|_, _| button::Style::default())
                    } else {
                        let connected = connections.contains(*id);

                        button(if connected { "^" } else { "v" })
                            .style(move |t, s| button_with_enabled(t, s, enabled))
                            .on_press(if connected {
                                Message::Disconnect((id, selected_channel))
                            } else {
                                Message::RequestConnect((id, selected_channel))
                            })
                    }
                },
            )
        };

        row![
            styled_scrollable_with_direction(
                row(once(channel(
                    self.selected_channel,
                    "M".to_owned(),
                    self.arrangement.master().0.clone(),
                    |enabled, id| {
                        radio("", enabled, Some(true), |_| Message::NodeToggleEnabled(id))
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
                                        .style(move |t, s| radio_with_enabled(t, s, enabled))
                                        .spacing(0.0),
                                    )
                                    .on_right_press(Message::TrackToggleSolo(i))
                                    .into()
                                },
                                |_, _| {
                                    button(styled_svg(X.clone()).height(TEXT_HEIGHT))
                                        .style(|t, s| {
                                            let mut style = button::danger(t, s);
                                            style.border.radius = Radius::new(f32::INFINITY);
                                            style
                                        })
                                        .padding(0.0)
                                        .on_press(Message::RemoveTrack(i))
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
                                        Message::NodeToggleEnabled(id)
                                    })
                                    .style(move |t, s| radio_with_enabled(t, s, enabled))
                                    .spacing(0.0)
                                    .into()
                                },
                                |_, id| {
                                    button(styled_svg(X.clone()).height(TEXT_HEIGHT))
                                        .style(|t, s| {
                                            let mut style = button::danger(t, s);
                                            style.border.radius = Radius::new(f32::INFINITY);
                                            style
                                        })
                                        .padding(0.0)
                                        .on_press(Message::RemoveChannel(id))
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
                        .on_press(Message::AddChannel)
                        .into(),
                )))
                .spacing(5.0),
                Direction::Horizontal(Scrollbar::default()),
            )
            .width(Length::Fill),
            self.selected_channel.map_or_else(
                || Element::new(vertical_space().width(0)),
                |id| {
                    row![
                        vertical_rule(11),
                        column![
                            styled_pick_list(
                                PLUGINS
                                    .keys()
                                    .filter(|d| d.ty == PluginType::AudioEffect)
                                    .collect::<Box<[_]>>(),
                                None::<&PluginDescriptor>,
                                |p| Message::LoadAudioEffectPlugin(p.to_owned()),
                            )
                            .width(Length::Fill)
                            .placeholder("Add Effect"),
                            if self.audio_effects_by_channel[*id].is_empty() {
                                Element::new(horizontal_space())
                            } else {
                                horizontal_rule(11.0).into()
                            },
                            styled_scrollable_with_direction(
                                column({
                                    self.audio_effects_by_channel[*id].iter().map(|(id, name)| {
                                        styled_button(text(name))
                                            .width(Length::Fill)
                                            .on_press(Message::ClapHost(
                                                ClapHostMessage::MainThread(
                                                    *id,
                                                    MainThreadMessage::GuiRequestShow,
                                                ),
                                            ))
                                            .into()
                                    })
                                }),
                                Direction::Vertical(Scrollbar::default())
                            )
                        ]
                        .width(Length::Fixed(300.0))
                    ]
                    .into()
                },
            ),
        ]
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
