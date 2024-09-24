mod plugin_state;

use crate::{
    generic_back::{pan, DirtyEvent, MidiClip, Position, Track},
    helpers::AtomicF32,
};
use generic_clap_host::{host::HostThreadMessage, main_thread::MainThreadMessage};
use plugin_state::PluginState;
use std::sync::{
    atomic::Ordering::SeqCst,
    mpsc::{Receiver, Sender},
    Arc, Mutex, RwLock,
};

#[derive(Debug)]
pub struct MidiTrack {
    pub clips: RwLock<Vec<MidiClip>>,
    /// between 0.0 and 1.0
    pub volume: AtomicF32,
    /// between -1.0 (left) and 1.0 (right)
    pub pan: AtomicF32,
    /// holds all the state needed for a generator plugin to function properly
    pub plugin_state: RwLock<PluginState>,
}

impl MidiTrack {
    pub fn create(
        plugin_sender: Sender<MainThreadMessage>,
        host_receiver: Arc<Mutex<Receiver<HostThreadMessage>>>,
    ) -> Track {
        Track::Midi(Self {
            clips: RwLock::new(Vec::new()),
            volume: AtomicF32::new(1.0),
            pan: AtomicF32::new(0.0),
            plugin_state: PluginState::create(plugin_sender, host_receiver),
        })
    }

    fn refresh_global_midi(&self) {
        self.plugin_state.write().unwrap().global_midi_cache = self
            .clips
            .read()
            .unwrap()
            .iter()
            .flat_map(MidiClip::get_global_midi)
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
                self.plugin_state.write().unwrap().last_buffer_index = 15;
            }

            last_buffer_index = (last_buffer_index + 1) % 16;
            if last_buffer_index == 0 {
                self.plugin_state
                    .write()
                    .unwrap()
                    .refresh_buffer(global_time);
            }

            self.plugin_state.write().unwrap().last_global_time = global_time;
            self.plugin_state.write().unwrap().last_buffer_index = last_buffer_index;
        }

        self.plugin_state.read().unwrap().running_buffer
            [usize::try_from(last_buffer_index).unwrap()]
            * self.volume.load(SeqCst)
            * pan(self.pan.load(SeqCst), global_time)
    }

    pub fn get_global_end(&self) -> Position {
        self.clips
            .read()
            .unwrap()
            .iter()
            .map(MidiClip::get_global_end)
            .max()
            .unwrap_or(Position::new(0, 0))
    }
}
