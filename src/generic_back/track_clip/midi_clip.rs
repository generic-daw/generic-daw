use super::TrackClip;
use crate::generic_back::clap_host::{HostThreadMessage, PluginThreadMessage};
use clack_host::{
    events::{
        event_types::{NoteOffEvent, NoteOnEvent},
        Match,
    },
    prelude::*,
};
use std::sync::{
    atomic::{AtomicU32, AtomicU8, Ordering::SeqCst},
    mpsc::{Receiver, Sender},
    Arc, Mutex,
};
use wmidi::{MidiMessage, Velocity};

#[derive(PartialEq, Eq)]
pub struct MidiNote<'a> {
    pub note: MidiMessage<'a>,
    pub global_start: u32,
    pub global_end: u32,
}

#[derive(Clone)]
enum DirtyEvent<'a> {
    // can we reasonably assume that only one of these will happen per sample?
    None,
    Jump,
    NoteAdded,
    NoteRemoved,
    NoteEndChanged((Arc<MidiNote<'a>>, Arc<MidiNote<'a>>)),
    NoteReplaced,
}

pub struct MidiClip<'a> {
    plugin_sender: Sender<PluginThreadMessage>,
    host_receiver: Mutex<Receiver<HostThreadMessage>>,
    pattern: Vec<Arc<MidiNote<'a>>>,
    started_notes: Mutex<Vec<Arc<MidiNote<'a>>>>,
    last_global_time: AtomicU32,
    running_buffer: Mutex<[f32; 16]>,
    last_buffer_index: AtomicU8,
    buffer_dirty: Mutex<DirtyEvent<'a>>,
    audio_ports: Arc<Mutex<AudioPorts>>,
}

impl<'a> TrackClip for MidiClip<'a> {
    fn get_at_global_time(&self, global_time: u32) -> f32 {
        let last_global_time = self.last_global_time.load(SeqCst);
        let mut last_buffer_index = self.last_buffer_index.load(SeqCst);

        if last_global_time != global_time {
            if global_time != last_global_time + 1 {
                *self.buffer_dirty.lock().unwrap() = DirtyEvent::Jump;
            }

            let buffer_dirty = self.buffer_dirty.lock().unwrap().clone();
            match buffer_dirty {
                DirtyEvent::None => {}
                _ => self.last_buffer_index.store(15, SeqCst),
            }

            last_buffer_index = (last_buffer_index + 1) % 16;
            if last_buffer_index == 0 {
                self.refresh_buffer(global_time);
            }

            self.last_global_time.store(global_time, SeqCst);
            self.last_buffer_index.store(last_buffer_index, SeqCst);
        }

        self.running_buffer.lock().unwrap()[last_buffer_index as usize]
    }

    fn get_global_start(&self) -> u32 {
        self.pattern
            .iter()
            .map(|note| note.global_start)
            .min()
            .unwrap_or(0)
            .to_owned()
    }

    fn get_global_end(&self) -> u32 {
        self.pattern
            .iter()
            .map(|note| note.global_end)
            .max()
            .unwrap_or(0)
            .to_owned()
    }
}

impl<'a> MidiClip<'a> {
    pub fn new(
        plugin_sender: Sender<PluginThreadMessage>,
        plugin_receiver: Receiver<HostThreadMessage>,
    ) -> Self {
        Self {
            plugin_sender,
            host_receiver: Mutex::new(plugin_receiver),
            pattern: Vec::new(),
            started_notes: Mutex::new(Vec::new()),
            last_global_time: AtomicU32::new(0),
            running_buffer: Mutex::new([0.0; 16]),
            last_buffer_index: AtomicU8::new(15),
            buffer_dirty: Mutex::new(DirtyEvent::None),
            audio_ports: Arc::new(Mutex::new(AudioPorts::with_capacity(2, 1))),
        }
    }

    pub fn push(&mut self, note: &Arc<MidiNote<'a>>) {
        self.pattern.push(note.clone());

        let buffer_start =
            self.last_global_time.load(SeqCst) - self.last_buffer_index.load(SeqCst) as u32;
        if note.global_start < buffer_start + 16 && note.global_end >= buffer_start {
            // if the note starts before the buffer ends and ends after the buffer starts
            // i.e. if the note is playing at any point in the buffer
            *self.buffer_dirty.lock().unwrap() = DirtyEvent::NoteAdded;
        }
    }

