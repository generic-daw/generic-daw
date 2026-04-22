use crate::{
	AudioGraph, AutomationPattern, AutomationPatternAction, AutomationPatternId, Channel, Clip,
	MidiPattern, MidiPatternAction, MidiPatternId, Node, NodeId, PanMode, PluginId, Sample,
	SampleId,
	clap_host::ClapId,
	time::{BeatRange, BeatTime, SecondsTime},
};
use audio_graph::NodeImpl as _;
use clap_host::{
	Cookie,
	events::{EventFlags, EventHeader, TransportEvent, TransportFlags},
};
use dsp::resample_cubic;
use hound::WavWriter;
use log::{trace, warn};
use rtrb::{Consumer, Producer, PushError, RingBuffer};
use std::{
	collections::HashMap,
	num::NonZero,
	path::Path,
	time::{Duration, Instant},
};
use utils::{boxed_slice, include_f32s, unique_id};

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
	AutomationPatternAdd(AutomationPattern),
	AutomationPatternRemove(AutomationPatternId),

	NodeAdd(Box<Node>),
	NodeRemove(NodeId),
	NodeConnect(NodeId, NodeId),
	NodeSetMix(NodeId, NodeId, f32),
	NodeDisconnect(NodeId, NodeId),

	Bpm(NonZero<u16>),
	Numerator(NonZero<u8>),
	TogglePlayback,
	ToggleMetronome,
	Position(Version, SecondsTime),
	LoopRange(Option<BeatRange>),
	Reset,

	RequestUpdate,
	ReturnUpdate(Vec<Update>),

	RequestProcessor(
		oneshot::Sender<AudioProcessor>,
		oneshot::Receiver<AudioProcessor>,
	),
}

const _: () = assert!(size_of::<Message>() == 64);

#[derive(Debug)]
pub enum NodeAction {
	ClipAdd(Clip, usize),
	ClipRemove(usize),
	ClipMoveTo(usize, BeatTime),
	ClipTrimStartTo(usize, BeatTime),
	ClipTrimEndTo(usize, BeatTime),
	ClipStretchStartTo(usize, BeatTime),
	ClipStretchEndTo(usize, BeatTime),
	ClipSlipTo(usize, BeatTime),

	ChannelToggleEnabled,
	ChannelToggleBypassed,
	ChannelVolumeChanged(f32),
	ChannelPanChanged(PanMode),

	PluginLoad(PluginId, Box<clap_host::AudioProcessor>),
	PluginRemove(usize),
	PluginMoveTo(usize, usize),
	PluginToggleEnabled(usize),
	PluginMixChanged(usize, f32),
	PluginParamChanged(usize, ClapId, f32, Cookie),
}

#[derive(Clone, Copy, Debug)]
pub enum Update {
	Peaks(NodeId, [f32; 2]),
	Polyphony(NodeId, usize),
	Param(PluginId, ClapId, f32),
	ConnectFailed(NodeId, NodeId),
	Load(Duration, usize),
}

#[derive(Clone, Debug)]
pub struct Batch {
	pub version: Version,
	pub position: SecondsTime,
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
	pub position: SecondsTime,
	pub loop_range: Option<BeatRange>,
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
			position: SecondsTime::ZERO,
			loop_range: None,
		}
	}

	#[must_use]
	pub fn as_clap(&self) -> TransportEvent {
		TransportEvent {
			header: EventHeader::new_core(0, EventFlags::empty()),
			flags: TransportFlags::HAS_TEMPO
				| TransportFlags::HAS_BEATS_TIMELINE
				| TransportFlags::HAS_SECONDS_TIMELINE
				| TransportFlags::HAS_TIME_SIGNATURE
				| if self.playing {
					TransportFlags::IS_PLAYING
				} else {
					TransportFlags::empty()
				} | if self.loop_range.is_some() {
				TransportFlags::IS_LOOP_ACTIVE
			} else {
				TransportFlags::empty()
			},
			song_pos_beats: self.position.to_beat_time(self).to_clap(),
			song_pos_seconds: self.position.to_clap(),
			tempo: self.bpm.get().into(),
			tempo_inc: 0.0,
			loop_start_beats: self
				.loop_range
				.map(|loop_range| loop_range.start().to_clap())
				.unwrap_or_default(),
			loop_end_beats: self
				.loop_range
				.map(|loop_range| loop_range.end().to_clap())
				.unwrap_or_default(),
			loop_start_seconds: self
				.loop_range
				.map(|loop_range| loop_range.start().to_seconds_time(self).to_clap())
				.unwrap_or_default(),
			loop_end_seconds: self
				.loop_range
				.map(|loop_range| loop_range.end().to_seconds_time(self).to_clap())
				.unwrap_or_default(),
			bar_start: self.position.to_beat_time(self).bar_floor(self).to_clap(),
			bar_number: self.position.to_beat_time(self).bar(self) as i32,
			time_signature_numerator: self.numerator.get().into(),
			time_signature_denominator: 4,
		}
	}
}

