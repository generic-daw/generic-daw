use crate::{
	arrangement_view::{
		clip::Clip,
		midi_clip::MidiClip,
		midi_pattern::{MidiPattern, MidiPatternPair},
		node::{Node, NodeType},
		plugin::PluginPair,
		poll_consumer,
		sample::{Sample, SamplePair},
		track::Track,
	},
	clap_host,
	config::Config,
	daw,
};
use generic_daw_core::{
	Batch, Message, MidiClipId, MidiKey, MidiNote, MidiNoteId, MidiPatternAction, MidiPatternId,
	NodeAction, NodeId, NodeImpl, PanMode, PluginId, SampleId, Stream, Transport, Update, Version,
	build_output_stream,
	clap_host::{HostInfo, PluginDescriptor},
	time::{BeatRange, BeatTime},
};
use iced::Task;
use rtrb::{Producer, PushError};
use smol::unblock;
use std::{
	collections::BTreeMap,
	num::NonZero,
	path::Path,
	sync::{Arc, LazyLock},
};
use utils::{NoDebug, ShiftMoveExt as _};

static HOST: LazyLock<HostInfo> = LazyLock::new(|| {
	HostInfo::new_from_cstring(
		c"Generic DAW".to_owned(),
		c"Generic DAW".to_owned(),
		c"https://github.com/generic-daw/generic-daw".to_owned(),
		c"0.0.0".to_owned(),
	)
});

#[derive(Debug)]
pub struct Arrangement {
	transport: Transport,
	load: Option<f32>,

	samples: BTreeMap<SampleId, Sample>,
	midi_patterns: BTreeMap<MidiPatternId, MidiPattern>,

	tracks: Vec<Track>,
	nodes: BTreeMap<NodeId, (Node, BTreeMap<NodeId, f32>)>,
	master: NodeId,

	producer: Producer<Message>,
	_stream: NoDebug<Stream>,
}

impl Arrangement {
	pub fn create(config: &Config) -> (Self, Task<Batch>) {
		let (master, transport, producer, consumer, stream) = build_output_stream(
			config.output_device.id.as_ref(),
			config.output_device.sample_rate,
			config.output_device.buffer_size,
		);

		let mut nodes = BTreeMap::new();
		nodes.insert(
			master,
			(Node::new(NodeType::Master, master), BTreeMap::new()),
		);

		(
			Self {
				transport,
				load: None,

				samples: BTreeMap::new(),
				midi_patterns: BTreeMap::new(),

				tracks: Vec::new(),
				nodes,
				master,

				producer,
				_stream: stream.into(),
			},
			Task::stream(poll_consumer(
				consumer,
				transport.sample_rate,
				Some(transport.frames),
			)),
		)
	}

	pub fn update(&mut self, mut batch: Batch) -> Vec<clap_host::Message> {
		let mut messages = Vec::new();

		if batch.version == self.transport.version {
			self.transport.position = batch.position;
		}

		for update in batch.updates.drain(..) {
			match update {
				Update::Peaks(node, peaks) => {
					if let Some((node, _)) = self.nodes.get_mut(&node) {
						node.update(peaks, batch.now);
					}
				}
				Update::Polyphony(node, polyphony) => {
					if let Some((node, _)) = self.nodes.get_mut(&node) {
						node.polyphony = polyphony;
					}
				}
				Update::Param(id, param_id, value) => {
					messages.push(clap_host::Message::ParamChange(id, param_id, value));
				}
				Update::ConnectFailed(from, to) => _ = self.outgoing_mut(from).remove(&to),
				Update::Load(duration, frames) => {
					let mix = self.transport.sample_rate.get() as f32 / frames as f32;
					let load = duration.as_secs_f32() * mix;
					self.load = Some(
						self.load
							.map_or(load, |new| (new * mix + load) / (mix + 1.0)),
					);
				}
			}
		}

		self.send(Message::ReturnUpdate(batch.updates));

		messages
	}

	pub fn transport(&self) -> &Transport {
		&self.transport
	}

	pub fn load(&self) -> f32 {
		self.load.unwrap_or_default()
	}

	pub fn samples(&self) -> &BTreeMap<SampleId, Sample> {
		&self.samples
	}

	pub fn midi_patterns(&self) -> &BTreeMap<MidiPatternId, MidiPattern> {
		&self.midi_patterns
	}

	pub fn send(&mut self, mut message: Message) {
		while let Err(PushError::Full(msg)) = self.producer.push(message) {
			message = msg;
			std::thread::yield_now();
		}
	}

