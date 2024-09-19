mod plugin_state;

use crate::generic_back::{
    pan,
    position::Position,
    track_clip::midi_clip::{dirty_event::DirtyEvent, MidiClip},
};
use generic_clap_host::{host::HostThreadMessage, main_thread::MainThreadMessage};
use plugin_state::PluginState;
use std::sync::{
    atomic::Ordering::SeqCst,
    mpsc::{Receiver, Sender},
    Arc, Mutex, RwLock,
};

use super::Track;

pub struct MidiTrack {
    pub clips: Vec<MidiClip>,
    /// between 0.0 and 1.0
    volume: f32,
    /// between -1.0 (left) and 1.0 (right)
    pan: f32,
    /// holds all the state needed for a generator plugin to function properly
    pub(in crate::generic_back) plugin_state: RwLock<PluginState>,
}

impl MidiTrack {
    pub fn create(
        plugin_sender: Sender<MainThreadMessage>,
        host_receiver: Arc<Mutex<Receiver<HostThreadMessage>>>,
    ) -> Track {
        Track::Midi(RwLock::new(Self {
            clips: Vec::new(),
            volume: 1.0,
            pan: 0.0,
            plugin_state: PluginState::create(plugin_sender, host_receiver),
        }))
    }

    fn refresh_global_midi(&self) {
        self.plugin_state.write().unwrap().global_midi_cache = self
            .clips
            .iter()
            .flat_map(MidiClip::get_global_midi)
            .collect();
    }

    pub(super) fn get_at_global_time(&self, global_time: u32) -> f32 {
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
            * self.volume
            * pan(self.pan, global_time)
    }

    pub(super) fn get_global_end(&self) -> Position {
        self.clips
            .iter()
            .map(MidiClip::get_global_end)
            .max()
            .unwrap_or(Position::new(0, 0))
    }

    pub(super) const fn get_volume(&self) -> f32 {
        self.volume
    }

    pub(super) fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }

    pub(super) const fn get_pan(&self) -> f32 {
        self.pan
    }

    pub(super) fn set_pan(&mut self, pan: f32) {
        self.pan = pan;
    }
}
