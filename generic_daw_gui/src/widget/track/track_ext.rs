use crate::widget::{ArrangementPosition, ArrangementScale};
use generic_daw_core::{Meter, TrackClip};
use iced::{advanced::graphics::Mesh, Rectangle, Theme};
use std::sync::Arc;

pub trait TrackExt {
    fn get_clip_at_global_time(&self, meter: &Meter, global_time: usize) -> Option<Arc<TrackClip>>;

    fn meshes(
        &self,
        theme: &Theme,
        bounds: Rectangle,
        viewport: Rectangle,
        position: &ArrangementPosition,
        scale: &ArrangementScale,
    ) -> Vec<Mesh>;
}
