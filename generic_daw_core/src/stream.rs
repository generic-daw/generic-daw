use crate::{Batch, Message, NodeId, RtState, daw_ctx::DawCtx};
use cpal::{
	BufferSize, SampleRate, StreamConfig, SupportedBufferSize, SupportedStreamConfigRange,
	traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _},
};
use generic_daw_utils::NoDebug;
use log::{error, info, trace};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{
	cmp::Ordering,
	collections::HashMap,
	num::NonZero,
	sync::{
		Arc, LazyLock,
		atomic::{AtomicUsize, Ordering::Relaxed},
		mpsc::Sender,
	},
};

static NEXT_STREAM_TOKEN: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug)]
pub struct StreamToken(Option<NonZero<usize>>);

impl StreamToken {
	#[must_use]
	pub fn unique() -> Self {
		Self(NonZero::new(NEXT_STREAM_TOKEN.fetch_add(1, Relaxed)))
	}

	#[must_use]
	pub fn get_ref(&self) -> StreamTokenRef {
		StreamTokenRef(self.0.unwrap())
	}

	#[must_use]
	pub(crate) fn take_ref(mut self) -> StreamTokenRef {
		StreamTokenRef(self.0.take().unwrap())
	}
}

impl Drop for StreamToken {
	fn drop(&mut self) {
		if self.0.is_some() {
			_ = STREAM_THREAD.send(StreamMessage::Close(Self(std::mem::take(&mut self.0))));
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StreamTokenRef(NonZero<usize>);

#[derive(Debug)]
pub enum StreamMessage {
	Input(InputRequest, oneshot::Sender<InputResponse>),
	Output(OutputRequest, oneshot::Sender<OutputResponse>),
	Pause(StreamTokenRef),
	Play(StreamTokenRef),
	Close(StreamToken),
}

#[derive(Debug)]
pub struct InputRequest {
	pub device_name: Option<Arc<str>>,
	pub sample_rate: NonZero<u32>,
	pub frames: Option<NonZero<u32>>,
}

#[derive(Debug)]
pub struct InputResponse {
	pub config: StreamConfig,
	pub consumer: Consumer<Box<[f32]>>,
	pub token: StreamToken,
}

#[derive(Debug)]
pub struct OutputRequest {
	pub device_name: Option<Arc<str>>,
	pub sample_rate: NonZero<u32>,
	pub frames: Option<NonZero<u32>>,
	pub metrics: NoDebug<&'static (dyn Fn(&mut dyn FnMut()) + Send + Sync)>,
}

#[derive(Debug)]
pub struct OutputResponse {
	pub master_node_id: NodeId,
	pub rtstate: RtState,
	pub producer: Producer<Message>,
	pub consumer: Consumer<Batch>,
	pub token: StreamToken,
}

pub static STREAM_THREAD: LazyLock<Sender<StreamMessage>> = LazyLock::new(|| {
	let (sender, receiver) = std::sync::mpsc::channel();

	std::thread::spawn(move || {
		let host = cpal::default_host();
		let mut streams = HashMap::new();

		while let Ok(msg) = receiver.recv() {
			trace!("{msg:?}");

			match msg {
				StreamMessage::Input(req, sender) => {
					let device = req
						.device_name
						.and_then(|device_name| Some((device_name, host.input_devices().ok()?)))
						.and_then(|(device_name, mut devices)| {
							devices.find(|device| {
								device.name().is_ok_and(|name| *name == *device_name)
							})
						})
						.or_else(|| host.default_input_device())
						.unwrap();

					let config = choose_config(
						device.supported_input_configs().unwrap(),
						req.sample_rate,
						req.frames,
					);

					info!("starting input stream with config {config:#?}");

					let sample_rate = NonZero::new(config.sample_rate.0).unwrap();
					let frames = frames_of_config(&config)
						.or(req.frames)
						.or(NonZero::new(8192))
						.unwrap();
					let channels = NonZero::new(config.channels.into()).unwrap();
					let buffer_len = frames.checked_mul(channels).unwrap();

					let (mut producer, consumer) =
						RingBuffer::new(sample_rate.get().div_ceil(frames.get()) as usize);

					let mut stereo = vec![0.0; 2 * frames.get() as usize].into_boxed_slice();

					let stream = device
						.build_input_stream(
							&config,
							move |buf, _| {
								for buf in buf.chunks(buffer_len.get() as usize) {
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

					let token = StreamToken::unique();
					let stream = streams.insert(token.get_ref(), stream);
					debug_assert!(stream.is_none());

					sender
						.send(InputResponse {
							config,
							consumer,
							token,
						})
						.unwrap();
				}
				StreamMessage::Output(req, sender) => {
					let device = req
						.device_name
						.and_then(|device_name| Some((device_name, host.output_devices().ok()?)))
						.and_then(|(device_name, mut devices)| {
							devices.find(|device| {
								device.name().is_ok_and(|name| *name == *device_name)
							})
						})
						.or_else(|| host.default_output_device())
						.unwrap();

					let config = choose_config(
						device.supported_output_configs().unwrap(),
						req.sample_rate,
						req.frames,
					);

					info!("starting output stream with config {config:#?}");

					let sample_rate = NonZero::new(config.sample_rate.0).unwrap();
					let frames = frames_of_config(&config)
						.or(req.frames)
						.or(NonZero::new(8192))
						.unwrap();
					let channels = NonZero::new(config.channels.into()).unwrap();
					let buffer_len = frames.checked_mul(channels).unwrap();

					let rtstate = RtState::new(sample_rate, frames);
					let (mut ctx, master_node_id, producer, consumer) = DawCtx::create(rtstate);

					let mut stereo = vec![0.0; 2 * frames.get() as usize].into_boxed_slice();

					let stream = device
						.build_output_stream(
							&config,
							move |buf, _| {
								(req.metrics)(&mut || {
									for buf in buf.chunks_mut(buffer_len.get() as usize) {
										let frames = buf.len() / channels.get() as usize;
										ctx.process(&mut stereo[..2 * frames]);
										from_stereo_to_other(buf, &stereo[..2 * frames], frames);
									}
								});
							},
							|err| error!("{err}"),
							None,
						)
						.unwrap();

					stream.play().unwrap();

					let token = StreamToken::unique();
					let stream = streams.insert(token.get_ref(), stream);
					debug_assert!(stream.is_none());

					sender
						.send(OutputResponse {
							master_node_id,
							rtstate,
							producer,
							consumer,
							token,
						})
						.unwrap();
				}
				StreamMessage::Play(token) => {
					streams.get_mut(&token).unwrap().play().unwrap();
				}
				StreamMessage::Pause(token) => {
					streams.get_mut(&token).unwrap().pause().unwrap();
				}
				StreamMessage::Close(token) => {
					let stream = streams.remove(&token.take_ref());
					debug_assert!(stream.is_some());
				}
			}
		}
	});

	sender
});

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

	let sample_rate = SampleRate(
		sample_rate
			.get()
			.clamp(config.min_sample_rate().0, config.max_sample_rate().0),
	);

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

fn frames_of_config(config: &StreamConfig) -> Option<NonZero<u32>> {
	match config.buffer_size {
		BufferSize::Fixed(buffer_size) => NonZero::new(buffer_size),
		BufferSize::Default => None,
	}
}
