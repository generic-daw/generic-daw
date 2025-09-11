use super::{
	node::{Node, NodeType},
	plugin::Plugin,
	track::Track,
};
use crate::{
	arrangement_view::poll_consumer, clap_host::Message as ClapHostMessage, config::Config,
};
use bit_set::BitSet;
use generic_daw_core::{
	self as core, Action, Batch, Clip, Event, Message, Mixer, MusicalTime, NodeId, NodeImpl as _,
	RtState, Stream, StreamTrait as _, Update, Version, build_output_stream,
	clap_host::{AudioProcessor, MainThreadMessage, PluginId},
	export,
};
use generic_daw_utils::{HoleyVec, NoDebug, ShiftMoveExt as _};
use iced::Task;
use rtrb::Producer;
use std::{path::Path, time::Instant};

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
		let (stream, master_node_id, rtstate, sender, consumer) = build_output_stream(
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

				producer: sender,
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
						self.nodes.get_mut(*node).unwrap().0.update(peaks, now);
						None
					}
					Update::Param(id, param_id, value) => Some(ClapHostMessage::MainThread(
						id,
						MainThreadMessage::ParamChanged(param_id, value),
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

	fn action(&mut self, node: NodeId, action: Action) {
		self.producer.push(Message::Action(node, action)).unwrap();
	}

	pub fn node_volume_changed(&mut self, node: NodeId, volume: f32) {
		self.nodes.get_mut(*node).unwrap().0.volume = volume;
		self.action(node, Action::NodeVolumeChanged(volume));
	}

	pub fn node_pan_changed(&mut self, node: NodeId, pan: f32) {
		self.nodes.get_mut(*node).unwrap().0.pan = pan;
		self.action(node, Action::NodePanChanged(pan));
	}

	pub fn node_toggle_enabled(&mut self, node: NodeId) {
		self.nodes.get_mut(*node).unwrap().0.enabled ^= true;
		self.action(node, Action::NodeToggleEnabled);
	}

	pub fn plugin_load(&mut self, node: NodeId, processor: AudioProcessor<Event>) {
		self.nodes
			.get_mut(*node)
			.unwrap()
			.0
			.plugins
			.push(Plugin::new(processor.id(), processor.descriptor().clone()));
		self.action(node, Action::PluginLoad(Box::new(processor)));
	}

	pub fn plugin_remove(&mut self, node: NodeId, index: usize) -> Plugin {
		let plugin = self.nodes.get_mut(*node).unwrap().0.plugins.remove(index);
		self.action(node, Action::PluginRemove(index));
		plugin
	}

	pub fn plugin_moved(&mut self, node: NodeId, from: usize, to: usize) {
		self.nodes
			.get_mut(*node)
			.unwrap()
			.0
			.plugins
			.shift_move(from, to);
		self.action(node, Action::PluginMoved(from, to));
	}

	pub fn plugin_toggle_enabled(&mut self, node: NodeId, index: usize) {
		self.nodes.get_mut(*node).unwrap().0.plugins[index].enabled ^= true;
		self.action(node, Action::PluginToggleEnabled(index));
	}

	pub fn plugin_mix_changed(&mut self, node: NodeId, index: usize, mix: f32) {
		self.nodes.get_mut(*node).unwrap().0.plugins[index].mix = mix;
		self.action(node, Action::PluginMixChanged(index, mix));
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

	pub fn master(&self) -> &(Node, BitSet) {
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
			let track = &self.tracks[i];

			if self.nodes.get_mut(*track.id).unwrap().0.enabled == (track.id == id) {
				continue;
			}

			self.node_toggle_enabled(track.id);
		}
	}

	pub fn enable_all_tracks(&mut self) {
		for i in 0..self.tracks.len() {
			let track = &self.tracks[i];

			if self.nodes.get_mut(*track.id).unwrap().0.enabled {
				continue;
			}

			self.node_toggle_enabled(track.id);
		}
	}

	pub fn channels(&self) -> impl Iterator<Item = &Node> {
		self.nodes
			.values()
			.filter_map(|(node, _)| (node.ty == NodeType::Mixer).then_some(node))
	}

	pub fn plugins(&self) -> impl Iterator<Item = PluginId> {
		self.nodes
			.values()
			.flat_map(|(node, _)| node.plugins.iter().map(|plugin| plugin.id))
	}

	pub fn node(&self, id: NodeId) -> &(Node, BitSet) {
		&self.nodes[*id]
	}

	pub fn push_channel(&mut self, node: Mixer) {
		self.nodes.insert(
			*node.id(),
			(Node::new(NodeType::Mixer, node.id()), BitSet::default()),
		);
		self.producer
			.push(Message::Insert(Box::new(node.into())))
			.unwrap();
	}

	pub fn add_channel(&mut self) -> oneshot::Receiver<(NodeId, NodeId)> {
		let node = Mixer::default();
		let id = node.id();
		self.push_channel(node);
		self.request_connect(self.master_node_id, id)
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
		self.request_connect(self.master_node_id, id)
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
		self.nodes.get_mut(*to).unwrap().1.insert(*from);
	}

	pub fn disconnect(&mut self, from: NodeId, to: NodeId) {
		self.nodes.get_mut(*to).unwrap().1.remove(*from);
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

	pub fn export(&mut self, path: &Path) {
		let (sender, receiver) = oneshot::channel();
		self.producer
			.push(Message::RequestAudioGraph(sender))
			.unwrap();
		let mut audio_graph = receiver.recv().unwrap();
		self.stream.pause().unwrap();
		export(
			&mut audio_graph,
			path,
			self.rtstate,
			self.tracks()
				.iter()
				.map(Track::len)
				.max()
				.unwrap_or_default(),
		);
		self.producer
			.push(Message::AudioGraph(Box::new(audio_graph)))
			.unwrap();
		self.stream.play().unwrap();
	}
}
