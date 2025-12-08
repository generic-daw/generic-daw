use crate::{
	AudioGraph, AudioGraphNode, AutomationPattern, AutomationPatternAction, AutomationPatternId,
	Channel, Clip, Event, Export, MidiPattern, MidiPatternAction, MidiPatternId, MusicalTime,
	NodeId, NotePosition, PanMode, Sample, SampleId,
	clap_host::{AudioProcessor, ClapId, PluginId},
	resampler::Resampler,
};
use log::{trace, warn};
use rtrb::{Consumer, Producer, PushError, RingBuffer};
use std::{num::NonZero, time::Instant};
use utils::{HoleyVec, NoDebug, include_f32s, unique_id};

unique_id!(version);

pub use version::Id as Version;

static ON_BAR_CLICK: [f32; 2940] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: [f32; 2940] = include_f32s!("../../assets/off_bar_click.pcm");

#[derive(Debug)]
pub enum Message {
	NodeAction(NodeId, NodeAction),
	MidiPatternAction(MidiPatternId, MidiPatternAction),
	AutomationPatternAction(AutomationPatternId, AutomationPatternAction),

	SampleAdd(Sample),
	SampleRemove(SampleId),
	MidiPatternAdd(MidiPattern),
	MidiPatternRemove(MidiPatternId),

	NodeAdd(Box<AudioGraphNode>),
	NodeRemove(NodeId),
	NodeConnect(NodeId, NodeId, oneshot::Sender<(NodeId, NodeId)>),
	NodeDisconnect(NodeId, NodeId),

	Bpm(NonZero<u16>),
	Numerator(NonZero<u8>),
	TogglePlayback,
	ToggleMetronome,
	Sample(Version, usize),
	LoopMarker(Option<NotePosition>),
	Reset,

	ReturnUpdateBuffer(Vec<Update>),

	RequestAudioGraph(oneshot::Sender<Export>),
	AudioGraph(Box<Export>),
}

const _: () = assert!(size_of::<Message>() == 56);

#[derive(Debug)]
pub enum NodeAction {
	ClipAdd(Clip, usize),
	ClipRemove(usize),
	ClipMoveTo(usize, MusicalTime),
	ClipTrimStartTo(usize, MusicalTime),
	ClipTrimEndTo(usize, MusicalTime),

	ChannelToggleEnabled,
	ChannelToggleBypassed,
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
	pub version: Version,
	pub sample: Option<(usize, bool)>,
	pub updates: Vec<Update>,
	pub now: Instant,
}

#[derive(Clone, Copy, Debug)]
pub struct Transport {
	pub version: Version,
	pub sample_rate: NonZero<u32>,
	pub frames: NonZero<u32>,
	pub bpm: NonZero<u16>,
	pub numerator: NonZero<u8>,
	pub playing: bool,
	pub metronome: bool,
	pub sample: usize,
	pub loop_marker: Option<NotePosition>,
}

impl Transport {
	#[must_use]
	pub fn new(sample_rate: NonZero<u32>, frames: NonZero<u32>) -> Self {
		Self {
			version: Version::unique(),
			sample_rate,
			frames,
			bpm: NonZero::new(140).unwrap(),
			numerator: NonZero::new(4).unwrap(),
			playing: false,
			metronome: false,
			sample: 0,
			loop_marker: None,
		}
	}
}

#[derive(Debug)]
pub struct State {
	pub transport: Transport,
	pub samples: HoleyVec<Sample>,
	pub midi_patterns: HoleyVec<MidiPattern>,
	pub automation_patterns: HoleyVec<AutomationPattern>,
}

pub struct DawCtx {
	audio_graph: AudioGraph,
	state: State,
	producer: Producer<Batch>,
	consumer: Consumer<Message>,
	on_bar_click: NoDebug<Box<[f32]>>,
	off_bar_click: NoDebug<Box<[f32]>>,
	update_buffers: Vec<Vec<Update>>,
}

impl DawCtx {
	pub fn create(transport: Transport) -> (Self, NodeId, Producer<Message>, Consumer<Batch>) {
		let (r_producer, consumer) = RingBuffer::new(transport.frames.get() as usize);
		let (producer, r_consumer) =
			RingBuffer::new(transport.sample_rate.get().div_ceil(transport.frames.get()) as usize);

		let mut on_bar_click = Resampler::new(
			NonZero::new(44100).unwrap(),
			transport.sample_rate,
			NonZero::new(2).unwrap(),
		)
		.unwrap();
		on_bar_click.process(&ON_BAR_CLICK);

		let mut off_bar_click = Resampler::new(
			NonZero::new(44100).unwrap(),
			transport.sample_rate,
			NonZero::new(2).unwrap(),
		)
		.unwrap();
		off_bar_click.process(&OFF_BAR_CLICK);

		let daw_ctx = Self {
			audio_graph: AudioGraph::new(Channel::default(), transport.frames),
			state: State {
				transport,
				samples: HoleyVec::default(),
				midi_patterns: HoleyVec::default(),
				automation_patterns: HoleyVec::default(),
			},
			producer,
			consumer,
			on_bar_click: on_bar_click.finish().into_boxed_slice().into(),
			off_bar_click: off_bar_click.finish().into_boxed_slice().into(),
			update_buffers: Vec::new(),
		};

		let id = daw_ctx.audio_graph.root();
		(daw_ctx, id, r_producer, r_consumer)
	}

