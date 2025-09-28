use audio_graph_node::AudioGraphNode;
use cpal::{
	BufferSize, SampleRate, StreamConfig, SupportedBufferSize, SupportedStreamConfigRange,
	traits::{DeviceTrait as _, HostTrait as _},
};
use daw_ctx::DawCtx;
use log::info;
use master::Master;
use rtrb::{Consumer, Producer, RingBuffer};
use std::{cmp::Ordering, sync::Arc};

mod audio_clip;
mod audio_graph_node;
mod channel;
mod clip;
mod daw_ctx;
mod event;
mod export;
mod master;
mod midi_clip;
mod midi_key;
mod midi_note;
mod musical_time;
mod pattern;
mod recording;
mod resampler;
mod sample;
mod track;

pub use audio_clip::AudioClip;
pub use audio_graph::{NodeId, NodeImpl};
pub use channel::{Channel, Flags};
pub use clap_host;
pub use clip::Clip;
pub use cpal::{Stream, traits::StreamTrait};
pub use daw_ctx::{Batch, Message, NodeAction, PatternAction, RtState, Update, Version};
pub use event::Event;
pub use export::export;
pub use midi_clip::MidiClip;
pub use midi_key::{Key, MidiKey};
pub use midi_note::MidiNote;
pub use musical_time::{ClipPosition, MusicalTime, NotePosition};
pub use pattern::{Pattern, PatternId};
pub use recording::Recording;
pub use sample::{Sample, SampleId};
pub use symphonia::core::io::MediaSource;
pub use track::Track;

pub type AudioGraph = audio_graph::AudioGraph<AudioGraphNode>;

#[must_use]
pub fn get_input_devices() -> Vec<Arc<str>> {
	cpal::default_host()
		.input_devices()
		.unwrap()
		.filter_map(|device| device.name().ok())
		.map(Arc::from)
		.collect()
}

#[must_use]
pub fn get_output_devices() -> Vec<Arc<str>> {
	cpal::default_host()
		.output_devices()
		.unwrap()
		.filter_map(|device| device.name().ok())
		.map(Arc::from)
		.collect()
}

fn buffer_size_of_config(config: &StreamConfig) -> Option<u32> {
	match config.buffer_size {
		BufferSize::Fixed(buffer_size) => Some(buffer_size),
		BufferSize::Default => None,
	}
}

fn build_input_stream(
	device_name: Option<&str>,
	sample_rate: u32,
	frames: u32,
) -> (Stream, StreamConfig, Consumer<Box<[f32]>>) {
	let (mut producer, consumer) = RingBuffer::new(sample_rate.div_ceil(frames) as usize);

	let host = cpal::default_host();

	let device = device_name
		.and_then(|device_name| {
			host.input_devices()
				.unwrap()
				.find(|device| device.name().is_ok_and(|name| name == device_name))
		})
		.unwrap_or_else(|| host.default_input_device().unwrap());

	let config = choose_config(
		device.supported_input_configs().unwrap(),
		sample_rate,
		frames,
	);

	let buffer_size = buffer_size_of_config(&config).unwrap_or(frames);
	let channels = u32::from(config.channels);
	let frames = buffer_size / channels;

	info!("starting input stream with config {config:#?}");

	let mut stereo = vec![0.0; 2 * frames as usize].into_boxed_slice();

	let stream = device
		.build_input_stream(
			&config,
			move |buf, _| {
				for buf in buf.chunks(buffer_size as usize) {
					let frames = buf.len() / config.channels as usize;
					from_other_to_stereo(&mut stereo[..2 * frames], buf, frames);
					producer.push(stereo[..2 * frames].into()).unwrap();
				}
			},
			|err| panic!("{err}"),
			None,
		)
		.unwrap();

	stream.play().unwrap();

	(stream, config, consumer)
}

pub fn build_output_stream(
	device_name: Option<&str>,
	sample_rate: u32,
	frames: u32,
) -> (Stream, NodeId, RtState, Producer<Message>, Consumer<Batch>) {
	let host = cpal::default_host();

	let device = device_name
		.and_then(|device_name| {
			host.output_devices()
				.unwrap()
				.find(|device| device.name().is_ok_and(|name| name == device_name))
		})
		.unwrap_or_else(|| host.default_output_device().unwrap());

	let config = choose_config(
		device.supported_output_configs().unwrap(),
		sample_rate,
		frames,
	);

	let sample_rate = config.sample_rate.0;
	let buffer_size = buffer_size_of_config(&config).unwrap_or(frames);
	let channels = u32::from(config.channels);
	let frames = buffer_size / channels;

	let rtstate = RtState::new(sample_rate, frames);
	let (mut ctx, node, producer, consumer) = DawCtx::create(rtstate);

	info!("starting output stream with config {config:#?}");

	let mut stereo = vec![0.0; 2 * frames as usize].into_boxed_slice();

	let stream = device
		.build_output_stream(
			&config,
			move |buf, _| {
				for buf in buf.chunks_mut(buffer_size as usize) {
					let frames = buf.len() / channels as usize;
					ctx.process(&mut stereo[..2 * frames]);
					from_stereo_to_other(buf, &stereo[..2 * frames], frames);
				}
			},
			|err| panic!("{err}"),
			None,
		)
		.unwrap();

	stream.play().unwrap();

	(stream, node, rtstate, producer, consumer)
}

