use crate::{
	action::Action,
	clap_host,
	components::{icon_button, text_icon_button},
	config::Config,
	daw::{self, RECORDINGS_DIR, format_now},
	file_tree::FileKind,
	icons::{
		arrow_left_right, arrow_up_down, chevron_down, chevron_up, grip_vertical, mic, plus, power,
		power_off, radius, x,
	},
	operation::scroll_into_view,
	state::{DEFAULT_SPLIT_POSITION, State},
	stylefns::{
		button_with_radius, container_with_radius, menu_style, scrollable_style, slider_secondary,
		slider_with_radius, split_style, sweeten_column_style, weak_bordered_box,
		weakest_bordered_box,
	},
	widget::{
		Delta, LINE_HEIGHT, TEXT_HEIGHT,
		clip::Clip,
		note::Note,
		piano::Piano,
		piano_roll::{self, PianoRoll},
		playlist::{self, Playlist},
		seeker::Seeker,
		snap_step,
		track::Track,
	},
};
use audio_clip::AudioClip;
use generic_daw_core::{
	Batch, MidiKey, MidiNote, MidiNoteId, MidiPatternId, NodeId, PanMode, PluginId, SampleId,
	clap_host::PluginDescriptor,
	time::{BeatRange, BeatTime},
};
use generic_daw_widget::{
	knob::Knob,
	peak_meter::{MAX_VOL, PeakMeter},
};
use iced::{
	Center, Element, Fill, Subscription, Task, Vector, border,
	futures::SinkExt as _,
	keyboard,
	mouse::Interaction,
	padding, stream,
	time::every,
	widget::{
		button, center_x, column, combo_box, container, mouse_area, opaque, operation::snap_to_end,
		row, rule, scrollable, slider, space, text, vertical_slider,
	},
};
use iced_split::{Split, Strategy};
use midi_clip::MidiClip;
use midi_pattern::MidiPatternPair;
use node::{Node, NodeType};
use rtrb::Consumer;
use sample::SamplePair;
use smol::{Timer, stream::Stream, unblock};
use std::{
	cell::RefCell,
	cmp::{Ordering, Reverse},
	collections::HashMap,
	io::Read,
	iter::{once, repeat},
	num::NonZero,
	path::Path,
	sync::Arc,
	time::Duration,
};
use sweeten::widget::drag::DragEvent;
use utils::{NoClone, NoDebug};

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

#[derive(Clone, Debug)]
pub enum Message {
	Batch(Batch),
	UpdateRequest,

	Connect(NodeId, NodeId),
	SetMix(NodeId, NodeId, f32),
	Disconnect(NodeId, NodeId),

	CycleTabForwards,
	CycleTabBackwards,
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
	PluginShow(PluginId),
	PluginMixChanged(NodeId, usize, f32),
	PluginToggleEnabled(NodeId, usize),
	PluginMoveTo(NodeId, DragEvent),
	PluginRemove(NodeId, usize),

	SampleLoaded(NoClone<Option<(Box<SamplePair>, Option<usize>, BeatTime)>>),
	MidiPatternLoaded(NoClone<Option<(Box<MidiPatternPair>, Option<usize>, BeatTime)>>),
	AddAudioClip(SampleId, Option<usize>, BeatTime),
	AddMidiClip(MidiPatternId, Option<usize>, BeatTime),

	TrackAdd,
	TrackRemove(NodeId),
	TrackToggleEnabled(NodeId),
	TrackToggleSolo(NodeId),

	SeekTo(BeatTime),
	SetLoopMarker(Option<BeatRange>),

	Recording(NodeId),
	RecordingFinalize,
	RecordingWrite(NoDebug<Box<[f32]>>),

	PlaylistAction(playlist::Action),
	PianoRollAction(piano_roll::Action),

	ArrowUp,
	ArrowDown,
	ArrowLeft,
	ArrowRight,
	TransposeOctUp,
	TransposeOctDown,
	Quantize,
	SelectAll,
	SelectInverse,
	UnselectAll,
	Duplicate,
	Delete,

	OnDrag(f32),
	OnDragEnd,
	OnDoubleClick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
	Playlist,
	Mixer,
	PianoRoll,
}

#[derive(Debug)]
pub struct ArrangementView {
	pub arrangement: Arrangement,

	tab: Tab,
	midi_clip: Option<(usize, usize)>,
	recording: Option<Recording>,

	playlist: RefCell<playlist::State>,
	piano_roll: RefCell<piano_roll::State>,

	soloed: Option<NodeId>,
	selected: NodeId,

	loading: usize,
}

impl ArrangementView {
	pub fn create(config: &Config, state: &State) -> (Self, Task<Message>) {
		let (arrangement, batches) = Arrangement::create(config);
		(Self::new(arrangement, state), batches.map(Message::Batch))
	}

	pub fn new(mut arrangement: Arrangement, state: &State) -> Self {
		if state.metronome {
			arrangement.toggle_metronome();
		}

		let playlist_scale_x = (arrangement.transport().sample_rate.get() as f32).log2() - 5.0;
		let piano_roll_scale_x = playlist_scale_x - 2.0;

		let selected = arrangement.master().id;

		Self {
			arrangement,

			tab: Tab::Playlist,
			midi_clip: None,
			recording: None,

			playlist: RefCell::new(playlist::State::new(
				Vector::default(),
				Vector::new(playlist_scale_x, 87.0),
			)),
			piano_roll: RefCell::new(piano_roll::State::new(
				Vector::new(0.0, 1000.0),
				Vector::new(piano_roll_scale_x, LINE_HEIGHT),
			)),

			soloed: None,
			selected,

			loading: 0,
		}
	}

