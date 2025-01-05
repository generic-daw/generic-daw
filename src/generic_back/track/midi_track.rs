use crate::generic_back::{pan, Meter, Position, Track, TrackClip};
use generic_clap_host::ClapPlugin;
use plugin_state::PluginState;
use portable_atomic::AtomicF32;
use std::sync::{atomic::Ordering::SeqCst, Arc, Mutex, RwLock};

mod dirty_event;
pub use dirty_event::{AtomicDirtyEvent, DirtyEvent};

mod plugin_state;
pub use plugin_state::BUFFER_SIZE;

#[derive(Debug)]
pub struct MidiTrack {
    /// these are all guaranteed to be `TrackClip::Midi`
    pub clips: Arc<RwLock<Vec<Arc<TrackClip>>>>,
    /// between 0.0 and 1.0
    pub volume: AtomicF32,
    /// between -1.0 (left) and 1.0 (right)
    pub pan: AtomicF32,
    /// holds all the state needed for a generator plugin to function properly
    pub plugin_state: Mutex<PluginState>,
    pub meter: Arc<Meter>,
}

impl MidiTrack {
    pub fn create(plugin: ClapPlugin, meter: Arc<Meter>) -> Arc<Track> {
        Arc::new(Track::Midi(Self {
            clips: Arc::new(RwLock::default()),
            volume: AtomicF32::new(1.0),
            pan: AtomicF32::new(0.0),
            plugin_state: PluginState::create(plugin),
            meter,
        }))
    }

    fn refresh_global_midi(&self) {
        self.plugin_state.lock().unwrap().global_midi_cache = self
            .clips
            .read()
            .unwrap()
            .iter()
            .flat_map(|clip| clip.get_global_midi())
            .collect();
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        let last_global_time = self.plugin_state.lock().unwrap().last_global_time;
        let mut last_buffer_index = self.plugin_state.lock().unwrap().last_buffer_index;

        if last_global_time != global_time {
            let dirty = self.plugin_state.lock().unwrap().dirty.load(SeqCst);
            if global_time != last_global_time + 1 || !matches!(dirty, DirtyEvent::None) {
                self.refresh_global_midi();
                self.plugin_state.lock().unwrap().last_buffer_index = BUFFER_SIZE - 1;
            }

            last_buffer_index = (last_buffer_index + 1) % BUFFER_SIZE;
            if last_buffer_index == 0 {
                self.plugin_state
                    .lock()
                    .unwrap()
                    .refresh_buffer(global_time);
            }

            self.plugin_state.lock().unwrap().last_global_time = global_time;
            self.plugin_state.lock().unwrap().last_buffer_index = last_buffer_index;
        }

        self.plugin_state.lock().unwrap().running_buffer[last_buffer_index]
            * self.volume.load(SeqCst)
            * pan(self.pan.load(SeqCst), global_time)
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