fn choose_config(
	configs: impl IntoIterator<Item = SupportedStreamConfigRange>,
	sample_rate: u32,
	frames: u32,
) -> StreamConfig {
	let config = configs
		.into_iter()
		.filter(|config| config.channels() != 0)
		.min_by(|l, r| {
			compare_by_sample_rate(l, r, sample_rate)
				.then_with(|| compare_by_frames(l, r, frames))
				.then_with(|| compare_by_channel_count(l, r))
		})
		.unwrap();

	let sample_rate =
		SampleRate(sample_rate.clamp(config.min_sample_rate().0, config.max_sample_rate().0));

	let buffer_size = match *config.buffer_size() {
		SupportedBufferSize::Unknown => BufferSize::Default,
		SupportedBufferSize::Range { min, max } => {
			BufferSize::Fixed((frames * u32::from(config.channels())).clamp(min, max))
		}
	};

	StreamConfig {
		channels: config.channels(),
		sample_rate,
		buffer_size,
	}
}

fn compare_by_sample_rate(
	l: &SupportedStreamConfigRange,
	r: &SupportedStreamConfigRange,
	sample_rate: u32,
) -> Ordering {
	let ldiff = sample_rate
		.clamp(l.min_sample_rate().0, l.max_sample_rate().0)
		.abs_diff(sample_rate);
	let rdiff = sample_rate
		.clamp(r.min_sample_rate().0, r.max_sample_rate().0)
		.abs_diff(sample_rate);

	ldiff.cmp(&rdiff)
}

fn compare_by_frames(
	l: &SupportedStreamConfigRange,
	r: &SupportedStreamConfigRange,
	frames: u32,
) -> Ordering {
	match (*l.buffer_size(), *r.buffer_size()) {
		(SupportedBufferSize::Unknown, SupportedBufferSize::Unknown) => Ordering::Equal,
		(SupportedBufferSize::Range { .. }, SupportedBufferSize::Unknown) => Ordering::Less,
		(SupportedBufferSize::Unknown, SupportedBufferSize::Range { .. }) => Ordering::Greater,
		(
			SupportedBufferSize::Range {
				min: lmin,
				max: lmax,
			},
			SupportedBufferSize::Range {
				min: rmin,
				max: rmax,
			},
		) => {
			let ldiff = frames
				.clamp(
					lmin / u32::from(l.channels()),
					lmax / u32::from(l.channels()),
				)
				.abs_diff(frames);
			let rdiff = frames
				.clamp(
					rmin / u32::from(r.channels()),
					rmax / u32::from(r.channels()),
				)
				.abs_diff(frames);
			ldiff.cmp(&rdiff)
		}
	}
}

fn compare_by_channel_count(
	l: &SupportedStreamConfigRange,
	r: &SupportedStreamConfigRange,
) -> Ordering {
	let ldiff = match l.channels() {
		0 => u16::MAX,
		1 => 5,
		2 => 0,
		x => x,
	};
	let rdiff = match r.channels() {
		0 => u16::MAX,
		1 => 5,
		2 => 0,
		x => x,
	};

	ldiff.cmp(&rdiff)
}

fn from_stereo_to_other(a: &mut [f32], b: &[f32], frames: usize) {
	debug_assert!(a.len().is_multiple_of(frames));
	debug_assert!(b.len().is_multiple_of(frames));
	debug_assert!(b.len() / frames == 2);

	match a.len().cmp(&b.len()) {
		Ordering::Greater => a
			.chunks_exact_mut(a.len() / frames)
			.zip(b.chunks_exact(2))
			.for_each(|(a, b)| {
				a[0] = b[0];
				a[1] = b[1];
			}),
		Ordering::Equal => a.iter_mut().zip(b).for_each(|(a, b)| *a = *b),
		Ordering::Less => a
			.iter_mut()
			.zip(b.chunks_exact(2))
			.for_each(|(a, b)| *a = b[0] + b[1]),
	}
}

fn from_other_to_stereo(a: &mut [f32], b: &[f32], frames: usize) {
	debug_assert!(a.len().is_multiple_of(frames));
	debug_assert!(a.len() / frames == 2);
	debug_assert!(b.len().is_multiple_of(frames));

	match a.len().cmp(&b.len()) {
		Ordering::Less => b
			.chunks_exact(b.len() / frames)
			.zip(a.chunks_exact_mut(2))
			.for_each(|(b, a)| {
				a[0] = b[0];
				a[1] = b[1];
			}),
		Ordering::Equal => a.iter_mut().zip(b).for_each(|(a, b)| *a = *b),
		Ordering::Greater => a.chunks_exact_mut(2).zip(b).for_each(|(a, b)| {
			a[0] = *b;
			a[1] = *b;
		}),
	}
}
