#[derive(PartialEq)]
pub struct MidiNote {
    pub channel: u8,
    pub note: u16,
    /// between 0.0 and 1.0
    pub velocity: f64,
    pub local_start: u32,
    pub local_end: u32,
}
