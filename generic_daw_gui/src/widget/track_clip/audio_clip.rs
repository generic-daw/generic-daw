use super::{ArrangementPosition, ArrangementScale, TrackClipExt, LINE_HEIGHT};
use generic_daw_core::AudioClip;
use iced::{
    advanced::graphics::{
        color,
        mesh::{self, SolidVertex2D},
        Mesh,
    },
    Point, Rectangle, Size, Theme, Transformation,
};
use std::cmp::{max_by, min};

impl TrackClipExt for AudioClip {
    fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        viewport: Rectangle,
        position: ArrangementPosition,
        scale: ArrangementScale,
    ) -> Option<Mesh> {
        // samples of the original audio per sample of lod
        let lod_sample_size = scale.x.floor().exp2() as usize;

        // samples of the original audio per pixel
        let pixel_size = scale.x.exp2();

        // samples in the lod per pixel
        let lod_samples_per_pixel = lod_sample_size as f32 / pixel_size;

        let global_start = self
            .get_global_start()
            .in_interleaved_samples_f(&self.meter);

        let clip_start = self.get_clip_start().in_interleaved_samples_f(&self.meter);

        // the first sample in the lod that is visible in the clip
        let first_index = ((max_by(0.0, position.x - global_start, f32::total_cmp) + clip_start)
            as usize)
            / lod_sample_size;

        // the last sample in the lod that is visible in the clip
        let last_index = min(
            self.audio.len() / lod_sample_size,
            first_index + (bounds.width / lod_samples_per_pixel) as usize,
        );

        // if there are less than 3 vertices, there's nothing to draw
        if (last_index.saturating_sub(first_index)) < 3 {
            return None;
        }

        // how many pixels of the top of the waveform are clipped off by the top of the arrangement
        let hidden = viewport.y - bounds.y;

        // height of the waveform
        let waveform_height = bounds.height - LINE_HEIGHT;

        // the part of the audio clip that is visible
        let clip_bounds = Rectangle::new(
            Point::new(0.0, hidden + LINE_HEIGHT),
            Size::new(bounds.width, waveform_height - hidden),
        );

        let color = color::pack(theme.extended_palette().secondary.base.text);
        let lod = scale.x as usize - 3;

        // vertices of the waveform
        let vertices = self.audio.lods[lod][first_index..last_index]
            .iter()
            .enumerate()
            .flat_map(|(x, (min, max))| {
                let x = x as f32 * lod_samples_per_pixel;

                [
                    SolidVertex2D {
                        position: [x, min.mul_add(waveform_height, LINE_HEIGHT)],
                        color,
                    },
                    SolidVertex2D {
                        position: [x, max.mul_add(waveform_height, LINE_HEIGHT)],
                        color,
                    },
                ]
            })
            .collect::<Vec<_>>();

        // triangles of the waveform
        let indices = (0..vertices.len() as u32 - 2)
            .flat_map(|i| [i, i + 1, i + 2])
            .collect();

        // the waveform mesh with the clip bounds
        Some(Mesh::Solid {
            buffers: mesh::Indexed { vertices, indices },
            transformation: Transformation::translate(bounds.x, bounds.y),
            clip_bounds,
        })
    }
}
