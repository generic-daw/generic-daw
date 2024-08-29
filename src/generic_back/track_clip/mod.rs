pub mod audio_clip;
pub mod midi_clip;

pub trait TrackClip: Send + Sync {
    fn get_at_global_time(&self, global_time: u32) -> f32;
    fn get_global_start(&self) -> u32;
    fn get_global_end(&self) -> u32;
}
