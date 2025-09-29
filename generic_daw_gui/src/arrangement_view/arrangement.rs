use super::{
	node::{Node, NodeType},
	plugin::Plugin,
	poll_consumer,
	track::Track,
};
use crate::{
	arrangement_view::{
		clip::Clip,
		pattern::{Pattern, PatternPair},
		sample::{Sample, SamplePair},
	},
	clap_host::Message as ClapHostMessage,
	config::Config,
	daw::Message as DawMessage,
};
use bit_set::BitSet;
use generic_daw_core::{
	self as core, AudioGraph, Batch, Event, Flags, Message, MidiKey, MidiNote, MusicalTime,
	NodeAction, NodeId, NodeImpl as _, PatternAction, PatternId, RtState, SampleId, Stream,
	StreamTrait as _, Update, Version, build_output_stream,
	clap_host::{AudioProcessor, MainThreadMessage, ParamRescanFlags},
	export,
};
use generic_daw_utils::{HoleyVec, NoClone, NoDebug, ShiftMoveExt as _};
use iced::Task;
use rtrb::Producer;
use smol::unblock;
use std::{path::Path, sync::Arc, time::Instant};

#[derive(Debug)]
pub struct Arrangement {
	rtstate: RtState,

	samples: HoleyVec<Sample>,
	patterns: HoleyVec<Pattern>,

	tracks: Vec<Track>,
	nodes: HoleyVec<(Node, BitSet)>,
	master_node_id: NodeId,

	producer: Producer<Message>,
	stream: NoDebug<Stream>,
}

