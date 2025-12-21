use crate::{
	arrangement_view::{
		audio_clip::AudioClip,
		midi_clip::MidiClip,
		midi_pattern::MidiPatternPair,
		node::{Node, NodeType},
		sample::SamplePair,
	},
	clap_host::{self, ClapHost},
	components::{icon_button, text_icon_button},
	config::Config,
	icons::{arrow_up_down, chevron_up, grip_vertical, mic, plus, power, power_off, x},
	state::{DEFAULT_SPLIT_POSITION, State},
	stylefns::{
		bordered_box_with_radius, button_with_radius, menu_style, scrollable_style,
		slider_secondary, slider_with_radius, split_style,
	},
	widget::{
		LINE_HEIGHT, TEXT_HEIGHT,
		clip::Clip,
		piano::Piano,
		piano_roll::{self, PianoRoll},
		playlist::{self, Playlist},
		seeker::Seeker,
		track::Track,
	},
};
use bit_set::BitSet;
use generic_daw_core::{
	Batch, MidiNote, MusicalTime, NodeId, NotePosition, PanMode, SampleId,
	clap_host::{HostInfo, MainThreadMessage, Plugin, PluginBundle, PluginDescriptor},
};
use generic_daw_widget::{
	knob::Knob,
	peak_meter::{MAX_VAL, PeakMeter},
};
use humantime::format_rfc3339_seconds;
use iced::{
	Center, Element, Fill, Function as _, Point, Shrink, Size, Task, Vector, border,
	futures::SinkExt as _,
	mouse::Interaction,
	padding, stream,
	widget::{
		button, column, combo_box, container, mouse_area, row, rule, scrollable, slider, space,
		text, vertical_slider,
	},
};
use iced_split::{Split, Strategy};
use log::warn;
use rtrb::Consumer;
use smol::{Timer, unblock};
use std::{
	cell::RefCell,
	cmp::{Ordering, Reverse},
	collections::HashMap,
	fmt::Write as _,
	io::Read,
	iter::once,
	num::NonZero,
	path::Path,
	sync::{Arc, LazyLock},
	time::{Duration, SystemTime},
};
use sweeten::widget::drag::DragEvent;
use utils::{NoClone, NoDebug, unique_id};

mod arrangement;
mod audio_clip;
mod clip;
mod midi_clip;
mod midi_pattern;
mod node;
mod plugin;
mod project;
mod recording;
mod sample;
mod track;

unique_id!(epoch);

pub use arrangement::Arrangement;
pub use audio_clip::AudioClipRef;
pub use epoch::Id as Epoch;
pub use midi_clip::MidiClipRef;
pub use project::Feedback;
pub use recording::Recording;

pub static DATA_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let data_dir = dirs::data_dir().unwrap().join("Generic DAW").into();
	_ = std::fs::create_dir(&data_dir);
	data_dir
});

pub static RECORDING_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let recording_dir = DATA_DIR.join("recordings").into();
	_ = std::fs::create_dir(&recording_dir);
	recording_dir
});

pub static PROJECT_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let project_dir = DATA_DIR.join("projects").into();
	_ = std::fs::create_dir(&project_dir);
	project_dir
});

pub static AUTOSAVE_DIR: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let autosave_dir = PROJECT_DIR.join("autosaved").into();
	_ = std::fs::create_dir(&autosave_dir);
	autosave_dir
});

#[derive(Clone, Debug)]
pub enum Message {
	ClapHost(clap_host::Message),
	Batch(Epoch, Box<Batch>),

	SetArrangement(NoClone<Box<Arrangement>>),

	ConnectRequest(NodeId, NodeId),
	ConnectSucceeded((NodeId, NodeId)),
	Disconnect(NodeId, NodeId),

	ChangedTab(Tab),

	ChannelAdd,
	ChannelRemove(NodeId),
	ChannelSelect(NodeId),
	ChannelVolumeChanged(NodeId, f32),
	ChannelPanChanged(NodeId, PanMode),
	ChannelToggleEnabled(NodeId),
	ChannelToggleBypassed(NodeId),

	PluginLoad(NodeId, PluginDescriptor, bool),
	PluginSetState(NodeId, usize, NoDebug<Box<[u8]>>),
	PluginMixChanged(NodeId, usize, f32),
	PluginToggleEnabled(NodeId, usize),
	PluginMoveTo(NodeId, DragEvent),
	PluginRemove(NodeId, usize),

	LoadHoveredFile,
	SampleLoaded(NoClone<Option<Box<SamplePair>>>, usize, MusicalTime),
	AddAudioClip(SampleId, usize, MusicalTime),

	TrackAdd,
	TrackRemove(NodeId),
	TrackToggleEnabled(NodeId),
	TrackToggleSolo(NodeId),

	TogglePlayback,
	Stop,
	SeekTo(MusicalTime),
	SetLoopMarker(Option<NotePosition>),

	Recording(NodeId),
	RecordingEndStream,
	RecordingFinalize,
	RecordingWrite(NoDebug<Box<[f32]>>),

	PlaylistAction(playlist::Action),
	PianoRollAction(piano_roll::Action),
	Pan(Vector, Size),
	Zoom(Vector, Point, Size),

	DeleteSelection,
	ClearSelection,

	OnDrag(f32),
	OnDragEnd,
	OnDoubleClick,
}

#[derive(Clone, Copy, Debug)]
pub enum Tab {
	Playlist,
	Mixer,
	PianoRoll(MidiClip),
}

#[derive(Debug)]
pub struct ArrangementView {
	pub arrangement: Arrangement,
	pub clap_host: ClapHost,

	recording: Option<(Recording, NodeId)>,
	tab: Tab,

	playlist_position: Vector,
	playlist_scale: Vector,
	playlist_selection: RefCell<playlist::Selection>,
	soloed_track: Option<NodeId>,

	selected_channel: Option<NodeId>,

