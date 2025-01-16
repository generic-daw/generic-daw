#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MidiNote {
    pub channel: u8,
    pub note: u16,
    /// between 0.0 and 1.0
    pub velocity: f64,
    /// start time of the note, relative to the beginning of the `MidiPattern` it belongs to
    pub local_start: usize,
    /// end time of the note, relative to the beginning of the `MidiPattern` it belongs to
    pub local_end: usize,
}
