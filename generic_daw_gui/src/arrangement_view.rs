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
	daw::DEFAULT_SPLIT_POSITION,
	icons::{arrow_up_down, chevron_up, grip_vertical, plus, power, power_off, x},
	state::State,
	stylefns::{
		bordered_box_with_radius, button_with_radius, menu_style, scrollable_style,
		slider_secondary, split_style,
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
use dragking::DragEvent;
use fragile::Fragile;
use generic_daw_core::{
	Batch, MidiNote, MusicalTime, NodeId, NotePosition, PanMode, SampleId,
	clap_host::{HostInfo, MainThreadMessage, Plugin, PluginBundle, PluginDescriptor},
};
use generic_daw_utils::{NoClone, NoDebug};
use generic_daw_widget::{dot::Dot, knob::Knob, peak_meter::PeakMeter};
use humantime::format_rfc3339;
use iced::{
	Center, Element, Fill, Function as _, Point, Shrink, Size, Subscription, Task, Vector, border,
	futures::SinkExt as _,
	mouse::Interaction,
	padding, stream,
	widget::{
		button, column, combo_box, container, mouse_area, row, rule, scrollable, slider, space,
		text, vertical_slider,
	},
};
use iced_split::{Split, Strategy};
use rtrb::Consumer;
use smol::{Timer, unblock};
use std::{
	cell::RefCell,
	cmp::{Ordering, Reverse},
	collections::{HashMap, HashSet},
	f32::consts::SQRT_2,
	fmt::Write as _,
	io::Read,
	iter::once,
	num::NonZero,
	path::Path,
	sync::{Arc, LazyLock},
	time::{Duration, SystemTime},
};

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

pub use arrangement::Arrangement;
pub use audio_clip::AudioClipRef;
pub use midi_clip::MidiClipRef;
pub use project::Feedback;
pub use recording::Recording;

pub static DATA_PATH: LazyLock<Arc<Path>> = LazyLock::new(|| {
	let data_path = dirs::data_dir().unwrap().join("Generic DAW").into();
	_ = std::fs::create_dir(&data_path);
	data_path
});

#[derive(Clone, Debug)]
pub enum Message {
	ClapHost(clap_host::Message),
	Batch(Batch),

	SetArrangement(NoClone<Box<Arrangement>>),

	ConnectRequest(NodeId, NodeId),
	ConnectSucceeded((NodeId, NodeId)),
	Disconnect(NodeId, NodeId),

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

	SampleLoadFromFile(Arc<Path>),
	SampleLoadedFromFile(NoClone<Option<Box<SamplePair>>>),
	AddAudioClip(SampleId),

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
	PlaylistPan(Vector),
	PlaylistZoom(Vector, Point),

	PianoRollAction(piano_roll::Action),
	PianoRollPan(Vector, Size),
	PianoRollZoom(Vector, Point, Size),

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

	pub recording: Option<(Recording, NodeId)>,
	pub tab: Tab,

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
	pub fn new(
		config: &Config,
		state: &State,
		plugin_bundles: &HashMap<PluginDescriptor, NoDebug<PluginBundle>>,
	) -> (Self, Task<Message>) {
		let (arrangement, task) = Arrangement::create(config);

		let mut plugins = plugin_bundles.keys().cloned().collect::<Vec<_>>();
		plugins.sort_unstable();

		let playlist_scale_x = (arrangement.rtstate().sample_rate.get() as f32).log2() - 5.0;
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
				plugins: combo_box::State::new(plugins),

				loading: 0,
			},
			task.map(Message::Batch),
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
			Message::Batch(msg) => {
				return Task::batch(
					self.arrangement
						.update(msg)
						.into_iter()
						.flatten()
						.map(|msg| self.update(msg, config, state, plugin_bundles)),
				);
			}
			Message::SetArrangement(NoClone(arrangement)) => {
				let pos_fact = arrangement.rtstate().sample_rate.get() as f32
					/ self.arrangement.rtstate().sample_rate.get() as f32;
				let scale_diff = pos_fact.log2();
				self.playlist_position.x *= pos_fact;
				self.playlist_scale.x += scale_diff;
				self.playlist_selection.get_mut().clear();
				self.selected_channel = None;
				self.piano_roll_position.x *= pos_fact;
				self.piano_roll_scale.x += scale_diff;
				self.piano_roll_selection.get_mut().clear();
				if matches!(self.tab, Tab::PianoRoll { .. }) {
					self.tab = Tab::Playlist;
				}
				self.arrangement = *arrangement;
				return Task::batch([
					self.update(
						Message::PlaylistZoom(Vector::ZERO, Point::ORIGIN),
						config,
						state,
						plugin_bundles,
					),
					self.update(
						Message::PianoRollZoom(Vector::ZERO, Point::ORIGIN, Size::ZERO),
						config,
						state,
						plugin_bundles,
					),
				]);
			}
			Message::ConnectRequest(from, to) => {
				return Task::perform(self.arrangement.request_connect(from, to), Result::ok)
					.and_then(Task::done)
					.map(Message::ConnectSucceeded);
			}
			Message::ConnectSucceeded((from, to)) => self.arrangement.connect_succeeded(from, to),
			Message::Disconnect(from, to) => self.arrangement.disconnect(from, to),
			Message::ChannelAdd => {
				let id = self.arrangement.add_channel();
				return Task::perform(
					self.arrangement
						.request_connect(id, self.arrangement.master().id),
					Result::ok,
				)
				.and_then(Task::done)
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
					self.arrangement.rtstate().sample_rate,
					self.arrangement.rtstate().frames,
					&HOST,
				);
				let id = plugin.plugin_id();

				self.arrangement.plugin_load(node, audio_processor);

				let mut fut = self.clap_host.update(
					clap_host::Message::Loaded(NoClone((Box::new(Fragile::new(plugin)), receiver))),
					config,
				);

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
			Message::SampleLoadFromFile(path) => {
				let mut iter = self.arrangement.samples().values();
				if let Some(sample) = iter.find(|sample| sample.path == path) {
					drop(iter);
					return self.update(
						Message::AddAudioClip(sample.id),
						config,
						state,
						plugin_bundles,
					);
				}

				self.loading += 1;
				let sample_rate = self.arrangement.rtstate().sample_rate;

				return Task::future(unblock(move || {
					Message::SampleLoadedFromFile(NoClone(
						SamplePair::new(path, sample_rate).map(Box::new),
					))
				}));
			}
			Message::SampleLoadedFromFile(NoClone(sample)) => {
				self.loading -= 1;

				if let Some(sample) = sample {
					let id = sample.gui.id;
					self.arrangement.add_sample(*sample);
					return self.update(Message::AddAudioClip(id), config, state, plugin_bundles);
				}
			}
			Message::AddAudioClip(sample) => {
				let audio = AudioClip::new(
					sample,
					self.arrangement.samples()[*sample].samples.len(),
					self.arrangement.rtstate(),
				);

				let (task, track) = self
					.arrangement
					.tracks()
					.iter()
					.position(|track| {
						track
							.clips
							.iter()
							.all(|clip| clip.position().start() >= audio.position.len())
					})
					.map_or_else(
						|| {
							(
								self.update(Message::TrackAdd, config, state, plugin_bundles),
								self.arrangement.tracks().len() - 1,
							)
						},
						|track| (Task::none(), track),
					);

				self.arrangement.add_clip(track, audio);

				return task;
			}
			Message::TrackAdd => {
				let track = self.arrangement.add_track();
				let id = self.arrangement.tracks()[track].id;
				if self.soloed_track.is_some() {
					self.arrangement.channel_toggle_enabled(id);
				}
				return Task::perform(
					self.arrangement
						.request_connect(id, self.arrangement.master().id),
					Result::ok,
				)
				.and_then(Task::done)
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

				return Task::batch([
					self.update(
						Message::PlaylistPan(Vector::ZERO),
						config,
						state,
						plugin_bundles,
					),
					self.update(Message::RecordingEndStream, config, state, plugin_bundles),
				]);
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

					let sample = recording.split_off(recording_path(), self.arrangement.rtstate());
					let id = sample.gui.id;
					self.arrangement.add_sample(sample);

					let track = self.arrangement.track_of(*r_node).unwrap();

					let mut clip = AudioClip::new(
						id,
						self.arrangement.samples()[*id].samples.len(),
						self.arrangement.rtstate(),
					);
					clip.position.move_to(pos);
					self.arrangement.add_clip(track, clip);

					self.recording.as_mut().unwrap().1 = node;
				} else {
					let (recording, task) = Recording::create(
						recording_path(),
						self.arrangement.rtstate(),
						config.input_device.name.clone(),
						config.input_device.sample_rate,
						config.input_device.buffer_size,
					);

					self.recording = Some((recording, node));
					self.arrangement.play();

					return poll_consumer(
						task,
						config.input_device.sample_rate,
						config.input_device.buffer_size,
					)
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
						self.arrangement.rtstate(),
					);
					clip.position.move_to(pos);
					self.arrangement.add_clip(track, clip);
				}
			}
			Message::RecordingWrite(samples) => self.recording.as_mut().unwrap().0.write(&samples),
			Message::PlaylistAction(action) => self.handle_playlist_action(action),
			Message::PlaylistPan(pos_diff) => {
				self.playlist_position = self.playlist_position + pos_diff;
				self.playlist_position.x = self.playlist_position.x.max(0.0);
				self.playlist_position.y = self.playlist_position.y.clamp(
					0.0,
					self.playlist_scale.y
						* self.arrangement.tracks().len().saturating_sub(1) as f32,
				);
			}
			Message::PlaylistZoom(scale_diff, cursor) => {
				let old_scale = self.playlist_scale;

				self.playlist_scale = old_scale + scale_diff;
				self.playlist_scale.x = self.playlist_scale.x.clamp(1.0, 16f32.next_down());
				self.playlist_scale.y = self.playlist_scale.y.clamp(46.0, 200.0);

				let pos_diff = Vector::new(
					cursor.x * (old_scale.x.exp2() - self.playlist_scale.x.exp2()),
					(cursor.y + self.playlist_position.y)
						* (old_scale.y.recip() - self.playlist_scale.y.recip())
						* self.playlist_scale.y,
				);

				return self.update(
					Message::PlaylistPan(pos_diff),
					config,
					state,
					plugin_bundles,
				);
			}
			Message::PianoRollAction(action) => self.handle_piano_roll_action(action),
			Message::PianoRollPan(pos_diff, size) => {
				self.piano_roll_position = self.piano_roll_position + pos_diff;
				self.piano_roll_position.x = self.piano_roll_position.x.max(0.0);
				self.piano_roll_position.y = self
					.piano_roll_position
					.y
					.clamp(0.0, self.piano_roll_scale.y.mul_add(128.0, -size.height));
			}
			Message::PianoRollZoom(scale_diff, cursor, size) => {
				let old_scale = self.piano_roll_scale;

				self.piano_roll_scale = old_scale + scale_diff;
				self.piano_roll_scale.x = self.piano_roll_scale.x.clamp(1.0, 16f32.next_down());
				self.piano_roll_scale.y = self
					.piano_roll_scale
					.y
					.clamp(LINE_HEIGHT, 2.0 * LINE_HEIGHT);

				let pos_diff = Vector::new(
					cursor.x * (old_scale.x.exp2() - self.piano_roll_scale.x.exp2()),
					(cursor.y + self.piano_roll_position.y)
						* (old_scale.y.recip() - self.piano_roll_scale.y.recip())
						* self.piano_roll_scale.y,
				);

				return self.update(
					Message::PianoRollPan(pos_diff, size),
					config,
					state,
					plugin_bundles,
				);
			}
			Message::DeleteSelection => match self.tab {
				Tab::Playlist => self.handle_piano_roll_action(piano_roll::Action::Delete),
				Tab::Mixer => {}
				Tab::PianoRoll(..) => self.handle_playlist_action(playlist::Action::Delete),
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

	fn handle_playlist_action(&mut self, action: playlist::Action) {
		let playlist::Selection {
			primary, secondary, ..
		} = self.playlist_selection.get_mut();

		match action {
			playlist::Action::Open => {
				debug_assert_eq!(primary.len(), 1);
				let &(track, clip) = primary.iter().next().unwrap();

				let clip::Clip::Midi(clip) = self.arrangement.tracks()[track].clips[clip] else {
					panic!()
				};

				self.piano_roll_selection.get_mut().primary.clear();
				self.tab = Tab::PianoRoll(clip);
			}
			playlist::Action::Add(track, pos) => {
				let pattern = MidiPatternPair::new(Vec::new());
				let id = pattern.gui.id;
				self.arrangement.add_midi_pattern(pattern);

				let mut clip = MidiClip::new(id);
				clip.position.trim_end_to(
					MusicalTime::BEAT * 4 * u64::from(self.arrangement.rtstate().numerator.get()),
				);
				clip.position.move_to(pos);
				let clip = self.arrangement.add_clip(track, clip);
				primary.insert((track, clip));
			}
			playlist::Action::Clone => {
				let mut new = HashSet::new();
				for &(track, clip) in &*primary {
					new.insert((
						track,
						self.arrangement
							.add_clip(track, self.arrangement.tracks()[track].clips[clip]),
					));
				}
				*primary = new;
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
				let filtered = primary
					.drain()
					.filter(|&(track, clip)| {
						let position = self.arrangement.tracks()[track].clips[clip].position();
						(position.start()..=position.end()).contains(&pos)
					})
					.collect::<Vec<_>>();

				for (track, lhs) in filtered {
					let clip = self.arrangement.tracks()[track].clips[lhs];
					if clip.position().start() == pos {
						primary.insert((track, lhs));
					} else if clip.position().end() == pos {
						secondary.insert((track, lhs));
					} else if clip.position().len() > MusicalTime::TICK {
						let start = clip.position().start() + MusicalTime::TICK;
						let end = clip.position().end() - MusicalTime::TICK;
						pos = pos.clamp(start, end);
						let rhs = self.arrangement.add_clip(track, clip);
						self.arrangement.clip_trim_end_to(track, lhs, pos);
						self.arrangement.clip_trim_start_to(track, rhs, pos);
						primary.insert((track, lhs));
						secondary.insert((track, rhs));
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
	}

	fn handle_piano_roll_action(&mut self, action: piano_roll::Action) {
		let Tab::PianoRoll(clip) = &mut self.tab else {
			panic!()
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
				let mut new = BitSet::new();
				for note in &*primary {
					new.insert(self.arrangement.add_note(
						clip.pattern,
						self.arrangement.midi_patterns()[*clip.pattern].notes[note],
					));
				}
				*primary = new;
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
				let filtered = primary
					.iter()
					.filter(|&lhs| {
						let note = self.arrangement.midi_patterns()[*clip.pattern].notes[lhs];
						(note.position.start()..=note.position.end()).contains(&pos)
					})
					.collect::<BitSet>();
				primary.clear();

				for lhs in &filtered {
					let note = self.arrangement.midi_patterns()[*clip.pattern].notes[lhs];
					if note.position.start() == pos {
						primary.insert(lhs);
					} else if note.position.end() == pos {
						secondary.insert(lhs);
					} else if note.position.len() > MusicalTime::TICK {
						let start = note.position.start() + MusicalTime::TICK;
						let end = note.position.end() - MusicalTime::TICK;
						pos = pos.clamp(start, end);
						let rhs = self.arrangement.add_note(clip.pattern, note);
						self.arrangement.note_trim_end_to(clip.pattern, lhs, pos);
						self.arrangement.note_trim_start_to(clip.pattern, rhs, pos);
						primary.insert(lhs);
						secondary.insert(rhs);
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
			self.arrangement.rtstate(),
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
									PeakMeter::new(&node.peaks_lin[0], node.enabled),
									PeakMeter::new(&node.peaks_lin[1], node.enabled)
								]
								.spacing(2),
								column![
									Knob::new(0.0..=1.0, node.volume.abs().cbrt(), move |v| {
										Message::ChannelVolumeChanged(
											id,
											v.powi(3).copysign(node.volume),
										)
									})
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
									button(
										Dot::new(
											self.recording.as_ref().is_some_and(|&(_, i)| i == id)
										)
										.radius(5.5)
									)
									.padding(2.0)
									.on_press(Message::Recording(id))
									.style(button_style(
										self.recording.as_ref().is_some_and(|&(_, i)| i == id)
									))
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
				self.arrangement.rtstate(),
				&self.playlist_position,
				&self.playlist_scale,
				self.arrangement
					.tracks()
					.iter()
					.enumerate()
					.map(|(track_idx, track)| {
						let node = self.arrangement.node(track.id);

						Track::new(
							track_idx,
							self.arrangement.rtstate(),
							&self.playlist_position,
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
										self.arrangement.rtstate(),
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
										self.arrangement.rtstate(),
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
												self.arrangement.rtstate(),
												&self.playlist_position,
												&self.playlist_scale,
												node.enabled,
												Message::PlaylistAction,
											)
										}),
								),
							Message::PlaylistAction,
						)
					}),
				Message::PlaylistAction,
			),
			Message::SeekTo,
			Message::SetLoopMarker,
			|p, _| Message::PlaylistPan(p),
			|s, c, _| Message::PlaylistZoom(s, c),
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
					.chain({
						let mut iter = self
							.arrangement
							.channels()
							.enumerate()
							.map(|(i, node)| self.channel(node, format!("C{}", i + 1)))
							.peekable();

						let one = iter.peek().map(|_| rule::vertical(1).into());
						iter.chain(one)
					})
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
						dragking::column(node.plugins.iter().enumerate().map(|(i, plugin)| {
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

		button(
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
					PeakMeter::new(&node.peaks_cbrt[0], node.enabled).width(16.0),
					vertical_slider(0.0..=1.0, node.volume.abs().cbrt(), |v| {
						Message::ChannelVolumeChanged(node.id, v.powi(3).copysign(node.volume))
					})
					.default(1.0)
					.width(17)
					.step(f32::EPSILON)
					.style(if node.enabled {
						slider::default
					} else {
						slider_secondary
					}),
					PeakMeter::new(&node.peaks_cbrt[1], node.enabled).width(16.0),
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
		.on_press(Message::ChannelSelect(node.id))
		.style(move |t, _| {
			let pair = if Some(node.id) == self.selected_channel {
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

	fn piano_roll(&self, clip: MidiClip) -> Element<'_, Message> {
		Seeker::new(
			self.arrangement.rtstate(),
			&self.piano_roll_position,
			&self.piano_roll_scale,
			Piano::new(&self.piano_roll_position, &self.piano_roll_scale),
			PianoRoll::new(
				&self.piano_roll_selection,
				&self.arrangement.midi_patterns()[*clip.pattern].notes,
				self.arrangement.rtstate(),
				&self.piano_roll_position,
				&self.piano_roll_scale,
				Message::PianoRollAction,
			),
			Message::SeekTo,
			Message::SetLoopMarker,
			Message::PianoRollPan,
			Message::PianoRollZoom,
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

fn recording_path() -> Arc<Path> {
	DATA_PATH
		.join(format!(
			"recording-{}.wav",
			format_rfc3339(SystemTime::now())
		))
		.into()
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

fn poll_consumer<T: Send + 'static>(
	mut consumer: Consumer<T>,
	sample_rate: NonZero<u32>,
	frames: Option<NonZero<u32>>,
) -> Task<T> {
	let min = 64.0 / sample_rate.get() as f32;
	let max = frames.or(NonZero::new(8192)).unwrap().get() as f32 / sample_rate.get() as f32;
	let mut backoff = 0.0;
	let mut backoff = move |counter: u16| {
		backoff = if counter == 0 {
			backoff * SQRT_2
		} else {
			backoff / f32::from(counter)
		}
		.clamp(min, max);
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
						break;
					}
				}
				if consumer.is_abandoned() {
					break;
				}
				backoff(counter).await;
			}
		},
	))
}
