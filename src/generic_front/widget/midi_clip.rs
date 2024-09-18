use crate::generic_back::track_clip::midi_clip::MidiClip;
use iced::{
    advanced::{layout::Layout, renderer::Quad, Renderer as _},
    Point, Rectangle, Renderer, Size, Theme, Vector,
};
use std::cmp::{max_by, min_by};

impl MidiClip {
    #[expect(clippy::unused_self)]
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        layout: Layout,
        clip_bounds: Rectangle,
    ) {
        let bounds = layout.bounds();

        // how many pixels of the top of the clip are clipped off by the top of the arrangement
        let hidden = min_by(0.0, bounds.y - clip_bounds.y, |a, b| {
            a.partial_cmp(b).unwrap()
        });

        // the translucent background of the clip
        let clip_background = Quad {
            bounds: Rectangle::new(
                Point::new(0.0, -hidden),
                Size::new(
                    bounds.width,
                    max_by(bounds.height + hidden, 0.0, |a, b| {
                        a.partial_cmp(b).unwrap()
                    }),
                ),
            ),
            ..Quad::default()
        };

        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
            renderer.fill_quad(
                clip_background,
                theme
                    .extended_palette()
                    .primary
                    .weak
                    .color
                    .scale_alpha(0.25),
            );
        });
    }
}
