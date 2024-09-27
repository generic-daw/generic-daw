use crate::{generic_back::Track, generic_front::ArrangementState};
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
        let arrangement = match self {
            Self::Audio(track) => track.arrangement.clone(),
            Self::Midi(track) => track.arrangement.clone(),
        };

        self.clips().iter().for_each(|clip| {
            let first_pixel = (clip
                .get_global_start()
                .in_interleaved_samples(&arrangement.meter) as f32
                - state.position.x)
                / state.scale.x.exp2()
                + bounds.x;

            let last_pixel = (clip
                .get_global_end()
                .in_interleaved_samples(&arrangement.meter) as f32
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
        let arrangement = match self {
            Self::Audio(track) => track.arrangement.clone(),
            Self::Midi(track) => track.arrangement.clone(),
        };

        self.clips()
            .iter()
            .filter_map(|clip| {
                let first_pixel = (clip
                    .get_global_start()
                    .in_interleaved_samples(&arrangement.meter)
                    as f32
                    - state.position.x)
                    / state.scale.x.exp2()
                    + bounds.x;

                let last_pixel = (clip
                    .get_global_end()
                    .in_interleaved_samples(&arrangement.meter)
                    as f32
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
}