impl Arrangement {
	pub fn create(config: &Config) -> (Self, Task<Batch>) {
		let (stream, master_node_id, rtstate, producer, consumer) = build_output_stream(
			config.output_device.name.as_deref(),
			config.output_device.sample_rate.unwrap_or(44100),
			config.output_device.buffer_size.unwrap_or(1024),
		);
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
				rtstate,

				samples: HoleyVec::default(),
				patterns: HoleyVec::default(),

				tracks: Vec::new(),
				nodes,
				master_node_id,

				producer,
				stream: stream.into(),
			},
			poll_consumer(consumer, rtstate.sample_rate, rtstate.frames),
		)
	}

	pub fn update(&mut self, mut update: Batch, now: Instant) -> Task<ClapHostMessage> {
		let task = if update.epoch == self.rtstate().epoch {
			if let Some(sample) = update.sample
				&& update.version.is_last()
			{
				self.rtstate.sample = sample;
			}

			Task::batch(
				update
					.updates
					.drain(..)
					.filter_map(|event| match event {
						Update::Peak(node, peaks) => {
							self.node_mut(node).update(peaks, now);
							None
						}
						Update::Param(id, param_id) => Some(ClapHostMessage::MainThread(
							id,
							MainThreadMessage::RescanParam(param_id, ParamRescanFlags::VALUES),
						)),
					})
					.map(Task::done),
			)
		} else {
			update.updates.clear();
			Task::none()
		};

		self.send(Message::ReturnUpdateBuffer(update.updates));

		task
	}

	pub fn rtstate(&self) -> &RtState {
		&self.rtstate
	}

	pub fn samples(&self) -> &HoleyVec<Sample> {
		&self.samples
	}

	pub fn patterns(&self) -> &HoleyVec<Pattern> {
		&self.patterns
	}

	fn send(&mut self, message: Message) {
		self.producer.push(message).unwrap();
	}

	fn node_action(&mut self, id: NodeId, action: NodeAction) {
		self.send(Message::NodeAction(id, action));
	}

	fn pattern_action(&mut self, id: PatternId, action: PatternAction) {
		self.send(Message::PatternAction(id, action));
	}

	pub fn channel_volume_changed(&mut self, id: NodeId, volume: f32) {
		self.node_mut(id).volume = volume;
		self.node_action(id, NodeAction::ChannelVolumeChanged(volume));
	}

	pub fn channel_pan_changed(&mut self, id: NodeId, pan: f32) {
		self.node_mut(id).pan = pan;
		self.node_action(id, NodeAction::ChannelPanChanged(pan));
	}

	pub fn channel_toggle_enabled(&mut self, id: NodeId) {
		self.node_mut(id).flags.toggle(Flags::ENABLED);
		self.node_action(id, NodeAction::ChannelToggleEnabled);
	}

	pub fn channel_toggle_bypassed(&mut self, id: NodeId) {
		self.node_mut(id).flags.toggle(Flags::BYPASSED);
		self.node_action(id, NodeAction::ChannelToggleBypassed);
	}

	pub fn channel_toggle_polarity(&mut self, id: NodeId) {
		self.node_mut(id).flags.toggle(Flags::POLARITY_INVERTED);
		self.node_action(id, NodeAction::ChannelTogglePolarity);
	}

	pub fn channel_swap_channels(&mut self, id: NodeId) {
		self.node_mut(id).flags.toggle(Flags::CHANNELS_SWAPPED);
		self.node_action(id, NodeAction::ChannelSwapChannels);
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

	pub fn seek_to(&mut self, position: MusicalTime) {
		let sample = position.to_samples(&self.rtstate);
		if self.rtstate.sample != sample {
			self.rtstate.sample = sample;
			self.send(Message::Sample(Version::unique(), sample));
		}
	}

	pub fn set_bpm(&mut self, bpm: u16) {
		if self.rtstate.bpm != bpm {
			self.rtstate.bpm = bpm;
			self.send(Message::Bpm(bpm));
		}
	}

	pub fn set_numerator(&mut self, numerator: u8) {
		if self.rtstate.numerator != numerator {
			self.rtstate.numerator = numerator;
			self.send(Message::Numerator(numerator));
		}
	}

	pub fn play(&mut self) {
		if !self.rtstate.playing {
			self.toggle_playback();
		}
	}

	pub fn pause(&mut self) {
		if self.rtstate.playing {
			self.toggle_playback();
		}
	}

	pub fn toggle_playback(&mut self) {
		self.rtstate.playing ^= true;
		self.send(Message::TogglePlayback);
	}

	pub fn stop(&mut self) {
		self.pause();
		self.seek_to(MusicalTime::ZERO);
		self.send(Message::Reset);
	}

	pub fn toggle_metronome(&mut self) {
		self.rtstate.metronome ^= true;
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

			if self.node_mut(track_id).flags.contains(Flags::ENABLED) == (id == track_id) {
				continue;
			}

			self.channel_toggle_enabled(track_id);
		}
	}

	pub fn enable_all_tracks(&mut self) {
		for i in 0..self.tracks.len() {
			let track_id = self.tracks[i].id;

			if self.node_mut(track_id).flags.contains(Flags::ENABLED) {
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

	pub fn add_channel(&mut self) -> NodeId {
		let channel = core::Channel::default();
		let id = channel.id();
		self.nodes
			.insert(*id, (Node::new(NodeType::Channel, id), BitSet::default()));
		self.send(Message::NodeAdd(Box::new(channel.into())));
		id
	}

	pub fn remove_channel(&mut self, id: NodeId) -> Node {
		let node = self.nodes.remove(*id).unwrap().0;
		self.send(Message::NodeRemove(id));
		node
	}

	pub fn add_track(&mut self) -> usize {
		let track = core::Track::default();
		let id = track.id();
		self.tracks.push(Track::new(id));
		self.nodes
			.insert(*id, (Node::new(NodeType::Track, id), BitSet::default()));
		self.send(Message::NodeAdd(Box::new(track.into())));
		self.tracks.len() - 1
	}

	pub fn remove_track(&mut self, idx: usize) {
		let track = self.tracks.remove(idx);
		for clip in track.clips {
			match clip {
				Clip::Audio(audio) => self.maybe_remove_sample(audio.sample),
				Clip::Midi(midi) => self.maybe_remove_pattern(midi.pattern),
			}
		}
	}

	pub fn request_connect(
		&mut self,
		from: NodeId,
		to: NodeId,
	) -> oneshot::Receiver<(NodeId, NodeId)> {
		let (sender, receiver) = oneshot::channel();
		self.send(Message::NodeConnect(from, to, sender));
		receiver
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

	pub fn maybe_remove_sample(&mut self, sample: SampleId) {
		if self
			.tracks
			.iter()
			.flat_map(|track| &track.clips)
			.all(|clip| match clip {
				Clip::Audio(audio) => audio.sample != sample,
				Clip::Midi(..) => true,
			}) {
			self.samples.remove(*sample);
			self.send(Message::SampleRemove(sample));
		}
	}

	pub fn add_pattern(&mut self, pattern: PatternPair) {
		self.patterns.insert(*pattern.gui.id, pattern.gui);
		self.send(Message::PatternAdd(pattern.core));
	}

	pub fn maybe_remove_pattern(&mut self, pattern: PatternId) {
		if self
			.tracks
			.iter()
			.flat_map(|track| &track.clips)
			.all(|clip| match clip {
				Clip::Audio(..) => true,
				Clip::Midi(midi) => midi.pattern != pattern,
			}) {
			self.patterns.remove(*pattern);
			self.send(Message::PatternRemove(pattern));
		}
	}

	pub fn add_clip(&mut self, track: usize, clip: impl Into<Clip>) -> usize {
		let clip = clip.into();
		self.tracks[track].clips.push(clip);
		self.node_action(self.tracks[track].id, NodeAction::ClipAdd(clip.into()));
		self.tracks[track].clips.len() - 1
	}

	pub fn remove_clip(&mut self, track: usize, clip: usize) -> Clip {
		self.node_action(self.tracks[track].id, NodeAction::ClipRemove(clip));
		self.tracks[track].clips.remove(clip)
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

	pub fn add_note(&mut self, pattern: PatternId, note: MidiNote) -> usize {
		self.patterns.get_mut(*pattern).unwrap().notes.push(note);
		self.pattern_action(pattern, PatternAction::Add(note));
		self.patterns[*pattern].notes.len() - 1
	}

	pub fn remove_note(&mut self, pattern: PatternId, note: usize) -> MidiNote {
		self.pattern_action(pattern, PatternAction::Remove(note));
		self.patterns.get_mut(*pattern).unwrap().notes.remove(note)
	}

	pub fn note_switch_key(&mut self, pattern: PatternId, note: usize, key: MidiKey) {
		self.patterns.get_mut(*pattern).unwrap().notes[note].key = key;
		self.pattern_action(pattern, PatternAction::ChangeKey(note, key));
	}

	pub fn note_move_to(&mut self, pattern: PatternId, note: usize, pos: MusicalTime) {
		self.patterns.get_mut(*pattern).unwrap().notes[note]
			.position
			.move_to(pos);
		self.pattern_action(pattern, PatternAction::MoveTo(note, pos));
	}

	pub fn note_trim_start_to(&mut self, pattern: PatternId, note: usize, pos: MusicalTime) {
		self.patterns.get_mut(*pattern).unwrap().notes[note]
			.position
			.trim_start_to(pos);
		self.pattern_action(pattern, PatternAction::TrimStartTo(note, pos));
	}

	pub fn note_trim_end_to(&mut self, pattern: PatternId, note: usize, pos: MusicalTime) {
		self.patterns.get_mut(*pattern).unwrap().notes[note]
			.position
			.trim_end_to(pos);
		self.pattern_action(pattern, PatternAction::TrimEndTo(note, pos));
	}

	pub fn start_export(&mut self, path: Arc<Path>) -> Task<DawMessage> {
		let (sender, receiver) = oneshot::channel();
		self.send(Message::RequestAudioGraph(sender));
		let mut audio_graph = receiver.recv().unwrap();
		self.stream.pause().unwrap();

		let (progress_sender, progress_receiver) = smol::channel::unbounded();
		let (audio_graph_sender, audio_graph_receiver) = oneshot::channel();

		let rtstate = self.rtstate;
		let len = self
			.tracks()
			.iter()
			.map(Track::len)
			.max()
			.unwrap_or_default();

		Task::batch([
			Task::future(unblock(move || {
				export(&mut audio_graph, &path, rtstate, len, |f| {
					progress_sender.try_send(f).unwrap();
				});
				audio_graph_sender.send(audio_graph).unwrap();
			}))
			.discard(),
			Task::stream(progress_receiver)
				.map(DawMessage::Progress)
				.chain(Task::perform(audio_graph_receiver, |audio_graph| {
					DawMessage::ExportedFile(NoClone(Box::new(audio_graph.unwrap())))
				})),
		])
	}

	pub fn finish_export(&mut self, audio_graph: AudioGraph) {
		self.send(Message::AudioGraph(Box::new(audio_graph)));
		self.stream.play().unwrap();
	}
}
