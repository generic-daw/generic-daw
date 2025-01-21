use super::{ArrangementPosition, ArrangementScale};
use iced::{advanced::graphics::Mesh, Rectangle, Theme};

pub trait TrackClipExt {
    fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        viewport: Rectangle,
        position: &ArrangementPosition,
        scale: &ArrangementScale,
    ) -> Option<Mesh>;
}
