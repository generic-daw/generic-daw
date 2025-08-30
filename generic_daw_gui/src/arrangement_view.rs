use crate::{
	arrangement_view::arrangement::Batch,
	clap_host::{ClapHost, Message as ClapHostMessage},
	components::{circle_plus, icon_button, space, styled_scrollable_with_direction},
	config::Config,
	icons::{chevron_up, grip_vertical, x},
	stylefns::{bordered_box_with_radius, button_with_radius, menu_with_border, slider_secondary},
	widget::{
		LINE_HEIGHT, TEXT_HEIGHT,
		arrangement::{Action as ArrangementAction, Arrangement as ArrangementWidget},
		audio_clip::AudioClip as AudioClipWidget,
		midi_clip::MidiClip as MidiClipWidget,
		piano::Piano,
		piano_roll::{Action as PianoRollAction, PianoRoll},
		recording::Recording as RecordingWidget,
		seeker::Seeker,
		track::Track as TrackWidget,
	},
};
use arc_swap::ArcSwap;
use arrangement::Arrangement as ArrangementWrapper;
use dragking::DragEvent;
use fragile::Fragile;
use generic_daw_core::{
	AudioClip, Clip, Decibels, MidiClip, MidiNote, MusicalTime, Recording, Sample,
	audio_graph::NodeId,
	clap_host::{self, MainThreadMessage, PluginBundle, PluginDescriptor},
};
use generic_daw_utils::{EnumDispatcher, NoDebug, Vec2};
use generic_daw_widget::{dot::Dot, knob::Knob, peak_meter::PeakMeter};
use humantime::format_rfc3339;
use iced::{
	Alignment, Element, Fill, Function as _, Size, Subscription, Task, border,
	mouse::Interaction,
	overlay::menu,
	padding,
	task::Handle,
	time::every,
	widget::{
		button, column, combo_box, container, horizontal_rule, mouse_area, row,
		scrollable::{Direction, Scrollbar},
		slider, text,
		text::Wrapping,
		value, vertical_rule, vertical_slider, vertical_space,
	},
};
use iced_persistent::persistent;
use iced_split::{Strategy, vertical_split};
use node::{Node, NodeType};
use smol::unblock;
use std::{
	cmp::Ordering,
	collections::BTreeMap,
	fs::File,
	io::Read,
	iter::once,
	ops::Deref as _,
	path::Path,
	sync::{Arc, Weak},
	time::{Duration, SystemTime},
};

mod arrangement;
mod node;
mod plugin;
mod project;
mod track;

#[derive(Clone, Debug)]
enum LoadStatus {
	Loading(usize, #[expect(dead_code)] Handle),
	Loaded(u32, Weak<Sample>),
}

#[derive(Clone, Debug)]
pub enum Message {
	ClapHost(ClapHostMessage),
	Batch(Batch),

	Gc,

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

	PluginLoad(PluginDescriptor, Option<Box<[u8]>>),
	PluginRemove(usize),
	PluginMixChanged(usize, f32),
	PluginToggleEnabled(usize),
	PluginsReordered(DragEvent),

	SampleLoadFromFile(Arc<Path>),
	SampleLoadedFromFile(Arc<Path>, Option<(u32, Arc<Sample>)>),

	AddMidiClip(NodeId, MusicalTime),
	OpenMidiClip(Arc<MidiClip>),

	TrackAdd,
	TrackRemove(NodeId),
	TrackToggleEnabled(NodeId),
	TrackToggleSolo(NodeId),

	SeekTo(MusicalTime),

	ToggleRecord(NodeId),
	RecordingSplit(NodeId),
	RecordingChunk(NoDebug<Box<[f32]>>),
	StopRecord,

	ArrangementAction(ArrangementAction),
	ArrangementPositionScaleDelta(Vec2, Vec2),

	PianoRollAction(PianoRollAction),
	PianoRollPositionScaleDelta(Vec2, Vec2, Size),

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
	plugins: combo_box::State<PluginDescriptor>,

	pub arrangement: ArrangementWrapper,
	audios: BTreeMap<Arc<Path>, LoadStatus>,
	midis: Vec<Weak<ArcSwap<Vec<MidiNote>>>>,

	pub tab: Tab,

	recording: Option<(Recording, NodeId)>,

