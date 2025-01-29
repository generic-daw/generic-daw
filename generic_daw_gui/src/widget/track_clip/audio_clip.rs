use super::{ArrangementScale, TrackClipExt, LINE_HEIGHT};
use crate::widget::ArrangementPosition;
use generic_daw_core::AudioClip;
use iced::{
    advanced::graphics::{
        color,
        mesh::{self, SolidVertex2D},
        Mesh,
    },
    Rectangle, Size, Theme, Transformation,
};
use std::cmp::max_by;

impl TrackClipExt for AudioClip {
    fn mesh(
        &self,
        theme: &Theme,
        mut size: Size,
        position: ArrangementPosition,
        scale: ArrangementScale,
    ) -> Mesh {
        size.height -= LINE_HEIGHT;
        if size.height < 0.0 {
            return Mesh::Solid {
                buffers: mesh::Indexed {
                    vertices: Vec::new(),
                    indices: Vec::new(),
                },
                transformation: Transformation::IDENTITY,
                clip_bounds: Rectangle::INFINITE,
            };
        }

        // samples of the original audio per sample of lod
        let lod_sample_size = scale.x.floor().exp2();

        // samples of the original audio per pixel
        let pixel_size = scale.x.exp2();

        // samples in the lod per pixel
        let lod_samples_per_pixel = lod_sample_size / pixel_size;

        let color = color::pack(theme.extended_palette().secondary.base.text);
        let lod = scale.x as usize - 3;

        let diff = max_by(
            0.0,
            position.x
                - self
                    .get_global_start()
                    .in_interleaved_samples_f(&self.meter),
            f32::total_cmp,
        );

        let clip_start = self.get_clip_start().in_interleaved_samples_f(&self.meter);

        let first_index = ((diff + clip_start) / lod_sample_size) as usize;
        let last_index = first_index + (size.width / lod_samples_per_pixel) as usize;

        // vertices of the waveform
        let vertices = self.audio.lods[lod][first_index..last_index]
            .iter()
            .enumerate()
            .flat_map(|(x, (min, max))| {
                let x = x as f32 * lod_samples_per_pixel;

                [
                    SolidVertex2D {
                        position: [x, min.mul_add(size.height, LINE_HEIGHT)],
                        color,
                    },
                    SolidVertex2D {
                        position: [x, max.mul_add(size.height, LINE_HEIGHT)],
                        color,
                    },
                ]
            })
            .collect::<Vec<_>>();

        // triangles of the waveform
        let indices = (0..vertices.len() as u32 - 2)
            .flat_map(|i| [i, i + 1, i + 2])
            .collect();

        // the waveform mesh
        Mesh::Solid {
            buffers: mesh::Indexed { vertices, indices },
            transformation: Transformation::IDENTITY,
            clip_bounds: Rectangle::INFINITE,
        }
    }
}
