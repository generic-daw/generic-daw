use crate::generic_back::track_clip::audio_clip::AudioClip;
use iced::{
    advanced::{
        graphics::{
            color,
            geometry::Renderer as _,
            mesh::Renderer as _,
            mesh::{self, SolidVertex2D},
            Mesh,
        },
        layout::Layout,
        Renderer as _,
    },
    widget::canvas::{Frame, Path, Text},
    Pixels, Point, Rectangle, Renderer, Size, Theme, Transformation, Vector,
};
use std::cmp::{max_by, min, min_by};

impl AudioClip {
    #[expect(clippy::too_many_lines)]
    pub fn draw(&self, renderer: &mut Renderer, theme: &Theme, layout: Layout, clip_top: f32) {
        let mut bounds = layout.bounds();
        let mut frame = Frame::new(renderer, bounds.size());

        // how many pixels of the top of the clip are clipped off by the top of the arrangement
        let hidden = min_by(0.0, bounds.y - clip_top, |a, b| a.partial_cmp(b).unwrap());

        // length of the downscaled audio we're using to draw
        let downscaled_len = self.arrangement.scale.read().unwrap().x.floor().exp2() as u32;

        // ratio of the length of the downscaled audio to the width of the clip
        let width_ratio = downscaled_len as f32 / self.arrangement.scale.read().unwrap().x.exp2();

        let global_start = self
            .get_global_start()
            .in_interleaved_samples(&self.arrangement.meter) as f32;

        // first_index: index of the first sample in the downscaled audio to draw
        let (first_index, index_offset) =
            if self.arrangement.position.read().unwrap().x > global_start {
                (
                    (self.arrangement.position.read().unwrap().x - global_start) as u32
                        / downscaled_len,
                    0,
                )
            } else {
                (
                    0,
                    (global_start - self.arrangement.position.read().unwrap().x) as u32
                        / downscaled_len,
                )
            };

        // index of the last sample in the downscaled audio to draw
        let last_index = min(
            self.get_global_end()
                .in_interleaved_samples(&self.arrangement.meter)
                / downscaled_len,
            first_index - index_offset + (frame.width() / width_ratio) as u32,
        );

        // first horizontal pixel of the clip
        let clip_first_x_pixel = index_offset as f32 * width_ratio;

        // text size
        let text_size = 12.0;
        // maximum height of the text
        let text_line_height = text_size * 1.5;
        // height of the waveform: the height of the clip minus the height of the text
        let waveform_height = self.arrangement.scale.read().unwrap().y - text_line_height;

        // the translucent background of the clip
        let background = Path::rectangle(
            Point::new(0.0, hidden),
            Size::new(bounds.width, bounds.height),
        );

        // the opaque background of the text
        let text_background = Path::rectangle(
            Point::new(0.0, hidden),
            Size::new(bounds.width, text_line_height),
        );

        let vertices_len = usize::try_from(2 * (last_index - first_index)).unwrap();

        if vertices_len > 2 {
            // vertices of the waveform
            let mut vertices = Vec::with_capacity(vertices_len);
            (first_index..last_index).enumerate().for_each(|(x, i)| {
                let (min, max) = self
                    .audio
                    .get_lod_at_index(self.arrangement.scale.read().unwrap().x as u32 - 3, i);
                vertices.push(SolidVertex2D {
                    position: [
                        (x as f32).mul_add(width_ratio, clip_first_x_pixel),
                        max_by(
                            min.mul_add(waveform_height, text_line_height),
                            clip_top - bounds.y,
                            |a, b| a.partial_cmp(b).unwrap(),
                        ),
                    ],
                    color: color::pack(theme.extended_palette().secondary.base.text.into_linear()),
                });
                vertices.push(SolidVertex2D {
                    position: [
                        (x as f32).mul_add(width_ratio, clip_first_x_pixel),
                        max_by(
                            max.mul_add(waveform_height, text_line_height),
                            clip_top - bounds.y,
                            |a, b| a.partial_cmp(b).unwrap(),
                        ),
                    ],
                    color: color::pack(theme.extended_palette().secondary.base.text.into_linear()),
                });
            });

            // triangles of the waveform
            let mut indices = Vec::with_capacity(3 * (vertices_len - 2));
            (0..vertices.len() - 2).for_each(|i| {
                let i = u32::try_from(i).unwrap();
                indices.push(i);
                indices.push(i + 1);
                indices.push(i + 2);
            });

            let waveform_mesh = Mesh::Solid {
                buffers: mesh::Indexed { vertices, indices },
                transformation: Transformation::IDENTITY,
                clip_bounds: Rectangle::INFINITE,
            };

            renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
                renderer.draw_mesh(waveform_mesh);
            });
        }

        // the name of the sample of the audio clip
        let text = Text {
            content: self.audio.name.clone(),
            position: Point::new(2.0, 2.0 + hidden),
            color: theme.extended_palette().secondary.base.text,
            size: Pixels(text_size),
            ..Text::default()
        };

        bounds.y -= hidden;

        frame.with_clip(bounds, |frame| {
            frame.fill(
                &background,
                theme
                    .extended_palette()
                    .primary
                    .weak
                    .color
                    .scale_alpha(0.25),
            );

            frame.fill(
                &text_background,
                theme.extended_palette().primary.weak.color,
            );

            frame.fill_text(text);
        });

        renderer.draw_geometry(frame.into_geometry());
    }
}
