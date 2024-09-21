use crate::{generic_back::MidiTrack, generic_front::ArrangementState};
use iced::{Point, Rectangle, Renderer, Size, Theme};

impl MidiTrack {
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        bounds: Rectangle,
        arrangement_bounds: Rectangle,
        state: &ArrangementState,
    ) {
        self.clips.read().unwrap().iter().for_each(|clip| {
            let first_pixel = (clip
                .get_global_start()
                .in_interleaved_samples(&clip.arrangement.meter)
                as f32
                - state.position.x)
                / state.scale.x.exp2()
                + bounds.x;

            let last_pixel = (clip
                .get_global_end()
                .in_interleaved_samples(&clip.arrangement.meter)
                as f32
                - state.position.x)
                / state.scale.x.exp2()
                + bounds.x;

            let clip_bounds = Rectangle::new(
                Point::new(first_pixel, bounds.y),
                Size::new(last_pixel - first_pixel, bounds.height),
            );
            let clip_bounds = bounds.intersection(&clip_bounds);
            if let Some(clip_bounds) = clip_bounds {
                clip.draw(renderer, theme, clip_bounds, arrangement_bounds, state);
            }
        });
    }
}
