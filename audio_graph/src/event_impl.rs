use std::fmt::Debug;

pub trait EventImpl: Copy + Debug + Send + Sync {
	#[must_use]
	fn time(self) -> usize;
	#[must_use]
	fn with_time(self, to: usize) -> Self;
}
