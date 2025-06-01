use crate::widget::LINE_HEIGHT;
use generic_daw_core::{Meter, Position};
use generic_daw_utils::Vec2;
use iced::{Point, Rectangle, Theme, Transformation, advanced::graphics::color, debug};
use iced_wgpu::graphics::{
    Mesh,
    mesh::{Indexed, SolidVertex2D},
};

#[expect(clippy::trivially_copy_pass_by_ref)]
pub fn mesh(
    meter: &Meter,
    global_start: Position,
    clip_start: Position,
    lods: &[impl AsRef<[(f32, f32)]>],
    position: &Vec2,
    scale: &Vec2,
    theme: &Theme,
    point: Point,
    bounds: Rectangle,
) -> Option<Mesh> {
    // the height of the waveform
    let height = scale.y - LINE_HEIGHT;

    debug_assert!(height >= 0.0);

    // samples of the original audio per sample of lod
    let lod_sample_size = scale.x.floor().exp2();

    // samples of the original audio per pixel
    let pixel_size = scale.x.exp2();

    // samples in the lod per pixel
    let lod_samples_per_pixel = lod_sample_size / pixel_size;

    let color = color::pack(theme.extended_palette().background.strong.text);
    let lod = scale.x as usize - 3;

    let global_start = global_start.in_samples_f(meter);
    let clip_start = clip_start.in_samples_f(meter);
    let offset = (clip_start / lod_sample_size).fract();

    let diff = 0f32.max(position.x - global_start);

    let first_index = ((diff + clip_start) / lod_sample_size) as usize;
    let last_index = first_index + (bounds.width / lod_samples_per_pixel) as usize;
    let last_index = last_index.min(lods[lod].as_ref().len() - 1);

    // there is nothing to draw
    if last_index < first_index || last_index - first_index < 2 {
        return None;
    }

    let debug = debug::time("Waveform Generation");

    let mut last = None::<(f32, f32)>;
    // vertices of the waveform
    let vertices = lods[lod].as_ref()[first_index..=last_index]
        .iter()
        .map(|&(mut min, mut max)| {
            if let Some((l_max, l_min)) = last {
                min = min.min(l_max);
                max = max.max(l_min);
            }
            last = Some((max, min));
            (min, max)
        })
        .map(|(min, max)| (min * height, max * height))
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
            let x = (x as f32 - offset) * lod_samples_per_pixel;

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

    // triangles of the waveform
    let indices = (0..vertices.len() as u32 - 2)
        .flat_map(|i| [i, i + 1, i + 2])
        .collect();

    debug.finish();

    // the waveform mesh
    Some(Mesh::Solid {
        buffers: Indexed { vertices, indices },
        transformation: Transformation::translate(point.x, point.y),
        clip_bounds: Rectangle::new(
            Point::new(0.0, (bounds.y - point.y).max(0.0)),
            bounds.size(),
        ),
    })
}
