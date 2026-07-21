use crate::{arrangement_view::sample::SamplePair, lod::LodsBuilder};
use generic_daw_core::{Sample, SampleId, Transport, time::BeatTime};
use hound::{WavSpec, WavWriter};
use std::{fs::File, io::BufWriter, num::NonZero, path::Path, sync::Arc};
use utils::NoDebug;

#[derive(Debug)]
pub struct Recording {
	pub writer: NoDebug<WavWriter<BufWriter<File>>>,
	pub samples: Vec<[f32; 2]>,
	pub lods: LodsBuilder,
	pub position: BeatTime,
	pub name: Arc<str>,
	pub path: Arc<Path>,
}

impl Recording {
	pub fn new(path: Arc<Path>, transport: &Transport) -> Self {
		let name = path.file_name().unwrap().to_str().unwrap().into();

		let writer = WavWriter::new(
			BufWriter::new(File::create(&path).unwrap()),
			WavSpec {
				bits_per_sample: 32,
				channels: 2,
				sample_format: hound::SampleFormat::Float,
				sample_rate: transport.sample_rate.get(),
			},
		)
		.unwrap()
		.into();

		Self {
			writer,
			samples: Vec::new(),
			lods: LodsBuilder::default(),
			position: transport.position.to_beat_time(transport),
			path,
			name,
		}
	}

	pub fn end(&self, transport: &Transport) -> BeatTime {
		self.position + self.len(transport)
	}

	pub fn len(&self, transport: &Transport) -> BeatTime {
		BeatTime::from_frames(self.samples.len(), transport)
	}

	pub fn recorded(&mut self, samples: &[[f32; 2]]) {
		for &[l, r] in samples {
			self.writer.write_sample(l).unwrap();
			self.writer.write_sample(r).unwrap();
		}

		let start = self.samples.len();
		self.samples.extend_from_slice(samples);
		self.lods.update(&self.samples, start);
	}

	pub fn finalize(self) -> (BeatTime, SamplePair) {
		(
			self.position,
			SamplePair::from_core_and_lods(
				Sample {
					id: SampleId::unique(),
					samples: NoDebug(self.samples.into()),
					sample_rate: NonZero::new(self.writer.spec().sample_rate).unwrap(),
				},
				self.lods.finalize(),
				self.path,
			)
			.unwrap(),
		)
	}
}
