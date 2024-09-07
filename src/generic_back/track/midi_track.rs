mod plugin_state;

use super::Track;
use crate::{
    generic_back::{
        meter::Meter,
        position::Position,
        track_clip::midi_clip::{
            midi_pattern::{AtomicDirtyEvent, DirtyEvent},
            MidiClip,
        },
    },
    generic_front::drawable::{Drawable, TimelinePosition, TimelineScale},
};
use clack_host::prelude::*;
use generic_clap_host::{host::HostThreadMessage, main_thread::MainThreadMessage};
use iced::{widget::canvas::Frame, Theme};
use plugin_state::PluginState;
use std::sync::{
    atomic::{AtomicU32, AtomicU8, Ordering::SeqCst},
    mpsc::{Receiver, Sender},
    Arc, Mutex, RwLock,
};

pub struct MidiTrack {
    pub clips: RwLock<Vec<MidiClip>>,
    volume: f32,
    plugin_state: PluginState,
}

impl MidiTrack {
    pub fn new(
        plugin_sender: Sender<MainThreadMessage>,
        host_receiver: Receiver<HostThreadMessage>,
    ) -> Self {
        Self {
            clips: RwLock::new(Vec::new()),
            volume: 1.0,
            plugin_state: PluginState {
                plugin_sender,
                host_receiver: Mutex::new(host_receiver),
                global_midi_cache: RwLock::new(Vec::new()),
                dirty: Arc::new(AtomicDirtyEvent::new(DirtyEvent::None)),
                started_notes: RwLock::new(Vec::new()),
                last_global_time: AtomicU32::new(0),
                running_buffer: RwLock::new([0.0; 16]),
                last_buffer_index: AtomicU8::new(15),
                audio_ports: Arc::new(RwLock::new(AudioPorts::with_capacity(2, 1))),
            },
        }
    }

    fn refresh_global_midi(&self, meter: &Meter) {
        *self.plugin_state.global_midi_cache.write().unwrap() = self
            .clips
            .read()
            .unwrap()
            .iter()
            .flat_map(|clip| clip.get_global_midi(meter))
            .collect();
    }
}

impl Track for MidiTrack {
    fn get_at_global_time(&self, global_time: u32, meter: &Meter) -> f32 {
        let last_global_time = self.plugin_state.last_global_time.load(SeqCst);
        let mut last_buffer_index = self.plugin_state.last_buffer_index.load(SeqCst);

        if last_global_time != global_time {
            if global_time != last_global_time + 1
                || self.plugin_state.dirty.load(SeqCst) != DirtyEvent::None
            {
                self.refresh_global_midi(meter);
                self.plugin_state.last_buffer_index.store(15, SeqCst);
            }

            last_buffer_index = (last_buffer_index + 1) % 16;
            if last_buffer_index == 0 {
                self.plugin_state.refresh_buffer(global_time);
            }

            self.plugin_state
                .last_global_time
                .store(global_time, SeqCst);
            self.plugin_state
                .last_buffer_index
                .store(last_buffer_index, SeqCst);
        }

        self.plugin_state.running_buffer.read().unwrap()[usize::from(last_buffer_index)]
            * self.volume
    }

    fn get_global_end(&self) -> Position {
        self.clips
            .read()
            .unwrap()
            .iter()
            .map(MidiClip::get_global_end)
            .max()
            .unwrap_or(Position::new(0, 0))
    }

    fn get_volume(&self) -> f32 {
        self.volume
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }
}

impl Drawable for MidiTrack {
    fn draw(
        &self,
        frame: &mut Frame,
        scale: TimelineScale,
        position: &TimelinePosition,
        meter: &Meter,
        theme: &Theme,
    ) {
        let path = iced::widget::canvas::Path::new(|path| {
            let y = (position.y + 1.0) * scale.y;
            path.line_to(iced::Point::new(0.0, y));
            path.line_to(iced::Point::new(frame.width(), y));
        });
        frame.stroke(
            &path,
            iced::widget::canvas::Stroke::default()
                .with_color(theme.extended_palette().secondary.base.color),
        );

        self.clips.read().unwrap().iter().for_each(|track| {
            track.draw(frame, scale, position, meter, theme);
        });
    }
}
