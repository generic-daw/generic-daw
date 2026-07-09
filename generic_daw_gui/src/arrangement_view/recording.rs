use crate::{arrangement_view::sample::SamplePair, lod::LodsBuilder};
use generic_daw_core::{Device, NodeId, Transport, time::BeatTime};
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
		device: &Device,
		sample_rate: Option<NonZero<u32>>,
		frames: Option<NonZero<u32>>,
		node: NodeId,
	) -> (Self, Consumer<[f32; 2]>) {
		let (core, consumer) = generic_daw_core::Recording::create(
			BufWriter::new(File::create(&path).unwrap()),
			device,
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

	pub fn end(&self, transport: &Transport) -> BeatTime {
		self.position + self.len(transport)
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
		self.name = path.file_name().unwrap().to_str().unwrap().into();
		std::mem::swap(&mut self.path, &mut path);

		SamplePair::from_core_and_lods(core, lods.finalize(), path).unwrap()
	}

	pub fn end_stream(&mut self) {
		self.core.end_stream();
	}

	pub fn finalize(self) -> SamplePair {
		SamplePair::from_core_and_lods(self.core.finalize(), self.lods.finalize(), self.path)
			.unwrap()
	}
}
