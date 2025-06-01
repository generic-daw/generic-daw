use std::fmt::Debug;

pub trait EventImpl: Copy + Debug {
    /// get the time of the event relative to the start of the last callback, in samples
    #[must_use]
    fn time(self) -> usize;
    /// set the time of the event relative to the start of the last callback, in samples
    #[must_use]
    fn with_time(self, to: usize) -> Self;
}
