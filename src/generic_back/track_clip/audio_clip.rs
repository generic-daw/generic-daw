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

    pub fn get_ver_at_index(&self, ver: usize, index: usize) -> (f32, f32) {
        let (min, max) = self.audio.get_ver_at_index(ver, index);
        (min * self.volume / 2.0 + 0.5, max * self.volume / 2.0 + 0.5)
    }

    pub fn get_at_global_time(&self, global_time: u32, meter: &Meter) -> f32 {
        if !meter.playing.load(SeqCst)
            || global_time < self.global_start.in_interleaved_samples(meter)
            || global_time > self.global_end.in_interleaved_samples(meter)
        {
            return 0.0;
        }
        self.audio.get_sample_at_index(
            usize::try_from(global_time).unwrap()
                - usize::try_from(
                    (self.global_start + self.clip_start).in_interleaved_samples(meter),
                )
                .unwrap(),
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
        let ver_len = scale.x.floor().exp2() as u32;
        let ratio = ver_len as f32 / scale.x.exp2();
        let start = if position.x > self.get_global_start() {
            (position.x - self.get_global_start()).in_interleaved_samples(meter) / ver_len
        } else {
            0
        };
        let offset = if self.get_global_start() > position.x {
            (self.get_global_start() - position.x).in_interleaved_samples(meter) / ver_len
        } else {
            0
        };
        let end = min(
            self.get_global_end().in_interleaved_samples(meter) / ver_len,
            start - offset + (frame.width() / ratio) as u32,
        );

        let text_scale = 12.0 * 1.5;
        let text_scale_ratio = 1.0 - (text_scale / scale.y);

        let background = iced::widget::canvas::Path::rectangle(
            Point::new(
                offset as f32 * ratio,
                position.y.mul_add(scale.y, text_scale),
            ),
            Size::new((end - start) as f32 * ratio, scale.y - text_scale),
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

        let background = iced::widget::canvas::Path::rectangle(
            Point::new(offset as f32 * ratio, position.y * scale.y),
            Size::new((end - start) as f32 * ratio, text_scale),
        );
        frame.fill(&background, theme.extended_palette().primary.weak.color);

        // this sometimes breaks, see https://github.com/iced-rs/iced/issues/2567

        let path = iced::widget::canvas::Path::new(|path| {
            (start..end).enumerate().for_each(|(x, i)| {
                let (min, max) =
                    self.get_ver_at_index(scale.x as usize, usize::try_from(i).unwrap());

                path.line_to(iced::Point::new(
                    (x + usize::try_from(offset).unwrap()) as f32 * ratio,
                    min.mul_add(text_scale_ratio, position.y)
                        .mul_add(scale.y, text_scale),
                ));

                if (min - max).abs() > f32::EPSILON {
                    path.line_to(iced::Point::new(
                        (x + usize::try_from(offset).unwrap()) as f32 * ratio,
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

        let text = Text {
            content: self.audio.name.to_string(),
            position: Point::new(
                (offset as f32).mul_add(ratio, 2.0),
                position.y.mul_add(scale.y, 2.0),
            ),
            color: theme.extended_palette().secondary.base.text,
            size: Pixels(text_scale / 1.5),
            ..Text::default()
        };
        frame.fill_text(text);
    }
}
