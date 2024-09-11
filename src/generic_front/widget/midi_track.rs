use crate::generic_back::track::midi_track::MidiTrack;
use iced::{advanced::layout::Layout, Rectangle, Renderer, Theme};

impl MidiTrack {
    #[expect(clippy::unused_self)]
    pub fn draw(
        &self,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _layout: Layout,
        _viewport: &Rectangle,
    ) {
    }
}
