use crate::{
	InputRequest, InputResponse, STREAM_THREAD, Sample, SampleId, StreamMessage, StreamToken,
	Transport, resampler::Resampler, stream::frames_of_config,
};
use cpal::StreamConfig;
use hound::{SampleFormat, WavSpec, WavWriter};
use rtrb::Consumer;
use std::{io, num::NonZero, sync::Arc};
use utils::NoDebug;

#[derive(Debug)]
pub struct Recording<W: io::Write + io::Seek> {
	resampler: Resampler,
	writer: NoDebug<WavWriter<W>>,

	stream: Option<NoDebug<StreamToken>>,
	config: StreamConfig,
}

impl<W: io::Write + io::Seek> Recording<W> {
	#[must_use]
	pub fn create(
		writer: W,
		transport: &Transport,
		device_name: Option<Arc<str>>,
		sample_rate: NonZero<u32>,
		frames: Option<NonZero<u32>>,
	) -> (Self, Consumer<Box<[f32]>>) {
		let (sender, receiver) = oneshot::channel();

		STREAM_THREAD
			.send(StreamMessage::Input(
				InputRequest {
					device_name,
					sample_rate,
					frames,
				},
				sender,
			))
			.unwrap();

		let InputResponse {
			config,
			consumer,
			token,
		} = receiver.recv().unwrap();

		let resampler = Resampler::new(
			NonZero::new(config.sample_rate.0).unwrap(),
			transport.sample_rate,
			NonZero::new(2).unwrap(),
		)
		.unwrap();

		let writer = WavWriter::new(
			writer,
			WavSpec {
				channels: 2,
				sample_rate: config.sample_rate.0,
				bits_per_sample: 32,
				sample_format: SampleFormat::Float,
			},
		)
		.unwrap();

		(
			Self {
				resampler,
				writer: writer.into(),

				stream: Some(token.into()),
				config,
			},
			consumer,
		)
	}

	#[must_use]
	pub fn sample_rate(&self) -> NonZero<u32> {
		NonZero::new(self.config.sample_rate.0).unwrap()
	}

	#[must_use]
	pub fn frames(&self) -> Option<NonZero<u32>> {
		frames_of_config(&self.config)
	}

	#[must_use]
	pub fn samples(&self) -> &[f32] {
		self.resampler.samples()
	}

	pub fn write(&mut self, samples: &[f32]) {
		let start = self.resampler.samples().len();
		self.resampler.process(samples);
		for &s in &self.resampler.samples()[start..] {
			self.writer.write_sample(s).unwrap();
		}
	}

	pub fn split_off(&mut self, writer: W, transport: &Transport) -> Sample {
		let mut resampler = Resampler::new(
			NonZero::new(self.config.sample_rate.0).unwrap(),
			transport.sample_rate,
			NonZero::new(2).unwrap(),
		)
		.unwrap();
		std::mem::swap(&mut self.resampler, &mut resampler);

		let mut writer = WavWriter::new(
			writer,
			WavSpec {
				channels: 2,
				sample_rate: self.config.sample_rate.0,
				bits_per_sample: 32,
				sample_format: SampleFormat::Float,
			},
		)
		.unwrap();
		std::mem::swap(&mut *self.writer, &mut writer);

		let start = resampler.samples().len();
		let samples = resampler.finish();

		for &s in &samples[start..] {
			writer.write_sample(s).unwrap();
		}
		writer.finalize().unwrap();

		Sample {
			id: SampleId::unique(),
			samples: NoDebug(samples.into()),
		}
	}

	pub fn end_stream(&mut self) {
		self.stream.take();
	}

	#[must_use]
	pub fn finalize(self) -> Sample {
		let Self {
			resampler,
			writer: NoDebug(mut writer),
			stream,
			..
		} = self;

		debug_assert!(stream.is_none());

		let start = resampler.samples().len();
		let samples = resampler.finish();

		for &s in &samples[start..] {
			writer.write_sample(s).unwrap();
		}
		writer.finalize().unwrap();

		Sample {
			id: SampleId::unique(),
			samples: NoDebug(samples.into()),
		}
	}
}
