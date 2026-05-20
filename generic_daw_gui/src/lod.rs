use iced::{
	Color, Point, Rectangle, Transformation,
	advanced::graphics::{
		Mesh, color,
		mesh::{Indexed, SolidVertex2D},
	},
};
use utils::{NoDebug, left, right};

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
			if self.0[i - 1].len() < CHUNK_SIZE {
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

	let hidden_start_px = clipped_bounds.x - unclipped_bounds.x;
	let hidden_top_px = unclipped_bounds.y - clipped_bounds.y;

	let lod_start_f = if samples_per_px.is_sign_positive() {
		offset as f32 * lod_slices_per_sample + hidden_start_px * lod_slices_per_px
	} else {
		(samples.len() as f32 - offset as f32) * lod_slices_per_sample
			- hidden_start_px * lod_slices_per_px
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

	let jitter_correct_px = (lod_start as f32 - lod_start_f) / lod_slices_per_px;

	let color = color::pack(color);

	let base = saved_lod.map_or_else(
		|| {
			left(
				samples[2 * lod_start..2 * lod_end]
					.chunks(2 * lod_slices_per_mesh_slice)
					.map(samples_min_max),
			)
		},
		|saved_lod| {
			right(
				lods[saved_lod].as_ref()[lod_start..lod_end]
					.chunks(lod_slices_per_mesh_slice)
					.map(lod_min_max),
			)
		},
	);

	let base = if samples_per_px.is_sign_positive() {
		left(base)
	} else {
		right(base.rev())
	};

	let vertices = base
		.map(|(min, max)| {
			let min = (min / 2.0 + 0.5) * unclipped_bounds.height + hidden_top_px;
			let max = (max / 2.0 + 0.5) * unclipped_bounds.height + hidden_top_px;
			(min, max)
		})
		.scan(None, |acc, (min, max)| {
			*acc = Some(acc.map_or((min, max), |(l_min, l_max)| {
				(min.min(l_max), max.max(l_min))
			}));
			*acc
		})
		.map(|(min, max)| {
			if max - min < 1.0 {
				let avg = (min + max) / 2.0;
				(avg - 0.5, avg + 0.5)
			} else {
				(min, max)
			}
		})
		.enumerate()
		.flat_map(|(x, (min, max))| {
			let x = x as f32 * px_per_mesh_slice + jitter_correct_px;
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
		.collect::<Vec<_>>();

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
	chunk
		.iter()
		.fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
			(min.min(c), max.max(c))
		})
}

fn lod_min_max(chunk: &[(f32, f32)]) -> (f32, f32) {
	chunk
		.iter()
		.fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
			(min.min(c.0), max.max(c.1))
		})
}
