use crate::{arrangement_view::crc, lod::Lods};
use generic_daw_core::{self as core, SampleId};
use generic_daw_utils::NoDebug;
use std::{fs::File, num::NonZero, path::Path, sync::Arc};

#[derive(Debug)]
pub struct Sample {
	pub id: SampleId,
	pub lods: Lods<Box<[(f32, f32)]>>,
	pub name: Arc<str>,
	pub path: Arc<Path>,
	pub samples: NoDebug<Arc<[f32]>>,
	pub crc: u32,
	pub refs: usize,
}

#[derive(Debug)]
pub struct SamplePair {
	pub gui: Sample,
	pub core: core::Sample,
}

impl SamplePair {
	pub fn new(path: Arc<Path>, sample_rate: NonZero<u32>) -> Option<Self> {
		let name = path.file_name()?.to_str()?.into();
		let core = core::Sample::new(Box::from(File::open(&path).ok()?), sample_rate)?;
		let crc = crc(File::open(&path).ok()?);
		let gui = Sample {
			id: core.id,
			lods: Lods::new(&core.samples),
			path,
			name,
			samples: core.samples.clone(),
			crc,
			refs: 0,
		};
		Some(Self { gui, core })
	}
}
