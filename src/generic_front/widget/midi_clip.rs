use std::cmp::min_by;

use crate::generic_back::track_clip::midi_clip::MidiClip;
use iced::{
    advanced::{graphics::geometry::Renderer as _, layout::Layout},
    widget::canvas::{Frame, Path},
    Point, Renderer, Size, Theme,
};

impl MidiClip {
    #[expect(clippy::unused_self)]
    pub fn draw(&self, renderer: &mut Renderer, theme: &Theme, layout: Layout, clip_top: f32) {
        let mut bounds = layout.bounds();
        let mut frame = Frame::new(renderer, bounds.size());

        // how many pixels of the top of the clip are clipped off by the top of the arrangement
        let hidden = min_by(0.0, bounds.y - clip_top, |a, b| a.partial_cmp(b).unwrap());

        // the translucent background of the clip
        let background = Path::rectangle(
            Point::new(0.0, hidden),
            Size::new(bounds.width, bounds.height),
        );

        bounds.y -= hidden;

        frame.with_clip(bounds, |frame| {
            frame.fill(
                &background,
                theme
                    .extended_palette()
                    .primary
                    .weak
                    .color
                    .scale_alpha(0.25),
            );
        });

        renderer.draw_geometry(frame.into_geometry());
    }
}
