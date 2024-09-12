use crate::generic_back::track_clip::audio_clip::AudioClip;
use iced::{
    advanced::layout::Layout,
    widget::canvas::{Frame, Path, Stroke, Text},
    Pixels, Point, Size, Theme,
};
use std::cmp::min;

impl AudioClip {
    #[expect(clippy::too_many_lines)]
    pub fn draw(&self, frame: &mut Frame, theme: &Theme, layout: Layout) {
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
        let background =
            Path::rectangle(Point::new(0.0, 0.0), Size::new(bounds.width, bounds.height));

        // the opaque background of the text
        let text_background = Path::rectangle(
            Point::new(0.0, 0.0),
            Size::new(bounds.width, text_line_height),
        );

        // the name of the sample of the audio clip
        let text = Text {
            content: self.audio.name.clone(),
            position: Point::new(2.0, 2.0),
            color: theme.extended_palette().secondary.base.text,
            size: Pixels(text_size),
            ..Text::default()
        };

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

            // draw the average of the min and max of the waveform so that it doesn't disappear if the difference between the min and max is too small
            let waveform = Path::new(|path| {
                // frame.fill() is O(n^2), so we split the waveform into many quads instead of filling it all at once
                let mut old = self
                    .get_downscaled_at_index(self.arrangement.scale.read().unwrap().x as u32, 0);

                path.line_to(Point::new(
                    clip_first_x_pixel,
                    ((old.0 + old.1) * 0.5).mul_add(waveform_height, text_line_height),
                ));

                (first_index + 1..last_index)
                    .enumerate()
                    .for_each(|(x, i)| {
                        let fill = Path::new(|fill| {
                            fill.line_to(Point::new(
                                (x as f32).mul_add(width_ratio, clip_first_x_pixel),
                                old.0.mul_add(waveform_height, text_line_height),
                            ));

                            if (old.1 - old.0).abs() > f32::EPSILON {
                                fill.line_to(Point::new(
                                    (x as f32).mul_add(width_ratio, clip_first_x_pixel),
                                    old.1.mul_add(waveform_height, text_line_height),
                                ));
                            }

                            let new = self.get_downscaled_at_index(
                                self.arrangement.scale.read().unwrap().x as u32,
                                i,
                            );

                            fill.line_to(Point::new(
                                (x as f32 + 1.0).mul_add(width_ratio, clip_first_x_pixel),
                                new.1.mul_add(waveform_height, text_line_height),
                            ));

                            if (new.1 - new.0).abs() > f32::EPSILON {
                                fill.line_to(Point::new(
                                    (x as f32 + 1.0).mul_add(width_ratio, clip_first_x_pixel),
                                    new.0.mul_add(waveform_height, text_line_height),
                                ));
                            }

                            fill.close();

                            if (new.0 + new.1 - old.0 - old.1).abs() > f32::EPSILON {
                                path.line_to(Point::new(
                                    (x as f32).mul_add(width_ratio, clip_first_x_pixel),
                                    ((new.0 + new.1) * 0.5)
                                        .mul_add(waveform_height, text_line_height),
                                ));
                            }

                            old = new;
                        });
                        frame.fill(&fill, theme.extended_palette().secondary.base.text);
                    });

                path.line_to(Point::new(
                    bounds.width,
                    ((old.0 + old.1) * 0.5).mul_add(waveform_height, text_line_height),
                ));
            });

            frame.stroke(
                &waveform,
                Stroke::default().with_color(theme.extended_palette().secondary.base.text),
            );

            frame.fill_text(text);
        });
    }
}
