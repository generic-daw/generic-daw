use crate::generic_back::{
    position::Meter,
    track_clip::{audio_clip::AudioClip, midi_clip::MidiClip, TrackClip},
};
use iced::widget::canvas::Frame;
use itertools::Itertools;
use std::cmp::min;
use std::sync::{Arc, RwLock};

pub trait DrawableClip {
    fn draw(
        &self,
        frame: &mut Frame,
        timeline_x_scale: usize,
        timeline_y_scale: usize,
        width: usize,
        y_offset: usize,
        meter: &Arc<RwLock<Meter>>,
    );
}

impl DrawableClip for AudioClip {
    fn draw(
        &self,
        frame: &mut Frame,
        timeline_x_scale: usize,
        timeline_y_scale: usize,
        width: usize,
        y_offset: usize,
        meter: &Arc<RwLock<Meter>>,
    ) {
        let mut minmax = false;
        let path = iced::widget::canvas::Path::new(|path| {
            (self.get_global_start().in_interleaved_samples(meter)
                ..min(
                    self.get_global_end().in_interleaved_samples(meter),
                    u32::try_from(width * timeline_x_scale).unwrap(),
                ))
                .chunks(timeline_x_scale)
                .into_iter()
                .enumerate()
                .for_each(|(x, samples_group)| {
                    let (mut a, mut b) = samples_group
                        .map(|global_time| self.get_at_global_time(global_time, meter))
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
                        a.mul_add(timeline_y_scale as f32, y_offset as f32),
                    ));

                    if (a - b).abs() > f32::EPSILON {
                        path.line_to(iced::Point::new(
                            x as f32,
                            b.mul_add(timeline_y_scale as f32, y_offset as f32),
                        ));
                    }

                    minmax ^= true;
                });
        });
        frame.stroke(&path, iced::widget::canvas::Stroke::default());
    }
}

impl<'a> DrawableClip for MidiClip<'a> {
    fn draw(
        &self,
        _frame: &mut Frame,
        _timeline_x_scale: usize,
        _timeline_y_scale: usize,
        _width: usize,
        _y_offset: usize,
        _meter: &Arc<RwLock<Meter>>,
    ) {
        unimplemented!()
    }
}
