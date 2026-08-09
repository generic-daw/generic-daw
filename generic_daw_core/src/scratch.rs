use crate::Event;
use audio_graph::ScratchImpl;
use std::num::NonZero;
use utils::{NoDebug, boxed_slice};

#[derive(Debug)]
pub struct Scratch {
	pub audio: NoDebug<Box<[[f32; 2]]>>,
	pub events: Vec<Event>,
}

impl ScratchImpl for Scratch {
	fn new(max_frames: NonZero<u32>) -> Self {
		Self {
			audio: boxed_slice![[0.0; 2]; max_frames.get() as usize].into(),
			events: Vec::new(),
		}
	}

	fn get_audio(&mut self) -> &mut [[f32; 2]] {
		&mut self.audio
	}
}