	pub fn update(
		&mut self,
		message: Message,
		config: &Config,
		state: &mut State,
	) -> Action<daw::Instruction, Message> {
		match message {
			Message::Batch(msg) => {
				let before = self.arrangement.transport().position;

				let action = Action::batch(
					self.arrangement
						.update(msg)
						.into_iter()
						.map(daw::Message::ClapHost)
						.map(daw::Instruction::Message)
						.map(Action::instruction),
				);

				let after = self.arrangement.transport().position;

				if state.autoscroll && after != before {
					match self.tab {
						Tab::Playlist => {
							let pos_diff = Vector::new(
								(after.to_samples(self.arrangement.transport()) as f32
									- before.to_samples(self.arrangement.transport()) as f32)
									/ self.playlist.get_mut().scale.x.exp2(),
								0.0,
							);
							return Action::batch([
								action,
								self.handle_playlist_action(
									playlist::Action::Pan(pos_diff, 0.0),
									config,
									state,
								),
							]);
						}
						Tab::Mixer => {}
						Tab::PianoRoll => {
							let pos_diff = Vector::new(
								(after.to_samples(self.arrangement.transport()) as f32
									- before.to_samples(self.arrangement.transport()) as f32)
									/ self.piano_roll.get_mut().scale.x.exp2(),
								0.0,
							);
							self.handle_piano_roll_action(piano_roll::Action::Pan(
								pos_diff, 0.0, 0.0,
							));
						}
					}
				}

				return action;
			}
			Message::UpdateRequest => self.arrangement.request_update(),
			Message::Connect(from, to) => self.arrangement.connect(from, to),
			Message::SetMix(from, to, mix) => self.arrangement.set_mix(from, to, mix),
			Message::Disconnect(from, to) => self.arrangement.disconnect(from, to),
			Message::CycleTabForwards => {
				return self.update(
					Message::ChangedTab(match self.tab {
						Tab::Playlist => Tab::Mixer,
						Tab::Mixer if self.midi_clip.is_some() => Tab::PianoRoll,
						Tab::Mixer | Tab::PianoRoll => Tab::Playlist,
					}),
					config,
					state,
				);
			}
			Message::CycleTabBackwards => {
				return self.update(
					Message::ChangedTab(match self.tab {
						Tab::Mixer => Tab::Playlist,
						Tab::Playlist if self.midi_clip.is_some() => Tab::PianoRoll,
						Tab::Playlist | Tab::PianoRoll => Tab::Mixer,
					}),
					config,
					state,
				);
			}
			Message::ChangedTab(tab) => {
				match self.tab {
					Tab::Playlist => self.playlist.get_mut().finish(),
					Tab::Mixer => {}
					Tab::PianoRoll => self.piano_roll.get_mut().finish(),
				}

				self.tab = tab;
			}
			Message::ChannelAdd => {
				self.selected = self.arrangement.add_channel();
				return Action::batch([
					self.update(
						Message::Connect(self.selected, self.arrangement.master().id),
						config,
						state,
					),
					snap_to_end("mixer").into(),
				]);
			}
			Message::ChannelRemove(node) => {
				if self.selected == node {
					self.select_next();
					if self.selected == node {
						self.select_prev();
					}
				}

				self.arrangement.remove_channel(node);
			}
			Message::ChannelSelect(node) => {
				self.selected = node;
				if self.tab == Tab::Mixer {
					return scroll_into_view(
						"mixer",
						self.arrangement.node(self.selected).widget_id.clone(),
					)
					.into();
				}
			}
			Message::ChannelVolumeChanged(node, mut volume) => {
				let db = amp_to_db(volume.abs());
				let nearest = (db / 6.0).round() * 6.0;
				if (db - nearest).abs() < 0.15 {
					volume = db_to_amp(nearest).copysign(volume);
				} else {
					let nearest = (db / 3.0).round() * 3.0;
					if (db - nearest).abs() < 0.05 {
						volume = db_to_amp(nearest).copysign(volume);
					}
				}

				self.arrangement.channel_volume_changed(node, volume);
			}
			Message::ChannelPanChanged(node, mut pan) => {
				let snap = |pan: &mut f32| {
					if pan.abs() < 0.015 {
						*pan = 0.0;
					}
				};

				match &mut pan {
					PanMode::Balance(pan) => snap(pan),
					PanMode::Stereo(l, r) => {
						snap(l);
						snap(r);
					}
				}

				self.arrangement.channel_pan_changed(node, pan);
			}
			Message::ChannelToggleEnabled(node) => self.arrangement.channel_toggle_enabled(node),
			Message::ChannelToggleBypassed(node) => self.arrangement.channel_toggle_bypassed(node),
			Message::PluginLoad(node, descriptor, show) => {
				let (plugin, instruction) = self.arrangement.plugin_load(node, descriptor);
				let mut action = Action::instruction(instruction);
				if show {
					action = Action::batch([
						action,
						Action::instruction(daw::Instruction::Message(daw::Message::ClapHost(
							clap_host::Message::GuiOpen(plugin),
						))),
					]);
				}
				return action;
			}
			Message::PluginSetState(node, i, state) => {
				let plugin = self.arrangement.node(node).plugins[i].id;
				return Action::instruction(daw::Instruction::PluginSetState(plugin, state));
			}
			Message::PluginShow(plugin) => {
				return Action::instruction(daw::Instruction::Message(daw::Message::ClapHost(
					clap_host::Message::GuiOpen(plugin),
				)));
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
			Message::PluginRemove(node, i) => self.arrangement.plugin_remove(node, i),
			Message::SampleLoaded(NoClone(loaded)) => {
				self.loading -= 1;

				if let Some((sample, track, pos)) = loaded {
					let id = sample.gui.id;
					self.arrangement.add_sample(*sample);
					return self.update(Message::AddAudioClip(id, track, pos), config, state);
				}
			}
			Message::MidiPatternLoaded(NoClone(loaded)) => {
				self.loading -= 1;

				if let Some((pattern, track, pos)) = loaded {
					let id = pattern.gui.id;
					self.arrangement.add_midi_pattern(*pattern);
					return self.update(Message::AddMidiClip(id, track, pos), config, state);
				}
			}
			Message::AddAudioClip(id, track, pos) => {
				let mut clip = AudioClip::new(id);
				clip.position.trim_end_to(
					self.arrangement.samples()[&id].len(self.arrangement.transport()),
					self.arrangement.transport(),
				);
				clip.position.move_to(pos);
				let (track, task) = if let Some(track) = track {
					(track, Action::none())
				} else {
					(
						self.arrangement.tracks().len(),
						self.update(Message::TrackAdd, config, state),
					)
				};
				let clip = self.arrangement.add_clip(track, clip);
				self.playlist.get_mut().primary.insert((track, clip));
				return task;
			}
			Message::AddMidiClip(id, track, pos) => {
				let mut clip = MidiClip::new(id);
				clip.position
					.trim_end_to(
						self.arrangement.midi_patterns()[&id]
							.len()
							.max(BeatTime::new(
								u64::from(self.arrangement.transport().numerator.get()),
								0,
							)),
					);
				clip.position.move_to(pos);
				let (track, task) = if let Some(track) = track {
					(track, Action::none())
				} else {
					(
						self.arrangement.tracks().len(),
						self.update(Message::TrackAdd, config, state),
					)
				};
				let clip = self.arrangement.add_clip(track, clip);
				self.playlist.get_mut().primary.insert((track, clip));
				return task;
			}
			Message::TrackAdd => {
				let track = self.arrangement.add_track();
				let node = self.arrangement.tracks()[track].id;
				if self.soloed.is_some() {
					self.arrangement.channel_toggle_enabled(node);
				}
				return self.update(
					Message::Connect(node, self.arrangement.master().id),
					config,
					state,
				);
			}
			Message::TrackRemove(node) => {
				if self.soloed == Some(node) {
					self.soloed = None;
				}

				if self.selected == node {
					self.select_next();
					if self.selected == node {
						self.select_prev();
					}
				}

				let track = self.arrangement.track_of(node).unwrap();
				self.arrangement.remove_track(node);

				self.midi_clip = self
					.midi_clip
					.and_then(|midi_clip| update_selection(midi_clip, track, None));

				let playlist = self.playlist.get_mut();
				playlist.primary = playlist
					.primary
					.drain()
					.filter_map(|clip| update_selection(clip, track, None))
					.collect();

				if self
					.recording
					.as_ref()
					.is_some_and(|recording| recording.node == node)
				{
					self.end_recording();
				}
			}
			Message::TrackToggleEnabled(node) => {
				self.soloed = None;
				return self.update(Message::ChannelToggleEnabled(node), config, state);
			}
			Message::TrackToggleSolo(node) => {
				if self.soloed == Some(node) {
					self.soloed = None;
					self.arrangement.enable_all_tracks();
				} else {
					self.soloed = Some(node);
					self.arrangement.solo_track(node);
				}
			}
			Message::SeekTo(pos) => {
				self.arrangement.seek_to(pos);
				self.end_recording();
			}
			Message::SetLoopMarker(marker) => self.arrangement.set_loop_range(marker),
			Message::Recording(node) => {
				let path = RECORDINGS_DIR.join(format!("{}.wav", format_now())).into();

				if let Some(recording) = &mut self.recording {
					if node == recording.node {
						self.end_recording();
					} else {
						let pos = recording.position;

						let sample = recording.split_off(path, self.arrangement.transport());
						let id = sample.gui.id;
						self.arrangement.add_sample(sample);

						let track = self.arrangement.track_of(recording.node).unwrap();

						let mut clip = AudioClip::new(id);
						clip.position.trim_end_to(
							self.arrangement.samples()[&id].len(self.arrangement.transport()),
							self.arrangement.transport(),
						);
						clip.position.move_to(pos);
						self.arrangement.add_clip(track, clip);

						recording.node = node;
					}
				} else {
					let (recording, task) = Recording::create(
						path,
						self.arrangement.transport(),
						config.input_device.id.as_ref(),
						config.input_device.sample_rate,
						config.input_device.buffer_size,
						node,
					);

					let sample_rate = recording.core.sample_rate();
					let frames = recording.core.frames().or(config.input_device.buffer_size);

					self.recording = Some(recording);
					self.arrangement.play();

					return Task::run(poll_consumer(task, sample_rate, frames), |samples| {
						Message::RecordingWrite(NoDebug(samples))
					})
					.chain(Task::done(Message::RecordingFinalize))
					.into();
				}
			}
			Message::RecordingFinalize => {
				let recording = self.recording.take().unwrap();
				let pos = recording.position;
				let node = recording.node;

				let sample = recording.finalize();

				if let Some(track) = self.arrangement.track_of(node) {
					let id = sample.gui.id;
					self.arrangement.add_sample(sample);

					let mut clip = AudioClip::new(id);
					clip.position.trim_end_to(
						self.arrangement.samples()[&id].len(self.arrangement.transport()),
						self.arrangement.transport(),
					);
					clip.position.move_to(pos);
					self.arrangement.add_clip(track, clip);
				}
			}
			Message::RecordingWrite(samples) => self.recording.as_mut().unwrap().write(&samples),
			Message::PlaylistAction(action) => {
				return self.handle_playlist_action(action, config, state);
			}
			Message::PianoRollAction(action) => self.handle_piano_roll_action(action),
			Message::ArrowUp => match self.tab {
				Tab::Playlist => {
					self.playlist.get_mut().finish();
					return self.handle_playlist_action(
						playlist::Action::Drag(Delta::Negative(1), Delta::Positive(BeatTime::ZERO)),
						config,
						state,
					);
				}
				Tab::Mixer => {}
				Tab::PianoRoll => {
					self.piano_roll.get_mut().finish();
					self.handle_piano_roll_action(piano_roll::Action::Drag(
						Delta::Positive(MidiKey(1)),
						Delta::Positive(BeatTime::ZERO),
					));
				}
			},
			Message::ArrowDown => match self.tab {
				Tab::Playlist => {
					self.playlist.get_mut().finish();
					return self.handle_playlist_action(
						playlist::Action::Drag(Delta::Positive(1), Delta::Positive(BeatTime::ZERO)),
						config,
						state,
					);
				}
				Tab::Mixer => {}
				Tab::PianoRoll => {
					self.piano_roll.get_mut().finish();
					self.handle_piano_roll_action(piano_roll::Action::Drag(
						Delta::Negative(MidiKey(1)),
						Delta::Positive(BeatTime::ZERO),
					));
				}
			},
			Message::ArrowLeft => match self.tab {
				Tab::Playlist => {
					self.playlist.get_mut().finish();
					let snap_step = snap_step(
						self.playlist.get_mut().scale.x,
						self.arrangement.transport(),
					);
					return self.handle_playlist_action(
						playlist::Action::Drag(Delta::Positive(0), Delta::Negative(snap_step)),
						config,
						state,
					);
				}
				Tab::Mixer => {
					self.select_prev();
					return scroll_into_view(
						"mixer",
						self.arrangement.node(self.selected).widget_id.clone(),
					)
					.into();
				}
				Tab::PianoRoll => {
					self.piano_roll.get_mut().finish();
					let snap_step = snap_step(
						self.playlist.get_mut().scale.x,
						self.arrangement.transport(),
					);
					self.handle_piano_roll_action(piano_roll::Action::Drag(
						Delta::Positive(MidiKey(0)),
						Delta::Negative(snap_step),
					));
				}
			},
			Message::ArrowRight => match self.tab {
				Tab::Playlist => {
					self.playlist.get_mut().finish();
					let snap_step = snap_step(
						self.playlist.get_mut().scale.x,
						self.arrangement.transport(),
					);
					return self.handle_playlist_action(
						playlist::Action::Drag(Delta::Positive(0), Delta::Positive(snap_step)),
						config,
						state,
					);
				}
				Tab::Mixer => {
					self.select_next();
					return scroll_into_view(
						"mixer",
						self.arrangement.node(self.selected).widget_id.clone(),
					)
					.into();
				}
				Tab::PianoRoll => {
					self.piano_roll.get_mut().finish();
					let snap_step = snap_step(
						self.playlist.get_mut().scale.x,
						self.arrangement.transport(),
					);
					self.handle_piano_roll_action(piano_roll::Action::Drag(
						Delta::Positive(MidiKey(0)),
						Delta::Positive(snap_step),
					));
				}
			},
			Message::TransposeOctUp => match self.tab {
				Tab::Playlist | Tab::Mixer => {}
				Tab::PianoRoll => {
					self.piano_roll.get_mut().finish();
					self.handle_piano_roll_action(piano_roll::Action::Drag(
						Delta::Positive(MidiKey(12)),
						Delta::Positive(BeatTime::ZERO),
					));
				}
			},
			Message::TransposeOctDown => match self.tab {
				Tab::Playlist | Tab::Mixer => {}
				Tab::PianoRoll => {
					self.piano_roll.get_mut().finish();
					self.handle_piano_roll_action(piano_roll::Action::Drag(
						Delta::Negative(MidiKey(12)),
						Delta::Positive(BeatTime::ZERO),
					));
				}
			},
			Message::Quantize => match self.tab {
				Tab::Playlist => {
					let playlist = self.playlist.get_mut();
					playlist.finish();
					for &(track, clip) in &playlist.primary {
						let pos = BeatRange::new(
							self.arrangement.tracks()[track].clips[clip].start(),
							self.arrangement.tracks()[track].clips[clip]
								.end(self.arrangement.transport()),
						)
						.round(snap_step(playlist.scale.x, self.arrangement.transport()));

						self.arrangement.clip_trim_end_to(track, clip, pos.end());
						self.arrangement
							.clip_trim_start_to(track, clip, pos.start());
					}
				}
				Tab::Mixer => {}
				Tab::PianoRoll => {
					let clip = self.midi_clip().unwrap();
					let piano_roll = self.piano_roll.get_mut();
					piano_roll.finish();
					for &note in &piano_roll.primary {
						let pos = self.arrangement.midi_patterns()[&clip.pattern].notes[note]
							.position
							.round(snap_step(piano_roll.scale.x, self.arrangement.transport()));

						self.arrangement
							.note_trim_end_to(clip.pattern, note, pos.end());
						self.arrangement
							.note_trim_start_to(clip.pattern, note, pos.start());
					}
				}
			},
			Message::SelectAll => match self.tab {
				Tab::Playlist => {
					self.playlist.get_mut().clear();
					self.playlist.get_mut().primary.extend(
						self.arrangement
							.tracks()
							.iter()
							.enumerate()
							.flat_map(|(t_idx, track)| repeat(t_idx).zip(0..track.clips.len())),
					);
				}
				Tab::Mixer => {}
				Tab::PianoRoll => {
					let clip = self.midi_clip().unwrap();
					self.piano_roll.get_mut().clear();
					self.piano_roll
						.get_mut()
						.primary
						.extend(0..self.arrangement.midi_patterns()[&clip.pattern].notes.len());
				}
			},
			Message::SelectInverse => match self.tab {
				Tab::Playlist => {
					self.playlist.get_mut().finish();
					for (t_idx, track) in self.arrangement.tracks().iter().enumerate() {
						for c_idx in 0..track.clips.len() {
							if !self.playlist.get_mut().primary.insert((t_idx, c_idx)) {
								self.playlist.get_mut().primary.remove(&(t_idx, c_idx));
							}
						}
					}
				}
				Tab::Mixer => {}
				Tab::PianoRoll => {
					let clip = self.midi_clip().unwrap();
					self.piano_roll.get_mut().finish();
					for note in 0..self.arrangement.midi_patterns()[&clip.pattern].notes.len() {
						if !self.piano_roll.get_mut().primary.insert(note) {
							self.piano_roll.get_mut().primary.remove(&note);
						}
					}
				}
			},
			Message::UnselectAll => match self.tab {
				Tab::Playlist => self.playlist.get_mut().clear(),
				Tab::Mixer => {}
				Tab::PianoRoll => self.piano_roll.get_mut().clear(),
			},
			Message::Duplicate => match self.tab {
				Tab::Playlist => {
					if let Some(delta) = self
						.playlist
						.get_mut()
						.primary
						.iter()
						.map(|&(track, clip)| {
							(
								self.arrangement.tracks()[track].clips[clip].start(),
								self.arrangement.tracks()[track].clips[clip]
									.end(self.arrangement.transport()),
							)
						})
						.reduce(|old, new| (old.0.min(new.0), old.1.max(new.1)))
					{
						self.playlist.get_mut().finish();
						return Action::batch([
							self.handle_playlist_action(playlist::Action::Clone, config, state),
							self.handle_playlist_action(
								playlist::Action::Drag(
									Delta::Positive(0),
									Delta::Positive(delta.1 - delta.0),
								),
								config,
								state,
							),
						]);
					}
				}
				Tab::Mixer => {}
				Tab::PianoRoll => {
					let clip = self.midi_clip().unwrap();
					if let Some(delta) = self
						.piano_roll
						.get_mut()
						.primary
						.iter()
						.map(|&note| {
							self.arrangement.midi_patterns()[&clip.pattern].notes[note].position
						})
						.reduce(|old, new| {
							BeatRange::new(old.start().min(new.start()), old.end().max(new.end()))
						}) {
						self.piano_roll.get_mut().finish();
						self.handle_piano_roll_action(piano_roll::Action::Clone);
						self.handle_piano_roll_action(piano_roll::Action::Drag(
							Delta::Positive(MidiKey(0)),
							Delta::Positive(delta.len()),
						));
					}
				}
			},
			Message::Delete => match self.tab {
				Tab::Playlist => {
					self.playlist.get_mut().finish();
					return self.handle_playlist_action(playlist::Action::Delete, config, state);
				}
				Tab::Mixer => match self.arrangement.node(self.selected).ty {
					NodeType::Master => {}
					NodeType::Channel => {
						return self.update(Message::ChannelRemove(self.selected), config, state);
					}
					NodeType::Track => {
						return self.update(Message::TrackRemove(self.selected), config, state);
					}
				},
				Tab::PianoRoll => {
					self.piano_roll.get_mut().finish();
					self.handle_piano_roll_action(piano_roll::Action::Delete);
				}
			},
			Message::OnDrag(split_at) => {
				state.plugins_panel_split_at = split_at.clamp(200.0, 400.0);
			}
			Message::OnDragEnd => state.write(),
			Message::OnDoubleClick => {
				return Action::batch([
					self.update(Message::OnDrag(DEFAULT_SPLIT_POSITION), config, state),
					self.update(Message::OnDragEnd, config, state),
				]);
			}
		}

		Action::none()
	}

	fn handle_playlist_action(
		&mut self,
		action: playlist::Action,
		config: &Config,
		state: &mut State,
	) -> Action<daw::Instruction, Message> {
		let playlist::State {
			primary,
			secondary,
			position,
			scale,
			..
		} = self.playlist.get_mut();

		match action {
			playlist::Action::Pan(pos_diff, visible) => {
				let old_position = *position;
				*position += pos_diff;
				position.x = position.x.max(0.0);
				position.y = position.y.clamp(0.0, old_position.y + visible);
			}
			playlist::Action::Zoom(scale_diff, cursor, visible) => {
				let old_scale = *scale;
				*scale += scale_diff;
				scale.x = scale.x.clamp(
					1.0,
					(self.arrangement.transport().sample_rate.get() as f32).log2(),
				);
				scale.y = scale.y.clamp(46.0, 200.0);

				let pos_diff = Vector::new(
					(cursor.x + position.x) * ((old_scale.x - scale.x).exp2() - 1.0),
					(cursor.y + position.y) * ((scale.y / old_scale.y) - 1.0),
				);

				return self.handle_playlist_action(
					playlist::Action::Pan(pos_diff, visible),
					config,
					state,
				);
			}
			playlist::Action::Add(info, track, pos) => {
				return if let Some((path, kind)) = info {
					if kind == FileKind::Midi {
						self.loading += 1;
						let transport = *self.arrangement.transport();
						Task::future(unblock(move || {
							Message::MidiPatternLoaded(
								MidiPatternPair::from_midi(path, &transport)
									.map(|pair| (Box::new(pair), track, pos))
									.into(),
							)
						}))
						.into()
					} else if let Some(sample) = self
						.arrangement
						.samples()
						.values()
						.find(|sample| sample.path == path)
					{
						self.update(Message::AddAudioClip(sample.id, track, pos), config, state)
					} else {
						self.loading += 1;
						Task::future(unblock(move || {
							Message::SampleLoaded(
								SamplePair::new(path)
									.map(|pair| (Box::new(pair), track, pos))
									.into(),
							)
						}))
						.into()
					}
				} else {
					self.loading += 1;
					self.update(
						Message::MidiPatternLoaded(NoClone(Some((
							Box::new(MidiPatternPair::from_notes(Vec::new(), "MIDI Pattern")),
							track,
							pos,
						)))),
						config,
						state,
					)
				};
			}
			playlist::Action::Open(track, clip) => {
				if self.midi_clip != Some((track, clip)) {
					self.midi_clip = Some((track, clip));
					self.piano_roll.get_mut().clear();
				}

				return self.update(Message::ChangedTab(Tab::PianoRoll), config, state);
			}
			playlist::Action::Clone => {
				let mut sorted = primary.drain().collect::<Vec<_>>();
				sorted.sort_unstable_by_key(|&(_, c)| c);
				for (track, clip) in sorted {
					primary.insert((track, self.arrangement.duplicate_clip(track, clip)));
				}
			}
			playlist::Action::Drag(track_diff, pos_diff) => {
				let mut sorted = primary.drain().collect::<Vec<_>>();
				match track_diff {
					Delta::Positive(0) | Delta::Negative(0) => {}
					Delta::Negative(..) => sorted.sort_unstable_by_key(|&(t, c)| (t, Reverse(c))),
					Delta::Positive(..) => {
						sorted.sort_unstable_by_key(|&(t, c)| (Reverse(t), Reverse(c)));
					}
				}

				for (track, clip) in sorted {
					let pos = self.arrangement.tracks()[track].clips[clip].start();
					self.arrangement.clip_move_to(track, clip, pos + pos_diff);

					let new_track = (track + track_diff).min(self.arrangement.tracks().len() - 1);
					let new_clip = self.arrangement.clip_switch_track(track, clip, new_track);

					primary.insert((new_track, new_clip));
				}
			}
			playlist::Action::TrimStart(pos_diff) => {
				for &(track, clip) in &*primary {
					let pos = self.arrangement.tracks()[track].clips[clip].start();
					self.arrangement
						.clip_trim_start_to(track, clip, pos + pos_diff);
				}
			}
			playlist::Action::TrimEnd(pos_diff) => {
				for &(track, clip) in &*primary {
					let pos = self.arrangement.tracks()[track].clips[clip]
						.end(self.arrangement.transport());
					self.arrangement
						.clip_trim_end_to(track, clip, pos + pos_diff);
				}
			}
			playlist::Action::StretchStart(pos_diff) => {
				for &(track, clip) in &*primary {
					let pos = self.arrangement.tracks()[track].clips[clip].start();
					self.arrangement
						.clip_stretch_start_to(track, clip, pos + pos_diff);
				}
			}
			playlist::Action::StretchEnd(pos_diff) => {
				for &(track, clip) in &*primary {
					let pos = self.arrangement.tracks()[track].clips[clip]
						.end(self.arrangement.transport());
					self.arrangement
						.clip_stretch_end_to(track, clip, pos + pos_diff);
				}
			}
			playlist::Action::SplitAt(mut pos) => {
				let mut extra = HashMap::<_, usize>::new();

				let mut sorted = primary
					.drain()
					.filter(|&(track, clip)| {
						(self.arrangement.tracks()[track].clips[clip].start()
							..=self.arrangement.tracks()[track].clips[clip]
								.end(self.arrangement.transport()))
							.contains(&pos)
					})
					.collect::<Vec<_>>();
				sorted.sort_unstable_by_key(|&(_, c)| c);

				for (track, mut lhs) in sorted {
					let extra = extra.entry(track).or_default();
					lhs += *extra;

					let clip = self.arrangement.tracks()[track].clips[lhs];
					if clip.end(self.arrangement.transport()) == pos {
						primary.insert((track, lhs));
					} else if clip.start() == pos {
						secondary.insert((track, lhs));
					} else if clip.len(self.arrangement.transport()) > BeatTime::TICK {
						let start = clip.start() + BeatTime::TICK;
						let end = clip.end(self.arrangement.transport()) - BeatTime::TICK;
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
					let new = self.arrangement.tracks()[track].clips[lhs].start() + BeatTime::TICK;

					clamped
						.entry(track)
						.and_modify(|old| *old = new.max(*old))
						.or_insert_with(|| new.max(pos));
				}

				for &(track, rhs) in &*secondary {
					let new = self.arrangement.tracks()[track].clips[rhs]
						.end(self.arrangement.transport())
						- BeatTime::TICK;

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
			playlist::Action::DragSlip(pos_diff) => {
				for &(track, clip) in &*primary {
					let pos = self.arrangement.tracks()[track].clips[clip]
						.offset(self.arrangement.transport());
					self.arrangement.clip_slip_to(track, clip, pos + pos_diff);
				}
			}
			playlist::Action::Delete => {
				let mut sorted = primary.drain().collect::<Vec<_>>();
				sorted.sort_unstable_by_key(|&(_, c)| Reverse(c));
				for (track, clip) in sorted {
					self.midi_clip = self
						.midi_clip
						.and_then(|midi_clip| update_selection(midi_clip, track, Some(clip)));

					let clip = self.arrangement.remove_clip(track, clip);
					self.arrangement.gc(clip);
				}
			}
		}

		Action::none()
	}

	fn handle_piano_roll_action(&mut self, action: piano_roll::Action) {
		let clip = self.midi_clip().unwrap();

		let piano_roll::State {
			primary,
			secondary,
			position,
			scale,
			..
		} = self.piano_roll.get_mut();

		match action {
			piano_roll::Action::Pan(pos_diff, height, visible) => {
				let old_position = *position;
				*position += pos_diff;
				position.x = position.x.max(0.0);
				position.y = position.y.clamp(0.0, old_position.y + visible - height);
			}
			piano_roll::Action::Zoom(scale_diff, cursor, height, visible) => {
				let old_scale = *scale;
				*scale += scale_diff;
				scale.x = scale.x.clamp(
					1.0,
					(self.arrangement.transport().sample_rate.get() as f32).log2(),
				);
				scale.y = scale.y.clamp(LINE_HEIGHT, 2.0 * LINE_HEIGHT);

				let pos_diff = Vector::new(
					(cursor.x + position.x) * ((old_scale.x - scale.x).exp2() - 1.0),
					(cursor.y + position.y) * ((scale.y / old_scale.y) - 1.0),
				);

				self.handle_piano_roll_action(piano_roll::Action::Pan(pos_diff, height, visible));
			}
			piano_roll::Action::Add(key, pos) => {
				let note = self.arrangement.add_note(
					clip.pattern,
					MidiNote {
						key,
						velocity: 1.0,
						position: BeatRange::new(pos, pos + BeatTime::new(1, 0)),
						id: MidiNoteId::unique(),
					},
				);
				primary.insert(note);
			}
			piano_roll::Action::Clone => {
				let mut sorted = primary.drain().collect::<Vec<_>>();
				sorted.sort_unstable();
				for note in sorted {
					primary.insert(self.arrangement.duplicate_note(clip.pattern, note));
				}
			}
			piano_roll::Action::Drag(key_diff, pos_diff) => {
				for &note in &*primary {
					let pos = self.arrangement.midi_patterns()[&clip.pattern].notes[note]
						.position
						.start();
					self.arrangement
						.note_move_to(clip.pattern, note, pos + pos_diff);

					let key = self.arrangement.midi_patterns()[&clip.pattern].notes[note].key;
					self.arrangement
						.note_change_key(clip.pattern, note, key + key_diff);
				}
			}
			piano_roll::Action::TrimStart(pos_diff) => {
				for &note in &*primary {
					let pos = self.arrangement.midi_patterns()[&clip.pattern].notes[note]
						.position
						.start();
					self.arrangement
						.note_trim_start_to(clip.pattern, note, pos + pos_diff);
				}
			}
			piano_roll::Action::TrimEnd(pos_diff) => {
				for &note in &*primary {
					let pos = self.arrangement.midi_patterns()[&clip.pattern].notes[note]
						.position
						.end();
					self.arrangement
						.note_trim_end_to(clip.pattern, note, pos + pos_diff);
				}
			}
			piano_roll::Action::SplitAt(mut pos) => {
				let mut extra = 0;

				let mut sorted = primary
					.drain()
					.filter(|&lhs| {
						let note = self.arrangement.midi_patterns()[&clip.pattern].notes[lhs];
						(note.position.start()..=note.position.end()).contains(&pos)
					})
					.collect::<Vec<_>>();
				sorted.sort_unstable();

				for mut lhs in sorted {
					lhs += extra;

					let note = self.arrangement.midi_patterns()[&clip.pattern].notes[lhs];
					if note.position.end() == pos {
						primary.insert(lhs);
					} else if note.position.start() == pos {
						secondary.insert(lhs);
					} else if note.position.len() > BeatTime::TICK {
						let start = note.position.start() + BeatTime::TICK;
						let end = note.position.end() - BeatTime::TICK;
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

				for &lhs in &*primary {
					let note = self.arrangement.midi_patterns()[&clip.pattern].notes[lhs];
					let new = note.position.start() + BeatTime::TICK;

					clamped
						.entry(note.key)
						.and_modify(|old| *old = new.max(*old))
						.or_insert_with(|| new.max(pos));
				}

				for &rhs in &*secondary {
					let note = self.arrangement.midi_patterns()[&clip.pattern].notes[rhs];
					let new = note.position.end() - BeatTime::TICK;

					clamped
						.entry(note.key)
						.and_modify(|old| *old = new.min(*old))
						.or_insert_with(|| new.min(pos));
				}

				for &lhs in &*primary {
					let note = self.arrangement.midi_patterns()[&clip.pattern].notes[lhs];
					self.arrangement
						.note_trim_end_to(clip.pattern, lhs, clamped[&note.key]);
				}

				for &rhs in &*secondary {
					let note = self.arrangement.midi_patterns()[&clip.pattern].notes[rhs];
					self.arrangement
						.note_trim_start_to(clip.pattern, rhs, clamped[&note.key]);
				}
			}
			piano_roll::Action::DragVelocity(val) => {
				for &note in &*primary {
					self.arrangement
						.note_change_velocity(clip.pattern, note, val);
				}
			}
			piano_roll::Action::Delete => {
				let mut sorted = primary.drain().collect::<Vec<_>>();
				sorted.sort_unstable_by_key(|&n| Reverse(n));
				for note in sorted {
					self.arrangement.remove_note(clip.pattern, note);
				}
			}
		}
	}

	pub fn view<'a>(
		&'a self,
		state: &'a State,
		plugins: &'a combo_box::State<PluginDescriptor>,
	) -> Element<'a, Message> {
		match self.tab {
			Tab::Playlist => self.view_arrangement(),
			Tab::Mixer => self.view_mixer(state, plugins),
			Tab::PianoRoll => self.view_piano_roll(),
		}
	}

	fn view_arrangement(&self) -> Element<'_, Message> {
		Seeker::new(
			self.arrangement.transport(),
			self.playlist.borrow().position,
			self.playlist.borrow().scale,
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
								column![
									row![
										PeakMeter::new(&node.peaks[0]),
										PeakMeter::new(&node.peaks[1])
									]
									.spacing(2),
									container(space().width(28).height(2)).style(
										container_with_radius(
											if node.polyphony > 0 {
												container::primary
											} else {
												container::secondary
											},
											border::bottom(f32::INFINITY),
										)
									)
								]
								.spacing(2),
								column![
									Knob::new(0.0..=MAX_VOL, node.volume.abs().cbrt(), move |v| {
										Message::ChannelVolumeChanged(
											id,
											v.powi(3).copysign(node.volume),
										)
									})
									.default(1.0)
									.enabled(node.enabled)
									.tooltip(format_db(node.volume.abs())),
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
									text_icon_button("S", button_style(self.soloed == Some(id)))
										.on_press(Message::TrackToggleSolo(id)),
									icon_button(
										mic(),
										button_style(
											self.recording
												.as_ref()
												.is_some_and(|recording| recording.node == id)
										)
									)
									.on_press(Message::Recording(id))
								]
								.spacing(5)
								.wrap()
							]
							.spacing(5),
						)
						.style(weak_bordered_box)
						.padding(5)
						.height(self.playlist.borrow().scale.y)
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
				&self.playlist,
				self.arrangement.transport(),
				self.arrangement
					.tracks()
					.iter()
					.enumerate()
					.map(|(track_idx, track)| {
						let node = self.arrangement.node(track.id);

						Track::new(
							track
								.clips
								.iter()
								.enumerate()
								.map(|(clip_idx, clip)| match clip {
									clip::Clip::Audio(clip) => Clip::new(
										AudioClipRef {
											sample: &self.arrangement.samples()[&clip.sample],
											clip,
											idx: (track_idx, clip_idx),
										},
										&self.playlist,
										self.arrangement.transport(),
										node.enabled,
										Message::PlaylistAction,
									),
									clip::Clip::Midi(clip) => Clip::new(
										MidiClipRef {
											pattern: &self.arrangement.midi_patterns()
												[&clip.pattern],
											clip,
											idx: (track_idx, clip_idx),
										},
										&self.playlist,
										self.arrangement.transport(),
										node.enabled,
										Message::PlaylistAction,
									),
								})
								.chain(
									self.recording
										.as_ref()
										.filter(|recording| recording.node == track.id)
										.map(|recording| {
											Clip::new(
												recording,
												&self.playlist,
												self.arrangement.transport(),
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
			|pos_diff, _, visible| {
				Message::PlaylistAction(playlist::Action::Pan(pos_diff, visible))
			},
			|scale_diff, cursor, _, visible| {
				Message::PlaylistAction(playlist::Action::Zoom(scale_diff, cursor, visible))
			},
		)
		.into()
	}

	fn view_mixer<'a>(
		&'a self,
		state: &'a State,
		plugins: &'a combo_box::State<PluginDescriptor>,
	) -> Element<'a, Message> {
		let node = self.arrangement.node(self.selected);

		Split::new(
			scrollable(
				row(once(self.view_channel(self.arrangement.master(), "M"))
					.chain(once(rule::vertical(1).into()))
					.chain({
						let mut iter = self
							.arrangement
							.tracks()
							.iter()
							.map(|track| self.arrangement.node(track.id))
							.enumerate()
							.map(|(i, node)| self.view_channel(node, format!("T{}", i + 1)))
							.peekable();

						let one = iter.peek().map(|_| rule::vertical(1).into());
						iter.chain(one)
					})
					.chain(
						self.arrangement
							.channels()
							.enumerate()
							.map(|(i, node)| self.view_channel(node, format!("C{}", i + 1))),
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
			.id("mixer")
			.direction(scrollable::Direction::Horizontal(
				scrollable::Scrollbar::default(),
			))
			.spacing(5)
			.style(scrollable_style)
			.width(Fill),
			column![
				combo_box(plugins, "Add Plugin", None, move |descriptor| {
					Message::PluginLoad(self.selected, descriptor, true)
				})
				.menu_style(menu_style)
				.width(Fill),
				container(rule::horizontal(1)).padding(padding::vertical(5)),
				scrollable(
					sweeten::column(
						node.plugins
							.iter()
							.enumerate()
							.map(|(i, plugin)| {
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
										Message::PluginMixChanged(self.selected, i, mix)
									})
									.radius(TEXT_HEIGHT)
									.enabled(plugin.enabled && node.enabled)
									.tooltip(format!("{:.0}%", plugin.mix * 100.0)),
									button(
										text(&*plugin.descriptor.name)
											.wrapping(text::Wrapping::None)
											.ellipsis(text::Ellipsis::End)
									)
									.padding(7)
									.style(button_with_radius(button_style(false), border::left(5)))
									.width(Fill)
									.on_press(Message::PluginShow(plugin.id)),
									column![
										icon_button(
											if plugin.enabled && !node.bypassed {
												power()
											} else {
												power_off()
											},
											button_style(node.bypassed)
										)
										.on_press(Message::PluginToggleEnabled(self.selected, i)),
										icon_button(
											x(),
											if plugin.enabled && node.enabled {
												button::danger
											} else {
												button::secondary
											}
										)
										.on_press(Message::PluginRemove(self.selected, i)),
									]
									.spacing(5),
								]
								.align_y(Center)
								.spacing(5)
							})
							.map(|widget| {
								row![
									opaque(widget),
									mouse_area(
										container(grip_vertical())
											.center_y(LINE_HEIGHT + 14.0)
											.style(container_with_radius(
												weak_bordered_box,
												border::right(5)
											))
									)
									.interaction(Interaction::Grab),
								]
								.align_y(Center)
								.spacing(5)
								.into()
							})
					)
					.spacing(5)
					.on_drag(|node| Message::PluginMoveTo(self.selected, node))
					.style(sweeten_column_style),
				)
				.spacing(5)
				.style(scrollable_style)
				.height(Fill)
			],
			state.plugins_panel_split_at,
		)
		.on_drag(Message::OnDrag)
		.on_drag_end(Message::OnDragEnd)
		.on_double_click(Message::OnDoubleClick)
		.strategy(Strategy::End)
		.focus_delay(Duration::ZERO)
		.style(split_style)
		.into()
	}

	fn view_channel<'a>(
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
						text_icon_button("S", button_style(self.soloed == Some(node.id)))
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
							if node.bypassed { power_off() } else { power() },
							button_style(node.bypassed)
						)
						.on_press(Message::ChannelToggleBypassed(node.id)),
						icon_button(
							arrow_up_down(),
							button_style(node.volume.is_sign_negative())
						)
						.on_press(Message::ChannelVolumeChanged(node.id, -node.volume)),
						icon_button(
							if node.pan.is_balance() {
								arrow_left_right()
							} else {
								radius()
							},
							button_style(false),
						)
						.on_press(Message::ChannelPanChanged(
							node.id,
							if node.pan.is_balance() {
								PanMode::Stereo(-1.0, 1.0)
							} else {
								PanMode::Balance(0.0)
							},
						))
					]
					.spacing(5),
					center_x(text(format_db(node.volume.abs())).line_height(1.0))
						.style(weak_bordered_box)
						.padding(2),
					row![
						column![
							space().height(7),
							row![
								PeakMeter::new(&node.peaks[0]).width(16.0),
								PeakMeter::new(&node.peaks[1]).width(16.0),
							]
							.spacing(3),
							container((node.ty == NodeType::Track).then(|| {
								container(space().width(35).height(3)).style(container_with_radius(
									if node.polyphony > 0 {
										container::primary
									} else {
										container::secondary
									},
									border::bottom(f32::INFINITY),
								))
							}))
							.height(7)
						]
						.spacing(3),
						vertical_slider(0.0..=MAX_VOL, node.volume.abs().cbrt(), |v| {
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
					]
					.spacing(3),
					{
						let incoming = self.arrangement.outgoing(node.id).get(&self.selected);
						let outgoing = self.arrangement.outgoing(self.selected).get(&node.id);

						let down = |r: border::Radius| {
							button(chevron_down())
								.padding(0)
								.style(button_with_radius(
									if node.enabled && incoming.is_some() {
										button::primary
									} else {
										button::secondary
									},
									r,
								))
								.on_press_maybe(if outgoing.is_some() {
									None
								} else if incoming.is_some() {
									Some(Message::Disconnect(node.id, self.selected))
								} else {
									Some(Message::Connect(node.id, self.selected))
								})
						};

						let up = |r: border::Radius| {
							button(chevron_up())
								.padding(0)
								.style(button_with_radius(
									if node.enabled && outgoing.is_some() {
										button::primary
									} else {
										button::secondary
									},
									r,
								))
								.on_press_maybe(if incoming.is_some() {
									None
								} else if outgoing.is_some() {
									Some(Message::Disconnect(self.selected, node.id))
								} else {
									Some(Message::Connect(self.selected, node.id))
								})
						};

						column![
							incoming
								.map(|val| (val, node.id, self.selected))
								.or_else(|| outgoing.map(|val| (val, self.selected, node.id)))
								.map(|(val, from, to)| {
									slider(0.0..=1.0, val.cbrt(), move |val| {
										Message::SetMix(from, to, val.powi(3))
									})
									.default(1.0)
									.step(f32::EPSILON)
									.handle((4, 4))
								}),
							if node.id == self.selected {
								row![]
							} else {
								match (node.ty, self.arrangement.node(self.selected).ty) {
									(NodeType::Track, NodeType::Track) => row![],
									(_, NodeType::Master)
									| (NodeType::Track, NodeType::Channel) => {
										row![down(border::radius(5))]
									}
									(NodeType::Master, _) | (_, NodeType::Track) => {
										row![up(border::radius(5))]
									}
									_ => row![down(border::left(5)), up(border::right(5))],
								}
							}
							.height(LINE_HEIGHT)
						]
						.align_x(Center)
						.spacing(3)
					}
				]
				.align_x(Center)
				.spacing(5),
			)
			.id(node.widget_id.clone())
			.padding(5)
			.style(|t| {
				let mut style = weakest_bordered_box(t);
				if node.id == self.selected {
					style.border.width = 1.5;
					style.border.color = t.palette().primary.base.color;
				}
				style
			}),
		)
		.interaction(Interaction::Pointer)
		.on_press(Message::ChannelSelect(node.id))
		.into()
	}

	fn view_piano_roll(&self) -> Element<'_, Message> {
		let clip = self.midi_clip().unwrap();

		Seeker::new(
			self.arrangement.transport(),
			self.piano_roll.borrow().position,
			self.piano_roll.borrow().scale,
			Piano::new(
				self.piano_roll.borrow().position,
				self.piano_roll.borrow().scale,
			),
			PianoRoll::new(
				&self.piano_roll,
				self.arrangement.transport(),
				self.arrangement.midi_patterns()[&clip.pattern]
					.notes
					.iter()
					.enumerate()
					.map(|(idx, note)| {
						Note::new(
							idx,
							note,
							&self.piano_roll,
							self.arrangement.transport(),
							Message::PianoRollAction,
						)
					}),
				Message::PianoRollAction,
			),
			Message::SeekTo,
			Message::SetLoopMarker,
			|pos_diff, height, visible| {
				Message::PianoRollAction(piano_roll::Action::Pan(pos_diff, height, visible))
			},
			|scale_diff, cursor, height, visible| {
				Message::PianoRollAction(piano_roll::Action::Zoom(
					scale_diff, cursor, height, visible,
				))
			},
		)
		.with_offset(
			clip.position
				.start()
				.to_samples(self.arrangement.transport()) as f32
				- clip
					.position
					.offset()
					.to_samples(self.arrangement.transport()) as f32,
		)
		.into()
	}

	pub fn subscription() -> Subscription<Message> {
		every(Duration::from_secs(1)).map(|_| Message::UpdateRequest)
	}

	pub fn keybinds(
		key: &keyboard::Key,
		physical_key: keyboard::key::Physical,
		modifiers: keyboard::Modifiers,
		repeat: bool,
	) -> Option<Message> {
		match (
			modifiers.command(),
			modifiers.shift(),
			modifiers.alt(),
			repeat,
		) {
			(false, false, false, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::Tab) => Some(Message::CycleTabForwards),
				keyboard::Key::Named(
					keyboard::key::Named::Delete | keyboard::key::Named::Backspace,
				) => Some(Message::Delete),
				keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::UnselectAll),
				keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(Message::ArrowUp),
				keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(Message::ArrowDown),
				keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => Some(Message::ArrowLeft),
				keyboard::Key::Named(keyboard::key::Named::ArrowRight) => Some(Message::ArrowRight),
				_ => None,
			},
			(false, false, false, true) => match key.as_ref() {
				keyboard::Key::Named(
					keyboard::key::Named::Delete | keyboard::key::Named::Backspace,
				) => Some(Message::Delete),
				keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(Message::ArrowUp),
				keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(Message::ArrowDown),
				keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => Some(Message::ArrowLeft),
				keyboard::Key::Named(keyboard::key::Named::ArrowRight) => Some(Message::ArrowRight),
				_ => None,
			},
			(true, false, false, false) => match key.to_latin(physical_key) {
				Some('a') => Some(Message::SelectAll),
				Some('d') => Some(Message::Duplicate),
				Some('i') => Some(Message::SelectInverse),
				_ => match key.as_ref() {
					keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
						Some(Message::TransposeOctUp)
					}
					keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
						Some(Message::TransposeOctDown)
					}
					_ => None,
				},
			},
			(true, false, false, true) => match key.to_latin(physical_key) {
				Some('d') => Some(Message::Duplicate),
				_ => match key.as_ref() {
					keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
						Some(Message::TransposeOctUp)
					}
					keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
						Some(Message::TransposeOctDown)
					}
					_ => None,
				},
			},
			(false, true, false, false) => match key.as_ref() {
				keyboard::Key::Named(keyboard::key::Named::Tab) => Some(Message::CycleTabBackwards),
				_ => None,
			},
			(false, false, true, false) => match key.to_latin(physical_key) {
				Some('q') => Some(Message::Quantize),
				_ => None,
			},
			_ => None,
		}
	}

	pub fn end_recording(&mut self) {
		if let Some(recording) = &mut self.recording {
			recording.end_stream();
		}
	}

	pub fn hover_file(&mut self, file: Arc<Path>, kind: FileKind) {
		self.playlist.get_mut().status = playlist::Status::Hovering(file, kind, None);
	}

	pub fn tab(&self) -> Tab {
		self.tab
	}

	pub fn midi_clip(&self) -> Option<MidiClip> {
		let (track, clip) = self.midi_clip?;
		if let clip::Clip::Midi(clip) = self.arrangement.tracks()[track].clips[clip] {
			Some(clip)
		} else {
			None
		}
	}

	pub fn loading(&self) -> bool {
		self.loading > 0
	}

	fn select_prev(&mut self) {
		self.selected = if self.arrangement.node(self.selected).ty == NodeType::Channel {
			self.arrangement
				.channels()
				.rfind(|channel| channel.id < self.selected)
				.map(|channel| channel.id)
				.or_else(|| self.arrangement.tracks().last().map(|track| track.id))
		} else {
			self.arrangement
				.tracks()
				.iter()
				.rfind(|track| track.id < self.selected)
				.map(|track| track.id)
		}
		.unwrap_or_else(|| self.arrangement.master().id);
	}

	fn select_next(&mut self) {
		self.selected = if self.arrangement.node(self.selected).ty == NodeType::Channel {
			self.arrangement
				.channels()
				.find(|&channel| channel.id > self.selected)
				.map(|channel| channel.id)
				.or_else(|| self.arrangement.channels().last().map(|channel| channel.id))
		} else {
			self.arrangement
				.tracks()
				.iter()
				.find(|track| track.id > self.selected)
				.map(|track| track.id)
				.or_else(|| self.arrangement.channels().next().map(|channel| channel.id))
				.or_else(|| self.arrangement.tracks().last().map(|track| track.id))
		}
		.unwrap_or_else(|| self.arrangement.master().id);
	}
}

