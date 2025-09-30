use crate::{
	AudioGraph, AudioGraphNode, Channel, Clip, Event, Export, Master, MidiKey, MidiNote,
	MusicalTime, PanMode, Pattern, PatternId, Sample, SampleId,
};
use audio_graph::{NodeId, NodeImpl as _};
use clap_host::{AudioProcessor, ClapId, PluginId};
use generic_daw_utils::{HoleyVec, unique_id};
use log::{trace, warn};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Mutex;

unique_id!(epoch);
unique_id!(version);

pub use epoch::Id as Epoch;
pub use version::Id as Version;

#[derive(Debug)]
pub enum Message {
	NodeAction(NodeId, NodeAction),
	PatternAction(PatternId, PatternAction),

	SampleAdd(Sample),
	SampleRemove(SampleId),
	PatternAdd(Pattern),
	PatternRemove(PatternId),

	NodeAdd(Box<AudioGraphNode>),
	NodeRemove(NodeId),
	NodeConnect(NodeId, NodeId, oneshot::Sender<(NodeId, NodeId)>),
	NodeDisconnect(NodeId, NodeId),

	Bpm(u16),
	Numerator(u8),
	TogglePlayback,
	ToggleMetronome,
	Sample(Version, usize),
	Reset,

	ReturnUpdateBuffer(Vec<Update>),

	RequestAudioGraph(oneshot::Sender<Export>),
	AudioGraph(Box<Export>),
}

const _: () = assert!(size_of::<Message>() <= 128);

#[derive(Clone, Copy, Debug)]
pub enum PatternAction {
	Add(MidiNote),
	Remove(usize),
	ChangeKey(usize, MidiKey),
	MoveTo(usize, MusicalTime),
	TrimStartTo(usize, MusicalTime),
	TrimEndTo(usize, MusicalTime),
}

#[derive(Debug)]
pub enum NodeAction {
	ClipAdd(Clip),
	ClipRemove(usize),
	ClipMoveTo(usize, MusicalTime),
	ClipTrimStartTo(usize, MusicalTime),
	ClipTrimEndTo(usize, MusicalTime),

	ChannelToggleEnabled,
	ChannelToggleBypassed,
	ChannelTogglePolarity,
	ChannelSwapChannels,
	ChannelVolumeChanged(f32),
	ChannelPanChanged(PanMode),

	PluginLoad(Box<AudioProcessor<Event>>),
	PluginRemove(usize),
	PluginMoveTo(usize, usize),
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
	pub epoch: Epoch,
	pub version: Version,
	pub sample: Option<usize>,
	pub updates: Vec<Update>,
}

#[derive(Clone, Copy, Debug)]
pub struct RtState {
	pub epoch: Epoch,
	pub version: Version,
	pub sample_rate: u32,
	pub frames: u32,
	pub bpm: u16,
	pub numerator: u8,
	pub playing: bool,
	pub metronome: bool,
	pub sample: usize,
}

impl RtState {
	#[must_use]
	pub fn new(sample_rate: u32, frames: u32) -> Self {
		Self {
			epoch: Epoch::unique(),
			version: Version::unique(),
			sample_rate,
			frames,
			bpm: 140,
			numerator: 4,
			playing: false,
			metronome: false,
			sample: 0,
		}
	}
}

#[derive(Debug)]
pub struct State {
	pub rtstate: RtState,
	pub samples: HoleyVec<Sample>,
	pub patterns: HoleyVec<Pattern>,
	pub updates: Mutex<Vec<Update>>,
}

pub struct DawCtx {
	audio_graph: AudioGraph,
	state: State,
	producer: Producer<Batch>,
	consumer: Consumer<Message>,
	update_buffers: Vec<Vec<Update>>,
}

