use crate::{DeviceId, Sample, SampleId, Stream, Transport, build_input_stream};
use hound::{SampleFormat, WavSpec, WavWriter};
use rtrb::Consumer;
use std::{io, num::NonZero};
use utils::NoDebug;

#[derive(Debug)]
pub struct Recording<W: io::Write + io::Seek> {
	writer: NoDebug<WavWriter<W>>,
	samples: Vec<[f32; 2]>,

	stream: Option<NoDebug<Stream>>,
	sample_rate: NonZero<u32>,
	frames: NonZero<u32>,
}

impl<W: io::Write + io::Seek> Recording<W> {
	#[must_use]
	pub fn create(
		writer: W,
		device_id: Option<&DeviceId>,
		sample_rate: Option<NonZero<u32>>,
		frames: Option<NonZero<u32>>,
	) -> (Self, Consumer<[f32; 2]>) {
		let (consumer, stream, sample_rate, frames) =
			build_input_stream(device_id, sample_rate, frames);

		let writer = WavWriter::new(
			writer,
			WavSpec {
				channels: 2,
				sample_rate: sample_rate.get(),
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
				sample_rate,
				frames,
			},
			consumer,
		)
	}

	#[must_use]
	pub fn resample_ratio(&self, transport: &Transport) -> f64 {
		f64::from(transport.sample_rate.get()) / f64::from(self.sample_rate.get())
	}

	#[must_use]
	pub fn sample_rate(&self) -> NonZero<u32> {
		self.sample_rate
	}

	#[must_use]
	pub fn frames(&self) -> NonZero<u32> {
		self.frames
	}

	#[must_use]
	pub fn samples(&self) -> &[[f32; 2]] {
		&self.samples
	}

	pub fn write(&mut self, samples: &[[f32; 2]]) {
		self.samples.extend_from_slice(samples);
		for &s in samples.as_flattened() {
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
					sample_rate: self.sample_rate.get(),
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
			sample_rate: self.sample_rate,
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
			sample_rate: self.sample_rate,
		}
	}
}
