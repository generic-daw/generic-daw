use crate::{
	LOD_LEVELS, MusicalTime, RtState, Sample, Stream, buffer_size_of_config, build_input_stream,
	lod::update_lods, resampler::Resampler,
};
use cpal::StreamConfig;
use generic_daw_utils::NoDebug;
use hound::{SampleFormat, WavSpec, WavWriter};
use rtrb::Consumer;
use std::{path::Path, sync::Arc};

#[derive(Debug)]
pub struct Recording {
	pub lods: NoDebug<Box<[Vec<(f32, f32)>; LOD_LEVELS]>>,
	path: Arc<Path>,
	pub name: Arc<str>,
	pub position: MusicalTime,

	resampler: Resampler,

	_stream: NoDebug<Stream>,
	config: StreamConfig,
}

impl Recording {
	#[must_use]
	pub fn create(
		path: Arc<Path>,
		rtstate: &RtState,
		device_name: Option<&str>,
		sample_rate: u32,
		frames: u32,
	) -> (Self, Consumer<Box<[f32]>>) {
		let (stream, config, receiver) = build_input_stream(device_name, sample_rate, frames);

		let position = MusicalTime::from_samples(rtstate.sample, rtstate);
		let name = path.file_name().unwrap().to_str().unwrap().into();

		let resampler =
			Resampler::new(config.sample_rate.0 as usize, rtstate.sample_rate as usize).unwrap();

		(
			Self {
				lods: Box::new([const { Vec::new() }; LOD_LEVELS]).into(),
				path,
				name,
				position,

				resampler,

				_stream: stream.into(),
				config,
			},
			receiver,
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

	pub fn write(&mut self, samples: &[f32]) {
		let start = self.resampler.samples().len();

		self.resampler.process(samples);

		update_lods(self.resampler.samples(), &mut self.lods, start);
	}

	pub fn split_off(&mut self, path: Arc<Path>, rtstate: &RtState) -> Arc<Sample> {
		let mut name = path.file_name().unwrap().to_str().unwrap().into();
		std::mem::swap(&mut self.name, &mut name);
		self.path = path.clone();

		let start = self.resampler.samples().len();

		let mut resampler = Resampler::new(
			self.config.sample_rate.0 as usize,
			rtstate.sample_rate as usize,
		)
		.unwrap();
		std::mem::swap(&mut self.resampler, &mut resampler);
		let samples = resampler.finish();

		let mut lods = Box::new([const { Vec::new() }; LOD_LEVELS]).into();
		std::mem::swap(&mut self.lods, &mut lods);

		update_lods(&samples, &mut lods, start);

		let mut writer = WavWriter::create(
			&path,
			WavSpec {
				channels: 2,
				sample_rate: self.config.sample_rate.0,
				bits_per_sample: 32,
				sample_format: SampleFormat::Float,
			},
		)
		.unwrap();

		for &s in &samples {
			writer.write_sample(s).unwrap();
		}

		writer.finalize().unwrap();

		Arc::new(Sample {
			samples: samples.into_boxed_slice().into(),
			lods: Box::new(lods.0.map(|x| x.into_boxed_slice())).into(),
			path,
			name,
		})
	}

	#[must_use]
	pub fn finalize(self) -> Arc<Sample> {
		let Self {
			mut lods,
			name,
			path,
			resampler,
			..
		} = self;

		let start = resampler.samples().len();
		let samples = resampler.finish();
		update_lods(&samples, &mut lods, start);

		let mut writer = WavWriter::create(
			&path,
			WavSpec {
				channels: 2,
				sample_rate: self.config.sample_rate.0,
				bits_per_sample: 32,
				sample_format: SampleFormat::Float,
			},
		)
		.unwrap();

		for &s in &samples {
			writer.write_sample(s).unwrap();
		}

		writer.finalize().unwrap();

		Arc::new(Sample {
			samples: samples.into_boxed_slice().into(),
			lods: Box::new(lods.0.map(|x| x.into_boxed_slice())).into(),
			path,
			name,
		})
	}

	#[must_use]
	pub fn len(&self) -> usize {
		self.resampler.samples().len()
	}

	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}