	fn recv_events(&mut self) {
		while let Ok(msg) = self.consumer.pop() {
			trace!("{msg:?}");

			match msg {
				Message::NodeAction(node, action) => self
					.audio_graph
					.for_node_mut(node, move |node| node.apply(action)),
				Message::MidiPatternAction(pattern, action) => {
					self.state
						.midi_patterns
						.get_mut(*pattern)
						.unwrap()
						.apply(action);
				}
				Message::AutomationPatternAction(pattern, action) => {
					self.state
						.automation_patterns
						.get_mut(*pattern)
						.unwrap()
						.apply(action);
				}
				Message::SampleAdd(sample) => {
					let sample = self.state.samples.insert(*sample.id, sample);
					debug_assert!(sample.is_none());
				}
				Message::SampleRemove(sample) => {
					let sample = self.state.samples.remove(*sample);
					debug_assert!(sample.is_some());
				}
				Message::MidiPatternAdd(pattern) => {
					let pattern = self.state.midi_patterns.insert(*pattern.id, pattern);
					debug_assert!(pattern.is_none());
				}
				Message::MidiPatternRemove(pattern) => {
					let pattern = self.state.midi_patterns.remove(*pattern);
					debug_assert!(pattern.is_some());
				}
				Message::NodeAdd(node) => self.audio_graph.insert(*node),
				Message::NodeRemove(node) => self.audio_graph.remove(node),
				Message::NodeConnect(from, to, sender) => {
					if self.audio_graph.connect(from, to) {
						sender.send((from, to)).unwrap();
					}
				}
				Message::NodeDisconnect(from, to) => self.audio_graph.disconnect(from, to),
				Message::Bpm(bpm) => self.state.transport.bpm = bpm,
				Message::Numerator(numerator) => self.state.transport.numerator = numerator,
				Message::TogglePlayback => self.state.transport.playing ^= true,
				Message::ToggleMetronome => self.state.transport.metronome ^= true,
				Message::Sample(version, sample) => {
					self.state.transport.version = version;
					self.state.transport.sample = sample;
				}
				Message::LoopMarker(loop_marker) => self.state.transport.loop_marker = loop_marker,
				Message::Reset => self.audio_graph.for_each_node_mut(AudioGraphNode::reset),
				Message::ReturnUpdateBuffer(update) => {
					debug_assert!(update.is_empty());
					self.update_buffers.push(update);
				}
				Message::RequestAudioGraph(sender) => {
					debug_assert!(self.consumer.is_empty());
					let mut audio_graph =
						AudioGraph::new(Channel::default(), self.state.transport.frames);
					std::mem::swap(&mut self.audio_graph, &mut audio_graph);

					let mut state = State {
						transport: self.state.transport,
						samples: HoleyVec::default(),
						midi_patterns: HoleyVec::default(),
						automation_patterns: HoleyVec::default(),
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
	}

	pub fn process(&mut self, mut buf: &mut [f32]) {
		self.recv_events();

		let mut looped = false;
		let mut updates = self.update_buffers.pop().unwrap_or_default();

		if self.state.transport.playing
			&& let Some(loop_marker) = self.state.transport.loop_marker
		{
			let loop_start = loop_marker.start().to_samples(&self.state.transport);
			let loop_end = loop_marker.end().to_samples(&self.state.transport);

			if loop_end >= self.state.transport.sample {
				while loop_end <= self.state.transport.sample + buf.len() {
					looped = true;
					let len = loop_end - self.state.transport.sample;

					self.audio_graph.process(&self.state, &mut buf[..len]);
					self.audio_graph
						.for_each_node_mut(|node| node.collect_updates(&mut updates));
					for s in &mut buf[..len] {
						*s = s.clamp(-1.0, 1.0);
					}
					self.metronome(&mut buf[..len], self.audio_graph.delay());

					self.state.transport.sample = loop_start;
					buf = &mut buf[len..];
				}
			}
		}

		self.audio_graph.process(&self.state, buf);
		self.audio_graph
			.for_each_node_mut(|node| node.collect_updates(&mut updates));
		for s in &mut *buf {
			*s = s.clamp(-1.0, 1.0);
		}
		self.metronome(buf, self.audio_graph.delay());

		let sample = self.state.transport.playing.then(|| {
			self.state.transport.sample += buf.len();
			(self.state.transport.sample, looped)
		});

		if sample.is_some() || !updates.is_empty() {
			let batch = Batch {
				version: self.state.transport.version,
				sample,
				updates,
				now: Instant::now(),
			};

			if let Err(PushError::Full(mut batch)) = self.producer.push(batch) {
				warn!("full ring buffer");
				batch.updates.clear();
				self.update_buffers.push(batch.updates);
			}
		}
	}

	fn metronome(&self, buf: &mut [f32], delay: usize) {
		if !self.state.transport.metronome || !self.state.transport.playing {
			return;
		}

		let mut start = MusicalTime::from_samples(
			self.state.transport.sample.saturating_sub(delay),
			&self.state.transport,
		)
		.beat_floor();
		let end = MusicalTime::from_samples(
			(self.state.transport.sample + buf.len()).saturating_sub(delay),
			&self.state.transport,
		)
		.beat_ceil();

		while start < end {
			let start_samples = start.to_samples(&self.state.transport) + delay;

			let click = if start
				.beat()
				.is_multiple_of(self.state.transport.numerator.get().into())
			{
				&**self.on_bar_click
			} else {
				&**self.off_bar_click
			};

			start += MusicalTime::BEAT;

			let buf_idx = start_samples.saturating_sub(self.state.transport.sample);
			let click_idx = self.state.transport.sample.saturating_sub(start_samples);

			if buf_idx >= buf.len() || click_idx >= click.len() {
				continue;
			}

			buf[buf_idx..]
				.iter_mut()
				.zip(&click[click_idx..])
				.for_each(|(buf, sample)| *buf += sample);
		}
	}
}
