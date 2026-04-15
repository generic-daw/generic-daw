use crate::{arrangement_view::crc, lod::Lods};
use generic_daw_core::{SampleId, Transport};
use std::{fs::File, path::Path, sync::Arc};
use utils::NoDebug;

#[derive(Debug)]
pub struct Sample {
	pub id: SampleId,
	pub lods: Lods,
	pub name: Arc<str>,
	pub path: Arc<Path>,
	pub samples: NoDebug<Arc<[f32]>>,
	pub crc: u32,
	pub len: u64,
	pub refs: usize,
}

#[derive(Debug)]
pub struct SamplePair {
	pub core: generic_daw_core::Sample,
	pub gui: Sample,
}

impl SamplePair {
	pub fn new(path: Arc<Path>, transport: &Transport) -> Option<Self> {
		Self::with_crc_and_len(
			path.clone(),
			transport,
			crc(File::open(&path).ok()?),
			std::fs::metadata(path).ok()?.len(),
		)
	}

	pub fn with_crc_and_len(
		path: Arc<Path>,
		transport: &Transport,
		crc: u32,
		len: u64,
	) -> Option<Self> {
		let name = path.file_name()?.to_str()?.into();
		let core = generic_daw_core::Sample::new(Box::from(File::open(&path).ok()?), transport)?;
		let gui = Sample {
			id: core.id,
			lods: Lods::new(&core.samples),
			path,
			name,
			samples: core.samples.clone(),
			crc,
			len,
			refs: 0,
		};
		Some(Self { core, gui })
	}
}
