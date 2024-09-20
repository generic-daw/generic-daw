use crate::generic_back::AudioTrack;
use iced::{Point, Rectangle, Renderer, Size, Theme};
use std::sync::atomic::Ordering::SeqCst;

impl AudioTrack {
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        bounds: Rectangle,
        arrangement_bounds: Rectangle,
    ) {
        self.clips.read().unwrap().iter().for_each(|clip| {
            let x_position = clip.arrangement.position.x.load(SeqCst);
            let x_scale = clip.arrangement.scale.x.load(SeqCst).exp2();

            let first_pixel = (clip
                .get_global_start()
                .in_interleaved_samples(&clip.arrangement.meter)
                as f32
                - x_position)
                / x_scale
                + bounds.x;

            let last_pixel = (clip
                .get_global_end()
                .in_interleaved_samples(&clip.arrangement.meter)
                as f32
                - x_position)
                / x_scale
                + bounds.x;

            let clip_bounds = Rectangle::new(
                Point::new(first_pixel, bounds.y),
                Size::new(last_pixel - first_pixel, bounds.height),
            );
            let clip_bounds = bounds.intersection(&clip_bounds);
            if let Some(clip_bounds) = clip_bounds {
                if clip_bounds.height > 1.0 {
                    clip.draw(renderer, theme, clip_bounds, arrangement_bounds);
                }
            }
        });
    }
}
