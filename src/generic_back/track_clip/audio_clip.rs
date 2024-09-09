pub mod interleaved_audio;

use crate::{
    generic_back::{meter::Meter, position::Position},
    generic_front::drawable::{Drawable, TimelinePosition, TimelineScale},
};
use iced::{
    widget::canvas::{Frame, Text},
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

    pub fn trim_start_to(&mut self, clip_start: Position) {
        self.clip_start = clip_start;
    }

    pub fn trim_end_to(&mut self, global_end: Position) {
        self.global_end = global_end;
    }

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
        //
        // index_offset: offset of the first drawn index from the left edge of the timeline
        // add this to the x position of whatever is being drawn
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

        // maximum height of the text
        let text_scale = 12.0 * 1.5;
        // ratio of the text height to the height of the track
        let text_scale_ratio = 1.0 - (text_scale / scale.y);

        // the translucent background of the track
        let background = iced::widget::canvas::Path::rectangle(
            Point::new(index_offset as f32 * width_ratio, position.y * scale.y),
            Size::new((last_index - first_index) as f32 * width_ratio, scale.y),
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
        let background = iced::widget::canvas::Path::rectangle(
            Point::new(index_offset as f32 * width_ratio, position.y * scale.y),
            Size::new((last_index - first_index) as f32 * width_ratio, text_scale),
        );
        frame.fill(&background, theme.extended_palette().primary.weak.color);

        // the path of the audio clip
        // this sometimes breaks, see https://github.com/iced-rs/iced/issues/2567
        let path = iced::widget::canvas::Path::new(|path| {
            (first_index..last_index).enumerate().for_each(|(x, i)| {
                let (min, max) = self.get_downscaled_at_index(scale.x as usize, i);

                path.line_to(iced::Point::new(
                    (x + index_offset) as f32 * width_ratio,
                    min.mul_add(text_scale_ratio, position.y)
                        .mul_add(scale.y, text_scale),
                ));

                if (min - max).abs() > f32::EPSILON {
                    path.line_to(iced::Point::new(
                        (x + index_offset) as f32 * width_ratio,
                        max.mul_add(text_scale_ratio, position.y)
                            .mul_add(scale.y, text_scale),
                    ));
                }
            });
        });
        frame.stroke(
            &path,
            iced::widget::canvas::Stroke::default()
                .with_color(theme.extended_palette().secondary.base.text),
        );

        // the name of the sample of the audio clip
        // TODO: clip this to the end of the track
        let text = Text {
            content: self.audio.name.to_string(),
            position: Point::new(
                (index_offset as f32).mul_add(width_ratio, 2.0),
                position.y.mul_add(scale.y, 2.0),
            ),
            color: theme.extended_palette().secondary.base.text,
            size: Pixels(text_scale / 1.5),
            ..Text::default()
        };
        frame.fill_text(text);
    }
}
