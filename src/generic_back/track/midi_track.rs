use crate::{
    clap_host::ClapPluginWrapper,
    generic_back::{Meter, Position, Track, TrackClip},
};
use atomig::Atomic;
use plugin_state::PluginState;
use std::sync::{Arc, Mutex, RwLock};

pub use dirty_event::DirtyEvent;

mod dirty_event;
mod plugin_state;

#[derive(Debug)]
pub struct MidiTrack {
    /// these are all guaranteed to be `TrackClip::Midi`
    pub clips: RwLock<Vec<Arc<TrackClip>>>,
    /// between 0.0 and 1.0
    pub volume: Atomic<f32>,
    /// between -1.0 (left) and 1.0 (right)
    pub pan: Atomic<f32>,
    /// holds all the state needed for a generator plugin to function properly
    pub plugin_state: Mutex<PluginState>,
    pub meter: Arc<Meter>,
}

impl MidiTrack {
    pub fn create(plugin: ClapPluginWrapper, meter: Arc<Meter>) -> Arc<Track> {
        Arc::new(Track::Midi(Self {
            clips: RwLock::default(),
            volume: Atomic::new(1.0),
            pan: Atomic::new(0.0),
            plugin_state: PluginState::create(plugin),
            meter,
        }))
    }

    pub fn get_global_end(&self) -> Position {
        self.clips
            .read()
            .unwrap()
            .iter()
            .map(|clip| clip.get_global_end())
            .max()
            .unwrap_or_else(Position::default)
    }
}
