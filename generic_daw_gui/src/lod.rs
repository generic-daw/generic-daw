use generic_daw_core::{Transition, Transport, time::SecondsTime};
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
	pub fn new(samples: &[[f32; 2]]) -> Self {
		let mut builder = LodsBuilder::default();
		builder.update(samples, 0);
		builder.finalize()
	}

	pub fn max_abs(&self) -> f32 {
		self.0.last().map_or(0.0, |lod| {
			lod.iter()
				.fold(0.0, |acc, &(min, max)| min.abs().max(max.abs()).max(acc))
		})
	}

	pub fn mesh(
		&self,
		samples: &[[f32; 2]],
		offset: SecondsTime,
		transport: &Transport,
		volume: f32,
		fade_start: Transition,
		fade_end: Transition,
		frames_per_px: f32,
		color: Color,
		unclipped_bounds: Rectangle,
		clipped_bounds: Rectangle,
	) -> Option<Mesh> {
		mesh(
			&self.0,
			samples,
			offset,
			transport,
			volume,
			fade_start,
			fade_end,
			frames_per_px,
			color,
			unclipped_bounds,
			clipped_bounds,
		)
	}
}

#[derive(Debug, Default)]
pub struct LodsBuilder(NoDebug<Vec<Vec<(f32, f32)>>>);

impl LodsBuilder {
	pub fn update(&mut self, samples: &[[f32; 2]], mut start: usize) {
		start /= CHUNK_SIZE;

		if self.0.is_empty() {
			self.0.push(Vec::new());
		}

		self.0[0].truncate(start);
		self.0[0].extend(
			samples[CHUNK_SIZE * start..]
				.chunks(CHUNK_SIZE)
				.map(samples_min_max),
		);

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
		samples: &[[f32; 2]],
		transport: &Transport,
		frames_per_px: f32,
		color: Color,
		unclipped_bounds: Rectangle,
		clipped_bounds: Rectangle,
	) -> Option<Mesh> {
		mesh(
			&self.0,
			samples,
			SecondsTime::ZERO,
			transport,
			1.0,
			Transition::default(),
			Transition::default(),
			frames_per_px,
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
	samples: &[[f32; 2]],
	offset: SecondsTime,
	transport: &Transport,
	volume: f32,
	fade_start_t: Transition,
	fade_end_t: Transition,
	frames_per_px: f32,
	color: Color,
	unclipped_bounds: Rectangle,
	clipped_bounds: Rectangle,
) -> Option<Mesh> {
	let offset = offset.to_frames(transport);

	let mesh_lod = (frames_per_px.abs().log2() - 1.0) as usize;
	let saved_lod = (mesh_lod / STEP_SIZE).checked_sub(1);
	let lod_slices_per_mesh_slice = 1 << (mesh_lod % STEP_SIZE);

	if saved_lod.is_some_and(|saved_lod| lods.len() <= saved_lod) {
		return None;
	}

	let frames_per_mesh_slice = (1 << mesh_lod) as f32;
	let px_per_mesh_slice = frames_per_mesh_slice / frames_per_px.abs();

	let lod_slices_per_frame = lod_slices_per_mesh_slice as f32 / frames_per_mesh_slice;
	let lod_slices_per_px = lod_slices_per_mesh_slice as f32 / px_per_mesh_slice;

	let lod_len_f = (clipped_bounds.width * lod_slices_per_px)
		.min((samples.len() as f32 - offset as f32) * lod_slices_per_frame);

	let hidden_start_px = clipped_bounds.x - unclipped_bounds.x;
	let hidden_top_px = unclipped_bounds.y - clipped_bounds.y;

	let lod_start_f = if frames_per_px.is_sign_positive() {
		offset as f32 * lod_slices_per_frame + hidden_start_px * lod_slices_per_px
	} else {
		(samples.len() as f32 - offset as f32) * lod_slices_per_frame
			- hidden_start_px * lod_slices_per_px
			- lod_len_f
	};

	let lod_end_f = lod_start_f + lod_len_f;

	let lod_start = lod_start_f as usize;
	let lod_end = lod_end_f.ceil() as usize;

	let lod_start = lod_start - lod_start % lod_slices_per_mesh_slice;
	let lod_end = lod_end.next_multiple_of(lod_slices_per_mesh_slice) + 1;

	let lod_len = saved_lod.map_or(samples.len(), |saved_lod| lods[saved_lod].as_ref().len());
	let lod_end = lod_end.min(lod_len);

	if lod_end <= lod_start {
		return None;
	}

	let fade_start_px = fade_start_t.len.to_frames(transport) as f32 / frames_per_px.abs();
	let fade_end_px = fade_end_t.len.to_frames(transport) as f32 / frames_per_px.abs();

	let jitter_correct_px = if frames_per_px.is_sign_positive() {
		(lod_start as f32 - lod_start_f) / lod_slices_per_px
	} else {
		(lod_end_f - lod_end as f32) / lod_slices_per_px
	};

	let color = color::pack(color);

	let base = saved_lod.map_or_else(
		|| {
			left(
				samples[lod_start..lod_end]
					.chunks(lod_slices_per_mesh_slice)
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

	let base = if frames_per_px.is_sign_positive() {
		left(base)
	} else {
		right(base.rev())
	};

	let vertices = base
		.map(|(min, max)| {
			if volume.is_sign_positive() {
				(min * volume, max * volume)
			} else {
				(max * volume, min * volume)
			}
		})
		.enumerate()
		.map(|(x, (min, max))| {
			let x = x as f32 * px_per_mesh_slice + jitter_correct_px + hidden_start_px;
			let mix = if fade_start_px > 0.0 && x < fade_start_px {
				fade_start_t.transition(x.max(0.0) / fade_start_px)
			} else if fade_end_px > 0.0 && unclipped_bounds.width - x < fade_end_px {
				fade_end_t.transition((unclipped_bounds.width - x).max(0.0) / fade_end_px)
			} else {
				1.0
			};
			let min = (min * mix / 2.0 + 0.5) * unclipped_bounds.height + hidden_top_px;
			let max = (max * mix / 2.0 + 0.5) * unclipped_bounds.height + hidden_top_px;
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

fn samples_min_max(chunk: &[[f32; 2]]) -> (f32, f32) {
	chunk
		.as_flattened()
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
