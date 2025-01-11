use crate::{
    clap_host::ClapPluginWrapper,
    generic_back::{AtomicDirtyEvent, DirtyEvent, MidiNote},
};
use clack_host::{
    events::{
        event_types::{NoteOffEvent, NoteOnEvent},
        Match, Pckn,
    },
    prelude::{AudioPorts, EventBuffer},
};
use std::sync::{atomic::Ordering::SeqCst, Arc, Mutex};

pub const BUFFER_SIZE: usize = 256;

#[derive(Debug)]
pub struct PluginState {
    /// send messages to the plugin
    ///
    /// TODO: don't use the giga UB wrapper type here
    pub plugin: ClapPluginWrapper,
    /// the combined midi of all clips in the track
    pub global_midi_cache: Vec<Arc<MidiNote>>,
    /// how the midi was modified since the last buffer refresh
    pub dirty: Arc<AtomicDirtyEvent>,
    /// all currently playing notes
    pub started_notes: Vec<Arc<MidiNote>>,
    /// the last global time that was fetched.
    ///
    /// use this to determine whether the playhead jumped
    pub last_global_time: usize,
    /// a buffer of samples generated by the plugin
    ///
    /// this is refreshed when trying to fetch from it, if `dirty` is marked with an event
    pub running_buffer: [f32; BUFFER_SIZE],
    /// the last index in the buffer that was accessed
    pub last_buffer_index: usize,
}

impl PluginState {
    pub fn create(plugin: ClapPluginWrapper) -> Mutex<Self> {
        Mutex::new(Self {
            plugin,
            global_midi_cache: Vec::new(),
            dirty: Arc::new(AtomicDirtyEvent::new(DirtyEvent::None)),
            started_notes: Vec::new(),
            last_global_time: 0,
            running_buffer: [0.0; BUFFER_SIZE],
            last_buffer_index: BUFFER_SIZE - 1,
        })
    }

    pub fn refresh_buffer(&mut self, global_time: usize) {
        let buffer = self.get_input_events(global_time);
        let input_audio = vec![vec![0.0; BUFFER_SIZE], vec![0.0; BUFFER_SIZE]];
        let input_ports = AudioPorts::with_capacity(0, 0);
        let output_ports = AudioPorts::with_capacity(2, 1);

        let (buffers, _) =
            self.plugin
                .inner()
                .process_audio(input_audio, input_ports, output_ports, buffer);
        (0..BUFFER_SIZE).for_each(|i| {
            let i = i * 2;
            self.running_buffer[i] = buffers[0][i];
            self.running_buffer[i + 1] = buffers[1][i];
        });

        self.dirty.store(DirtyEvent::None, SeqCst);
    }

    fn get_input_events(&mut self, global_time: usize) -> EventBuffer {
        let mut buffer = EventBuffer::new();

        let steady_time = self.plugin.inner().get_counter() as usize;

        match self.dirty.load(SeqCst) {
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

        buffer
    }

    fn events_refresh(&mut self, buffer: &mut EventBuffer, global_time: usize, steady_time: usize) {
        self.global_midi_cache
            .iter()
            .filter(|note| {
                note.local_start >= global_time && note.local_start < global_time + BUFFER_SIZE * 2
            })
            .for_each(|note| {
                // notes that start during the running buffer
                let time = note.local_start - global_time + steady_time;
                buffer.push(&NoteOnEvent::new(
                    time as u32,
                    Pckn::new(0u8, note.channel, note.note, Match::All),
                    note.velocity,
                ));
                self.started_notes.push(note.clone());
            });

        let mut indices = Vec::new();

        self.started_notes
            .iter()
            .enumerate()
            .filter(|(_, note)| {
                note.local_end >= global_time && note.local_end < global_time + BUFFER_SIZE * 2
            })
            .for_each(|(index, note)| {
                // notes that end before the running buffer ends
                let time = note.local_end - global_time + steady_time;
                buffer.push(&NoteOffEvent::new(
                    time as u32,
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
        global_time: usize,
        steady_time: usize,
    ) {
        self.started_notes.iter().for_each(|note| {
            // stop all started notes
            buffer.push(&NoteOffEvent::new(
                steady_time as u32,
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
                    steady_time as u32,
                    Pckn::new(0u8, note.channel, note.note, Match::All),
                    note.velocity,
                ));
                self.started_notes.push(note.clone());
            });
    }

    fn note_add_events_refresh(
        &mut self,
        buffer: &mut EventBuffer,
        global_time: usize,
        steady_time: usize,
    ) {
        let new_notes = self
            .global_midi_cache
            .iter()
            .filter(|note| !self.started_notes.contains(note))
            .filter(|note| note.local_start <= global_time && note.local_end > global_time)
            .collect::<Box<_>>();

        for note in new_notes {
            // start all new notes that would be currently playing
            buffer.push(&NoteOnEvent::new(
                steady_time as u32,
                Pckn::new(0u8, note.channel, note.note, Match::All),
                note.velocity,
            ));
            self.started_notes.push(note.clone());
        }
    }

    fn note_remove_events_refresh(&mut self, buffer: &mut EventBuffer, steady_time: usize) {
        let mut indices = Vec::new();

        self.started_notes
            .iter()
            .enumerate()
            .filter(|(_, note)| !self.global_midi_cache.contains(note))
            .for_each(|(index, note)| {
                // stop all started notes that are no longer in the pattern
                buffer.push(&NoteOffEvent::new(
                    steady_time as u32,
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
