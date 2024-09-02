use std::sync::{Arc, RwLock};

use crate::generic_back::{
    position::Meter,
    track_clip::{audio_clip::AudioClip, midi_clip::MidiClip, TrackClip},
};
use iced::widget::canvas::Frame;
use itertools::Itertools;

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
        (self.get_global_start().in_interleaved_samples(meter)
            ..self.get_global_end().in_interleaved_samples(meter))
            .chunks(timeline_x_scale)
            .into_iter()
            .enumerate()
            .filter(|(x, _)| *x <= width)
            .for_each(|(x, samples_group)| {
                let path = iced::widget::canvas::Path::new(|path| {
                    let (a, b) = samples_group
                        .map(|global_time| self.get_at_global_time(global_time, meter))
                        .minmax()
                        .into_option()
                        .unwrap();

                    path.line_to(iced::Point::new(
                        x as f32,
                        a.mul_add(timeline_y_scale as f32, y_offset as f32),
                    ));
                    path.line_to(iced::Point::new(
                        x as f32,
                        b.mul_add(timeline_y_scale as f32, y_offset as f32),
                    ));
                });
                frame.stroke(&path, iced::widget::canvas::Stroke::default());
            });
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
