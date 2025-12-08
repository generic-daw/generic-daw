use crate::{
	arrangement_view::{
		Message as ArrangementMessage,
		clip::Clip,
		midi_pattern::{MidiPattern, MidiPatternPair},
		node::{Node, NodeType},
		plugin::Plugin,
		poll_consumer,
		sample::{Sample, SamplePair},
		track::Track,
	},
	clap_host::Message as ClapHostMessage,
	config::Config,
	daw::Message as DawMessage,
};
use bit_set::BitSet;
use generic_daw_core::{
	self as core, AudioGraphNode, Batch, Event, Export, Message, MidiKey, MidiNote,
	MidiPatternAction, MidiPatternId, MusicalTime, NodeAction, NodeId, NodeImpl, NotePosition,
	OutputRequest, OutputResponse, PanMode, STREAM_THREAD, StreamMessage, StreamToken, Transport,
	Update, Version,
	clap_host::{AudioProcessor, MainThreadMessage, ParamRescanFlags},
};
use iced::Task;
use rtrb::{Producer, PushError};
use smol::unblock;
use std::{num::NonZero, path::Path, sync::Arc};
use utils::{HoleyVec, NoClone, NoDebug, ShiftMoveExt as _};

#[derive(Debug)]
pub struct Arrangement {
	transport: Transport,

	samples: HoleyVec<Sample>,
	midi_patterns: HoleyVec<MidiPattern>,

	tracks: Vec<Track>,
	nodes: HoleyVec<(Node, BitSet)>,
	master_node_id: NodeId,

	producer: Producer<Message>,
	stream: NoDebug<StreamToken>,
}

impl Arrangement {
	pub fn create(config: &Config) -> (Self, Task<Batch>) {
		let (sender, receiver) = oneshot::channel();

		STREAM_THREAD
			.send(StreamMessage::Output(
				OutputRequest {
					device_name: config.output_device.name.clone(),
					sample_rate: config.output_device.sample_rate,
					frames: config.output_device.buffer_size,
					metrics: NoDebug(&|f| iced::debug::time_with("Output callback", f)),
				},
				sender,
			))
			.unwrap();

		let OutputResponse {
			master_node_id,
			transport,
			producer,
			consumer,
			token,
		} = receiver.recv().unwrap();

		let mut nodes = HoleyVec::default();
		nodes.insert(
			*master_node_id,
			(
				Node::new(NodeType::Master, master_node_id),
				BitSet::default(),
			),
		);

		(
			Self {
				transport,

				samples: HoleyVec::default(),
				midi_patterns: HoleyVec::default(),

				tracks: Vec::new(),
				nodes,
				master_node_id,

				producer,
				stream: token.into(),
			},
			poll_consumer(consumer, transport.sample_rate, Some(transport.frames)),
		)
	}

	pub fn update(&mut self, mut batch: Batch) -> Vec<ArrangementMessage> {
		let mut messages = Vec::new();

		if let Some((sample, looped)) = batch.sample
			&& batch.version.is_latest()
		{
			self.transport.sample = sample;
			if looped {
				messages.push(ArrangementMessage::RecordingEndStream);
			}
		}

		for update in batch.updates.drain(..) {
			match update {
				Update::Peak(node, peaks) => {
					if let Some((node, _)) = self.nodes.get_mut(*node) {
						node.update(peaks, batch.now);
					}
				}
				Update::Param(id, param_id) => {
					messages.push(ArrangementMessage::ClapHost(ClapHostMessage::MainThread(
						id,
						MainThreadMessage::RescanParam(param_id, ParamRescanFlags::VALUES),
					)));
				}
			}
		}

		self.send(Message::ReturnUpdateBuffer(batch.updates));

		messages
	}

	pub fn transport(&self) -> &Transport {
		&self.transport
	}

	pub fn samples(&self) -> &HoleyVec<Sample> {
		&self.samples
	}

	pub fn midi_patterns(&self) -> &HoleyVec<MidiPattern> {
		&self.midi_patterns
	}

