use generic_daw_core::{OffsetPosition, Transport};
use iced::{
	Color, Point, Rectangle, Size, Transformation,
	advanced::graphics::{
		Mesh,
		color::{self, Packed},
		mesh::{Indexed, SolidVertex2D},
	},
};
use utils::NoDebug;

const STEP_SIZE: usize = 3;
const CHUNK_SIZE: usize = 1 << STEP_SIZE;
const BASE_SAMPLES: usize = 2 * CHUNK_SIZE;
const LOD_LEVELS: usize = 5;

#[derive(Debug, Default)]
pub struct Lods<T: AsRef<[(f32, f32)]>>(NoDebug<[T; LOD_LEVELS]>);

#[derive(Debug)]
pub struct LodsBuilder {
	lods: [Vec<(f32, f32)>; LOD_LEVELS],
	pending_samples: Vec<f32>,
	pending_lods: [Vec<(f32, f32)>; LOD_LEVELS - 1],
}

impl<T: AsRef<[(f32, f32)]>> Lods<T> {
	pub fn mesh(
		&self,
		transport: &Transport,
		position: OffsetPosition,
		x_scale: f32,
		height: f32,
		color: Color,
		clipped_size: Size,
		hidden_start_px: f32,
		hidden_top_px: f32,
	) -> Option<Mesh> {
		fn vertices(
			iter: impl IntoIterator<Item = (f32, f32)>,
			height: f32,
			color: Packed,
			px_per_mesh_slice: f32,
			jitter_correct: f32,
			hidden_top_px: f32,
		) -> Vec<SolidVertex2D> {
			iter.into_iter()
				.map(|(min, max)| (min * height + hidden_top_px, max * height + hidden_top_px))
				.scan(None, |acc, (mut min, mut max)| {
					if let Some((l_max, l_min)) = *acc {
						min = min.min(l_max);
						max = max.max(l_min);
					}
					*acc = Some((max, min));
					Some((min, max))
				})
				.map(|(min, max)| {
					if max - min < 1.0 {
						let avg = min.midpoint(max);
						(avg - 0.5, avg + 0.5)
					} else {
						(min, max)
					}
				})
				.enumerate()
				.map(|(x, mm)| (x as f32 * px_per_mesh_slice + jitter_correct, mm))
				.flat_map(|(x, (min, max))| {
					[
						SolidVertex2D {
							position: [x, min],
							color,
						},
						SolidVertex2D {
							position: [x, max],
							color,
						},
					]
				})
				.collect()
		}

		let samples_per_px = x_scale.exp2();
		let saved_lod = (x_scale.floor().exp2() as usize / BASE_SAMPLES)
			.max(1)
			.ilog2() as usize
			/ STEP_SIZE;
		let saved_lod = saved_lod.min(LOD_LEVELS - 1);

		let samples_per_lod_slice = BASE_SAMPLES << (saved_lod * STEP_SIZE);
		let samples_per_mesh_slice = samples_per_px.floor().max(samples_per_lod_slice as f32);
		let samples_per_mesh_slice = samples_per_mesh_slice as usize;
		let lod_slices_per_mesh_slice =
			(samples_per_mesh_slice / samples_per_lod_slice).next_power_of_two();
		let samples_per_mesh_slice = samples_per_lod_slice * lod_slices_per_mesh_slice;
		let px_per_mesh_slice = samples_per_mesh_slice as f32 / samples_per_px;

		let (start, end, offset) = position.to_samples(transport);
		let hidden_start_samples = 0.0f32.max(hidden_start_px * -samples_per_px).round() as usize;
		let visible_start = offset + hidden_start_samples;
		let visible_len = (end - start).saturating_sub(hidden_start_samples).min(
			(clipped_size.width * samples_per_px).ceil() as usize + samples_per_mesh_slice,
		);
		let visible_end = visible_start + visible_len;

		let first_lod = (visible_start / samples_per_lod_slice / lod_slices_per_mesh_slice)
			* lod_slices_per_mesh_slice;
		let last_lod = visible_end.div_ceil(samples_per_lod_slice);
		let last_lod = last_lod.div_ceil(lod_slices_per_mesh_slice) * lod_slices_per_mesh_slice;
		let last_lod = last_lod.min(self.0[saved_lod].as_ref().len());

		if last_lod <= first_lod {
			return None;
		}

		let color = color::pack(color);
		let jitter_correct = -((offset % samples_per_mesh_slice) as f32 / samples_per_px);
		let vertices = vertices(
			self.0[saved_lod].as_ref()[first_lod..last_lod]
				.chunks(lod_slices_per_mesh_slice)
				.map(lod_min_max),
			height,
			color,
			px_per_mesh_slice,
			jitter_correct,
			hidden_top_px,
		);

		if vertices.len() < 3 {
			return None;
		}

		let indices = (0..vertices.len() as u32 - 2)
			.flat_map(|i| [i, i + 1, i + 2])
			.collect();

		Some(Mesh::Solid {
			buffers: Indexed { vertices, indices },
			transformation: Transformation::IDENTITY,
			clip_bounds: Rectangle::new(Point::ORIGIN, clipped_size),
		})
	}
}

