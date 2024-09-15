use crate::generic_back::track_clip::midi_clip::{
    dirty_event::{AtomicDirtyEvent, DirtyEvent},
    midi_note::MidiNote,
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
    atomic::Ordering::SeqCst,
    mpsc::{Receiver, Sender},
    Arc, Mutex, RwLock,
};

pub(in crate::generic_back) struct PluginState {
    /// send messages to the plugin
    pub(super) plugin_sender: Sender<MainThreadMessage>,
    /// receive messages from the plugin
    pub(super) host_receiver: Arc<Mutex<Receiver<HostThreadMessage>>>,
    /// the combined midi of all clips in the track
    pub(super) global_midi_cache: Vec<Arc<MidiNote>>,
    /// how the midi was modified since the last buffer refresh
    pub(in crate::generic_back) dirty: Arc<AtomicDirtyEvent>,
    /// all currently playing notes
    pub(super) started_notes: Vec<Arc<MidiNote>>,
    /// the last global time that was fetched.
    ///
    /// use this to determine whether the playhead jumped
    pub(super) last_global_time: u32,
    /// a buffer of samples generated by the plugin
    ///
    /// this is refreshed when `dirty` is marked with an event
    pub(super) running_buffer: [f32; 16],
    /// the last index in the buffer that was accessed
    pub(super) last_buffer_index: u32,
}

impl PluginState {
    pub fn create(
        plugin_sender: Sender<MainThreadMessage>,
        host_receiver: Arc<Mutex<Receiver<HostThreadMessage>>>,
    ) -> RwLock<Self> {
        RwLock::new(Self {
            plugin_sender,
            host_receiver,
            global_midi_cache: Vec::new(),
            dirty: Arc::new(AtomicDirtyEvent::new(DirtyEvent::None)),
            started_notes: Vec::new(),
            last_global_time: 0,
            running_buffer: [0.0; 16],
            last_buffer_index: 15,
        })
    }

    pub fn refresh_buffer(&mut self, global_time: u32) {
        let buffer = self.get_input_events(global_time);
        let input_audio = [vec![0.0; 8], vec![0.0; 8]];
        let input_ports = AudioPorts::with_capacity(2, 1);
        let output_ports = AudioPorts::with_capacity(2, 1);

        self.plugin_sender
            .send(MainThreadMessage::ProcessAudio(
                input_audio,
                input_ports,
                output_ports,
                buffer,
            ))
            .unwrap();

        let message = self.host_receiver.lock().unwrap().recv().unwrap();
        if let HostThreadMessage::AudioProcessed(buffers, _) = message {
            (0..16).step_by(2).for_each(|i| {
                self.running_buffer[i] = buffers[0][i];
                self.running_buffer[i + 1] = buffers[1][i];
            });

            self.dirty.store(DirtyEvent::None, SeqCst);
        };
    }

    fn get_input_events(&mut self, global_time: u32) -> EventBuffer {
        let mut buffer = EventBuffer::new();

        self.plugin_sender
            .send(MainThreadMessage::GetCounter)
            .unwrap();

        let message = self.host_receiver.lock().unwrap().recv().unwrap();
        if let HostThreadMessage::Counter(steady_time) = message {
            let steady_time = u32::try_from(steady_time).unwrap();
            let dirty = self.dirty.load(SeqCst);
            match dirty {
                DirtyEvent::None => {
                    if global_time != self.last_global_time + 1 {
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

    fn events_refresh(&mut self, buffer: &mut EventBuffer, global_time: u32, steady_time: u32) {
        self.global_midi_cache
            .iter()
            .filter(|note| note.local_start >= global_time && note.local_start < global_time + 16)
            .for_each(|note| {
                // notes that start during the running buffer
                buffer.push(&NoteOnEvent::new(
                    note.local_start - global_time + steady_time,
                    Pckn::new(0u8, note.channel, note.note, Match::All),
                    note.velocity,
                ));
                self.started_notes.push(note.clone());
            });

        let mut indices = Vec::new();

        self.started_notes
            .iter()
            .enumerate()
            .filter(|(_, note)| note.local_end >= global_time && note.local_end < global_time + 16)
            .for_each(|(index, note)| {
                // notes that end before the running buffer ends
                buffer.push(&NoteOffEvent::new(
                    note.local_end - global_time + steady_time,
                    Pckn::new(0u8, note.channel, note.note, Match::All),
                    note.velocity,
                ));
                indices.push(index);
            });

        indices.iter().rev().for_each(|i| {
            self.started_notes.remove(*i);
        });
    }

    fn jump_events_refresh(
        &mut self,
        buffer: &mut EventBuffer,
        global_time: u32,
        steady_time: u32,
    ) {
        self.started_notes.iter().for_each(|note| {
            // stop all started notes
            buffer.push(&NoteOffEvent::new(
                steady_time,
                Pckn::new(0u8, note.channel, note.note, Match::All),
                note.velocity,
            ));
        });

        self.started_notes.clear();

        self.global_midi_cache
            .iter()
            .filter(|note| note.local_start <= global_time && note.local_end > global_time)
            .for_each(|note| {
                // start all notes that would be currently playing
                buffer.push(&NoteOnEvent::new(
                    steady_time,
                    Pckn::new(0u8, note.channel, note.note, Match::All),
                    note.velocity,
                ));
                self.started_notes.push(note.clone());
            });
    }

    fn note_add_events_refresh(
        &mut self,
        buffer: &mut EventBuffer,
        global_time: u32,
        steady_time: u32,
    ) {
        let new_notes: Vec<_> = self
            .global_midi_cache
            .iter()
            .filter(|note| !self.started_notes.contains(note))
            .filter(|note| note.local_start <= global_time && note.local_end > global_time)
            .collect();

        for note in new_notes {
            // start all new notes that would be currently playing
            buffer.push(&NoteOnEvent::new(
                steady_time,
                Pckn::new(0u8, note.channel, note.note, Match::All),
                note.velocity,
            ));
            self.started_notes.push(note.clone());
        }
    }

    fn note_remove_events_refresh(&mut self, buffer: &mut EventBuffer, steady_time: u32) {
        let mut indices = Vec::new();

        self.started_notes
            .iter()
            .enumerate()
            .filter(|(_, note)| !self.global_midi_cache.contains(note))
            .for_each(|(index, note)| {
                // stop all started notes that are no longer in the pattern
                buffer.push(&NoteOffEvent::new(
                    steady_time,
                    Pckn::new(0u8, note.channel, note.note, Match::All),
                    note.velocity,
                ));
                indices.push(index);
            });

        indices.iter().rev().for_each(|i| {
            self.started_notes.remove(*i);
        });
    }
}
