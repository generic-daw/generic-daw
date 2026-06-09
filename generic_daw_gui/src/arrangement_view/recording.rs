use crate::{
	arrangement_view::{
		crc,
		sample::{Sample, SamplePair},
	},
	lod::LodsBuilder,
};
use generic_daw_core::{DeviceId, NodeId, Transport, time::BeatTime};
use rtrb::Consumer;
use std::{fs::File, io::BufWriter, num::NonZero, path::Path, sync::Arc};

#[derive(Debug)]
pub struct Recording {
	pub core: generic_daw_core::Recording<BufWriter<File>>,
	pub lods: LodsBuilder,
	pub position: BeatTime,
	pub name: Arc<str>,
	pub path: Arc<Path>,
	pub node: NodeId,
}

impl Recording {
	pub fn create(
		path: Arc<Path>,
		transport: &Transport,
		device_id: Option<&DeviceId>,
		sample_rate: Option<NonZero<u32>>,
		frames: Option<NonZero<u32>>,
		node: NodeId,
	) -> (Self, Consumer<[f32; 2]>) {
		let (core, consumer) = generic_daw_core::Recording::create(
			BufWriter::new(File::create(&path).unwrap()),
			device_id,
			sample_rate,
			frames,
		);
		let name = path.file_name().unwrap().to_str().unwrap().into();

		(
			Self {
				core,
				lods: LodsBuilder::default(),
				position: transport.position.to_beat_time(transport),
				path,
				name,
				node,
			},
			consumer,
		)
	}

	pub fn len(&self, transport: &Transport) -> BeatTime {
		BeatTime::from_frames(self.core.samples().len(), transport)
			* self.core.resample_ratio(transport)
	}

	pub fn write(&mut self, samples: &[[f32; 2]]) {
		let start = self.core.samples().len();
		self.core.write(samples);
		self.lods.update(self.core.samples(), start);
	}

	pub fn split_off(&mut self, mut path: Arc<Path>, transport: &Transport) -> SamplePair {
		let core = self
			.core
			.split_off(BufWriter::new(File::create(&path).unwrap()));

		let lods = std::mem::take(&mut self.lods);

		self.position = transport.position.to_beat_time(transport);

		let name = std::mem::replace(
			&mut self.name,
			path.file_name().unwrap().to_str().unwrap().into(),
		);

		std::mem::swap(&mut self.path, &mut path);

		let gui = Sample {
			id: core.id,
			lods: lods.finalize(),
			name,
			path: path.clone(),
			samples: core.samples.clone(),
			sample_rate: core.sample_rate,
			crc: crc(File::open(&path).unwrap()),
			len: std::fs::metadata(path).unwrap().len(),
			refs: 0,
		};

		SamplePair { core, gui }
	}

	pub fn end_stream(&mut self) {
		self.core.end_stream();
	}

	pub fn finalize(self) -> SamplePair {
		let core = self.core.finalize();

		let gui = Sample {
			id: core.id,
			lods: self.lods.finalize(),
			name: self.name,
			path: self.path.clone(),
			samples: core.samples.clone(),
			sample_rate: core.sample_rate,
			crc: crc(File::open(&self.path).unwrap()),
			len: std::fs::metadata(self.path).unwrap().len(),
			refs: 0,
		};

		SamplePair { core, gui }
	}
}
