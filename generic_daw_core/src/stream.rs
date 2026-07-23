use crate::{
	DeviceDescription, DeviceId, HostId, Stream,
	audio_thread::{AudioCallback, AudioThread},
};
use cpal::{
	BufferSize, Device, FromSample, I24, InputCallbackInfo, OutputCallbackInfo, Sample,
	SampleFormat, StreamConfig, U24,
	traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _},
};
use log::{error, warn};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{collections::HashMap, num::NonZero, sync::LazyLock};
use utils::boxed_slice;

pub static DEFAULT_HOST: LazyLock<HostId> = LazyLock::new(|| cpal::default_host().id());

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub struct Channels {
	pub left: u16,
	pub right: u16,
}

impl Channels {
	#[must_use]
	pub fn base(channels: NonZero<u16>) -> Self {
		Self {
			left: 0,
			right: channels.get().min(2) - 1,
		}
	}

	#[must_use]
	pub fn fits_in(self, channels: u16) -> bool {
		self.left < channels && self.right < channels
	}

	#[must_use]
	pub fn left(self, left: u16) -> Self {
		Self { left, ..self }
	}

	#[must_use]
	pub fn right(self, right: u16) -> Self {
		Self { right, ..self }
	}
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Devices {
	#[default]
	Default,
	WithHost {
		host: HostId,
		input: Option<Box<str>>,
		output: Option<Box<str>>,
	},
}

impl Devices {
	#[must_use]
	pub fn host(&self) -> Option<HostId> {
		match self {
			Self::Default => None,
			Self::WithHost { host, .. } => Some(*host),
		}
	}

	#[must_use]
	pub fn input(&self) -> Option<DeviceId> {
		match self {
			Self::WithHost {
				host,
				input: Some(input),
				..
			} => Some(DeviceId::new(*host, input)),
			Self::Default | Self::WithHost { .. } => None,
		}
	}