	fn node_action(&mut self, id: NodeId, action: NodeAction) {
		self.send(Message::NodeAction(id, action));
	}

	fn midi_pattern_action(&mut self, id: MidiPatternId, action: MidiPatternAction) {
		self.send(Message::MidiPatternAction(id, action));
	}

	pub fn request_update(&mut self) {
		self.send(Message::RequestUpdate);
	}

	pub fn channel_volume_changed(&mut self, id: NodeId, volume: f32) {
		self.node_mut(id).volume = volume;
		self.node_action(id, NodeAction::ChannelVolumeChanged(volume));
	}

	pub fn channel_pan_changed(&mut self, id: NodeId, pan: PanMode) {
		self.node_mut(id).pan = pan;
		self.node_action(id, NodeAction::ChannelPanChanged(pan));
	}

	pub fn channel_toggle_enabled(&mut self, id: NodeId) {
		self.node_mut(id).enabled ^= true;
		self.node_action(id, NodeAction::ChannelToggleEnabled);
	}

	pub fn channel_toggle_bypassed(&mut self, id: NodeId) {
		self.node_mut(id).bypassed ^= true;
		self.node_action(id, NodeAction::ChannelToggleBypassed);
	}

	pub fn plugin_load(
		&mut self,
		id: NodeId,
		descriptor: PluginDescriptor,
	) -> (PluginId, daw::Instruction) {
		let (plugin, processor, receiver) =
			PluginPair::new(descriptor, &self.transport, HOST.clone());
		let plugin_id = plugin.gui.id;
		self.node_mut(id).plugins.push(plugin.gui);
		self.node_action(id, NodeAction::PluginLoad(plugin_id, Box::new(processor)));
		(
			plugin_id,
			daw::Instruction::PluginLoad(plugin_id, plugin.core, receiver),
		)
	}

	pub fn plugin_remove(&mut self, id: NodeId, index: usize) {
		self.node_action(id, NodeAction::PluginRemove(index));
		self.node_mut(id).plugins.remove(index);
	}

	pub fn plugin_move_to(&mut self, id: NodeId, from: usize, to: usize) {
		self.node_mut(id).plugins.shift_move(from, to);
		self.node_action(id, NodeAction::PluginMoveTo(from, to));
	}

	pub fn plugin_toggle_enabled(&mut self, id: NodeId, index: usize) {
		self.node_mut(id).plugins[index].enabled ^= true;
		self.node_action(id, NodeAction::PluginToggleEnabled(index));
	}

	pub fn plugin_mix_changed(&mut self, id: NodeId, index: usize, mix: f32) {
		self.node_mut(id).plugins[index].mix = mix;
		self.node_action(id, NodeAction::PluginMixChanged(index, mix));
	}

	pub fn set_loop_range(&mut self, loop_range: Option<BeatRange>) {
		if self.transport.loop_range != loop_range {
			self.transport.loop_range = loop_range;
			self.send(Message::LoopRange(loop_range));
		}
	}

	pub fn seek_to(&mut self, position: BeatTime) {
		self.transport.version = Version::unique();
		self.transport.position = position.to_seconds_time(self.transport());
		self.send(Message::Position(
			self.transport.version,
			self.transport.position,
		));
	}

	pub fn set_bpm(&mut self, bpm: NonZero<u16>) {
		if self.transport.bpm != bpm {
			self.transport.bpm = bpm;
			self.send(Message::Bpm(bpm));
		}
	}

	pub fn set_numerator(&mut self, numerator: NonZero<u8>) {
		if self.transport.numerator != numerator {
			self.transport.numerator = numerator;
			self.send(Message::Numerator(numerator));
		}
	}

	pub fn play(&mut self) {
		if !self.transport.playing {
			self.toggle_playback();
		}
	}

	pub fn pause(&mut self) {
		if self.transport.playing {
			self.toggle_playback();
		}
	}

	pub fn stop(&mut self) {
		self.pause();
		self.seek_to(
			self.transport
				.loop_range
				.map_or(BeatTime::ZERO, BeatRange::start),
		);
		self.send(Message::Reset);
	}

	pub fn toggle_playback(&mut self) {
		self.transport.playing ^= true;
		self.send(Message::TogglePlayback);
	}

	pub fn toggle_metronome(&mut self) {
		self.transport.metronome ^= true;
		self.send(Message::ToggleMetronome);
	}

	pub fn master(&self) -> &Node {
		self.node(self.master)
	}

	pub fn tracks(&self) -> &[Track] {
		&self.tracks
	}