	piano_roll_position: Vector,
	piano_roll_scale: Vector,
	piano_roll_selection: RefCell<piano_roll::Selection>,

	split_at: f32,
	plugins: combo_box::State<PluginDescriptor>,

	loading: usize,
}

impl ArrangementView {
	pub fn new(config: &Config, state: &State) -> (Self, Task<Message>) {
		let (arrangement, task) = Arrangement::create(config);

		let playlist_scale_x = (arrangement.transport().sample_rate.get() as f32).log2() - 5.0;
		let piano_roll_scale_x = playlist_scale_x - 2.0;

		(
			Self {
				arrangement,
				clap_host: ClapHost::default(),

				recording: None,
				tab: Tab::Playlist,

				playlist_position: Vector::default(),
				playlist_scale: Vector::new(playlist_scale_x, 87.0),
				playlist_selection: RefCell::default(),
				soloed_track: None,

				selected_channel: None,

				piano_roll_position: Vector::new(0.0, 1000.0),
				piano_roll_scale: Vector::new(piano_roll_scale_x, LINE_HEIGHT),
				piano_roll_selection: RefCell::default(),

				split_at: state.plugins_panel_split_at,
				plugins: combo_box::State::new(Vec::new()),

				loading: 0,
			},
			task.map(Box::new).map(Message::Batch.with(Epoch::unique())),
		)
	}

