use iced::{
	Color, Point, Rectangle, Transformation,
	advanced::graphics::{
		Mesh,
		color::{self, Packed},
		mesh::{Indexed, SolidVertex2D},
	},
};
use utils::NoDebug;

const STEP_SIZE: usize = 3;
const CHUNK_SIZE: usize = 1 << STEP_SIZE;

#[derive(Clone, Debug)]
pub struct Lods(NoDebug<Box<[Box<[(f32, f32)]>]>>);

impl Lods {
	pub fn new(samples: &[f32]) -> Self {
		let mut builder = LodsBuilder::default();
		builder.update(samples, 0);
		builder.finalize()
	}

	pub fn mesh(
		&self,
		samples: &[f32],
		offset: usize,
		samples_per_px: f32,
		color: Color,
		unclipped_bounds: Rectangle,
		clipped_bounds: Rectangle,
	) -> Option<Mesh> {
		mesh(
			&self.0,
			samples,
			offset,
			samples_per_px,
			color,
			unclipped_bounds,
			clipped_bounds,
		)
	}
}

#[derive(Debug, Default)]
pub struct LodsBuilder(NoDebug<Vec<Vec<(f32, f32)>>>);

impl LodsBuilder {
	pub fn update(&mut self, samples: &[f32], mut start: usize) {
		const FIRST: usize = 2 * CHUNK_SIZE;
		start /= FIRST;

		if self.0.is_empty() {
			self.0.push(Vec::new());
		}

		self.0[0].truncate(start);
		self.0[0].extend(samples[FIRST * start..].chunks(FIRST).map(samples_min_max));

		for i in 1.. {
			if self.0[i - 1].len() <= CHUNK_SIZE {
				return;
			}

			if self.0.len() == i {
				self.0.push(Vec::new());
			}

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

	pub fn mesh(
		&self,
		samples: &[f32],
		offset: usize,
		samples_per_px: f32,
		color: Color,
		unclipped_bounds: Rectangle,
		clipped_bounds: Rectangle,
	) -> Option<Mesh> {
		mesh(
			&self.0,
			samples,
			offset,
			samples_per_px,
			color,
			unclipped_bounds,
			clipped_bounds,
		)
	}

	pub fn finalize(self) -> Lods {
		Lods(NoDebug(
			self.0
				.0
				.into_iter()
				.map(Vec::into_boxed_slice)
				.collect::<Box<_>>(),
		))
	}
}

fn mesh(
	lods: &[impl AsRef<[(f32, f32)]>],
	samples: &[f32],
	offset: usize,
	samples_per_px: f32,
	color: Color,
	unclipped_bounds: Rectangle,
	clipped_bounds: Rectangle,
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

	let mesh_lod = (samples_per_px.abs().log2() - 1.0) as usize;
	let saved_lod = (mesh_lod / STEP_SIZE).checked_sub(1);
	let lod_slices_per_mesh_slice = 1 << (mesh_lod % STEP_SIZE);

	if saved_lod.is_some_and(|saved_lod| lods.len() <= saved_lod) {
		return None;
	}

	let samples_per_mesh_slice = (2 << mesh_lod) as f32;
	let px_per_mesh_slice = samples_per_mesh_slice / samples_per_px.abs();

	let lod_slices_per_sample = lod_slices_per_mesh_slice as f32 / samples_per_mesh_slice;
	let lod_slices_per_px = lod_slices_per_mesh_slice as f32 / px_per_mesh_slice;

	let lod_len_f = (clipped_bounds.width * lod_slices_per_px)
		.min((samples.len() as f32 - offset as f32) * lod_slices_per_sample);

	let lod_start_f = if samples_per_px.is_sign_positive() {
		offset as f32 * lod_slices_per_sample
			+ (clipped_bounds.x - unclipped_bounds.x) * lod_slices_per_px
	} else {
		(samples.len() as f32 - offset as f32) * lod_slices_per_sample
			- (clipped_bounds.x - unclipped_bounds.x) * lod_slices_per_px
			- lod_len_f
	};

	let lod_end_f = lod_start_f + lod_len_f;

	let lod_start = lod_start_f as usize;
	let lod_end = lod_end_f.ceil() as usize;

	let lod_start = lod_start - lod_start % lod_slices_per_mesh_slice;
	let lod_end = lod_end.next_multiple_of(lod_slices_per_mesh_slice) + 1;

	let lod_len = saved_lod.map_or(samples.len() / 2, |saved_lod| {
		lods[saved_lod].as_ref().len()
	});
	let lod_end = lod_end.min(lod_len);

	if lod_end <= lod_start {
		return None;
	}

	let color = color::pack(color);
	let vertices = saved_lod.map_or_else(
		|| {
			let base = samples[2 * lod_start..2 * lod_end]
				.chunks(2 * lod_slices_per_mesh_slice)
				.map(samples_min_max);

			if samples_per_px.is_sign_positive() {
				vertices(
					base,
					unclipped_bounds.height,
					color,
					px_per_mesh_slice,
					(lod_start as f32 - lod_start_f) / lod_slices_per_px,
					unclipped_bounds.y - clipped_bounds.y,
				)
			} else {
				vertices(
					base.rev(),
					unclipped_bounds.height,
					color,
					px_per_mesh_slice,
					(lod_end_f - lod_end as f32) / lod_slices_per_px,
					unclipped_bounds.y - clipped_bounds.y,
				)
			}
		},
		|saved_lod| {
			let base = lods[saved_lod].as_ref()[lod_start..lod_end]
				.chunks(lod_slices_per_mesh_slice)
				.map(lod_min_max);

			if samples_per_px.is_sign_positive() {
				vertices(
					base,
					unclipped_bounds.height,
					color,
					px_per_mesh_slice,
					(lod_start as f32 - lod_start_f) / lod_slices_per_px,
					unclipped_bounds.y - clipped_bounds.y,
				)
			} else {
				vertices(
					base.rev(),
					unclipped_bounds.height,
					color,
					px_per_mesh_slice,
					(lod_end_f - lod_end as f32) / lod_slices_per_px,
					unclipped_bounds.y - clipped_bounds.y,
				)
			}
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
		clip_bounds: Rectangle::new(Point::ORIGIN, clipped_bounds.size()),
	})
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