	arrangement_position: Vec2,
	arrangement_scale: Vec2,
	soloed_track: Option<NodeId>,

	piano_roll_position: Vec2,
	piano_roll_scale: Vec2,
	last_note_len: MusicalTime,
	selected_channel: Option<NodeId>,

	split_at: f32,

	tree: iced_persistent::Tree,
}

impl ArrangementView {
	pub fn new(
		config: &Config,
		plugin_bundles: &BTreeMap<PluginDescriptor, PluginBundle>,
	) -> (Self, Task<Message>) {
		let (arrangement, task) = ArrangementWrapper::create(config);
		(
			Self {
				clap_host: ClapHost::default(),
				plugins: combo_box::State::new(plugin_bundles.keys().cloned().collect()),

				arrangement,
				audios: BTreeMap::new(),
				midis: Vec::new(),

				tab: Tab::Arrangement { grabbed_clip: None },

				recording: None,

				arrangement_position: Vec2::default(),
				arrangement_scale: Vec2::new(10.0, 95.0),
				soloed_track: None,

				piano_roll_position: Vec2::new(0.0, 40.0),
				piano_roll_scale: Vec2::new(8.0, LINE_HEIGHT),
				last_note_len: MusicalTime::BEAT,
				selected_channel: None,

				split_at: 300.0,

				tree: iced_persistent::Tree::empty(),
			},
			task.map(Message::Batch),
		)
	}

