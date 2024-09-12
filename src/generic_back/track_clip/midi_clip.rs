pub mod midi_pattern;

use crate::generic_back::{arrangement::Arrangement, position::Position};
use midi_pattern::{MidiNote, MidiPattern};
use std::sync::Arc;

pub struct MidiClip {
    pub pattern: MidiPattern,
    global_start: Position,
    global_end: Position,
    pattern_start: Position,
    pub arrangement: Arc<Arrangement>,
}

impl MidiClip {
    #[expect(dead_code)]
    pub fn new(pattern: MidiPattern, arrangement: Arc<Arrangement>) -> Self {
        let len = pattern.len();
        Self {
            pattern,
            global_start: Position::new(0, 0),
            global_end: Position::from_interleaved_samples(len, &arrangement.meter),
            pattern_start: Position::new(0, 0),
            arrangement,
        }
    }

    pub const fn get_global_start(&self) -> Position {
        self.global_start
    }

    pub const fn get_global_end(&self) -> Position {
        self.global_end
    }

    #[expect(dead_code)]
    pub fn trim_start_to(&mut self, clip_start: Position) {
        self.pattern_start = clip_start;
    }

    #[expect(dead_code)]
    pub fn trim_end_to(&mut self, global_end: Position) {
        self.global_end = global_end;
    }

    #[expect(dead_code)]
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

    pub fn get_global_midi(&self) -> Vec<Arc<MidiNote>> {
        let global_start = self
            .global_start
            .in_interleaved_samples(&self.arrangement.meter);
        let global_end = self
            .global_end
            .in_interleaved_samples(&self.arrangement.meter);
        self.pattern
            .notes
            .iter()
            .map(|note| {
                Arc::new(MidiNote {
                    channel: note.channel,
                    note: note.note,
                    velocity: note.velocity,
                    local_start: (note.local_start + global_start - global_end)
                        .clamp(global_start, global_end),
                    local_end: (note.local_end + global_start - global_end)
                        .clamp(global_start, global_end),
                })
            })
            .filter(|note| note.local_start != note.local_end)
            .collect()
    }
}
