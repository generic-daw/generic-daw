use async_channel::{Receiver, Sender};
use audio_graph_node::AudioGraphNode;
use cpal::{
	BufferSize, SampleRate, StreamConfig, SupportedBufferSize, SupportedStreamConfigRange,
	traits::{DeviceTrait as _, HostTrait as _},
};
use daw_ctx::DawCtx;
use log::info;
use master::Master;
use std::cmp::Ordering;

mod audio_clip;
mod audio_graph_node;
mod clip;
mod clip_position;
mod daw_ctx;
mod decibels;
mod event;
mod export;
mod lod;
mod master;
mod midi_clip;
mod mixer;
mod musical_time;
mod recording;
mod resampler;
mod track;

pub use audio_clip::{AudioClip, Sample};
pub use audio_graph::{NodeId, NodeImpl};
pub use clap_host;
pub use clip::Clip;
pub use clip_position::ClipPosition;
pub use cpal::{Stream, traits::StreamTrait};
pub use daw_ctx::{Action, Message, RtState, Update, Version};
pub use decibels::Decibels;
pub use export::export;
pub use lod::LOD_LEVELS;
pub use midi_clip::{Key, MidiClip, MidiKey, MidiNote};
pub use mixer::Mixer;
pub use musical_time::MusicalTime;
pub use recording::Recording;
pub use track::Track;

#[must_use]
pub fn get_input_devices() -> Vec<String> {
	cpal::default_host()
		.input_devices()
		.unwrap()
		.map(|device| device.name().unwrap())
		.collect()
}

#[must_use]
pub fn get_output_devices() -> Vec<String> {
	cpal::default_host()
		.output_devices()
		.unwrap()
		.map(|device| device.name().unwrap())
		.collect()
}

pub fn build_input_stream(
	device_name: Option<&str>,
	sample_rate: u32,
	buffer_size: u32,
) -> (Stream, StreamConfig, Receiver<Box<[f32]>>) {
	let (sender, receiver) = async_channel::unbounded();

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
		buffer_size,
	);

	info!("starting input stream with config {config:?}");

	let mut stereo = vec![];

	let stream = device
		.build_input_stream(
			&config,
			move |buf, _| {
				let frames = buf.len() / config.channels as usize;
				stereo.clear();
				stereo.resize(frames * 2, 0.0);
				from_other_to_stereo(&mut stereo, buf, frames);
				sender.try_send(stereo.clone().into_boxed_slice()).unwrap();
			},
			|err| panic!("{err}"),
			None,
		)
		.unwrap();

	stream.play().unwrap();

	(stream, config, receiver)
}

pub fn build_output_stream(
	device_name: Option<&str>,
	sample_rate: u32,
	buffer_size: u32,
) -> (Stream, NodeId, RtState, Sender<Message>, Receiver<Update>) {
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
		buffer_size,
	);

	let (mut ctx, node, rtstate, sender, receiver) =
		DawCtx::create(config.sample_rate.0, buffer_size);

	info!("starting output stream with config {config:?}");

	let mut stereo = vec![];

	let stream = device
		.build_output_stream(
			&config,
			move |buf, _| {
				let frames = buf.len() / config.channels as usize;
				stereo.clear();
				stereo.resize(frames * 2, 0.0);
				ctx.process(&mut stereo);
				from_stereo_to_other(buf, &stereo, frames);
			},
			|err| panic!("{err}"),
			None,
		)
		.unwrap();

	stream.play().unwrap();

	(stream, node, rtstate, sender, receiver)
}

fn choose_config(
	configs: impl IntoIterator<Item = SupportedStreamConfigRange>,
	sample_rate: u32,
	buffer_size: u32,
) -> StreamConfig {
	let config = configs
		.into_iter()
		.min_by(|l, r| {
			compare_by_sample_rate(l, r, sample_rate)
				.then_with(|| compare_by_buffer_size(l, r, buffer_size))
				.then_with(|| r.channels().cmp(&l.channels()))
		})
		.unwrap();

	let sample_rate =
		SampleRate(sample_rate.clamp(config.min_sample_rate().0, config.max_sample_rate().0));

	let buffer_size = match *config.buffer_size() {
		SupportedBufferSize::Unknown => BufferSize::Default,
		SupportedBufferSize::Range { min, max } => BufferSize::Fixed(buffer_size.clamp(min, max)),
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

fn compare_by_buffer_size(
	l: &SupportedStreamConfigRange,
	r: &SupportedStreamConfigRange,
	buffer_size: u32,
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
			let ldiff = buffer_size.clamp(lmin, lmax).abs_diff(buffer_size);
			let rdiff = buffer_size.clamp(rmin, rmax).abs_diff(buffer_size);
			ldiff.cmp(&rdiff)
		}
	}
}

fn from_stereo_to_other(a: &mut [f32], b: &[f32], frames: usize) {
	debug_assert!(a.len().is_multiple_of(frames));
	debug_assert!(b.len().is_multiple_of(frames));
	debug_assert!(b.len() / frames == 2);

	if a.len() >= b.len() {
		a.chunks_exact_mut(a.len() / frames)
			.flat_map(|a| a.iter_mut().take(2))
			.zip(b)
			.for_each(|(a, b)| *a = *b);
	} else {
		a.iter_mut()
			.zip(b.as_chunks().0.iter().map(|[l, r]| l + r))
			.for_each(|(a, b)| *a = b);
	}
}

fn from_other_to_stereo(a: &mut [f32], b: &[f32], frames: usize) {
	debug_assert!(a.len().is_multiple_of(frames));
	debug_assert!(a.len() / frames == 2);
	debug_assert!(b.len().is_multiple_of(frames));

	if a.len() < b.len() {
		b.chunks_exact(b.len() / frames)
			.flat_map(|b| b.iter().take(2))
			.zip(a)
			.for_each(|(b, a)| *a = *b);
	} else {
		a.iter_mut()
			.zip(b.iter().flat_map(|x| [x, x]))
			.for_each(|(a, b)| *a = *b);
	}
}
