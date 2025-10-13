use crate::{arrangement_view::crc, lod::Lods};
use generic_daw_core::{self as core, SampleId};
use generic_daw_utils::NoDebug;
use std::{fs::File, path::Path, sync::Arc};

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
	pub fn new(path: impl AsRef<Path>, sample_rate: u32) -> Option<Self> {
		let name = path.as_ref().file_name()?.to_str()?;
		let core = core::Sample::new(Box::from(File::open(path.as_ref()).ok()?), sample_rate)?;
		let gui = Sample {
			id: core.id,
			lods: Lods::<Box<[(f32, f32)]>>::new(&core.samples),
			path: path.as_ref().into(),
			name: name.into(),
			samples: core.samples.clone(),
			crc: crc(File::open(path).ok()?),
			refs: 0,
		};
		Some(Self { gui, core })
	}
}