	pub fn update(
		&mut self,
		message: Message,
		config: &Config,
		plugin_bundles: &BTreeMap<PluginDescriptor, PluginBundle>,
	) -> Task<Message> {
		match message {
			Message::ClapHost(msg) => return self.clap_host.update(msg).map(Message::ClapHost),
			Message::Batch(msg) => self.arrangement.update(msg),
			Message::Gc => {
				self.audios.retain(|_, audio| {
					if let LoadStatus::Loaded(_, audio) = audio {
						audio.strong_count() > 0
					} else {
						true
					}
				});
				self.midis.retain(|midi| midi.strong_count() > 0);
			}
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
				let node = self.arrangement.remove_channel(id);

				if self.selected_channel == Some(id) {
					self.selected_channel = None;
				}

				return Task::batch(node.plugins.into_iter().map(|plugin| {
					self.clap_host
						.update(ClapHostMessage::MainThread(
							plugin.id,
							MainThreadMessage::GuiClosed,
						))
						.map(Message::ClapHost)
				}));
			}
			Message::ChannelSelect(id) => {
				self.selected_channel = if self.selected_channel == Some(id) {
					None
				} else {
					Some(id)
				};
			}
			Message::ChannelVolumeChanged(id, volume) => {
				self.arrangement.node_volume_changed(id, volume);
			}
			Message::ChannelPanChanged(id, pan) => self.arrangement.node_pan_changed(id, pan),
			Message::ChannelToggleEnabled(id) => self.arrangement.node_toggle_enabled(id),
			Message::PluginLoad(descriptor, state) => {
				let selected = self.selected_channel.unwrap();

				let (mut gui, receiver, audio_processor) = clap_host::init(
					&plugin_bundles[&descriptor],
					descriptor,
					self.arrangement.rtstate().sample_rate,
					self.arrangement.rtstate().buffer_size,
				);

				if let Some(state) = state {
					gui.set_state(&state);
				}

				self.arrangement.plugin_load(selected, audio_processor);

				return self
					.clap_host
					.update(ClapHostMessage::Opened(
						Arc::new(Fragile::new(gui)),
						receiver,
					))
					.map(Message::ClapHost);
			}
			Message::PluginMixChanged(i, mix) => {
				let selected = self.selected_channel.unwrap();
				self.arrangement.plugin_mix_changed(selected, i, mix);
			}
			Message::PluginToggleEnabled(i) => {
				let selected = self.selected_channel.unwrap();
				self.arrangement.plugin_toggle_enabled(selected, i);
			}
			Message::PluginsReordered(event) => {
				if let DragEvent::Dropped {
					index,
					target_index,
				} = event && index != target_index
				{
					let selected = self.selected_channel.unwrap();

					self.arrangement.plugin_moved(selected, index, target_index);
				}
			}
			Message::PluginRemove(i) => {
				let selected = self.selected_channel.unwrap();
				let plugin = self.arrangement.plugin_remove(selected, i);
				return self
					.clap_host
					.update(ClapHostMessage::MainThread(
						plugin.id,
						MainThreadMessage::GuiClosed,
					))
					.map(Message::ClapHost);
			}
			Message::SampleLoadFromFile(path) => {
				if let Some(entry) = self.audios.get_mut(&path) {
					match entry {
						LoadStatus::Loading(count, _) => {
							*count += 1;
							return Task::none();
						}
						LoadStatus::Loaded(crc, audio) => {
							if let Some(audio) = audio.upgrade() {
								let crc = *crc;
								return self.update(
									Message::SampleLoadedFromFile(path, Some((crc, audio))),
									config,
									plugin_bundles,
								);
							}
						}
					}
				}

				let sample_rate = self.arrangement.rtstate().sample_rate;

				let (task, handle) = Task::perform(
					{
						let path = path.clone();
						unblock(move || {
							let sample = Sample::create(path.clone(), sample_rate)?;
							let crc = crc(File::open(path).ok()?);
							Some((crc, sample))
						})
					},
					Message::SampleLoadedFromFile.with(path.clone()),
				)
				.abortable();
				let handle = handle.abort_on_drop();

				self.audios
					.insert(path.clone(), LoadStatus::Loading(1, handle));

				return task;
			}
			Message::SampleLoadedFromFile(path, audio) => {
				let Some((crc, audio)) = audio else {
					self.audios.remove(&path);
					return Task::none();
				};

				let count = match self.audios[&path] {
					LoadStatus::Loading(count, _) => {
						self.audios.insert(
							path.clone(),
							LoadStatus::Loaded(crc, Arc::downgrade(&audio)),
						);

						count
					}
					LoadStatus::Loaded(..) => 1,
				};

				let clip = AudioClip::create(audio, self.arrangement.rtstate());
				let end = clip.position.end();

				let mut futs = Vec::new();
				let mut track = 0;

				for _ in 0..count {
					while self.arrangement.tracks().get(track).is_some_and(|track| {
						track.clips.iter().any(|clip| clip.position().start() < end)
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
			Message::OpenMidiClip(clip) => {
				self.tab = Tab::PianoRoll {
					clip,
					grabbed_note: None,
				}
			}
			Message::AddMidiClip(track, pos) => {
				let pattern = Arc::default();
				self.midis.push(Arc::downgrade(&pattern));
				let clip = MidiClip::create(pattern);
				clip.position.trim_end_to(
					MusicalTime::BEAT * u32::from(self.arrangement.rtstate().numerator),
				);
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

				return self
					.update(
						Message::ArrangementPositionScaleDelta(Vec2::ZERO, Vec2::ZERO),
						config,
						plugin_bundles,
					)
					.chain(self.update(Message::ChannelRemove(id), config, plugin_bundles));
			}
			Message::TrackToggleEnabled(id) => {
				self.soloed_track = None;
				return self.update(Message::ChannelToggleEnabled(id), config, plugin_bundles);
			}
			Message::TrackToggleSolo(id) => {
				if self.soloed_track == Some(id) {
					self.soloed_track = None;
					self.arrangement.enable_all_tracks();
				} else {
					self.soloed_track = Some(id);
					self.arrangement.solo_track(id);
				}
			}
			Message::SeekTo(pos) => self.arrangement.seek_to(pos),
			Message::ToggleRecord(id) => {
				if let Some((_, i)) = &self.recording {
					return self.update(
						if *i == id {
							Message::StopRecord
						} else {
							Message::RecordingSplit(id)
						},
						config,
						plugin_bundles,
					);
				}

				let (recording, receiver) = Recording::create(
					recording_path(),
					self.arrangement.rtstate(),
					config.input_device.name.as_deref(),
					config.input_device.sample_rate.unwrap_or(44100),
					config.input_device.buffer_size.unwrap_or(1024),
				);
				self.recording = Some((recording, id));

				self.arrangement.play();

				return Task::stream(receiver)
					.map(NoDebug)
					.map(Message::RecordingChunk);
			}
			Message::RecordingSplit(id) => {
				if let Some((mut recording, track)) = self.recording.take() {
					let mut pos = MusicalTime::from_samples(
						self.arrangement.rtstate().sample,
						self.arrangement.rtstate(),
					);
					(pos, recording.position) = (recording.position, pos);

					let sample = recording.split_off(recording_path(), self.arrangement.rtstate());
					self.audios.insert(
						sample.path.clone(),
						LoadStatus::Loaded(
							crc(File::open(&sample.path).unwrap()),
							Arc::downgrade(&sample),
						),
					);

					let track = self.arrangement.track_of(track).unwrap();
					let clip = AudioClip::create(sample, self.arrangement.rtstate());
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
					self.arrangement.pause();
					let pos = recording.position;

					let sample = recording.finalize();
					self.audios.insert(
						sample.path.clone(),
						LoadStatus::Loaded(
							crc(File::open(&sample.path).unwrap()),
							Arc::downgrade(&sample),
						),
					);

					let track = self.arrangement.track_of(track).unwrap();
					let clip = AudioClip::create(sample, self.arrangement.rtstate());
					clip.position.move_to(pos);
					self.arrangement.add_clip(track, clip);
				}
			}
			Message::ArrangementAction(action) => self.handle_arrangement_action(action),
			Message::ArrangementPositionScaleDelta(pos, scale) => {
				let old_scale = self.arrangement_scale;

				self.arrangement_scale += scale;
				self.arrangement_scale.x = self.arrangement_scale.x.clamp(3.0, 15f32.next_down());
				self.arrangement_scale.y = self.arrangement_scale.y.clamp(77.0, 200.0);

				if scale == Vec2::ZERO || old_scale != self.arrangement_scale {
					self.arrangement_position += pos;
					self.arrangement_position.x = self.arrangement_position.x.max(0.0);
					self.arrangement_position.y = self.arrangement_position.y.clamp(
						0.0,
						self.arrangement.tracks().len().saturating_sub(1) as f32,
					);
				}
			}
			Message::PianoRollAction(action) => self.handle_piano_roll_action(action),
			Message::PianoRollPositionScaleDelta(pos, scale, size) => {
				let old_scale = self.piano_roll_scale;

				self.piano_roll_scale += scale;
				self.piano_roll_scale.x = self.piano_roll_scale.x.clamp(3.0, 15f32.next_down());
				self.piano_roll_scale.y = self
					.piano_roll_scale
					.y
					.clamp(LINE_HEIGHT, 2.0 * LINE_HEIGHT);

				if scale == Vec2::ZERO || old_scale == self.piano_roll_scale {
					self.piano_roll_position += pos;
					self.piano_roll_position.x = self.piano_roll_position.x.max(0.0);
					self.piano_roll_position.y = self.piano_roll_position.y.clamp(
						0.0,
						128.0 - ((size.height - LINE_HEIGHT) / self.piano_roll_scale.y),
					);
				}
			}
			Message::SplitAt(split_at) => self.split_at = split_at.clamp(200.0, 400.0),
		}

		Task::none()
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
			ArrangementAction::Delete(track, clip) => self.arrangement.remove_clip(track, clip),
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

	pub fn loading(&self) -> bool {
		self.audios
			.values()
			.any(|audio| matches!(audio, LoadStatus::Loading(..)))
	}

	pub fn view(&self) -> Element<'_, Message> {
		let view = match &self.tab {
			Tab::Arrangement { .. } => self.arrangement(),
			Tab::Mixer => self.mixer(),
			Tab::PianoRoll { clip, .. } => self.piano_roll(clip),
		};
		self.arrangement.clear_l_r();
		view
	}

	fn arrangement(&self) -> Element<'_, Message> {
		Seeker::new(
			self.arrangement.rtstate(),
			&self.arrangement_position,
			&self.arrangement_scale,
			column(
				self.arrangement
					.tracks()
					.iter()
					.map(|track| track.id)
					.map(|id| {
						let node = &self.arrangement.node(id).0;

						container(
							row![
								row![
									PeakMeter::new(node.l_r.get()[0], node.enabled),
									PeakMeter::new(node.l_r.get()[1], node.enabled)
								]
								.spacing(2),
								column![
									Knob::new(
										0.0..=1.0,
										node.volume.cbrt(),
										node.enabled,
										move |v| Message::ChannelVolumeChanged(id, v.powi(3))
									)
									.tooltip(Decibels::from_amplitude(node.volume)),
									Knob::new(
										-1.0..=1.0,
										node.pan,
										node.enabled,
										Message::ChannelPanChanged.with(id)
									)
									.center(0.0)
									.reset(0.0)
									.tooltip(pan_to_string(node.pan)),
								]
								.spacing(5)
								.wrap(),
								column![
									icon_button(
										text('M'),
										if node.enabled {
											button::primary
										} else {
											button::secondary
										}
									)
									.on_press(Message::TrackToggleEnabled(id)),
									icon_button(
										text('S'),
										if self.soloed_track == Some(id) {
											button::warning
										} else if node.enabled {
											button::primary
										} else {
											button::secondary
										}
									)
									.on_press(Message::TrackToggleSolo(id)),
									icon_button(
										x(),
										if node.enabled {
											button::danger
										} else {
											button::secondary
										}
									)
									.on_press(Message::TrackRemove(id)),
									column![
										vertical_space(),
										button(
											Dot::new(
												self.recording
													.as_ref()
													.is_some_and(|&(_, i)| i == id)
											)
											.radius(5.0)
										)
										.padding(1.5)
										.on_press(Message::ToggleRecord(id))
										.style(
											if self
												.recording
												.as_ref()
												.is_some_and(|&(_, i)| i == id)
											{
												button::danger
											} else if node.enabled {
												button::primary
											} else {
												button::secondary
											}
										)
									]
								]
								.spacing(5)
							]
							.spacing(5),
						)
						.style(bordered_box_with_radius(0))
						.padding(5)
						.height(self.arrangement_scale.y)
					})
					.map(Element::new)
					.chain(once(
						container(circle_plus().on_press(Message::TrackAdd))
							.padding(padding::right(5).top(5))
							.into(),
					)),
			)
			.align_x(Alignment::Center),
			ArrangementWidget::new(
				self.arrangement.rtstate(),
				&self.arrangement_position,
				&self.arrangement_scale,
				column(
					self.arrangement
						.tracks()
						.iter()
						.map(|track| {
							let id = track.id;
							let node = &self.arrangement.node(track.id).0;

							let clips_iter = track.clips.iter().map(|clip| match clip {
								Clip::Audio(clip) => AudioClipWidget::new(
									clip,
									self.arrangement.rtstate(),
									&self.arrangement_position,
									&self.arrangement_scale,
									node.enabled,
								)
								.into(),
								Clip::Midi(clip) => MidiClipWidget::new(
									clip,
									self.arrangement.rtstate(),
									&self.arrangement_position,
									&self.arrangement_scale,
									node.enabled,
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
												self.arrangement.rtstate(),
												&self.arrangement_position,
												&self.arrangement_scale,
											)
											.into(),
										)),
									)
								} else {
									EnumDispatcher::B(clips_iter)
								};

							TrackWidget::new(
								self.arrangement.rtstate(),
								&self.arrangement_position,
								&self.arrangement_scale,
								clips_iter,
								Message::AddMidiClip.with(id),
							)
						})
						.map(Element::new),
				),
				Message::ArrangementAction,
			),
			Message::SeekTo,
			|p, s, _| Message::ArrangementPositionScaleDelta(p, s),
		)
		.into()
	}

