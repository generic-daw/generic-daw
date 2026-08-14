use crate::{
	DeviceDescription, DeviceId, HostId, Stream,
	audio_thread::{AudioCallback, AudioThread, MidiAction},
};
use cpal::{
	BufferSize, Device, FromSample, I24, InputCallbackInfo, OutputCallbackInfo, Sample,
	SampleFormat, StreamConfig, SupportedBufferSize, U24,
	traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _},
};
use log::{error, warn};
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use midly::{MidiMessage, live::LiveEvent};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{
	collections::HashMap,
	mem::MaybeUninit,
	num::NonZero,
	sync::{Arc, LazyLock},
};
use utils::boxed_slice;

pub static DEFAULT_HOST: LazyLock<HostId> = LazyLock::new(|| cpal::default_host().id());

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Channels {
	pub left: u16,
	pub right: u16,
	pub midi: u16,
	pub enable_audio: bool,
	pub enable_midi: bool,
}

impl Channels {
	#[must_use]
	pub fn base(channels: u16) -> Self {
		Self {
			left: 0,
			right: channels.clamp(1, 2) - 1,
			midi: 0,
			enable_audio: false,
			enable_midi: false,
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

	#[must_use]
	pub fn midi(self, midi: u16) -> Self {
		Self { midi, ..self }
	}

	#[must_use]
	pub fn enable_audio(self, enable_audio: bool) -> Self {
		Self {
			enable_audio,
			..self
		}
	}

	#[must_use]
	pub fn enable_midi(self, enable_midi: bool) -> Self {
		Self {
			enable_midi,
			..self
		}
	}
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Devices {
	#[default]
	Default,
	WithHost {
		host: HostId,
		input: Option<Arc<str>>,
		output: Option<Arc<str>>,
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

#[must_use]
pub fn get_input_ports() -> HashMap<Arc<str>, Arc<str>> {
	MidiInput::new("Generic DAW")
		.into_iter()
		.flat_map(|input| {
			input.ports().into_iter().filter_map(move |port| {
				Some((port.id().into(), input.port_name(&port).ok()?.into()))
			})
		})
		.collect()
}

#[must_use]
pub fn get_output_ports() -> HashMap<Arc<str>, Arc<str>> {
	MidiOutput::new("Generic DAW")
		.into_iter()
		.flat_map(|input| {
			input.ports().into_iter().filter_map(move |port| {
				Some((port.id().into(), input.port_name(&port).ok()?.into()))
			})
		})
		.collect()
}

pub fn build_streams(
	devices: &Devices,
	input_port: Option<&str>,
	output_port: Option<&str>,
	sample_rate: Option<NonZero<u32>>,
	frames: Option<NonZero<u32>>,
	receiver: oneshot::Receiver<AudioThread>,
) -> (Stream, u16, NonZero<u16>, NonZero<u32>, NonZero<u32>) {
	let (midi_input_connection, midi_consumer) = build_midi_input_connection(input_port);

	let midi_output_connection = build_midi_output_connection(output_port);

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
		.filter(|sample_rate| {
			output_device
				.supported_output_configs()
				.unwrap()
				.any(|config| config.contains_rate(sample_rate.get()))
		})
		.or_else(|| NonZero::new(output_device.default_output_config().unwrap().sample_rate()))
		.unwrap();

	let frames = frames.filter(|frames| {
		output_device
			.supported_output_configs()
			.unwrap()
			.any(|config| matches!(config.buffer_size(), &SupportedBufferSize::Range { min, max } if (min..=max).contains(&frames.get())))
	});

	let (audio_input_stream, input_channels, audio_consumer) =
		build_audio_input_stream(input_device.as_ref(), sample_rate, frames);

	let (audio_output_stream, output_channels) = build_audio_output_stream(
		&output_device,
		sample_rate,
		frames,
		input_channels,
		AudioCallback::Away(receiver),
		midi_input_connection,
		midi_output_connection,
		midi_consumer,
		audio_input_stream,
		audio_consumer,
	);

	audio_output_stream.play().unwrap();

	(
		audio_output_stream,
		input_channels,
		output_channels,
		sample_rate,
		frames.or(NonZero::new(2048)).unwrap(),
	)
}

fn build_midi_input_connection(
	port: Option<&str>,
) -> (Option<MidiInputConnection<()>>, Consumer<MidiAction>) {
	fn build_midi_input_connection(
		port: Option<&str>,
	) -> Option<(MidiInputConnection<()>, Consumer<MidiAction>)> {
		let port = port?;

		let mut input = MidiInput::new("Generic DAW").ok()?;
		let port = input.find_port_by_id(port)?;

		input.ignore(Ignore::All);

		let (producer, consumer) = RingBuffer::new(2048);

		Some((
			input
				.connect(
					&port,
					"Generic DAW",
					build_midi_input_callback(producer),
					(),
				)
				.inspect_err(|err| warn!("{err}"))
				.ok()?,
			consumer,
		))
	}

	let Some((stream, consumer)) = build_midi_input_connection(port) else {
		return (None, RingBuffer::new(0).1);
	};

	(Some(stream), consumer)
}

fn build_midi_input_callback(
	mut producer: Producer<MidiAction>,
) -> impl FnMut(u64, &[u8], &mut ()) {
	move |_, raw, ()| {
		let Ok(event) = LiveEvent::parse(raw).inspect_err(|err| warn!("{err}")) else {
			return;
		};

		let event = match event {
			LiveEvent::Midi {
				channel,
				message: MidiMessage::NoteOn { key, vel },
			} if vel != 0 => MidiAction::NoteOn(channel, key, vel),
			LiveEvent::Midi {
				channel,
				message: MidiMessage::NoteOff { key, vel } | MidiMessage::NoteOn { key, vel },
			} => MidiAction::NoteOff(channel, key, vel),
			_ => return,
		};

		if producer.push(event).is_err() {
			warn!("full ring buffer");
		}
	}
}

fn build_midi_output_connection(port: Option<&str>) -> Option<MidiOutputConnection> {
	let port = port?;

	let input = MidiOutput::new("Generic DAW").ok()?;

	let port = input.find_port_by_id(port)?;

	input
		.connect(&port, "Generic DAW")
		.inspect_err(|err| warn!("{err}"))
		.ok()
}

fn build_audio_input_stream(
	device: Option<&Device>,
	sample_rate: NonZero<u32>,
	frames: Option<NonZero<u32>>,
) -> (Option<Stream>, u16, Consumer<f32>) {
	fn build_audio_input_stream(
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
			RingBuffer::new(usize::from(channels.get()) * sample_rate.get() as usize);

		macro_rules! build_audio_input_stream {
			($($pat:pat => $ty:ty),*$(,)?) => {
				match device.default_input_config()?.sample_format() {
					$(
						$pat => device.build_input_stream(
							config,
							build_audio_input_callback::<$ty>(frames.or(NonZero::new(2048)).unwrap(), channels, producer),
							|err| error!("{err}"),
							None,
						)?,
					)*
					sample_format => panic!("unsupported sample format {sample_format}"),
				}
			}
		}

		Ok((
			build_audio_input_stream! {
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

	let Ok((stream, channels, consumer)) = build_audio_input_stream(device, sample_rate, frames)
		.inspect_err(|err| _ = err.as_ref().inspect(|err| warn!("{err}")))
	else {
		return (None, 0, RingBuffer::new(0).1);
	};

	(Some(stream), channels.get(), consumer)
}

fn build_audio_input_callback<T: Sample>(
	frames: NonZero<u32>,
	channels: NonZero<u16>,
	mut producer: Producer<f32>,
) -> impl FnMut(&[T], &InputCallbackInfo)
where
	f32: FromSample<T>,
{
	let chunk_size = NonZero::new(frames.get() * u32::from(channels.get())).unwrap();
	let mut audio_in = boxed_slice![0.0; chunk_size.get() as usize];
	move |buf, _| {
		for buf in buf.chunks(chunk_size.get() as usize) {
			for (buf, input) in buf.iter().zip(&mut audio_in[..buf.len()]) {
				*input = f32::from_sample(*buf);
			}

			if let (_, rest) = producer.push_partial_slice(&audio_in[..buf.len()])
				&& !rest.is_empty()
			{
				warn!("full ring buffer");
			}
		}
	}
}

fn build_audio_output_stream(
	device: &Device,
	sample_rate: NonZero<u32>,
	frames: Option<NonZero<u32>>,
	input_channels: u16,
	processor: AudioCallback,
	midi_input: Option<MidiInputConnection<()>>,
	midi_output: Option<MidiOutputConnection>,
	midi_consumer: Consumer<MidiAction>,
	audio_input: Option<Stream>,
	audio_consumer: Consumer<f32>,
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

	macro_rules! build_audio_output_stream {
		($($pat:pat => $ty:ty),*$(,)?) => {
			match device.default_output_config().unwrap().sample_format() {
				$(
					$pat => device.build_output_stream(
						config,
						build_audio_output_callback::<$ty>(frames.or(NonZero::new(2048)).unwrap(), input_channels, channels, midi_input, midi_output, midi_consumer, audio_input, audio_consumer, processor),
						|err| error!("{err}"),
						None,
					).unwrap(),
				)*
				sample_format => panic!("unsupported sample format {sample_format}"),
			}
		}
	}

	(
		build_audio_output_stream! {
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

fn build_audio_output_callback<T: Sample + FromSample<f32>>(
	frames: NonZero<u32>,
	input_channels: u16,
	output_channels: NonZero<u16>,
	midi_input: Option<MidiInputConnection<()>>,
	mut midi_output: Option<MidiOutputConnection>,
	mut midi_consumer: Consumer<MidiAction>,
	mut audio_input: Option<Stream>,
	mut audio_consumer: Consumer<f32>,
	mut processor: AudioCallback,
) -> impl FnMut(&mut [T], &OutputCallbackInfo) {
	let chunk_size = NonZero::new(frames.get() * u32::from(output_channels.get())).unwrap();
	let mut midi_in = boxed_slice![MaybeUninit::uninit(); midi_consumer.buffer().capacity()];
	let mut audio_in = boxed_slice![0.0; frames.get() as usize * usize::from(input_channels)];
	let mut audio_out = boxed_slice![0.0; chunk_size.get() as usize];
	let mut warn = false;

	if let Some(audio_input) = &mut audio_input {
		audio_input.play().unwrap();
	}

	move |buf, _| {
		for buf in buf.chunks_mut(chunk_size.get() as usize) {
			let frames = buf.len() / usize::from(output_channels.get());
			let input_len = frames * usize::from(input_channels);

			midi_input.as_ref();
			let midi_input = midi_consumer.pop_partial_slice_uninit(&mut midi_in).0;

			audio_input.as_ref();
			if let (_, rest) = audio_consumer.pop_partial_slice(&mut audio_in[..input_len])
				&& !rest.is_empty()
			{
				if warn {
					warn!("empty ring buffer");
				}

				rest.fill(0.0);
			} else {
				warn = true;
			}

			audio_out[..buf.len()].fill(0.0);

			processor.process(
				midi_input,
				midi_output.as_mut(),
				&audio_in[..input_len],
				&mut audio_out[..buf.len()],
			);

			for (output, buf) in audio_out[..buf.len()].iter().zip(buf) {
				*buf = T::from_sample(*output);
			}
		}
	}
}
