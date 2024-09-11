use crate::{generic_back::track::midi_track::MidiTrack, generic_front::timeline::Message};
use iced::{
    advanced::{
        layout::{self, Layout},
        renderer,
        widget::{self, Widget},
    },
    mouse, Length, Rectangle, Renderer, Size, Theme,
};
use std::sync::{Arc, RwLock};

impl Widget<Message, Theme, Renderer> for Arc<RwLock<MidiTrack>> {
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
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}
