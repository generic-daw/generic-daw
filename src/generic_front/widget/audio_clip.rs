use crate::{generic_back::AudioClip, generic_front::ArrangementState};
use iced::{
    advanced::{
        graphics::{
            color,
            mesh::{self, Renderer as _, SolidVertex2D},
            Mesh,
        },
        renderer::Quad,
        text::Renderer as _,
        Renderer as _, Text,
    },
    alignment::{Horizontal, Vertical},
    widget::text::{LineHeight, Shaping, Wrapping},
    Font, Pixels, Point, Rectangle, Renderer, Size, Theme, Transformation, Vector,
};
use std::cmp::{max_by, min};

impl AudioClip {
    #[expect(clippy::too_many_lines)]
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        bounds: Rectangle,
        arrangement_bounds: Rectangle,
        state: &ArrangementState,
    ) {
        // samples of the original audio per sample of lod
        let lod_sample_size = state.scale.x.floor().exp2() as u32;

        // samples of the original audio per pixel
        let pixel_size = state.scale.x.exp2();

        // samples in the lod per pixel
        let lod_samples_per_pixel = lod_sample_size as f32 / pixel_size;

        let global_start = self
            .get_global_start()
            .in_interleaved_samples(&self.arrangement.meter) as f32;

        let clip_start = self
            .get_clip_start()
            .in_interleaved_samples(&self.arrangement.meter);

        // the first sample in the lod that is visible in the clip
        let first_index = (max_by(0.0, state.position.x - global_start, |a, b| {
            a.partial_cmp(b).unwrap()
        }) as u32
            - clip_start)
            / lod_sample_size;

        // the distance between the left side of the timeline and the left side of the clip, in samples in the lod
        let index_offset = (max_by(0.0, global_start - state.position.x, |a, b| {
            a.partial_cmp(b).unwrap()
        }) as u32
            - clip_start)
            / lod_sample_size;

        // the last sample in the lod that is visible in the clip
        let last_index = min(
            (self
                .get_global_end()
                .in_interleaved_samples(&self.arrangement.meter)
                - clip_start)
                / lod_sample_size,
            first_index + index_offset + (bounds.width / lod_samples_per_pixel) as u32,
        );

        // if there are less than 3 vertices, there's nothing to draw
        if (last_index - first_index) < 2 {
            return;
        }

        // how many pixels of the top of the clip are clipped off by the top of the arrangement
        let hidden = max_by(0.0, arrangement_bounds.y - bounds.y, |a, b| {
            a.partial_cmp(b).unwrap()
        });

        // height of the waveform: the height of the clip minus the height of the text
        let waveform_height = bounds.height - 18.0;

        // the part of the audio clip that is visible
        let clip_bounds = Rectangle::new(
            Point::new(0.0, hidden),
            bounds.intersection(&arrangement_bounds).unwrap().size(),
        );

        // the translucent background of the clip
        let clip_background = Quad {
            bounds: Rectangle::new(
                Point::new(0.0, hidden),
                Size::new(
                    clip_bounds.width,
                    max_by(0.0, clip_bounds.height, |a, b| a.partial_cmp(b).unwrap()),
                ),
            ),
            ..Quad::default()
        };

        // the opaque background of the text
        let text_background = Quad {
            bounds: Rectangle::new(
                Point::new(0.0, hidden),
                Size::new(
                    clip_bounds.width,
                    max_by(0.0, 18.0 - hidden, |a, b| a.partial_cmp(b).unwrap()),
                ),
            ),
            ..Quad::default()
        };

        // vertices of the waveform
        let mut vertices =
            Vec::with_capacity(2 * usize::try_from(last_index - first_index).unwrap());
        let color = color::pack(theme.extended_palette().secondary.base.text);
        let lod = state.scale.x as usize - 3;
        (first_index..last_index).enumerate().for_each(|(x, i)| {
            let (min, max) = *self.audio.lods.read().unwrap()[lod]
                .get(usize::try_from(i).unwrap())
                .unwrap_or(&(0.0, 0.0));

            vertices.push(SolidVertex2D {
                position: [
                    x as f32 * lod_samples_per_pixel,
                    min.mul_add(waveform_height, 18.0),
                ],
                color,
            });
            vertices.push(SolidVertex2D {
                position: [
                    x as f32 * lod_samples_per_pixel,
                    max.mul_add(waveform_height, 18.0),
                ],
                color,
            });
        });

        // triangles of the waveform
        let mut indices = Vec::with_capacity(3 * (vertices.len() - 2));
        (0..vertices.len() - 2).for_each(|i| {
            let i = u32::try_from(i).unwrap();
            indices.push(i);
            indices.push(i + 1);
            indices.push(i + 2);
        });

        // the waveform mesh with the clip bounds
        let waveform_mesh = Mesh::Solid {
            buffers: mesh::Indexed { vertices, indices },
            transformation: Transformation::IDENTITY,
            clip_bounds,
        };

        // the text containing the name of the sample
        let text = Text {
            content: self.audio.name.clone(),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: Pixels(12.0),
            line_height: LineHeight::default(),
            font: Font::default(),
            horizontal_alignment: Horizontal::Left,
            vertical_alignment: Vertical::Top,
            shaping: Shaping::default(),
            wrapping: Wrapping::default(),
        };

        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
            renderer.fill_quad(
                clip_background,
                theme
                    .extended_palette()
                    .primary
                    .weak
                    .color
                    .scale_alpha(0.25),
            );

            renderer.fill_quad(text_background, theme.extended_palette().primary.weak.color);

            renderer.fill_text(
                text,
                Point::new(2.0, 2.0),
                theme.extended_palette().secondary.base.text,
                clip_bounds,
            );

            renderer.draw_mesh(waveform_mesh);
        });
    }
}
