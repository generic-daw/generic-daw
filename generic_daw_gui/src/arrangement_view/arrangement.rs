use crate::{
	arrangement_view::{
		self,
		channel::Channel,
		midi_pattern::{MidiPattern, MidiPatternPair},
		node::{Node, NodeType},
		plugin::{Plugin, PluginPair},
		poll_consumer,
		sample::{Sample, SamplePair},
		track::Track,
	},
	clap_host,
	config::Config,
	daw,
};
use generic_daw_core::{
	AudioClip, AudioThread, Batch, Clip, ClipId, Message, MidiClip, MidiKey, MidiNote, MidiNoteId,
	MidiPatternAction, MidiPatternId, NodeAction, NodeId, NodeImpl as _, PanMode, PluginId, Point,
	SampleId, Stream, Transport, Update, Version, build_output_stream,
	clap_host::{ClapId, HostInfo, PluginDescriptor},
	time::{BeatRange, BeatTime, SecondsTime},
};
use iced::Task;
use log::warn;
use rtrb::{Producer, PushError, RingBuffer};
use smol::unblock;
use std::{
	collections::{BTreeMap, VecDeque},
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
	channels: Vec<Channel>,
	master: NodeId,
	nodes: BTreeMap<NodeId, (Node, BTreeMap<NodeId, f32>)>,

	producer: Producer<Message>,
	queue: VecDeque<Message>,
	stream: Option<NoDebug<Stream>>,
}

