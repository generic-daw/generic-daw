use crate::{Batch, Message, NodeId, Stream, StreamTrait as _, Transport, daw_ctx::DawCtx};
use cpal::{
	BufferSize, StreamConfig, SupportedBufferSize, SupportedStreamConfigRange,
	traits::{DeviceTrait as _, HostTrait as _},
};
use log::{error, info};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{cmp::Ordering, num::NonZero, sync::Arc};
use utils::boxed_slice;

pub fn build_input_stream(
	device_name: Option<Arc<str>>,
	sample_rate: NonZero<u32>,
	frames: Option<NonZero<u32>>,
) -> (StreamConfig, Consumer<Box<[f32]>>, Stream) {
	let host = cpal::default_host();

	let device = device_name
		.and_then(|device_name| Some((device_name, host.input_devices().ok()?)))
		.and_then(|(device_name, mut devices)| {
			devices.find(|device| {
				device
					.description()
					.is_ok_and(|description| *description.name() == *device_name)
			})
		})
		.or_else(|| host.default_input_device())
		.unwrap();

	let config = choose_config(
		device.supported_input_configs().unwrap(),
		sample_rate,
		frames,
	);

	info!("starting input stream with config {config:#?}");

	let sample_rate = NonZero::new(config.sample_rate).unwrap();
	let frames = frames_of_config(&config)
		.or(frames)
		.or(NonZero::new(8192))
		.unwrap();
	let channels = NonZero::new(u32::from(config.channels)).unwrap();
	let buffer_len = frames.get() * channels.get();

	let (mut producer, consumer) =
		RingBuffer::new(sample_rate.get().div_ceil(frames.get()) as usize);

	let mut stereo = boxed_slice![0.0; 2 * frames.get() as usize];

	let stream = device
		.build_input_stream(
			&config,
			move |buf, _| {
				for buf in buf.chunks(buffer_len as usize) {
					let frames = buf.len() / usize::from(config.channels);
					from_other_to_stereo(&mut stereo[..2 * frames], buf, frames);
					producer.push(stereo[..2 * frames].into()).unwrap();
				}
			},
			|err| error!("{err}"),
			None,
		)
		.unwrap();

	stream.play().unwrap();

	(config, consumer, stream)
}

pub fn build_output_stream(
	device_name: Option<Arc<str>>,
	sample_rate: NonZero<u32>,
	frames: Option<NonZero<u32>>,
) -> (
	NodeId,
	Transport,
	Producer<Message>,
	Consumer<Batch>,
	Stream,
) {
	let host = cpal::default_host();

	let device = device_name
		.and_then(|device_name| Some((device_name, host.output_devices().ok()?)))
		.and_then(|(device_name, mut devices)| {
			devices.find(|device| {
				device
					.description()
					.is_ok_and(|description| *description.name() == *device_name)
			})
		})
		.or_else(|| host.default_output_device())
		.unwrap();

	let config = choose_config(
		device.supported_output_configs().unwrap(),
		sample_rate,
		frames,
	);

	info!("starting output stream with config {config:#?}");

	let sample_rate = NonZero::new(config.sample_rate).unwrap();
	let frames = frames_of_config(&config)
		.or(frames)
		.or(NonZero::new(8192))
		.unwrap();
	let channels = NonZero::new(u32::from(config.channels)).unwrap();
	let buffer_len = frames.get() * channels.get();

	let transport = Transport::new(sample_rate, frames);
	let (mut ctx, master_node_id, producer, consumer) = DawCtx::create(transport);

	let mut stereo = boxed_slice![0.0; 2 * frames.get() as usize];

	let stream = device
		.build_output_stream(
			&config,
			move |buf, _| {
				for buf in buf.chunks_mut(buffer_len as usize) {
					let frames = buf.len() / channels.get() as usize;
					ctx.process(&mut stereo[..2 * frames]);
					from_stereo_to_other(buf, &stereo[..2 * frames], frames);
				}
			},
			|err| error!("{err}"),
			None,
		)
		.unwrap();

	stream.play().unwrap();

	(master_node_id, transport, producer, consumer, stream)
}

fn choose_config(
	configs: impl IntoIterator<Item = SupportedStreamConfigRange>,
	sample_rate: NonZero<u32>,
	frames: Option<NonZero<u32>>,
) -> StreamConfig {
	let config = configs
		.into_iter()
		.filter(|config| config.channels() != 0)
		.min_by(|l, r| {
			compare_by_sample_rate(l, r, sample_rate)
				.then_with(|| {
					frames.map_or(Ordering::Equal, |frames| compare_by_frames(l, r, frames))
				})
				.then_with(|| compare_by_channel_count(l, r))
		})
		.unwrap();

	let sample_rate = sample_rate
		.get()
		.clamp(config.min_sample_rate(), config.max_sample_rate());

	let buffer_size = match (*config.buffer_size(), frames) {
		(SupportedBufferSize::Unknown, _) | (_, None) => BufferSize::Default,
		(SupportedBufferSize::Range { min, max }, Some(frames)) => {
			BufferSize::Fixed(frames.get().clamp(min, max))
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
	sample_rate: NonZero<u32>,
) -> Ordering {
	let sample_rate = sample_rate.get();
	let ldiff = sample_rate
		.clamp(l.min_sample_rate(), l.max_sample_rate())
		.abs_diff(sample_rate);
	let rdiff = sample_rate
		.clamp(r.min_sample_rate(), r.max_sample_rate())
		.abs_diff(sample_rate);

	ldiff.cmp(&rdiff)
}

fn compare_by_frames(
	l: &SupportedStreamConfigRange,
	r: &SupportedStreamConfigRange,
	frames: NonZero<u32>,
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
			let frames = frames.get();
			let ldiff = frames.clamp(lmin, lmax).abs_diff(frames);
			let rdiff = frames.clamp(rmin, rmax).abs_diff(frames);
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
		1 => u8::MAX.into(),
		2 => 0,
		x => x,
	};
	let rdiff = match r.channels() {
		0 => u16::MAX,
		1 => u8::MAX.into(),
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
			.zip(b.as_chunks::<2>().0)
			.for_each(|(a, b)| {
				a[0] = b[0];
				a[1] = b[1];
			}),
		Ordering::Equal => a.iter_mut().zip(b).for_each(|(a, b)| *a = *b),
		Ordering::Less => a
			.iter_mut()
			.zip(b.as_chunks::<2>().0)
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
			.zip(a.as_chunks_mut::<2>().0)
			.for_each(|(b, a)| {
				a[0] = b[0];
				a[1] = b[1];
			}),
		Ordering::Equal => a.iter_mut().zip(b).for_each(|(a, b)| *a = *b),
		Ordering::Greater => b.iter().zip(a.as_chunks_mut::<2>().0).for_each(|(b, a)| {
			a[0] = *b;
			a[1] = *b;
		}),
	}
}

pub fn frames_of_config(config: &StreamConfig) -> Option<NonZero<u32>> {
	match config.buffer_size {
		BufferSize::Fixed(buffer_size) => NonZero::new(buffer_size),
		BufferSize::Default => None,
	}
}
