use crate::generic_back::{
    position::{Meter, Position},
    track_clip::{audio_clip::AudioClip, midi_clip::MidiClip, TrackClip},
};
use iced::{widget::canvas::Frame, Theme};
use std::cmp::{max, min};

pub struct TimelinePosition {
    /// position of the left of the timeline relative to the start of the arrangement, in beats
    pub x: Position,
    /// position of the top of the timeline relative to the top of the first track, in tracks
    pub y: f32,
}

#[derive(Clone, Copy)]
pub struct TimelineScale {
    /// log2 of the horizontal scale
    pub x: f32,
    /// height in pixels of each track in the timeline
    pub y: f32,
}

pub trait DrawableClip {
    fn draw(
        &self,
        frame: &mut Frame,
        scale: TimelineScale,
        offset: TimelinePosition,
        meter: &Meter,
        theme: &Theme,
    );
}

impl DrawableClip for AudioClip {
    fn draw(
        &self,
        frame: &mut Frame,
        scale: TimelineScale,
        position: TimelinePosition,
        meter: &Meter,
        theme: &Theme,
    ) {
        let path = iced::widget::canvas::Path::new(|path| {
            let mut minmax = false;
            let ver_len = scale.x.floor().exp2() as u32;
            let ratio = ver_len as f32 / scale.x.exp2();
            let start = max(
                self.get_global_start().in_interleaved_samples(meter) / ver_len,
                position.x.in_interleaved_samples(meter) / ver_len,
            );
            let end = min(
                self.get_global_end().in_interleaved_samples(meter) / ver_len,
                start + (frame.width() / ratio) as u32,
            );
            (start..end).enumerate().for_each(|(x, i)| {
                let (mut a, mut b) = self.get_ver_at_index(scale.x as usize, i as usize);

                // this is a workaround for iced not rendering too steep lines
                if minmax {
                    if a < b {
                        std::mem::swap(&mut a, &mut b);
                    }
                } else if a > b {
                    std::mem::swap(&mut a, &mut b);
                }

                path.line_to(iced::Point::new(
                    x as f32 * ratio,
                    (a + position.y) * scale.y,
                ));

                if (a - b).abs() > f32::EPSILON {
                    path.line_to(iced::Point::new(
                        x as f32 * ratio,
                        (b + position.y) * scale.y,
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
        _scale: TimelineScale,
        _offset: TimelinePosition,
        _meter: &Meter,
        _theme: &Theme,
    ) {
        unimplemented!()
    }
}