impl Arrangement {
	pub fn create(
		sample_rate: NonZero<u32>,
		frames: NonZero<u32>,
		p_sender: oneshot::Sender<AudioThread>,
	) -> (Self, Task<Batch>) {
		let (p_producer, consumer) = RingBuffer::new(2048);
		let (producer, p_consumer) = RingBuffer::new(2048);

		let (processor, master, transport) =
			AudioThread::create(sample_rate, frames, p_producer, p_consumer);
		p_sender.send(processor).unwrap();

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
				channels: Vec::new(),
				master,
				nodes,

				producer,
				queue: VecDeque::new(),
				stream: None,
			},
			Task::stream(poll_consumer(consumer)),
		)
	}

	pub fn set_stream(&mut self, stream: Stream) {
		self.stream = Some(stream.into());
	}

	pub fn change_config(&mut self, config: &Config) {
		let (a_sender, p_receiver) = oneshot::channel();
		let (_, a_receiver) = oneshot::channel();
		self.send(Message::RequestProcessor(a_sender, a_receiver));

		let mut processor = p_receiver.recv().unwrap();
		let (p_sender, a_receiver) = oneshot::channel();

		self.stream = None;
		let (stream, sample_rate, frames) = build_output_stream(
			config.output_device.id.as_ref(),
			config.output_device.sample_rate,
			config.output_device.buffer_size,
			a_receiver,
		);
		self.stream = Some(stream.into());

		processor.change_config(sample_rate, frames);
		p_sender.send(processor).unwrap();

		self.transport.sample_rate = sample_rate;
		self.transport.frames = frames;
	}

	pub fn take_stream_from(
		&mut self,
		other: &mut Self,
		a_receiver: oneshot::Receiver<AudioThread>,
	) -> oneshot::Receiver<AudioThread> {
		let (a_sender, p_receiver) = oneshot::channel();
		other.send(Message::RequestProcessor(a_sender, a_receiver));
		self.stream = other.stream.take();
		p_receiver
	}

	pub fn update(&mut self, mut batch: Batch) -> Vec<clap_host::Message> {
		let mut messages = Vec::new();

		if batch.version == self.transport.version {
			self.transport.position = batch.position;
		}

		for update in batch.updates.drain(..) {
			match update {
				Update::Load(duration, frames) => {
					let mix = self.transport.sample_rate.get() as f32 / frames as f32;
					let load = duration.as_secs_f32() * mix;
					self.load = Some(
						self.load
							.map_or(load, |new| (new * mix + load) / (mix + 1.0)),
					);
				}
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
					messages.push(clap_host::Message::PluginParamChange(id, param_id, value));
				}
				Update::ConnectFailed(from, to) => _ = self.outgoing_mut(from).remove(&to),
			}
		}

		self.send(Message::ReturnUpdate(batch.updates));

		messages
	}

	pub fn drain_queue(&mut self) -> bool {
		while let Some(message) = self.queue.pop_front() {
			if let Err(PushError::Full(message)) = self.producer.push(message) {
				self.queue.push_front(message);
				return false;
			}
		}
		true
	}

	pub fn queue_empty(&self) -> bool {
		self.queue.is_empty()
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

	fn send(&mut self, message: Message) {
		if self.queue_empty() {
			if let Err(PushError::Full(message)) = self.producer.push(message) {
				warn!("full ring buffer");
				self.queue.push_back(message);
			}
		} else {
			warn!("full ring buffer");
			self.queue.push_back(message);
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
		if self.node(id).utility.volume != volume {
			self.node_mut(id).utility.volume = volume;
			self.node_action(id, NodeAction::ChannelVolumeChanged(volume));
		}
	}

	pub fn channel_pan_changed(&mut self, id: NodeId, pan: PanMode) {
		if self.node(id).utility.pan != pan {
			self.node_mut(id).utility.pan = pan;
			self.node_action(id, NodeAction::ChannelPanChanged(pan));
		}
	}

	pub fn channel_toggle_enabled(&mut self, id: NodeId) {
		self.node_mut(id).enabled ^= true;
		self.node_action(id, NodeAction::ChannelToggleEnabled);
	}

	pub fn channel_toggle_bypassed(&mut self, id: NodeId) {
		self.node_mut(id).bypassed ^= true;
		self.node_action(id, NodeAction::ChannelToggleBypassed);
	}

	pub fn track_toggle_enabled(&mut self, id: NodeId) {
		if let Some(solo) = self.transport.solo {
			for i in 0..self.tracks.len() {
				let node = self.node(self.tracks[i].id);

				if node.enabled == (solo == node.id || solo == id || node.id == id) {
					continue;
				}

				self.channel_toggle_enabled(node.id);
			}

			self.toggle_solo(solo);
		} else {
			self.channel_toggle_enabled(id);
		}
	}

	pub fn plugin_add(
		&mut self,
		id: NodeId,
		descriptor: PluginDescriptor,
	) -> Option<(PluginId, daw::Instruction)> {
		self.plugin_insert(id, descriptor, self.node(id).plugins.len())
	}

	pub fn plugin_insert(
		&mut self,
		id: NodeId,
		descriptor: PluginDescriptor,
		index: usize,
	) -> Option<(PluginId, daw::Instruction)> {
		let (plugin, receiver) = PluginPair::new(descriptor, HOST.clone())?;
		let plugin_id = plugin.gui.id;
		self.node_mut(id).plugins.insert(index, plugin.gui);
		self.node_action(id, NodeAction::PluginInsert(index, plugin_id));
		Some((
			plugin_id,
			daw::Instruction::PluginAdd(plugin_id, plugin.core, receiver),
		))
	}

	pub fn plugin_remove(&mut self, id: NodeId, index: usize) -> Plugin {
		let plugin = self.node_mut(id).plugins.remove(index);
		self.node_action(id, NodeAction::PluginRemove(index));
		plugin
	}

	pub fn plugin_move(&mut self, id: NodeId, from: usize, to: usize) {
		self.node_mut(id).plugins.shift_move(from, to);
		self.node_action(id, NodeAction::PluginMoveTo(from, to));
	}

	fn plugin_copy(
		&mut self,
		from: NodeId,
		from_i: usize,
		to: NodeId,
		to_i: usize,
	) -> impl Iterator<Item = daw::Instruction> {
		self.plugin_mix_changed(to, to_i, self.node(from).plugins[from_i].mix);

		[
			Some(daw::Instruction::PluginCopyState(
				self.node(from).plugins[from_i].id,
				self.node(to).plugins[to_i].id,
			)),
			self.node(from).plugins[from_i]
				.active
				.then_some(daw::Instruction::Message(daw::Message::ClapHost(
					clap_host::Message::Activate(self.node(to).plugins[to_i].id),
				))),
		]
		.into_iter()
		.flatten()
	}

	pub fn plugin_duplicate(&mut self, id: NodeId, index: usize) -> Option<Vec<daw::Instruction>> {
		let (_, instruction) = self.plugin_insert(
			id,
			self.node(id).plugins[index].descriptor.clone(),
			index + 1,
		)?;
		let mut instructions = vec![instruction];
		instructions.extend(self.plugin_copy(id, index, id, index + 1));
		Some(instructions)
	}

	pub fn plugin_activate(
		&mut self,
		id: NodeId,
		index: usize,
		processor: Option<Box<clap_host::AudioThread>>,
	) {
		self.node_mut(id).plugins[index].active = processor.is_some();
		if let Some(processor) = processor {
			self.node_action(id, NodeAction::PluginActivate(index, processor));
		}
	}

	pub fn plugin_deactivate(&mut self, id: NodeId, index: usize) {
		self.node_mut(id).plugins[index].active = false;
		self.node_action(id, NodeAction::PluginDeactivate(index));
	}

	pub fn plugin_mix_changed(&mut self, id: NodeId, index: usize, mix: f32) {
		if self.node(id).plugins[index].mix != mix {
			self.node_mut(id).plugins[index].mix = mix;
			self.node_action(id, NodeAction::PluginMixChanged(index, mix));
		}
	}

	pub fn plugin_param_changed(&mut self, id: NodeId, index: usize, clap_id: ClapId, value: f32) {
		self.node_action(id, NodeAction::PluginParamChanged(index, clap_id, value));
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

	pub fn seek_to(&mut self, position: BeatTime) {
		self.transport.version = Version::unique();
		self.transport.position = position.to_seconds_time(self.transport());
		self.send(Message::Position(
			self.transport.version,
			self.transport.position,
		));
	}

	pub fn set_loop_range(&mut self, loop_range: Option<BeatRange>) {
		if self.transport.loop_range != loop_range {
			self.send(Message::LoopRange(loop_range));
			if std::mem::replace(&mut self.transport.loop_range, loop_range).is_none() {
				self.seek_to(loop_range.unwrap().start());
			}
		}
	}

	pub fn toggle_solo(&mut self, id: NodeId) {
		if !self.node(id).enabled {
			self.channel_toggle_enabled(id);
		}
		let solo = (self.transport.solo != Some(id)).then_some(id);
		if self.transport.solo != solo {
			self.transport.solo = solo;
			self.send(Message::Solo(solo));
		}
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

	pub fn channel_of(&self, id: NodeId) -> Option<usize> {
		self.channels.iter().position(|c| c.id == id)
	}

	pub fn plugin_of(&self, id: PluginId) -> Option<(NodeId, usize)> {
		self.nodes
			.values()
			.find_map(|(node, _)| Some((node.id, node.plugins.iter().position(|p| p.id == id)?)))
	}

	pub fn channels(&self) -> &[Channel] {
		&self.channels
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

	fn add(&mut self, node: impl Into<generic_daw_core::Node>, ty: NodeType) -> NodeId {
		let node = node.into();
		let id = node.id();
		self.nodes.insert(id, (Node::new(ty, id), BTreeMap::new()));
		self.send(Message::NodeAdd(Box::new(node)));
		id
	}

	fn remove(&mut self, id: NodeId) -> Node {
		for (_, outgoing) in self.nodes.values_mut() {
			outgoing.remove(&id);
		}
		if self.transport.solo == Some(id) {
			self.toggle_solo(id);
		}
		self.send(Message::NodeRemove(id));
		self.nodes.remove(&id).unwrap().0
	}

	pub fn add_channel(&mut self) -> NodeId {
		self.insert_channel(self.channels.len())
	}

	pub fn insert_channel(&mut self, index: usize) -> NodeId {
		let id = self.add(generic_daw_core::Channel::default(), NodeType::Channel);
		self.channels.insert(index, Channel::new(id));
		id
	}

	pub fn remove_channel(&mut self, id: NodeId) -> Node {
		let index = self.channel_of(id).unwrap();
		self.channels.remove(index);
		self.remove(id)
	}

	pub fn move_channel(&mut self, channel: usize, new_channel: usize) {
		self.channels.shift_move(channel, new_channel);
	}

	fn copy_node(&mut self, from: NodeId, to: NodeId) -> Vec<daw::Instruction> {
		self.channel_volume_changed(to, self.node(from).utility.volume);
		self.channel_pan_changed(to, self.node(from).utility.pan);

		if !self.node(from).enabled {
			self.channel_toggle_enabled(to);
		}

		if self.node(from).bypassed {
			self.channel_toggle_bypassed(to);
		}

		let mut instructions = Vec::new();

		for i in 0..self.node(from).plugins.len() {
			let j = self.node(to).plugins.len();
			let Some((_, instruction)) =
				self.plugin_add(to, self.node(from).plugins[i].descriptor.clone())
			else {
				continue;
			};
			instructions.push(instruction);
			instructions.extend(self.plugin_copy(from, i, to, j));
		}

		let incoming = self
			.nodes
			.values()
			.filter_map(|(node, outgoing)| outgoing.get(&from).map(|&mix| (node.id, mix)))
			.collect::<Vec<_>>();

		for (incoming, mix) in incoming {
			self.connect(incoming, to);
			self.set_mix(incoming, to, mix);
		}

		for (outgoing, mix) in self.outgoing(from).clone() {
			self.connect(to, outgoing);
			self.set_mix(to, outgoing, mix);
		}

		instructions
	}

	pub fn duplicate_channel(&mut self, id: NodeId) -> (NodeId, Vec<daw::Instruction>) {
		let new_id = self.insert_channel(self.channel_of(id).unwrap() + 1);
		let instructions = self.copy_node(id, new_id);
		(new_id, instructions)
	}

	pub fn add_track(&mut self) -> NodeId {
		self.insert_track(self.tracks.len())
	}

	pub fn insert_track(&mut self, index: usize) -> NodeId {
		let id = self.add(generic_daw_core::Track::default(), NodeType::Track);
		self.tracks.insert(index, Track::new(id));
		id
	}

	pub fn remove_track(&mut self, id: NodeId) -> usize {
		let index = self.track_of(id).unwrap();
		let track = self.tracks.remove(index);
		self.remove(id);
		for clip in track.clips {
			match clip {
				Clip::Audio(clip) => self.samples.get_mut(&clip.sample).unwrap().refs -= 1,
				Clip::Midi(clip) => self.midi_patterns.get_mut(&clip.pattern).unwrap().refs -= 1,
			}

			self.gc(clip);
		}
		index
	}

	pub fn move_track(&mut self, track: usize, new_track: usize) {
		self.tracks.shift_move(track, new_track);
	}

	pub fn duplicate_track(&mut self, id: NodeId) -> (NodeId, Vec<daw::Instruction>) {
		let track = self.track_of(id).unwrap();
		let new_id = self.insert_track(track + 1);
		let instructions = self.copy_node(id, new_id);
		for clip in self.tracks[track].clips.clone() {
			self.add_clip(track + 1, clip);
		}
		(new_id, instructions)
	}

	pub fn connect(&mut self, from: NodeId, to: NodeId) {
		self.outgoing_mut(from).insert(to, 1.0);
		self.send(Message::NodeConnect(from, to));
	}

	pub fn set_mix(&mut self, from: NodeId, to: NodeId, mix: f32) {
		if self.outgoing(from)[&to] != mix {
			*self.outgoing_mut(from).get_mut(&to).unwrap() = mix;
			self.send(Message::NodeSetMix(from, to, mix));
		}
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
			Clip::Audio(clip) => Clip::Audio(AudioClip {
				id: ClipId::unique(),
				..clip
			}),
			Clip::Midi(clip) => Clip::Midi(MidiClip {
				id: ClipId::unique(),
				..clip
			}),
		};
		self.insert_clip(track, clip, self.tracks[track].clips.len())
	}

	fn insert_clip(&mut self, track: usize, clip: impl Into<Clip>, index: usize) -> usize {
		let clip = clip.into();
		match clip {
			Clip::Audio(clip) => self.samples.get_mut(&clip.sample).unwrap().refs += 1,
			Clip::Midi(clip) => self.midi_patterns.get_mut(&clip.pattern).unwrap().refs += 1,
		}
		self.tracks[track].clips.insert(index, clip);
		self.node_action(self.tracks[track].id, NodeAction::ClipAdd(Box::new(clip)));
		index
	}

	pub fn remove_clip(&mut self, track: usize, clip: usize) -> Clip {
		let clip = self.tracks[track].clips.remove(clip);
		self.node_action(self.tracks[track].id, NodeAction::ClipRemove(clip.id()));
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
			self.node_action(
				self.tracks[track].id,
				NodeAction::ClipMoveTo(self.tracks[track].clips[clip].id(), pos),
			);
		}
	}

	pub fn clip_trim_start_to(&mut self, track: usize, clip: usize, pos: BeatTime) {
		if self.tracks[track].clips[clip].start() != pos {
			self.tracks[track].clips[clip].trim_start_to(pos, &self.transport);
			if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
				audio.fade_start.len = audio.fade_start.len.min(audio.position.len());
				audio.fade_end.len = audio
					.fade_end
					.len
					.min(audio.position.len() - audio.fade_start.len);
			}
			self.node_action(
				self.tracks[track].id,
				NodeAction::ClipTrimStartTo(self.tracks[track].clips[clip].id(), pos),
			);
		}
	}

	pub fn clip_trim_end_to(&mut self, track: usize, clip: usize, pos: BeatTime) {
		if self.tracks[track].clips[clip].end(&self.transport) != pos {
			self.tracks[track].clips[clip].trim_end_to(pos, &self.transport);
			if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
				audio.fade_end.len = audio.fade_end.len.min(audio.position.len());
				audio.fade_start.len = audio
					.fade_start
					.len
					.min(audio.position.len() - audio.fade_end.len);
			}
			self.node_action(
				self.tracks[track].id,
				NodeAction::ClipTrimEndTo(self.tracks[track].clips[clip].id(), pos),
			);
		}
	}

	pub fn clip_volume_changed(&mut self, track: usize, clip: usize, volume: f32) {
		if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip]
			&& audio.volume != volume
		{
			audio.volume = volume;
			let id = audio.id;
			self.node_action(
				self.tracks[track].id,
				NodeAction::ClipVolumeChanged(id, volume),
			);
		}
	}

	pub fn clip_fade_start_len(&mut self, track: usize, clip: usize, len: SecondsTime) {
		if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
			let len = len.min(audio.position.len());
			if audio.fade_start.len != len {
				audio.fade_end.len = audio.fade_end.len.min(audio.position.len() - len);
				audio.fade_start.len = len;
				let id = audio.id;
				self.node_action(self.tracks[track].id, NodeAction::ClipFadeStartLen(id, len));
			}
		}
	}

	pub fn clip_fade_start_p(&mut self, track: usize, clip: usize, p: Point) {
		if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip]
			&& audio.fade_start.p != p
		{
			audio.fade_start.p = p;
			let id = audio.id;
			self.node_action(self.tracks[track].id, NodeAction::ClipFadeStartP(id, p));
		}
	}

	pub fn clip_fade_start_toggle_symmetric(&mut self, track: usize, clip: usize) {
		if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
			audio.fade_start.symmetric ^= true;
			let id = audio.id;
			self.node_action(
				self.tracks[track].id,
				NodeAction::ClipFadeStartToggleSymmetric(id),
			);
		}
	}

	pub fn clip_fade_end_len(&mut self, track: usize, clip: usize, len: SecondsTime) {
		if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
			let len = len.min(audio.position.len());
			if audio.fade_end.len != len {
				audio.fade_start.len = audio.fade_start.len.min(audio.position.len() - len);
				audio.fade_end.len = len;
				let id = audio.id;
				self.node_action(self.tracks[track].id, NodeAction::ClipFadeEndLen(id, len));
			}
		}
	}

	pub fn clip_fade_end_p(&mut self, track: usize, clip: usize, p: Point) {
		if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip]
			&& audio.fade_end.p != p
		{
			audio.fade_end.p = p;
			let id = audio.id;
			self.node_action(self.tracks[track].id, NodeAction::ClipFadeEndP(id, p));
		}
	}

	pub fn clip_fade_end_toggle_symmetric(&mut self, track: usize, clip: usize) {
		if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
			audio.fade_end.symmetric ^= true;
			let id = audio.id;
			self.node_action(
				self.tracks[track].id,
				NodeAction::ClipFadeEndToggleSymmetric(id),
			);
		}
	}

	pub fn clip_stretch_start_to(&mut self, track: usize, clip: usize, pos: BeatTime) {
		if self.tracks[track].clips[clip].start() != pos {
			if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
				let fac = audio.position.stretch_start_to(pos, &self.transport);
				audio.fade_start.len /= fac;
				audio.fade_end.len /= fac;
				audio.stretch *= fac;
				let id = audio.id;
				self.node_action(
					self.tracks[track].id,
					NodeAction::ClipStretchStartTo(id, pos),
				);
			} else {
				self.clip_trim_start_to(track, clip, pos);
			}
		}
	}

	pub fn clip_stretch_end_to(&mut self, track: usize, clip: usize, pos: BeatTime) {
		if self.tracks[track].clips[clip].end(&self.transport) != pos {
			if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
				let fac = audio.position.stretch_end_to(pos, &self.transport);
				audio.fade_start.len /= fac;
				audio.fade_end.len /= fac;
				audio.stretch *= fac;
				let id = audio.id;
				self.node_action(self.tracks[track].id, NodeAction::ClipStretchEndTo(id, pos));
			} else {
				self.clip_trim_end_to(track, clip, pos);
			}
		}
	}

	pub fn clip_reverse(&mut self, track: usize, clip: usize) {
		if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
			audio.stretch *= -1.0;
			audio.position.reverse(
				self.samples[&audio.sample].len(&self.transport),
				audio.stretch.abs(),
			);
			(audio.fade_start, audio.fade_end) = (audio.fade_end, audio.fade_start);
			let id = audio.id;
			self.node_action(self.tracks[track].id, NodeAction::ClipReverse(id));
		}
	}

	pub fn clip_normalize(&mut self, track: usize, clip: usize) {
		if let Clip::Audio(audio) = &mut self.tracks[track].clips[clip] {
			let sample = &self.samples[&audio.sample];
			let max_abs = sample.lods.max_abs();
			if max_abs != 0.0 {
				self.clip_volume_changed(track, clip, 1.0 / max_abs);
			}
		}
	}

	pub fn clip_slip_to(&mut self, track: usize, clip: usize, pos: BeatTime) {
		if self.tracks[track].clips[clip].offset(&self.transport) != pos {
			self.tracks[track].clips[clip].slip_to(pos, &self.transport);
			self.node_action(
				self.tracks[track].id,
				NodeAction::ClipSlipTo(self.tracks[track].clips[clip].id(), pos),
			);
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

	fn insert_note(&mut self, pattern: MidiPatternId, note: MidiNote, index: usize) -> usize {
		self.midi_patterns
			.get_mut(&pattern)
			.unwrap()
			.notes
			.insert(index, note);
		self.midi_pattern_action(pattern, MidiPatternAction::Add(note));
		index
	}

	pub fn remove_note(&mut self, pattern: MidiPatternId, note: usize) -> MidiNote {
		let note = self
			.midi_patterns
			.get_mut(&pattern)
			.unwrap()
			.notes
			.remove(note);
		self.midi_pattern_action(pattern, MidiPatternAction::Remove(note.id));
		note
	}

	pub fn note_change_velocity(&mut self, pattern: MidiPatternId, note: usize, velocity: f32) {
		if self.midi_patterns[&pattern].notes[note].velocity != velocity {
			self.midi_patterns.get_mut(&pattern).unwrap().notes[note].velocity = velocity;
			self.midi_pattern_action(
				pattern,
				MidiPatternAction::ChangeVelocity(
					self.midi_patterns[&pattern].notes[note].id,
					velocity,
				),
			);
		}
	}

	pub fn note_change_key(&mut self, pattern: MidiPatternId, note: usize, key: MidiKey) {
		if self.midi_patterns[&pattern].notes[note].key != key {
			self.midi_patterns.get_mut(&pattern).unwrap().notes[note].key = key;
			self.midi_pattern_action(
				pattern,
				MidiPatternAction::ChangeKey(self.midi_patterns[&pattern].notes[note].id, key),
			);
		}
	}

	pub fn note_move_to(&mut self, pattern: MidiPatternId, note: usize, pos: BeatTime) {
		if self.midi_patterns[&pattern].notes[note].position.start() != pos {
			self.midi_patterns.get_mut(&pattern).unwrap().notes[note]
				.position
				.move_to(pos);
			self.midi_pattern_action(
				pattern,
				MidiPatternAction::MoveTo(self.midi_patterns[&pattern].notes[note].id, pos),
			);
		}
	}

	pub fn note_trim_start_to(&mut self, pattern: MidiPatternId, note: usize, pos: BeatTime) {
		if self.midi_patterns[&pattern].notes[note].position.start() != pos {
			self.midi_patterns.get_mut(&pattern).unwrap().notes[note]
				.position
				.trim_start_to(pos);
			self.midi_pattern_action(
				pattern,
				MidiPatternAction::TrimStartTo(self.midi_patterns[&pattern].notes[note].id, pos),
			);
		}
	}

	pub fn note_trim_end_to(&mut self, pattern: MidiPatternId, note: usize, pos: BeatTime) {
		if self.midi_patterns[&pattern].notes[note].position.end() != pos {
			self.midi_patterns.get_mut(&pattern).unwrap().notes[note]
				.position
				.trim_end_to(pos);
			self.midi_pattern_action(
				pattern,
				MidiPatternAction::TrimEndTo(self.midi_patterns[&pattern].notes[note].id, pos),
			);
		}
	}

	pub fn render(&mut self, path: Arc<Path>) -> Task<daw::Message> {
		let (progress_sender, progress_receiver) = smol::channel::unbounded();

		let beat_range = self.transport.loop_range.unwrap_or_else(|| {
			BeatRange::new(
				BeatTime::ZERO,
				self.tracks()
					.iter()
					.map(|track| track.len(&self.transport))
					.max()
					.unwrap_or_default(),
			)
		});

		let master = self.master;

		let (a_sender, p_receiver) = oneshot::channel();
		let (p_sender, a_receiver) = oneshot::channel();
		self.send(Message::RequestProcessor(a_sender, a_receiver));

		Task::batch([
			Task::future(unblock(move || {
				let mut processor = p_receiver.recv().unwrap();
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
				.map(|progress| daw::Message::Progress(progress as f32))
				.chain(Task::done(daw::Message::RenderedFile)),
		])
	}

	pub fn freeze(
		&mut self,
		node: NodeId,
		path: Arc<Path>,
		project: daw::Project,
	) -> Task<daw::Message> {
		let beat_range = if let Some(loop_range) = self.transport.loop_range {
			loop_range
		} else {
			let Some((start, end)) = self.tracks()[self.track_of(node).unwrap()]
				.clips
				.iter()
				.map(|clip| (clip.start(), clip.end(&self.transport)))
				.reduce(|l, r| (l.0.min(r.0), l.1.max(r.1)))
			else {
				return Task::done(daw::Message::RenderedFile);
			};
			BeatRange::new(start, end)
		};

		let (progress_sender, progress_receiver) = smol::channel::unbounded();
		let transport = self.transport;

		let (a_sender, p_receiver) = oneshot::channel();
		let (p_sender, a_receiver) = oneshot::channel();
		self.send(Message::RequestProcessor(a_sender, a_receiver));

		Task::batch([
			Task::future(unblock(move || {
				let mut samples = Vec::with_capacity(beat_range.len().to_frames(&transport));

				let mut processor = p_receiver.recv().unwrap();
				processor.render(
					&path,
					node,
					beat_range,
					|buf| samples.extend_from_slice(buf),
					|f| progress_sender.try_send(f).unwrap(),
				);
				p_sender.send(processor).unwrap();

				daw::Message::Arrangement(
					project,
					arrangement_view::Message::FreezeDone(
						node,
						Box::new(
							SamplePair::from_core(
								generic_daw_core::Sample {
									id: SampleId::unique(),
									samples: NoDebug(samples.into()),
									sample_rate: transport.sample_rate,
								},
								path,
							)
							.unwrap(),
						),
						beat_range.start(),
					),
				)
			})),
			Task::stream(progress_receiver)
				.map(|progress| daw::Message::Progress(progress as f32))
				.chain(Task::done(daw::Message::RenderedFile)),
		])
	}
}
