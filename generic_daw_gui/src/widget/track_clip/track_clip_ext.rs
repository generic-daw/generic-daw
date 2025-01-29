use super::ArrangementScale;
use crate::widget::ArrangementPosition;
use iced::{advanced::graphics::Mesh, Size, Theme};

pub trait TrackClipExt {
    fn mesh(
        &self,
        theme: &Theme,
        size: Size,
        position: ArrangementPosition,
        scale: ArrangementScale,
    ) -> Mesh;
}
