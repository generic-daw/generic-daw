use crate::{arrangement_view::crc, lod::Lods};
use generic_daw_core::{SampleId, Transport, time::SecondsTime};
use std::{fs::File, num::NonZero, path::Path, sync::Arc};
use utils::NoDebug;

#[derive(Clone, Debug)]
pub struct Sample {
	pub id: SampleId,
	pub fade_start: SecondsTime,
	pub fade_end: SecondsTime,
	pub lods: Lods,
	pub name: Arc<str>,
	pub path: Arc<Path>,
	pub samples: NoDebug<Arc<[[f32; 2]]>>,
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
		SecondsTime::from_frames(self.samples.len(), transport) * self.resample_ratio(transport)
	}
}

#[derive(Clone, Debug)]
pub struct SamplePair {
	pub core: generic_daw_core::Sample,
	pub gui: Sample,
}

impl SamplePair {
	pub fn new(path: Arc<Path>) -> Option<Self> {
		let core = generic_daw_core::Sample::new(Box::from(File::open(&path).ok()?))?;
		Self::from_core(core, path)
	}

	pub fn with_crc_and_len(crc: u32, len: u64, path: Arc<Path>) -> Option<Self> {
		let core = generic_daw_core::Sample::new(Box::from(File::open(&path).ok()?))?;
		Self::from_core_with_crc_and_len(core, crc, len, path)
	}

	pub fn from_core(core: generic_daw_core::Sample, path: Arc<Path>) -> Option<Self> {
		let crc = crc(File::open(&path).ok()?);
		let len = std::fs::metadata(&path).ok()?.len();
		Self::from_core_with_crc_and_len(core, crc, len, path)
	}

	pub fn from_core_and_lods(
		core: generic_daw_core::Sample,
		lods: Lods,
		path: Arc<Path>,
	) -> Option<Self> {
		let crc = crc(File::open(&path).ok()?);
		let len = std::fs::metadata(&path).ok()?.len();
		Self::from_core_and_lods_with_crc_and_len(core, lods, crc, len, path)
	}

	pub fn from_core_with_crc_and_len(
		core: generic_daw_core::Sample,
		crc: u32,
		len: u64,
		path: Arc<Path>,
	) -> Option<Self> {
		let lods = Lods::new(&core.samples);
		Self::from_core_and_lods_with_crc_and_len(core, lods, crc, len, path)
	}

	pub fn from_core_and_lods_with_crc_and_len(
		core: generic_daw_core::Sample,
		lods: Lods,
		crc: u32,
		len: u64,
		path: Arc<Path>,
	) -> Option<Self> {
		let name = path.file_name()?.to_str()?.into();

		let ms_frames = core.sample_rate.get().div_ceil(1000) as usize;

		let fade_in_frames = find_first_zero_crossing(core.samples.iter().copied().take(ms_frames))
			.unwrap_or(ms_frames)
			.min(core.samples.len() / 2);

		let fade_out_frames =
			find_first_zero_crossing(core.samples.iter().copied().rev().take(ms_frames))
				.unwrap_or(ms_frames)
				.min(core.samples.len() / 2);

		let gui = Sample {
			id: core.id,
			fade_start: SecondsTime::from_float(fade_in_frames as f64 / (1000 * ms_frames) as f64),
			fade_end: SecondsTime::from_float(fade_out_frames as f64 / (1000 * ms_frames) as f64),
			lods,
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

fn find_first_zero_crossing(iter: impl IntoIterator<Item = [f32; 2]>) -> Option<usize> {
	let mut iter = iter.into_iter();
	let [first_l, first_r] = iter.next()?;
	iter.position(|[l, r]| {
		l.partial_cmp(&0.0) != first_l.partial_cmp(&0.0)
			|| r.partial_cmp(&0.0) != first_r.partial_cmp(&0.0)
	})
}
