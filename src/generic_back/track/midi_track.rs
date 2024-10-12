use crate::{
    generic_back::{pan, Meter, Position, Track, TrackClip},
    helpers::AtomicF32,
};
use generic_clap_host::{HostThreadMessage, MainThreadMessage};
use plugin_state::PluginState;
use std::sync::{
    atomic::Ordering::SeqCst,
    mpsc::{Receiver, Sender},
    Arc, Mutex, RwLock,
};

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
    pub plugin_state: RwLock<PluginState>,
    pub meter: Arc<Meter>,
}

impl MidiTrack {
    pub fn create(
        plugin_sender: Sender<MainThreadMessage>,
        host_receiver: Arc<Mutex<Receiver<HostThreadMessage>>>,
        meter: Arc<Meter>,
    ) -> Track {
        Track::Midi(Self {
            clips: Arc::new(RwLock::default()),
            volume: AtomicF32::new(1.0),
            pan: AtomicF32::new(0.0),
            plugin_state: PluginState::create(plugin_sender, host_receiver),
            meter,
        })
    }

    fn refresh_global_midi(&self) {
        self.plugin_state.write().unwrap().global_midi_cache = self
            .clips
            .read()
            .unwrap()
            .iter()
            .flat_map(|clip| clip.get_global_midi())
            .collect();
    }

    pub fn get_at_global_time(&self, global_time: u32) -> f32 {
        let last_global_time = self.plugin_state.read().unwrap().last_global_time;
        let mut last_buffer_index = self.plugin_state.read().unwrap().last_buffer_index;

        if last_global_time != global_time {
            if global_time != last_global_time + 1
                || self.plugin_state.read().unwrap().dirty.load(SeqCst) != DirtyEvent::None
            {
                self.refresh_global_midi();
                self.plugin_state.write().unwrap().last_buffer_index = BUFFER_SIZE - 1;
            }

            last_buffer_index = (last_buffer_index + 1) % BUFFER_SIZE;
            if last_buffer_index == 0 {
                self.plugin_state
                    .write()
                    .unwrap()
                    .refresh_buffer(global_time);
            }

            self.plugin_state.write().unwrap().last_global_time = global_time;
            self.plugin_state.write().unwrap().last_buffer_index = last_buffer_index;
        }

        self.plugin_state.read().unwrap().running_buffer[last_buffer_index]
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