impl DawCtx {
	pub fn create(rtstate: RtState) -> (Self, NodeId, Producer<Message>, Consumer<Batch>) {
		let (r_sender, consumer) = RingBuffer::new(rtstate.frames as usize);
		let (producer, r_receiver) =
			RingBuffer::new(rtstate.sample_rate.div_ceil(rtstate.frames) as usize);

		let master = Master::new(rtstate.sample_rate);
		let id = master.id();

		let audio_ctx = Self {
			audio_graph: AudioGraph::new(master.into(), rtstate.frames),
			state: State {
				rtstate,
				samples: HoleyVec::default(),
				patterns: HoleyVec::default(),
				updates: Mutex::new(Vec::new()),
			},
			producer,
			consumer,
			update_buffers: Vec::new(),
		};

		(audio_ctx, id, r_sender, r_receiver)
	}

	pub fn process(&mut self, buf: &mut [f32]) {
		while let Ok(msg) = self.consumer.pop() {
			trace!("{msg:?}");

			match msg {
				Message::NodeAction(node, action) => self
					.audio_graph
					.with_mut_node(node, move |node| node.apply(action)),
				Message::PatternAction(pattern, action) => {
					self.state.patterns.get_mut(*pattern).unwrap().apply(action);
				}
				Message::SampleAdd(sample) => _ = self.state.samples.insert(*sample.id, sample),
				Message::SampleRemove(sample) => _ = self.state.samples.remove(*sample),
				Message::PatternAdd(pattern) => {
					self.state.patterns.insert(*pattern.id, pattern);
				}
				Message::PatternRemove(pattern) => _ = self.state.patterns.remove(*pattern),
				Message::NodeAdd(node) => self.audio_graph.insert(*node),
				Message::NodeRemove(node) => self.audio_graph.remove(node),
				Message::NodeConnect(from, to, sender) => {
					if self.audio_graph.connect(from, to) {
						sender.send((from, to)).unwrap();
					}
				}
				Message::NodeDisconnect(from, to) => self.audio_graph.disconnect(from, to),
				Message::Bpm(bpm) => self.state.rtstate.bpm = bpm,
				Message::Numerator(numerator) => self.state.rtstate.numerator = numerator,
				Message::TogglePlayback => self.state.rtstate.playing ^= true,
				Message::ToggleMetronome => self.state.rtstate.metronome ^= true,
				Message::Sample(version, sample) => {
					self.state.rtstate.version = version;
					self.state.rtstate.sample = sample;
				}
				Message::Reset => self.audio_graph.for_each_mut_node(AudioGraphNode::reset),
				Message::ReturnUpdateBuffer(update) => {
					debug_assert!(update.is_empty());
					self.update_buffers.push(update);
				}
				Message::RequestAudioGraph(sender) => {
					debug_assert!(self.consumer.is_empty());
					let mut audio_graph =
						AudioGraph::new(Channel::default().into(), self.state.rtstate.frames);
					std::mem::swap(&mut self.audio_graph, &mut audio_graph);

					let mut state = State {
						rtstate: self.state.rtstate,
						patterns: HoleyVec::default(),
						samples: HoleyVec::default(),
						updates: Mutex::default(),
					};
					std::mem::swap(&mut self.state, &mut state);

					sender.send(Export { audio_graph, state }).unwrap();
				}
				Message::AudioGraph(export) => {
					self.audio_graph = export.audio_graph;
					self.state = export.state;
				}
			}
		}

		let updates = self.state.updates.get_mut().unwrap();

		if updates.capacity() == 0 {
			*updates = self.update_buffers.pop().unwrap_or_default();
		}

		self.audio_graph.process(&self.state, buf);

		for s in &mut *buf {
			*s = s.clamp(-1.0, 1.0);
		}

		let sample = self.state.rtstate.playing.then(|| {
			self.state.rtstate.sample += buf.len();
			self.state.rtstate.sample
		});

		let updates = self.state.updates.get_mut().unwrap();

		if sample.is_some() || !updates.is_empty() {
			let batch = Batch {
				epoch: self.state.rtstate.epoch,
				version: self.state.rtstate.version,
				sample,
				updates: std::mem::take(updates),
			};

			if let Err(err) = self.producer.push(batch) {
				warn!("{err}");
			}
		}
	}
}
