use crate::generic_back::track_clip::audio_clip::AudioClip;
use iced::{
    advanced::{
        graphics::{
            color,
            mesh::{self, Renderer as _, SolidVertex2D},
            Mesh,
        },
        layout::Layout,
        renderer::Quad,
        text::Renderer as _,
        Renderer as _, Text,
    },
    alignment::{Horizontal, Vertical},
    widget::text::{LineHeight, Shaping, Wrapping},
    Font, Pixels, Point, Rectangle, Renderer, Size, Theme, Transformation, Vector,
};
use std::cmp::{max_by, min, min_by};

impl AudioClip {
    #[expect(clippy::too_many_lines)]
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        layout: Layout,
        clip_bounds: Rectangle,
    ) {
        let bounds = layout.bounds();

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
            first_index - index_offset + (bounds.width / width_ratio) as u32,
        );

        let vertices_len = usize::try_from(2 * (last_index - first_index)).unwrap();

        // if there are less than 3 vertices, there's nothing to draw
        if vertices_len < 3 {
            return;
        }

        // first horizontal pixel of the clip
        let clip_first_x_pixel = index_offset as f32 * width_ratio;

        // how many pixels of the top of the clip are clipped off by the top of the arrangement
        let hidden = min_by(0.0, bounds.y - clip_bounds.y, |a, b| {
            a.partial_cmp(b).unwrap()
        });

        // text size
        let text_size = 12.0;
        // maximum height of the text
        let text_line_height = text_size * 1.5;
        // height of the waveform: the height of the clip minus the height of the text
        let waveform_height = self.arrangement.scale.read().unwrap().y - text_line_height;

        let clip_bounds = Rectangle::new(
            Point::new(0.0, -hidden),
            bounds.intersection(&clip_bounds).unwrap().size(),
        );

        if clip_bounds.height < 1.0 {
            return;
        }

        // the translucent background of the clip
        let clip_background = Quad {
            bounds: Rectangle::new(
                Point::new(0.0, -hidden),
                Size::new(
                    bounds.width,
                    max_by(bounds.height + hidden, 0.0, |a, b| {
                        a.partial_cmp(b).unwrap()
                    }),
                ),
            ),
            ..Quad::default()
        };

        // the opaque background of the text
        let text_background = Quad {
            bounds: Rectangle::new(
                Point::new(0.0, -hidden),
                Size::new(
                    bounds.width,
                    max_by(text_line_height + hidden, 0.0, |a, b| {
                        a.partial_cmp(b).unwrap()
                    }),
                ),
            ),
            ..Quad::default()
        };

        // vertices of the waveform
        let mut vertices = Vec::with_capacity(vertices_len);
        (first_index..last_index).enumerate().for_each(|(x, i)| {
            let (min, max) = self
                .audio
                .get_lod_at_index(self.arrangement.scale.read().unwrap().x as u32 - 3, i);
            vertices.push(SolidVertex2D {
                position: [
                    (x as f32).mul_add(width_ratio, clip_first_x_pixel),
                    min.mul_add(waveform_height, text_line_height),
                ],
                color: color::pack(theme.extended_palette().secondary.base.text.into_linear()),
            });
            vertices.push(SolidVertex2D {
                position: [
                    (x as f32).mul_add(width_ratio, clip_first_x_pixel),
                    max.mul_add(waveform_height, text_line_height),
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

        // the waveform mesh with the clip bounds
        let waveform_mesh = Mesh::Solid {
            buffers: mesh::Indexed { vertices, indices },
            transformation: Transformation::IDENTITY,
            clip_bounds,
        };

        let text = Text {
            content: self.audio.name.clone(),
            bounds: Size::new(f32::INFINITY, 0.0),
            size: Pixels(text_size),
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
