use crate::generic_back::track_clip::midi_clip::MidiClip;
use iced::{
    advanced::{
        graphics::geometry::{frame::Backend, Renderer as _},
        layout::{self, Layout},
        renderer,
        widget::{self, Widget},
    },
    mouse,
    widget::canvas::Path,
    Length, Point, Rectangle, Renderer, Size, Theme,
};

impl<Message> Widget<Message, Theme, Renderer> for MidiClip {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(
        &self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(Size::new(limits.max().width, limits.max().height))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
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
