use crate::DelayLine;

#[derive(Clone, Debug)]
pub struct AllPass {
	delay_line: DelayLine,
	feedback: f32,
}

impl AllPass {
	#[must_use]
	pub fn new(len: usize) -> Self {
		Self {
			delay_line: DelayLine::new(len),
			feedback: Default::default(),
		}
		.feedback(0.5)
	}

	#[must_use]
	pub const fn feedback(mut self, feedback: f32) -> Self {
		self.feedback = feedback;
		self
	}

	#[must_use]
	pub fn tick(&mut self, input: f32) -> f32 {
		self.delay_line
			.write(self.feedback.mul_add(self.delay_line.read(), input))
			- input
	}

	pub fn process(&mut self, audio: &mut [f32]) {
		for sample in audio {
			*sample = self.tick(*sample);
		}
	}
}
