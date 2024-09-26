use crate::{generic_back::Track, generic_front::ArrangementState};
use iced::{advanced::graphics::Mesh, Rectangle, Renderer, Theme};

impl Track {
    pub fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        bounds: Rectangle,
        clip_bounds: Rectangle,
        state: &ArrangementState,
    ) {
        match self {
            Self::Audio(track) => track.draw(renderer, theme, bounds, clip_bounds, state),
            Self::Midi(track) => track.draw(renderer, theme, bounds, clip_bounds, state),
        }
    }

    pub fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        clip_bounds: Rectangle,
        state: &ArrangementState,
    ) -> Vec<Mesh> {
        match self {
            Self::Audio(track) => track.meshes(theme, bounds, clip_bounds, state),
            Self::Midi(_) => Vec::new(),
        }
    }
}
