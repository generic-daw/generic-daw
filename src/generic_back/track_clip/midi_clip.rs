use crate::{
    generic_back::{meter::Meter, position::Position},
    generic_front::drawable::{Drawable, TimelinePosition, TimelineScale},
};
use clack_host::{
    events::{
        event_types::{NoteOffEvent, NoteOnEvent},
        Match,
    },
    prelude::*,
};
use generic_clap_host::{host::HostThreadMessage, main_thread::MainThreadMessage};
use iced::{widget::canvas::Frame, Theme};
use std::sync::{
    atomic::{AtomicU32, AtomicU8, Ordering::SeqCst},
    mpsc::{Receiver, Sender},
    Arc, Mutex, RwLock,
};
use wmidi::{Channel, Note, Velocity};

#[derive(PartialEq, Eq)]
pub struct MidiNote {
    pub channel: Channel,
    pub note: Note,
    pub velocity: Velocity,
    pub local_start: u32,
    pub local_end: u32,
}

#[derive(PartialEq, Eq)]
enum DirtyEvent {
    // can we reasonably assume that only one of these will happen per sample?
    None,
    NoteAdded,
    NoteRemoved,
    NoteReplaced,
}

pub struct MidiPattern {
    pub notes: Vec<Arc<MidiNote>>,
    dirty: DirtyEvent,
    plugin_sender: Sender<MainThreadMessage>,
    host_receiver: RwLock<Receiver<HostThreadMessage>>,
}

impl MidiPattern {
    const fn new(
        plugin_sender: Sender<MainThreadMessage>,
        host_receiver: Receiver<HostThreadMessage>,
    ) -> Self {
        Self {
            notes: Vec::new(),
            dirty: DirtyEvent::None,
            plugin_sender,
            host_receiver: RwLock::new(host_receiver),
        }
    }

    fn len(&self) -> u32 {
        self.notes
            .iter()
            .map(|note| note.local_end)
            .max()
            .unwrap_or(0)
    }

    fn clear_dirty(&mut self) {
        self.dirty = DirtyEvent::None;
    }

    fn push(&mut self, note: Arc<MidiNote>) {
        self.notes.push(note);
        self.dirty = DirtyEvent::NoteAdded;
    }

    fn remove(&mut self, note: &Arc<MidiNote>) {
        let pos = self.notes.iter().position(|n| n == note).unwrap();
        self.notes.remove(pos);
        self.dirty = DirtyEvent::NoteRemoved;
    }

    fn replace(&mut self, note: &Arc<MidiNote>, new_note: Arc<MidiNote>) {
        let pos = self.notes.iter().position(|n| n == note).unwrap();
        self.notes[pos] = new_note;
        self.dirty = DirtyEvent::NoteReplaced;
    }
}

pub struct MidiClip {
    pub pattern: Arc<Mutex<MidiPattern>>,
    global_start: Position,
    global_end: Position,
    pattern_start: Position,
    started_notes: RwLock<Vec<Arc<MidiNote>>>,
    last_global_time: AtomicU32,
    running_buffer: RwLock<[f32; 16]>,
    last_buffer_index: AtomicU8,
    audio_ports: Arc<RwLock<AudioPorts>>,
}

impl MidiClip {
    pub fn get_at_global_time(&self, global_time: u32, meter: &Meter) -> f32 {
        let last_global_time = self.last_global_time.load(SeqCst);
        let mut last_buffer_index = self.last_buffer_index.load(SeqCst);

        if last_global_time != global_time {
            if global_time != last_global_time + 1
                || self.pattern.lock().unwrap().dirty != DirtyEvent::None
            {
                self.last_buffer_index.store(15, SeqCst);
            }

            last_buffer_index = (last_buffer_index + 1) % 16;
            if last_buffer_index == 0 {
                self.refresh_buffer(global_time, meter);
            }

            self.last_global_time.store(global_time, SeqCst);
            self.last_buffer_index.store(last_buffer_index, SeqCst);
        }

        self.running_buffer.read().unwrap()[last_buffer_index as usize]
    }

    pub const fn get_global_start(&self) -> Position {
        self.global_start
    }

    pub const fn get_global_end(&self) -> Position {
        self.global_end
    }

    pub fn trim_start_to(&mut self, clip_start: Position) {
        self.pattern_start = clip_start;
    }

    pub fn trim_end_to(&mut self, global_end: Position) {
        self.global_end = global_end;
    }

