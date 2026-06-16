use crate::{
	DeviceDescription, DeviceId, Stream,
	audio_thread::{AudioCallback, AudioThread},
};
use cpal::{
	BufferSize, Device, FromSample, I24, InputCallbackInfo, OutputCallbackInfo, Sample,
	SampleFormat, StreamConfig, SupportedBufferSize, SupportedStreamConfig,
	SupportedStreamConfigRange, U24,
	traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _},
};
use log::{error, info, warn};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{cmp::Ordering, collections::HashMap, num::NonZero};
use utils::boxed_slice;

#[must_use]
pub fn get_devices() -> HashMap<DeviceId, DeviceDescription> {
	cpal::default_host()
		.devices()
		.unwrap()
		.filter_map(|device| Some((device.id().ok()?, device.description().ok()?)))
		.collect()
}

pub fn build_input_stream(
	device_id: Option<&DeviceId>,
	sample_rate: Option<NonZero<u32>>,
	frames: Option<NonZero<u32>>,
) -> (Consumer<[f32; 2]>, Stream, NonZero<u32>, NonZero<u32>) {
	let host = cpal::default_host();

	let device = device_id
		.and_then(|device_id| host.device_by_id(device_id))
		.filter(Device::supports_input)
		.or_else(|| host.default_input_device())
		.unwrap();

	let supported_config = choose_supported_config(
		device.supported_input_configs().unwrap(),
		&device.default_input_config().unwrap(),
		sample_rate,
		frames,
	);

	let config = supported_config_to_config(&supported_config, frames);

	info!("starting input stream with config {config:#?}");

	let sample_rate = NonZero::new(config.sample_rate).unwrap();
	let frames = frames_of_config(&config).or(NonZero::new(2048)).unwrap();
	let channels = NonZero::new(u32::from(config.channels)).unwrap();

	let (producer, consumer) = RingBuffer::new(sample_rate.get() as usize);

	macro_rules! build_input_stream {
		($($pat:pat => $ty:ty),*$(,)?) => {
			match supported_config.sample_format() {
				$(
					$pat => device.build_input_stream(
						config,
						build_input_callback::<$ty>(frames, channels, producer),
						|err| error!("{err}"),
						None,
					),
				)*
				sample_format => panic!("unsupported sample format {sample_format}"),
			}
		}
	}

	let stream = build_input_stream! {
		SampleFormat::I8 => i8,
		SampleFormat::I16 => i16,
		SampleFormat::I24 => I24,
		SampleFormat::I32 => i32,
		SampleFormat::I64 => i64,
		SampleFormat::U8 => u8,
		SampleFormat::U16 => u16,
		SampleFormat::U24 => U24,
		SampleFormat::U32 => u32,
		SampleFormat::U64 => u64,
		SampleFormat::F32 => f32,
		SampleFormat::F64 => f64,
	}
	.unwrap();

	stream.play().unwrap();

	(consumer, stream, sample_rate, frames)
}

pub fn build_output_stream(
	device_id: Option<&DeviceId>,
	sample_rate: Option<NonZero<u32>>,
	frames: Option<NonZero<u32>>,
	receiver: oneshot::Receiver<AudioThread>,
) -> (Stream, NonZero<u32>, NonZero<u32>) {
	let host = cpal::default_host();

	let device = device_id
		.and_then(|device_id| host.device_by_id(device_id))
		.filter(Device::supports_output)
		.or_else(|| host.default_output_device())
		.unwrap();

	let supported_config = choose_supported_config(
		device.supported_output_configs().unwrap(),
		&device.default_output_config().unwrap(),
		sample_rate,
		frames,
	);

	let config = supported_config_to_config(&supported_config, frames);

	info!("starting output stream with config {config:#?}");

	let sample_rate = NonZero::new(config.sample_rate).unwrap();
	let frames = frames_of_config(&config).or(NonZero::new(2048)).unwrap();
	let channels = NonZero::new(u32::from(config.channels)).unwrap();

	macro_rules! build_output_stream {
		($($pat:pat => $ty:ty),*$(,)?) => {
			match supported_config.sample_format() {
				$(
					$pat => device.build_output_stream(
						config,
						build_output_callback::<$ty>(frames, channels, AudioCallback::Away(receiver)),
						|err| error!("{err}"),
						None,
					),
				)*
				sample_format => panic!("unsupported sample format {sample_format}"),
			}
		}
	}

	let stream = build_output_stream! {
		SampleFormat::I8 => i8,
		SampleFormat::I16 => i16,
		SampleFormat::I24 => I24,
		SampleFormat::I32 => i32,
		SampleFormat::I64 => i64,
		SampleFormat::U8 => u8,
		SampleFormat::U16 => u16,
		SampleFormat::U24 => U24,
		SampleFormat::U32 => u32,
		SampleFormat::U64 => u64,
		SampleFormat::F32 => f32,
		SampleFormat::F64 => f64,
	}
	.unwrap();

	stream.play().unwrap();

	(stream, sample_rate, frames)
}

