use crate::generic_back::{
    position::Meter,
    track_clip::{audio_clip::AudioClip, midi_clip::MidiClip, TrackClip},
};
use iced::{widget::canvas::Frame, Theme};
use itertools::Itertools;
use std::cmp::{max, min};

pub struct Position {
    pub x: usize,
    pub y: isize,
}

#[derive(Clone, Copy)]
pub struct Scale {
    pub x: usize,
    pub y: usize,
}

pub trait DrawableClip {
    fn draw(&self, frame: &mut Frame, scale: Scale, offset: Position, meter: &Meter, theme: &Theme);
}

impl DrawableClip for AudioClip {
    fn draw(
        &self,
        frame: &mut Frame,
        scale: Scale,
        offset: Position,
        meter: &Meter,
        theme: &Theme,
    ) {
        let mut minmax = false;
        let path = iced::widget::canvas::Path::new(|path| {
            (max(
                self.get_global_start().in_interleaved_samples(meter),
                offset.x as u32,
            )
                ..min(
                    self.get_global_end().in_interleaved_samples(meter),
                    u32::try_from(frame.width() as usize * scale.x).unwrap() + offset.x as u32,
                ))
                .chunks(scale.x)
                .into_iter()
                .enumerate()
                .for_each(|(x, samples_group)| {
                    let (mut a, mut b) = samples_group
                        .map(|global_time| {
                            self.get_at_global_time(global_time, meter).clamp(-1.0, 1.0)
                        })
                        .minmax()
                        .into_option()
                        .unwrap();

                    if minmax {
                        if a < b {
                            std::mem::swap(&mut a, &mut b);
                        }
                    } else if a > b {
                        std::mem::swap(&mut a, &mut b);
                    }

                    path.line_to(iced::Point::new(
                        x as f32,
                        a.mul_add(scale.y as f32, offset.y as f32),
                    ));

                    if (a - b).abs() > f32::EPSILON {
                        path.line_to(iced::Point::new(
                            x as f32,
                            b.mul_add(scale.y as f32, offset.y as f32),
                        ));
                    }

                    minmax ^= true;
                });
        });
        frame.stroke(
            &path,
            iced::widget::canvas::Stroke::default().with_color(theme.palette().text),
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
