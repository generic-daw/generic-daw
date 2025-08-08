use crate::widget::LINE_HEIGHT;
use generic_daw_core::{LOD_LEVELS, MusicalTime, RtState};
use generic_daw_utils::Vec2;
use iced::{Point, Rectangle, Theme, Transformation, advanced::graphics::color, debug};
use iced_wgpu::graphics::{
	Mesh,
	mesh::{Indexed, SolidVertex2D},
};

#[expect(clippy::trivially_copy_pass_by_ref)]
pub fn mesh(
	rtstate: &RtState,
	start: MusicalTime,
	offset: MusicalTime,
	lods: &[impl AsRef<[(f32, f32)]>; LOD_LEVELS],
	position: &Vec2,
	scale: &Vec2,
	theme: &Theme,
	point: Point,
	bounds: Rectangle,
) -> Option<Mesh> {
	debug::time_with("Waveform Mesh", || {
		make_mesh(
			rtstate, start, offset, lods, position, scale, theme, point, bounds,
		)
	})
}

#[expect(clippy::trivially_copy_pass_by_ref)]
fn make_mesh(
	rtstate: &RtState,
	start: MusicalTime,
	offset: MusicalTime,
	lods: &[impl AsRef<[(f32, f32)]>],
	position: &Vec2,
	scale: &Vec2,
	theme: &Theme,
	point: Point,
	bounds: Rectangle,
) -> Option<Mesh> {
	let height = scale.y - LINE_HEIGHT;

	debug_assert!(height > 0.0);

	let lod_sample_size = scale.x.floor().exp2();

	let pixel_size = scale.x.exp2();

	let lod_samples_per_pixel = lod_sample_size / pixel_size;

	let color = color::pack(theme.extended_palette().background.strong.text);
	let lod = scale.x as usize - 3;

	let start = start.to_samples_f(rtstate);
	let offset = offset.to_samples_f(rtstate);
	let subpixel = (offset / lod_sample_size).fract();

	let diff = 0f32.max(position.x - start);

	let first_index = ((diff + offset) / lod_sample_size) as usize;
	let last_index = first_index + (bounds.width / lod_samples_per_pixel) as usize;
	let last_index = last_index.min(lods[lod].as_ref().len().saturating_sub(1));

	if last_index < first_index || last_index - first_index < 2 {
		return None;
	}

	let mut last = None::<(f32, f32)>;
	let vertices = lods[lod].as_ref()[first_index..=last_index]
		.iter()
		.map(|&(mut min, mut max)| {
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
		.collect::<Vec<_>>();

	let indices = (0..vertices.len() as u32 - 2)
		.flat_map(|i| [i, i + 1, i + 2])
		.collect();

	Some(Mesh::Solid {
		buffers: Indexed { vertices, indices },
		transformation: Transformation::translate(point.x, point.y),
		clip_bounds: Rectangle::new(
			Point::new(0.0, (bounds.y - point.y).max(0.0)),
			bounds.size(),
		),
	})
}
