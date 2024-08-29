use super::TrackClip;
use crate::generic_back::clap_host::{HostThreadMessage, PluginThreadMessage};
use clack_host::{
    events::{
        event_types::{NoteOffEvent, NoteOnEvent},
        Match,
    },
    prelude::*,
};
use std::{
    cmp::min,
    sync::{
        atomic::{AtomicU32, AtomicU8, Ordering::SeqCst},
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
};
use wmidi::{MidiMessage, Velocity};

#[derive(PartialEq, Eq)]
pub struct MidiNote<'a> {
    pub note: MidiMessage<'a>,
    pub local_start: u32,
    pub local_end: u32,
}

#[derive(Clone, PartialEq, Eq)]
enum DirtyEvent {
    // can we reasonably assume that only one of these will happen per sample?
    None,
    NoteAdded,
    NoteRemoved,
    NoteReplaced,
}

pub struct MidiPattern<'a> {
    notes: Vec<Arc<MidiNote<'a>>>,
    dirty: DirtyEvent,
    plugin_sender: Sender<PluginThreadMessage>,
    host_receiver: Mutex<Receiver<HostThreadMessage>>,
}

impl<'a> MidiPattern<'a> {
    fn new(
        plugin_sender: Sender<PluginThreadMessage>,
        host_receiver: Receiver<HostThreadMessage>,
    ) -> Self {
        Self {
            notes: Vec::new(),
            dirty: DirtyEvent::None,
            plugin_sender,
            host_receiver: Mutex::new(host_receiver),
        }
    }

    fn len(&self) -> u32 {
        self.notes
            .iter()
            .map(|note| note.local_end)
            .max()
            .unwrap_or(0)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn clear_dirty(&mut self) {
        self.dirty = DirtyEvent::None;
    }

    fn push(&mut self, note: &Arc<MidiNote<'a>>) {
        self.notes.push(note.clone());
        self.dirty = DirtyEvent::NoteAdded;
    }

    fn remove(&mut self, note: &Arc<MidiNote<'a>>) {
        let pos = self
            .notes
            .iter()
            .enumerate()
            .find(|(_, n)| n == &note)
            .map(|(pos, _)| pos)
            .unwrap();
        self.notes.remove(pos);
        self.dirty = DirtyEvent::NoteRemoved;
    }

    fn replace(&mut self, note: &Arc<MidiNote<'a>>, new_note: &Arc<MidiNote<'a>>) {
        let pos = self.notes.iter().position(|n| n == note).unwrap();
        self.notes[pos] = new_note.clone();
        self.dirty = DirtyEvent::NoteReplaced;
    }
}

pub struct MidiClip<'a> {
    pattern: Arc<Mutex<MidiPattern<'a>>>,
    global_start: u32,
    global_end: u32,
    pattern_start: u32,
    started_notes: Mutex<Vec<Arc<MidiNote<'a>>>>,
    last_global_time: AtomicU32,
    running_buffer: Mutex<[f32; 16]>,
    last_buffer_index: AtomicU8,
    audio_ports: Arc<Mutex<AudioPorts>>,
}