	fn mixer(&self) -> Element<'_, Message> {
		fn channel<'a>(
			selected_channel: Option<NodeId>,
			name: String,
			node: &'a Node,
			buttons: impl Fn(bool, NodeId) -> Element<'a, Message>,
			connect: impl Fn(bool, NodeId) -> Element<'a, Message>,
		) -> Element<'a, Message> {
			button(
				column![
					row![
						column![
							text(name).size(13).line_height(1.0),
							Knob::new(
								-1.0..=1.0,
								node.pan,
								node.enabled,
								Message::ChannelPanChanged.with(node.id)
							)
							.center(0.0)
							.reset(0.0)
							.tooltip(pan_to_string(node.pan)),
						]
						.spacing(3)
						.align_x(Alignment::Center),
						buttons(node.enabled, node.id)
					]
					.spacing(3),
					container(value(Decibels::from_amplitude(node.volume)).line_height(1.0))
						.width(54)
						.style(bordered_box_with_radius(0))
						.align_x(Alignment::Center)
						.padding(2),
					row![
						PeakMeter::new(node.l_r.get()[0].cbrt(), node.enabled).width(16.0),
						vertical_slider(0.0..=1.0, node.volume.cbrt(), |v| {
							Message::ChannelVolumeChanged(node.id, v.powi(3))
						})
						.step(f32::EPSILON)
						.style(if node.enabled {
							slider::default
						} else {
							slider_secondary
						}),
						PeakMeter::new(node.l_r.get()[1].cbrt(), node.enabled).width(16.0),
					]
					.spacing(3),
					connect(node.enabled, node.id)
				]
				.spacing(3)
				.align_x(Alignment::Center),
			)
			.padding(5)
			.on_press(Message::ChannelSelect(node.id))
			.style(move |t, _| {
				let pair = if Some(node.id) == selected_channel {
					t.extended_palette().background.weak
				} else {
					t.extended_palette().background.weakest
				};

				button::Style {
					background: Some(pair.color.into()),
					text_color: pair.text,
					border: border::width(1).color(t.extended_palette().background.strong.color),
					..button::Style::default()
				}
			})
			.into()
		}

		let selected_channel = self.selected_channel.map(|c| self.arrangement.node(c));

		let connect = |enabled: bool, id: NodeId| {
			selected_channel.map_or_else(
				|| Element::new(space().height(LINE_HEIGHT)),
				|(node, connections)| {
					let selected_channel = self.selected_channel.unwrap();

					if node.ty == NodeType::Master || id == selected_channel {
						space().height(LINE_HEIGHT).into()
					} else {
						let connected = connections.contains(*id);

						button(chevron_up())
							.style(if enabled && connected {
								button::primary
							} else {
								button::secondary
							})
							.padding(0)
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

		let mixer_panel = persistent(
			styled_scrollable_with_direction(
				row(once(channel(
					self.selected_channel,
					"M".to_owned(),
					&self.arrangement.master().0,
					|enabled, id| {
						column![
							icon_button(
								text('M'),
								if enabled {
									button::primary
								} else {
									button::secondary
								}
							)
							.on_press(Message::ChannelToggleEnabled(id)),
							space().height(13),
							space().height(13)
						]
						.spacing(5)
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
							let name = format!("T{}", i + 1);
							let node = &self.arrangement.node(track.id).0;

							channel(
								self.selected_channel,
								name,
								node,
								|enabled, id| {
									column![
										icon_button(
											text('M'),
											if enabled {
												button::primary
											} else {
												button::secondary
											}
										)
										.on_press(Message::TrackToggleEnabled(id)),
										icon_button(
											text('S'),
											if self.soloed_track == Some(id) {
												button::warning
											} else if enabled {
												button::primary
											} else {
												button::secondary
											}
										)
										.on_press(Message::TrackToggleSolo(id)),
										icon_button(
											x(),
											if enabled {
												button::danger
											} else {
												button::secondary
											}
										)
										.on_press(Message::TrackRemove(id))
									]
									.spacing(5)
									.into()
								},
								|_, _| space().height(LINE_HEIGHT).into(),
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
							let name = format!("C{}", i + 1);

							channel(
								self.selected_channel,
								name,
								node,
								|enabled, id| {
									column![
										icon_button(
											text('M'),
											if enabled {
												button::primary
											} else {
												button::secondary
											}
										)
										.on_press(Message::ChannelToggleEnabled(id)),
										space().height(13),
										icon_button(
											x(),
											if enabled {
												button::danger
											} else {
												button::secondary
											}
										)
										.on_press(Message::ChannelRemove(id)),
									]
									.spacing(5)
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
				.chain(once(circle_plus().on_press(Message::ChannelAdd).into())))
				.align_y(Alignment::Center)
				.spacing(5),
				Direction::Horizontal(Scrollbar::default()),
			)
			.width(Fill),
			&self.tree,
		);

		if let Some(selected) = self.selected_channel {
			vertical_split(
				mixer_panel,
				column![
					combo_box(&self.plugins, "Add Plugin", None, |descriptor| {
						Message::PluginLoad(descriptor, None)
					})
					.menu_style(menu_with_border(menu::default, border::width(0)))
					.width(Fill),
					container(horizontal_rule(1)).padding([5, 0]),
					styled_scrollable_with_direction(
						dragking::column(
							self.arrangement
								.node(selected)
								.0
								.plugins
								.iter()
								.enumerate()
								.map(|(i, plugin)| {
									row![
										Knob::new(
											0.0..=1.0,
											plugin.mix,
											plugin.enabled,
											Message::PluginMixChanged.with(i)
										)
										.radius(TEXT_HEIGHT)
										.tooltip(((plugin.mix * 100.0) as u8).to_string() + "%"),
										button(
											container(
												text(&*plugin.descriptor.name)
													.wrapping(Wrapping::None)
											)
											.clip(true)
										)
										.style(button_with_radius(
											if plugin.enabled {
												button::primary
											} else {
												button::secondary
											},
											border::left(5)
										))
										.width(Fill)
										.on_press(Message::ClapHost(
											ClapHostMessage::MainThread(
												plugin.id,
												MainThreadMessage::GuiRequestShow,
											)
										)),
										column![
											icon_button(
												text('M'),
												if plugin.enabled {
													button::primary
												} else {
													button::secondary
												}
											)
											.on_press(Message::PluginToggleEnabled(i)),
											icon_button(
												x(),
												if plugin.enabled {
													button::danger
												} else {
													button::secondary
												}
											)
											.on_press(Message::PluginRemove(i)),
										]
										.spacing(5),
										mouse_area(
											container(
												grip_vertical().line_height(
													(LINE_HEIGHT + 10.0) / LINE_HEIGHT
												)
											)
											.style(bordered_box_with_radius(border::right(5)))
										)
										.interaction(Interaction::Grab),
									]
									.spacing(5)
									.into()
								})
						)
						.spacing(5)
						.on_drag(Message::PluginsReordered),
						Direction::Vertical(Scrollbar::default())
					)
					.height(Fill)
				],
				self.split_at,
				Message::SplitAt,
			)
			.strategy(Strategy::End)
			.into()
		} else {
			mixer_panel.into()
		}
	}

	fn piano_roll<'a>(&'a self, clip: &'a MidiClip) -> Element<'a, Message> {
		Seeker::new(
			self.arrangement.rtstate(),
			&self.piano_roll_position,
			&self.piano_roll_scale,
			Piano::new(&self.piano_roll_position, &self.piano_roll_scale),
			PianoRoll::new(
				clip.pattern.load().deref().clone(),
				self.arrangement.rtstate(),
				&self.piano_roll_position,
				&self.piano_roll_scale,
				Message::PianoRollAction,
			),
			Message::SeekTo,
			Message::PianoRollPositionScaleDelta,
		)
		.with_offset(
			clip.position
				.start()
				.to_samples_f(self.arrangement.rtstate())
				- clip
					.position
					.offset()
					.to_samples_f(self.arrangement.rtstate()),
		)
		.into()
	}

	pub fn subscription(&self) -> Subscription<Message> {
		Subscription::batch([
			every(Duration::from_secs(60)).map(|_| Message::Gc),
			self.clap_host.subscription().map(Message::ClapHost),
		])
	}
}

fn pan_to_string(pan: f32) -> String {
	let pan = (pan * 100.0) as i8;
	match pan.cmp(&0) {
		Ordering::Greater => format!("{}% right", pan.abs()),
		Ordering::Equal => "center".to_owned(),
		Ordering::Less => format!("{}% left", pan.abs()),
	}
}

fn recording_path() -> Arc<Path> {
	let file_name = format!("recording-{}.wav", format_rfc3339(SystemTime::now()));

	let data_dir = dirs::data_dir().unwrap().join("Generic Daw");
	_ = std::fs::create_dir(&data_dir);

	data_dir.join(file_name).into()
}

fn crc(mut r: impl Read) -> u32 {
	#[repr(align(8))]
	struct Aligned([u8; 4096]);
	let mut buf = Aligned([0; 4096]);

	let mut crc = 0;
	let mut len;

	while {
		len = r.read(&mut buf.0).unwrap();
		len != 0
	} {
		crc = crc32c::crc32c_append(crc, &buf.0[..len]);
	}

	crc
}