fn update_selection(
	(ct, cc): (usize, usize),
	track: usize,
	clip: Option<usize>,
) -> Option<(usize, usize)> {
	clip.filter(|_| ct == track).map_or_else(
		|| match ct.cmp(&track) {
			Ordering::Equal => None,
			Ordering::Less => Some((ct, cc)),
			Ordering::Greater => Some((ct - 1, cc)),
		},
		|clip| match cc.cmp(&clip) {
			Ordering::Equal => None,
			Ordering::Less => Some((ct, cc)),
			Ordering::Greater => Some((ct, cc - 1)),
		},
	)
}

fn amp_to_db(amp: f32) -> f32 {
	20.0 * amp.log10()
}

fn db_to_amp(db: f32) -> f32 {
	10f32.powf(db / 20.0)
}

fn format_db(amp: f32) -> String {
	let db = amp_to_db(amp);
	let dba = db.abs();

	format!(
		"{}{dba:.*}",
		if dba < 0.05 {
			""
		} else if db.is_sign_positive() {
			"+"
		} else {
			"-"
		},
		(dba < 99.95).into()
	)
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

fn poll_consumer<T>(
	mut consumer: Consumer<T>,
	sample_rate: NonZero<u32>,
	frames: Option<NonZero<u32>>,
) -> impl Stream<Item = T> {
	let min = 64.0 / sample_rate.get() as f32;
	let max = frames.or(NonZero::new(8192)).unwrap().get() as f32 / sample_rate.get() as f32;
	let mut backoff = 0.0;
	let mut backoff = move |counter: u16| {
		let divisor = f32::from(counter).max(0.5);
		backoff = ((backoff + backoff / divisor) * 0.5).clamp(min, max);
		Timer::after(Duration::from_secs_f32(backoff))
	};

	stream::channel(consumer.buffer().capacity(), async move |mut sender| {
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
	})
}
