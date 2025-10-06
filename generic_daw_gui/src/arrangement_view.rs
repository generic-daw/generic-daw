use crate::{
	arrangement_view::{
		audio_clip::AudioClip, clip::Clip, midi_clip::MidiClip, node::NodeType,
		pattern::PatternPair, sample::SamplePair,
	},
	clap_host::{ClapHost, Message as ClapHostMessage},
	components::{
		icon_button, styled_scrollable, styled_scrollable_with_direction, text_icon_button,
	},
	config::Config,
	daw::DEFAULT_SPLIT_POSITION,
	icons::{arrow_up_down, chevron_up, circle_off, grip_vertical, plus, x},
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
use dragking::DragEvent;
use fragile::Fragile;
use generic_daw_core::{
	Batch, Flags, MidiNote, MusicalTime, NodeId, NotePosition, PanMode, SampleId,
	clap_host::{HostInfo, MainThreadMessage, Plugin, PluginBundle, PluginDescriptor},
};
use generic_daw_utils::{NoClone, NoDebug, Vec2};
use generic_daw_widget::{dot::Dot, knob::Knob, peak_meter::PeakMeter};
use humantime::format_rfc3339;
use iced::{
	Center, Element, Fill, Function as _, Shrink, Size, Subscription, Task, border,
	futures::SinkExt as _,
	mouse::Interaction,
	overlay::menu,
	padding, stream,
	widget::{
		button, column, combo_box, container, mouse_area, row, rule, scrollable, slider, space,
		text, vertical_slider,
	},
};
use iced_persistent::persistent;
use iced_split::{Strategy, vertical_split};
use node::Node;
use rtrb::Consumer;
use smol::{Timer, unblock};
use std::{
	collections::HashMap,
	fmt::Write as _,
	io::Read,
	iter::once,
	path::Path,
	sync::{
		Arc, LazyLock,
		atomic::{self, Ordering::Acquire},
	},
	time::{Duration, Instant, SystemTime},
};

mod arrangement;
mod audio_clip;
mod clip;
mod midi_clip;
mod node;
mod pattern;
mod plugin;
mod project;
mod recording;
mod sample;
mod track;

pub use arrangement::Arrangement as ArrangementWrapper;
pub use audio_clip::AudioClipRef;
pub use midi_clip::MidiClipRef;
pub use project::Feedback;
pub use recording::Recording;

#[derive(Clone, Debug)]
pub enum Message {
	ClapHost(ClapHostMessage),
	Batch(Batch),

	SetArrangement(NoClone<Box<ArrangementWrapper>>),

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
	ChannelInvertPolarity(NodeId),

	PluginLoad(NodeId, PluginDescriptor, bool),
	PluginSetState(NodeId, usize, NoDebug<Box<[u8]>>),
	PluginMixChanged(NodeId, usize, f32),
	PluginToggleEnabled(NodeId, usize),
	PluginMoveTo(NodeId, DragEvent),
	PluginRemove(NodeId, usize),

	SampleLoadFromFile(Arc<Path>),
	SampleLoadedFromFile(NoClone<Option<Box<SamplePair>>>),
	AddSample(SampleId),

	OpenMidiClip(MidiClip),

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

	ArrangementAction(ArrangementAction),
	ArrangementPositionScaleDelta(Vec2, Vec2),

	PianoRollAction(PianoRollAction),
	PianoRollPositionScaleDelta(Vec2, Vec2, Size),

	SplitAt(f32),
}

#[derive(Clone, Copy, Debug)]
pub enum Tab {
	Arrangement {
		grabbed_clip: Option<(usize, usize, Option<usize>)>,
	},
	Mixer,
	PianoRoll {
		clip: MidiClip,
		grabbed_note: Option<(usize, Option<usize>)>,
	},
}

#[derive(Debug)]
pub struct ArrangementView {
	pub arrangement: ArrangementWrapper,
	pub clap_host: ClapHost,

	pub recording: Option<(Recording, NodeId)>,
	pub tab: Tab,

	arrangement_position: Vec2,
	arrangement_scale: Vec2,
	soloed_track: Option<NodeId>,

	piano_roll_position: Vec2,
	piano_roll_scale: Vec2,
	last_note_len: MusicalTime,
	selected_channel: Option<NodeId>,

	split_at: f32,
	plugins: combo_box::State<PluginDescriptor>,
	tree: iced_persistent::Tree,
}

impl ArrangementView {
	pub fn new(
		config: &Config,
		plugin_bundles: &HashMap<PluginDescriptor, NoDebug<PluginBundle>>,
	) -> (Self, Task<Message>) {
		let (arrangement, task) = ArrangementWrapper::create(config);

		let mut plugins = plugin_bundles.keys().cloned().collect::<Vec<_>>();
		plugins.sort_unstable();

		(
			Self {
				arrangement,
				clap_host: ClapHost::default(),

				recording: None,
				tab: Tab::Arrangement { grabbed_clip: None },

				arrangement_position: Vec2::default(),
				arrangement_scale: Vec2::new(10.0, 87.0),
				soloed_track: None,

				piano_roll_position: Vec2::new(0.0, 40.0),
				piano_roll_scale: Vec2::new(8.0, LINE_HEIGHT),
				last_note_len: MusicalTime::BEAT,
				selected_channel: None,

				split_at: DEFAULT_SPLIT_POSITION,
				plugins: combo_box::State::new(plugins),
				tree: iced_persistent::Tree::empty(),
			},
			task.map(Message::Batch),
		)
	}

	pub fn update(
		&mut self,
		message: Message,
		config: &Config,
		plugin_bundles: &HashMap<PluginDescriptor, NoDebug<PluginBundle>>,
	) -> Task<Message> {
		match message {
			Message::ClapHost(msg) => {
				return self.clap_host.update(msg, config).map(Message::ClapHost);
			}
			Message::Batch(msg) => {
				return Task::batch(
					self.arrangement
						.update(msg, Instant::now())
						.into_iter()
						.flatten()
						.map(|msg| self.update(msg, config, plugin_bundles)),
				);
			}
			Message::SetArrangement(NoClone(arrangement)) => {
				self.arrangement = *arrangement;
				self.selected_channel = None;
				match &mut self.tab {
					Tab::Arrangement { grabbed_clip } => *grabbed_clip = None,
					Tab::Mixer => {}
					Tab::PianoRoll { .. } => self.tab = Tab::Arrangement { grabbed_clip: None },
				}
			}
			Message::ConnectRequest(from, to) => {
				return Task::future(self.arrangement.request_connect(from, to))
					.and_then(Task::done)
					.map(Message::ConnectSucceeded);
			}
			Message::ConnectSucceeded((from, to)) => self.arrangement.connect_succeeded(from, to),
			Message::Disconnect(from, to) => self.arrangement.disconnect(from, to),
			Message::ChannelAdd => {
				let node = self.arrangement.add_channel();
				return Task::future(
					self.arrangement
						.request_connect(node, self.arrangement.master().id),
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
			Message::ChannelInvertPolarity(id) => self.arrangement.channel_toggle_polarity(id),
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
					ClapHostMessage::Loaded(NoClone((Box::new(Fragile::new(plugin)), receiver))),
					config,
				);

				if show {
					fut = Task::batch([
						fut,
						self.clap_host.update(
							ClapHostMessage::MainThread(id, MainThreadMessage::GuiRequestShow),
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
					.update(ClapHostMessage::SetState(id, state), config)
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
			Message::PluginRemove(node, i) => {
				self.arrangement.plugin_remove(node, i);
			}
			Message::SampleLoadFromFile(path) => {
				let mut iter = self.arrangement.samples().values();
				if let Some(sample) = iter.find(|sample| sample.path == path) {
					drop(iter);
					return self.update(Message::AddSample(sample.id), config, plugin_bundles);
				}

				let sample_rate = self.arrangement.rtstate().sample_rate;

				return Task::perform(
					{
						let path = path.clone();
						unblock(move || {
							NoClone(SamplePair::new(path.clone(), sample_rate).map(Box::new))
						})
					},
					Message::SampleLoadedFromFile,
				);
			}
			Message::SampleLoadedFromFile(NoClone(sample)) => {
				let Some(sample) = sample else {
					return Task::none();
				};
				let id = sample.gui.id;
				self.arrangement.add_sample(*sample);
				return self.update(Message::AddSample(id), config, plugin_bundles);
			}
			Message::AddSample(sample) => {
				let task = self.update(Message::TrackAdd, config, plugin_bundles);
				self.arrangement.add_clip(
					self.arrangement.tracks().len() - 1,
					AudioClip::new(
						sample,
						self.arrangement.samples()[*sample].len,
						self.arrangement.rtstate(),
					),
				);
				return task;
			}
			Message::OpenMidiClip(clip) => {
				self.tab = Tab::PianoRoll {
					clip,
					grabbed_note: None,
				}
			}
			Message::TrackAdd => {
				let track = self.arrangement.add_track();
				if self.soloed_track.is_some() {
					self.arrangement
						.channel_toggle_enabled(self.arrangement.tracks()[track].id);
				}
				return Task::future(self.arrangement.request_connect(
					self.arrangement.tracks()[track].id,
					self.arrangement.master().id,
				))
				.and_then(Task::done)
				.map(Message::ConnectSucceeded);
			}
			Message::TrackRemove(id) => {
				self.arrangement.remove_track(id);
				if self.soloed_track == Some(id) {
					self.soloed_track = None;
				}

				return Task::batch([
					self.update(
						Message::ArrangementPositionScaleDelta(Vec2::ZERO, Vec2::ZERO),
						config,
						plugin_bundles,
					),
					self.update(Message::RecordingEndStream, config, plugin_bundles),
				]);
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
			Message::TogglePlayback => {
				self.arrangement.toggle_playback();
				return self.update(Message::RecordingEndStream, config, plugin_bundles);
			}
			Message::Stop => {
				self.arrangement.stop();
				return self.update(Message::RecordingEndStream, config, plugin_bundles);
			}
			Message::SeekTo(pos) => {
				self.arrangement.seek_to(pos);
				return self.update(Message::RecordingEndStream, config, plugin_bundles);
			}
			Message::SetLoopMarker(marker) => self.arrangement.set_loop_marker(marker),
			Message::Recording(node) => {
				if let Some((recording, r_node)) = &mut self.recording {
					if node == *r_node {
						return self.update(Message::RecordingEndStream, config, plugin_bundles);
					}

					let pos = recording.position;

					let sample = recording.split_off(recording_path(), self.arrangement.rtstate());
					let id = sample.gui.id;
					self.arrangement.add_sample(sample);

					let track = self.arrangement.track_of(*r_node).unwrap();

					let mut clip = AudioClip::new(
						id,
						self.arrangement.samples()[*id].len,
						self.arrangement.rtstate(),
					);
					clip.position.move_to(pos);
					self.arrangement.add_clip(track, clip);

					self.recording.as_mut().unwrap().1 = node;
				} else {
					let (recording, task) = Recording::create(
						recording_path(),
						self.arrangement.rtstate(),
						config.input_device.name.as_deref(),
						config.input_device.sample_rate.unwrap_or(44100),
						config.input_device.buffer_size.unwrap_or(1024),
					);

					let sample_rate = recording.sample_rate();
					let frames = recording
						.frames()
						.or(config.input_device.buffer_size)
						.unwrap_or(1024);

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
						self.arrangement.samples()[*id].len,
						self.arrangement.rtstate(),
					);
					clip.position.move_to(pos);
					self.arrangement.add_clip(track, clip);
				}
			}
			Message::RecordingWrite(samples) => self.recording.as_mut().unwrap().0.write(&samples),
			Message::ArrangementAction(action) => self.handle_arrangement_action(action),
			Message::ArrangementPositionScaleDelta(pos, scale) => {
				let old_scale = self.arrangement_scale;

				self.arrangement_scale += scale;
				self.arrangement_scale.x = self.arrangement_scale.x.clamp(3.0, 15f32.next_down());
				self.arrangement_scale.y = self.arrangement_scale.y.clamp(46.0, 200.0);

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

				if scale == Vec2::ZERO || old_scale != self.piano_roll_scale {
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
			ArrangementAction::Grab(track, clip) => *grabbed_clip = Some((track, clip, None)),
			ArrangementAction::Drop => *grabbed_clip = None,
			ArrangementAction::Add(track, pos) => {
				let pattern = PatternPair::new(Vec::new());
				let id = pattern.gui.id;
				self.arrangement.add_pattern(pattern);

				let mut clip = MidiClip::new(id);
				clip.position.trim_end_to(
					MusicalTime::BEAT * 4 * u32::from(self.arrangement.rtstate().numerator),
				);
				clip.position.move_to(pos);
				let clip = self.arrangement.add_clip(track, clip);
				*grabbed_clip = Some((track, clip, None));
			}
			ArrangementAction::Clone(track, mut clip) => {
				clip = self
					.arrangement
					.add_clip(track, self.arrangement.tracks()[track].clips[clip]);
				*grabbed_clip = Some((track, clip, None));
			}
			ArrangementAction::Drag(new_track, pos) => {
				let (track, clip, ..) = grabbed_clip.as_mut().unwrap();
				if *track != new_track {
					*clip = self.arrangement.clip_switch_track(*track, *clip, new_track);
					*track = new_track;
				}
				self.arrangement.clip_move_to(*track, *clip, pos);
			}
			ArrangementAction::SplitAt(track, lhs, mut pos) => {
				let clip = self.arrangement.tracks()[track].clips[lhs];
				let (lhs, rhs) = if clip.position().end() == pos
					&& let Some(rhs) = self.arrangement.tracks()[track]
						.clips
						.iter()
						.position(|clip| clip.position().start() == pos)
				{
					(lhs, rhs)
				} else if clip.position().start() == pos
					&& let Some(rhs) = self.arrangement.tracks()[track]
						.clips
						.iter()
						.position(|clip| clip.position().end() == pos)
				{
					(rhs, lhs)
				} else {
					let start = clip.position().start() + MusicalTime::TICK;
					let end = clip.position().end() - MusicalTime::TICK;
					if start > end {
						return;
					}
					let rhs = self.arrangement.add_clip(track, clip);
					pos = pos.clamp(start, end);
					self.arrangement.clip_trim_end_to(track, lhs, pos);
					self.arrangement.clip_trim_start_to(track, rhs, pos);
					(lhs, rhs)
				};
				*grabbed_clip = Some((track, lhs, Some(rhs)));
			}
			ArrangementAction::DragSplit(mut pos) => {
				let Some((track, lhs, Some(rhs))) = *grabbed_clip else {
					return;
				};
				let start = self.arrangement.tracks()[track].clips[lhs]
					.position()
					.start() + MusicalTime::TICK;
				let end = self.arrangement.tracks()[track].clips[rhs].position().end()
					- MusicalTime::TICK;
				if start > end {
					return;
				}
				pos = pos.clamp(start, end);
				self.arrangement.clip_trim_end_to(track, lhs, pos);
				self.arrangement.clip_trim_start_to(track, rhs, pos);
			}
			ArrangementAction::TrimStart(pos) => {
				let (track, clip, ..) = grabbed_clip.unwrap();
				self.arrangement.clip_trim_start_to(track, clip, pos);
			}
			ArrangementAction::TrimEnd(pos) => {
				let (track, clip, ..) = grabbed_clip.unwrap();
				self.arrangement.clip_trim_end_to(track, clip, pos);
			}
			ArrangementAction::Delete(track, clip) => {
				let clip = self.arrangement.remove_clip(track, clip);
				self.arrangement.gc(clip);
			}
		}
	}

	fn handle_piano_roll_action(&mut self, action: PianoRollAction) {
		let Tab::PianoRoll { clip, grabbed_note } = &mut self.tab else {
			panic!()
		};

		let notes = &self.arrangement.patterns()[*clip.pattern].notes;

		match action {
			PianoRollAction::Grab(note) => *grabbed_note = Some((note, None)),
			PianoRollAction::Drop => {
				let (note, ..) = grabbed_note.take().unwrap();
				self.last_note_len = notes[note].position.len();
			}
			PianoRollAction::Add(key, pos) => {
				let note = self.arrangement.add_note(
					clip.pattern,
					MidiNote {
						key,
						velocity: 1.0,
						position: NotePosition::new(pos, pos + self.last_note_len),
					},
				);
				*grabbed_note = Some((note, None));
			}
			PianoRollAction::Clone(note) => {
				let note = self.arrangement.add_note(clip.pattern, notes[note]);
				*grabbed_note = Some((note, None));
			}
			PianoRollAction::Drag(key, pos) => {
				let (note, ..) = grabbed_note.unwrap();
				if notes[note].key != key {
					self.arrangement.note_switch_key(clip.pattern, note, key);
				}
				self.arrangement.note_move_to(clip.pattern, note, pos);
			}
			PianoRollAction::SplitAt(lhs, mut pos) => {
				let note = notes[lhs];
				let (lhs, rhs) = if note.position.end() == pos
					&& let Some(rhs) = notes.iter().position(|note| note.position.start() == pos)
				{
					(lhs, rhs)
				} else if clip.position.start() == pos
					&& let Some(rhs) = notes.iter().position(|note| note.position.end() == pos)
				{
					(rhs, lhs)
				} else {
					let start = note.position.start() + MusicalTime::TICK;
					let end = note.position.end() - MusicalTime::TICK;
					if start > end {
						return;
					}
					let rhs = self.arrangement.add_note(clip.pattern, note);
					pos = pos.clamp(start, end);
					self.arrangement.note_trim_end_to(clip.pattern, lhs, pos);
					self.arrangement.note_trim_start_to(clip.pattern, rhs, pos);
					(lhs, rhs)
				};
				*grabbed_note = Some((lhs, Some(rhs)));
			}
			PianoRollAction::DragSplit(mut pos) => {
				let Some((lhs, Some(rhs))) = *grabbed_note else {
					return;
				};
				let start = notes[lhs].position.start() + MusicalTime::TICK;
				let end = notes[rhs].position.end() - MusicalTime::TICK;
				if start > end {
					return;
				}
				pos = pos.clamp(start, end);
				self.arrangement.note_trim_end_to(clip.pattern, lhs, pos);
				self.arrangement.note_trim_start_to(clip.pattern, rhs, pos);
			}
			PianoRollAction::TrimStart(pos) => {
				let (note, ..) = grabbed_note.unwrap();
				self.arrangement.note_trim_start_to(clip.pattern, note, pos);
			}
			PianoRollAction::TrimEnd(pos) => {
				let (note, ..) = grabbed_note.unwrap();
				self.arrangement.note_trim_end_to(clip.pattern, note, pos);
			}
			PianoRollAction::Delete(note) => _ = self.arrangement.remove_note(clip.pattern, note),
		}
	}

	pub fn view(&self) -> Element<'_, Message> {
		match &self.tab {
			Tab::Arrangement { .. } => self.arrangement(),
			Tab::Mixer => self.mixer(),
			Tab::PianoRoll { clip, .. } => self.piano_roll(clip),
		}
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
						let node = self.arrangement.node(id);

						let button_style = |cond: bool| {
							if !node.flags.contains(Flags::ENABLED) {
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
									PeakMeter::new(
										&node.peaks[0][0],
										node.flags.contains(Flags::ENABLED)
									),
									PeakMeter::new(
										&node.peaks[0][1],
										node.flags.contains(Flags::ENABLED)
									)
								]
								.spacing(2),
								column![
									Knob::new(0.0..=1.0, node.volume.cbrt(), move |v| {
										Message::ChannelVolumeChanged(id, v.powi(3))
									})
									.enabled(node.flags.contains(Flags::ENABLED))
									.tooltip(format_decibels(node.volume)),
									node.pan_knob(20.0),
								]
								.align_x(Center)
								.spacing(5)
								.wrap(),
								column![
									icon_button(
										x(),
										if node.flags.contains(Flags::ENABLED) {
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
						.height(self.arrangement_scale.y)
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
			ArrangementWidget::new(
				self.arrangement.rtstate(),
				&self.arrangement_position,
				&self.arrangement_scale,
				column(
					self.arrangement
						.tracks()
						.iter()
						.map(|track| {
							let node = self.arrangement.node(track.id);

							TrackWidget::new(
								&self.arrangement_scale,
								track
									.clips
									.iter()
									.map(|clip| match clip {
										Clip::Audio(clip) => AudioClipWidget::new(
											AudioClipRef {
												sample: &self.arrangement.samples()[*clip.sample],
												clip,
											},
											self.arrangement.rtstate(),
											&self.arrangement_position,
											&self.arrangement_scale,
											node.flags.contains(Flags::ENABLED),
										)
										.into(),
										Clip::Midi(clip) => MidiClipWidget::new(
											MidiClipRef {
												pattern: &self.arrangement.patterns()
													[*clip.pattern],
												clip,
											},
											self.arrangement.rtstate(),
											&self.arrangement_position,
											&self.arrangement_scale,
											node.flags.contains(Flags::ENABLED),
											Message::OpenMidiClip(*clip),
										)
										.into(),
									})
									.chain(
										self.recording
											.as_ref()
											.filter(|&&(_, i)| i == track.id)
											.map(|(recording, _)| {
												RecordingWidget::new(
													recording,
													self.arrangement.rtstate(),
													&self.arrangement_position,
													&self.arrangement_scale,
												)
												.into()
											}),
									),
							)
						})
						.map(Element::new),
				),
				Message::ArrangementAction,
			),
			Message::SeekTo,
			Message::SetLoopMarker,
			|p, s, _| Message::ArrangementPositionScaleDelta(p, s),
		)
		.into()
	}

	fn mixer(&self) -> Element<'_, Message> {
		let mixer_panel = persistent(
			styled_scrollable_with_direction(
				row(
					once(self.channel(self.arrangement.master(), "M".to_owned()))
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
						)),
				)
				.align_y(Center)
				.spacing(5),
				scrollable::Direction::Horizontal(scrollable::Scrollbar::default()),
			)
			.width(Fill),
			&self.tree,
		);

		if let Some(selected) = self.selected_channel {
			vertical_split(
				mixer_panel,
				column![
					combo_box(&self.plugins, "Add Plugin", None, move |descriptor| {
						Message::PluginLoad(selected, descriptor, true)
					})
					.menu_style(menu_with_border(menu::default, border::width(0)))
					.width(Fill),
					container(rule::horizontal(1)).padding([5, 0]),
					styled_scrollable(
						dragking::column(
							self.arrangement
								.node(selected)
								.plugins
								.iter()
								.enumerate()
								.map(|(i, plugin)| {
									row![
										Knob::new(0.0..=1.0, plugin.mix, move |mix| {
											Message::PluginMixChanged(selected, i, mix)
										})
										.radius(TEXT_HEIGHT)
										.enabled(plugin.enabled)
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
												circle_off(),
												if plugin.enabled {
													button::primary
												} else {
													button::secondary
												}
											)
											.on_press(Message::PluginToggleEnabled(selected, i)),
											icon_button(
												x(),
												if plugin.enabled {
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
								})
						)
						.spacing(5)
						.on_drag(Message::PluginMoveTo.with(selected)),
					)
					.height(Fill)
				],
				self.split_at,
				Message::SplitAt,
			)
			.strategy(Strategy::End)
			.on_double_click(Message::SplitAt(DEFAULT_SPLIT_POSITION))
			.into()
		} else {
			mixer_panel.into()
		}
	}

	fn channel<'a>(
		&'a self,
		node: &'a Node,
		name: impl text::IntoFragment<'a>,
	) -> Element<'a, Message> {
		let button_style = |cond: bool| {
			if !node.flags.contains(Flags::ENABLED) {
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
						if node.flags.contains(Flags::ENABLED) {
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
						circle_off(),
						button_style(node.flags.contains(Flags::BYPASSED))
					)
					.on_press(Message::ChannelToggleBypassed(node.id)),
					icon_button(
						arrow_up_down(),
						button_style(node.flags.contains(Flags::POLARITY_INVERTED))
					)
					.on_press(Message::ChannelInvertPolarity(node.id)),
					node.pan_switcher()
				]
				.spacing(5),
				container(text(format_decibels(node.volume)).line_height(1.0))
					.style(bordered_box_with_radius(0))
					.center_x(55)
					.padding(2),
				row![
					PeakMeter::new(&node.peaks[1][0], node.flags.contains(Flags::ENABLED))
						.width(16.0),
					vertical_slider(0.0..=1.0, node.volume.cbrt(), |v| {
						Message::ChannelVolumeChanged(node.id, v.powi(3))
					})
					.width(17)
					.step(f32::EPSILON)
					.style(if node.flags.contains(Flags::ENABLED) {
						slider::default
					} else {
						slider_secondary
					}),
					PeakMeter::new(&node.peaks[1][1], node.flags.contains(Flags::ENABLED))
						.width(16.0),
				]
				.spacing(3),
				self.selected_channel.map_or_else(
					|| Element::new(space().height(LINE_HEIGHT)),
					|selected_channel| {
						if node.ty == NodeType::Track
							|| node.id == selected_channel
							|| self.arrangement.master().id == selected_channel
						{
							space().height(LINE_HEIGHT).into()
						} else {
							let connected = self
								.arrangement
								.outgoing(selected_channel)
								.contains(*node.id);

							button(chevron_up())
								.style(if node.flags.contains(Flags::ENABLED) && connected {
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
					},
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

	fn piano_roll<'a>(&'a self, clip: &'a MidiClip) -> Element<'a, Message> {
		Seeker::new(
			self.arrangement.rtstate(),
			&self.piano_roll_position,
			&self.piano_roll_scale,
			Piano::new(&self.piano_roll_position, &self.piano_roll_scale),
			PianoRoll::new(
				&self.arrangement.patterns()[*clip.pattern].notes,
				self.arrangement.rtstate(),
				&self.piano_roll_position,
				&self.piano_roll_scale,
				Message::PianoRollAction,
			),
			Message::SeekTo,
			Message::SetLoopMarker,
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
	let file_name = format!("recording-{}.wav", format_rfc3339(SystemTime::now()));

	let data_dir = dirs::data_dir().unwrap().join("Generic DAW");
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

fn poll_consumer<T: Send + 'static>(
	mut consumer: Consumer<T>,
	sample_rate: u32,
	frames: u32,
) -> Task<T> {
	let wait = 1_000_000 / sample_rate.div_ceil(frames).min(1_000);
	let mut backoff = 500;
	let mut backoff = move |reset| {
		Timer::after(Duration::from_micros(u64::from(if reset {
			backoff = wait.min(backoff * 2);
			backoff
		} else {
			backoff = 500;
			wait
		})))
	};

	Task::stream(stream::channel(
		consumer.buffer().capacity(),
		async move |mut sender| {
			loop {
				let mut timer = backoff(true);
				if consumer.is_abandoned() {
					atomic::fence(Acquire);
				}
				while let Ok(t) = consumer.pop() {
					timer = backoff(false);
					if sender.send(t).await.is_err() {
						break;
					}
				}
				if consumer.is_abandoned() {
					break;
				}
				timer.await;
			}
		},
	))
}