    pub fn move_start_to(&mut self, global_start: Position) {
        match self.global_start.cmp(&global_start) {
            std::cmp::Ordering::Less => {
                self.global_end += global_start - self.global_start;
            }
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Greater => {
                self.global_end += self.global_start - global_start;
            }
        }
        self.global_start = global_start;
    }

    pub fn new(pattern: Arc<Mutex<MidiPattern>>, meter: &Meter) -> Self {
        let len = pattern.lock().unwrap().len();
        Self {
            pattern,
            global_start: Position::new(0, 0),
            global_end: Position::from_interleaved_samples(len, meter),
            pattern_start: Position::new(0, 0),
            started_notes: RwLock::new(Vec::new()),
            last_global_time: AtomicU32::new(0),
            running_buffer: RwLock::new([0.0; 16]),
            last_buffer_index: AtomicU8::new(15),
            audio_ports: Arc::new(RwLock::new(AudioPorts::with_capacity(2, 1))),
        }
    }

    pub fn push(&self, note: Arc<MidiNote>) {
        self.pattern.lock().unwrap().push(note);
    }

    pub fn remove(&self, note: &Arc<MidiNote>) {
        self.pattern.lock().unwrap().remove(note);
    }

    pub fn replace(&self, note: &Arc<MidiNote>, new_note: Arc<MidiNote>) {
        self.pattern.lock().unwrap().replace(note, new_note);
    }

    fn refresh_buffer(&self, global_time: u32, meter: &Meter) {
        let buffer = self.get_input_events(global_time, meter);

        let input_audio = [vec![0.0; 8], vec![0.0; 8]];

        self.pattern
            .lock()
            .unwrap()
            .plugin_sender
            .send(MainThreadMessage::ProcessAudio(
                input_audio,
                self.audio_ports.clone(),
                self.audio_ports.clone(),
                buffer,
            ))
            .unwrap();

        let message = self
            .pattern
            .lock()
            .unwrap()
            .host_receiver
            .read()
            .unwrap()
            .recv()
            .unwrap();
        if let HostThreadMessage::AudioProcessed(buffers, _) = message {
            (0..16).step_by(2).for_each(|i| {
                self.running_buffer.write().unwrap()[i] = buffers[0][i];
                self.running_buffer.write().unwrap()[i + 1] = buffers[1][i];
            });

            self.pattern.lock().unwrap().clear_dirty();
        };
    }

    fn get_input_events(&self, global_time: u32, meter: &Meter) -> EventBuffer {
        let mut buffer = EventBuffer::new();

        self.pattern
            .lock()
            .unwrap()
            .plugin_sender
            .send(MainThreadMessage::GetCounter)
            .unwrap();

        let message = self
            .pattern
            .lock()
            .unwrap()
            .host_receiver
            .read()
            .unwrap()
            .recv()
            .unwrap();
        if let HostThreadMessage::Counter(steady_time) = message {
            if global_time == self.global_end.in_interleaved_samples(meter) {
                self.started_notes.read().unwrap().iter().for_each(|note| {
                    // stop all started notes
                    buffer.push(&NoteOffEvent::new(
                        u32::try_from(steady_time).unwrap(),
                        Pckn::new(0u8, note.channel.index(), note.note as u16, Match::All),
                        f64::from(u8::from(note.velocity)) / f64::from(u8::from(Velocity::MAX)),
                    ));
                });

                self.started_notes.write().unwrap().clear();

                return buffer;
            }

            match self.pattern.lock().unwrap().dirty {
                DirtyEvent::None => {
                    let last_global_time = self.last_global_time.load(SeqCst);
                    if global_time != last_global_time + 1
                        || (self.pattern_start.in_interleaved_samples(meter) != 0
                            && global_time == self.global_start.in_interleaved_samples(meter))
                    {
                        self.jump_events_refresh(&mut buffer, global_time, steady_time, meter);
                    }
                }
                DirtyEvent::NoteAdded => {
                    self.note_add_events_refresh(&mut buffer, global_time, steady_time, meter);
                }
                DirtyEvent::NoteRemoved => {
                    self.note_remove_events_refresh(&mut buffer, steady_time);
                }
                DirtyEvent::NoteReplaced => {
                    self.note_remove_events_refresh(&mut buffer, steady_time);
                    self.note_add_events_refresh(&mut buffer, global_time, steady_time, meter);
                }
            }

            self.events_refresh(&mut buffer, global_time, steady_time, meter);
        }

        buffer
    }