	pub fn update(
		&mut self,
		message: Message,
		config: &Config,
		state: &mut State,
		plugin_bundles: &HashMap<PluginDescriptor, NoDebug<PluginBundle>>,
	) -> Task<Message> {
		match message {
			Message::ClapHost(msg) => {
				return self.clap_host.update(msg, config).map(Message::ClapHost);
			}
			Message::Batch(epoch, msg) => {
				if epoch.is_latest() {
					return Task::batch(
						self.arrangement
							.update(*msg)
							.into_iter()
							.map(|msg| self.update(msg, config, state, plugin_bundles)),
					);
				}
			}
			Message::SetArrangement(NoClone(arrangement)) => {
				if let Some((recording, _)) = &mut self.recording {
					recording.end_stream();
				}

				let pos_fact = arrangement.transport().sample_rate.get() as f32
					/ self.arrangement.transport().sample_rate.get() as f32;
				let scale_diff = pos_fact.log2();

				self.arrangement = *arrangement;

				self.tab = Tab::Playlist;

				self.playlist_position.x *= pos_fact;
				self.playlist_scale.x += scale_diff;
				self.playlist_selection.get_mut().clear();
				self.soloed_track = None;

				self.selected_channel = None;

				self.piano_roll_position.x *= pos_fact;
				self.piano_roll_scale.x += scale_diff;
				self.piano_roll_selection.get_mut().clear();
			}
			Message::ConnectRequest(from, to) => {
				return self
					.arrangement
					.request_connect(from, to)
					.map(Message::ConnectSucceeded);
			}
			Message::ConnectSucceeded((from, to)) => self.arrangement.connect_succeeded(from, to),
			Message::Disconnect(from, to) => self.arrangement.disconnect(from, to),
			Message::ChangedTab(tab) => {
				match self.tab {
					Tab::Playlist => {
						let playlist_selection = self.playlist_selection.get_mut();
						playlist_selection.status = playlist::Status::None;
						playlist_selection
							.primary
							.extend(playlist_selection.secondary.drain());
					}
					Tab::Mixer => {}
					Tab::PianoRoll(..) => {
						let piano_roll_selection = self.piano_roll_selection.get_mut();
						piano_roll_selection.status = piano_roll::Status::None;
						piano_roll_selection.primary.clear();
						piano_roll_selection.secondary.clear();
					}
				}

				self.tab = tab;
			}
			Message::ChannelAdd => {
				let id = self.arrangement.add_channel();
				return self
					.arrangement
					.request_connect(id, self.arrangement.master().id)
					.map(Message::ConnectSucceeded);
			}
			Message::ChannelRemove(id) => {
				self.arrangement.remove_channel(id);
				if self.selected_channel == Some(id) {
					self.selected_channel = None;
				}
			}
			Message::ChannelSelect(id) => {
				self.selected_channel = if self.selected_channel == Some(id) {
					None
				} else {
					Some(id)
				};
			}
			Message::ChannelVolumeChanged(id, volume) => {
				self.arrangement.channel_volume_changed(id, volume);
			}
			Message::ChannelPanChanged(id, pan) => self.arrangement.channel_pan_changed(id, pan),
			Message::ChannelToggleEnabled(id) => self.arrangement.channel_toggle_enabled(id),
			Message::ChannelToggleBypassed(id) => self.arrangement.channel_toggle_bypassed(id),
			Message::PluginLoad(node, descriptor, show) => {
				static HOST: LazyLock<HostInfo> = LazyLock::new(|| {
					HostInfo::new_from_cstring(
						c"Generic DAW".to_owned(),
						c"Generic DAW".to_owned(),
						c"https://github.com/generic-daw/generic-daw".to_owned(),
						c"0.0.0".to_owned(),
					)
				});

				let (audio_processor, plugin, receiver) = Plugin::new(
					&plugin_bundles[&descriptor],
					descriptor,
					self.arrangement.transport().sample_rate,
					self.arrangement.transport().frames,
					&HOST,
				);

				let id = self.arrangement.plugin_load(node, audio_processor);
				let mut fut = self.clap_host.plugin_load(id, plugin, receiver);

				if show {
					fut = Task::batch([
						fut,
						self.clap_host.update(
							clap_host::Message::MainThread(id, MainThreadMessage::GuiRequestShow),
							config,
						),
					]);
				}

				return fut.map(Message::ClapHost);
			}
			Message::PluginSetState(node, i, state) => {
				let id = self.arrangement.node(node).plugins[i].id;
				return self
					.clap_host
					.update(clap_host::Message::SetState(id, state), config)
					.map(Message::ClapHost);
			}
			Message::PluginMixChanged(node, i, mix) => {
				self.arrangement.plugin_mix_changed(node, i, mix);
			}
			Message::PluginToggleEnabled(node, i) => {
				self.arrangement.plugin_toggle_enabled(node, i);
			}
			Message::PluginMoveTo(node, event) => {
				if let DragEvent::Dropped {
					index,
					target_index,
				} = event && index != target_index
				{
					self.arrangement.plugin_move_to(node, index, target_index);
				}
			}
			Message::PluginRemove(node, i) => _ = self.arrangement.plugin_remove(node, i),
			Message::LoadHoveredFile => {
				let playlist::Selection { file, .. } = self.playlist_selection.get_mut();
				if let (Some((path, Some((track, pos)))), Tab::Playlist) =
					(std::mem::take(file), self.tab)
				{
					let mut iter = self.arrangement.samples().values();
					return if let Some(sample) = iter.find(|sample| sample.path == path) {
						drop(iter);
						self.update(
							Message::AddAudioClip(sample.id, track, pos),
							config,
							state,
							plugin_bundles,
						)
					} else {
						self.loading += 1;
						let sample_rate = self.arrangement.transport().sample_rate;
						Task::future(unblock(move || {
							Message::SampleLoaded(
								SamplePair::new(path, sample_rate).map(Box::new).into(),
								track,
								pos,
							)
						}))
					};
				}
			}
			Message::SampleLoaded(NoClone(sample), track, pos) => {
				self.loading -= 1;

				if let Some(sample) = sample {
					let id = sample.gui.id;
					self.arrangement.add_sample(*sample);
					return self.update(
						Message::AddAudioClip(id, track, pos),
						config,
						state,
						plugin_bundles,
					);
				}
			}
			Message::AddAudioClip(sample, track, pos) => {
				let mut audio = AudioClip::new(
					sample,
					self.arrangement.samples()[*sample].samples.len(),
					self.arrangement.transport(),
				);
				audio.position.move_to(pos);
				let task = if track == self.arrangement.tracks().len() {
					self.update(Message::TrackAdd, config, state, plugin_bundles)
				} else {
					Task::none()
				};
				self.arrangement.add_clip(track, audio);
				return task;
			}
			Message::TrackAdd => {
				let track = self.arrangement.add_track();
				let id = self.arrangement.tracks()[track].id;
				if self.soloed_track.is_some() {
					self.arrangement.channel_toggle_enabled(id);
				}
				return self
					.arrangement
					.request_connect(id, self.arrangement.master().id)
					.map(Message::ConnectSucceeded);
			}
			Message::TrackRemove(id) => {
				let idx = self.arrangement.track_of(id).unwrap();
				self.arrangement.remove_track(id);

				if self.soloed_track == Some(id) {
					self.soloed_track = None;
				}
				if self.selected_channel == Some(id) {
					self.selected_channel = None;
				}

				let selection = self.playlist_selection.get_mut();
				selection.primary = selection
					.primary
					.drain()
					.filter_map(|(track, clip)| match track.cmp(&idx) {
						Ordering::Equal => None,
						Ordering::Less => Some((track, clip)),
						Ordering::Greater => Some((track - 1, clip)),
					})
					.collect();

				return self.update(Message::RecordingEndStream, config, state, plugin_bundles);
			}
			Message::TrackToggleEnabled(id) => {
				self.soloed_track = None;
				return self.update(
					Message::ChannelToggleEnabled(id),
					config,
					state,
					plugin_bundles,
				);
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
			Message::TogglePlayback => {
				self.arrangement.toggle_playback();
				return self.update(Message::RecordingEndStream, config, state, plugin_bundles);
			}
			Message::Stop => {
				self.arrangement.stop();
				return self.update(Message::RecordingEndStream, config, state, plugin_bundles);
			}
			Message::SeekTo(pos) => {
				self.arrangement.seek_to(pos);
				return self.update(Message::RecordingEndStream, config, state, plugin_bundles);
			}
			Message::SetLoopMarker(marker) => self.arrangement.set_loop_marker(marker),
			Message::Recording(node) => {
				let path = RECORDING_DIR
					.join(format!(
						"recording-{}.wav",
						format_rfc3339_seconds(SystemTime::now())
					))
					.into();

				if let Some((recording, r_node)) = &mut self.recording {
					if node == *r_node {
						return self.update(
							Message::RecordingEndStream,
							config,
							state,
							plugin_bundles,
						);
					}

					let pos = recording.position;

					let sample = recording.split_off(path, self.arrangement.transport());
					let id = sample.gui.id;
					self.arrangement.add_sample(sample);

					let track = self.arrangement.track_of(*r_node).unwrap();

					let mut clip = AudioClip::new(
						id,
						self.arrangement.samples()[*id].samples.len(),
						self.arrangement.transport(),
					);
					clip.position.move_to(pos);
					self.arrangement.add_clip(track, clip);

					self.recording.as_mut().unwrap().1 = node;
				} else {
					let (recording, task) = Recording::create(
						path,
						self.arrangement.transport(),
						config.input_device.name.clone(),
						config.input_device.sample_rate,
						config.input_device.buffer_size,
					);

					let sample_rate = recording.sample_rate();
					let frames = recording.frames().or(config.input_device.buffer_size);

					self.recording = Some((recording, node));
					self.arrangement.play();

					return poll_consumer(task, sample_rate, frames)
						.map(NoDebug)
						.map(Message::RecordingWrite)
						.chain(Task::done(Message::RecordingFinalize));
				}
			}
			Message::RecordingEndStream => {
				if let Some((recording, _)) = &mut self.recording {
					recording.end_stream();
				}
			}
			Message::RecordingFinalize => {
				let (recording, node) = self.recording.take().unwrap();
				let pos = recording.position;

				let sample = recording.finalize();

				if let Some(track) = self.arrangement.track_of(node) {
					let id = sample.gui.id;
					self.arrangement.add_sample(sample);

					let mut clip = AudioClip::new(
						id,
						self.arrangement.samples()[*id].samples.len(),
						self.arrangement.transport(),
					);
					clip.position.move_to(pos);
					self.arrangement.add_clip(track, clip);
				}
			}
			Message::RecordingWrite(samples) => self.recording.as_mut().unwrap().0.write(&samples),
			Message::PlaylistAction(action) => return self.handle_playlist_action(action),
			Message::PianoRollAction(action) => self.handle_piano_roll_action(action),
			Message::Pan(pos_diff, size) => match self.tab {
				Tab::Playlist => {
					self.playlist_position += pos_diff;
					self.playlist_position.x = self.playlist_position.x.max(0.0);
					self.playlist_position.y = self.playlist_position.y.max(0.0);
				}
				Tab::Mixer => {}
				Tab::PianoRoll(..) => {
					self.piano_roll_position += pos_diff;
					self.piano_roll_position.x = self.piano_roll_position.x.max(0.0);
					self.piano_roll_position.y = self
						.piano_roll_position
						.y
						.clamp(0.0, self.piano_roll_scale.y.mul_add(128.0, -size.height));
				}
			},
			Message::Zoom(scale_diff, cursor, size) => {
				let (old_scale, pos, new_scale) = match self.tab {
					Tab::Playlist => {
						let old_scale = self.playlist_scale;
						self.playlist_scale += scale_diff;
						self.playlist_scale.x = self.playlist_scale.x.clamp(1.0, 16f32.next_down());
						self.playlist_scale.y = self.playlist_scale.y.clamp(46.0, 200.0);
						(old_scale, self.playlist_position, self.playlist_scale)
					}
					Tab::Mixer => return Task::none(),
					Tab::PianoRoll(..) => {
						let old_scale = self.piano_roll_scale;
						self.piano_roll_scale += scale_diff;
						self.piano_roll_scale.x =
							self.piano_roll_scale.x.clamp(1.0, 16f32.next_down());
						self.piano_roll_scale.y = self
							.piano_roll_scale
							.y
							.clamp(LINE_HEIGHT, 2.0 * LINE_HEIGHT);
						(old_scale, self.piano_roll_position, self.piano_roll_scale)
					}
				};

				let pos_diff = Vector::new(
					(cursor.x + pos.x) * ((old_scale.x - new_scale.x).exp2() - 1.0),
					(cursor.y + pos.y) * ((new_scale.y / old_scale.y) - 1.0),
				);

				return self.update(Message::Pan(pos_diff, size), config, state, plugin_bundles);
			}
			Message::DeleteSelection => match self.tab {
				Tab::Playlist => return self.handle_playlist_action(playlist::Action::Delete),
				Tab::Mixer => {}
				Tab::PianoRoll(..) => self.handle_piano_roll_action(piano_roll::Action::Delete),
			},
			Message::ClearSelection => match self.tab {
				Tab::Playlist => self.playlist_selection.get_mut().clear(),
				Tab::Mixer => {}
				Tab::PianoRoll(..) => self.piano_roll_selection.get_mut().clear(),
			},
			Message::OnDrag(split_at) => self.split_at = split_at.clamp(200.0, 400.0),
			Message::OnDragEnd => {
				if state.plugins_panel_split_at != self.split_at {
					state.plugins_panel_split_at = self.split_at;
					state.write();
				}
			}
			Message::OnDoubleClick => {
				return Task::batch([
					self.update(
						Message::OnDrag(DEFAULT_SPLIT_POSITION),
						config,
						state,
						plugin_bundles,
					),
					self.update(Message::OnDragEnd, config, state, plugin_bundles),
				]);
			}
		}

		Task::none()
	}

	fn handle_playlist_action(&mut self, action: playlist::Action) -> Task<Message> {
		let playlist::Selection {
			primary, secondary, ..
		} = self.playlist_selection.get_mut();

		match action {
			playlist::Action::Open => {
				debug_assert_eq!(primary.len(), 1);
				let &(track, clip) = primary.iter().next().unwrap();

				let clip::Clip::Midi(clip) = self.arrangement.tracks()[track].clips[clip] else {
					warn!("tried to open non-midi clip at {track}:{clip}");
					return Task::none();
				};

				self.piano_roll_selection.get_mut().primary.clear();
				return Task::done(Message::ChangedTab(Tab::PianoRoll(clip)));
			}
			playlist::Action::Add(track, pos) => {
				let pattern = MidiPatternPair::new(Vec::new());
				let id = pattern.gui.id;
				self.arrangement.add_midi_pattern(pattern);

				let mut clip = MidiClip::new(id);
				clip.position.trim_end_to(MusicalTime::new(
					4 * u64::from(self.arrangement.transport().numerator.get()),
					0,
				));
				clip.position.move_to(pos);
				let clip = self.arrangement.add_clip(track, clip);
				primary.insert((track, clip));
			}
			playlist::Action::Clone => {
				let mut sorted = primary.drain().collect::<Vec<_>>();
				sorted.sort_unstable_by_key(|&(_, c)| c);
				for (track, clip) in sorted {
					primary.insert((
						track,
						self.arrangement
							.add_clip(track, self.arrangement.tracks()[track].clips[clip]),
					));
				}
			}
			playlist::Action::Drag(track_diff, pos_diff) => {
				let mut sorted = primary.drain().collect::<Vec<_>>();
				match track_diff.cmp(&0) {
					Ordering::Equal => {}
					Ordering::Less => sorted.sort_unstable_by_key(|&(t, c)| (t, Reverse(c))),
					Ordering::Greater => {
						sorted.sort_unstable_by_key(|&(t, c)| (Reverse(t), Reverse(c)));
					}
				}

				for (mut track, mut clip) in sorted {
					let new_track = track
						.saturating_add_signed(track_diff)
						.min(self.arrangement.tracks().len() - 1);
					if track != new_track {
						clip = self.arrangement.clip_switch_track(track, clip, new_track);
						track = new_track;
					}

					let pos = self.arrangement.tracks()[track].clips[clip]
						.position()
						.start();
					self.arrangement.clip_move_to(track, clip, pos + pos_diff);

					primary.insert((track, clip));
				}
			}
			playlist::Action::SplitAt(mut pos) => {
				let mut extra = HashMap::<_, usize>::new();

				let mut sorted = primary
					.drain()
					.filter(|&(track, clip)| {
						let position = self.arrangement.tracks()[track].clips[clip].position();
						(position.start()..=position.end()).contains(&pos)
					})
					.collect::<Vec<_>>();
				sorted.sort_unstable_by_key(|&(_, c)| c);

				for (track, mut lhs) in sorted {
					let extra = extra.entry(track).or_default();
					lhs += *extra;

					let clip = self.arrangement.tracks()[track].clips[lhs];
					if clip.position().start() == pos {
						primary.insert((track, lhs));
					} else if clip.position().end() == pos {
						secondary.insert((track, lhs));
					} else if clip.position().len() > MusicalTime::TICK {
						let start = clip.position().start() + MusicalTime::TICK;
						let end = clip.position().end() - MusicalTime::TICK;
						pos = pos.clamp(start, end);
						let rhs = self.arrangement.insert_clip(track, clip, lhs + 1);
						self.arrangement.clip_trim_end_to(track, lhs, pos);
						self.arrangement.clip_trim_start_to(track, rhs, pos);
						primary.insert((track, lhs));
						secondary.insert((track, rhs));
						*extra += 1;
					}
				}
			}
			playlist::Action::DragSplit(pos) => {
				let mut clamped = HashMap::new();

				for &(track, lhs) in &*primary {
					let new = self.arrangement.tracks()[track].clips[lhs]
						.position()
						.start() + MusicalTime::TICK;

					clamped
						.entry(track)
						.and_modify(|old| *old = new.max(*old))
						.or_insert_with(|| new.max(pos));
				}

				for &(track, rhs) in &*secondary {
					let new = self.arrangement.tracks()[track].clips[rhs].position().end()
						- MusicalTime::TICK;

					clamped
						.entry(track)
						.and_modify(|old| *old = new.min(*old))
						.or_insert_with(|| new.min(pos));
				}

				for &(track, lhs) in &*primary {
					self.arrangement
						.clip_trim_end_to(track, lhs, clamped[&track]);
				}

				for &(track, rhs) in &*secondary {
					self.arrangement
						.clip_trim_start_to(track, rhs, clamped[&track]);
				}
			}
			playlist::Action::TrimStart(pos_diff) => {
				for &(track, clip) in &*primary {
					let pos = self.arrangement.tracks()[track].clips[clip]
						.position()
						.start();
					self.arrangement
						.clip_trim_start_to(track, clip, pos + pos_diff);
				}
			}
			playlist::Action::TrimEnd(pos_diff) => {
				for &(track, clip) in &*primary {
					let pos = self.arrangement.tracks()[track].clips[clip]
						.position()
						.end();
					self.arrangement
						.clip_trim_end_to(track, clip, pos + pos_diff);
				}
			}
			playlist::Action::Delete => {
				let mut sorted = primary.drain().collect::<Vec<_>>();
				sorted.sort_unstable_by_key(|&(_, c)| Reverse(c));
				for (track, clip) in sorted {
					let clip = self.arrangement.remove_clip(track, clip);
					self.arrangement.gc(clip);
				}
			}
		}

		Task::none()
	}

	fn handle_piano_roll_action(&mut self, action: piano_roll::Action) {
		let Tab::PianoRoll(clip) = &mut self.tab else {
			warn!("tried to handle {:?} while in {:?}", action, self.tab);
			return;
		};

		let piano_roll::Selection {
			primary, secondary, ..
		} = self.piano_roll_selection.get_mut();

		match action {
			piano_roll::Action::Add(key, pos) => {
				let note = self.arrangement.add_note(
					clip.pattern,
					MidiNote {
						key,
						velocity: 1.0,
						position: NotePosition::new(pos, pos + MusicalTime::BEAT),
					},
				);
				primary.insert(note);
			}
			piano_roll::Action::Clone => {
				let sorted = primary.clone();
				primary.clear();
				for note in &sorted {
					primary.insert(self.arrangement.add_note(
						clip.pattern,
						self.arrangement.midi_patterns()[*clip.pattern].notes[note],
					));
				}
			}
			piano_roll::Action::Drag(key_diff, pos_diff) => {
				for idx in &*primary {
					let note = self.arrangement.midi_patterns()[*clip.pattern].notes[idx];
					let new_key = note.key + key_diff;
					if new_key != note.key {
						self.arrangement.note_switch_key(clip.pattern, idx, new_key);
					}
					let pos = note.position.start();
					self.arrangement
						.note_move_to(clip.pattern, idx, pos + pos_diff);
				}
			}
			piano_roll::Action::SplitAt(mut pos) => {
				let mut extra = 0;

				let sorted = primary
					.iter()
					.filter(|&lhs| {
						let note = self.arrangement.midi_patterns()[*clip.pattern].notes[lhs];
						(note.position.start()..=note.position.end()).contains(&pos)
					})
					.collect::<BitSet>();
				primary.clear();

				for mut lhs in &sorted {
					lhs += extra;
					let note = self.arrangement.midi_patterns()[*clip.pattern].notes[lhs];
					if note.position.start() == pos {
						primary.insert(lhs);
					} else if note.position.end() == pos {
						secondary.insert(lhs);
					} else if note.position.len() > MusicalTime::TICK {
						let start = note.position.start() + MusicalTime::TICK;
						let end = note.position.end() - MusicalTime::TICK;
						pos = pos.clamp(start, end);
						let rhs = self.arrangement.insert_note(clip.pattern, note, lhs + 1);
						self.arrangement.note_trim_end_to(clip.pattern, lhs, pos);
						self.arrangement.note_trim_start_to(clip.pattern, rhs, pos);
						primary.insert(lhs);
						secondary.insert(rhs);
						extra += 1;
					}
				}
			}
			piano_roll::Action::DragSplit(pos) => {
				let mut clamped = HashMap::new();

				for lhs in &*primary {
					let note = self.arrangement.midi_patterns()[*clip.pattern].notes[lhs];
					let new = note.position.start() + MusicalTime::TICK;

					clamped
						.entry(note.key)
						.and_modify(|old| *old = new.max(*old))
						.or_insert_with(|| new.max(pos));
				}

				for rhs in &*secondary {
					let note = self.arrangement.midi_patterns()[*clip.pattern].notes[rhs];
					let new = note.position.end() - MusicalTime::TICK;

					clamped
						.entry(note.key)
						.and_modify(|old| *old = new.min(*old))
						.or_insert_with(|| new.min(pos));
				}

				for lhs in &*primary {
					let note = self.arrangement.midi_patterns()[*clip.pattern].notes[lhs];
					self.arrangement
						.note_trim_end_to(clip.pattern, lhs, clamped[&note.key]);
				}

				for rhs in &*secondary {
					let note = self.arrangement.midi_patterns()[*clip.pattern].notes[rhs];
					self.arrangement
						.note_trim_start_to(clip.pattern, rhs, clamped[&note.key]);
				}
			}
			piano_roll::Action::TrimStart(pos_diff) => {
				for note in &*primary {
					let pos = self.arrangement.midi_patterns()[*clip.pattern].notes[note]
						.position
						.start();
					self.arrangement
						.note_trim_start_to(clip.pattern, note, pos + pos_diff);
				}
			}
			piano_roll::Action::TrimEnd(pos_diff) => {
				for note in &*primary {
					let pos = self.arrangement.midi_patterns()[*clip.pattern].notes[note]
						.position
						.end();
					self.arrangement
						.note_trim_end_to(clip.pattern, note, pos + pos_diff);
				}
			}
			piano_roll::Action::Delete => {
				let mut sorted = primary.iter().collect::<Vec<_>>();
				primary.clear();
				sorted.sort_unstable_by_key(|&n| Reverse(n));
				for note in sorted {
					self.arrangement.remove_note(clip.pattern, note);
				}
			}
		}
	}

	pub fn view(&self) -> Element<'_, Message> {
		match self.tab {
			Tab::Playlist => self.arrangement(),
			Tab::Mixer => self.mixer(),
			Tab::PianoRoll(clip) => self.piano_roll(clip),
		}
	}

