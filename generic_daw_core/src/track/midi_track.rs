use crate::{Meter, Position, Track, TrackClip};
use atomig::Atomic;
use clap_host::PluginAudioProcessor;
use plugin_state::PluginState;
use std::sync::{Arc, Mutex, RwLock};

pub mod dirty_event;
pub mod plugin_state;

#[derive(Debug)]
pub struct MidiTrack {
    /// these are all guaranteed to be `TrackClip::Midi`
    pub(crate) clips: RwLock<Vec<Arc<TrackClip>>>,
    /// 0 <= volume
    pub volume: Atomic<f32>,
    /// -1 <= pan <= 1
    pub pan: Atomic<f32>,
    /// holds all the state needed for a generator plugin to function properly
    pub(crate) plugin_state: Mutex<PluginState>,
    pub(crate) meter: Arc<Meter>,
}

impl MidiTrack {
    #[must_use]
    pub fn create(plugin: PluginAudioProcessor, meter: Arc<Meter>) -> Arc<Track> {
        Arc::new(Track::Midi(Self {
            clips: RwLock::default(),
            volume: Atomic::new(1.0),
            pan: Atomic::new(0.0),
            plugin_state: PluginState::create(plugin),
            meter,
        }))
    }

    #[must_use]
    pub fn len(&self) -> Position {
        self.clips
            .read()
            .unwrap()
            .iter()
            .map(|clip| clip.get_global_end())
            .max()
            .unwrap_or_else(Position::default)
    }
}