    pub fn remove(&mut self, note: &Arc<MidiNote<'a>>) {
        let pos = self
            .pattern
            .iter()
            .enumerate()
            .find(|(_, n)| n == &note)
            .map(|(pos, _)| pos)
            .unwrap();
        self.pattern.remove(pos);

        let buffer_start =
            self.last_global_time.load(SeqCst) - self.last_buffer_index.load(SeqCst) as u32;
        if note.global_start < buffer_start + 16 && note.global_end >= buffer_start {
            // if the note starts before the buffer ends and ends after the buffer starts
            // i.e. if the note is playing at any point in the buffer
            *self.buffer_dirty.lock().unwrap() = DirtyEvent::NoteRemoved;
        }
    }

    pub fn replace(&mut self, note: &Arc<MidiNote<'a>>, new_note: &Arc<MidiNote<'a>>) {
        let pos = self.pattern.iter().position(|n| n == note).unwrap();
        self.pattern[pos] = new_note.clone();

        let buffer_start =
            self.last_global_time.load(SeqCst) - self.last_buffer_index.load(SeqCst) as u32;
        if note.note != new_note.note
            || (note.global_start != new_note.global_start
                && (note.global_start >= buffer_start || new_note.global_start >= buffer_start))
        // if the note is different
        // or if the note start changes within the buffer
        {
            *self.buffer_dirty.lock().unwrap() = if note.global_end != new_note.global_end
                && (note.global_end < buffer_start + 16 || new_note.global_end < buffer_start + 16)
            // if the note end changes within the buffer
            {
                DirtyEvent::NoteEndChanged((note.clone(), new_note.clone()))
            } else {
                DirtyEvent::NoteReplaced
            }
        }
    }

    fn refresh_buffer(&self, global_time: u32) {
        let buffer = self.get_input_events(global_time);

        self.plugin_sender
            .send(PluginThreadMessage::ProcessAudio(
                [[0.0; 8]; 2],
                self.audio_ports.clone(),
                self.audio_ports.clone(),
                buffer,
                EventBuffer::new(),
            ))
            .unwrap();

        if let HostThreadMessage::AudioProcessed(buffers, _) =
            self.host_receiver.lock().unwrap().recv().unwrap()
        {
            (0..16).step_by(2).for_each(|i| {
                self.running_buffer.lock().unwrap()[i] = buffers[0][i];
                self.running_buffer.lock().unwrap()[i + 1] = buffers[1][i];
            });

            *self.buffer_dirty.lock().unwrap() = DirtyEvent::None;
        };
    }

    fn get_input_events(&self, global_time: u32) -> EventBuffer {
        let mut buffer = EventBuffer::new();

        self.plugin_sender
            .send(PluginThreadMessage::GetCounter)
            .unwrap();
        if let HostThreadMessage::Counter(plugin_counter) =
            self.host_receiver.lock().unwrap().recv().unwrap()
        {
            let buffer_dirty = self.buffer_dirty.lock().unwrap().clone();
            match buffer_dirty {
                DirtyEvent::None => {}
                DirtyEvent::Jump => {
                    self.jump_refresh(&mut buffer, global_time, plugin_counter);
                }
                DirtyEvent::NoteAdded => {
                    self.note_add_refresh(&mut buffer, global_time, plugin_counter);
                }
                DirtyEvent::NoteRemoved => {
                    self.note_remove_refresh(&mut buffer, global_time, plugin_counter);
                }
                DirtyEvent::NoteEndChanged((note, new_note)) => {
                    self.note_end_changed_refresh(&note, &new_note);
                }
                DirtyEvent::NoteReplaced => {
                    self.note_remove_refresh(&mut buffer, global_time, plugin_counter);
                    self.note_add_refresh(&mut buffer, global_time, plugin_counter);
                }
            }

            self.pattern
                .iter()
                .filter(|midi_note| {
                    midi_note.global_start >= global_time
                        && midi_note.global_start < global_time + 16
                })
                .for_each(|midi_note| {
                    // notes that start during the running buffer
                    if let MidiMessage::NoteOn(channel, note, velocity) = midi_note.note {
                        buffer.push(&NoteOnEvent::new(
                            midi_note.global_start + plugin_counter,
                            Pckn::new(0u8, channel.index(), note as u16, Match::All),
                            u8::from(velocity) as f64 / (u8::from(Velocity::MAX) as f64),
                        ));
                        self.started_notes.lock().unwrap().push(midi_note.clone());
                    };
                });

            let mut indices = Vec::new();

            self.started_notes
                .lock()
                .unwrap()
                .iter()
                .enumerate()
                .filter(|(_, note)| note.global_end < global_time + 16)
                .for_each(|(index, midi_note)| {
                    // notes that end before the running buffer ends
                    if let MidiMessage::NoteOn(channel, note, velocity) = midi_note.note {
                        buffer.push(&NoteOffEvent::new(
                            midi_note.global_end + plugin_counter,
                            Pckn::new(0u8, channel.index(), note as u16, Match::All),
                            u8::from(velocity) as f64 / (u8::from(Velocity::MAX) as f64),
                        ));
                        indices.push(index);
                    };
                });

            indices.iter().rev().for_each(|i| {
                self.started_notes.lock().unwrap().remove(*i);
            });
        }

        buffer
    }