	fn arrangement(&self) -> Element<'_, Message> {
		Seeker::new(
			self.arrangement.transport(),
			&self.playlist_position,
			&self.playlist_scale,
			column(
				self.arrangement
					.tracks()
					.iter()
					.map(|track| track.id)
					.map(|id| {
						let node = self.arrangement.node(id);

						let button_style = |cond: bool| {
							if !node.enabled {
								button::secondary
							} else if cond {
								button::warning
							} else {
								button::primary
							}
						};

						container(
							row![
								row![
									PeakMeter::new(&node.peaks[0]),
									PeakMeter::new(&node.peaks[1])
								]
								.spacing(2),
								column![
									Knob::new(0.0..=MAX_VAL, node.volume.abs().cbrt(), move |v| {
										Message::ChannelVolumeChanged(
											id,
											v.powi(3).copysign(node.volume),
										)
									})
									.default(1.0)
									.enabled(node.enabled)
									.tooltip(format_decibels(node.volume.abs())),
									node.pan_knob(20.0),
								]
								.align_x(Center)
								.spacing(5)
								.wrap(),
								column![
									icon_button(
										x(),
										if node.enabled {
											button::danger
										} else {
											button::secondary
										}
									)
									.on_press(Message::TrackRemove(id)),
									text_icon_button("M", button_style(false))
										.on_press(Message::TrackToggleEnabled(id)),
									text_icon_button(
										"S",
										button_style(self.soloed_track == Some(id))
									)
									.on_press(Message::TrackToggleSolo(id)),
									icon_button(
										mic(),
										button_style(
											self.recording.as_ref().is_some_and(|&(_, i)| i == id)
										)
									)
									.on_press(Message::Recording(id))
								]
								.spacing(5)
								.wrap()
							]
							.spacing(5),
						)
						.style(bordered_box_with_radius(0))
						.padding(5)
						.height(self.playlist_scale.y)
					})
					.map(Element::new)
					.chain(once(
						container(
							button(plus().size(LINE_HEIGHT + 6.0))
								.padding(5)
								.style(button_with_radius(button::primary, f32::INFINITY))
								.on_press(Message::TrackAdd),
						)
						.padding(padding::right(5).top(5))
						.into(),
					)),
			)
			.align_x(Center),
			Playlist::new(
				&self.playlist_selection,
				self.arrangement.transport(),
				&self.playlist_position,
				&self.playlist_scale,
				self.arrangement
					.tracks()
					.iter()
					.enumerate()
					.map(|(track_idx, track)| {
						let node = self.arrangement.node(track.id);

						Track::new(
							&self.playlist_scale,
							track
								.clips
								.iter()
								.enumerate()
								.map(|(clip_idx, clip)| match clip {
									clip::Clip::Audio(clip) => Clip::new(
										AudioClipRef {
											sample: &self.arrangement.samples()[*clip.sample],
											clip,
											idx: (track_idx, clip_idx),
										},
										&self.playlist_selection,
										self.arrangement.transport(),
										&self.playlist_position,
										&self.playlist_scale,
										node.enabled,
										Message::PlaylistAction,
									),
									clip::Clip::Midi(clip) => Clip::new(
										MidiClipRef {
											pattern: &self.arrangement.midi_patterns()
												[*clip.pattern],
											clip,
											idx: (track_idx, clip_idx),
										},
										&self.playlist_selection,
										self.arrangement.transport(),
										&self.playlist_position,
										&self.playlist_scale,
										node.enabled,
										Message::PlaylistAction,
									),
								})
								.chain(
									self.recording
										.as_ref()
										.filter(|&&(_, i)| i == track.id)
										.map(|(recording, _)| {
											Clip::new(
												recording,
												&self.playlist_selection,
												self.arrangement.transport(),
												&self.playlist_position,
												&self.playlist_scale,
												node.enabled,
												Message::PlaylistAction,
											)
										}),
								),
						)
					}),
				Message::PlaylistAction,
			),
			Message::SeekTo,
			Message::SetLoopMarker,
			Message::Pan,
			Message::Zoom,
		)
		.into()
	}

	fn mixer(&self) -> Element<'_, Message> {
		Split::new(
			scrollable(
				row(once(self.channel(self.arrangement.master(), "M"))
					.chain(once(rule::vertical(1).into()))
					.chain({
						let mut iter = self
							.arrangement
							.tracks()
							.iter()
							.map(|track| self.arrangement.node(track.id))
							.enumerate()
							.map(|(i, node)| self.channel(node, format!("T{}", i + 1)))
							.peekable();

						let one = iter.peek().map(|_| rule::vertical(1).into());
						iter.chain(one)
					})
					.chain(
						self.arrangement
							.channels()
							.enumerate()
							.map(|(i, node)| self.channel(node, format!("C{}", i + 1))),
					)
					.chain(once(
						button(plus().size(LINE_HEIGHT + 6.0))
							.padding(5)
							.style(button_with_radius(button::primary, f32::INFINITY))
							.on_press(Message::ChannelAdd)
							.into(),
					)))
				.align_y(Center)
				.spacing(5),
			)
			.direction(scrollable::Direction::Horizontal(
				scrollable::Scrollbar::default(),
			))
			.spacing(5)
			.style(scrollable_style)
			.width(Fill),
			self.selected_channel.map(|selected| {
				let node = self.arrangement.node(selected);
				column![
					combo_box(&self.plugins, "Add Plugin", None, move |descriptor| {
						Message::PluginLoad(selected, descriptor, true)
					})
					.menu_style(menu_style)
					.width(Fill),
					container(rule::horizontal(1)).padding(padding::vertical(5)),
					scrollable(
						sweeten::column(node.plugins.iter().enumerate().map(|(i, plugin)| {
							let button_style = |cond: bool| {
								if !plugin.enabled || !node.enabled {
									button::secondary
								} else if cond {
									button::warning
								} else {
									button::primary
								}
							};

							row![
								Knob::new(0.0..=1.0, plugin.mix, move |mix| {
									Message::PluginMixChanged(selected, i, mix)
								})
								.radius(TEXT_HEIGHT)
								.enabled(plugin.enabled && node.enabled)
								.tooltip(format!("{:.0}%", plugin.mix * 100.0)),
								button(
									container(
										text(&*plugin.descriptor.name)
											.wrapping(text::Wrapping::None)
									)
									.padding(7)
									.clip(true)
								)
								.padding(0)
								.style(button_with_radius(button_style(false), border::left(5)))
								.width(Fill)
								.on_press(Message::ClapHost(
									clap_host::Message::MainThread(
										plugin.id,
										MainThreadMessage::GuiRequestShow,
									)
								)),
								column![
									icon_button(
										if plugin.enabled && !node.bypassed {
											power
										} else {
											power_off
										}(),
										button_style(node.bypassed)
									)
									.on_press(Message::PluginToggleEnabled(selected, i)),
									icon_button(
										x(),
										if plugin.enabled && node.enabled {
											button::danger
										} else {
											button::secondary
										}
									)
									.on_press(Message::PluginRemove(selected, i)),
								]
								.spacing(5),
								mouse_area(
									container(grip_vertical())
										.center_y(LINE_HEIGHT + 14.0)
										.style(bordered_box_with_radius(border::right(5)))
								)
								.interaction(Interaction::Grab),
							]
							.align_y(Center)
							.spacing(5)
							.into()
						}))
						.spacing(5)
						.on_drag(Message::PluginMoveTo.with(selected)),
					)
					.spacing(5)
					.style(scrollable_style)
					.height(Fill)
				]
			}),
			self.selected_channel.map_or(0.0, |_| self.split_at),
		)
		.on_drag_maybe(self.selected_channel.map(|_| Message::OnDrag))
		.on_drag_end_maybe(self.selected_channel.map(|_| Message::OnDragEnd))
		.on_double_click_maybe(self.selected_channel.map(|_| Message::OnDoubleClick))
		.strategy(Strategy::End)
		.focus_delay(Duration::ZERO)
		.style(split_style)
		.into()
	}

	fn channel<'a>(
		&'a self,
		node: &'a Node,
		name: impl text::IntoFragment<'a>,
	) -> Element<'a, Message> {
		let button_style = |cond: bool| {
			if !node.enabled {
				button::secondary
			} else if cond {
				button::warning
			} else {
				button::primary
			}
		};

		mouse_area(
			container(
				column![
					text(name).size(14).line_height(1.0),
					node.pan_knob(23.0),
					row![
						text_icon_button("M", button_style(false))
							.on_press(Message::ChannelToggleEnabled(node.id)),
						text_icon_button("S", button_style(self.soloed_track == Some(node.id)))
							.on_press_maybe(
								(node.ty == NodeType::Track)
									.then_some(Message::TrackToggleSolo(node.id)),
							),
						icon_button(
							x(),
							if node.enabled {
								button::danger
							} else {
								button::secondary
							},
						)
						.on_press_maybe(match node.ty {
							NodeType::Master => None,
							NodeType::Channel => Some(Message::ChannelRemove(node.id)),
							NodeType::Track => Some(Message::TrackRemove(node.id)),
						}),
					]
					.spacing(5),
					row![
						icon_button(
							if node.bypassed { power_off } else { power }(),
							button_style(node.bypassed)
						)
						.on_press(Message::ChannelToggleBypassed(node.id)),
						icon_button(
							arrow_up_down(),
							button_style(node.volume.is_sign_negative())
						)
						.on_press(Message::ChannelVolumeChanged(node.id, -node.volume)),
						node.pan_switcher()
					]
					.spacing(5),
					container(text(format_decibels(node.volume.abs())).line_height(1.0))
						.style(bordered_box_with_radius(0))
						.center_x(55)
						.padding(2),
					row![
						container(PeakMeter::new(&node.peaks[0]).width(16.0))
							.padding(padding::vertical(10)),
						vertical_slider(0.0..=MAX_VAL, node.volume.abs().cbrt(), |v| {
							Message::ChannelVolumeChanged(node.id, v.powi(3).copysign(node.volume))
						})
						.default(1.0)
						.width(17)
						.step(f32::EPSILON)
						.handle((15, 20))
						.style(slider_with_radius(
							if node.enabled {
								slider::default
							} else {
								slider_secondary
							},
							5
						)),
						container(PeakMeter::new(&node.peaks[1]).width(16.0))
							.padding(padding::vertical(10)),
					]
					.spacing(3),
					self.selected_channel
						.filter(|_| node.ty != NodeType::Track)
						.filter(|&selected_channel| node.id != selected_channel)
						.filter(|&selected_channel| self.arrangement.master().id != selected_channel)
						.map_or_else(
							|| Element::new(space().height(LINE_HEIGHT)),
							|selected_channel| {
								let connected = self
									.arrangement
									.outgoing(selected_channel)
									.contains(*node.id);

								button(chevron_up())
									.style(if node.enabled && connected {
										button::primary
									} else {
										button::secondary
									})
									.padding(0)
									.on_press(if connected {
										Message::Disconnect(selected_channel, node.id)
									} else {
										Message::ConnectRequest(selected_channel, node.id)
									})
									.into()
							}
						)
				]
				.width(Shrink)
				.spacing(5)
				.align_x(Center),
			)
			.padding(5)
			.style(|t| {
				if self.selected_channel == Some(node.id) {
					bordered_box_with_radius(0)(t)
						.background(t.extended_palette().background.weaker.color)
				} else {
					bordered_box_with_radius(0)(t)
						.background(t.extended_palette().background.weakest.color)
				}
			}),
		)
		.interaction(Interaction::Pointer)
		.on_press(Message::ChannelSelect(node.id))
		.into()
	}

	fn piano_roll(&self, clip: MidiClip) -> Element<'_, Message> {
		Seeker::new(
			self.arrangement.transport(),
			&self.piano_roll_position,
			&self.piano_roll_scale,
			Piano::new(&self.piano_roll_position, &self.piano_roll_scale),
			PianoRoll::new(
				&self.piano_roll_selection,
				&self.arrangement.midi_patterns()[*clip.pattern].notes,
				self.arrangement.transport(),
				&self.piano_roll_position,
				&self.piano_roll_scale,
				Message::PianoRollAction,
			),
			Message::SeekTo,
			Message::SetLoopMarker,
			Message::Pan,
			Message::Zoom,
		)
		.with_offset(
			clip.position
				.start()
				.to_samples_f(self.arrangement.transport())
				- clip
					.position
					.offset()
					.to_samples_f(self.arrangement.transport()),
		)
		.into()
	}

	pub fn set_plugins(
		&mut self,
		plugin_bundles: &HashMap<PluginDescriptor, NoDebug<PluginBundle>>,
	) {
		let mut plugins = plugin_bundles.keys().cloned().collect::<Vec<_>>();
		plugins.sort_unstable();
		self.plugins = combo_box::State::new(plugins);
	}

	pub fn hover_file(&mut self, file: Arc<Path>) {
		self.playlist_selection.get_mut().file = Some((file, None));
	}

	pub fn hovering_file(&self) -> bool {
		self.playlist_selection.borrow().file.is_some()
	}

	pub fn tab(&self) -> &Tab {
		&self.tab
	}

	pub fn loading(&self) -> bool {
		self.loading > 0
	}
}