#[derive(Debug)]
pub struct State {
	pub transport: Transport,
	pub samples: HashMap<SampleId, Sample>,
	pub midi_patterns: HashMap<MidiPatternId, MidiPattern>,
	pub automation_patterns: HashMap<AutomationPatternId, AutomationPattern>,
}

#[derive(Debug)]
pub struct AudioProcessor {
	audio_graph: AudioGraph,
	master: NodeId,
	producer: Producer<Batch>,
	consumer: Consumer<Message>,
	needs_update: bool,
	updates: Vec<Update>,
	update_buffers: Vec<Vec<Update>>,
}

impl AudioProcessor {
	pub fn create(transport: Transport) -> (Callback, NodeId, Producer<Message>, Consumer<Batch>) {
		let (r_producer, consumer) = RingBuffer::new(transport.frames.get() as usize);
		let (producer, r_consumer) = RingBuffer::new(transport.frames.get() as usize);

		let master_channel = Channel::default();
		let master = master_channel.id();

		let mut audio_graph = AudioGraph::new(
			State {
				transport,
				samples: HashMap::new(),
				midi_patterns: HashMap::new(),
				automation_patterns: HashMap::new(),
			},
			transport.frames,
		);

		audio_graph.insert(master_channel.into());

		let processor = Self {
			audio_graph,
			master,
			producer,
			consumer,
			needs_update: false,
			updates: Vec::new(),
			update_buffers: Vec::new(),
		};

		(
			Callback::Processing(processor),
			master,
			r_producer,
			r_consumer,
		)
	}

	#[must_use]
	fn recv_events(&mut self) -> Option<(oneshot::Sender<Self>, oneshot::Receiver<Self>)> {
		while let Ok(msg) = self.consumer.pop() {
			trace!("{msg:?}");

			match msg {
				Message::NodeAction(node, action) => self
					.audio_graph
					.for_node_mut(node, move |node, state| node.apply(action, state)),
				Message::MidiPatternAction(pattern, action) => {
					self.state_mut()
						.midi_patterns
						.get_mut(&pattern)
						.unwrap()
						.apply(action);
				}
				Message::AutomationPatternAction(pattern, action) => {
					self.state_mut()
						.automation_patterns
						.get_mut(&pattern)
						.unwrap()
						.apply(action);
				}
				Message::SampleAdd(sample) => {
					let sample = self.state_mut().samples.insert(sample.id, sample);
					debug_assert!(sample.is_none());
				}
				Message::SampleRemove(sample) => {
					let sample = self.state_mut().samples.remove(&sample);
					debug_assert!(sample.is_some());
				}
				Message::MidiPatternAdd(pattern) => {
					let pattern = self.state_mut().midi_patterns.insert(pattern.id, pattern);
					debug_assert!(pattern.is_none());
				}
				Message::MidiPatternRemove(pattern) => {
					let pattern = self.state_mut().midi_patterns.remove(&pattern);
					debug_assert!(pattern.is_some());
				}
				Message::AutomationPatternAdd(pattern) => {
					let pattern = self
						.state_mut()
						.automation_patterns
						.insert(pattern.id, pattern);
					debug_assert!(pattern.is_none());
				}
				Message::AutomationPatternRemove(pattern) => {
					let pattern = self.state_mut().automation_patterns.remove(&pattern);
					debug_assert!(pattern.is_some());
				}
				Message::NodeAdd(node) => self.audio_graph.insert(*node),
				Message::NodeRemove(node) => self.audio_graph.remove(node),
				Message::NodeConnect(from, to) => {
					if !self.audio_graph.connect(from, to) {
						self.updates.push(Update::ConnectFailed(from, to));
					}
				}
				Message::NodeSetMix(from, to, mix) => self.audio_graph.set_mix(from, to, mix),
				Message::NodeDisconnect(from, to) => self.audio_graph.disconnect(from, to),
				Message::Bpm(bpm) => self.transport_mut().bpm = bpm,
				Message::Numerator(numerator) => self.transport_mut().numerator = numerator,
				Message::TogglePlayback => self.transport_mut().playing ^= true,
				Message::ToggleMetronome => self.transport_mut().metronome ^= true,
				Message::Position(version, sample) => {
					self.transport_mut().version = version;
					self.transport_mut().position = sample;
				}
				Message::LoopRange(loop_range) => self.transport_mut().loop_range = loop_range,
				Message::Reset => self.audio_graph.reset(),
				Message::RequestUpdate => self.needs_update = true,
				Message::ReturnUpdate(update) => {
					debug_assert!(update.is_empty());
					self.update_buffers.push(update);
				}
				Message::RequestProcessor(sender, receiver) => return Some((sender, receiver)),
			}
		}

		None
	}

