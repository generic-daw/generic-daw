use crate::generic_back::track_clip::midi_clip::MidiClip;
use iced::{
    advanced::{graphics::geometry::Renderer as _, layout::Layout},
    widget::canvas::{Frame, Path},
    Point, Renderer, Size, Theme,
};

impl MidiClip {
    #[expect(clippy::unused_self)]
    pub fn draw(&self, renderer: &mut Renderer, theme: &Theme, layout: Layout) {
        let bounds = layout.bounds();

        let mut frame = Frame::new(renderer, bounds.size());

        // the translucent background of the clip
        let background =
            Path::rectangle(Point::new(0.0, 0.0), Size::new(bounds.width, bounds.height));
        frame.fill(
            &background,
            theme
                .extended_palette()
                .primary
                .weak
                .color
                .scale_alpha(0.25),
        );

        renderer.draw_geometry(frame.into_geometry());
    }
}
