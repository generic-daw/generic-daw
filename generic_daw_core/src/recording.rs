use crate::{
	RtState, Sample, SampleId, Stream, buffer_size_of_config, build_input_stream,
	resampler::Resampler,
};
use cpal::StreamConfig;
use generic_daw_utils::NoDebug;
use hound::{SampleFormat, WavSpec, WavWriter};
use rtrb::Consumer;
use std::io;

#[derive(Debug)]
pub struct Recording<W: io::Write + io::Seek> {
	resampler: Resampler,
	writer: NoDebug<WavWriter<W>>,

	stream: NoDebug<Stream>,
	config: StreamConfig,
}

impl<W: io::Write + io::Seek> Recording<W> {
	#[must_use]
	pub fn create(
		writer: W,
		rtstate: &RtState,
		device_name: Option<&str>,
		sample_rate: u32,
		frames: u32,
	) -> (Self, Consumer<Box<[f32]>>) {
		let (stream, config, consumer) = build_input_stream(device_name, sample_rate, frames);

		let resampler = Resampler::new(
			config.sample_rate.0 as usize,
			rtstate.sample_rate as usize,
			2,
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

				stream: stream.into(),
				config,
			},
			consumer,
		)
	}

	#[must_use]
	pub fn sample_rate(&self) -> u32 {
		self.config.sample_rate.0
	}

	#[must_use]
	pub fn frames(&self) -> Option<u32> {
		buffer_size_of_config(&self.config)
			.map(|buffer_size| buffer_size / u32::from(self.config.channels))
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

	pub fn split_off(&mut self, writer: W, rtstate: &RtState) -> Sample {
		let mut resampler = Resampler::new(
			self.config.sample_rate.0 as usize,
			rtstate.sample_rate as usize,
			2,
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
		.unwrap()
		.into();
		std::mem::swap(&mut self.writer, &mut writer);

		let start = resampler.samples().len();
		let samples = resampler.finish();

		for &s in &samples[start..] {
			writer.write_sample(s).unwrap();
		}
		writer.0.finalize().unwrap();

		Sample {
			id: SampleId::unique(),
			samples: samples.into_boxed_slice().into(),
		}
	}

	#[must_use]
	pub fn finalize(self) -> Sample {
		let Self {
			resampler,
			mut writer,
			stream,
			..
		} = self;

		drop(stream);

		let start = resampler.samples().len();
		let samples = resampler.finish();

		for &s in &samples[start..] {
			writer.write_sample(s).unwrap();
		}
		writer.0.finalize().unwrap();

		Sample {
			id: SampleId::unique(),
			samples: samples.into_boxed_slice().into(),
		}
	}
}