impl<'a> TrackClip for MidiClip<'a> {
    fn get_at_global_time(&self, global_time: u32) -> f32 {
        let last_global_time = self.last_global_time.load(SeqCst);
        let mut last_buffer_index = self.last_buffer_index.load(SeqCst);

        if last_global_time != global_time {
            if global_time != last_global_time + 1
                || self.pattern.lock().unwrap().dirty.clone() != DirtyEvent::None
            {
                self.last_buffer_index.store(15, SeqCst);
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
        self.global_start
    }

    fn get_global_end(&self) -> u32 {
        self.global_end
    }
}

impl<'a> MidiClip<'a> {
    pub fn new(pattern: Arc<Mutex<MidiPattern<'a>>>) -> Self {
        let len = pattern.lock().unwrap().len();
        Self {
            pattern,
            global_start: 0,
            global_end: len,
            pattern_start: 0,
            started_notes: Mutex::new(Vec::new()),
            last_global_time: AtomicU32::new(0),
            running_buffer: Mutex::new([0.0; 16]),
            last_buffer_index: AtomicU8::new(15),
            audio_ports: Arc::new(Mutex::new(AudioPorts::with_capacity(2, 1))),
        }
    }

    pub fn push(&self, note: &Arc<MidiNote<'a>>) {
        self.pattern.lock().unwrap().push(note);
    }

    pub fn remove(&self, note: &Arc<MidiNote<'a>>) {
        self.pattern.lock().unwrap().remove(note);
    }

    pub fn replace(&self, note: &Arc<MidiNote<'a>>, new_note: &Arc<MidiNote<'a>>) {
        self.pattern.lock().unwrap().replace(note, new_note);
    }

    pub fn trim_start(&mut self, samples: i32) {
        if samples < 0 {
            let samples = -samples as u32;
            let samples = min(samples, self.global_start);
            let samples = min(samples, self.pattern_start);

            self.global_start -= samples;
            self.pattern_start -= samples;
        } else {
            let samples = samples as u32;
            let samples = min(samples, self.global_end - self.global_start);

            self.global_start += samples;
            self.pattern_start += samples;
        }
    }

    pub fn trim_end(&mut self, samples: i32) {
        if samples < 0 {
            let samples = -samples as u32;
            let samples = min(samples, self.global_end - self.global_start);

            self.global_end -= samples;
        } else {
            let samples = samples as u32;
            let samples = min(
                samples,
                self.pattern.lock().unwrap().len() - self.pattern_start,
            );

            self.global_end += samples;
        }
    }

    pub fn move_by(&mut self, samples: i32) {
        if samples < 0 {
            let samples = -samples as u32;
            let samples = min(samples, self.global_end - self.global_start);

            self.global_start -= samples;
            self.global_end -= samples;
        } else {
            let samples = samples as u32;

            self.global_start += samples;
            self.global_end += samples;
        }
    }

    fn refresh_buffer(&self, global_time: u32) {
        let buffer = self.get_input_events(global_time);

        self.pattern
            .lock()
            .unwrap()
            .plugin_sender
            .send(PluginThreadMessage::ProcessAudio(
                [[0.0; 8]; 2],
                self.audio_ports.clone(),
                self.audio_ports.clone(),
                buffer,
                EventBuffer::new(),
            ))
            .unwrap();

        if let HostThreadMessage::AudioProcessed(buffers, _) = self
            .pattern
            .lock()
            .unwrap()
            .host_receiver
            .lock()
            .unwrap()
            .recv()
            .unwrap()
        {
            (0..16).step_by(2).for_each(|i| {
                self.running_buffer.lock().unwrap()[i] = buffers[0][i];
                self.running_buffer.lock().unwrap()[i + 1] = buffers[1][i];
            });

            self.pattern.lock().unwrap().clear_dirty();
        };
    }

    fn get_input_events(&self, global_time: u32) -> EventBuffer {
        let mut buffer = EventBuffer::new();

        self.pattern
            .lock()
            .unwrap()
            .plugin_sender
            .send(PluginThreadMessage::GetCounter)
            .unwrap();
        if let HostThreadMessage::Counter(plugin_counter) = self
            .pattern
            .lock()
            .unwrap()
            .host_receiver
            .lock()
            .unwrap()
            .recv()
            .unwrap()
        {
            if global_time == self.global_end {
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

                return buffer;
            }

            let dirty = self.pattern.lock().unwrap().dirty.clone();
            match dirty {
                DirtyEvent::None => {
                    let last_global_time = self.last_global_time.load(SeqCst);
                    if global_time != last_global_time + 1 {
                        self.jump_refresh(&mut buffer, global_time, plugin_counter);
                    }
                }
                DirtyEvent::NoteAdded => {
                    self.note_add_refresh(&mut buffer, global_time, plugin_counter);
                }
                DirtyEvent::NoteRemoved => {
                    self.note_remove_refresh(&mut buffer, global_time, plugin_counter);
                }
                DirtyEvent::NoteReplaced => {
                    self.note_remove_refresh(&mut buffer, global_time, plugin_counter);
                    self.note_add_refresh(&mut buffer, global_time, plugin_counter);
                }
            }

            let offset = plugin_counter + self.pattern_start;

            self.pattern
                .lock()
                .unwrap()
                .notes
                .iter()
                .filter(|midi_note| {
                    self.pattern_start + midi_note.local_start >= global_time
                        && self.pattern_start + midi_note.local_start < global_time + 16
                })
                .for_each(|midi_note| {
                    // notes that start during the running buffer
                    if let MidiMessage::NoteOn(channel, note, velocity) = midi_note.note {
                        buffer.push(&NoteOnEvent::new(
                            midi_note.local_start + offset,
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
                .filter(|(_, note)| self.pattern_start + note.local_end < global_time + 16)
                .for_each(|(index, midi_note)| {
                    // notes that end before the running buffer ends
                    if let MidiMessage::NoteOn(channel, note, velocity) = midi_note.note {
                        buffer.push(&NoteOffEvent::new(
                            midi_note.local_end + offset,
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
        let offset = plugin_counter + self.pattern_start;
        self.started_notes.lock().unwrap().iter().for_each(|note| {
            // stop all started notes
            if let MidiMessage::NoteOn(channel, note, velocity) = note.note {
                buffer.push(&NoteOffEvent::new(
                    global_time + offset,
                    Pckn::new(0u8, channel.index(), note as u16, Match::All),
                    u8::from(velocity) as f64 / (u8::from(Velocity::MAX) as f64),
                ));
            };
        });

        self.started_notes.lock().unwrap().clear();

        self.pattern
            .lock()
            .unwrap()
            .notes
            .iter()
            .filter(|note| {
                self.pattern_start + note.local_start <= global_time
                    && self.pattern_start + note.local_end > global_time
            })
            .for_each(|note| {
                // start all notes that would be currently playing
                if let MidiMessage::NoteOn(channel, note, velocity) = note.note {
                    buffer.push(&NoteOnEvent::new(
                        global_time + offset,
                        Pckn::new(0u8, channel.index(), note as u16, Match::All),
                        u8::from(velocity) as f64 / (u8::from(Velocity::MAX) as f64),
                    ));
                };
                self.started_notes.lock().unwrap().push(note.clone());
            });
    }

    fn note_add_refresh(&self, buffer: &mut EventBuffer, global_time: u32, plugin_counter: u32) {
        let offset = plugin_counter + self.pattern_start;
        self.pattern
            .lock()
            .unwrap()
            .notes
            .iter()
            .filter(|note| !self.started_notes.lock().unwrap().contains(note))
            .filter(|note| {
                self.pattern_start + note.local_start <= global_time
                    && self.pattern_start + note.local_end > global_time
            })
            .for_each(|note| {
                // start all new notes that would be currently playing
                if let MidiMessage::NoteOn(channel, note, velocity) = note.note {
                    buffer.push(&NoteOnEvent::new(
                        global_time + offset,
                        Pckn::new(0u8, channel.index(), note as u16, Match::All),
                        u8::from(velocity) as f64 / (u8::from(Velocity::MAX) as f64),
                    ));
                };
                self.started_notes.lock().unwrap().push(note.clone());
            });
    }

    fn note_remove_refresh(&self, buffer: &mut EventBuffer, global_time: u32, plugin_counter: u32) {
        let offset = plugin_counter + self.pattern_start;
        let mut indices = Vec::new();

        self.started_notes
            .lock()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, note)| !self.pattern.lock().unwrap().notes.contains(note))
            .for_each(|(index, note)| {
                // stop all started notes that are no longer in the pattern
                if let MidiMessage::NoteOn(channel, note, velocity) = note.note {
                    buffer.push(&NoteOffEvent::new(
                        global_time + offset,
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
}
