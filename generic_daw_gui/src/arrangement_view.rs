use crate::{
	clap_host::{ClapHost, Message as ClapHostMessage},
	components::{
		circle_plus, icon_button, space, styled_combo_box, styled_scrollable_with_direction,
	},
	config::Config,
	icons::{chevron_up, grip_vertical, x},
	stylefns::{button_with_base, slider_with_enabled},
	widget::{
		AnimatedDot, Arrangement as ArrangementWidget, AudioClip as AudioClipWidget, Knob,
		LINE_HEIGHT, MidiClip as MidiClipWidget, PeakMeter, Piano, PianoRoll,
		Recording as RecordingWidget, Seeker, TEXT_HEIGHT, Track as TrackWidget,
		arrangement::Action as ArrangementAction, piano_roll::Action as PianoRollAction,
	},
};
use arc_swap::ArcSwap;
use arrangement::Arrangement as ArrangementWrapper;
use dragking::DragEvent;
use fragile::Fragile;
use generic_daw_core::{
	self as core, AudioClip, Clip, Decibels, MidiClip, MidiKey, MidiNote, Mixer, MusicalTime,
	Recording, Sample, Update,
	audio_graph::{NodeId, NodeImpl as _},
	clap_host::{self, MainThreadMessage, PluginBundle, PluginDescriptor},
};
use generic_daw_project::{proto, reader::Reader, writer::Writer};
use generic_daw_utils::{EnumDispatcher, NoDebug, Vec2, hash_reader};
use iced::{
	Alignment, Element, Fill, Function as _, Size, Subscription, Task, border,
	mouse::Interaction,
	padding,
	widget::{
		button, column, combo_box, container, horizontal_rule, mouse_area, row,
		scrollable::{Direction, Scrollbar},
		text,
		text::Wrapping,
		vertical_rule, vertical_slider, vertical_space,
	},
};
use iced_split::{Split, Strategy};
use log::info;
use node::{Node, NodeType};
use smol::unblock;
use std::{
	cmp::Ordering,
	collections::{BTreeMap, BTreeSet, HashMap},
	fs::File,
	hash::{DefaultHasher, Hash as _, Hasher as _},
	io::{Read as _, Write as _},
	iter::once,
	ops::Deref as _,
	path::Path,
	sync::{Arc, Weak, mpsc},
	time::Instant,
};
use walkdir::WalkDir;

mod arrangement;
mod node;
mod plugin;
mod track;

#[derive(Clone, Debug)]
enum LoadStatus {
	Loading(usize),
	Loaded(Weak<Sample>),
}

#[derive(Clone, Debug)]
pub enum Message {
	ClapHost(ClapHostMessage),
	Update(Update),

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
	SampleLoadedFromFile(Arc<Path>, Option<Arc<Sample>>),

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
	plugin_descriptors: combo_box::State<PluginDescriptor>,

	pub arrangement: ArrangementWrapper,
	loading: BTreeSet<Arc<Path>>,
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
}

