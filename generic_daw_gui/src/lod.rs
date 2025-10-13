use crate::widget::LINE_HEIGHT;
use generic_daw_core::{MusicalTime, RtState};
use generic_daw_utils::{NoDebug, Vec2};
use iced::{
	Point, Rectangle, Theme, Transformation,
	advanced::graphics::{
		Mesh,
		color::{self, Packed},
		mesh::{Indexed, SolidVertex2D},
	},
};
use std::sync::Arc;

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
		rtstate: &RtState,
		start: MusicalTime,
		offset: MusicalTime,
		position: Vec2,
		scale: Vec2,
		theme: &Theme,
		pos_y: f32,
		bounds: Rectangle,
	) -> Option<Mesh> {
		fn vertices(
			iter: impl IntoIterator<Item = (f32, f32)>,
			height: f32,
			subpixel: f32,
			lod_samples_per_pixel: f32,
			color: Packed,
		) -> Arc<[SolidVertex2D]> {
			let mut last = None::<(f32, f32)>;
			iter.into_iter()
				.map(|(mut min, mut max)| {
					if let Some((l_max, l_min)) = last {
						min = min.min(l_max);
						max = max.max(l_min);
					}
					last = Some((max, min));
					(min * height, max * height)
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
				.flat_map(|(x, (min, max))| {
					let x = (x as f32 - subpixel) * lod_samples_per_pixel;

					[
						SolidVertex2D {
							position: [x, min + LINE_HEIGHT],
							color,
						},
						SolidVertex2D {
							position: [x, max + LINE_HEIGHT],
							color,
						},
					]
				})
				.collect()
		}

		let height = scale.y - LINE_HEIGHT;

		debug_assert!(height > 0.0);

		let lod_sample_size = scale.x.floor().exp2();

		let pixel_size = scale.x.exp2();

		let lod_samples_per_pixel = lod_sample_size / pixel_size;

		let color = color::pack(theme.extended_palette().background.strong.text);

		let start = start.to_samples_f(rtstate);
		let offset = offset.to_samples_f(rtstate);
		let subpixel = (offset / lod_sample_size).fract();

		let diff = 0f32.max(position.x - start);

		let lod = scale.x as usize - 1;
		let chunk = 1 << (lod % STEP_SIZE);
		let lod = lod / STEP_SIZE;
		let len = lod
			.checked_sub(1)
			.map_or(samples.len() / 2, |lod| self.0[lod].as_ref().len())
			/ chunk;

		let first_index = ((diff + offset) / lod_sample_size) as usize;
		let last_index = first_index + (bounds.width / lod_samples_per_pixel) as usize;
		let last_index = last_index.min(len);

		if last_index <= first_index || last_index - first_index <= 2 {
			return None;
		}

		let first_index = chunk * first_index;
		let last_index = chunk * last_index;

		let vertices = lod.checked_sub(1).map_or_else(
			|| {
				vertices(
					samples[2 * first_index..2 * last_index]
						.chunks(2 * chunk)
						.map(first_min_max),
					height,
					subpixel,
					lod_samples_per_pixel,
					color,
				)
			},
			|lod| {
				vertices(
					self.0[lod].as_ref()[first_index..last_index]
						.chunks(chunk)
						.map(other_min_max),
					height,
					subpixel,
					lod_samples_per_pixel,
					color,
				)
			},
		);

		let indices = (0..vertices.len() as u32 - 2)
			.flat_map(|i| [i, i + 1, i + 2])
			.collect();

		Some(Mesh::Solid {
			buffers: Indexed { vertices, indices },
			transformation: Transformation::IDENTITY,
			clip_bounds: Rectangle::new(
				Point::new(0.0, (bounds.y - pos_y).max(0.0)),
				bounds.size(),
			),
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
		self.0[0].extend(samples[FIRST * start..].chunks(FIRST).map(first_min_max));

		for i in 1..SAVED_LOD_LEVELS {
			let [last, current] = &mut self.0[i - 1..=i] else {
				unreachable!()
			};

			start /= CHUNK_SIZE;
			current.truncate(start);
			current.extend(
				last[CHUNK_SIZE * start..]
					.chunks(CHUNK_SIZE)
					.map(other_min_max),
			);
		}
	}

	pub fn finalize(self) -> Lods<Box<[(f32, f32)]>> {
		Lods(self.0.0.map(Vec::into_boxed_slice).into())
	}
}

fn first_min_max(chunk: &[f32]) -> (f32, f32) {
	let (min, max) = chunk
		.iter()
		.fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
			(min.min(c), max.max(c))
		});
	(min.mul_add(0.5, 0.5), max.mul_add(0.5, 0.5))
}

fn other_min_max(chunk: &[(f32, f32)]) -> (f32, f32) {
	chunk
		.iter()
		.fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &c| {
			(min.min(c.0), max.max(c.1))
		})
}