	pub fn track_of(&self, id: NodeId) -> Option<usize> {
		self.tracks.iter().position(|t| t.id == id)
	}

	pub fn plugin_of(&self, id: PluginId) -> Option<(NodeId, usize)> {
		self.nodes
			.values()
			.find_map(|(node, _)| Some((node.id, node.plugins.iter().position(|p| p.id == id)?)))
	}

	pub fn solo_track(&mut self, id: NodeId) {
		for i in 0..self.tracks.len() {
			let track_id = self.tracks[i].id;

			if self.node_mut(track_id).enabled == (id == track_id) {
				continue;
			}

			self.channel_toggle_enabled(track_id);
		}
	}

	pub fn enable_all_tracks(&mut self) {
		for i in 0..self.tracks.len() {
			let track_id = self.tracks[i].id;

			if self.node_mut(track_id).enabled {
				continue;
			}

			self.channel_toggle_enabled(track_id);
		}
	}

	pub fn channels(&self) -> impl DoubleEndedIterator<Item = &Node> {
		self.nodes
			.values()
			.filter_map(|(node, _)| (node.ty == NodeType::Channel).then_some(node))
	}

	pub fn node(&self, id: NodeId) -> &Node {
		&self.nodes[&id].0
	}

	fn node_mut(&mut self, id: NodeId) -> &mut Node {
		&mut self.nodes.get_mut(&id).unwrap().0
	}

	pub fn outgoing(&self, id: NodeId) -> &BTreeMap<NodeId, f32> {
		&self.nodes[&id].1
	}

	fn outgoing_mut(&mut self, id: NodeId) -> &mut BTreeMap<NodeId, f32> {
		&mut self.nodes.get_mut(&id).unwrap().1
	}

	fn add(&mut self, node: impl Into<generic_daw_core::Node> + NodeImpl, ty: NodeType) -> NodeId {
		let id = node.id();
		self.nodes.insert(id, (Node::new(ty, id), BTreeMap::new()));
		self.send(Message::NodeAdd(Box::new(node.into())));
		id
	}

	pub fn add_channel(&mut self) -> NodeId {
		self.add(generic_daw_core::Channel::default(), NodeType::Channel)
	}

	pub fn remove_channel(&mut self, id: NodeId) -> Node {
		debug_assert!(self.track_of(id).is_none());
		let node = self.nodes.remove(&id).unwrap().0;
		for (_, outgoing) in self.nodes.values_mut() {
			outgoing.remove(&id);
		}
		self.send(Message::NodeRemove(id));
		node
	}

	pub fn add_track(&mut self) -> usize {
		let id = self.add(generic_daw_core::Track::default(), NodeType::Track);
		self.tracks.push(Track::new(id));
		self.tracks.len() - 1
	}

	pub fn remove_track(&mut self, id: NodeId) {
		let idx = self.track_of(id).unwrap();
		let track = self.tracks.remove(idx);
		self.remove_channel(id);
		for clip in track.clips {
			match clip {
				Clip::Audio(clip) => self.samples.get_mut(&clip.sample).unwrap().refs -= 1,
				Clip::Midi(clip) => self.midi_patterns.get_mut(&clip.pattern).unwrap().refs -= 1,
			}

			self.gc(clip);
		}
	}

	pub fn connect(&mut self, from: NodeId, to: NodeId) {
		self.outgoing_mut(from).insert(to, 1.0);
		self.send(Message::NodeConnect(from, to));
	}

	pub fn set_mix(&mut self, from: NodeId, to: NodeId, mix: f32) {
		*self.outgoing_mut(from).get_mut(&to).unwrap() = mix;
		self.send(Message::NodeSetMix(from, to, mix));
	}

	pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
		self.outgoing_mut(from).remove(&to);
		self.send(Message::NodeDisconnect(from, to));
	}

	pub fn add_sample(&mut self, sample: SamplePair) {
		self.samples.insert(sample.gui.id, sample.gui);
		self.send(Message::SampleAdd(sample.core));
	}

	pub fn gc(&mut self, clip: impl Into<Clip>) {
		match clip.into() {
			Clip::Audio(clip) => {
				if self.samples[&clip.sample].refs == 0 {
					self.samples.remove(&clip.sample);
					self.send(Message::SampleRemove(clip.sample));
				}
			}
			Clip::Midi(clip) => {
				if self.midi_patterns[&clip.pattern].refs == 0 {
					self.midi_patterns.remove(&clip.pattern);
					self.send(Message::MidiPatternRemove(clip.pattern));
				}
			}
		}
	}

	pub fn add_midi_pattern(&mut self, pattern: MidiPatternPair) {
		self.midi_patterns.insert(pattern.gui.id, pattern.gui);
		self.send(Message::MidiPatternAdd(pattern.core));
	}

	pub fn add_clip(&mut self, track: usize, clip: impl Into<Clip>) -> usize {
		self.insert_clip(track, clip, self.tracks[track].clips.len())
	}

	pub fn duplicate_clip(&mut self, track: usize, clip: usize) -> usize {
		let clip = match self.tracks[track].clips[clip] {
			Clip::Audio(clip) => Clip::Audio(clip),
			Clip::Midi(clip) => Clip::Midi(MidiClip {
				id: MidiClipId::unique(),
				..clip
			}),
		};
		self.insert_clip(track, clip, self.tracks[track].clips.len())
	}

	pub fn insert_clip(&mut self, track: usize, clip: impl Into<Clip>, idx: usize) -> usize {
		let clip = clip.into();
		match clip {
			Clip::Audio(clip) => self.samples.get_mut(&clip.sample).unwrap().refs += 1,
			Clip::Midi(clip) => self.midi_patterns.get_mut(&clip.pattern).unwrap().refs += 1,
		}
		self.tracks[track].clips.insert(idx, clip);
		self.node_action(self.tracks[track].id, NodeAction::ClipAdd(clip.into(), idx));
		idx
	}

	pub fn remove_clip(&mut self, track: usize, clip: usize) -> Clip {
		self.node_action(self.tracks[track].id, NodeAction::ClipRemove(clip));
		let clip = self.tracks[track].clips.remove(clip);
		match clip {
			Clip::Audio(clip) => self.samples.get_mut(&clip.sample).unwrap().refs -= 1,
			Clip::Midi(clip) => self.midi_patterns.get_mut(&clip.pattern).unwrap().refs -= 1,
		}
		clip
	}

	pub fn clip_switch_track(&mut self, track: usize, clip: usize, new_track: usize) -> usize {
		if track == new_track {
			clip
		} else {
			let clip = self.remove_clip(track, clip);
			self.add_clip(new_track, clip)
		}
	}

	pub fn clip_move_to(&mut self, track: usize, clip: usize, pos: BeatTime) {
		if self.tracks[track].clips[clip].start() != pos {
			self.tracks[track].clips[clip].move_to(pos);
			self.node_action(self.tracks[track].id, NodeAction::ClipMoveTo(clip, pos));
		}
	}

	pub fn clip_trim_start_to(&mut self, track: usize, clip: usize, pos: BeatTime) {
		if self.tracks[track].clips[clip].start() != pos {
			self.tracks[track].clips[clip].trim_start_to(pos, &self.transport);
			self.node_action(
				self.tracks[track].id,
				NodeAction::ClipTrimStartTo(clip, pos),
			);
		}
	}

	pub fn clip_trim_end_to(&mut self, track: usize, clip: usize, pos: BeatTime) {
		if self.tracks[track].clips[clip].end(&self.transport) != pos {
			self.tracks[track].clips[clip].trim_end_to(pos, &self.transport);
			self.node_action(self.tracks[track].id, NodeAction::ClipTrimEndTo(clip, pos));
		}
	}

	pub fn clip_stretch_start_to(&mut self, track: usize, clip: usize, pos: BeatTime) {
		if self.tracks[track].clips[clip].start() != pos {
			if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
				audio.stretch *= audio.position.stretch_start_to(pos, &self.transport);
				audio.stretch = audio.stretch.clamp(2f32.powi(-10), 2f32.powi(10));
				self.node_action(
					self.tracks[track].id,
					NodeAction::ClipStretchStartTo(clip, pos),
				);
			} else {
				self.clip_trim_start_to(track, clip, pos);
			}
		}
	}

	pub fn clip_stretch_end_to(&mut self, track: usize, clip: usize, pos: BeatTime) {
		if self.tracks[track].clips[clip].end(&self.transport) != pos {
			if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
				audio.stretch *= audio.position.stretch_end_to(pos, &self.transport);
				audio.stretch = audio.stretch.clamp(2f32.powi(-10), 2f32.powi(10));
				self.node_action(
					self.tracks[track].id,
					NodeAction::ClipStretchEndTo(clip, pos),
				);
			} else {
				self.clip_trim_end_to(track, clip, pos);
			}
		}
	}

	pub fn clip_slip_to(&mut self, track: usize, clip: usize, pos: BeatTime) {
		if self.tracks[track].clips[clip].offset(&self.transport) != pos {
			self.tracks[track].clips[clip].slip_to(pos, &self.transport);
			self.node_action(self.tracks[track].id, NodeAction::ClipSlipTo(clip, pos));
		}
	}

	pub fn add_note(&mut self, pattern: MidiPatternId, note: MidiNote) -> usize {
		self.insert_note(pattern, note, self.midi_patterns[&pattern].notes.len())
	}

	pub fn duplicate_note(&mut self, pattern: MidiPatternId, note: usize) -> usize {
		let note = MidiNote {
			id: MidiNoteId::unique(),
			..self.midi_patterns[&pattern].notes[note]
		};
		self.insert_note(pattern, note, self.midi_patterns[&pattern].notes.len())
	}

	pub fn insert_note(&mut self, pattern: MidiPatternId, note: MidiNote, idx: usize) -> usize {
		self.midi_patterns
			.get_mut(&pattern)
			.unwrap()
			.notes
			.insert(idx, note);
		self.midi_pattern_action(pattern, MidiPatternAction::Add(note, idx));
		idx
	}

	pub fn remove_note(&mut self, pattern: MidiPatternId, note: usize) -> MidiNote {
		self.midi_pattern_action(pattern, MidiPatternAction::Remove(note));
		self.midi_patterns
			.get_mut(&pattern)
			.unwrap()
			.notes
			.remove(note)
	}

	pub fn note_change_velocity(&mut self, pattern: MidiPatternId, note: usize, velocity: f32) {
		if self.midi_patterns[&pattern].notes[note].velocity != velocity {
			self.midi_patterns.get_mut(&pattern).unwrap().notes[note].velocity = velocity;
			self.midi_pattern_action(pattern, MidiPatternAction::ChangeVelocity(note, velocity));
		}
	}

	pub fn note_change_key(&mut self, pattern: MidiPatternId, note: usize, key: MidiKey) {
		if self.midi_patterns[&pattern].notes[note].key != key {
			self.midi_patterns.get_mut(&pattern).unwrap().notes[note].key = key;
			self.midi_pattern_action(pattern, MidiPatternAction::ChangeKey(note, key));
		}
	}

	pub fn note_move_to(&mut self, pattern: MidiPatternId, note: usize, pos: BeatTime) {
		if self.midi_patterns[&pattern].notes[note].position.start() != pos {
			self.midi_patterns.get_mut(&pattern).unwrap().notes[note]
				.position
				.move_to(pos);
			self.midi_pattern_action(pattern, MidiPatternAction::MoveTo(note, pos));
		}
	}

	pub fn note_trim_start_to(&mut self, pattern: MidiPatternId, note: usize, pos: BeatTime) {
		if self.midi_patterns[&pattern].notes[note].position.start() != pos {
			self.midi_patterns.get_mut(&pattern).unwrap().notes[note]
				.position
				.trim_start_to(pos);
			self.midi_pattern_action(pattern, MidiPatternAction::TrimStartTo(note, pos));
		}
	}

	pub fn note_trim_end_to(&mut self, pattern: MidiPatternId, note: usize, pos: BeatTime) {
		if self.midi_patterns[&pattern].notes[note].position.end() != pos {
			self.midi_patterns.get_mut(&pattern).unwrap().notes[note]
				.position
				.trim_end_to(pos);
			self.midi_pattern_action(pattern, MidiPatternAction::TrimEndTo(note, pos));
		}
	}

	pub fn render(&mut self, path: Arc<Path>) -> Task<daw::Message> {
		let (a_sender, p_receiver) = oneshot::channel();
		let (p_sender, a_receiver) = oneshot::channel();
		self.send(Message::RequestProcessor(a_sender, a_receiver));
		let mut processor = p_receiver.recv().unwrap();

		let (progress_sender, progress_receiver) = smol::channel::unbounded();

		let len = self
			.tracks()
			.iter()
			.map(|track| track.len(&self.transport))
			.max()
			.unwrap_or_default();

		let beat_range = BeatRange::new(BeatTime::ZERO, len);

		let master = self.master;

		Task::batch([
			Task::future(unblock(move || {
				processor.render(
					&path,
					master,
					beat_range,
					|_| {},
					|f| progress_sender.try_send(f).unwrap(),
				);
				p_sender.send(processor).unwrap();
			}))
			.discard(),
			Task::stream(progress_receiver)
				.map(daw::Message::Progress)
				.chain(Task::done(daw::Message::RenderedFile)),
		])
	}
}
