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
const TOTAL_LOD_LEVELS: usize = 5;
const SAVED_LOD_LEVELS: usize = TOTAL_LOD_LEVELS - 1;

#[derive(Debug, Default)]
pub struct Lods<T: AsRef<[(f32, f32)]>>(NoDebug<[T; SAVED_LOD_LEVELS]>);

impl<T: AsRef<[(f32, f32)]>> Lods<T> {
	pub fn mesh(
		&self,
		samples: &[f32],
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
				.map(|(min, max)| {
					(
						min.mul_add(height, hidden_top_px),
						max.mul_add(height, hidden_top_px),
					)
				})
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
				.map(|(x, mm)| ((x as f32).mul_add(px_per_mesh_slice, jitter_correct), mm))
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

		let mesh_lod = x_scale as usize - 1;
		let saved_lod = mesh_lod / STEP_SIZE;
		let lod_slices_per_mesh_slice = 1 << (mesh_lod % STEP_SIZE);

		let samples_per_mesh_slice = x_scale.floor().exp2();
		let samples_per_px = x_scale.exp2();

		let px_per_mesh_slice = samples_per_mesh_slice / samples_per_px;

		let lod_slices_per_sample = lod_slices_per_mesh_slice as f32 / samples_per_mesh_slice;
		let lod_slices_per_px = lod_slices_per_mesh_slice as f32 / px_per_mesh_slice;

		let start = position.start().to_samples_f(transport);
		let end = position.end().to_samples_f(transport);
		let offset = position.offset().to_samples_f(transport);

		let hidden_start_samples = 0f32.max(hidden_start_px * -samples_per_px);

		let lod_start_f = (offset + hidden_start_samples) * lod_slices_per_sample;
		let view_len_f = (end - start) * lod_slices_per_sample;
		let view_len_f = view_len_f.min(clipped_size.width * lod_slices_per_px);
		let lod_end_f = lod_start_f + view_len_f;

		let lod_start = lod_start_f / lod_slices_per_mesh_slice as f32;
		let lod_start = lod_start as usize * lod_slices_per_mesh_slice;
		let lod_end = lod_end_f / lod_slices_per_mesh_slice as f32;
		let lod_end = lod_end as usize * lod_slices_per_mesh_slice;

		let lod_len = saved_lod
			.checked_sub(1)
			.map_or(samples.len() / 2, |saved_lod| {
				self.0[saved_lod].as_ref().len()
			});
		let lod_end = lod_end.min(lod_len);

		if lod_end <= lod_start {
			return None;
		}

		let color = color::pack(color);
		let jitter_correct = -(offset / samples_per_mesh_slice).fract() * px_per_mesh_slice;
		let vertices = saved_lod.checked_sub(1).map_or_else(
			|| {
				vertices(
					samples[2 * lod_start..2 * lod_end]
						.chunks(2 * lod_slices_per_mesh_slice)
						.map(samples_min_max),
					height,
					color,
					px_per_mesh_slice,
					jitter_correct,
					hidden_top_px,
				)
			},
			|saved_lod| {
				vertices(
					self.0[saved_lod].as_ref()[lod_start..lod_end]
						.chunks(lod_slices_per_mesh_slice)
						.map(lod_min_max),
					height,
					color,
					px_per_mesh_slice,
					jitter_correct,
					hidden_top_px,
				)
			},
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
			Vec::with_capacity(samples.len().div_ceil(1 << ((i + 1) * STEP_SIZE + 1)))
		})));
		lods.update(samples, 0);
		lods.finalize()
	}
}

impl Lods<Vec<(f32, f32)>> {
	pub fn update(&mut self, samples: &[f32], mut start: usize) {
		const FIRST: usize = 2 * CHUNK_SIZE;
		start /= FIRST;
		self.0[0].truncate(start);
		self.0[0].extend(samples[FIRST * start..].chunks(FIRST).map(samples_min_max));

		for i in 1..SAVED_LOD_LEVELS {
			let [last, current] = &mut self.0[i - 1..=i] else {
				unreachable!()
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

fn samples_min_max(chunk: &[f32]) -> (f32, f32) {
	let (min, max) = chunk
		.iter()
		.fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
			(min.min(c), max.max(c))
		});
	(min.mul_add(0.5, 0.5), max.mul_add(0.5, 0.5))
}

fn lod_min_max(chunk: &[(f32, f32)]) -> (f32, f32) {
	chunk
		.iter()
		.fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
			(min.min(c.0), max.max(c.1))
		})
}
