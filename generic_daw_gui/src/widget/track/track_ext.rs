use crate::widget::{ArrangementPosition, ArrangementScale};
use generic_daw_core::Meter;
use iced::{advanced::graphics::Mesh, Rectangle, Theme};

pub trait TrackExt {
    fn get_clip_at_global_time(&self, meter: &Meter, global_time: usize) -> Option<usize>;

    fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        viewport: Rectangle,
        position: ArrangementPosition,
        scale: ArrangementScale,
    ) -> Vec<Mesh>;
}
