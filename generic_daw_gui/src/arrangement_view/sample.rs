use crate::{arrangement_view::crc, lod::Lods};
use generic_daw_core::{SampleId, Transport, time::SecondsTime};
use std::{fs::File, num::NonZero, path::Path, sync::Arc};
use utils::NoDebug;

#[derive(Clone, Debug)]
pub struct Sample {
	pub id: SampleId,
	pub lods: Lods,
	pub name: Arc<str>,
	pub path: Arc<Path>,
	pub samples: NoDebug<Arc<[f32]>>,
	#[expect(clippy::struct_field_names)]
	pub sample_rate: NonZero<u32>,
	pub crc: u32,
	pub len: u64,
	pub refs: usize,
}

impl Sample {
	pub fn resample_ratio(&self, transport: &Transport) -> f64 {
		f64::from(transport.sample_rate.get()) / f64::from(self.sample_rate.get())
	}

	pub fn len(&self, transport: &Transport) -> SecondsTime {
		SecondsTime::from_samples(self.samples.len(), transport) * self.resample_ratio(transport)
	}
}

#[derive(Clone, Debug)]
pub struct SamplePair {
	pub core: generic_daw_core::Sample,
	pub gui: Sample,
}

impl SamplePair {
	pub fn new(path: Arc<Path>) -> Option<Self> {
		Self::with_crc_and_len(
			path.clone(),
			crc(File::open(&path).ok()?),
			std::fs::metadata(path).ok()?.len(),
		)
	}

	pub fn with_crc_and_len(path: Arc<Path>, crc: u32, len: u64) -> Option<Self> {
		let name = path.file_name()?.to_str()?.into();
		let core = generic_daw_core::Sample::new(Box::from(File::open(&path).ok()?))?;
		let gui = Sample {
			id: core.id,
			lods: Lods::new(&core.samples),
			path,
			name,
			samples: core.samples.clone(),
			sample_rate: core.sample_rate,
			crc,
			len,
			refs: 0,
		};
		Some(Self { core, gui })
	}

	pub fn from_core(core: generic_daw_core::Sample, path: Arc<Path>) -> Option<Self> {
		let name = path.file_name()?.to_str()?.into();
		let crc = crc(File::open(&path).ok()?);
		let len = std::fs::metadata(&path).ok()?.len();
		let gui = Sample {
			id: core.id,
			lods: Lods::new(&core.samples),
			path,
			name,
			samples: core.samples.clone(),
			sample_rate: core.sample_rate,
			crc,
			len,
			refs: 0,
		};
		Some(Self { core, gui })
	}
}
