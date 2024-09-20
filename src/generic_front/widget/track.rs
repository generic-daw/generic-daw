use crate::generic_back::Track;
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
            Self::Audio(track) => track.draw(renderer, theme, bounds, clip_bounds),
            Self::Midi(track) => track.draw(renderer, theme, bounds, clip_bounds),
        }
    }
}
