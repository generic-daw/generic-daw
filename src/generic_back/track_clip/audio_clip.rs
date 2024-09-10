pub mod interleaved_audio;

use crate::{
    generic_back::{meter::Meter, position::Position},
    generic_front::drawable::{Drawable, TimelinePosition, TimelineScale},
};
use iced::{
    widget::canvas::{Frame, Path, Stroke, Text},
    Pixels, Point, Size, Theme,
};
use interleaved_audio::InterleavedAudio;
use std::{
    cmp::min,
    sync::{atomic::Ordering::SeqCst, Arc},
};

pub struct AudioClip {
    audio: Arc<InterleavedAudio>,
    global_start: Position,
    global_end: Position,
    clip_start: Position,
    volume: f32,
}

impl AudioClip {
    pub fn new(audio: Arc<InterleavedAudio>, meter: &Meter) -> Self {
        let samples = audio.len();
        Self {
            audio,
            global_start: Position::new(0, 0),
            global_end: Position::from_interleaved_samples(samples, meter),
            clip_start: Position::new(0, 0),
            volume: 1.0,
        }
    }

    pub fn get_downscaled_at_index(&self, ds_index: usize, index: usize) -> (f32, f32) {
        let (min, max) = self.audio.get_downscaled_at_index(ds_index, index);
        (min * self.volume / 2.0 + 0.5, max * self.volume / 2.0 + 0.5)
    }

    pub fn get_at_global_time(&self, global_time: usize, meter: &Meter) -> f32 {
        if !meter.playing.load(SeqCst)
            || global_time < self.global_start.in_interleaved_samples(meter)
            || global_time > self.global_end.in_interleaved_samples(meter)
        {
            return 0.0;
        }
        self.audio.get_sample_at_index(
            global_time - (self.global_start + self.clip_start).in_interleaved_samples(meter),
        ) * self.volume
    }

    pub const fn get_global_start(&self) -> Position {
        self.global_start
    }

    pub const fn get_global_end(&self) -> Position {
        self.global_end
    }

    #[expect(dead_code)]
    pub fn trim_start_to(&mut self, clip_start: Position) {
        self.clip_start = clip_start;
    }

    #[expect(dead_code)]
    pub fn trim_end_to(&mut self, global_end: Position) {
        self.global_end = global_end;
    }

    #[expect(dead_code)]
    pub fn move_start_to(&mut self, global_start: Position) {
        match self.global_start.cmp(&global_start) {
            std::cmp::Ordering::Less => {
                self.global_end += global_start - self.global_start;
            }
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Greater => {
                self.global_end += self.global_start - global_start;
            }
        }
        self.global_start = global_start;
    }
}

impl Drawable for AudioClip {
    fn draw(
        &self,
        frame: &mut Frame,
        scale: TimelineScale,
        position: &TimelinePosition,
        meter: &Meter,
        theme: &Theme,
    ) {
        // length of the downscaled audio we're using to draw
        let downscaled_len = scale.x.floor().exp2() as usize;

        // ratio of the length of the downscaled audio to the width of the timeline
        let width_ratio = downscaled_len as f32 / scale.x.exp2();

        let global_start = self.get_global_start().in_interleaved_samples(meter) as f32;

        // first_index: index of the first sample in the downscaled audio to draw
        let (first_index, index_offset) = if position.x > global_start {
            ((position.x - global_start) as usize / downscaled_len, 0)
        } else {
            (0, (global_start - position.x) as usize / downscaled_len)
        };

        // index of the last sample in the downscaled audio to draw
        let last_index = min(
            self.get_global_end().in_interleaved_samples(meter) / downscaled_len,
            first_index - index_offset + (frame.width() / width_ratio) as usize,
        );

        // first horizontal pixel of the clip
        let clip_first_x_pixel = index_offset as f32 * width_ratio;
        // width of the clip in pixels
        let clip_width_pixels = (last_index - first_index) as f32 * width_ratio;

        // first vertical pixel of the clip
        let clip_first_y_pixel = position.y * scale.y;

        // text size
        let text_size = 12.0;
        // maximum height of the text
        let text_line_height = text_size * 1.5;

        // height of the waveform: the height of the clip minus the height of the text
        let waveform_height = scale.y - text_line_height;
        // first vertical pixel of the waveform
        let waveform_first_y_pixel = clip_first_y_pixel + text_line_height;

        // the translucent background of the clip
        let background = Path::rectangle(
            Point::new(clip_first_x_pixel, clip_first_y_pixel),
            Size::new(clip_width_pixels, scale.y),
        );
        frame.fill(
            &background,
            theme
                .extended_palette()
                .primary
                .weak
                .color
                .scale_alpha(0.25),
        );

        // the opaque background of the text
        let background = Path::rectangle(
            Point::new(clip_first_x_pixel, clip_first_y_pixel),
            Size::new(clip_width_pixels, text_line_height),
        );
        frame.fill(&background, theme.extended_palette().primary.weak.color);

        // the path of the audio clip
        // this sometimes breaks, see https://github.com/iced-rs/iced/issues/2567
        let path = Path::new(|path| {
            (first_index..last_index).enumerate().for_each(|(x, i)| {
                let (min, max) = self.get_downscaled_at_index(scale.x as usize, i);

                path.line_to(Point::new(
                    (x as f32).mul_add(width_ratio, clip_first_x_pixel),
                    min.mul_add(waveform_height, waveform_first_y_pixel),
                ));

                if (min - max).abs() > f32::EPSILON {
                    path.line_to(Point::new(
                        (x as f32).mul_add(width_ratio, clip_first_x_pixel),
                        max.mul_add(waveform_height, waveform_first_y_pixel),
                    ));
                }
            });
        });
        frame.stroke(
            &path,
            Stroke::default().with_color(theme.extended_palette().secondary.base.text),
        );

        // the name of the sample of the audio clip
        // TODO: clip this to the end of the clip
        let text = Text {
            content: self.audio.name.to_string(),
            position: Point::new(clip_first_x_pixel + 2.0, clip_first_y_pixel + 2.0),
            color: theme.extended_palette().secondary.base.text,
            size: Pixels(text_size),
            ..Text::default()
        };
        frame.fill_text(text);
    }
}
