use super::Track;
use crate::{
    generic_back::{
        meter::Meter,
        position::Position,
        track_clip::midi_clip::{AtomicDirtyEvent, DirtyEvent, MidiClip, MidiNote},
    },
    generic_front::drawable::{Drawable, TimelinePosition, TimelineScale},
};
use clack_host::{
    events::{
        event_types::{NoteOffEvent, NoteOnEvent},
        Match, Pckn,
    },
    prelude::{AudioPorts, EventBuffer},
};
use generic_clap_host::{host::HostThreadMessage, main_thread::MainThreadMessage};
use iced::{widget::canvas::Frame, Theme};
use std::sync::{
    atomic::{AtomicU32, AtomicU8, Ordering::SeqCst},
    mpsc::{Receiver, Sender},
    Arc, Mutex, RwLock,
};

pub struct MidiTrack {
    pub clips: RwLock<Vec<Arc<MidiClip>>>,
    plugin_sender: Sender<MainThreadMessage>,
    host_receiver: Mutex<Receiver<HostThreadMessage>>,
    global_midi_cache: RwLock<Vec<Arc<MidiNote>>>,
    dirty: Arc<AtomicDirtyEvent>,
    started_notes: RwLock<Vec<Arc<MidiNote>>>,
    last_global_time: AtomicU32,
    running_buffer: RwLock<[f32; 16]>,
    last_buffer_index: AtomicU8,
    audio_ports: Arc<RwLock<AudioPorts>>,
}

impl MidiTrack {
    pub fn new(
        plugin_sender: Sender<MainThreadMessage>,
        host_receiver: Receiver<HostThreadMessage>,
    ) -> Self {
        Self {
            clips: RwLock::new(Vec::new()),
            plugin_sender,
            host_receiver: Mutex::new(host_receiver),
            global_midi_cache: RwLock::new(Vec::new()),
            dirty: Arc::new(AtomicDirtyEvent::new(DirtyEvent::None)),
            started_notes: RwLock::new(Vec::new()),
            last_global_time: AtomicU32::new(0),
            running_buffer: RwLock::new([0.0; 16]),
            last_buffer_index: AtomicU8::new(15),
            audio_ports: Arc::new(RwLock::new(AudioPorts::with_capacity(2, 1))),
        }
    }

    fn refresh_global_midi(&self, meter: &Meter) {
        *self.global_midi_cache.write().unwrap() = self
            .clips
            .read()
            .unwrap()
            .iter()
            .flat_map(|clip| clip.get_global_midi(meter))
            .collect();
    }

    fn refresh_buffer(&self, global_time: u32) {
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

impl Track for MidiTrack {
    fn get_at_global_time(&self, global_time: u32, meter: &Meter) -> f32 {
        let last_global_time = self.last_global_time.load(SeqCst);
        let mut last_buffer_index = self.last_buffer_index.load(SeqCst);

        if last_global_time != global_time {
            if global_time != last_global_time + 1 || self.dirty.load(SeqCst) != DirtyEvent::None {
                self.refresh_global_midi(meter);
                self.last_buffer_index.store(15, SeqCst);
            }

            last_buffer_index = (last_buffer_index + 1) % 16;
            if last_buffer_index == 0 {
                self.refresh_buffer(global_time);
            }

            self.last_global_time.store(global_time, SeqCst);
            self.last_buffer_index.store(last_buffer_index, SeqCst);
        }

        self.running_buffer.read().unwrap()[last_buffer_index as usize]
    }

    fn get_global_end(&self) -> Position {
        self.clips
            .read()
            .unwrap()
            .iter()
            .map(|clip| clip.get_global_end())
            .max()
            .unwrap_or(Position::new(0, 0))
    }
}

impl Drawable for MidiTrack {
    fn draw(
        &self,
        _frame: &mut Frame,
        _scale: TimelineScale,
        _offset: &TimelinePosition,
        _meter: &Meter,
        _theme: &Theme,
    ) {
        unimplemented!()
    }
}
