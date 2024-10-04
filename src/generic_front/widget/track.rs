use std::sync::Arc;

use crate::{
    generic_back::{Meter, Track, TrackClip},
    generic_front::ArrangementState,
};
use iced::{advanced::graphics::Mesh, Point, Rectangle, Renderer, Size, Theme};

impl Track {
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        bounds: Rectangle,
        arrangement_bounds: Rectangle,
        state: &ArrangementState,
    ) {
        self.clips().read().unwrap().iter().for_each(|clip| {
            let meter = match &**clip {
                TrackClip::Audio(clip) => &clip.meter,
                TrackClip::Midi(clip) => &clip.meter,
            };

            let first_pixel = (clip.get_global_start().in_interleaved_samples(meter) as f32
                - state.position.x)
                / state.scale.x.exp2()
                + bounds.x;

            let last_pixel = (clip.get_global_end().in_interleaved_samples(meter) as f32
                - state.position.x)
                / state.scale.x.exp2()
                + bounds.x;

            let clip_bounds = Rectangle::new(
                Point::new(first_pixel, bounds.y),
                Size::new(last_pixel - first_pixel, bounds.height),
            );
            let clip_bounds = bounds.intersection(&clip_bounds);
            if let Some(clip_bounds) = clip_bounds {
                clip.draw(renderer, theme, clip_bounds, arrangement_bounds);
            }
        });
    }

    pub fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        arrangement_bounds: Rectangle,
        state: &ArrangementState,
    ) -> Vec<Mesh> {
        self.clips()
            .read()
            .unwrap()
            .iter()
            .filter_map(|clip| {
                let meter = match &**clip {
                    TrackClip::Audio(clip) => &clip.meter,
                    TrackClip::Midi(clip) => &clip.meter,
                };

                let first_pixel = (clip.get_global_start().in_interleaved_samples(meter) as f32
                    - state.position.x)
                    / state.scale.x.exp2()
                    + bounds.x;

                let last_pixel = (clip.get_global_end().in_interleaved_samples(meter) as f32
                    - state.position.x)
                    / state.scale.x.exp2()
                    + bounds.x;

                let clip_bounds = Rectangle::new(
                    Point::new(first_pixel, bounds.y),
                    Size::new(last_pixel - first_pixel, bounds.height),
                );
                let clip_bounds = bounds.intersection(&clip_bounds);
                clip_bounds.and_then(|clip_bounds| {
                    clip.meshes(theme, clip_bounds, arrangement_bounds, state)
                })
            })
            .collect()
    }

    pub fn get_clip_at_global_time(
        &self,
        meter: &Arc<Meter>,
        global_time: u32,
    ) -> Option<Arc<TrackClip>> {
        self.clips().read().unwrap().iter().rev().find_map(|clip| {
            if clip.get_global_start().in_interleaved_samples(meter) <= global_time
                && global_time <= clip.get_global_end().in_interleaved_samples(meter)
            {
                Some(clip.clone())
            } else {
                None
            }
        })
    }
}
