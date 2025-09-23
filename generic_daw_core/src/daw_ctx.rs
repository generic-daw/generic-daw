use crate::{AudioGraphNode, Channel, Clip, Event, Master};
use audio_graph::{AudioGraph, NodeId, NodeImpl as _};
use clap_host::{AudioProcessor, ClapId, PluginId};
use crossbeam_queue::SegQueue;
use generic_daw_utils::unique_id;
use log::{trace, warn};
use rtrb::{Consumer, Producer, RingBuffer};

unique_id!(version);

pub use version::Id as Version;

#[derive(Debug)]
pub enum Message {
	Action(NodeId, Action),

	Insert(Box<AudioGraphNode>),
	Remove(NodeId),
	Connect(NodeId, NodeId, oneshot::Sender<(NodeId, NodeId)>),
	Disconnect(NodeId, NodeId),

	Bpm(u16),
	Numerator(u8),
	TogglePlayback,
	ToggleMetronome,
	Reset,
	Sample(Version, usize),

	ReturnUpdateBuffer(Vec<Update>),

	RequestAudioGraph(oneshot::Sender<AudioGraph<AudioGraphNode>>),
	AudioGraph(Box<AudioGraph<AudioGraphNode>>),
}

const _: () = assert!(size_of::<Message>() <= 128);

#[derive(Debug)]
pub enum Action {
	AddClip(Clip),
	RemoveClip(usize),

	ChannelToggleEnabled,
	ChannelToggleBypassed,
	ChannelTogglePolarity,
	ChannelSwapChannels,
	ChannelVolumeChanged(f32),
	ChannelPanChanged(f32),
	PluginLoad(Box<AudioProcessor<Event>>),
	PluginRemove(usize),
	PluginMoved(usize, usize),
	PluginToggleEnabled(usize),
	PluginMixChanged(usize, f32),
}

#[derive(Clone, Copy, Debug)]
pub enum Update {
	Peak(NodeId, [f32; 2]),
	Param(PluginId, ClapId),
}

#[derive(Clone, Debug)]
pub struct Batch {
	pub version: Version,
	pub sample: Option<usize>,
	pub updates: Vec<Update>,
}

#[derive(Clone, Copy, Debug)]
pub struct RtState {
	pub sample_rate: u32,
	pub frames: u32,
	pub bpm: u16,
	pub numerator: u8,
	pub playing: bool,
	pub metronome: bool,
	pub sample: usize,
}

#[derive(Debug)]
pub struct State {
	pub rtstate: RtState,
	pub updates: SegQueue<Update>,
}

impl From<RtState> for State {
	fn from(value: RtState) -> Self {
		Self {
			rtstate: value,
			updates: SegQueue::new(),
		}
	}
}

pub struct DawCtx {
	audio_graph: AudioGraph<AudioGraphNode>,
	state: State,
	version: Version,
	producer: Producer<Batch>,
	consumer: Consumer<Message>,
	update_buffers: Vec<Vec<Update>>,
}

impl DawCtx {
	pub fn create(
		sample_rate: u32,
		frames: u32,
	) -> (Self, NodeId, RtState, Producer<Message>, Consumer<Batch>) {
		let (r_sender, consumer) = RingBuffer::new(frames as usize);
		let (producer, r_receiver) = RingBuffer::new(sample_rate.div_ceil(frames) as usize);

		let master = Master::new(sample_rate);
		let id = master.id();

		let version = Version::unique();

		let rtstate = RtState {
			sample_rate,
			frames,
			bpm: 140,
			numerator: 4,
			playing: false,
			metronome: false,
			sample: 0,
		};

		let audio_ctx = Self {
			audio_graph: AudioGraph::new(master.into(), frames),
			state: State {
				rtstate,
				updates: SegQueue::new(),
			},
			version,
			producer,
			consumer,
			update_buffers: Vec::new(),
		};

		(audio_ctx, id, rtstate, r_sender, r_receiver)
	}

	pub fn process(&mut self, buf: &mut [f32]) {
		while let Ok(msg) = self.consumer.pop() {
			trace!("{msg:?}");

			match msg {
				Message::Action(node, action) => self
					.audio_graph
					.with_mut_node(node, move |node| node.apply(action)),
				Message::Insert(node) => self.audio_graph.insert(*node),
				Message::Remove(node) => self.audio_graph.remove(node),
				Message::Connect(from, to, sender) => {
					if self.audio_graph.connect(from, to) {
						sender.send((from, to)).unwrap();
					}
				}
				Message::Disconnect(from, to) => self.audio_graph.disconnect(from, to),
				Message::Bpm(bpm) => self.state.rtstate.bpm = bpm,
				Message::Numerator(numerator) => self.state.rtstate.numerator = numerator,
				Message::TogglePlayback => self.state.rtstate.playing ^= true,
				Message::ToggleMetronome => self.state.rtstate.metronome ^= true,
				Message::Reset => self.audio_graph.for_each_mut_node(AudioGraphNode::reset),
				Message::Sample(version, sample) => {
					self.state.rtstate.sample = sample;
					self.version = version;
				}
				Message::ReturnUpdateBuffer(update) => {
					debug_assert!(update.is_empty());
					self.update_buffers.push(update);
				}
				Message::RequestAudioGraph(sender) => {
					debug_assert!(self.consumer.is_empty());
					let mut audio_graph =
						AudioGraph::new(Channel::default().into(), self.state.rtstate.frames);
					std::mem::swap(&mut self.audio_graph, &mut audio_graph);
					sender.send(audio_graph).unwrap();
				}
				Message::AudioGraph(audio_graph) => self.audio_graph = *audio_graph,
			}
		}

		self.audio_graph.process(&self.state, buf);

		for s in &mut *buf {
			*s = s.clamp(-1.0, 1.0);
		}

		let sample = self.state.rtstate.playing.then(|| {
			self.state.rtstate.sample += buf.len();
			self.state.rtstate.sample
		});

		if sample.is_some() || !self.state.updates.is_empty() {
			let mut batch = Batch {
				version: self.version,
				sample,
				updates: self.update_buffers.pop().unwrap_or_default(),
			};
			while let Some(update) = self.state.updates.pop() {
				batch.updates.push(update);
			}
			if let Err(err) = self.producer.push(batch) {
				warn!("{err}");
			}
		}
	}
}
