use crate::{generic_back::track::midi_track::MidiTrack, generic_front::timeline::Message};
use iced::{
    advanced::{
        graphics::geometry::Renderer as _,
        layout::{self, Layout},
        renderer,
        widget::{self, Widget},
    },
    mouse,
    widget::canvas::{Frame, Path},
    Length, Point, Rectangle, Renderer, Size, Theme,
};
use std::sync::{Arc, RwLock};

impl Widget<Message, Theme, Renderer> for Arc<RwLock<MidiTrack>> {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Shrink,
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
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let mut frame = Frame::new(renderer, bounds.size());

        let path = Path::new(|path| {
            path.line_to(Point::new(0.0, bounds.height - 2.0));
            path.line_to(Point::new(bounds.width, bounds.height - 2.0));
        });

        frame.with_clip(bounds, |frame| {
            frame.stroke(
                &path,
                iced::widget::canvas::Stroke::default()
                    .with_color(theme.extended_palette().secondary.weak.color),
            );
        });

        renderer.draw_geometry(frame.into_geometry());
    }
}
