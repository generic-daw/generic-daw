use super::{TimelinePosition, TimelineScale};
use crate::generic_back::AudioClip;
use iced::{
    advanced::graphics::{
        color,
        mesh::{self, SolidVertex2D},
        Mesh,
    },
    Point, Rectangle, Theme, Transformation,
};
use std::cmp::{max_by, min};

impl AudioClip {
    pub fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        viewport: Rectangle,
        position: &TimelinePosition,
        scale: &TimelineScale,
    ) -> Option<Mesh> {
        // samples of the original audio per sample of lod
        let lod_sample_size = scale.x.get().floor().exp2() as u32;

        // samples of the original audio per pixel
        let pixel_size = scale.x.get().exp2();

        // samples in the lod per pixel
        let lod_samples_per_pixel = lod_sample_size as f32 / pixel_size;

        let global_start = self.get_global_start().in_interleaved_samples(&self.meter) as f32;

        let clip_start = self.get_clip_start().in_interleaved_samples(&self.meter) as f32;

        // the first sample in the lod that is visible in the clip
        let first_index = ((max_by(0.0, position.x.get() - global_start, |a, b| {
            a.partial_cmp(b).unwrap()
        }) + clip_start) as u32)
            / lod_sample_size;

        // the last sample in the lod that is visible in the clip
        let last_index = min(
            u32::try_from(self.audio.samples.len()).unwrap() / lod_sample_size,
            first_index + (bounds.width / lod_samples_per_pixel) as u32,
        );

        // if there are less than 3 vertices, there's nothing to draw
        if (last_index - first_index) < 2 {
            return None;
        }

        // how many pixels of the top of the clip are clipped off by the top of the arrangement
        let hidden = max_by(0.0, viewport.y - bounds.y + 18.0, |a, b| {
            a.partial_cmp(b).unwrap()
        });

        // height of the waveform: the height of the clip minus the height of the text
        let waveform_height = bounds.height - 18.0;

        // the part of the audio clip that is visible
        let clip_bounds = Rectangle::new(
            Point::new(0.0, hidden),
            bounds.intersection(&viewport).unwrap().size(),
        );

        let color = color::pack(theme.extended_palette().secondary.base.text);
        let lod = scale.x.get() as usize - 3;

        // vertices of the waveform
        let vertices: Vec<_> = (first_index..last_index)
            .enumerate()
            .map(|(x, i)| {
                (
                    x as f32 * lod_samples_per_pixel,
                    *self.audio.lods[lod]
                        .read()
                        .unwrap()
                        .get(usize::try_from(i).unwrap())
                        .unwrap_or(&(0.0, 0.0)),
                )
            })
            .flat_map(|(x, (min, max))| {
                [
                    SolidVertex2D {
                        position: [x, min.mul_add(waveform_height, 18.0)],
                        color,
                    },
                    SolidVertex2D {
                        position: [x, max.mul_add(waveform_height, 18.0)],
                        color,
                    },
                ]
            })
            .collect();

        // triangles of the waveform
        let indices = (0..vertices.len() - 2)
            .flat_map(|i| [i, i + 1, i + 2])
            .map(|i| u32::try_from(i).unwrap())
            .collect();

        // height of the clip, excluding the text, clipped off by the top of the arrangement
        let clip_height = max_by(0.0, 18.0 - hidden, |a, b| a.partial_cmp(b).unwrap());

        let mut waveform_clip_bounds = clip_bounds;
        waveform_clip_bounds.y += clip_height;
        waveform_clip_bounds.height -= clip_height;

        // the waveform mesh with the clip bounds
        Some(Mesh::Solid {
            buffers: mesh::Indexed { vertices, indices },
            transformation: Transformation::translate(bounds.x, bounds.y),
            clip_bounds: waveform_clip_bounds,
        })
    }
}