fn choose_supported_config(
	configs: impl IntoIterator<Item = SupportedStreamConfigRange>,
	default_config: &SupportedStreamConfig,
	sample_rate: Option<NonZero<u32>>,
	frames: Option<NonZero<u32>>,
) -> SupportedStreamConfig {
	let config = configs
		.into_iter()
		.filter(|config| config.channels() != 0)
		.min_by(|l, r| {
			compare_by_sample_format(l, r)
				.then_with(|| {
					sample_rate.map_or(Ordering::Equal, |sample_rate| {
						compare_by_sample_rate(l, r, sample_rate)
					})
				})
				.then_with(|| {
					frames.map_or(Ordering::Equal, |frames| compare_by_frames(l, r, frames))
				})
				.then_with(|| compare_by_channel_count(l, r))
		})
		.unwrap();

	let sample_rate = sample_rate
		.map_or_else(|| default_config.sample_rate(), NonZero::get)
		.clamp(config.min_sample_rate(), config.max_sample_rate());

	config.with_sample_rate(sample_rate)
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

fn compare_by_sample_format(
	l: &SupportedStreamConfigRange,
	r: &SupportedStreamConfigRange,
) -> Ordering {
	match (l.sample_format().is_dsd(), r.sample_format().is_dsd()) {
		(true, true) => Ordering::Equal,
		(true, false) => Ordering::Greater,
		(false, true) => Ordering::Less,
		(false, false) => match (l.sample_format().is_float(), r.sample_format().is_float()) {
			(true, false) => Ordering::Greater,
			(false, true) => Ordering::Less,
			_ => l
				.sample_format()
				.bits_per_sample()
				.cmp(&r.sample_format().bits_per_sample()),
		},
	}
}

fn build_input_callback<T: Sample>(
	frames: NonZero<u32>,
	channels: NonZero<u32>,
	mut producer: Producer<[f32; 2]>,
) -> impl FnMut(&[T], &InputCallbackInfo)
where
	f32: FromSample<T>,
{
	let chunk_size = NonZero::new(frames.get() * channels.get()).unwrap();
	let mut stereo = boxed_slice![[0.0; 2]; frames.get() as usize];
	move |buf, _| {
		for buf in buf.chunks(chunk_size.get() as usize) {
			let frames = buf.len() / channels.get() as usize;
			from_other_to_stereo(&mut stereo[..frames], buf);
			if let (_, t) = producer.push_partial_slice(&stereo[..frames])
				&& !t.is_empty()
			{
				warn!("full ring buffer");
			}
		}
	}
}

fn build_output_callback<T: Sample + FromSample<f32>>(
	frames: NonZero<u32>,
	channels: NonZero<u32>,
	mut processor: AudioCallback,
) -> impl FnMut(&mut [T], &OutputCallbackInfo) {
	let chunk_size = NonZero::new(frames.get() * channels.get()).unwrap();
	let mut stereo = boxed_slice![[0.0; 2]; frames.get() as usize];
	move |buf, _| {
		for buf in buf.chunks_mut(chunk_size.get() as usize) {
			let frames = buf.len() / channels.get() as usize;
			processor.process(&mut stereo[..frames]);
			from_stereo_to_other(buf, &stereo[..frames]);
		}
	}
}

fn from_stereo_to_other<S: Sample + FromSample<f32>>(a: &mut [S], b: &[[f32; 2]]) {
	match a.len().cmp(&b.len()) {
		Ordering::Greater => a
			.chunks_exact_mut(a.len() / b.len())
			.zip(b)
			.for_each(|(a, b)| {
				a[0] = S::from_sample(b[0]);
				a[1] = S::from_sample(b[1]);
			}),
		Ordering::Equal => a
			.iter_mut()
			.zip(b.as_flattened())
			.for_each(|(a, b)| *a = S::from_sample(*b)),
		Ordering::Less => a
			.iter_mut()
			.zip(b)
			.for_each(|(a, b)| *a = S::from_sample((b[0] + b[1]) / 2.0)),
	}
}

fn from_other_to_stereo<S: Sample>(a: &mut [[f32; 2]], b: &[S])
where
	f32: FromSample<S>,
{
	match a.len().cmp(&b.len()) {
		Ordering::Less => b.chunks_exact(b.len() / a.len()).zip(a).for_each(|(b, a)| {
			a[0] = f32::from_sample(b[0]);
			a[1] = f32::from_sample(b[1]);
		}),
		Ordering::Equal => a
			.as_flattened_mut()
			.iter_mut()
			.zip(b)
			.for_each(|(a, b)| *a = f32::from_sample(*b)),
		Ordering::Greater => b.iter().zip(a).for_each(|(b, a)| {
			a[0] = f32::from_sample(*b);
			a[1] = f32::from_sample(*b);
		}),
	}
}

fn supported_config_to_config(
	supported_config: &SupportedStreamConfig,
	frames: Option<NonZero<u32>>,
) -> StreamConfig {
	let buffer_size = match (*supported_config.buffer_size(), frames) {
		(SupportedBufferSize::Unknown, _) | (_, None) => BufferSize::Default,
		(SupportedBufferSize::Range { min, max }, Some(frames)) => {
			BufferSize::Fixed(frames.get().clamp(min, max))
		}
	};

	StreamConfig {
		channels: supported_config.channels(),
		sample_rate: supported_config.sample_rate(),
		buffer_size,
	}
}

fn frames_of_config(config: &StreamConfig) -> Option<NonZero<u32>> {
	match config.buffer_size {
		BufferSize::Fixed(buffer_size) => NonZero::new(buffer_size),
		BufferSize::Default => None,
	}
}
