use crate::generic_back::track_clip::midi_clip::MidiClip;
use iced::{
    advanced::{
        graphics::geometry::{frame::Backend, Renderer as _},
        layout::Layout,
    },
    widget::canvas::Path,
    Point, Rectangle, Renderer, Size, Theme,
};

impl MidiClip {
    #[expect(clippy::unused_self)]
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        layout: Layout,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let mut frame = renderer.new_frame(bounds.size());

        // the translucent background of the clip
        let background = Path::rectangle(
            Point::new(0.0, 0.0),
            Size::new(viewport.width, viewport.height),
        );
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