fn format_decibels(amp: f32) -> String {
	let mut f = String::with_capacity(4);

	let db = 20.0 * amp.log10();
	let dba = db.abs();

	if dba >= 0.05 {
		if db.is_sign_positive() {
			write!(f, "+").unwrap();
		} else {
			write!(f, "-").unwrap();
		}
	}

	write!(f, "{dba:.*}", (dba < 9.95).into()).unwrap();

	f
}

fn crc(mut r: impl Read) -> u32 {
	#[repr(align(8))]
	struct Aligned([u8; 4096]);
	let Aligned(buf) = &mut Aligned([0; 4096]);

	let mut crc = 0;
	let mut len;

	while {
		len = r.read(buf).unwrap();
		len != 0
	} {
		crc = crc32c::crc32c_append(crc, &buf[..len]);
	}

	crc
}

fn poll_consumer<T: Send + 'static>(
	mut consumer: Consumer<T>,
	sample_rate: NonZero<u32>,
	frames: Option<NonZero<u32>>,
) -> Task<T> {
	let min = 64.0 / sample_rate.get() as f32;
	let max = frames.or(NonZero::new(8192)).unwrap().get() as f32 / sample_rate.get() as f32;
	let mut backoff = 0.0;
	let mut backoff = move |counter: u16| {
		let divisor = f32::from(counter).max(0.5);
		backoff = ((backoff + backoff / divisor) * 0.5).clamp(min, max);
		Timer::after(Duration::from_secs_f32(backoff))
	};

	Task::stream(stream::channel(
		consumer.buffer().capacity(),
		async move |mut sender| {
			loop {
				let mut counter = 0;
				while let Ok(t) = consumer.pop() {
					counter += 1;
					if sender.send(t).await.is_err() {
						return;
					}
				}
				if consumer.is_abandoned() {
					return;
				}
				backoff(counter).await;
			}
		},
	))
}