    fn jump_refresh(&self, buffer: &mut EventBuffer, global_time: u32, plugin_counter: u32) {
        self.started_notes.lock().unwrap().iter().for_each(|note| {
            // stop all started notes
            if let MidiMessage::NoteOn(channel, note, velocity) = note.note {
                buffer.push(&NoteOffEvent::new(
                    global_time + plugin_counter,
                    Pckn::new(0u8, channel.index(), note as u16, Match::All),
                    u8::from(velocity) as f64 / (u8::from(Velocity::MAX) as f64),
                ));
            };
        });

        self.started_notes.lock().unwrap().clear();

        self.pattern
            .iter()
            .filter(|note| note.global_start < global_time && note.global_end > global_time)
            .for_each(|note| {
                // start all notes that start before the running buffer starts and end after
                if let MidiMessage::NoteOn(channel, note, velocity) = note.note {
                    buffer.push(&NoteOnEvent::new(
                        global_time + plugin_counter,
                        Pckn::new(0u8, channel.index(), note as u16, Match::All),
                        u8::from(velocity) as f64 / (u8::from(Velocity::MAX) as f64),
                    ));
                };
                self.started_notes.lock().unwrap().push(note.clone());
            });
    }

    fn note_add_refresh(&self, buffer: &mut EventBuffer, global_time: u32, plugin_counter: u32) {
        self.pattern
            .iter()
            .filter(|note| !self.started_notes.lock().unwrap().contains(note))
            .for_each(|note| {
                // start all new notes
                if let MidiMessage::NoteOn(channel, note, velocity) = note.note {
                    buffer.push(&NoteOnEvent::new(
                        global_time + plugin_counter,
                        Pckn::new(0u8, channel.index(), note as u16, Match::All),
                        u8::from(velocity) as f64 / (u8::from(Velocity::MAX) as f64),
                    ));
                };
                self.started_notes.lock().unwrap().push(note.clone());
            });
    }

    fn note_remove_refresh(&self, buffer: &mut EventBuffer, global_time: u32, plugin_counter: u32) {
        let mut indices = Vec::new();

        self.started_notes
            .lock()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, note)| !self.pattern.contains(note))
            .for_each(|(index, note)| {
                // stop all started notes that are no longer in the pattern
                if let MidiMessage::NoteOn(channel, note, velocity) = note.note {
                    buffer.push(&NoteOffEvent::new(
                        global_time + plugin_counter,
                        Pckn::new(0u8, channel.index(), note as u16, Match::All),
                        u8::from(velocity) as f64 / (u8::from(Velocity::MAX) as f64),
                    ));
                    indices.push(index);
                };
            });

        indices.iter().rev().for_each(|i| {
            self.started_notes.lock().unwrap().remove(*i);
        });
    }

    fn note_end_changed_refresh(&self, note: &Arc<MidiNote<'a>>, new_note: &Arc<MidiNote<'a>>) {
        let index = self
            .started_notes
            .lock()
            .unwrap()
            .iter()
            .position(|n| n == note)
            .unwrap();

        self.started_notes.lock().unwrap().remove(index);
        self.started_notes.lock().unwrap().push(new_note.clone());
    }
}