	#[must_use]
	pub fn process(
		&mut self,
		mut buf: &mut [f32],
	) -> Option<(oneshot::Sender<Self>, oneshot::Receiver<Self>)> {
		let start = Instant::now();
		let frames = buf.len() / 2;

		let acc = self
			.updates
			.pop_if(|update| matches!(update, Update::Load(..)));

		if let Some((sender, receiver)) = self.recv_events() {
			return Some((sender, receiver));
		}

		if self.updates.capacity() == 0
			&& let Some(updates) = self.update_buffers.pop()
		{
			self.updates = updates;
		}

		while !buf.is_empty() {
			let (looped, len) = if self.transport().playing
				&& let Some(loop_range) = self.transport().loop_range
				&& let Some(len) = loop_range
					.end()
					.to_seconds_time(self.transport())
					.checked_sub(self.transport().position)
				&& let len = len.to_samples(self.transport())
				&& len <= buf.len()
			{
				(Some(loop_range.start()), len)
			} else {
				(None, buf.len())
			};

			self.audio_graph.process(len);
			self.audio_graph.copy_output(self.master, &mut buf[..len]);
			for s in &mut buf[..len] {
				*s = s.clamp(-1.0, 1.0);
			}
			self.metronome(&mut buf[..len]);

			if let Some(position) = looped {
				self.transport_mut().position = position.to_seconds_time(self.transport());
			} else if self.transport().playing {
				let len = SecondsTime::from_samples(len, self.transport());
				self.transport_mut().position += len;
			}

			buf = &mut buf[len..];
		}

		self.audio_graph
			.for_each_node_mut(|node| node.collect_updates(&mut self.updates));

		let now = Instant::now();
		let mut duration = now - start;
		let mut frames = frames;

		if let Some(Update::Load(d, f)) = acc {
			duration += d;
			frames += f;
		}

		self.updates.push(Update::Load(duration, frames));

		if std::mem::take(&mut self.needs_update)
			|| self.transport().playing
			|| self.updates.len() > 1
		{
			let batch = Batch {
				version: self.transport().version,
				position: self.transport().position,
				updates: std::mem::take(&mut self.updates),
				now,
			};

			if let Err(PushError::Full(Batch { updates, .. })) = self.producer.push(batch) {
				warn!("full ring buffer");
				self.needs_update = true;
				self.updates = updates;
			}
		}

		None
	}

	fn metronome(&self, buf: &mut [f32]) {
		if !self.transport().metronome || !self.transport().playing {
			return;
		}

		let delay =
			SecondsTime::from_samples(self.audio_graph.delay(self.master), self.transport());

		let mut start = self
			.transport()
			.position
			.saturating_sub(delay)
			.to_beat_time(self.transport())
			.beat_floor();

		let end = (self.transport().position
			+ SecondsTime::from_samples(buf.len(), self.transport()))
		.saturating_sub(delay)
		.to_beat_time(self.transport())
		.beat_ceil();

		while start < end {
			let start_offset = start.to_seconds_time(self.transport()) + delay;

			let click = if start
				.beat()
				.is_multiple_of(self.transport().numerator.get().into())
			{
				&ON_BAR_CLICK
			} else {
				&OFF_BAR_CLICK
			};

			start += BeatTime::BEAT;

			let write_start = start_offset
				.saturating_sub(self.transport().position)
				.to_samples(self.transport());

			if write_start >= buf.len() {
				return;
			}

			let play_pos = self
				.transport()
				.position
				.saturating_sub(start_offset)
				.to_samples(self.transport());

			let resample_ratio = 44100.0 / self.transport().sample_rate.get() as f32;

			resample_cubic(&mut buf[write_start..], click, resample_ratio, play_pos / 2);
		}
	}