	fn send(&mut self, mut message: Message) {
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

	pub fn plugin_load(&mut self, id: NodeId, processor: AudioProcessor<Event>) {
		self.node_mut(id)
			.plugins
			.push(Plugin::new(processor.id(), processor.descriptor().clone()));
		self.node_action(id, NodeAction::PluginLoad(Box::new(processor)));
	}

	pub fn plugin_remove(&mut self, id: NodeId, index: usize) -> Plugin {
		self.node_action(id, NodeAction::PluginRemove(index));
		self.node_mut(id).plugins.remove(index)
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

	pub fn set_loop_marker(&mut self, loop_marker: Option<NotePosition>) {
		if self.transport.loop_marker != loop_marker {
			self.transport.loop_marker = loop_marker;
			self.send(Message::LoopMarker(loop_marker));
		}
	}

	pub fn seek_to(&mut self, position: MusicalTime) {
		let sample = position.to_samples(&self.transport);
		if self.transport.sample != sample {
			self.transport.sample = sample;
			self.send(Message::Sample(Version::unique(), sample));
		}
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

	pub fn toggle_playback(&mut self) {
		self.transport.playing ^= true;
		self.send(Message::TogglePlayback);
	}

	pub fn stop(&mut self) {
		self.pause();
		self.seek_to(
			self.transport
				.loop_marker
				.map_or(MusicalTime::ZERO, NotePosition::start),
		);
		self.send(Message::Reset);
	}

	pub fn toggle_metronome(&mut self) {
		self.transport.metronome ^= true;
		self.send(Message::ToggleMetronome);
	}

	pub fn master(&self) -> &Node {
		self.node(self.master_node_id)
	}

	pub fn tracks(&self) -> &[Track] {
		&self.tracks
	}

	pub fn track_of(&self, id: NodeId) -> Option<usize> {
		self.tracks.iter().position(|t| t.id == id)
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
		&self.nodes[*id].0
	}

	fn node_mut(&mut self, id: NodeId) -> &mut Node {
		&mut self.nodes.get_mut(*id).unwrap().0
	}

	pub fn outgoing(&self, id: NodeId) -> &BitSet {
		&self.nodes[*id].1
	}

	fn outgoing_mut(&mut self, id: NodeId) -> &mut BitSet {
		&mut self.nodes.get_mut(*id).unwrap().1
	}

	fn add(&mut self, node: impl Into<AudioGraphNode> + NodeImpl, ty: NodeType) -> NodeId {
		let id = node.id();
		self.nodes
			.insert(*id, (Node::new(ty, id), BitSet::default()));
		self.send(Message::NodeAdd(Box::new(node.into())));
		id
	}

	pub fn add_channel(&mut self) -> NodeId {
		self.add(core::Channel::default(), NodeType::Channel)
	}

	pub fn remove_channel(&mut self, id: NodeId) -> Node {
		debug_assert!(self.track_of(id).is_none());
		let node = self.nodes.remove(*id).unwrap().0;
		for (_, outgoing) in self.nodes.values_mut() {
			outgoing.remove(*id);
		}
		self.send(Message::NodeRemove(id));
		node
	}

	pub fn add_track(&mut self) -> usize {
		let id = self.add(core::Track::default(), NodeType::Track);
		self.tracks.push(Track::new(id));
		self.tracks.len() - 1
	}

	pub fn remove_track(&mut self, id: NodeId) {
		let idx = self.track_of(id).unwrap();
		let track = self.tracks.remove(idx);
		self.remove_channel(id);
		for clip in track.clips {
			match clip {
				Clip::Audio(audio) => self.samples.get_mut(*audio.sample).unwrap().refs -= 1,
				Clip::Midi(midi) => self.midi_patterns.get_mut(*midi.pattern).unwrap().refs -= 1,
			}

			self.gc(clip);
		}
	}

	pub fn request_connect(&mut self, from: NodeId, to: NodeId) -> Task<(NodeId, NodeId)> {
		let (sender, receiver) = oneshot::channel();
		self.send(Message::NodeConnect(from, to, sender));
		Task::perform(receiver, Result::ok)
			.and_then(Task::done)
			.map(move |success| success.then_some((from, to)))
			.and_then(Task::done)
	}

	pub fn connect_succeeded(&mut self, from: NodeId, to: NodeId) {
		self.outgoing_mut(from).insert(*to);
	}

	pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
		self.outgoing_mut(from).remove(*to);
		self.send(Message::NodeDisconnect(from, to));
	}

	pub fn add_sample(&mut self, sample: SamplePair) {
		self.samples.insert(*sample.gui.id, sample.gui);
		self.send(Message::SampleAdd(sample.core));
	}

	pub fn gc(&mut self, clip: impl Into<Clip>) {
		match clip.into() {
			Clip::Audio(audio) => {
				if self.samples[*audio.sample].refs == 0 {
					self.samples.remove(*audio.sample);
					self.send(Message::SampleRemove(audio.sample));
				}
			}
			Clip::Midi(midi) => {
				if self.midi_patterns[*midi.pattern].refs == 0 {
					self.midi_patterns.remove(*midi.pattern);
					self.send(Message::MidiPatternRemove(midi.pattern));
				}
			}
		}
	}

	pub fn add_midi_pattern(&mut self, pattern: MidiPatternPair) {
		self.midi_patterns.insert(*pattern.gui.id, pattern.gui);
		self.send(Message::MidiPatternAdd(pattern.core));
	}

	pub fn add_clip(&mut self, track: usize, clip: impl Into<Clip>) -> usize {
		self.insert_clip(track, clip, self.tracks[track].clips.len())
	}

	pub fn insert_clip(&mut self, track: usize, clip: impl Into<Clip>, idx: usize) -> usize {
		let clip = clip.into();
		match clip {
			Clip::Audio(audio) => self.samples.get_mut(*audio.sample).unwrap().refs += 1,
			Clip::Midi(midi) => self.midi_patterns.get_mut(*midi.pattern).unwrap().refs += 1,
		}
		self.tracks[track].clips.insert(idx, clip);
		self.node_action(self.tracks[track].id, NodeAction::ClipAdd(clip.into(), idx));
		idx
	}

	pub fn remove_clip(&mut self, track: usize, clip: usize) -> Clip {
		self.node_action(self.tracks[track].id, NodeAction::ClipRemove(clip));
		let clip = self.tracks[track].clips.remove(clip);
		match clip {
			Clip::Audio(audio) => self.samples.get_mut(*audio.sample).unwrap().refs -= 1,
			Clip::Midi(midi) => self.midi_patterns.get_mut(*midi.pattern).unwrap().refs -= 1,
		}
		clip
	}

	pub fn clip_switch_track(&mut self, track: usize, clip: usize, new_track: usize) -> usize {
		let clip = self.remove_clip(track, clip);
		self.add_clip(new_track, clip)
	}

	pub fn clip_move_to(&mut self, track: usize, clip: usize, pos: MusicalTime) {
		self.tracks[track].clips[clip].move_to(pos);
		self.node_action(self.tracks[track].id, NodeAction::ClipMoveTo(clip, pos));
	}

	pub fn clip_trim_start_to(&mut self, track: usize, clip: usize, pos: MusicalTime) {
		self.tracks[track].clips[clip].trim_start_to(pos);
		self.node_action(
			self.tracks[track].id,
			NodeAction::ClipTrimStartTo(clip, pos),
		);
	}

	pub fn clip_trim_end_to(&mut self, track: usize, clip: usize, pos: MusicalTime) {
		self.tracks[track].clips[clip].trim_end_to(pos);
		self.node_action(self.tracks[track].id, NodeAction::ClipTrimEndTo(clip, pos));
	}

	pub fn add_note(&mut self, pattern: MidiPatternId, note: MidiNote) -> usize {
		self.insert_note(pattern, note, self.midi_patterns[*pattern].notes.len())
	}

	pub fn insert_note(&mut self, pattern: MidiPatternId, note: MidiNote, idx: usize) -> usize {
		self.midi_patterns
			.get_mut(*pattern)
			.unwrap()
			.notes
			.insert(idx, note);
		self.midi_pattern_action(pattern, MidiPatternAction::Add(note, idx));
		idx
	}

	pub fn remove_note(&mut self, pattern: MidiPatternId, note: usize) -> MidiNote {
		self.midi_pattern_action(pattern, MidiPatternAction::Remove(note));
		self.midi_patterns
			.get_mut(*pattern)
			.unwrap()
			.notes
			.remove(note)
	}

	pub fn note_switch_key(&mut self, pattern: MidiPatternId, note: usize, key: MidiKey) {
		self.midi_patterns.get_mut(*pattern).unwrap().notes[note].key = key;
		self.midi_pattern_action(pattern, MidiPatternAction::ChangeKey(note, key));
	}

	pub fn note_move_to(&mut self, pattern: MidiPatternId, note: usize, pos: MusicalTime) {
		self.midi_patterns.get_mut(*pattern).unwrap().notes[note]
			.position
			.move_to(pos);
		self.midi_pattern_action(pattern, MidiPatternAction::MoveTo(note, pos));
	}

	pub fn note_trim_start_to(&mut self, pattern: MidiPatternId, note: usize, pos: MusicalTime) {
		self.midi_patterns.get_mut(*pattern).unwrap().notes[note]
			.position
			.trim_start_to(pos);
		self.midi_pattern_action(pattern, MidiPatternAction::TrimStartTo(note, pos));
	}

	pub fn note_trim_end_to(&mut self, pattern: MidiPatternId, note: usize, pos: MusicalTime) {
		self.midi_patterns.get_mut(*pattern).unwrap().notes[note]
			.position
			.trim_end_to(pos);
		self.midi_pattern_action(pattern, MidiPatternAction::TrimEndTo(note, pos));
	}

	pub fn start_export(&mut self, path: Arc<Path>) -> Task<DawMessage> {
		let (sender, receiver) = oneshot::channel();
		self.send(Message::RequestAudioGraph(sender));
		let mut export = receiver.recv().unwrap();
		STREAM_THREAD
			.send(StreamMessage::Pause(self.stream.get_ref()))
			.unwrap();

		let (progress_sender, progress_receiver) = smol::channel::unbounded();
		let (export_sender, audio_graph_receiver) = oneshot::channel();

		let len = self
			.tracks()
			.iter()
			.map(Track::len)
			.max()
			.unwrap_or_default();

		Task::batch([
			Task::future(unblock(move || {
				export.export(&path, len, |f| {
					progress_sender.try_send(DawMessage::Progress(f)).unwrap();
				});
				export_sender.send(export).unwrap();
			}))
			.discard(),
			Task::stream(progress_receiver).chain(Task::perform(audio_graph_receiver, |export| {
				DawMessage::ExportedFile(NoClone(Box::new(export.unwrap())))
			})),
		])
	}

	pub fn finish_export(&mut self, export: Export) {
		self.send(Message::AudioGraph(Box::new(export)));
		STREAM_THREAD
			.send(StreamMessage::Play(self.stream.get_ref()))
			.unwrap();
	}
}
