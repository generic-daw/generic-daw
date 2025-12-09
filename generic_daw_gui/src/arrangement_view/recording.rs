use crate::{
	arrangement_view::{
		crc,
		sample::{Sample, SamplePair},
	},
	lod::Lods,
};
use generic_daw_core::{self as core, MusicalTime, Transport};
use rtrb::Consumer;
use std::{fs::File, io::BufWriter, num::NonZero, path::Path, sync::Arc};

#[derive(Debug)]
pub struct Recording {
	pub core: core::Recording<BufWriter<File>>,
	pub lods: Lods<Vec<(f32, f32)>>,
	pub position: MusicalTime,
	pub name: Arc<str>,
	pub path: Arc<Path>,
}

impl Recording {
	pub fn create(
		path: Arc<Path>,
		transport: &Transport,
		device_name: Option<Arc<str>>,
		sample_rate: NonZero<u32>,
		frames: Option<NonZero<u32>>,
	) -> (Self, Consumer<Box<[f32]>>) {
		let (core, consumer) = core::Recording::create(
			BufWriter::new(File::create(&path).unwrap()),
			transport,
			device_name,
			sample_rate,
			frames,
		);
		let name = path.file_name().unwrap().to_str().unwrap().into();

		(
			Self {
				core,
				lods: Lods::default(),
				position: MusicalTime::from_samples(transport.sample, transport),
				path,
				name,
			},
			consumer,
		)
	}

	pub fn sample_rate(&self) -> NonZero<u32> {
		self.core.sample_rate()
	}

	pub fn frames(&self) -> Option<NonZero<u32>> {
		self.core.frames()
	}

	pub fn write(&mut self, samples: &[f32]) {
		let start = self.core.samples().len();
		self.core.write(samples);
		self.lods.update(self.core.samples(), start);
	}

	pub fn split_off(&mut self, mut path: Arc<Path>, transport: &Transport) -> SamplePair {
		let start = self.core.samples().len();
		let core = self
			.core
			.split_off(BufWriter::new(File::create(&path).unwrap()), transport);
		self.lods.update(self.core.samples(), start);

		let mut lods = Lods::default();
		std::mem::swap(&mut self.lods, &mut lods);

		self.position = MusicalTime::from_samples(transport.sample, transport);

		let mut name = path.file_name().unwrap().to_str().unwrap().into();
		std::mem::swap(&mut self.name, &mut name);

		std::mem::swap(&mut self.path, &mut path);

		SamplePair {
			gui: Sample {
				id: core.id,
				lods: lods.finalize(),
				name,
				path: path.clone(),
				samples: core.samples.clone(),
				crc: crc(File::open(&path).unwrap()),
				refs: 0,
			},
			core,
		}
	}

	pub fn end_stream(&mut self) {
		self.core.end_stream();
	}

	pub fn finalize(mut self) -> SamplePair {
		let start = self.core.samples().len();
		let core = self.core.finalize();
		self.lods.update(&core.samples, start);

		SamplePair {
			gui: Sample {
				id: core.id,
				lods: self.lods.finalize(),
				name: self.name,
				path: self.path.clone(),
				samples: core.samples.clone(),
				crc: crc(File::open(&self.path).unwrap()),
				refs: 0,
			},
			core,
		}
	}
}
