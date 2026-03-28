use crate::{
	arrangement_view::crc,
	lod::{Lods, LodsBuilder},
};
use generic_daw_core::{SampleId, Transport};
use std::{fs::File, path::Path, sync::Arc};

#[derive(Debug)]
pub struct Sample {
	pub id: SampleId,
	pub lods: Lods<Box<[(f32, f32)]>>,
	pub name: Arc<str>,
	pub path: Arc<Path>,
	pub sample_len: usize,
	pub crc: u32,
	pub len: u64,
	pub refs: usize,
}

#[derive(Debug)]
pub struct SamplePair {
	pub gui: Sample,
	pub core: generic_daw_core::Sample,
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
		let mut lods = LodsBuilder::default();
		let core = generic_daw_core::Sample::new_with_callback(
			Box::from(File::open(&path).ok()?),
			transport,
			|samples, _| lods.push_samples(samples),
		)?;
		let gui = Sample {
			id: core.id,
			lods: lods.finish(),
			path,
			name,
			sample_len: core.len(),
			crc,
			len,
			refs: 0,
		};
		Some(Self { gui, core })
	}
}
