use crate::{
	AudioGraph, AutomationPattern, AutomationPatternAction, AutomationPatternId, Channel, Clip,
	MidiPattern, MidiPatternAction, MidiPatternId, MusicalTime, Node, NodeId, PanMode, PluginId,
	Position, Sample, SampleId, clap_host::ClapId, sample::resample_cubic,
};
use clap_host::{
	Cookie,
	events::{EventFlags, EventHeader, TransportEvent, TransportFlags},
};
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
	Sample(Version, usize),
	LoopMarker(Option<Position>),
	Reset,

	RequestUpdate,
	ReturnUpdate(Vec<Update>),

	RequestRender(
		oneshot::Sender<AudioProcessor>,
		oneshot::Receiver<AudioProcessor>,
	),
}

const _: () = assert!(size_of::<Message>() == 64);

#[derive(Debug)]
pub enum NodeAction {
	ClipAdd(Clip, usize),
	ClipRemove(usize),
	ClipMoveTo(usize, MusicalTime),
	ClipTrimStartTo(usize, MusicalTime),
	ClipTrimEndTo(usize, MusicalTime),
	ClipStretchStartTo(usize, MusicalTime),
	ClipStretchEndTo(usize, MusicalTime),

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
	pub sample: usize,
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
	pub loop_marker: Option<Position>,
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
				} | if self.loop_marker.is_some() {
				TransportFlags::IS_LOOP_ACTIVE
			} else {
				TransportFlags::empty()
			},
			song_pos_beats: MusicalTime::from_samples(self.sample, self).to_beat_time(self),
			song_pos_seconds: MusicalTime::from_samples(self.sample, self).to_seconds_time(self),
			tempo: self.bpm.get().into(),
			tempo_inc: 0.0,
			loop_start_beats: self
				.loop_marker
				.map(|loop_marker| loop_marker.start().to_beat_time(self))
				.unwrap_or_default(),
			loop_end_beats: self
				.loop_marker
				.map(|loop_marker| loop_marker.end().to_beat_time(self))
				.unwrap_or_default(),
			loop_start_seconds: self
				.loop_marker
				.map(|loop_marker| loop_marker.start().to_seconds_time(self))
				.unwrap_or_default(),
			loop_end_seconds: self
				.loop_marker
				.map(|loop_marker| loop_marker.end().to_seconds_time(self))
				.unwrap_or_default(),
			bar_start: MusicalTime::from_samples(self.sample, self)
				.bar_floor(self)
				.to_beat_time(self),
			bar_number: MusicalTime::from_samples(self.sample, self).bar(self) as i32,
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

		let processor = Self {
			audio_graph: AudioGraph::new(
				State {
					transport,
					samples: HashMap::new(),
					midi_patterns: HashMap::new(),
					automation_patterns: HashMap::new(),
				},
				Channel::default(),
				transport.frames,
			),
			producer,
			consumer,
			needs_update: false,
			updates: Vec::new(),
			update_buffers: Vec::new(),
		};

		let id = processor.audio_graph.root();
		(Callback::Processing(processor), id, r_producer, r_consumer)
	}

	#[must_use]
	fn recv_events(&mut self) -> Option<(oneshot::Sender<Self>, oneshot::Receiver<Self>)> {
		while let Ok(msg) = self.consumer.pop() {
			trace!("{msg:?}");

			match msg {
				Message::NodeAction(node, action) => self
					.audio_graph
					.for_node_mut(node, move |node| node.apply(action)),
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
				Message::Sample(version, sample) => {
					self.transport_mut().version = version;
					self.transport_mut().sample = sample;
				}
				Message::LoopMarker(loop_marker) => self.transport_mut().loop_marker = loop_marker,
				Message::Reset => self.audio_graph.reset(),
				Message::RequestUpdate => self.needs_update = true,
				Message::ReturnUpdate(update) => {
					debug_assert!(update.is_empty());
					self.update_buffers.push(update);
				}
				Message::RequestRender(sender, receiver) => return Some((sender, receiver)),
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

		let loop_marker = self
			.transport()
			.loop_marker
			.map(|loop_marker| loop_marker.to_samples(self.transport()));

		while !buf.is_empty() {
			if self.transport().playing
				&& let Some((start, end)) = loop_marker
				&& self.transport().sample == end
			{
				self.transport_mut().sample = start;
			}

			let len = loop_marker
				.and_then(|(_, end)| end.checked_sub(self.transport().sample))
				.map_or(buf.len(), |len| len.min(buf.len()));

			self.audio_graph.process(&mut buf[..len]);
			for s in &mut buf[..len] {
				*s = s.clamp(-1.0, 1.0);
			}
			self.metronome(&mut buf[..len]);

			if self.transport().playing {
				self.transport_mut().sample += len;
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
				sample: self.transport().sample,
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

		let delay = self.audio_graph.delay();

		let mut start = MusicalTime::from_samples(
			self.transport().sample.saturating_sub(delay),
			self.transport(),
		)
		.beat_floor();
		let end = MusicalTime::from_samples(
			(self.transport().sample + buf.len()).saturating_sub(delay),
			self.transport(),
		)
		.beat_ceil();

		while start < end {
			let start_samples = start.to_samples(self.transport()) + delay;

			let click = if start
				.beat()
				.is_multiple_of(self.transport().numerator.get().into())
			{
				&ON_BAR_CLICK
			} else {
				&OFF_BAR_CLICK
			};

			start += MusicalTime::BEAT;

			let resample_ratio = 44100.0 / self.transport().sample_rate.get() as f32;

			let buf_idx = start_samples.saturating_sub(self.transport().sample);
			let click_idx = self.transport().sample.saturating_sub(start_samples);

			if buf_idx >= buf.len() || click_idx >= click.len() {
				continue;
			}

			resample_cubic(&mut buf[buf_idx..], click, resample_ratio, click_idx / 2);
		}
	}

	pub fn render(
		&mut self,
		path: impl AsRef<Path>,
		len: MusicalTime,
		mut progress_fn: impl FnMut(f32),
	) {
		let old = *self.transport();
		self.audio_graph.reset();

		self.transport_mut().sample = 0;
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

		let mut updates = Vec::new();

		let mut delay;
		let mut end;

		while {
			delay = self.audio_graph.delay();
			end = len.to_samples(self.transport()) + delay;
			self.transport().sample < delay
		} {
			let diff = buffer_size.min(delay - self.transport().sample);

			self.audio_graph.process(&mut buf[..diff]);
			self.audio_graph
				.for_each_node_mut(|node| node.collect_updates(&mut updates));
			updates.clear();

			self.transport_mut().sample += diff;
			progress_fn(self.transport().sample as f32 / end as f32);
		}

		while {
			delay = self.audio_graph.delay();
			end = len.to_samples(self.transport()) + delay;
			self.transport().sample < end
		} {
			let diff = buffer_size.min(end - self.transport().sample);

			self.audio_graph.process(&mut buf[..diff]);
			self.audio_graph
				.for_each_node_mut(|node| node.collect_updates(&mut updates));
			updates.clear();

			for &s in &buf[..diff] {
				writer.write_sample(s).unwrap();
			}

			self.transport_mut().sample += diff;
			progress_fn(self.transport().sample as f32 / end as f32);
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
	Exporting(oneshot::Receiver<AudioProcessor>),
}

impl Callback {
	pub fn process(&mut self, buf: &mut [f32]) {
		match self {
			Self::Processing(processor) => {
				if let Some((sender, receiver)) = processor.process(buf) {
					let Self::Processing(processor) =
						std::mem::replace(self, Self::Exporting(receiver))
					else {
						unreachable!();
					};

					sender.send(processor).unwrap();
				}
			}
			Self::Exporting(receiver) => {
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
