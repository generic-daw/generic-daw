use crate::generic_back::track::Track;
use iced::{Rectangle, Renderer, Theme};

impl Track {
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        bounds: Rectangle,
        clip_bounds: Rectangle,
    ) {
        match self {
            Self::Audio(track) => track
                .read()
                .unwrap()
                .draw(renderer, theme, bounds, clip_bounds),
            Self::Midi(track) => track
                .read()
                .unwrap()
                .draw(renderer, theme, bounds, clip_bounds),
        }
    }
}
