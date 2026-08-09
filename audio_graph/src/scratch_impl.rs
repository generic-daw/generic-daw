use std::num::NonZero;

pub trait ScratchImpl: Send + Sync {
	#[must_use]
	fn new(max_frames: NonZero<u32>) -> Self;
	#[must_use]
	fn get_audio(&mut self) -> &mut [[f32; 2]];
}
