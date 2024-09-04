use crate::generic_back::{
    position::Meter,
    track_clip::{audio_clip::AudioClip, midi_clip::MidiClip, TrackClip},
};
use iced::{widget::canvas::Frame, Theme};
use std::cmp::{max, min};

pub struct Position {
    pub x: usize,
    pub y: isize,
}

#[derive(Clone, Copy)]
pub struct Scale {
    pub x: f32,
    pub y: f32,
}

pub trait DrawableClip {
    fn draw(&self, frame: &mut Frame, scale: Scale, offset: Position, meter: &Meter, theme: &Theme);
}

impl DrawableClip for AudioClip {
    fn draw(
        &self,
        frame: &mut Frame,
        scale: Scale,
        position: Position,
        meter: &Meter,
        theme: &Theme,
    ) {
        let path = iced::widget::canvas::Path::new(|path| {
            let mut minmax = false;
            let ver_len = scale.x.floor().exp2() as usize;
            let ratio = ver_len as f32 / scale.x.exp2();
            let start = max(
                self.get_global_start().in_interleaved_samples(meter) as usize / ver_len,
                position.x / ver_len,
            );
            let end = min(
                self.get_global_end().in_interleaved_samples(meter) as usize / ver_len,
                start + (frame.width() / ratio) as usize,
            );
            (start..end).enumerate().for_each(|(x, i)| {
                let (mut a, mut b) = self.get_ver_at_index(scale.x as usize, i);
                if minmax {
                    if a < b {
                        std::mem::swap(&mut a, &mut b);
                    }
                } else if a > b {
                    std::mem::swap(&mut a, &mut b);
                }

                path.line_to(iced::Point::new(
                    x as f32 * ratio,
                    a.mul_add(scale.y, position.y as f32),
                ));

                if (a - b).abs() > f32::EPSILON {
                    path.line_to(iced::Point::new(
                        x as f32 * ratio,
                        b.mul_add(scale.y, position.y as f32),
                    ));
                }

                minmax ^= true;
            });
        });
        frame.stroke(
            &path,
            iced::widget::canvas::Stroke::default()
                .with_color(theme.extended_palette().secondary.strong.color),
        );
    }
}

impl<'a> DrawableClip for MidiClip<'a> {
    fn draw(
        &self,
        _frame: &mut Frame,
        _scale: Scale,
        _offset: Position,
        _meter: &Meter,
        _theme: &Theme,
    ) {
        unimplemented!()
    }
}
