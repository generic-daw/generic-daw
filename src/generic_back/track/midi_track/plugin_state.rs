use crate::generic_back::track_clip::midi_clip::midi_pattern::{
    AtomicDirtyEvent, DirtyEvent, MidiNote,
};
use clack_host::{
    events::{
        event_types::{NoteOffEvent, NoteOnEvent},
        Match, Pckn,
    },
    prelude::{AudioPorts, EventBuffer},
};
use generic_clap_host::{host::HostThreadMessage, main_thread::MainThreadMessage};
use std::sync::{
    atomic::{AtomicU32, AtomicU8, Ordering::SeqCst},
    mpsc::{Receiver, Sender},
    Arc, Mutex, RwLock,
};

pub struct PluginState {
    pub plugin_sender: Sender<MainThreadMessage>,
    pub host_receiver: Mutex<Receiver<HostThreadMessage>>,
    pub global_midi_cache: RwLock<Vec<Arc<MidiNote>>>,
    pub dirty: Arc<AtomicDirtyEvent>,
    pub started_notes: RwLock<Vec<Arc<MidiNote>>>,
    pub last_global_time: AtomicU32,
    pub running_buffer: RwLock<[f32; 16]>,
    pub last_buffer_index: AtomicU8,
    pub audio_ports: Arc<RwLock<AudioPorts>>,
}

impl PluginState {
    pub fn refresh_buffer(&self, global_time: u32) {
        let buffer = self.get_input_events(global_time);

        let input_audio = [vec![0.0; 8], vec![0.0; 8]];

        self.plugin_sender
            .send(MainThreadMessage::ProcessAudio(
                input_audio,
                self.audio_ports.clone(),
                self.audio_ports.clone(),
                buffer,
            ))
            .unwrap();

        let message = self.host_receiver.lock().unwrap().recv().unwrap();
        if let HostThreadMessage::AudioProcessed(buffers, _) = message {
            (0..16).step_by(2).for_each(|i| {
                self.running_buffer.write().unwrap()[i] = buffers[0][i];
                self.running_buffer.write().unwrap()[i + 1] = buffers[1][i];
            });

            self.dirty.store(DirtyEvent::None, SeqCst);
        };
    }

    fn get_input_events(&self, global_time: u32) -> EventBuffer {
        let mut buffer = EventBuffer::new();

        self.plugin_sender
            .send(MainThreadMessage::GetCounter)
            .unwrap();

        let message = self.host_receiver.lock().unwrap().recv().unwrap();
        if let HostThreadMessage::Counter(steady_time) = message {
            let dirty = self.dirty.load(SeqCst);
            match dirty {
                DirtyEvent::None => {
                    let last_global_time = self.last_global_time.load(SeqCst);
                    if global_time != last_global_time + 1 {
                        self.jump_events_refresh(&mut buffer, global_time, steady_time);
                    }
                }
                DirtyEvent::NoteAdded => {
                    self.note_add_events_refresh(&mut buffer, global_time, steady_time);
                }
                DirtyEvent::NoteRemoved => {
                    self.note_remove_events_refresh(&mut buffer, steady_time);
                }
                DirtyEvent::NoteReplaced => {
                    self.note_remove_events_refresh(&mut buffer, steady_time);
                    self.note_add_events_refresh(&mut buffer, global_time, steady_time);
                }
            }

            self.events_refresh(&mut buffer, global_time, steady_time);
        }

        buffer
    }

    fn events_refresh(&self, buffer: &mut EventBuffer, global_time: u32, steady_time: u64) {
        self.global_midi_cache
            .read()
            .unwrap()
            .iter()
            .filter(|note| note.local_start >= global_time && note.local_start < global_time + 16)
            .for_each(|note| {
                // notes that start during the running buffer
                buffer.push(&NoteOnEvent::new(
                    note.local_start - global_time + u32::try_from(steady_time).unwrap(),
                    Pckn::new(0u8, note.channel, note.note, Match::All),
                    note.velocity,
                ));
                self.started_notes.write().unwrap().push(note.clone());
            });

        let mut indices = Vec::new();

        self.started_notes
            .read()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, note)| note.local_end >= global_time && note.local_end < global_time + 16)
            .for_each(|(index, note)| {
                // notes that end before the running buffer ends
                buffer.push(&NoteOffEvent::new(
                    note.local_end - global_time + u32::try_from(steady_time).unwrap(),
                    Pckn::new(0u8, note.channel, note.note, Match::All),
                    note.velocity,
                ));
                indices.push(index);
            });

        indices.iter().rev().for_each(|i| {
            self.started_notes.write().unwrap().remove(*i);
        });
    }

    fn jump_events_refresh(&self, buffer: &mut EventBuffer, global_time: u32, steady_time: u64) {
        self.started_notes.read().unwrap().iter().for_each(|note| {
            // stop all started notes
            buffer.push(&NoteOffEvent::new(
                u32::try_from(steady_time).unwrap(),
                Pckn::new(0u8, note.channel, note.note, Match::All),
                note.velocity,
            ));
        });

        self.started_notes.write().unwrap().clear();

        self.global_midi_cache
            .read()
            .unwrap()
            .iter()
            .filter(|note| note.local_start <= global_time && note.local_end > global_time)
            .for_each(|note| {
                // start all notes that would be currently playing
                buffer.push(&NoteOnEvent::new(
                    u32::try_from(steady_time).unwrap(),
                    Pckn::new(0u8, note.channel, note.note, Match::All),
                    note.velocity,
                ));
                self.started_notes.write().unwrap().push(note.clone());
            });
    }

    fn note_add_events_refresh(
        &self,
        buffer: &mut EventBuffer,
        global_time: u32,
        steady_time: u64,
    ) {
        self.global_midi_cache
            .read()
            .unwrap()
            .iter()
            .filter(|note| !self.started_notes.read().unwrap().contains(note))
            .filter(|note| note.local_start <= global_time && note.local_end > global_time)
            .for_each(|note| {
                // start all new notes that would be currently playing
                buffer.push(&NoteOnEvent::new(
                    u32::try_from(steady_time).unwrap(),
                    Pckn::new(0u8, note.channel, note.note, Match::All),
                    note.velocity,
                ));
                self.started_notes.write().unwrap().push(note.clone());
            });
    }

    fn note_remove_events_refresh(&self, buffer: &mut EventBuffer, steady_time: u64) {
        let mut indices = Vec::new();

        self.started_notes
            .read()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, note)| !self.global_midi_cache.read().unwrap().contains(note))
            .for_each(|(index, note)| {
                // stop all started notes that are no longer in the pattern
                buffer.push(&NoteOffEvent::new(
                    u32::try_from(steady_time).unwrap(),
                    Pckn::new(0u8, note.channel, note.note, Match::All),
                    note.velocity,
                ));
                indices.push(index);
            });

        indices.iter().rev().for_each(|i| {
            self.started_notes.write().unwrap().remove(*i);
        });
    }
}
