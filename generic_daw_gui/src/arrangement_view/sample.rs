use crate::{
	arrangement_view::crc,
	lod::{LOD_LEVELS, create_lods},
};
use generic_daw_core::{self as core, SampleId};
use generic_daw_utils::NoDebug;
use std::{fs::File, path::Path, sync::Arc};

#[derive(Debug)]
pub struct Sample {
	pub id: SampleId,
	pub lods: NoDebug<[Box<[(f32, f32)]>; LOD_LEVELS]>,
	pub name: Arc<str>,
	pub path: Arc<Path>,
	pub len: usize,
	pub crc: u32,
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
			lods: create_lods(&core.samples).into(),
			path: path.as_ref().into(),
			name: name.into(),
			len: core.samples.len(),
			crc: crc(File::open(path).ok()?),
		};
		Some(Self { gui, core })
	}
}