impl Lods<Box<[(f32, f32)]>> {
	pub fn new(samples: &[f32]) -> Self {
		let mut lods = Lods(NoDebug(std::array::from_fn(|i| {
			Vec::with_capacity(samples.len().div_ceil(BASE_SAMPLES << (i * STEP_SIZE)))
		})));
		lods.update(samples, 0);
		lods.finalize()
	}
}

impl Lods<Vec<(f32, f32)>> {
	pub fn update(&mut self, samples: &[f32], mut start: usize) {
		start /= BASE_SAMPLES;
		self.0[0].truncate(start);
		self.0[0].extend(samples[BASE_SAMPLES * start..].chunks(BASE_SAMPLES).map(samples_min_max));

		for i in 1..LOD_LEVELS {
			let [last, current] = &mut self.0[i - 1..=i] else {
				unreachable!();
			};

			start /= CHUNK_SIZE;
			current.truncate(start);
			current.extend(
				last[CHUNK_SIZE * start..]
					.chunks(CHUNK_SIZE)
					.map(lod_min_max),
			);
		}
	}

	pub fn finalize(self) -> Lods<Box<[(f32, f32)]>> {
		Lods(self.0.0.map(Vec::into_boxed_slice).into())
	}
}

impl Default for LodsBuilder {
	fn default() -> Self {
		Self {
			lods: std::array::from_fn(|_| Vec::new()),
			pending_samples: Vec::new(),
			pending_lods: std::array::from_fn(|_| Vec::new()),
		}
	}
}

impl LodsBuilder {
	pub fn push_samples(&mut self, mut samples: &[f32]) {
		if !self.pending_samples.is_empty() {
			let take = (BASE_SAMPLES - self.pending_samples.len()).min(samples.len());
			self.pending_samples.extend_from_slice(&samples[..take]);
			samples = &samples[take..];

			if self.pending_samples.len() == BASE_SAMPLES {
				let pair = samples_min_max(&self.pending_samples);
				self.pending_samples.clear();
				self.push_lod(0, pair);
			}
		}

		while samples.len() >= BASE_SAMPLES {
			self.push_lod(0, samples_min_max(&samples[..BASE_SAMPLES]));
			samples = &samples[BASE_SAMPLES..];
		}

		self.pending_samples.extend_from_slice(samples);
	}

	pub fn finish(mut self) -> Lods<Box<[(f32, f32)]>> {
		if !self.pending_samples.is_empty() {
			let pair = samples_min_max(&self.pending_samples);
			self.pending_samples.clear();
			self.push_lod(0, pair);
		}

		for level in 0..self.pending_lods.len() {
			if !self.pending_lods[level].is_empty() {
				let pair = lod_min_max(&self.pending_lods[level]);
				self.pending_lods[level].clear();
				self.push_lod(level + 1, pair);
			}
		}

		Lods(self.lods.map(Vec::into_boxed_slice).into())
	}

	fn push_lod(&mut self, level: usize, pair: (f32, f32)) {
		self.lods[level].push(pair);

		let Some(pending) = self.pending_lods.get_mut(level) else {
			return;
		};

		pending.push(pair);
		if pending.len() == CHUNK_SIZE {
			let pair = lod_min_max(pending);
			pending.clear();
			self.push_lod(level + 1, pair);
		}
	}
}

fn samples_min_max(chunk: &[f32]) -> (f32, f32) {
	let (min, max) = chunk
		.iter()
		.fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
			(min.min(c), max.max(c))
		});
	((min + 1.0) * 0.5, (max + 1.0) * 0.5)
}

fn lod_min_max(chunk: &[(f32, f32)]) -> (f32, f32) {
	chunk
		.iter()
		.fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
			(min.min(c.0), max.max(c.1))
		})
}
