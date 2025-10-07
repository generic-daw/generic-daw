use crate::{
	arrangement_view::{
		crc,
		sample::{Sample, SamplePair},
	},
	lod::{LOD_LEVELS, update_lods},
};
use generic_daw_core::{self as core, MusicalTime, RtState};
use generic_daw_utils::NoDebug;
use rtrb::Consumer;
use std::{fs::File, io::BufWriter, path::Path, sync::Arc};

#[derive(Debug)]
pub struct Recording {
	pub core: core::Recording<BufWriter<File>>,
	pub lods: NoDebug<[Vec<(f32, f32)>; LOD_LEVELS]>,
	pub position: MusicalTime,
	pub name: Arc<str>,
	pub path: Arc<Path>,
}

impl Recording {
	pub fn create(
		path: impl AsRef<Path>,
		rtstate: &RtState,
		device_name: Option<Arc<str>>,
		sample_rate: u32,
		frames: u32,
	) -> (Self, Consumer<Box<[f32]>>) {
		let (core, consumer) = core::Recording::create(
			BufWriter::new(File::create(path.as_ref()).unwrap()),
			rtstate,
			device_name,
			sample_rate,
			frames,
		);

		(
			Self {
				core,
				lods: NoDebug([const { Vec::new() }; _]),
				position: MusicalTime::from_samples(rtstate.sample, rtstate),
				path: path.as_ref().into(),
				name: path.as_ref().file_name().unwrap().to_str().unwrap().into(),
			},
			consumer,
		)
	}

	pub fn sample_rate(&self) -> u32 {
		self.core.sample_rate()
	}

	pub fn frames(&self) -> Option<u32> {
		self.core.frames()
	}

	pub fn write(&mut self, samples: &[f32]) {
		let start = self.core.samples().len();
		self.core.write(samples);
		update_lods(self.core.samples(), &mut self.lods, start);
	}

	pub fn split_off(&mut self, path: impl AsRef<Path>, rtstate: &RtState) -> SamplePair {
		let start = self.core.samples().len();
		let core = self.core.split_off(
			BufWriter::new(File::create(path.as_ref()).unwrap()),
			rtstate,
		);
		update_lods(&core.samples, &mut self.lods, start);

		let mut lods = [const { Vec::new() }; _];
		std::mem::swap(&mut *self.lods, &mut lods);

		self.position = MusicalTime::from_samples(rtstate.sample, rtstate);

		let mut name = path.as_ref().file_name().unwrap().to_str().unwrap().into();
		std::mem::swap(&mut self.name, &mut name);

		let mut path = path.as_ref().into();
		std::mem::swap(&mut self.path, &mut path);

		SamplePair {
			gui: Sample {
				id: core.id,
				lods: lods.map(Vec::into_boxed_slice).into(),
				name,
				len: core.samples.len(),
				crc: crc(File::open(&path).unwrap()),
				path,
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
		update_lods(&core.samples, &mut self.lods, start);

		SamplePair {
			gui: Sample {
				id: core.id,
				lods: self.lods.0.map(Vec::into_boxed_slice).into(),
				name: self.name,
				len: core.samples.len(),
				crc: crc(File::open(&self.path).unwrap()),
				path: self.path,
				refs: 0,
			},
			core,
		}
	}
}