    fn events_refresh(
        &self,
        buffer: &mut EventBuffer,
        global_time: u32,
        steady_time: u64,
        meter: &Meter,
    ) {
        let offset = (self.global_start - self.pattern_start).in_interleaved_samples(meter);

        self.pattern
            .lock()
            .unwrap()
            .notes
            .iter()
            .filter(|midi_note| {
                offset + midi_note.local_start >= global_time
                    && offset + midi_note.local_start < global_time + 16
            })
            .for_each(|note| {
                // notes that start during the running buffer
                buffer.push(&NoteOnEvent::new(
                    note.local_start - global_time + u32::try_from(steady_time).unwrap(),
                    Pckn::new(0u8, note.channel.index(), note.note as u16, Match::All),
                    f64::from(u8::from(note.velocity)) / f64::from(u8::from(Velocity::MAX)),
                ));
                self.started_notes.write().unwrap().push(note.clone());
            });

        let mut indices = Vec::new();

        self.started_notes
            .read()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, note)| {
                (self.global_start - self.pattern_start).in_interleaved_samples(meter)
                    + note.local_end
                    < global_time + 16
            })
            .for_each(|(index, note)| {
                // notes that end before the running buffer ends
                buffer.push(&NoteOffEvent::new(
                    note.local_end - global_time + u32::try_from(steady_time).unwrap(),
                    Pckn::new(0u8, note.channel.index(), note.note as u16, Match::All),
                    f64::from(u8::from(note.velocity)) / f64::from(u8::from(Velocity::MAX)),
                ));
                indices.push(index);
            });

        indices.iter().rev().for_each(|i| {
            self.started_notes.write().unwrap().remove(*i);
        });
    }

    fn jump_events_refresh(
        &self,
        buffer: &mut EventBuffer,
        global_time: u32,
        steady_time: u64,
        meter: &Meter,
    ) {
        let offset = (self.global_start - self.pattern_start).in_interleaved_samples(meter);

        self.started_notes.read().unwrap().iter().for_each(|note| {
            // stop all started notes
            buffer.push(&NoteOffEvent::new(
                u32::try_from(steady_time).unwrap(),
                Pckn::new(0u8, note.channel.index(), note.note as u16, Match::All),
                f64::from(u8::from(note.velocity)) / f64::from(u8::from(Velocity::MAX)),
            ));
        });

        self.started_notes.write().unwrap().clear();

        self.pattern
            .lock()
            .unwrap()
            .notes
            .iter()
            .filter(|note| {
                offset + note.local_start <= global_time && offset + note.local_end > global_time
            })
            .for_each(|note| {
                // start all notes that would be currently playing
                buffer.push(&NoteOnEvent::new(
                    u32::try_from(steady_time).unwrap(),
                    Pckn::new(0u8, note.channel.index(), note.note as u16, Match::All),
                    f64::from(u8::from(note.velocity)) / f64::from(u8::from(Velocity::MAX)),
                ));
                self.started_notes.write().unwrap().push(note.clone());
            });
    }

    fn note_add_events_refresh(
        &self,
        buffer: &mut EventBuffer,
        global_time: u32,
        steady_time: u64,
        meter: &Meter,
    ) {
        let offset = (self.global_start - self.pattern_start).in_interleaved_samples(meter);

        self.pattern
            .lock()
            .unwrap()
            .notes
            .iter()
            .filter(|note| !self.started_notes.read().unwrap().contains(note))
            .filter(|note| {
                offset + note.local_start <= global_time && offset + note.local_end > global_time
            })
            .for_each(|note| {
                // start all new notes that would be currently playing
                buffer.push(&NoteOnEvent::new(
                    u32::try_from(steady_time).unwrap(),
                    Pckn::new(0u8, note.channel.index(), note.note as u16, Match::All),
                    f64::from(u8::from(note.velocity)) / f64::from(u8::from(Velocity::MAX)),
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
            .filter(|(_, note)| !self.pattern.lock().unwrap().notes.contains(note))
            .for_each(|(index, note)| {
                // stop all started notes that are no longer in the pattern
                buffer.push(&NoteOffEvent::new(
                    u32::try_from(steady_time).unwrap(),
                    Pckn::new(0u8, note.channel.index(), note.note as u16, Match::All),
                    f64::from(u8::from(note.velocity)) / f64::from(u8::from(Velocity::MAX)),
                ));
                indices.push(index);
            });

        indices.iter().rev().for_each(|i| {
            self.started_notes.write().unwrap().remove(*i);
        });
    }
}

impl Drawable for MidiClip {
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