	pub fn render(
		&mut self,
		path: impl AsRef<Path>,
		node: NodeId,
		beat_range: BeatRange,
		mut samples_fn: impl FnMut(&[f32]),
		mut progress_fn: impl FnMut(f32),
	) {
		let old = *self.transport();
		self.audio_graph.reset();

		self.transport_mut().position = SecondsTime::ZERO;
		self.transport_mut().playing = true;

		let mut writer = WavWriter::create(
			path,
			hound::WavSpec {
				channels: 2,
				sample_rate: self.transport().sample_rate.get(),
				bits_per_sample: 32,
				sample_format: hound::SampleFormat::Float,
			},
		)
		.unwrap();

		let buffer_size = 2 * self.transport().frames.get() as usize;
		let mut buf = boxed_slice![0.0; buffer_size];
		let buffer_size = SecondsTime::from_samples(buffer_size, self.transport());

		let range_start = beat_range.start().to_seconds_time(self.transport());
		let range_len = beat_range.len().to_seconds_time(self.transport());

		let mut updates = Vec::new();

		let mut render_start;
		let mut render_end;

		while {
			render_start = range_start
				+ SecondsTime::from_samples(self.audio_graph.delay(node), self.transport());
			render_end = render_start + range_len;
			self.transport().position < render_start
		} {
			let diff = buffer_size.min(render_start - self.transport().position);
			let diff_samples = diff.to_samples(self.transport());

			self.audio_graph.process(diff_samples);
			self.audio_graph
				.for_each_node_mut(|node| node.collect_updates(&mut updates));
			updates.clear();

			self.transport_mut().position += diff;
			progress_fn(self.transport().position / render_end);
		}

		while {
			render_start =
				SecondsTime::from_samples(self.audio_graph.delay(node), self.transport());
			render_end = render_start + range_len;
			self.transport().position < render_end
		} {
			let diff = buffer_size.min(render_end - self.transport().position);
			let diff_samples = diff.to_samples(self.transport());

			self.audio_graph.process(diff_samples);
			self.audio_graph.copy_output(node, &mut buf[..diff_samples]);
			self.audio_graph
				.for_each_node_mut(|node| node.collect_updates(&mut updates));
			updates.clear();

			samples_fn(&buf[..diff_samples]);
			for &s in &buf[..diff_samples] {
				writer.write_sample(s).unwrap();
			}

			self.transport_mut().position += diff;
			progress_fn(self.transport().position / render_end);
		}

		writer.finalize().unwrap();

		*self.transport_mut() = old;
		self.audio_graph.reset();
	}

	fn state(&self) -> &State {
		self.audio_graph.state()
	}

	fn state_mut(&mut self) -> &mut State {
		self.audio_graph.state_mut()
	}

	fn transport(&self) -> &Transport {
		&self.state().transport
	}

	fn transport_mut(&mut self) -> &mut Transport {
		&mut self.state_mut().transport
	}
}

#[derive(Debug)]
#[expect(clippy::large_enum_variant)]
pub enum Callback {
	Processing(AudioProcessor),
	Away(oneshot::Receiver<AudioProcessor>),
}

impl Callback {
	pub fn process(&mut self, buf: &mut [f32]) {
		match self {
			Self::Processing(processor) => {
				if let Some((sender, receiver)) = processor.process(buf) {
					let Self::Processing(processor) = std::mem::replace(self, Self::Away(receiver))
					else {
						unreachable!();
					};

					sender.send(processor).unwrap();
				}
			}
			Self::Away(receiver) => {
				if let Ok(processor) = receiver.try_recv() {
					*self = Self::Processing(processor);
					self.process(buf);
				} else {
					buf.fill(0.0);
				}
			}
		}
	}
}
