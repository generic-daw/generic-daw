use super::{
	node::{Node, NodeType},
	plugin::Plugin,
	poll_consumer,
	track::Track,
};
use crate::{clap_host::Message as ClapHostMessage, config::Config, daw::Message as DawMessage};
use bit_set::BitSet;
use generic_daw_core::{
	self as core, Action, AudioGraph, Batch, Clip, Event, Flags, Message, MusicalTime, NodeId,
	NodeImpl as _, RtState, Stream, StreamTrait as _, Update, Version, build_output_stream,
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
		if let Some(sample) = update.sample
			&& update.version.is_last()
		{
			self.rtstate.sample = sample;
		}

		let task = Task::batch(
			update
				.updates
				.drain(..)
				.filter_map(|event| match event {
					Update::Peak(node, peaks) => {
						if let Some(node) = self.nodes.get_mut(*node) {
							node.0.update(peaks, now);
						}
						None
					}
					Update::Param(id, param_id) => Some(ClapHostMessage::MainThread(
						id,
						MainThreadMessage::RescanParam(param_id, ParamRescanFlags::VALUES),
					)),
				})
				.map(Task::done),
		);

		self.producer
			.push(Message::ReturnUpdateBuffer(update.updates))
			.unwrap();

		task
	}

	pub fn rtstate(&self) -> &RtState {
		&self.rtstate
	}

	fn action(&mut self, id: NodeId, action: Action) {
		self.producer.push(Message::Action(id, action)).unwrap();
	}

	pub fn channel_volume_changed(&mut self, id: NodeId, volume: f32) {
		self.node_mut(id).volume = volume;
		self.action(id, Action::ChannelVolumeChanged(volume));
	}

	pub fn channel_pan_changed(&mut self, id: NodeId, pan: f32) {
		self.node_mut(id).pan = pan;
		self.action(id, Action::ChannelPanChanged(pan));
	}

	pub fn channel_toggle_enabled(&mut self, id: NodeId) {
		self.node_mut(id).flags.toggle(Flags::ENABLED);
		self.action(id, Action::ChannelToggleEnabled);
	}

	pub fn channel_toggle_bypassed(&mut self, id: NodeId) {
		self.node_mut(id).flags.toggle(Flags::BYPASSED);
		self.action(id, Action::ChannelToggleBypassed);
	}

	pub fn channel_toggle_polarity(&mut self, id: NodeId) {
		self.node_mut(id).flags.toggle(Flags::POLARITY_INVERTED);
		self.action(id, Action::ChannelTogglePolarity);
	}

	pub fn channel_swap_channels(&mut self, id: NodeId) {
		self.node_mut(id).flags.toggle(Flags::CHANNELS_SWAPPED);
		self.action(id, Action::ChannelSwapChannels);
	}

	pub fn plugin_load(&mut self, id: NodeId, processor: AudioProcessor<Event>) {
		self.node_mut(id)
			.plugins
			.push(Plugin::new(processor.id(), processor.descriptor().clone()));
		self.action(id, Action::PluginLoad(Box::new(processor)));
	}

	pub fn plugin_remove(&mut self, id: NodeId, index: usize) -> Plugin {
		let plugin = self.node_mut(id).plugins.remove(index);
		self.action(id, Action::PluginRemove(index));
		plugin
	}

	pub fn plugin_moved(&mut self, id: NodeId, from: usize, to: usize) {
		self.node_mut(id).plugins.shift_move(from, to);
		self.action(id, Action::PluginMoved(from, to));
	}

	pub fn plugin_toggle_enabled(&mut self, id: NodeId, index: usize) {
		self.node_mut(id).plugins[index].enabled ^= true;
		self.action(id, Action::PluginToggleEnabled(index));
	}

	pub fn plugin_mix_changed(&mut self, id: NodeId, index: usize, mix: f32) {
		self.node_mut(id).plugins[index].mix = mix;
		self.action(id, Action::PluginMixChanged(index, mix));
	}

	pub fn seek_to(&mut self, position: MusicalTime) {
		let sample = position.to_samples(&self.rtstate);
		if self.rtstate.sample == sample {
			return;
		}
		self.rtstate.sample = sample;
		self.producer
			.push(Message::Sample(Version::unique(), sample))
			.unwrap();
	}

	pub fn set_bpm(&mut self, bpm: u16) {
		if self.rtstate.bpm == bpm {
			return;
		}
		self.rtstate.bpm = bpm;
		self.producer.push(Message::Bpm(bpm)).unwrap();
	}

	pub fn set_numerator(&mut self, numerator: u8) {
		if self.rtstate.numerator == numerator {
			return;
		}
		self.rtstate.numerator = numerator;
		self.producer.push(Message::Numerator(numerator)).unwrap();
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
		self.producer.push(Message::TogglePlayback).unwrap();
	}

	pub fn stop(&mut self) {
		self.pause();
		self.seek_to(MusicalTime::ZERO);
		self.producer.push(Message::Reset).unwrap();
	}

	pub fn toggle_metronome(&mut self) {
		self.rtstate.metronome ^= true;
		self.producer.push(Message::ToggleMetronome).unwrap();
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

	pub fn push_channel(&mut self, channel: core::Channel) {
		self.nodes.insert(
			*channel.id(),
			(
				Node::new(NodeType::Channel, channel.id()),
				BitSet::default(),
			),
		);
		self.producer
			.push(Message::Insert(Box::new(channel.into())))
			.unwrap();
	}

	pub fn add_channel(&mut self) -> oneshot::Receiver<(NodeId, NodeId)> {
		let node = core::Channel::default();
		let id = node.id();
		self.push_channel(node);
		self.request_connect(id, self.master_node_id)
	}

	pub fn remove_channel(&mut self, id: NodeId) -> Node {
		let node = self.nodes.remove(*id).unwrap().0;
		self.producer.push(Message::Remove(id)).unwrap();
		node
	}

	pub fn push_track(&mut self, track: core::Track) {
		let mut track2 = Track::new(track.id());
		track2.clips.clone_from(&track.clips);
		self.tracks.push(track2);
		self.nodes.insert(
			*track.id(),
			(Node::new(NodeType::Track, track.id()), BitSet::default()),
		);
		self.producer
			.push(Message::Insert(Box::new(track.into())))
			.unwrap();
	}

	pub fn add_track(&mut self) -> oneshot::Receiver<(NodeId, NodeId)> {
		let track = core::Track::default();
		let id = track.id();
		self.push_track(track);
		self.request_connect(id, self.master_node_id)
	}

	pub fn remove_track(&mut self, idx: usize) {
		self.tracks.remove(idx);
	}

	pub fn request_connect(
		&mut self,
		from: NodeId,
		to: NodeId,
	) -> oneshot::Receiver<(NodeId, NodeId)> {
		let (sender, receiver) = oneshot::channel();
		self.producer
			.push(Message::Connect(from, to, sender))
			.unwrap();
		receiver
	}

	pub fn connect_succeeded(&mut self, from: NodeId, to: NodeId) {
		self.outgoing_mut(from).insert(*to);
	}

	pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
		self.outgoing_mut(from).remove(*to);
		self.producer.push(Message::Disconnect(from, to)).unwrap();
	}

	pub fn add_clip(&mut self, track: usize, clip: impl Into<Clip>) {
		let clip = clip.into();
		self.tracks[track].clips.push(clip.clone());
		self.action(self.tracks[track].id, Action::AddClip(clip));
	}

	pub fn clone_clip(&mut self, track: usize, clip: usize) {
		let clip = self.tracks[track].clips[clip].deep_clone();
		self.tracks[track].clips.push(clip.clone());
		self.action(self.tracks[track].id, Action::AddClip(clip));
	}

	pub fn remove_clip(&mut self, track: usize, clip: usize) {
		self.tracks[track].clips.remove(clip);
		self.action(self.tracks[track].id, Action::RemoveClip(clip));
	}

	pub fn clip_switch_track(&mut self, track: usize, clip_index: usize, new_track: usize) {
		let clip = self.tracks[track].clips.remove(clip_index);
		self.tracks[new_track].clips.push(clip.clone());
		self.action(self.tracks[track].id, Action::RemoveClip(clip_index));
		self.action(self.tracks[new_track].id, Action::AddClip(clip));
	}

	pub fn start_export(&mut self, path: Arc<Path>) -> Task<DawMessage> {
		let (sender, receiver) = oneshot::channel();
		self.producer
			.push(Message::RequestAudioGraph(sender))
			.unwrap();
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
		self.producer
			.push(Message::AudioGraph(Box::new(audio_graph)))
			.unwrap();
		self.stream.play().unwrap();
	}
}