	#[must_use]
	pub fn output(&self) -> Option<DeviceId> {
		match self {
			Self::WithHost {
				host,
				output: Some(output),
				..
			} => Some(DeviceId::new(*host, output)),
			Self::Default | Self::WithHost { .. } => None,
		}
	}
}

#[must_use]
pub fn get_devices() -> HashMap<DeviceId, DeviceDescription> {
	cpal::available_hosts()
		.into_iter()
		.filter_map(|host| cpal::host_from_id(host).ok())
		.filter_map(|host| host.devices().ok())
		.flatten()
		.filter_map(|device| Some((device.id().ok()?, device.description().ok()?)))
		.collect()
}

pub fn build_audio_streams(
	devices: &Devices,
	sample_rate: Option<NonZero<u32>>,
	frames: Option<NonZero<u32>>,
	receiver: oneshot::Receiver<AudioThread>,
) -> (
	Option<Stream>,
	Stream,
	u16,
	NonZero<u16>,
	NonZero<u32>,
	NonZero<u32>,
) {
	let host = devices
		.host()
		.and_then(|host| cpal::host_from_id(host).ok())
		.unwrap_or_else(cpal::default_host);

	let input_device = devices
		.input()
		.and_then(|device| host.device_by_id(&device))
		.filter(Device::supports_input)
		.or_else(|| host.default_input_device());

	let output_device = devices
		.output()
		.and_then(|device| host.device_by_id(&device))
		.filter(Device::supports_output)
		.or_else(|| host.default_output_device())
		.unwrap();

	let sample_rate = sample_rate
		.or_else(|| NonZero::new(output_device.default_output_config().unwrap().sample_rate()))
		.unwrap();

	let (input_stream, input_channels, consumer) =
		build_input_stream(input_device.as_ref(), sample_rate, frames);

	let (output_stream, output_channels) = build_output_stream(
		&output_device,
		sample_rate,
		frames,
		input_channels,
		receiver,
		consumer,
	);

	if let Some(input_stream) = &input_stream {
		input_stream.play().unwrap();
	}

	output_stream.play().unwrap();

	(
		input_stream,
		output_stream,
		input_channels,
		output_channels,
		sample_rate,
		frames.or(NonZero::new(2048)).unwrap(),
	)
}

pub fn build_input_stream(
	device: Option<&Device>,
	sample_rate: NonZero<u32>,
	frames: Option<NonZero<u32>>,
) -> (Option<Stream>, u16, Consumer<f32>) {
	pub fn build_input_stream(
		device: Option<&Device>,
		sample_rate: NonZero<u32>,
		frames: Option<NonZero<u32>>,
	) -> Result<(Stream, NonZero<u16>, Consumer<f32>), Option<cpal::Error>> {
		let device = device.ok_or(None)?;

		let channels = device
			.supported_input_configs()?
			.map(|config| config.channels())
			.max()
			.and_then(NonZero::new)
			.ok_or(None)?;

		let config = StreamConfig {
			channels: channels.into(),
			sample_rate: sample_rate.get(),
			buffer_size: frames.map_or(BufferSize::Default, |frames| {
				BufferSize::Fixed(frames.get())
			}),
		};

		let (producer, consumer) =
			RingBuffer::new(channels.get() as usize * sample_rate.get() as usize);

		macro_rules! build_input_stream {
			($($pat:pat => $ty:ty),*$(,)?) => {
				match device.default_input_config()?.sample_format() {
					$(
						$pat => device.build_input_stream(
							config,
							build_input_callback::<$ty>(frames.or(NonZero::new(2048)).unwrap(), channels, producer),
							|err| error!("{err}"),
							None,
						)?,
					)*
					sample_format => panic!("unsupported sample format {sample_format}"),
				}
			}
		}

		Ok((
			build_input_stream! {
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
			},
			channels,
			consumer,
		))
	}

	let Ok((stream, channels, consumer)) = build_input_stream(device, sample_rate, frames)
		.inspect_err(|err| _ = err.as_ref().inspect(|err| warn!("{err}")))
	else {
		return (None, 0, RingBuffer::new(0).1);
	};

	(Some(stream), channels.get(), consumer)
}

fn build_input_callback<T: Sample>(
	frames: NonZero<u32>,
	channels: NonZero<u16>,
	mut producer: Producer<f32>,
) -> impl FnMut(&[T], &InputCallbackInfo)
where
	f32: FromSample<T>,
{
	let chunk_size = NonZero::new(frames.get() * u32::from(channels.get())).unwrap();
	let mut input = boxed_slice![0.0; chunk_size.get() as usize];
	move |buf, _| {
		for buf in buf.chunks(chunk_size.get() as usize) {
			for (buf, input) in buf.iter().zip(&mut input[..buf.len()]) {
				*input = f32::from_sample(*buf);
			}

			if let (_, t) = producer.push_partial_slice(&input[..buf.len()])
				&& !t.is_empty()
			{
				warn!("full ring buffer");
			}
		}
	}
}

pub fn build_output_stream(
	device: &Device,
	sample_rate: NonZero<u32>,
	frames: Option<NonZero<u32>>,
	input_channels: u16,
	receiver: oneshot::Receiver<AudioThread>,
	consumer: Consumer<f32>,
) -> (Stream, NonZero<u16>) {
	let channels = device
		.supported_output_configs()
		.unwrap()
		.map(|config| config.channels())
		.max()
		.and_then(NonZero::new)
		.unwrap();

	let config = StreamConfig {
		channels: channels.get(),
		sample_rate: sample_rate.get(),
		buffer_size: frames.map_or(BufferSize::Default, |frames| {
			BufferSize::Fixed(frames.get())
		}),
	};

	macro_rules! build_output_stream {
		($($pat:pat => $ty:ty),*$(,)?) => {
			match device.default_output_config().unwrap().sample_format() {
				$(
					$pat => device.build_output_stream(
						config,
						build_output_callback::<$ty>(frames.or(NonZero::new(2048)).unwrap(), input_channels, channels, consumer, AudioCallback::Away(receiver)),
						|err| error!("{err}"),
						None,
					).unwrap(),
				)*
				sample_format => panic!("unsupported sample format {sample_format}"),
			}
		}
	}

	(
		build_output_stream! {
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
		},
		channels,
	)
}

fn build_output_callback<T: Sample + FromSample<f32>>(
	frames: NonZero<u32>,
	input_channels: u16,
	output_channels: NonZero<u16>,
	mut consumer: Consumer<f32>,
	mut processor: AudioCallback,
) -> impl FnMut(&mut [T], &OutputCallbackInfo) {
	let chunk_size = NonZero::new(frames.get() * u32::from(output_channels.get())).unwrap();
	let mut input = boxed_slice![0.0; frames.get() as usize * input_channels as usize];
	let mut output = boxed_slice![0.0; frames.get() as usize * output_channels.get() as usize];
	let mut warn = false;
	move |buf, _| {
		for buf in buf.chunks_mut(chunk_size.get() as usize) {
			let frames = buf.len() / output_channels.get() as usize;
			let input_len = frames * input_channels as usize;

			if let (_, t) = consumer.pop_partial_slice(&mut input[..input_len])
				&& !t.is_empty()
			{
				if warn {
					warn!("empty ring buffer");
				}

				t.fill(0.0);
			} else {
				warn = true;
			}

			output[..buf.len()].fill(0.0);

			processor.process(&input[..input_len], &mut output[..buf.len()]);

			for (output, buf) in output[..buf.len()].iter().zip(buf) {
				*buf = T::from_sample(*output);
			}
		}
	}
}