impl ArrangementView {
	pub fn new(
		config: &Config,
		plugin_bundles: &BTreeMap<PluginDescriptor, PluginBundle>,
	) -> (Self, Task<Message>) {
		let (arrangement, receiver) = ArrangementWrapper::create(config);
		(
			Self {
				clap_host: ClapHost::default(),
				plugin_descriptors: combo_box::State::new(plugin_bundles.keys().cloned().collect()),

				arrangement,
				loading: BTreeSet::new(),
				audios: BTreeMap::new(),
				midis: Vec::new(),

				tab: Tab::Arrangement { grabbed_clip: None },

				recording: None,

				arrangement_position: Vec2::default(),
				arrangement_scale: Vec2::new(10.0, const { LINE_HEIGHT * 4.0 + 15.0 }),
				soloed_track: None,

				piano_roll_position: Vec2::new(0.0, 40.0),
				piano_roll_scale: Vec2::new(8.0, LINE_HEIGHT),
				last_note_len: MusicalTime::BEAT,
				selected_channel: None,

				split_at: 300.0,
			},
			Task::stream(receiver).map(Message::Update),
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
			Message::Update(msg) => self.arrangement.update(msg),
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
			Message::PluginLoad(descriptor) => {
				let selected = self.selected_channel.unwrap();

				let (gui, receiver, audio_processor) = clap_host::init(
					&plugin_bundles[&descriptor],
					descriptor,
					self.arrangement.rtstate().sample_rate,
					self.arrangement.rtstate().buffer_size,
				);

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
						LoadStatus::Loading(count) => {
							*count += 1;

							return Task::none();
						}
						LoadStatus::Loaded(audio) => {
							if let Some(audio) = audio.upgrade() {
								return self.update(
									Message::SampleLoadedFromFile(path, Some(audio)),
									config,
									plugin_bundles,
								);
							}
						}
					}
				}

				self.audios.insert(path.clone(), LoadStatus::Loading(1));
				self.loading.insert(path.clone());
				let sample_rate = self.arrangement.rtstate().sample_rate;

				return Task::perform(
					{
						let path = path.clone();
						unblock(move || Sample::create(path, sample_rate))
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
					Self::make_recording_path(),
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

					let track = self.arrangement.track_of(track).unwrap();
					let clip = AudioClip::create(
						recording
							.split_off(Self::make_recording_path(), self.arrangement.rtstate()),
						self.arrangement.rtstate(),
					);
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

					let track = self.arrangement.track_of(track).unwrap();
					let clip = AudioClip::create(recording.finalize(), self.arrangement.rtstate());
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

	fn make_recording_path() -> Arc<Path> {
		let mut hasher = DefaultHasher::new();
		Instant::now().hash(&mut hasher);

		let file_name = "recording-".to_owned() + &hasher.finish().to_string() + ".wav";

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

	pub fn save(&mut self, path: &Path) {
		let mut writer = Writer::new(
			u32::from(self.arrangement.rtstate().bpm),
			u32::from(self.arrangement.rtstate().numerator),
		);

		let mut audios = HashMap::new();
		for entry in &self.audios {
			let audio = match entry.1 {
				LoadStatus::Loaded(audio) => {
					let Some(audio) = audio.upgrade() else {
						continue;
					};
					audio
				}
				LoadStatus::Loading(..) => continue,
			};
			audios.insert(
				audio.path.clone(),
				writer.push_audio(&audio.name, audio.hash),
			);
		}

		let mut midis = HashMap::new();
		for entry in &self.midis {
			let Some(pattern) = entry.upgrade() else {
				continue;
			};

			midis.insert(
				Arc::as_ptr(&pattern).addr(),
				writer.push_midi(pattern.load().iter().map(|note| proto::Note {
					key: u32::from(note.key.0),
					velocity: note.velocity as f32,
					start: note.start.into(),
					end: note.end.into(),
				})),
			);
		}

		let mut tracks = HashMap::new();
		for track in self.arrangement.tracks() {
			let node = &self.arrangement.node(track.id).0;
			tracks.insert(
				track.id,
				writer.push_track(
					track.clips.iter().map(|clip| match clip {
						Clip::Audio(audio) => proto::AudioClip {
							audio: audios[&audio.sample.path],
							position: proto::ClipPosition {
								start: audio.position.start().into(),
								end: audio.position.end().into(),
								offset: audio.position.offset().into(),
							},
						}
						.into(),
						Clip::Midi(midi) => proto::MidiClip {
							midi: midis[&Arc::as_ptr(&midi.pattern).addr()],
							position: proto::ClipPosition {
								start: midi.position.start().into(),
								end: midi.position.end().into(),
								offset: midi.position.offset().into(),
							},
						}
						.into(),
					}),
					self.arrangement
						.node(track.id)
						.0
						.plugins
						.iter()
						.map(|plugin| proto::Plugin {
							id: plugin.descriptor.id.to_bytes_with_nul().to_owned(),
							state: self.clap_host.get_state(plugin.id),
						}),
					node.volume,
					node.pan,
				),
			);
		}

		let mut channels = HashMap::new();
		for channel in once(&self.arrangement.master().0).chain(self.arrangement.channels()) {
			channels.insert(
				channel.id,
				writer.push_channel(
					self.arrangement
						.node(channel.id)
						.0
						.plugins
						.iter()
						.map(|plugin| proto::Plugin {
							id: plugin.descriptor.id.to_bytes_with_nul().to_owned(),
							state: self.clap_host.get_state(plugin.id),
						}),
					channel.volume,
					channel.pan,
				),
			);
		}

		for track in self.arrangement.tracks() {
			for connection in &self.arrangement.node(track.id).1 {
				writer.connect_track_to_channel(tracks[&track.id], channels[&connection]);
			}
		}

		for channel in self.arrangement.channels() {
			for connection in &self.arrangement.node(channel.id).1 {
				writer.connect_channel_to_channel(channels[&channel.id], channels[&connection]);
			}
		}

		File::create(path)
			.unwrap()
			.write_all(&writer.finalize())
			.unwrap();
	}

	pub fn load(
		&mut self,
		path: &Path,
		config: &Config,
		plugin_bundles: &BTreeMap<PluginDescriptor, PluginBundle>,
	) -> Option<Task<Message>> {
		info!("loading project {}", path.display());

		let mut gdp = Vec::new();
		File::open(path).ok()?.read_to_end(&mut gdp).ok()?;
		let reader = Reader::new(&gdp)?;

		let mut futs = Vec::new();

		let (mut arrangement, receiver) = ArrangementWrapper::create(config);
		futs.push(Task::stream(receiver).map(Message::Update));

		arrangement.set_bpm(reader.rtstate().bpm as u16);
		arrangement.set_numerator(reader.rtstate().numerator as u8);

		let mut audios = HashMap::new();
		let mut midis = HashMap::new();

		let (sender, receiver) = mpsc::channel();
		std::thread::scope(|s| {
			for (idx, audio) in reader.iter_audios() {
				let sender = sender.clone();
				let sample_rate = arrangement.rtstate().sample_rate;
				s.spawn(move || {
					let audio = config
						.sample_paths
						.iter()
						.flat_map(WalkDir::new)
						.flatten()
						.filter(|dir_entry| dir_entry.file_type().is_file())
						.filter(|dir_entry| {
							dir_entry
								.path()
								.file_name()
								.is_some_and(|name| *name == *audio.name)
						})
						.filter(|dir_entry| {
							audio.hash
								== hash_reader::<DefaultHasher>(
									File::open(dir_entry.path()).unwrap(),
								)
						})
						.find_map(|dir_entry| {
							Sample::create_with_hash(
								dir_entry.path().into(),
								sample_rate,
								audio.hash,
							)
						});

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
						velocity: f64::from(note.velocity),
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

		let mut load_channel = |node: &Node, channel: &proto::Channel| {
			futs.push(Task::done(Message::ChannelVolumeChanged(
				node.id,
				channel.volume,
			)));
			futs.push(Task::done(Message::ChannelPanChanged(node.id, channel.pan)));

			for plugin in &channel.plugins {
				futs.push(Task::done(Message::PluginLoad(
					plugin_bundles
						.keys()
						.find(|d| *d.id == *plugin.id())?
						.clone(),
				)));
			}

			Some(())
		};

		let mut tracks = HashMap::new();
		for (idx, clips, channel) in reader.iter_tracks() {
			let mut track = core::Track::default();

			for clip in clips {
				track.clips.push(match clip {
					proto::Clip::Audio(audio) => {
						let clip = AudioClip::create(
							audios.get(&audio.audio)?.clone(),
							arrangement.rtstate(),
						);
						clip.position.move_to(audio.position.start.into());
						clip.position.trim_end_to(audio.position.end.into());
						clip.position.trim_start_to(audio.position.offset.into());
						Clip::Audio(clip)
					}
					proto::Clip::Midi(midi) => {
						let clip = MidiClip::create(midis.get(&midi.midi)?.clone());
						clip.position.move_to(midi.position.start.into());
						clip.position.trim_end_to(midi.position.end.into());
						clip.position.trim_start_to(midi.position.offset.into());
						Clip::Midi(clip)
					}
				});
			}

			let id = track.id();
			tracks.insert(idx, id);
			arrangement.push_track(track);
			load_channel(&arrangement.node(id).0, channel)?;
		}

		let mut channels = HashMap::new();
		let mut iter_channels = reader.iter_channels();

		let node = &arrangement.master().0;
		let (idx, channel) = iter_channels.next()?;
		load_channel(node, channel)?;
		channels.insert(idx, node.id);

		for (idx, channel) in iter_channels {
			let mixer_node = Mixer::default();
			let id = mixer_node.id();
			channels.insert(idx, id);
			arrangement.push_channel(mixer_node);
			load_channel(&arrangement.node(id).0, channel)?;
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

		info!("loaded project {}", path.display());

		futs.extend(self.clear());

		self.plugin_descriptors = combo_box::State::new(plugin_bundles.keys().cloned().collect());
		self.arrangement = arrangement;
		self.audios.extend(audios.values().map(|audio| {
			(
				audio.path.clone(),
				LoadStatus::Loaded(Arc::downgrade(audio)),
			)
		}));
		self.midis.extend(midis.values().map(Arc::downgrade));

		Some(Task::batch(futs))
	}

	pub fn unload(
		&mut self,
		config: &Config,
		plugin_bundles: &BTreeMap<PluginDescriptor, PluginBundle>,
	) -> Task<Message> {
		let (arrangement, receiver) = ArrangementWrapper::create(config);

		let futs = Task::batch(
			self.clear()
				.chain(once(Task::stream(receiver).map(Message::Update))),
		);
		self.plugin_descriptors = combo_box::State::new(plugin_bundles.keys().cloned().collect());
		self.arrangement = arrangement;

		futs
	}

	fn clear(&mut self) -> impl Iterator<Item = Task<Message>> {
		self.loading.clear();
		self.audios.clear();
		self.midis.clear();
		self.recording = None;
		self.soloed_track = None;
		self.selected_channel = None;
		self.arrangement.plugins().map(|id| {
			self.clap_host
				.update(ClapHostMessage::MainThread(
					id,
					MainThreadMessage::GuiClosed,
				))
				.map(Message::ClapHost)
		})
	}

	pub fn loading(&self) -> bool {
		!self.loading.is_empty()
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
					.map(|track| {
						let node = &self.arrangement.node(track.id).0;

						container(
							row![
								PeakMeter::new(node.l_r.get(), node.enabled),
								column![
									Knob::new(
										0.0..=1.0,
										node.volume,
										0.0,
										1.0,
										node.enabled,
										Message::ChannelVolumeChanged.with(track.id)
									)
									.tooltip(Decibels::from_amplitude(node.volume).to_string()),
									Knob::new(
										-1.0..=1.0,
										node.pan,
										0.0,
										0.0,
										node.enabled,
										Message::ChannelPanChanged.with(track.id)
									)
									.tooltip({
										let pan = (node.pan * 100.0) as i8;
										match pan.cmp(&0) {
											Ordering::Greater => pan.to_string() + "% right",
											Ordering::Equal => "center".to_owned(),
											Ordering::Less => (-pan).to_string() + "% left",
										}
									}),
								]
								.spacing(5)
								.wrap(),
								column![
									icon_button(text('M'))
										.on_press(Message::TrackToggleEnabled(track.id))
										.style(move |t, s| {
											button_with_base(
												t,
												s,
												if node.enabled {
													button::primary
												} else {
													button::secondary
												},
											)
										}),
									icon_button(text('S'))
										.on_press(Message::TrackToggleSolo(track.id))
										.style(move |t, s| {
											button_with_base(
												t,
												s,
												if self.soloed_track == Some(track.id) {
													button::warning
												} else if node.enabled {
													button::primary
												} else {
													button::secondary
												},
											)
										}),
									icon_button(x())
										.on_press(Message::TrackRemove(track.id))
										.style(move |t, s| {
											button_with_base(
												t,
												s,
												if node.enabled {
													button::danger
												} else {
													button::secondary
												},
											)
										}),
									column![
										vertical_space(),
										button(
											AnimatedDot::new(
												self.recording
													.as_ref()
													.is_some_and(|&(_, i)| i == track.id)
											)
											.radius(5.0)
										)
										.padding(1.5)
										.on_press(Message::ToggleRecord(track.id))
										.style(move |t, s| {
											button_with_base(
												t,
												s,
												if self
													.recording
													.as_ref()
													.is_some_and(|&(_, i)| i == track.id)
												{
													button::danger
												} else if node.enabled {
													button::primary
												} else {
													button::secondary
												},
											)
										})
									]
								]
								.spacing(5)
							]
							.spacing(5),
						)
						.style(|t| {
							container::background(t.extended_palette().background.weak.color)
								.border(
									border::width(1)
										.color(t.extended_palette().background.strong.color),
								)
						})
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
							text(name),
							Knob::new(
								-1.0..=1.0,
								node.pan,
								0.0,
								0.0,
								node.enabled,
								Message::ChannelPanChanged.with(node.id)
							)
							.tooltip({
								let pan = (node.pan * 100.0) as i8;
								match pan.cmp(&0) {
									Ordering::Greater => pan.to_string() + "% right",
									Ordering::Equal => "center".to_owned(),
									Ordering::Less => (-pan).to_string() + "% left",
								}
							}),
							PeakMeter::new(node.l_r.get(), node.enabled)
						]
						.spacing(5)
						.align_x(Alignment::Center),
						column![
							buttons(node.enabled, node.id),
							vertical_slider(
								0.0..=1.0,
								node.volume,
								Message::ChannelVolumeChanged.with(node.id)
							)
							.step(f32::EPSILON)
							.style(move |t, s| slider_with_enabled(t, s, node.enabled))
						]
						.spacing(5)
						.align_x(Alignment::Center)
					]
					.spacing(5),
					connect(node.enabled, node.id)
				]
				.spacing(5)
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

		let mixer_panel = styled_scrollable_with_direction(
			row(once(channel(
				self.selected_channel,
				"M".to_owned(),
				&self.arrangement.master().0,
				|enabled, id| {
					column![
						icon_button(text('M'))
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
						let name = "T ".to_owned() + &(i + 1).to_string();
						let node = &self.arrangement.node(track.id).0;

						channel(
							self.selected_channel,
							name,
							node,
							|enabled, id| {
								column![
									icon_button(text('M'))
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
									icon_button(text('S'))
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
									icon_button(x()).on_press(Message::TrackRemove(id)).style(
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
						let name = "C ".to_owned() + &(i + 1).to_string();

						channel(
							self.selected_channel,
							name,
							node,
							|enabled, id| {
								column![
									icon_button(text('M'))
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
									space().height(13),
									icon_button(x()).on_press(Message::ChannelRemove(id)).style(
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
		.width(Fill);

		let plugin_picker = styled_combo_box(
			&self.plugin_descriptors,
			"Add Plugin",
			None,
			Message::PluginLoad,
		)
		.width(Fill);

		if let Some(selected) = self.selected_channel {
			let node = &self.arrangement.node(selected).0;
			Split::new(
				mixer_panel,
				column![
					plugin_picker,
					container(horizontal_rule(1)).padding([5, 0]),
					styled_scrollable_with_direction(
						dragking::column({
							node.plugins.iter().enumerate().map(|(i, plugin)| {
								row![
									Knob::new(
										0.0..=1.0,
										plugin.mix,
										0.0,
										1.0,
										plugin.enabled,
										Message::PluginMixChanged.with(i)
									)
									.radius(TEXT_HEIGHT)
									.tooltip(((plugin.mix * 100.0) as u8).to_string() + "%"),
									button(
										container(
											text(&*plugin.descriptor.name).wrapping(Wrapping::None)
										)
										.clip(true)
									)
									.style(move |t, s| button_with_base(
										t,
										s,
										if plugin.enabled {
											button::primary
										} else {
											button::secondary
										}
									))
									.width(Fill)
									.on_press(Message::ClapHost(
										ClapHostMessage::MainThread(
											plugin.id,
											MainThreadMessage::GuiRequestShow,
										)
									)),
									column![
										icon_button(text('M'))
											.on_press(Message::PluginToggleEnabled(i))
											.style(move |t, s| {
												button_with_base(
													t,
													s,
													if plugin.enabled {
														button::primary
													} else {
														button::secondary
													},
												)
											}),
										icon_button(x()).on_press(Message::PluginRemove(i)).style(
											move |t, s| {
												button_with_base(
													t,
													s,
													if plugin.enabled {
														button::danger
													} else {
														button::secondary
													},
												)
											}
										),
									]
									.spacing(5),
									mouse_area(
										container(
											grip_vertical()
												.line_height((LINE_HEIGHT + 10.0) / LINE_HEIGHT)
										)
										.style(|t| {
											container::background(
												t.extended_palette().background.weak.color,
											)
											.border(border::width(1).color(
												t.extended_palette().background.strong.color,
											))
										})
									)
									.interaction(Interaction::Grab),
								]
								.spacing(5)
								.into()
							})
						})
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
		self.clap_host.subscription().map(Message::ClapHost)
	}
}
