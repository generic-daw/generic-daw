use std::fmt::Debug;

pub trait EventImpl: Debug + Send + Sync {
	#[must_use]
	fn time(&self) -> usize;
	#[must_use]
	fn at(&self, at: usize) -> Self;
}
