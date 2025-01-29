use generic_daw_core::Meter;

pub trait TrackExt {
    fn get_clip_at_global_time(&self, meter: &Meter, global_time: usize) -> Option<usize>;
}
