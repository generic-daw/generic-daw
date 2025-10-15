use crate::{
	AudioGraph, AudioGraphNode, Channel, Clip, Event, Export, MidiKey, MidiNote, MusicalTime,
	NodeId, NotePosition, PanMode, Pattern, PatternId, Sample, SampleId,
	clap_host::{AudioProcessor, ClapId, PluginId},
	resampler::Resampler,
};
use generic_daw_utils::{HoleyVec, NoDebug, include_f32s, unique_id};
use log::{trace, warn};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Mutex;

unique_id!(epoch);
unique_id!(version);

pub use epoch::Id as Epoch;
pub use version::Id as Version;

static ON_BAR_CLICK: [f32; 2940] = include_f32s!("../../assets/on_bar_click.pcm");
static OFF_BAR_CLICK: [f32; 2940] = include_f32s!("../../assets/off_bar_click.pcm");

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
	LoopMarker(Option<NotePosition>),
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
	pub sample: Option<(usize, bool)>,
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
	pub loop_marker: Option<NotePosition>,
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
			loop_marker: None,
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
	on_bar_click: NoDebug<Box<[f32]>>,
	off_bar_click: NoDebug<Box<[f32]>>,
	update_buffers: Vec<Vec<Update>>,
}

impl DawCtx {
	pub fn create(rtstate: RtState) -> (Self, NodeId, Producer<Message>, Consumer<Batch>) {
		let (r_producer, consumer) = RingBuffer::new(rtstate.frames as usize);
		let (producer, r_consumer) =
			RingBuffer::new(rtstate.sample_rate.div_ceil(rtstate.frames) as usize);

		let mut on_bar_click = Resampler::new(44100, rtstate.sample_rate as usize, 2).unwrap();
		on_bar_click.process(&ON_BAR_CLICK);

		let mut off_bar_click = Resampler::new(44100, rtstate.sample_rate as usize, 2).unwrap();
		off_bar_click.process(&OFF_BAR_CLICK);

		let daw_ctx = Self {
			audio_graph: AudioGraph::new(Channel::default(), rtstate.frames),
			state: State {
				rtstate,
				samples: HoleyVec::default(),
				patterns: HoleyVec::default(),
				updates: Mutex::new(Vec::new()),
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
				Message::PatternAction(pattern, action) => {
					self.state.patterns.get_mut(*pattern).unwrap().apply(action);
				}
				Message::SampleAdd(sample) => {
					let sample = self.state.samples.insert(*sample.id, sample);
					debug_assert!(sample.is_none());
				}
				Message::SampleRemove(sample) => {
					let sample = self.state.samples.remove(*sample);
					debug_assert!(sample.is_some());
				}
				Message::PatternAdd(pattern) => {
					let pattern = self.state.patterns.insert(*pattern.id, pattern);
					debug_assert!(pattern.is_none());
				}
				Message::PatternRemove(pattern) => {
					let pattern = self.state.patterns.remove(*pattern);
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
				Message::Bpm(bpm) => self.state.rtstate.bpm = bpm,
				Message::Numerator(numerator) => self.state.rtstate.numerator = numerator,
				Message::TogglePlayback => self.state.rtstate.playing ^= true,
				Message::ToggleMetronome => self.state.rtstate.metronome ^= true,
				Message::Sample(version, sample) => {
					self.state.rtstate.version = version;
					self.state.rtstate.sample = sample;
				}
				Message::LoopMarker(loop_marker) => self.state.rtstate.loop_marker = loop_marker,
				Message::Reset => self.audio_graph.for_each_node_mut(AudioGraphNode::reset),
				Message::ReturnUpdateBuffer(update) => {
					debug_assert!(update.is_empty());
					self.update_buffers.push(update);
				}
				Message::RequestAudioGraph(sender) => {
					debug_assert!(self.consumer.is_empty());
					let mut audio_graph =
						AudioGraph::new(Channel::default(), self.state.rtstate.frames);
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
	}

	pub fn process(&mut self, mut buf: &mut [f32]) {
		self.recv_events();

		let mut looped = false;

		if self.state.rtstate.playing
			&& let Some(loop_marker) = self.state.rtstate.loop_marker
		{
			let loop_start = loop_marker.start().to_samples(&self.state.rtstate);
			let loop_end = loop_marker.end().to_samples(&self.state.rtstate);

			if loop_end >= self.state.rtstate.sample {
				while loop_end <= self.state.rtstate.sample + buf.len() {
					looped = true;
					let diff = loop_end - self.state.rtstate.sample;

					self.audio_graph.process(&self.state, &mut buf[..diff]);
					for s in &mut buf[..diff] {
						*s = s.clamp(-1.0, 1.0);
					}
					self.metronome(&mut buf[..diff]);

					self.state.rtstate.sample = loop_start;
					buf = &mut buf[diff..];
				}
			}
		}

		self.audio_graph.process(&self.state, buf);
		for s in &mut *buf {
			*s = s.clamp(-1.0, 1.0);
		}
		self.metronome(buf);

		let sample = self.state.rtstate.playing.then(|| {
			self.state.rtstate.sample += buf.len();
			(self.state.rtstate.sample, looped)
		});

		let updates = self.state.updates.get_mut().unwrap();

		if sample.is_some() || !updates.is_empty() {
			let batch = Batch {
				epoch: self.state.rtstate.epoch,
				version: self.state.rtstate.version,
				sample,
				updates: std::mem::take(updates),
			};

			*updates = self.update_buffers.pop().unwrap_or_default();

			if let Err(err) = self.producer.push(batch) {
				warn!("{err}");
			}
		}
	}

	fn metronome(&self, buf: &mut [f32]) {
		if !self.state.rtstate.metronome || !self.state.rtstate.playing {
			return;
		}

		let mut start =
			MusicalTime::from_samples(self.state.rtstate.sample, &self.state.rtstate).floor();
		let end =
			MusicalTime::from_samples(self.state.rtstate.sample + buf.len(), &self.state.rtstate)
				.ceil();

		while start < end {
			let start_samples = start.to_samples(&self.state.rtstate);

			let click = if start
				.beat()
				.is_multiple_of(u64::from(self.state.rtstate.numerator))
			{
				&**self.on_bar_click
			} else {
				&**self.off_bar_click
			};

			start += MusicalTime::BEAT;

			let buf_idx = start_samples.saturating_sub(self.state.rtstate.sample);
			let click_idx = self.state.rtstate.sample.saturating_sub(start_samples);

			if click_idx >= click.len() {
				continue;
			}

			buf[buf_idx..]
				.iter_mut()
				.zip(&click[click_idx..])
				.for_each(|(buf, sample)| *buf += sample);
		}
	}
}
