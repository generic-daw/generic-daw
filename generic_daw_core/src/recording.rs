use crate::{
	DeviceId, Sample, SampleId, Stream, Transport, build_input_stream, stream::frames_of_config,
};
use cpal::StreamConfig;
use hound::{SampleFormat, WavSpec, WavWriter};
use rtrb::Consumer;
use std::{io, num::NonZero};
use utils::NoDebug;

#[derive(Debug)]
pub struct Recording<W: io::Write + io::Seek> {
	writer: NoDebug<WavWriter<W>>,
	samples: Vec<f32>,

	stream: Option<NoDebug<Stream>>,
	config: StreamConfig,
}

impl<W: io::Write + io::Seek> Recording<W> {
	#[must_use]
	pub fn create(
		writer: W,
		device_id: Option<&DeviceId>,
		sample_rate: Option<NonZero<u32>>,
		frames: Option<NonZero<u32>>,
	) -> (Self, Consumer<f32>) {
		let (config, consumer, stream) = build_input_stream(device_id, sample_rate, frames);

		let writer = WavWriter::new(
			writer,
			WavSpec {
				channels: 2,
				sample_rate: config.sample_rate,
				bits_per_sample: 32,
				sample_format: SampleFormat::Float,
			},
		)
		.unwrap();

		(
			Self {
				writer: writer.into(),
				samples: Vec::new(),

				stream: Some(stream.into()),
				config,
			},
			consumer,
		)
	}

	#[must_use]
	pub fn resample_ratio(&self, transport: &Transport) -> f64 {
		f64::from(transport.sample_rate.get()) / f64::from(self.config.sample_rate)
	}

	#[must_use]
	pub fn sample_rate(&self) -> NonZero<u32> {
		NonZero::new(self.config.sample_rate).unwrap()
	}

	#[must_use]
	pub fn frames(&self) -> NonZero<u32> {
		frames_of_config(&self.config)
			.or(NonZero::new(2048))
			.unwrap()
	}

	#[must_use]
	pub fn samples(&self) -> &[f32] {
		&self.samples
	}

	pub fn write(&mut self, samples: &[f32]) {
		self.samples.extend_from_slice(samples);
		for &s in samples {
			self.writer.write_sample(s).unwrap();
		}
	}

	pub fn split_off(&mut self, writer: W) -> Sample {
		let samples = std::mem::take(&mut self.samples);

		let writer = std::mem::replace(
			&mut *self.writer,
			WavWriter::new(
				writer,
				WavSpec {
					channels: 2,
					sample_rate: self.config.sample_rate,
					bits_per_sample: 32,
					sample_format: SampleFormat::Float,
				},
			)
			.unwrap(),
		);

		writer.finalize().unwrap();

		Sample {
			id: SampleId::unique(),
			samples: NoDebug(samples.into()),
			sample_rate: NonZero::new(self.config.sample_rate).unwrap(),
		}
	}

	pub fn end_stream(&mut self) {
		self.stream.take();
	}

	#[must_use]
	pub fn finalize(self) -> Sample {
		let Self {
			samples,
			writer: NoDebug(writer),
			stream,
			..
		} = self;

		debug_assert!(stream.is_none());

		writer.finalize().unwrap();

		Sample {
			id: SampleId::unique(),
			samples: NoDebug(samples.into()),
			sample_rate: NonZero::new(self.config.sample_rate).unwrap(),
		}
	}
}
